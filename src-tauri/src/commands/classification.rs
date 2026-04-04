//! Tauri commands for managing classification rules and testing the engine.

use tauri::{State, Manager};
use serde::{Deserialize, Serialize};
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

// ── Active learning: user confirms or corrects a classification ───────────────

#[derive(Deserialize)]
pub struct ClassificationSubmitLabelRequest {
    pub war_id: String,
    pub event_id: String,
    pub process_name: String,
    pub window_title: String,
    pub ocr_text: Option<String>,
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub recorded_at: String,
    pub source: String, // "user_confirmed" | "user_corrected"
}

#[tauri::command]
pub fn classification_submit_label(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    request: ClassificationSubmitLabelRequest,
) -> Result<(), String> {
    let features = extract(&request.process_name, &request.window_title, request.ocr_text.as_deref());
    let sample_id = Ulid::new().to_string().to_lowercase();
    let entry_id = Ulid::new().to_string().to_lowercase();
    let now = Utc::now().to_rfc3339();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    let sample_count = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO labeled_samples \
             (id,feature_text,process_name,window_title,client_id,project_id,task_id,source,device_id,created_at,modified_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
            rusqlite::params![
                sample_id, features.combined_text,
                request.process_name, request.window_title,
                request.client_id, request.project_id, request.task_id,
                request.source, device_id, now,
            ],
        ).map_err(|e| e.to_string())?;

        conn.execute(
            "INSERT INTO time_entries \
             (id,description,started_at,ended_at,project_id,task_id,is_break,device_id,created_at,modified_at,source) \
             VALUES (?1,'',?2,?2,?3,?4,0,?5,?6,?6,'auto')",
            rusqlite::params![
                entry_id, request.recorded_at,
                request.project_id, request.task_id,
                device_id, now,
            ],
        ).map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE classification_events SET outcome = ?1 WHERE id = ?2",
            rusqlite::params![request.source, request.event_id],
        ).map_err(|e| e.to_string())?;

        conn.execute(
            "UPDATE window_activity_records SET classified_at = ?1 WHERE id = ?2",
            rusqlite::params![now, request.war_id],
        ).map_err(|e| e.to_string())?;

        trainer::count_samples(&conn)
    }; // db lock released

    // Remove from in-memory queue
    {
        let mut alq = state.active_learning_queue.lock().map_err(|e| e.to_string())?;
        alq.dequeue(&request.war_id);
    }

    // Trigger retrain if enough new samples
    let should_retrain = {
        let cs = state.classification_state.lock().map_err(|e| e.to_string())?;
        sample_count - cs.sample_count_at_last_train >= 10
            && sample_count >= trainer::PHASE2_MIN_SAMPLES
    };

    if should_retrain {
        tauri::async_runtime::spawn(async move {
            let state = app.state::<AppState>();
            let model_opt = {
                let conn_guard = state.db.lock();
                conn_guard.ok().and_then(|conn| trainer::retrain(&conn))
            }; // conn lock dropped
            if let Some(model) = model_opt {
                if let Ok(mut cs) = state.classification_state.lock() {
                    cs.model = Some(model);
                    cs.sample_count_at_last_train = sample_count;
                }; // semicolon drops lock temporary before state
            }
        });
    }

    Ok(())
}

// ── Active learning: user dismisses toast without answering ───────────────────

#[derive(Deserialize)]
pub struct ClassificationDismissRequest {
    pub war_id: String,
    pub pattern_key: String,
}

#[tauri::command]
pub fn classification_dismiss(
    state: State<'_, AppState>,
    request: ClassificationDismissRequest,
) -> Result<(), String> {
    let snooze_json = {
        let mut alq = state.active_learning_queue.lock().map_err(|e| e.to_string())?;
        alq.record_dismissal(&request.pattern_key);
        alq.dequeue(&request.war_id);
        serde_json::to_string(&alq.snooze_state()).map_err(|e| e.to_string())?
    }; // alq lock released

    let conn = state.db.lock().map_err(|e| e.to_string())?;
    conn.execute(
        "UPDATE user_preferences SET classification_snooze_json = ?1 WHERE id = 1",
        rusqlite::params![snooze_json],
    ).map_err(|e| e.to_string())?;

    Ok(())
}

