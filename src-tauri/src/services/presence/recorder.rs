//! The presence recorder: the sole writer of `presence_spans` (ADR-0006
//! decision 2). `Recorder` applies the derivation's writes and remembers the
//! open span and the previous tick between ticks; `start_presence_recorder`
//! runs it on its own 1 s tick. It is not a tracking loop and is never
//! suspended: it observes suspension rather than being subject to it.

use std::sync::{Arc, Mutex};

use chrono::{DateTime, Utc};
use rusqlite::Connection;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::{broadcast, Notify};

use crate::commands::AppState;
use crate::services::suspension::SessionStateQuery;

use super::store::{self, timestamp};
use super::{derive, Observation, OpenSpan, Presence, Write, HEARTBEAT};

/// How often the presence recorder ticks when nothing wakes it.
const TICK: std::time::Duration = std::time::Duration::from_secs(1);

/// The Tauri event name of `presence-changed`.
pub const PRESENCE_CHANGED_EVENT: &str = "tracey://presence-changed";

/// The presence recorder's shared parts. Lives in `AppState`.
pub struct PresenceRecording {
    pub recorder: Mutex<Recorder>,
    /// `presence-changed` for in-process consumers (ADR-0006 decision 2).
    pub changed: broadcast::Sender<PresenceChanged>,
    /// Brings the next tick forward. The session watch notifies it on lock
    /// and unlock so lock instants are exact.
    pub wake: Arc<Notify>,
}

impl PresenceRecording {
    pub fn new() -> Self {
        Self {
            recorder: Mutex::new(Recorder::new()),
            changed: broadcast::channel(16).0,
            wake: Arc::new(Notify::new()),
        }
    }
}

/// What the presence recorder reads on each tick.
#[derive(Debug, Clone, Copy)]
pub struct Snapshot {
    pub idle_seconds: u64,
    pub inactivity_timeout_seconds: u64,
    pub locked: bool,
    /// Always false until `Paused` (#62) has a source.
    pub paused: bool,
    pub now: DateTime<Utc>,
}

/// `presence-changed`: emitted after each transition the recorder writes,
/// never for a heartbeat. `state` is `None` when nothing is open, which is
/// `Untracked`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PresenceChanged {
    pub state: Option<&'static str>,
    pub away_cause: Option<&'static str>,
    pub at: String,
}

/// The open span as stored: its row id and its extent.
#[derive(Debug, Clone)]
struct StoredSpan {
    id: String,
    span: OpenSpan,
}

#[derive(Debug, Default)]
pub struct Recorder {
    open: Option<StoredSpan>,
    /// The latest tick seen. Never moves back, so a clock set back writes
    /// nothing until it passes this again.
    previous_tick: Option<DateTime<Utc>>,
    /// Set by the exit close: nothing is recorded after it.
    exited: bool,
}

impl Recorder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Record one tick's snapshot. Returns the `presence-changed` to emit
    /// when it wrote a transition.
    pub fn observe(
        &mut self,
        conn: &Connection,
        snapshot: Snapshot,
    ) -> rusqlite::Result<Option<PresenceChanged>> {
        if self.exited {
            return Ok(None);
        }
        let observation = Observation {
            idle_seconds: snapshot.idle_seconds,
            inactivity_timeout_seconds: snapshot.inactivity_timeout_seconds,
            locked: snapshot.locked,
            paused: snapshot.paused,
            now: snapshot.now,
            previous_tick: self.previous_tick,
        };
        self.previous_tick = self.previous_tick.max(Some(snapshot.now));
        let writes = derive(self.open.as_ref().map(|stored| &stored.span), &observation);
        self.apply(conn, &writes)?;
        Ok(self.change_written_by(&writes))
    }

    /// Close the open span at the last tick, on a clean exit.
    pub fn close_at_exit(&mut self, conn: &Connection) -> rusqlite::Result<()> {
        self.exited = true;
        match (self.open.is_some(), self.previous_tick) {
            (true, Some(last_tick)) => self.apply(
                conn,
                &[Write::Close {
                    ended_at: last_tick,
                }],
            ),
            _ => Ok(()),
        }
    }

    /// Apply `writes` in one transaction. The open span changes in memory only
    /// once they are committed.
    fn apply(&mut self, conn: &Connection, writes: &[Write]) -> rusqlite::Result<()> {
        if writes.is_empty() {
            return Ok(());
        }
        let tx = conn.unchecked_transaction()?;
        let mut open = self.open.clone();
        for write in writes {
            open = apply_one(&tx, open, write)?;
        }
        tx.commit()?;
        self.open = open;
        Ok(())
    }

    /// The `presence-changed` for `writes`, just applied: `None` unless they
    /// hold a transition.
    fn change_written_by(&self, writes: &[Write]) -> Option<PresenceChanged> {
        let transition_at = writes.iter().find_map(|write| match write {
            Write::Extend { .. } => None,
            Write::Open { started_at, .. } => Some(*started_at),
            Write::Close { ended_at } => Some(*ended_at),
            Write::Discard => None,
        })?;
        let presence = self.open.as_ref().map(|stored| stored.span.presence);
        Some(PresenceChanged {
            state: presence.map(Presence::state),
            away_cause: presence.and_then(Presence::away_cause),
            at: timestamp(
                self.open
                    .as_ref()
                    .map_or(transition_at, |s| s.span.started_at),
            ),
        })
    }

    /// The row id of the span being extended, if any.
    pub fn open_span_id(&self) -> Option<&str> {
        self.open.as_ref().map(|stored| stored.id.as_str())
    }
}

