//! Suspension: application-level state saying which tracking loops must not
//! observe right now. See `CONTEXT.md` § Suspension and ADR-0001.
//!
//! Tracking is suspended while at least one suspension reason holds. Each reason
//! has intent-named operations and no generic "clear", so a reason can only be
//! cleared through its own owner's operation. `Locked` is the only reason today;
//! `Paused` (#62) slots in as a second variant of `Reason`.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

/// A loop that observes the user and records what it finds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // IdleDetection, Classification opt in with #62
pub enum TrackingLoop {
    ScreenshotCapture,
    WindowActivity,
    IdleDetection,
    Classification,
}

impl TrackingLoop {
    #[cfg(test)]
    pub const ALL: [TrackingLoop; 4] = [
        TrackingLoop::ScreenshotCapture,
        TrackingLoop::WindowActivity,
        TrackingLoop::IdleDetection,
        TrackingLoop::Classification,
    ];
}

/// A named cause of suspension. Private: callers go through the intent-named
/// operations on `Suspension`, which is what keeps each reason owner-cleared.
#[derive(Debug, Clone, Copy)]
enum Reason {
    Locked,
    /// Test-only stand-in for a second reason (`Paused`, #62) that suspends
    /// every loop, so tests can hold suspension while `Locked` clears.
    #[cfg(test)]
    Other,
}

impl Reason {
    fn bit(self) -> u8 {
        match self {
            Reason::Locked => 1 << 0,
            #[cfg(test)]
            Reason::Other => 1 << 7,
        }
    }

    /// Whether this reason suspends `tracking_loop` (ADR-0001 decision 5, ADR-0003).
    fn suspends(self, tracking_loop: TrackingLoop) -> bool {
        match self {
            Reason::Locked => matches!(
                tracking_loop,
                TrackingLoop::ScreenshotCapture | TrackingLoop::WindowActivity
            ),
            #[cfg(test)]
            Reason::Other => true,
        }
    }
}

#[cfg(not(test))]
const ALL_REASONS: [Reason; 1] = [Reason::Locked];
#[cfg(test)]
const ALL_REASONS: [Reason; 2] = [Reason::Locked, Reason::Other];

/// The set of suspension reasons currently held. Lives in `AppState`.
/// In-memory only: never persisted, so `Locked` is re-seeded on each start.
#[derive(Debug, Default)]
pub struct Suspension {
    reasons: AtomicU8,
}

impl Suspension {
    pub fn new() -> Self {
        Self::default()
    }

    /// Raise `Locked`. Owned by the session watch (ADR-0002).
    pub fn raise_locked(&self) {
        self.raise(Reason::Locked);
    }

    /// Clear `Locked`. Owned by the session watch (ADR-0002). Clears nothing else.
    pub fn clear_locked(&self) {
        self.clear(Reason::Locked);
    }

    /// Raise the test-only stand-in for a second reason.
    #[cfg(test)]
    pub fn raise_other_reason(&self) {
        self.raise(Reason::Other);
    }

    /// Clear the test-only stand-in for a second reason.
    #[cfg(test)]
    pub fn clear_other_reason(&self) {
        self.clear(Reason::Other);
    }

    /// Whether the `Locked` reason is held, whatever else holds. For the
    /// session watch, which reconciles the reason against the session state.
    pub fn is_locked(&self) -> bool {
        self.reasons.load(Ordering::SeqCst) & Reason::Locked.bit() != 0
    }

    /// Whether `tracking_loop` is suspended: the OR of the held reasons that
    /// suspend that loop. A loop consults this once per tick, before observing.
    pub fn is_suspended(&self, tracking_loop: TrackingLoop) -> bool {
        let held = self.reasons.load(Ordering::SeqCst);
        ALL_REASONS
            .iter()
            .any(|r| held & r.bit() != 0 && r.suspends(tracking_loop))
    }

    fn raise(&self, reason: Reason) {
        self.reasons.fetch_or(reason.bit(), Ordering::SeqCst);
    }

    fn clear(&self, reason: Reason) {
        self.reasons.fetch_and(!reason.bit(), Ordering::SeqCst);
    }
}

/// The session-state query adapter: reconciles `Locked` from the real session
/// state (ADR-0002 decision 5). Chosen once, in `lib.rs`: the session watch's
/// `reconcile` in the app, a no-op under the `test` feature.
pub type SessionStateQuery = Arc<dyn Fn(&Suspension) + Send + Sync>;

/// How a loop's tick starts, as reported by `LoopSuspension::begin_tick`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickStart {
    /// The loop is suspended: skip the whole tick, observing nothing.
    Suspended,
    /// Suspension ends on this tick: run the loop's re-entry, then observe.
    SuspensionEnds,
    /// Not suspended, and was not last tick: observe as usual.
    Observing,
}

