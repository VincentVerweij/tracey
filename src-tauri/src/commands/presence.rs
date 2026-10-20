//! `presence_list`: the presence spans for the Timeline bar (ADR-0006 decision 5).

use crate::commands::AppState;
use crate::services::presence::store::{self, PresenceSpan};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
pub struct PresenceListRequest {
    pub from: String,
    pub to: String,
}

/// Spans overlapping `[from, to)`, ascending by start and unclipped.
/// `Untracked` is never returned: it is every uncovered stretch.
#[tauri::command]
pub fn presence_list(
    state: State<'_, AppState>,
    request: PresenceListRequest,
) -> Result<Vec<PresenceSpan>, String> {
    let from = parse_instant("from", &request.from)?;
    let to = parse_instant("to", &request.to)?;
    // Held across the query, so no transition can move the open span between
    // reading its id and reading the spans. Same order as the recorder:
    // recorder, then DB.
    let recorder = state.presence.recorder.lock().map_err(|e| e.to_string())?;
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    store::list(&conn, from, to, recorder.open_span_id()).map_err(|e| e.to_string())
}

fn parse_instant(name: &str, value: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(value)
        .map(|t| t.with_timezone(&Utc))
        .map_err(|e| format!("presence_list: `{name}` is not an RFC 3339 instant: {e}"))
}
