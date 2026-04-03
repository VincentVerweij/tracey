//! Tauri commands for managing classification rules and testing the engine.

use tauri::{State, Manager};
use serde::Deserialize;
use ulid::Ulid;
use chrono::Utc;

use crate::commands::AppState;
use crate::services::classification::{
    self,
    feature_extractor::extract,
    heuristic::HeuristicRule,
    trainer,
    ClassificationResult,
};

// ── Rules CRUD ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn classification_rules_get(state: State<'_, AppState>) -> Result<Vec<HeuristicRule>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let json: Option<String> = conn.query_row(
        "SELECT classification_rules_json FROM user_preferences LIMIT 1",
        [],
        |r| r.get(0),
    ).ok().flatten();
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default())
}

#[tauri::command]
pub fn classification_rules_update(
    state: State<'_, AppState>,
    rules: Vec<HeuristicRule>,
) -> Result<(), String> {
    let json = serde_json::to_string(&rules).map_err(|e| e.to_string())?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE user_preferences SET classification_rules_json = ?1 WHERE id = 1",
            rusqlite::params![json],
        ).map_err(|e| e.to_string())?;
    }
    // Refresh in-memory rules cache
    let mut cs = state.classification_state.lock().map_err(|e| e.to_string())?;
    cs.rules = rules;
    Ok(())
}

// ── Test-classify (dev/debug) ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ClassifyTestRequest {
    pub process_name: String,
    pub window_title: String,
    pub ocr_text: Option<String>,
}

#[tauri::command]
pub fn classification_classify_test(
    state: State<'_, AppState>,
    request: ClassifyTestRequest,
) -> Result<ClassificationResult, String> {
    let features = extract(&request.process_name, &request.window_title, request.ocr_text.as_deref());
    let cs = state.classification_state.lock().map_err(|e| e.to_string())?;
    Ok(classification::classify(&features, &cs.rules, cs.model.as_ref()))
}

// ── Labeled sample submit ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LabeledSampleSubmitRequest {
    pub process_name: String,
    pub window_title: String,
    pub ocr_text: Option<String>,
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub source: String, // "user_confirmed" | "user_corrected"
}

#[tauri::command]
pub fn labeled_sample_submit(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: LabeledSampleSubmitRequest,
) -> Result<(), String> {
    let features = extract(&request.process_name, &request.window_title, request.ocr_text.as_deref());
    let id = Ulid::new().to_string().to_lowercase();
    let now = Utc::now().to_rfc3339();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    let (should_retrain, sample_count) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO labeled_samples \
             (id, feature_text, process_name, window_title, client_id, project_id, task_id, \
              source, device_id, created_at, modified_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
            rusqlite::params![
                id, features.combined_text,
                features.process_name, features.window_title,
                request.client_id, request.project_id, request.task_id,
                request.source, device_id, now,
            ],
        ).map_err(|e| e.to_string())?;

        let total = trainer::count_samples(&conn);
        let cs = state.classification_state.lock().map_err(|e| e.to_string())?;
        let new_since_last = total - cs.sample_count_at_last_train;
        (new_since_last >= 10 && total >= trainer::PHASE2_MIN_SAMPLES, total)
    }; // db lock released

    if should_retrain {
        // AppHandle is Clone + 'static — safe to move into spawn
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let conn_guard = state.db.lock();
            if let Ok(conn) = conn_guard {
                if let Some(model) = trainer::retrain(&conn) {
                    drop(conn); // release db lock before locking classification_state
                    if let Ok(mut cs) = state.classification_state.lock() {
                        cs.model = Some(model);
                        cs.sample_count_at_last_train = sample_count;
                        log::info!("[classification] Model retrained and loaded");
                    }
                }
            }
        });
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("
            CREATE TABLE user_preferences (
                id INTEGER PRIMARY KEY, classification_rules_json TEXT
            );
            INSERT INTO user_preferences (id) VALUES (1);
            CREATE TABLE labeled_samples (
                id TEXT PRIMARY KEY, feature_text TEXT NOT NULL,
                process_name TEXT NOT NULL, window_title TEXT NOT NULL,
                client_id TEXT, project_id TEXT, task_id TEXT,
                source TEXT NOT NULL, device_id TEXT NOT NULL,
                created_at TEXT NOT NULL, modified_at TEXT NOT NULL DEFAULT (datetime('now')),
                synced_at TEXT
            );
            CREATE TABLE classifier_model (
                id TEXT PRIMARY KEY, model_json TEXT NOT NULL,
                trained_at TEXT NOT NULL, sample_count INTEGER NOT NULL,
                device_id TEXT NOT NULL
            );
        ").unwrap();
        conn
    }

    #[test]
    fn rules_roundtrip_empty() {
        let conn = setup_db();
        let json: Option<String> = conn.query_row(
            "SELECT classification_rules_json FROM user_preferences LIMIT 1",
            [], |r| r.get(0),
        ).ok().flatten();
        let rules: Vec<HeuristicRule> = json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        assert!(rules.is_empty(), "Should be empty on fresh DB");
    }

    #[test]
    fn rules_roundtrip_write_and_read() {
        let conn = setup_db();
        let rule = HeuristicRule {
            app_contains: Some("code".to_string()),
            title_contains: Some("tracey".to_string()),
            client_id: None,
            project_id: Some("proj-1".to_string()),
            task_id: None,
        };
        let json = serde_json::to_string(&[&rule]).unwrap();
        conn.execute(
            "UPDATE user_preferences SET classification_rules_json = ?1 WHERE id = 1",
            rusqlite::params![json],
        ).unwrap();

        let stored: Option<String> = conn.query_row(
            "SELECT classification_rules_json FROM user_preferences LIMIT 1",
            [], |r| r.get(0),
        ).ok().flatten();
        let loaded: Vec<HeuristicRule> = stored
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].project_id.as_deref(), Some("proj-1"));
    }

    #[test]
    fn labeled_sample_insert_includes_modified_at() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO labeled_samples \
             (id,feature_text,process_name,window_title,client_id,project_id,task_id,source,device_id,created_at,modified_at) \
             VALUES ('s1','feat','app','title',NULL,'p1',NULL,'user_confirmed','dev',datetime('now'),datetime('now'))",
            [],
        ).unwrap();
        let modified_at: String = conn.query_row(
            "SELECT modified_at FROM labeled_samples WHERE id = 's1'", [],
            |r| r.get(0),
        ).unwrap();
        assert!(!modified_at.is_empty(), "modified_at should be set");
    }
}