/// One loop's view of suspension, owned by that loop's schedule. See
/// `CONTEXT.md` § Suspension. It holds the loop's *suspended last tick* bit
/// (ADR-0004 decision 1) and reconciles `Locked` from the session-state query
/// on suspended ticks and immediately before each record (ADR-0002 decision 5,
/// ADR-0003 decision 3), and nowhere else.
pub struct LoopSuspension {
    suspension: Arc<Suspension>,
    tracking_loop: TrackingLoop,
    query: SessionStateQuery,
    suspended_last_tick: bool,
}

impl LoopSuspension {
    pub fn new(
        suspension: Arc<Suspension>,
        tracking_loop: TrackingLoop,
        query: SessionStateQuery,
    ) -> Self {
        Self { suspension, tracking_loop, query, suspended_last_tick: false }
    }

    /// Call once at the start of each tick, before observing anything. On a
    /// suspended tick it reconciles first, so a wrong `Locked` cannot hold the
    /// loop shut.
    pub fn begin_tick(&mut self) -> TickStart {
        if self.suspension.is_suspended(self.tracking_loop) && self.reconciled_suspended() {
            return TickStart::Suspended;
        }
        if std::mem::take(&mut self.suspended_last_tick) {
            return TickStart::SuspensionEnds;
        }
        TickStart::Observing
    }

    /// Call immediately before recording an observation. Reconciles, then
    /// reports whether the loop may still record. A `false` is remembered, so
    /// a later unsuspended tick reports `SuspensionEnds`.
    pub fn may_record(&mut self) -> bool {
        !self.reconciled_suspended()
    }

    /// Runs the session-state query, then reports whether this loop is still
    /// suspended. A suspended answer is remembered as a suspended tick.
    fn reconciled_suspended(&mut self) -> bool {
        (self.query)(&self.suspension);
        let suspended = self.suspension.is_suspended(self.tracking_loop);
        self.suspended_last_tick |= suspended;
        suspended
    }
}

