//! Presence: whether the user was working at the device, recorded as spans
//! for the Timeline bar. See `GLOSSARY.md` § Presence, ADR-0005 and ADR-0006.
//!
//! The derivation here is pure: given the open span and one observation, it
//! says which writes the presence recorder applies. Only `Present` and `Away`
//! are ever written; every gap between spans is `Untracked`.

use chrono::{DateTime, Duration, Utc};

pub mod recorder;
pub mod store;

/// How often the open span's end is moved to now. A crash, kill or power
/// loss leaves the span at most this late (ADR-0006 decision 1).
pub const HEARTBEAT: Duration = Duration::seconds(30);

/// A longer gap between ticks means the machine slept. The gap is left
/// `Untracked` (ADR-0006 decision 3).
pub const SLEEP_GAP: Duration = Duration::seconds(10);

/// The tracked presence states. `Untracked` is never stored, so it has no
/// variant: it is the absence of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Presence {
    Present,
    Away(AwayCause),
}

impl Presence {
    /// The stored `state`.
    pub fn state(self) -> &'static str {
        match self {
            Presence::Present => "Present",
            Presence::Away(_) => "Away",
        }
    }

    /// The stored `away_cause`: set exactly when `Away`.
    pub fn away_cause(self) -> Option<&'static str> {
        match self {
            Presence::Present => None,
            Presence::Away(AwayCause::Locked) => Some("Locked"),
            Presence::Away(AwayCause::Idle) => Some("Idle"),
        }
    }
}

/// Why a span is `Away`. `Locked` wins when both hold (ADR-0005 decision 4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AwayCause {
    Locked,
    Idle,
}

/// The span the presence recorder is still extending.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpenSpan {
    pub started_at: DateTime<Utc>,
    /// The end last written; the heartbeat moves it to now.
    pub ended_at: DateTime<Utc>,
    pub presence: Presence,
}

/// One snapshot, read by the presence recorder on each tick.
#[derive(Debug, Clone, Copy)]
pub struct Observation {
    pub idle_seconds: u64,
    pub inactivity_timeout_seconds: u64,
    pub locked: bool,
    /// Always false until `Paused` (#62) has a source.
    pub paused: bool,
    pub now: DateTime<Utc>,
    pub previous_tick: Option<DateTime<Utc>>,
}

/// One change to `presence_spans`. Close, extend and discard act on the open span.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Write {
    /// Insert a new open span.
    Open {
        started_at: DateTime<Utc>,
        ended_at: DateTime<Utc>,
        presence: Presence,
    },
    /// Heartbeat: move the open span's end, keeping it open.
    Extend { ended_at: DateTime<Utc> },
    /// Set the open span's end and stop extending it.
    Close { ended_at: DateTime<Utc> },
    /// Delete the open span: a backdated transition left it empty.
    Discard,
}

/// The writes one observation makes, given the open span.
pub fn derive(open: Option<&OpenSpan>, observation: &Observation) -> Vec<Write> {
    if observation.clock_set_back() {
        // Writing now would end a span before it starts, or overlap one.
        return Vec::new();
    }
    match (open, observation.slept_since_previous_tick()) {
        (Some(span), Some(previous_tick)) => {
            let mut writes = vec![end_at(span, previous_tick)];
            writes.extend(derive_from(None, observation));
            writes
        }
        _ => derive_from(open, observation),
    }
}

/// Ends the open span at `at`, deleting it instead when it would be empty.
fn end_at(span: &OpenSpan, at: DateTime<Utc>) -> Write {
    if at > span.started_at {
        Write::Close { ended_at: at }
    } else {
        Write::Discard
    }
}

/// The writes one observation makes once the tick is known to follow the
/// previous one without a sleep in between.
fn derive_from(open: Option<&OpenSpan>, observation: &Observation) -> Vec<Write> {
    let now = observation.now;
    match (open, observation.presence()) {
        (None, None) => Vec::new(),
        (None, Some(presence)) => vec![Write::Open {
            started_at: now,
            ended_at: now,
            presence,
        }],
        (Some(_), None) => vec![Write::Close { ended_at: now }],
        (Some(span), Some(presence)) if span.presence == presence => {
            heartbeat(span, now).into_iter().collect()
        }
        (Some(span), Some(presence)) => hand_over(span, presence, observation),
    }
}

/// Ends the open span and opens `next` where it ended.
fn hand_over(span: &OpenSpan, next: Presence, observation: &Observation) -> Vec<Write> {
    let at = transition_instant(span, next, observation);
    vec![
        end_at(span, at),
        Write::Open {
            started_at: at,
            ended_at: observation.now,
            presence: next,
        },
    ]
}

