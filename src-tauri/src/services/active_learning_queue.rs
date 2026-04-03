//! In-memory queue for low-confidence classification records pending user input.
//! Enforces snooze logic: a pattern dismissed ≥ 3 times is silently suppressed.

use std::collections::HashMap;
use chrono::{DateTime, Utc, Duration};
use serde::{Deserialize, Serialize};

/// A pattern key is "{process_name}|{title_prefix}" (title truncated at 50 chars).
pub fn make_pattern_key(process_name: &str, window_title: &str) -> String {
    let prefix: String = window_title.chars().take(50).collect();
    format!("{}|{}", process_name.to_lowercase(), prefix.to_lowercase())
}

#[derive(Debug, Clone)]
pub struct PendingRecord {
    pub war_id: String,
    pub process_name: String,
    pub window_title: String,
    pub ocr_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnoozeEntry {
    pub dismissed_count: u32,
    pub last_snoozed_at: DateTime<Utc>,
}

#[derive(Debug, Default)]
pub struct ActiveLearningQueue {
    /// pattern_key → snooze state
    snooze: HashMap<String, SnoozeState>,
    /// Currently pending records (war_id → record)
    pending: HashMap<String, PendingRecord>,
}

#[derive(Debug)]
struct SnoozeState {
    dismissed_count: u32,
    last_snoozed_at: DateTime<Utc>,
}

const SNOOZE_MINUTES: i64 = 10;
const SNOOZE_CAP: u32 = 3;

impl ActiveLearningQueue {
    pub fn new() -> Self { Self::default() }

    /// Returns `true` if the prompt should be shown for this pattern.
    /// Returns `false` if snoozed (within 10 min) or permanently suppressed (≥ 3 dismissals).
    pub fn should_prompt(&self, pattern_key: &str) -> bool {
        match self.snooze.get(pattern_key) {
            None => true,
            Some(s) => {
                if s.dismissed_count >= SNOOZE_CAP { return false; }
                let elapsed = Utc::now() - s.last_snoozed_at;
                elapsed > Duration::minutes(SNOOZE_MINUTES)
            }
        }
    }

    /// Enqueue a record for user prompting. Returns `true` if the record was
    /// accepted (not snoozed/suppressed) and the frontend should be notified.
    pub fn enqueue(&mut self, record: PendingRecord) -> bool {
        let key = make_pattern_key(&record.process_name, &record.window_title);
        if !self.should_prompt(&key) { return false; }
        self.pending.insert(record.war_id.clone(), record);
        true
    }

    /// Remove a record from the pending queue (user responded or toast auto-dismissed).
    pub fn dequeue(&mut self, war_id: &str) {
        self.pending.remove(war_id);
    }

    /// Record a dismissal (user closed toast without answering).
    pub fn record_dismissal(&mut self, pattern_key: &str) {
        let entry = self.snooze.entry(pattern_key.to_string()).or_insert(SnoozeState {
            dismissed_count: 0,
            last_snoozed_at: Utc::now(),
        });
        entry.dismissed_count += 1;
        entry.last_snoozed_at = Utc::now();
    }

    pub fn pending_count(&self) -> usize { self.pending.len() }

    /// Returns the snooze map serializable for DB persistence.
    pub fn snooze_state(&self) -> HashMap<String, SnoozeEntry> {
        self.snooze.iter().map(|(k, v)| (k.clone(), SnoozeEntry {
            dismissed_count: v.dismissed_count,
            last_snoozed_at: v.last_snoozed_at,
        })).collect()
    }

    /// Load persisted snooze state back into the queue on startup.
    pub fn load_snooze_state(&mut self, entries: HashMap<String, SnoozeEntry>) {
        for (key, entry) in entries {
            self.snooze.insert(key, SnoozeState {
                dismissed_count: entry.dismissed_count,
                last_snoozed_at: entry.last_snoozed_at,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_pattern_should_prompt() {
        let q = ActiveLearningQueue::new();
        assert!(q.should_prompt("code|tracey"));
    }

    #[test]
    fn snoozed_pattern_suppressed_within_window() {
        let mut q = ActiveLearningQueue::new();
        q.record_dismissal("code|tracey");
        assert!(!q.should_prompt("code|tracey"));
    }

    #[test]
    fn permanently_suppressed_after_cap() {
        let mut q = ActiveLearningQueue::new();
        for _ in 0..SNOOZE_CAP {
            q.record_dismissal("code|tracey");
        }
        assert!(!q.should_prompt("code|tracey"));
    }

    #[test]
    fn enqueue_accepts_new_pattern() {
        let mut q = ActiveLearningQueue::new();
        let accepted = q.enqueue(PendingRecord {
            war_id: "id1".into(),
            process_name: "Code".into(),
            window_title: "tracey".into(),
            ocr_text: None,
        });
        assert!(accepted);
        assert_eq!(q.pending_count(), 1);
    }

    #[test]
    fn enqueue_rejects_snoozed_pattern() {
        let mut q = ActiveLearningQueue::new();
        q.record_dismissal(&make_pattern_key("Code", "tracey"));
        let accepted = q.enqueue(PendingRecord {
            war_id: "id1".into(),
            process_name: "Code".into(),
            window_title: "tracey".into(),
            ocr_text: None,
        });
        assert!(!accepted);
        assert_eq!(q.pending_count(), 0);
    }

    #[test]
    fn dequeue_removes_record() {
        let mut q = ActiveLearningQueue::new();
        q.enqueue(PendingRecord { war_id: "id1".into(), process_name: "X".into(), window_title: "Y".into(), ocr_text: None });
        q.dequeue("id1");
        assert_eq!(q.pending_count(), 0);
    }
}