#[cfg(test)]
mod submit_label_tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("
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
            CREATE TABLE time_entries (
                id TEXT PRIMARY KEY, description TEXT NOT NULL DEFAULT '',
                started_at TEXT NOT NULL, ended_at TEXT,
                project_id TEXT, task_id TEXT, is_break INTEGER NOT NULL DEFAULT 0,
                device_id TEXT NOT NULL, created_at TEXT NOT NULL, modified_at TEXT NOT NULL,
                source TEXT NOT NULL DEFAULT 'manual'
            );
            CREATE TABLE classification_events (
                id TEXT PRIMARY KEY NOT NULL, war_id TEXT NOT NULL,
                process_name TEXT NOT NULL, window_title TEXT NOT NULL,
                client_id TEXT, project_id TEXT, task_id TEXT,
                confidence REAL NOT NULL DEFAULT 0.0,
                classification_source TEXT NOT NULL DEFAULT 'unclassified',
                outcome TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL
            );
            CREATE TABLE window_activity_records (
                id TEXT PRIMARY KEY, process_name TEXT, window_title TEXT,
                recorded_at TEXT, classified_at TEXT
            );
            CREATE TABLE user_preferences (
                id INTEGER PRIMARY KEY, classification_snooze_json TEXT
            );
            INSERT INTO user_preferences (id) VALUES (1);
        ").unwrap();
        conn
    }

    #[test]
    fn sample_count_increases_after_insert() {
        let conn = setup_db();
        let before = trainer::count_samples(&conn);
        conn.execute(
            "INSERT INTO labeled_samples \
             (id,feature_text,process_name,window_title,source,device_id,created_at,modified_at) \
             VALUES ('s1','feat','app','title','user_confirmed','dev',datetime('now'),datetime('now'))",
            [],
        ).unwrap();
        let after = trainer::count_samples(&conn);
        assert_eq!(after, before + 1);
    }

    #[test]
    fn retrain_threshold_requires_10_new_samples_since_last_train() {
        let conn = setup_db();
        let sample_count_at_last_train = 5i64;
        for i in 0..9 {
            conn.execute(
                &format!("INSERT INTO labeled_samples \
                 (id,feature_text,process_name,window_title,source,device_id,created_at,modified_at) \
                 VALUES ('s{i}','feat','app','title','user_confirmed','dev',datetime('now'),datetime('now'))"),
                [],
            ).unwrap();
        }
        let total = trainer::count_samples(&conn);
        let should_retrain = (total - sample_count_at_last_train) >= 10
            && total >= trainer::PHASE2_MIN_SAMPLES;
        assert!(!should_retrain, "9 new samples should not trigger retrain");

        conn.execute(
            "INSERT INTO labeled_samples \
             (id,feature_text,process_name,window_title,source,device_id,created_at,modified_at) \
             VALUES ('s9','feat','app','title','user_confirmed','dev',datetime('now'),datetime('now'))",
            [],
        ).unwrap();
        let total = trainer::count_samples(&conn);
        let should_retrain = (total - sample_count_at_last_train) >= 10
            && total >= trainer::PHASE2_MIN_SAMPLES;
        assert_eq!(should_retrain, total >= trainer::PHASE2_MIN_SAMPLES);
    }

    #[test]
    fn dismiss_increments_snooze_count() {
        let mut alq = crate::services::active_learning_queue::ActiveLearningQueue::new();
        let key = "code|tracey";
        alq.record_dismissal(key);
        let state = alq.snooze_state();
        assert_eq!(state.get(key).map(|e| e.dismissed_count), Some(1));
        alq.record_dismissal(key);
        let state = alq.snooze_state();
        assert_eq!(state.get(key).map(|e| e.dismissed_count), Some(2));
    }
}

// ── Classification event list (for Classification page) ───────────────────────

#[derive(Serialize)]
pub struct ClassificationEventItem {
    pub id: String,
    pub war_id: String,
    pub process_name: String,
    pub window_title: String,
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub confidence: f64,
    pub classification_source: String,
    pub outcome: String,
    pub ocr_text: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct ClassificationEventListRequest {
    pub page: i64,
    pub page_size: i64,
}

#[derive(Serialize)]
pub struct ClassificationEventListResponse {
    pub items: Vec<ClassificationEventItem>,
    pub total: i64,
}

#[tauri::command]
pub fn classification_event_list(
    state: State<'_, AppState>,
    request: ClassificationEventListRequest,
) -> Result<ClassificationEventListResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let offset = request.page * request.page_size;

    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM classification_events",
        [],
        |r| r.get(0),
    ).unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT id, war_id, process_name, window_title, client_id, project_id, task_id, \
                confidence, classification_source, outcome, ocr_text, created_at \
         FROM classification_events \
         ORDER BY created_at DESC \
         LIMIT ?1 OFFSET ?2",
    ).map_err(|e| e.to_string())?;

    let items: Vec<ClassificationEventItem> = stmt.query_map(
        rusqlite::params![request.page_size, offset],
        |r| Ok(ClassificationEventItem {
            id:                    r.get(0)?,
            war_id:                r.get(1)?,
            process_name:          r.get(2)?,
            window_title:          r.get(3)?,
            client_id:             r.get(4)?,
            project_id:            r.get(5)?,
            task_id:               r.get(6)?,
            confidence:            r.get(7)?,
            classification_source: r.get(8)?,
            outcome:               r.get(9)?,
            ocr_text:              r.get(10)?,
            created_at:            r.get(11)?,
        }),
    ).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(ClassificationEventListResponse { items, total })
}

#[cfg(test)]
mod event_list_tests {
    use rusqlite::Connection;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("
            CREATE TABLE classification_events (
                id TEXT PRIMARY KEY, war_id TEXT NOT NULL,
                process_name TEXT NOT NULL, window_title TEXT NOT NULL,
                client_id TEXT, project_id TEXT, task_id TEXT,
                confidence REAL NOT NULL DEFAULT 0.0,
                classification_source TEXT NOT NULL DEFAULT 'unclassified',
                outcome TEXT NOT NULL DEFAULT 'pending',
                ocr_text TEXT,
                created_at TEXT NOT NULL
            );
        ").unwrap();
        conn.execute_batch("
            INSERT INTO classification_events VALUES
              ('e1','w1','Code','tracey',NULL,'p1',NULL,0.9,'heuristic','auto',NULL,'2026-01-01T10:00:00Z'),
              ('e2','w2','Slack','general',NULL,'p2',NULL,0.4,'tf_idf','pending',NULL,'2026-01-01T10:01:00Z');
        ").unwrap();
        conn
    }

    #[test]
    fn list_returns_events_descending() {
        let conn = test_db();
        let mut stmt = conn.prepare(
            "SELECT id, process_name, confidence FROM classification_events \
             ORDER BY created_at DESC LIMIT 50 OFFSET 0"
        ).unwrap();
        let rows: Vec<(String, String, f64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap().filter_map(|r| r.ok()).collect();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].0, "e2"); // most recent first
        assert_eq!(rows[1].0, "e1");
    }
}