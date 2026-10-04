//! Suspension: application-level state saying which tracking loops must not
//! observe right now. See `CONTEXT.md` § Suspension and ADR-0001.
//!
//! Tracking is suspended while at least one suspension reason holds. Each reason
//! has intent-named operations and no generic "clear", so a reason can only be
//! cleared through its own owner's operation. `Locked` is the only reason today;
//! `Paused` (#62) slots in as a second variant of `Reason`.

use std::sync::atomic::{AtomicU8, Ordering};

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
    #[allow(dead_code)] // called by the session watch (#80); tests drive it directly
    pub fn raise_locked(&self) {
        self.raise(Reason::Locked);
    }

    /// Clear `Locked`. Owned by the session watch (ADR-0002). Clears nothing else.
    #[allow(dead_code)] // called by the session watch (#80); tests drive it directly
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
