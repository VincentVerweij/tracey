//! T082 — Window Activity Tracker
//! Polls foreground window every 1 second.
//! Applies process deny-list before any storage write.
//! MutexGuard ALWAYS dropped before any .await point (inner block pattern).
//!
//! T083 note: External sync of window_activity_records is already handled by
//! `sync_service.rs` (queries `synced_at IS NULL` rows every 30 seconds).
//! No separate flush loop is needed here.

use tauri::{AppHandle, Manager};
use chrono::Utc;
use ulid::Ulid;

use crate::commands::AppState;
use crate::services::suspension::{LoopSuspension, TickStart};

fn new_id() -> String {
    Ulid::new().to_string()
}

/// A foreground window as `(process_name, title)`.
type WindowKey = (String, String);

/// Per-tick activity decision: write a row when the foreground window changes,
/// unless its process is on the deny-list.
struct ActivitySchedule {
    /// Last seen window. None = first tick / no window.
    last_window: Option<WindowKey>,
    suspension: LoopSuspension,
}

impl ActivitySchedule {
    fn new(suspension: LoopSuspension) -> Self {
        Self { last_window: None, suspension }
    }

    /// Returns the window to write a row for, or `None` to write nothing this tick.
    /// `observe` queries the foreground window; `read_deny_list` reads the process
    /// deny-list and returns `None` when storage is unavailable, in which case the
    /// change is retried next tick.
    fn tick(
        &mut self,
        observe: impl FnOnce() -> Option<WindowKey>,
        read_deny_list: impl FnOnce() -> Option<Vec<String>>,
    ) -> Option<WindowKey> {
        match self.suspension.begin_tick() {
            // A suspended tick skips everything, including the foreground query and
            // change-detection updates (ADR-0004 decision 3). No marker row is written.
            TickStart::Suspended => return None,
            // Re-entry when suspension ends (ADR-0004 decision 4): forget the last
            // window so this tick writes one fresh row, even for the same window.
            TickStart::SuspensionEnds => self.last_window = None,
            TickStart::Observing => {}
        }

        let current = observe();

        // Initial state (last_window = None, current Some) counts as a change.
        let changed = match (&self.last_window, &current) {
            (None, Some(_)) => true,
            (Some(prev), Some(cur)) => prev != cur,
            (Some(_), None) => true, // window disappeared
            (None, None) => false,
        };
        if !changed {
            return None; // no-op tick; skip all DB work
        }

        // DB unavailable: retry next tick; last_window unchanged
        let deny_list = read_deny_list()?;

        // Always advance last_window, even when denied.
        // (Denied processes still mark a "real" window change in OS terms.)
        self.last_window = current.clone();

        // Apply process deny-list BEFORE any storage write (decisions.md)
        let row = current.filter(|(process_name, _)| !is_denied(process_name, &deny_list))?;

        // Reconcile `Locked` immediately before the write (ADR-0003 amending
        // ADR-0002 decision 5): a missed lock must not record the lock screen.
        self.suspension.may_record().then_some(row)
    }
}

fn is_denied(process_name: &str, deny_list: &[String]) -> bool {
    let process_lower = process_name.to_lowercase();
    deny_list
        .iter()
        .any(|entry| process_lower.contains(&entry.to_lowercase()))
}

fn load_deny_list(conn: &rusqlite::Connection) -> Vec<String> {
    let deny_list_json: String = conn
        .query_row(
            "SELECT process_deny_list_json FROM user_preferences LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap_or_else(|_| "[]".to_string());
    serde_json::from_str(&deny_list_json).unwrap_or_default()
}

fn write_activity_row(conn: &rusqlite::Connection, (process_name, title): &WindowKey) {
    let id = new_id();
    let now = Utc::now().to_rfc3339();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());
    // window_handle: composite string identifier (no raw HWND)
    let window_handle = format!("{}:{}", process_name, title);

    let _ = conn.execute(
        "INSERT INTO window_activity_records \
         (id, process_name, window_title, window_handle, \
          recorded_at, device_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![id, process_name, title, window_handle, now, device_id],
    );
}