/// Moves the open span's end to `now` once a heartbeat is due.
fn heartbeat(span: &OpenSpan, now: DateTime<Utc>) -> Option<Write> {
    (now - span.ended_at >= HEARTBEAT).then_some(Write::Extend { ended_at: now })
}

impl Observation {
    /// Whether the wall clock is behind the previous tick: it was set back.
    fn clock_set_back(&self) -> bool {
        self.previous_tick
            .is_some_and(|previous| self.now < previous)
    }

    /// The previous tick, when more than `SLEEP_GAP` passed since it: the
    /// process was frozen, so the machine slept or hibernated.
    fn slept_since_previous_tick(&self) -> Option<DateTime<Utc>> {
        self.previous_tick
            .filter(|previous| self.now - *previous > SLEEP_GAP)
    }

    /// The presence this snapshot shows, `None` being `Untracked`.
    /// Precedence: `Untracked` > `Away` > `Present` (ADR-0005 decision 2).
    fn presence(&self) -> Option<Presence> {
        if self.paused {
            None
        } else if self.locked {
            Some(Presence::Away(AwayCause::Locked))
        } else if self.idle_seconds >= self.inactivity_timeout_seconds {
            Some(Presence::Away(AwayCause::Idle))
        } else {
            Some(Presence::Present)
        }
    }

    /// The last input: when idle began.
    fn idle_since(&self) -> DateTime<Utc> {
        self.now - Duration::seconds(self.idle_seconds as i64)
    }
}