/// A session-state query that never queries, for the `test` feature and unit
/// tests, so a real session state never overrides a `Locked` they drive.
#[cfg(any(test, feature = "test"))]
pub fn no_session_state_query() -> SessionStateQuery {
    Arc::new(|_: &Suspension| {})
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn nothing_is_suspended_at_startup() {
        let suspension = Suspension::new();
        for tracking_loop in TrackingLoop::ALL {
            assert!(!suspension.is_suspended(tracking_loop));
        }
    }

    #[test]
    fn locked_suspends_screenshot_capture_and_window_activity_tracking() {
        let suspension = Suspension::new();
        suspension.raise_locked();
        assert!(suspension.is_suspended(TrackingLoop::ScreenshotCapture));
        assert!(suspension.is_suspended(TrackingLoop::WindowActivity));
    }

    #[test]
    fn locked_does_not_suspend_idle_detection_or_classification() {
        let suspension = Suspension::new();
        suspension.raise_locked();
        assert!(!suspension.is_suspended(TrackingLoop::IdleDetection));
        assert!(!suspension.is_suspended(TrackingLoop::Classification));
    }

    #[test]
    fn clearing_locked_lifts_its_suspension() {
        let suspension = Suspension::new();
        suspension.raise_locked();
        suspension.clear_locked();
        for tracking_loop in TrackingLoop::ALL {
            assert!(!suspension.is_suspended(tracking_loop));
        }
    }

    #[test]
    fn raising_and_clearing_locked_are_idempotent() {
        let suspension = Suspension::new();
        suspension.raise_locked();
        suspension.raise_locked();
        assert!(suspension.is_suspended(TrackingLoop::ScreenshotCapture));
        suspension.clear_locked();
        suspension.clear_locked();
        assert!(!suspension.is_suspended(TrackingLoop::ScreenshotCapture));
    }

    /// Window activity tracking's view of `suspension`, reconciling through `query`.
    fn window_activity(suspension: &Arc<Suspension>, query: SessionStateQuery) -> LoopSuspension {
        LoopSuspension::new(suspension.clone(), TrackingLoop::WindowActivity, query)
    }

    #[test]
    fn a_tick_observes_while_nothing_is_held() {
        let suspension = Arc::new(Suspension::new());
        let mut loop_suspension = window_activity(&suspension, no_session_state_query());
        assert_eq!(loop_suspension.begin_tick(), TickStart::Observing);
        assert!(loop_suspension.may_record());
        assert_eq!(loop_suspension.begin_tick(), TickStart::Observing);
    }

    #[test]
    fn suspension_ends_once_on_the_first_tick_after_locked_clears() {
        let suspension = Arc::new(Suspension::new());
        let mut loop_suspension = window_activity(&suspension, no_session_state_query());
        suspension.raise_locked();
        assert_eq!(loop_suspension.begin_tick(), TickStart::Suspended);
        assert_eq!(loop_suspension.begin_tick(), TickStart::Suspended);
        suspension.clear_locked();
        assert_eq!(loop_suspension.begin_tick(), TickStart::SuspensionEnds);
        assert_eq!(loop_suspension.begin_tick(), TickStart::Observing);
    }

    #[test]
    fn a_reason_that_does_not_suspend_the_loop_is_not_a_suspension() {
        let suspension = Arc::new(Suspension::new());
        let mut loop_suspension = LoopSuspension::new(
            suspension.clone(),
            TrackingLoop::IdleDetection,
            no_session_state_query(),
        );
        suspension.raise_locked();
        assert_eq!(loop_suspension.begin_tick(), TickStart::Observing);
        assert!(loop_suspension.may_record());
    }

    #[test]
    fn clearing_locked_while_another_reason_holds_is_not_a_suspension_end() {
        let suspension = Arc::new(Suspension::new());
        let mut loop_suspension = window_activity(&suspension, no_session_state_query());
        suspension.raise_locked();
        suspension.raise_other_reason();
        assert_eq!(loop_suspension.begin_tick(), TickStart::Suspended);
        suspension.clear_locked();
        assert_eq!(loop_suspension.begin_tick(), TickStart::Suspended);
        suspension.clear_other_reason();
        assert_eq!(loop_suspension.begin_tick(), TickStart::SuspensionEnds);
    }

    /// A query scripted to report the session as locked while `locked` is set.
    fn scripted(locked: &Arc<AtomicBool>) -> SessionStateQuery {
        let locked = locked.clone();
        Arc::new(move |s: &Suspension| {
            if locked.load(Ordering::SeqCst) {
                s.raise_locked();
            } else {
                s.clear_locked();
            }
        })
    }

    #[test]
    fn a_wrongly_raised_locked_is_cleared_by_the_reconcile_and_suspension_ends() {
        let suspension = Arc::new(Suspension::new());
        let session_locked = Arc::new(AtomicBool::new(false));
        let mut loop_suspension = window_activity(&suspension, scripted(&session_locked));
        session_locked.store(true, Ordering::SeqCst);
        suspension.raise_locked();
        assert_eq!(loop_suspension.begin_tick(), TickStart::Suspended);

        session_locked.store(false, Ordering::SeqCst); // the unlock event was missed
        assert_eq!(loop_suspension.begin_tick(), TickStart::SuspensionEnds);
        assert!(loop_suspension.may_record());
        assert_eq!(loop_suspension.begin_tick(), TickStart::Observing);
    }

    #[test]
    fn a_wrongly_raised_locked_at_startup_is_cleared_and_the_loop_observes() {
        let suspension = Arc::new(Suspension::new());
        let session_locked = Arc::new(AtomicBool::new(false));
        let mut loop_suspension = window_activity(&suspension, scripted(&session_locked));
        suspension.raise_locked(); // e.g. an UNKNOWN seed
        // No suspended tick came before, so there is no suspension end to report.
        assert_eq!(loop_suspension.begin_tick(), TickStart::Observing);
        assert!(loop_suspension.may_record());
    }

    #[test]
    fn a_missed_lock_found_by_the_reconcile_blocks_recording() {
        let suspension = Arc::new(Suspension::new());
        let session_locked = Arc::new(AtomicBool::new(false));
        let mut loop_suspension = window_activity(&suspension, scripted(&session_locked));
        assert_eq!(loop_suspension.begin_tick(), TickStart::Observing);
        assert!(loop_suspension.may_record());

        session_locked.store(true, Ordering::SeqCst); // the lock event was missed
        assert_eq!(loop_suspension.begin_tick(), TickStart::Observing);
        assert!(!loop_suspension.may_record());
        assert_eq!(loop_suspension.begin_tick(), TickStart::Suspended);

        // The reconcile's unlock is a suspension end like any other.
        session_locked.store(false, Ordering::SeqCst);
        assert_eq!(loop_suspension.begin_tick(), TickStart::SuspensionEnds);
    }

    #[test]
    fn the_reconcile_runs_on_suspended_ticks_and_before_each_record_and_nowhere_else() {
        let suspension = Arc::new(Suspension::new());
        let queries = Arc::new(AtomicU8::new(0));
        let counter = queries.clone();
        let mut loop_suspension = window_activity(
            &suspension,
            Arc::new(move |_: &Suspension| {
                counter.fetch_add(1, Ordering::SeqCst);
            }),
        );
        let count = || queries.load(Ordering::SeqCst);

        assert_eq!(loop_suspension.begin_tick(), TickStart::Observing);
        assert_eq!(count(), 0, "queried on an unsuspended tick start");
        assert!(loop_suspension.may_record());
        assert_eq!(count(), 1, "one query per record");

        suspension.raise_locked();
        assert_eq!(loop_suspension.begin_tick(), TickStart::Suspended);
        assert_eq!(count(), 2, "a suspended tick must reconcile to recover");
        assert_eq!(loop_suspension.begin_tick(), TickStart::Suspended);
        assert_eq!(count(), 3);

        suspension.clear_locked();
        assert_eq!(loop_suspension.begin_tick(), TickStart::SuspensionEnds);
        assert_eq!(count(), 3, "queried on the tick suspension ends, before any record");
    }
}
