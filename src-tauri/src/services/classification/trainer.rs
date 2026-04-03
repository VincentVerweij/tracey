//! Loads labeled samples from SQLite, trains a TfIdfModel, and persists it.

use rusqlite::Connection;
use ulid::Ulid;
use chrono::Utc;
use super::tfidf::{TfIdfModel, TrainingSample};

/// Minimum total sample count to activate Phase 2.
pub const PHASE2_MIN_SAMPLES: i64 = 20;

/// Load all labeled samples from the database.
pub fn load_samples(conn: &Connection) -> Vec<TrainingSample> {
    let mut stmt = match conn.prepare(
        "SELECT feature_text, client_id, project_id, task_id FROM labeled_samples",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |row| {
        Ok(TrainingSample {
            text: row.get(0)?,
            client_id: row.get(1)?,
            project_id: row.get(2)?,
            task_id: row.get(3)?,
        })
    })
    .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
    .unwrap_or_default()
}

/// Count labeled samples in the database.
pub fn count_samples(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM labeled_samples", [], |r| r.get(0))
        .unwrap_or(0)
}

/// Train and persist the model. Returns the trained model on success.
/// Returns `None` if there are fewer than `PHASE2_MIN_SAMPLES` total samples.
pub fn retrain(conn: &Connection) -> Option<TfIdfModel> {
    let count = count_samples(conn);
    if count < PHASE2_MIN_SAMPLES { return None; }

    let samples = load_samples(conn);
    let model = TfIdfModel::train(&samples)?;

    // Serialize and persist
    let model_json = serde_json::to_string(&model).ok()?;
    let id = Ulid::new().to_string().to_lowercase();
    let now = Utc::now().to_rfc3339();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    // Replace any existing model row
    let _ = conn.execute("DELETE FROM classifier_model", []);
    let _ = conn.execute(
        "INSERT INTO classifier_model (id, model_json, trained_at, sample_count, device_id) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, model_json, now, count, device_id],
    );

    log::info!("[trainer] Model retrained on {} samples", count);
    Some(model)
}

/// Load the latest persisted model from the database (called at startup).
pub fn load_model(conn: &Connection) -> Option<TfIdfModel> {
    let model_json: String = conn.query_row(
        "SELECT model_json FROM classifier_model LIMIT 1",
        [],
        |r| r.get(0),
    ).ok()?;
    serde_json::from_str(&model_json).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open_test_db() -> Connection {
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
                trained_at TEXT NOT NULL, sample_count INTEGER NOT NULL, device_id TEXT NOT NULL
            );
        ").unwrap();
        conn
    }

    fn insert_sample(conn: &Connection, text: &str, project: &str) {
        conn.execute(
            "INSERT INTO labeled_samples (id,feature_text,process_name,window_title,client_id,project_id,task_id,source,device_id,created_at,modified_at)
             VALUES (?,?,?,?,NULL,?,NULL,'user_confirmed','dev',datetime('now'),datetime('now'))",
            rusqlite::params![Ulid::new().to_string(), text, "app", "title", project],
        ).unwrap();
    }

    #[test]
    fn retrain_returns_none_below_threshold() {
        let conn = open_test_db();
        for i in 0..19 {
            insert_sample(&conn, &format!("code tracey rust {i}"), "proj-a");
        }
        assert_eq!(count_samples(&conn), 19);
        assert!(retrain(&conn).is_none());
    }

    #[test]
    fn retrain_succeeds_at_threshold() {
        let conn = open_test_db();
        for _ in 0..10 {
            insert_sample(&conn, "code tracey rust", "proj-a");
            insert_sample(&conn, "slack general chat", "proj-b");
        }
        assert_eq!(count_samples(&conn), 20);
        let model = retrain(&conn);
        assert!(model.is_some());
        // Model should be persisted
        let loaded = load_model(&conn);
        assert!(loaded.is_some());
    }
}