/// Where the open span ends when it changes to `next`. Going idle from
/// `Present` is backdated to the last input (ADR-0005 decision 3), but never
/// before the span starts, so it cannot reach into an `Untracked` gap.
fn transition_instant(span: &OpenSpan, next: Presence, observation: &Observation) -> DateTime<Utc> {
    match (span.presence, next) {
        (Presence::Present, Presence::Away(AwayCause::Idle)) => {
            observation.idle_since().max(span.started_at)
        }
        _ => observation.now,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const THRESHOLD: u64 = 300;

    fn at(secs: i64) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-10-20T09:00:00Z")
            .unwrap()
            .with_timezone(&Utc)
            + Duration::seconds(secs)
    }

    /// An observation at `now` seconds, one second after the previous tick.
    fn observed(now: i64) -> Observation {
        Observation {
            idle_seconds: 0,
            inactivity_timeout_seconds: THRESHOLD,
            locked: false,
            paused: false,
            now: at(now),
            previous_tick: Some(at(now - 1)),
        }
    }

    #[test]
    fn a_present_observation_with_no_open_span_opens_present_at_the_tick() {
        let writes = derive(None, &observed(10));
        assert_eq!(
            writes,
            [Write::Open {
                started_at: at(10),
                ended_at: at(10),
                presence: Presence::Present
            }]
        );
    }

    fn open_span(started_at: i64, ended_at: i64, presence: Presence) -> OpenSpan {
        OpenSpan {
            started_at: at(started_at),
            ended_at: at(ended_at),
            presence,
        }
    }

    #[test]
    fn going_idle_truncates_present_to_the_last_input_and_backdates_away_there() {
        let present = open_span(0, 990, Presence::Present);
        let observation = Observation {
            idle_seconds: THRESHOLD,
            ..observed(1000)
        };
        assert_eq!(
            derive(Some(&present), &observation),
            [
                Write::Close { ended_at: at(700) },
                Write::Open {
                    started_at: at(700),
                    ended_at: at(1000),
                    presence: Presence::Away(AwayCause::Idle)
                }
            ]
        );
    }

    #[test]
    fn idle_away_is_never_backdated_before_the_open_span_starts() {
        // Idle since 700, but the open span only began at 900: before that is
        // an Untracked gap, which presence must not claim.
        let present = open_span(900, 990, Presence::Present);
        let observation = Observation {
            idle_seconds: THRESHOLD,
            ..observed(1000)
        };
        assert_eq!(
            derive(Some(&present), &observation),
            [
                Write::Discard,
                Write::Open {
                    started_at: at(900),
                    ended_at: at(1000),
                    presence: Presence::Away(AwayCause::Idle)
                }
            ]
        );
    }

    /// The writes for a span that ends at `at` and is followed by `next`,
    /// opened there and extended to `now`.
    fn handover(at: DateTime<Utc>, now: DateTime<Utc>, next: Presence) -> [Write; 2] {
        [
            Write::Close { ended_at: at },
            Write::Open {
                started_at: at,
                ended_at: now,
                presence: next,
            },
        ]
    }

    #[test]
    fn locking_while_idle_splits_away_at_the_lock_and_locked_is_the_cause() {
        let idle = open_span(700, 990, Presence::Away(AwayCause::Idle));
        let observation = Observation {
            idle_seconds: 400,
            locked: true,
            ..observed(1000)
        };
        assert_eq!(
            derive(Some(&idle), &observation),
            handover(at(1000), at(1000), Presence::Away(AwayCause::Locked))
        );
    }

    #[test]
    fn unlocking_while_still_idle_splits_away_at_the_unlock() {
        let locked = open_span(1000, 1990, Presence::Away(AwayCause::Locked));
        let observation = Observation {
            idle_seconds: 1300,
            ..observed(2000)
        };
        assert_eq!(
            derive(Some(&locked), &observation),
            handover(at(2000), at(2000), Presence::Away(AwayCause::Idle))
        );
    }

    #[test]
    fn input_after_away_returns_to_present_at_the_tick() {
        let idle = open_span(700, 1990, Presence::Away(AwayCause::Idle));
        assert_eq!(
            derive(Some(&idle), &observed(2000)),
            handover(at(2000), at(2000), Presence::Present)
        );
    }

    #[test]
    fn without_a_transition_the_heartbeat_moves_the_open_span_end_to_now() {
        let present = open_span(0, 970, Presence::Present);
        assert_eq!(
            derive(Some(&present), &observed(1000)),
            [Write::Extend { ended_at: at(1000) }]
        );
    }

    #[test]
    fn without_a_transition_nothing_is_written_between_heartbeats() {
        let present = open_span(0, 971, Presence::Present);
        assert_eq!(derive(Some(&present), &observed(1000)), []);
    }

    #[test]
    fn a_tick_gap_from_sleep_closes_at_the_previous_tick_and_leaves_the_gap_untracked() {
        // Asleep from 1000 to 9000; untouched since 990, so idle covers the sleep.
        let present = open_span(0, 990, Presence::Present);
        let resumed = Observation {
            idle_seconds: 8010,
            previous_tick: Some(at(1000)),
            ..observed(9000)
        };
        assert_eq!(
            derive(Some(&present), &resumed),
            [
                Write::Close { ended_at: at(1000) },
                Write::Open {
                    started_at: at(9000),
                    ended_at: at(9000),
                    presence: Presence::Away(AwayCause::Idle)
                }
            ]
        );
    }

    #[test]
    fn a_tick_gap_of_ten_seconds_is_not_sleep() {
        let present = open_span(0, 990, Presence::Present);
        let late = Observation {
            previous_tick: Some(at(990)),
            ..observed(1000)
        };
        assert_eq!(derive(Some(&present), &late), []);
    }

    #[test]
    fn raising_paused_closes_the_open_span_at_the_tick() {
        let present = open_span(0, 990, Presence::Present);
        let paused = Observation {
            paused: true,
            ..observed(1000)
        };
        assert_eq!(
            derive(Some(&present), &paused),
            [Write::Close { ended_at: at(1000) }]
        );
    }

    #[test]
    fn nothing_is_opened_while_paused_even_when_locked() {
        let paused_and_locked = Observation {
            paused: true,
            locked: true,
            ..observed(1000)
        };
        assert_eq!(derive(None, &paused_and_locked), []);
    }

    #[test]
    fn a_raised_threshold_ends_idle_away_from_the_next_observation() {
        let idle = open_span(700, 990, Presence::Away(AwayCause::Idle));
        let raised = Observation {
            idle_seconds: 400,
            inactivity_timeout_seconds: 600,
            ..observed(1000)
        };
        assert_eq!(
            derive(Some(&idle), &raised),
            handover(at(1000), at(1000), Presence::Present)
        );
    }

    #[test]
    fn a_lowered_threshold_backdates_no_further_than_the_open_span() {
        // A closed span before 900 is not the derivation's to rewrite.
        let present = open_span(900, 990, Presence::Present);
        let lowered = Observation {
            idle_seconds: 60,
            inactivity_timeout_seconds: 60,
            ..observed(1000)
        };
        assert_eq!(
            derive(Some(&present), &lowered),
            handover(at(940), at(1000), Presence::Away(AwayCause::Idle))
        );
    }

    #[test]
    fn a_sleep_right_after_a_span_opens_leaves_no_empty_span() {
        let present = open_span(1000, 1000, Presence::Present);
        let resumed = Observation {
            previous_tick: Some(at(1000)),
            ..observed(9000)
        };
        assert_eq!(
            derive(Some(&present), &resumed),
            [
                Write::Discard,
                Write::Open {
                    started_at: at(9000),
                    ended_at: at(9000),
                    presence: Presence::Present
                }
            ]
        );
    }

    #[test]
    fn a_clock_set_back_writes_nothing_until_it_passes_the_previous_tick() {
        let present = open_span(900, 990, Presence::Present);
        let set_back = Observation {
            locked: true,
            previous_tick: Some(at(1000)),
            ..observed(950)
        };
        assert_eq!(derive(Some(&present), &set_back), []);
    }
}
