//! Background loop that classifies unprocessed window activity records.
//! Polls every 2 seconds for records with classified_at IS NULL.
//! High confidence (≥ threshold) → auto-create/extend time entry.
//! Low confidence → enqueue in ActiveLearningQueue + emit tracey://classification-needed.

use tauri::{AppHandle, Emitter, Manager};
use chrono::Utc;
use ulid::Ulid;

use crate::commands::AppState;
use crate::services::active_learning_queue::make_pattern_key;
use crate::services::classification::{
    self,
    feature_extractor::extract,
    ClassificationSource,
};

struct UnclassifiedRecord {
    id: String,
    process_name: String,
    window_title: String,
    recorded_at: String,
}

pub fn start_classification_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
            process_batch(&app).await;
        }
    });
}

async fn process_batch(app: &AppHandle) {
    // Fetch up to 10 unclassified records — lock released before any await
    let records: Vec<UnclassifiedRecord> = {
        let state = app.state::<AppState>();
        let conn = match state.db.lock() { Ok(c) => c, Err(_) => return };
        let mut stmt = match conn.prepare(
            "SELECT id, process_name, window_title, recorded_at \
             FROM window_activity_records \
             WHERE classified_at IS NULL \
             ORDER BY recorded_at ASC LIMIT 10",
        ) { Ok(s) => s, Err(_) => return };

        stmt.query_map([], |r| Ok(UnclassifiedRecord {
            id: r.get(0)?,
            process_name: r.get(1)?,
            window_title: r.get(2)?,
            recorded_at: r.get(3)?,
        }))
        .map(|rows| rows.filter_map(|r| r.ok()).collect::<Vec<_>>())
        .unwrap_or_default()
    }; // db lock released

    for rec in records {
        classify_record(app, &rec).await;
    }
}