/// Start the presence recorder on its own tick. `session_state_query`
/// reconciles `Locked` on each heartbeat (ADR-0002 decision 5).
pub fn start_presence_recorder(app: AppHandle, session_state_query: SessionStateQuery) {
    tauri::async_runtime::spawn(async move {
        let presence = app.state::<AppState>().presence.clone();
        let mut last_reconcile: Option<DateTime<Utc>> = None;
        loop {
            tokio::select! {
                _ = tokio::time::sleep(TICK) => {}
                _ = presence.wake.notified() => {}
            }
            let state = app.state::<AppState>();
            let now = Utc::now();
            let reconcile_due = match last_reconcile {
                Some(last) => now - last >= HEARTBEAT,
                None => true,
            };
            if reconcile_due {
                session_state_query(&state.suspension);
                last_reconcile = Some(now);
            }
            if let Some(changed) = record_tick(&state, now) {
                emit(&app, &presence, changed);
            }
        }
    });
}

/// Read one snapshot and record it.
fn record_tick(state: &AppState, now: DateTime<Utc>) -> Option<PresenceChanged> {
    let Ok(mut recorder) = state.presence.recorder.lock() else {
        log::error!("presence recorder: recorder lock poisoned");
        return None;
    };
    let Ok(conn) = state.db.lock() else {
        log::error!("presence recorder: DB lock poisoned");
        return None;
    };
    let snapshot = Snapshot {
        idle_seconds: state.platform.get_idle_seconds(),
        inactivity_timeout_seconds: inactivity_timeout_seconds(&conn),
        locked: state.suspension.is_locked(),
        paused: false,
        now,
    };
    recorder.observe(&conn, snapshot).unwrap_or_else(|e| {
        log::warn!("presence recorder: failed to record a tick: {e}");
        None
    })
}

/// The idle threshold, read as the idle loop reads it.
fn inactivity_timeout_seconds(conn: &Connection) -> u64 {
    conn.query_row(
        "SELECT inactivity_timeout_seconds FROM user_preferences LIMIT 1",
        [],
        |r| r.get::<_, i64>(0),
    )
    .map(|secs| secs.max(0) as u64)
    .unwrap_or_else(|e| {
        log::warn!("presence recorder: failed to read the idle threshold, using 300 s: {e}");
        300
    })
}

fn emit(app: &AppHandle, presence: &PresenceRecording, changed: PresenceChanged) {
    log::debug!(
        "presence changed to {:?} ({:?}) at {}",
        changed.state,
        changed.away_cause,
        changed.at
    );
    if let Err(e) = app.emit(PRESENCE_CHANGED_EVENT, &changed) {
        log::warn!("presence recorder: failed to emit presence-changed: {e}");
    }
    // An error only means no in-process consumer is subscribed yet.
    let _ = presence.changed.send(changed);
}

/// Close the open span at the last tick, best effort, on `RunEvent::Exit`.
pub fn close_on_exit(app: &AppHandle) {
    let state = app.state::<AppState>();
    let (Ok(mut recorder), Ok(conn)) = (state.presence.recorder.lock(), state.db.lock()) else {
        log::warn!("presence recorder: could not close the open span at exit");
        return;
    };
    match recorder.close_at_exit(&conn) {
        Ok(()) => log::info!("presence recorder: open span closed at exit"),
        Err(e) => log::warn!("presence recorder: failed to close the open span at exit: {e}"),
    }
}

