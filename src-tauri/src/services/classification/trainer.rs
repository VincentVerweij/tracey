//! Loads labeled samples from SQLite, trains a TfIdfModel, and persists it. (stub — implemented in Task 4)

use rusqlite::Connection;
use super::tfidf::TfIdfModel;

pub const PHASE2_MIN_SAMPLES: i64 = 20;

pub fn load_model(_conn: &Connection) -> Option<TfIdfModel> { None }
pub fn count_samples(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM labeled_samples", [], |r| r.get(0))
        .unwrap_or(0)
}
pub fn retrain(_conn: &Connection) -> Option<TfIdfModel> { None }