async fn classify_record(app: &AppHandle, rec: &UnclassifiedRecord) {
    // 1. Fetch OCR text from most recent screenshot for this window — lock dropped before await
    let ocr_text: Option<String> = {
        let state = app.state::<AppState>();
        let conn = match state.db.lock() { Ok(c) => c, Err(_) => return };
        conn.query_row(
            "SELECT ocr_text FROM screenshots \
             WHERE process_name = ?1 AND captured_at <= ?2 \
             ORDER BY captured_at DESC LIMIT 1",
            rusqlite::params![rec.process_name, rec.recorded_at],
            |r| r.get(0),
        ).ok().flatten()
    };

    // 2. Extract features + classify
    let features = extract(&rec.process_name, &rec.window_title, ocr_text.as_deref());
    let (result, threshold, group_gap, auto_enabled) = {
        let state = app.state::<AppState>();
        let conn = match state.db.lock() { Ok(c) => c, Err(_) => return };
        let cs = match state.classification_state.lock() { Ok(c) => c, Err(_) => return };
        let (threshold, gap, enabled): (f64, i64, i64) = conn.query_row(
            "SELECT auto_classification_confidence_threshold, \
                    auto_classification_group_gap_seconds, \
                    auto_classification_enabled \
             FROM user_preferences LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        ).unwrap_or((0.7, 120, 1));
        (classification::classify(&features, &cs.rules, cs.model.as_ref()), threshold as f32, gap, enabled)
    };

    if auto_enabled == 0 {
        let state = app.state::<AppState>();
        if let Ok(conn) = state.db.lock() {
            mark_classified(&conn, &rec.id);
        };
        return;
    }

    // 3. Store classification event + finalise in a single lock acquisition
    let event_id = Ulid::new().to_string().to_lowercase();
    let now = Utc::now().to_rfc3339();
    let source_str = match result.top.source {
        ClassificationSource::Heuristic => "heuristic",
        ClassificationSource::TfIdf => "tf_idf",
        ClassificationSource::Unclassified => "unclassified",
    };

    {
        let state = app.state::<AppState>();
        let conn = match state.db.lock() { Ok(c) => c, Err(_) => return };
        if let Err(e) = conn.execute(
            "INSERT INTO classification_events \
             (id, war_id, process_name, window_title, client_id, project_id, task_id, \
              confidence, classification_source, outcome, ocr_text, created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'pending',?10,?11)",
            rusqlite::params![
                event_id, rec.id, rec.process_name, rec.window_title,
                result.top.client_id, result.top.project_id, result.top.task_id,
                result.top.confidence, source_str, ocr_text, now,
            ],
        ) {
            log::warn!("[classification_loop] Failed to insert classification event: {e}");
        }

        if result.top.confidence >= threshold {
            // Auto-create/extend time entry — conn passed directly, no second lock
            auto_create_or_extend_time_entry(&conn, &rec.id, &rec.recorded_at, &event_id, &result, group_gap);
        } else {
            // Enqueue for active learning — release conn before locking ALQ
            drop(conn);
            let pattern_key = make_pattern_key(&rec.process_name, &rec.window_title);
            let should_prompt = {
                let mut alq = match state.active_learning_queue.lock() { Ok(q) => q, Err(_) => return };
                alq.enqueue(crate::services::active_learning_queue::PendingRecord {
                    war_id: rec.id.clone(),
                    process_name: rec.process_name.clone(),
                    window_title: rec.window_title.clone(),
                    ocr_text: ocr_text.clone(),
                })
            };

            if should_prompt {
                let suggestions_json = serde_json::to_value(&result).unwrap_or_default();
                if let Err(e) = app.emit(
                    "tracey://classification-needed",
                    serde_json::json!({
                        "war_id": rec.id,
                        "event_id": event_id,
                        "process_name": rec.process_name,
                        "window_title": rec.window_title,
                        "ocr_text": ocr_text,
                        "pattern_key": pattern_key,
                        "suggestions": suggestions_json,
                    }),
                ) {
                    log::warn!("[classification_loop] Failed to emit classification-needed: {e}");
                }
            } else {
                // Snoozed — re-acquire db lock to update outcome
                if let Ok(conn2) = state.db.lock() {
                    update_event_outcome(&conn2, &event_id, "unclassified");
                };
            }
            // mark_classified after enqueue path — re-acquire lock
            if let Ok(conn2) = state.db.lock() {
                mark_classified(&conn2, &rec.id);
            };
            return;
        }

        // High-confidence path: mark classified using the same lock
        mark_classified(&conn, &rec.id);
    }
}

fn mark_classified(conn: &rusqlite::Connection, war_id: &str) {
    let now = Utc::now().to_rfc3339();
    if let Err(e) = conn.execute(
        "UPDATE window_activity_records SET classified_at = ?1 WHERE id = ?2",
        rusqlite::params![now, war_id],
    ) {
        log::warn!("[classification_loop] Failed to mark {war_id} as classified: {e}");
    }
}

fn update_event_outcome(conn: &rusqlite::Connection, event_id: &str, outcome: &str) {
    if let Err(e) = conn.execute(
        "UPDATE classification_events SET outcome = ?1 WHERE id = ?2",
        rusqlite::params![outcome, event_id],
    ) {
        log::warn!("[classification_loop] Failed to update event outcome for {event_id}: {e}");
    }
}

fn auto_create_or_extend_time_entry(
    conn: &rusqlite::Connection,
    war_id: &str,
    recorded_at: &str,
    event_id: &str,
    result: &crate::services::classification::ClassificationResult,
    group_gap_seconds: i64,
) {
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    // Try to find a recent auto time entry for the same project/task
    let existing: Option<(String, String)> = conn.query_row(
        "SELECT id, ended_at FROM time_entries \
         WHERE project_id IS ?1 AND task_id IS ?2 AND source = 'auto' AND device_id = ?3 \
         ORDER BY ended_at DESC LIMIT 1",
        rusqlite::params![result.top.project_id, result.top.task_id, device_id],
        |r| Ok((r.get(0)?, r.get(1)?)),
    ).ok();

    if let Some((entry_id, ended_at)) = existing {
        if let (Ok(ended), Ok(start)) = (
            chrono::DateTime::parse_from_rfc3339(&ended_at),
            chrono::DateTime::parse_from_rfc3339(recorded_at),
        ) {
            let gap = start.signed_duration_since(ended).num_seconds();
            if gap >= 0 && gap <= group_gap_seconds {
                if let Err(e) = conn.execute(
                    "UPDATE time_entries SET ended_at = ?1, modified_at = ?2 WHERE id = ?3",
                    rusqlite::params![recorded_at, Utc::now().to_rfc3339(), entry_id],
                ) {
                    log::warn!("[classification_loop] Failed to extend time entry {entry_id}: {e}");
                }
                update_event_outcome(conn, event_id, "auto");
                return;
            }
        }
    }

    // Create new auto time entry
    let entry_id = Ulid::new().to_string().to_lowercase();
    let now_str = Utc::now().to_rfc3339();
    if let Err(e) = conn.execute(
        "INSERT INTO time_entries \
         (id, description, started_at, ended_at, project_id, task_id, is_break, \
          device_id, created_at, modified_at, source) \
         VALUES (?1,'',?2,?2,?3,?4,0,?5,?6,?6,'auto')",
        rusqlite::params![
            entry_id, recorded_at,
            result.top.project_id, result.top.task_id,
            device_id, now_str,
        ],
    ) {
        log::warn!("[classification_loop] Failed to insert auto time entry: {e}");
    }
    update_event_outcome(conn, event_id, "auto");
    let _ = war_id; // war_id used by caller for mark_classified
}