/// Apply one write to the open span, returning the open span after it.
/// A span that vanished while open (`data_delete_all`) is forgotten, so the
/// next tick opens a fresh one.
fn apply_one(
    conn: &Connection,
    open: Option<StoredSpan>,
    write: &Write,
) -> rusqlite::Result<Option<StoredSpan>> {
    match (open, *write) {
        (
            _,
            Write::Open {
                started_at,
                ended_at,
                presence,
            },
        ) => {
            let id = ulid::Ulid::new().to_string();
            store::insert(conn, &id, started_at, ended_at, presence)?;
            let span = OpenSpan {
                started_at,
                ended_at,
                presence,
            };
            Ok(Some(StoredSpan { id, span }))
        }
        (Some(mut stored), Write::Extend { ended_at }) => {
            let exists = store::set_end(conn, &stored.id, ended_at)?;
            stored.span.ended_at = ended_at;
            Ok(exists.then_some(stored))
        }
        (Some(stored), Write::Close { ended_at }) => {
            store::set_end(conn, &stored.id, ended_at)?;
            Ok(None)
        }
        (Some(stored), Write::Discard) => {
            store::delete(conn, &stored.id)?;
            Ok(None)
        }
        (None, _) => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::presence::store::{list, PresenceSpan};
    use chrono::Duration;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-20T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::seconds(secs)
    }

    fn present(now: i64) -> Snapshot {
        Snapshot {
            idle_seconds: 0,
            inactivity_timeout_seconds: 300,
            locked: false,
            paused: false,
            now: at(now),
        }
    }

    fn locked(now: i64) -> Snapshot {
        Snapshot {
            locked: true,
            ..present(now)
        }
    }

    /// Ticks once a second from `from` to `to` inclusive, collecting what
    /// each tick reports.
    fn tick(
        recorder: &mut Recorder,
        conn: &Connection,
        seconds: std::ops::RangeInclusive<i64>,
        snapshot: fn(i64) -> Snapshot,
    ) -> Vec<PresenceChanged> {
        seconds
            .filter_map(|s| recorder.observe(conn, snapshot(s)).unwrap())
            .collect()
    }

    fn spans(conn: &Connection, recorder: &Recorder) -> Vec<PresenceSpan> {
        list(conn, at(-3600), at(3600), recorder.open_span_id()).unwrap()
    }

    fn span(
        started: i64,
        ended: i64,
        state: &str,
        cause: Option<&str>,
        open: bool,
    ) -> PresenceSpan {
        PresenceSpan {
            started_at: timestamp(at(started)),
            ended_at: timestamp(at(ended)),
            state: state.into(),
            away_cause: cause.map(Into::into),
            open,
        }
    }

    #[test]
    fn a_lock_and_unlock_are_recorded_as_adjacent_spans_with_the_last_one_open() {
        let conn = crate::db::open_in_memory();
        let mut recorder = Recorder::new();
        tick(&mut recorder, &conn, 0..=9, present);
        tick(&mut recorder, &conn, 10..=19, locked);
        tick(&mut recorder, &conn, 20..=24, present);
        assert_eq!(
            spans(&conn, &recorder),
            [
                span(0, 10, "Present", None, false),
                span(10, 20, "Away", Some("Locked"), false),
                span(20, 20, "Present", None, true),
            ]
        );
    }

    #[test]
    fn presence_changed_is_reported_for_each_transition_and_never_for_a_heartbeat() {
        let conn = crate::db::open_in_memory();
        let mut recorder = Recorder::new();
        let mut changes = tick(&mut recorder, &conn, 0..=95, present);
        changes.extend(tick(&mut recorder, &conn, 96..=96, locked));
        assert_eq!(
            changes,
            [
                PresenceChanged {
                    state: Some("Present"),
                    away_cause: None,
                    at: timestamp(at(0)),
                },
                PresenceChanged {
                    state: Some("Away"),
                    away_cause: Some("Locked"),
                    at: timestamp(at(96)),
                },
            ]
        );
    }

    #[test]
    fn the_heartbeat_extends_the_open_span_every_thirty_seconds() {
        let conn = crate::db::open_in_memory();
        let mut recorder = Recorder::new();
        tick(&mut recorder, &conn, 0..=65, present);
        assert_eq!(
            spans(&conn, &recorder),
            [span(0, 60, "Present", None, true)]
        );
    }

    #[test]
    fn a_clean_exit_closes_the_open_span_at_the_last_tick() {
        let conn = crate::db::open_in_memory();
        let mut recorder = Recorder::new();
        tick(&mut recorder, &conn, 0..=45, present);
        recorder.close_at_exit(&conn).unwrap();
        assert_eq!(
            spans(&conn, &recorder),
            [span(0, 45, "Present", None, false)]
        );
    }

    #[test]
    fn a_span_deleted_while_open_is_reopened_after_the_next_heartbeat() {
        let conn = crate::db::open_in_memory();
        let mut recorder = Recorder::new();
        tick(&mut recorder, &conn, 0..=5, present);
        conn.execute("DELETE FROM presence_spans", []).unwrap(); // delete all
        tick(&mut recorder, &conn, 6..=31, present);
        assert_eq!(
            spans(&conn, &recorder),
            [span(31, 31, "Present", None, true)]
        );
    }

    #[test]
    fn nothing_is_recorded_after_the_exit_close() {
        let conn = crate::db::open_in_memory();
        let mut recorder = Recorder::new();
        tick(&mut recorder, &conn, 0..=45, present);
        recorder.close_at_exit(&conn).unwrap();
        tick(&mut recorder, &conn, 46..=47, locked);
        assert_eq!(
            spans(&conn, &recorder),
            [span(0, 45, "Present", None, false)]
        );
    }

    #[test]
    fn a_clock_set_back_resumes_the_open_span_once_it_passes_the_last_tick() {
        let conn = crate::db::open_in_memory();
        let mut recorder = Recorder::new();
        tick(&mut recorder, &conn, 0..=40, present);
        tick(&mut recorder, &conn, 35..=60, present); // set back by six seconds
        assert_eq!(
            spans(&conn, &recorder),
            [span(0, 60, "Present", None, true)]
        );
    }
}