/// Start the window activity polling loop.
/// Calls `state.platform.get_foreground_window_info()` every second.
/// On window change: applies process deny-list, writes to `window_activity_records`.
/// The first tick always counts as a window change (initial state is None).
/// Call once from lib.rs `.setup()`, with this loop's suspension handle.
pub fn start_activity_loop(app: AppHandle, suspension: LoopSuspension) {
    tauri::async_runtime::spawn(async move {
        let mut schedule = ActivitySchedule::new(suspension);

        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            // All synchronous work inside this block.
            // MutexGuard is dropped at the closing brace of this block, before the next .await.
            {
                let state = app.state::<AppState>();
                // The DB guard is taken only on a window change, and the same guard
                // covers both the deny-list read and the row write.
                let mut conn = None;
                let row = schedule.tick(
                    || {
                        state
                            .platform
                            .get_foreground_window_info()
                            .map(|w| (w.process_name, w.title))
                    },
                    || {
                        let guard = state.db.lock().ok()?;
                        let deny_list = load_deny_list(&guard);
                        conn = Some(guard);
                        Some(deny_list)
                    },
                );
                if let (Some(window), Some(conn)) = (row, conn) {
                    write_activity_row(&conn, &window);
                };
            } // MutexGuard dropped here — NEVER held across an .await point
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use crate::services::suspension::{SessionStateQuery, Suspension, TrackingLoop};

    fn window(title: &str) -> Option<WindowKey> {
        Some(("app.exe".to_string(), title.to_string()))
    }

    /// A schedule whose session-state query finds nothing to change.
    fn schedule(suspension: &Arc<Suspension>) -> ActivitySchedule {
        schedule_querying(suspension, Arc::new(|_: &Suspension| {}))
    }

    fn schedule_querying(suspension: &Arc<Suspension>, query: SessionStateQuery) -> ActivitySchedule {
        ActivitySchedule::new(LoopSuspension::new(
            suspension.clone(),
            TrackingLoop::WindowActivity,
            query,
        ))
    }

    /// Ticks once with the given foreground window and an empty deny-list.
    fn tick(schedule: &mut ActivitySchedule, foreground: Option<WindowKey>) -> Option<WindowKey> {
        schedule.tick(|| foreground, || Some(vec![]))
    }

    #[test]
    fn writes_a_row_on_window_change_only() {
        let suspension = Arc::new(Suspension::new());
        let mut schedule = schedule(&suspension);
        assert_eq!(tick(&mut schedule, window("doc")), window("doc"));
        assert_eq!(tick(&mut schedule, window("doc")), None);
        assert_eq!(tick(&mut schedule, window("mail")), window("mail"));
    }

    #[test]
    fn no_row_and_no_foreground_query_while_locked() {
        let suspension = Arc::new(Suspension::new());
        suspension.raise_locked();
        let mut schedule = schedule(&suspension);
        let mut queried = false;
        let row = schedule.tick(
            || {
                queried = true;
                window("LockApp")
            },
            || Some(vec![]),
        );
        assert_eq!(row, None);
        assert!(!queried, "queried the foreground window while Locked");
    }

    #[test]
    fn one_fresh_row_after_suspension_ends_even_for_the_same_window() {
        let suspension = Arc::new(Suspension::new());
        let mut schedule = schedule(&suspension);
        assert_eq!(tick(&mut schedule, window("doc")), window("doc"));
        suspension.raise_locked();
        assert_eq!(tick(&mut schedule, window("doc")), None);
        suspension.clear_locked();
        assert_eq!(tick(&mut schedule, window("doc")), window("doc"));
        assert_eq!(tick(&mut schedule, window("doc")), None);
    }

    #[test]
    fn a_denied_window_produces_no_forced_row() {
        let suspension = Arc::new(Suspension::new());
        let mut schedule = schedule(&suspension);
        let deny = || Some(vec!["KeePass".to_string()]);
        let keepass = || Some(("KeePass.exe".to_string(), "vault".to_string()));
        assert_eq!(schedule.tick(keepass, deny), None);
        suspension.raise_locked();
        assert_eq!(schedule.tick(keepass, deny), None);
        suspension.clear_locked();
        assert_eq!(schedule.tick(keepass, deny), None);
        assert_eq!(schedule.tick(keepass, deny), None);
    }

    #[test]
    fn a_lock_found_at_the_write_point_writes_no_row() {
        let suspension = Arc::new(Suspension::new());
        // The lock event was missed; the query before the write finds it.
        let mut schedule =
            schedule_querying(&suspension, Arc::new(|s: &Suspension| s.raise_locked()));
        assert_eq!(tick(&mut schedule, window("LockApp")), None);
    }
}
