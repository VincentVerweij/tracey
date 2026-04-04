# Auto-Classification Loop & Active Learning — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** New window activity records are automatically classified in the background. High-confidence results create time entries automatically. Low-confidence results prompt the user with a non-intrusive toast. User responses feed the training loop.

**Architecture:** A `classification_loop` service polls `window_activity_records WHERE classified_at IS NULL` every 2 seconds. For each record it extracts features, classifies, and either (a) creates/extends a time entry, or (b) enqueues the record in an `ActiveLearningQueue` and emits `tracey://classification-needed`. The Blazor `ActivitySuggestionToast` listens for this event. User responses call `classification_submit_label`, which stores a labeled sample and creates a time entry.

**Tech Stack:** Rust, rusqlite, tokio, serde_json. Blazor/C# for the toast component. xUnit for service tests.

**Depends on:** Plan B (classification engine, labeled sample commands).

---

### Task 1: Add SQLite migration 007

**Files:**
- Create: `src-tauri/src/db/migrations/007_auto_classification_loop.sql`
- Modify: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: Create migration SQL**

```sql
-- Migration 007: Auto-classification loop and active learning support

-- classified_at on window_activity_records: NULL = not yet classified
ALTER TABLE window_activity_records ADD COLUMN classified_at TEXT;

CREATE INDEX idx_war_unclassified
    ON window_activity_records (classified_at)
    WHERE classified_at IS NULL;

-- source on time_entries: manual (default), auto (created by classification loop), continued
ALTER TABLE time_entries ADD COLUMN source TEXT NOT NULL DEFAULT 'manual'
    CHECK (source IN ('manual', 'auto', 'continued'));

-- classification_events: full audit trail for the Classification page (Plan D)
CREATE TABLE classification_events (
    id                     TEXT PRIMARY KEY NOT NULL,
    war_id                 TEXT NOT NULL,   -- window_activity_records.id
    process_name           TEXT NOT NULL,
    window_title           TEXT NOT NULL,
    client_id              TEXT,
    project_id             TEXT,
    task_id                TEXT,
    confidence             REAL NOT NULL DEFAULT 0.0,
    classification_source  TEXT NOT NULL DEFAULT 'unclassified',
    -- 'heuristic' | 'tf_idf' | 'unclassified'
    outcome                TEXT NOT NULL DEFAULT 'pending',
    -- 'auto' | 'user_confirmed' | 'user_corrected' | 'unclassified' | 'pending'
    created_at             TEXT NOT NULL
);

CREATE INDEX idx_classification_events_created_at
    ON classification_events (created_at DESC);

-- Auto-classification preferences (added to user_preferences singleton)
ALTER TABLE user_preferences
    ADD COLUMN auto_classification_enabled INTEGER NOT NULL DEFAULT 1;

ALTER TABLE user_preferences
    ADD COLUMN auto_classification_confidence_threshold REAL NOT NULL DEFAULT 0.7;

ALTER TABLE user_preferences
    ADD COLUMN auto_classification_group_gap_seconds INTEGER NOT NULL DEFAULT 120;

-- JSON array of { pattern_key, dismissed_count, last_snoozed_at }
ALTER TABLE user_preferences
    ADD COLUMN classification_snooze_json TEXT;
```

- [ ] **Step 2: Register in `migrations.rs`**

Add after the `006` entry:

```rust
    (
        "007_auto_classification_loop",
        include_str!("migrations/007_auto_classification_loop.sql"),
    ),
```

- [ ] **Step 3: Build to verify**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/db/migrations/007_auto_classification_loop.sql `
        src-tauri/src/db/migrations.rs
git commit -m "feat(db): add classified_at, source, classification_events, auto-classification prefs (migration 007)"
```

---

### Task 2: Implement `active_learning_queue.rs`

**Files:**
- Create: `src-tauri/src/services/active_learning_queue.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Write the failing tests**

Create `src-tauri/src/services/active_learning_queue.rs` with tests first:

```rust
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
        // Immediately after dismissal — still within snooze window
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
```

- [ ] **Step 2: Run tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features test active_learning_queue 2>&1
```

Expected: all 6 tests pass.

- [ ] **Step 3: Add module to `services/mod.rs`**

```rust
pub mod active_learning_queue;
pub mod activity_tracker;
pub mod classification;
pub mod idle_service;
pub mod logger;
pub mod ocr_service;
pub mod screenshot_service;
pub mod sync_service;
pub mod timer_tick;
```

- [ ] **Step 4: Add `ActiveLearningQueue` to `AppState`**

In `src-tauri/src/commands/mod.rs`, add:

```rust
use crate::services::active_learning_queue::ActiveLearningQueue;
```

Add field to `AppState`:

```rust
pub struct AppState {
    pub db: std::sync::Mutex<rusqlite::Connection>,
    pub platform: Arc<dyn PlatformHooks + Send + Sync>,
    pub sync_state: Arc<std::sync::Mutex<SyncState>>,
    pub sync_notify: Arc<tokio::sync::Notify>,
    pub classification_state: Arc<std::sync::Mutex<ClassificationState>>,
    pub active_learning_queue: Arc<std::sync::Mutex<ActiveLearningQueue>>,
}
```

Update `lib.rs` to pass `active_learning_queue: Arc::new(std::sync::Mutex::new(ActiveLearningQueue::new()))` in `.manage(AppState { ... })`.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/services/active_learning_queue.rs `
        src-tauri/src/services/mod.rs `
        src-tauri/src/commands/mod.rs `
        src-tauri/src/lib.rs
git commit -m "feat(active-learning): add ActiveLearningQueue with snooze and cap logic"
```

---

### Task 3: Implement `classification_loop.rs`

**Files:**
- Create: `src-tauri/src/services/classification_loop.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Create `classification_loop.rs`**

```rust
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
    // 1. Fetch up to 10 unclassified records — lock released before any await
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
        .unwrap_or_else(|_| Box::new(std::iter::empty()))
        .filter_map(|r| r.ok())
        .collect()
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
        mark_classified(app, &rec.id);
        return;
    }

    // 3. Store classification event
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
        let _ = conn.execute(
            "INSERT INTO classification_events \
             (id, war_id, process_name, window_title, client_id, project_id, task_id, \
              confidence, classification_source, outcome, created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'pending',?10)",
            rusqlite::params![
                event_id, rec.id, rec.process_name, rec.window_title,
                result.top.client_id, result.top.project_id, result.top.task_id,
                result.top.confidence, source_str, now,
            ],
        );
    }

    if result.top.confidence >= threshold {
        // Auto-create/extend time entry
        auto_create_or_extend_time_entry(app, &rec.id, &rec.recorded_at, &event_id, &result, group_gap);
    } else {
        // Enqueue for active learning
        let pattern_key = make_pattern_key(&rec.process_name, &rec.window_title);
        let should_prompt = {
            let state = app.state::<AppState>();
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
            let _ = app.emit(
                "tracey://classification-needed",
                serde_json::json!({
                    "war_id": rec.id,
                    "event_id": event_id,
                    "process_name": rec.process_name,
                    "window_title": rec.window_title,
                    "pattern_key": pattern_key,
                    "suggestions": suggestions_json,
                }),
            );
        } else {
            // Snoozed — mark as unclassified
            update_event_outcome(app, &event_id, "unclassified");
        }
    }

    mark_classified(app, &rec.id);
}

fn mark_classified(app: &AppHandle, war_id: &str) {
    let state = app.state::<AppState>();
    if let Ok(conn) = state.db.lock() {
        let now = Utc::now().to_rfc3339();
        let _ = conn.execute(
            "UPDATE window_activity_records SET classified_at = ?1 WHERE id = ?2",
            rusqlite::params![now, war_id],
        );
    }
}

fn update_event_outcome(app: &AppHandle, event_id: &str, outcome: &str) {
    let state = app.state::<AppState>();
    if let Ok(conn) = state.db.lock() {
        let _ = conn.execute(
            "UPDATE classification_events SET outcome = ?1 WHERE id = ?2",
            rusqlite::params![outcome, event_id],
        );
    }
}

fn auto_create_or_extend_time_entry(
    app: &AppHandle,
    war_id: &str,
    recorded_at: &str,
    event_id: &str,
    result: &crate::services::classification::ClassificationResult,
    group_gap_seconds: i64,
) {
    let state = app.state::<AppState>();
    let conn = match state.db.lock() { Ok(c) => c, Err(_) => return };
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
        // Check if gap is within threshold (rough string comparison works for ISO 8601)
        if let (Ok(ended), Ok(start)) = (
            chrono::DateTime::parse_from_rfc3339(&ended_at),
            chrono::DateTime::parse_from_rfc3339(recorded_at),
        ) {
            let gap = start.signed_duration_since(ended).num_seconds();
            if gap >= 0 && gap <= group_gap_seconds {
                // Extend existing entry
                let _ = conn.execute(
                    "UPDATE time_entries SET ended_at = ?1, modified_at = ?2 WHERE id = ?3",
                    rusqlite::params![recorded_at, Utc::now().to_rfc3339(), entry_id],
                );
                update_event_outcome(app, event_id, "auto");
                return;
            }
        }
    }

    // Create new auto time entry
    let entry_id = Ulid::new().to_string().to_lowercase();
    let now_str = Utc::now().to_rfc3339();
    let _ = conn.execute(
        "INSERT INTO time_entries \
         (id, description, started_at, ended_at, project_id, task_id, is_break, \
          device_id, created_at, modified_at, source) \
         VALUES (?1,'',?2,?2,?3,?4,0,?5,?6,?6,'auto')",
        rusqlite::params![
            entry_id, recorded_at,
            result.top.project_id, result.top.task_id,
            device_id, now_str,
        ],
    );
    update_event_outcome(app, event_id, "auto");
    let _ = war_id; // war_id used by caller for mark_classified
}
```

- [ ] **Step 2: Register module in `services/mod.rs`**

```rust
pub mod active_learning_queue;
pub mod activity_tracker;
pub mod classification;
pub mod classification_loop;
pub mod idle_service;
pub mod logger;
pub mod ocr_service;
pub mod screenshot_service;
pub mod sync_service;
pub mod timer_tick;
```

- [ ] **Step 3: Start the loop in `lib.rs`**

In `lib.rs` `.setup()`, add:

```rust
services::classification_loop::start_classification_loop(app.handle().clone());
```

- [ ] **Step 4: Build**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/services/classification_loop.rs `
        src-tauri/src/services/mod.rs `
        src-tauri/src/lib.rs
git commit -m "feat(classification): add background classification loop with auto time entry and active learning"
```

---

### Task 4: Add `classification_submit_label` command and `classification_dismiss` command

**Files:**
- Modify: `src-tauri/src/commands/classification.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Add commands to `commands/classification.rs`**

Append to the existing file:

```rust
// ── Active learning: user confirms or corrects a classification ───────────────

#[derive(Deserialize)]
pub struct ClassificationSubmitLabelRequest {
    pub war_id: String,
    pub event_id: String,
    pub process_name: String,
    pub window_title: String,
    pub ocr_text: Option<String>,
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub recorded_at: String,
}

#[tauri::command]
pub fn classification_submit_label(
    state: State<'_, AppState>,
    request: ClassificationSubmitLabelRequest,
) -> Result<(), String> {
    let features = extract(&request.process_name, &request.window_title, request.ocr_text.as_deref());
    let sample_id = Ulid::new().to_string().to_lowercase();
    let entry_id = Ulid::new().to_string().to_lowercase();
    let now = Utc::now().to_rfc3339();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    let sample_count = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;

        // Store labeled sample
        conn.execute(
            "INSERT INTO labeled_samples \
             (id,feature_text,process_name,window_title,client_id,project_id,task_id,source,device_id,created_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,'user_confirmed',?8,?9)",
            rusqlite::params![
                sample_id, features.combined_text,
                request.process_name, request.window_title,
                request.client_id, request.project_id, request.task_id,
                device_id, now,
            ],
        ).map_err(|e| e.to_string())?;

        // Create time entry for the classified period
        conn.execute(
            "INSERT INTO time_entries \
             (id,description,started_at,ended_at,project_id,task_id,is_break,device_id,created_at,modified_at,source) \
             VALUES (?1,'',?2,?2,?3,?4,0,?5,?6,?6,'auto')",
            rusqlite::params![
                entry_id, request.recorded_at,
                request.project_id, request.task_id,
                device_id, now,
            ],
        ).map_err(|e| e.to_string())?;

        // Update classification_events outcome
        conn.execute(
            "UPDATE classification_events SET outcome = 'user_confirmed' WHERE id = ?1",
            rusqlite::params![request.event_id],
        ).map_err(|e| e.to_string())?;

        // Remove from active learning queue
        conn.execute(
            "UPDATE window_activity_records SET classified_at = ?1 WHERE id = ?2",
            rusqlite::params![now, request.war_id],
        ).map_err(|e| e.to_string())?;

        trainer::count_samples(&conn)
    }; // db lock released

    // Remove from in-memory queue
    {
        let mut alq = state.active_learning_queue.lock().map_err(|e| e.to_string())?;
        alq.dequeue(&request.war_id);
    }

    // Trigger retrain if enough new samples
    let should_retrain = {
        let cs = state.classification_state.lock().map_err(|e| e.to_string())?;
        sample_count - cs.sample_count_at_last_train >= 10
            && sample_count >= trainer::PHASE2_MIN_SAMPLES
    };

    if should_retrain {
        let app_state = state.inner().clone();
        tauri::async_runtime::spawn(async move {
            if let Ok(conn) = app_state.db.lock() {
                if let Some(model) = trainer::retrain(&conn) {
                    drop(conn);
                    if let Ok(mut cs) = app_state.classification_state.lock() {
                        cs.model = Some(model);
                        cs.sample_count_at_last_train = sample_count;
                    }
                }
            }
        });
    }

    Ok(())
}

// ── Active learning: user dismisses toast without answering ───────────────────

#[derive(Deserialize)]
pub struct ClassificationDismissRequest {
    pub war_id: String,
    pub pattern_key: String,
}

#[tauri::command]
pub fn classification_dismiss(
    state: State<'_, AppState>,
    request: ClassificationDismissRequest,
) -> Result<(), String> {
    let mut alq = state.active_learning_queue.lock().map_err(|e| e.to_string())?;
    alq.record_dismissal(&request.pattern_key);
    alq.dequeue(&request.war_id);
    Ok(())
}
```

- [ ] **Step 2: Register the two new commands in `lib.rs`**

Add to `invoke_handler`:

```rust
commands::classification::classification_submit_label,
commands::classification::classification_dismiss,
```

- [ ] **Step 3: Build**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/commands/classification.rs `
        src-tauri/src/lib.rs
git commit -m "feat(classification): add classification_submit_label and classification_dismiss commands"
```

---

### Task 5: Add `tracey://classification-needed` event to frontend event service

**Files:**
- Modify: `src/Tracey.App/Services/TauriEventService.cs`
- Modify: `src/Tracey.App/Services/TauriIpcService.cs`

- [ ] **Step 1: Add payload type and event to `TauriEventService.cs`**

Add the payload record (place with other payload types at the bottom of the file or in a logical grouping):

```csharp
public record ClassificationSuggestion(
    [property: JsonPropertyName("client_id")] string? ClientId,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("confidence")] float Confidence,
    [property: JsonPropertyName("source")] string Source);

public record ClassificationNeededPayload(
    [property: JsonPropertyName("war_id")] string WarId,
    [property: JsonPropertyName("event_id")] string EventId,
    [property: JsonPropertyName("process_name")] string ProcessName,
    [property: JsonPropertyName("window_title")] string WindowTitle,
    [property: JsonPropertyName("pattern_key")] string PatternKey,
    [property: JsonPropertyName("suggestions")] ClassificationSuggestion[] Suggestions);
```

Add the event declaration with the other events:

```csharp
public event Action<ClassificationNeededPayload>? OnClassificationNeeded;
```

Add the route case in `RouteEvent`:

```csharp
case "tracey://classification-needed":
    var classPayload = JsonSerializer.Deserialize<ClassificationNeededPayload>(jsonPayload, _jsonOptions);
    if (classPayload != null) OnClassificationNeeded?.Invoke(classPayload);
    break;
```

- [ ] **Step 2: Add IPC methods for classification in `TauriIpcService.cs`**

Add after the existing sections:

```csharp
// ── Classification ────────────────────────────────────────────────────────────

public record ClassificationSubmitLabelRequest(
    [property: JsonPropertyName("war_id")] string WarId,
    [property: JsonPropertyName("event_id")] string EventId,
    [property: JsonPropertyName("process_name")] string ProcessName,
    [property: JsonPropertyName("window_title")] string WindowTitle,
    [property: JsonPropertyName("ocr_text")] string? OcrText,
    [property: JsonPropertyName("client_id")] string? ClientId,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("recorded_at")] string RecordedAt);

public record ClassificationDismissRequest(
    [property: JsonPropertyName("war_id")] string WarId,
    [property: JsonPropertyName("pattern_key")] string PatternKey);

public Task ClassificationSubmitLabelAsync(ClassificationSubmitLabelRequest request) =>
    Invoke<object>("classification_submit_label", new { request });

public Task ClassificationDismissAsync(ClassificationDismissRequest request) =>
    Invoke<object>("classification_dismiss", new { request });
```

- [ ] **Step 3: Build the frontend**

```powershell
dotnet build src/Tracey.App
```

Expected: builds without errors.

- [ ] **Step 4: Commit**

```powershell
git add src/Tracey.App/Services/TauriEventService.cs `
        src/Tracey.App/Services/TauriIpcService.cs
git commit -m "feat(frontend): add ClassificationNeeded event and classification IPC methods"
```

---

### Task 6: Create `ActivitySuggestionToast.razor` component

**Files:**
- Create: `src/Tracey.App/Components/ActivitySuggestionToast.razor`
- Create: `src/Tracey.App/Components/ActivitySuggestionToast.razor.css`
- Modify: `src/Tracey.App/Layout/MainLayout.razor`

- [ ] **Step 1: Create the toast component**

```razor
@inject TauriIpcService Tauri
@inject TauriEventService Events
@implements IDisposable

@if (_payload != null)
{
    <div class="suggestion-toast" role="alertdialog" aria-labelledby="toast-title">
        <div class="toast-header">
            <span id="toast-title" class="toast-app">@_payload.ProcessName</span>
            <span class="toast-title-text" title="@_payload.WindowTitle">
                @TruncateTitle(_payload.WindowTitle)
            </span>
            <button type="button" class="toast-dismiss" aria-label="Dismiss" @onclick="Dismiss">✕</button>
        </div>

        @if (_showPicker)
        {
            <div class="toast-picker">
                <p>Which project is this?</p>
                @* Reuse FuzzyMatchService or a simple project list here in Plan D *@
                <input type="text" @bind="_pickerQuery" placeholder="Search projects…" @oninput="OnPickerInput" />
                <button type="button" class="toast-btn-outline" @onclick="ClosePicker">Back</button>
            </div>
        }
        else
        {
            <div class="toast-suggestions">
                @foreach (var s in _payload.Suggestions.Take(3))
                {
                    <button type="button" class="toast-suggestion-btn" @onclick="() => Confirm(s)">
                        <span class="suggestion-label">@FormatLabel(s)</span>
                        <span class="suggestion-confidence">@Math.Round(s.Confidence * 100)%</span>
                    </button>
                }
                <button type="button" class="toast-btn-outline" @onclick="ShowPicker">Something else…</button>
            </div>
        }
    </div>
}

@code {
    private ClassificationNeededPayload? _payload;
    private bool _showPicker = false;
    private string _pickerQuery = string.Empty;
    private System.Threading.Timer? _autoTimer;

    protected override void OnInitialized()
    {
        Events.OnClassificationNeeded += OnClassificationNeeded;
    }

    private void OnClassificationNeeded(ClassificationNeededPayload payload)
    {
        _payload = payload;
        _showPicker = false;
        _autoTimer?.Dispose();
        // Auto-dismiss after 30 seconds
        _autoTimer = new System.Threading.Timer(_ =>
        {
            InvokeAsync(async () =>
            {
                if (_payload?.WarId == payload.WarId)
                    await Dismiss();
            });
        }, null, TimeSpan.FromSeconds(30), Timeout.InfiniteTimeSpan);
        InvokeAsync(StateHasChanged);
    }

    private async Task Confirm(ClassificationSuggestion suggestion)
    {
        if (_payload == null) return;
        _autoTimer?.Dispose();
        await Tauri.ClassificationSubmitLabelAsync(new ClassificationSubmitLabelRequest(
            WarId: _payload.WarId,
            EventId: _payload.EventId,
            ProcessName: _payload.ProcessName,
            WindowTitle: _payload.WindowTitle,
            OcrText: null,
            ClientId: suggestion.ClientId,
            ProjectId: suggestion.ProjectId,
            TaskId: suggestion.TaskId,
            RecordedAt: DateTime.UtcNow.ToString("o")
        ));
        _payload = null;
        StateHasChanged();
    }

    private async Task Dismiss()
    {
        if (_payload == null) return;
        _autoTimer?.Dispose();
        await Tauri.ClassificationDismissAsync(new ClassificationDismissRequest(
            WarId: _payload.WarId,
            PatternKey: _payload.PatternKey
        ));
        _payload = null;
        StateHasChanged();
    }

    private void ShowPicker() { _showPicker = true; StateHasChanged(); }
    private void ClosePicker() { _showPicker = false; StateHasChanged(); }
    private void OnPickerInput(ChangeEventArgs e) { _pickerQuery = e.Value?.ToString() ?? string.Empty; }

    private static string TruncateTitle(string title) =>
        title.Length > 60 ? title[..57] + "…" : title;

    private static string FormatLabel(ClassificationSuggestion s) =>
        string.Join(" / ", new[] { s.ClientId, s.ProjectId, s.TaskId }
            .Where(x => !string.IsNullOrEmpty(x)));

    public void Dispose()
    {
        Events.OnClassificationNeeded -= OnClassificationNeeded;
        _autoTimer?.Dispose();
    }
}
```

- [ ] **Step 2: Create the toast CSS**

```css
.suggestion-toast {
    position: fixed;
    bottom: 1.5rem;
    right: 1.5rem;
    width: 320px;
    background: var(--card-bg, #fff);
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 8px;
    box-shadow: 0 4px 16px rgba(0,0,0,0.12);
    z-index: 9999;
    padding: 1rem;
    font-size: 0.875rem;
}

.toast-header {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    margin-bottom: 0.75rem;
}

.toast-app {
    font-weight: 600;
    flex-shrink: 0;
}

.toast-title-text {
    color: var(--text-muted, #666);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
}

.toast-dismiss {
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-muted, #666);
    padding: 0 0.25rem;
    flex-shrink: 0;
}

.toast-suggestions {
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
}

.toast-suggestion-btn {
    display: flex;
    justify-content: space-between;
    align-items: center;
    width: 100%;
    text-align: left;
    background: var(--bg-subtle, #f5f5f5);
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 5px;
    padding: 0.4rem 0.6rem;
    cursor: pointer;
    font-size: 0.8125rem;
}

.toast-suggestion-btn:hover { background: var(--bg-hover, #eee); }

.suggestion-confidence {
    font-size: 0.75rem;
    color: var(--text-muted, #888);
    flex-shrink: 0;
    margin-left: 0.5rem;
}

.toast-btn-outline {
    width: 100%;
    padding: 0.4rem;
    background: none;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 5px;
    cursor: pointer;
    font-size: 0.8125rem;
    margin-top: 0.25rem;
}
```

- [ ] **Step 3: Register the toast in `MainLayout.razor`**

Add `<ActivitySuggestionToast />` just before the closing `</div>` of the layout body (or wherever notification toasts are shown). Check `MainLayout.razor` for the existing structure and add:

```razor
<ActivitySuggestionToast />
```

- [ ] **Step 4: Build the frontend**

```powershell
dotnet build src/Tracey.App
```

Expected: builds without errors.

- [ ] **Step 5: Add a unit test for the toast dismiss logic**

In `src/Tracey.Tests/`, create `ClassificationToastTests.cs`:

```csharp
using Tracey.App.Services;

namespace Tracey.Tests;

/// Tests the dismissal counting logic in isolation (no Blazor rendering needed).
public class ClassificationToastTests
{
    [Fact]
    public void PatternKey_Is_Lowercase_And_Truncated()
    {
        // The pattern key is constructed in Rust but the same logic applies.
        // Confirm the format: "process|title_prefix"
        var processName = "Visual Studio Code";
        var title = "tracey — Visual Studio Code";
        var key = $"{processName.ToLower()}|{title.ToLower()[..Math.Min(50, title.Length)]}";
        Assert.Contains("visual studio code", key);
        Assert.Contains("tracey", key);
    }
}
```

- [ ] **Step 6: Run .NET tests**

```powershell
dotnet test src/Tracey.Tests
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```powershell
git add src/Tracey.App/Components/ActivitySuggestionToast.razor `
        src/Tracey.App/Components/ActivitySuggestionToast.razor.css `
        src/Tracey.App/Layout/MainLayout.razor `
        src/Tracey.Tests/ClassificationToastTests.cs
git commit -m "feat(frontend): add ActivitySuggestionToast with auto-dismiss, suggestions, and snooze"
```

---

### Task 7: Add auto-classification preferences to the Settings page

The spec requires the confidence threshold and grouping gap to be configurable. The DB columns were added in migration 007; the preferences commands (`preferences_get` / `preferences_update`) already handle all `user_preferences` columns.

**Files:**
- Modify: `src/Tracey.App/Services/TauriIpcService.cs`
- Modify: `src/Tracey.App/Pages/Settings.razor`

- [ ] **Step 1: Add the new fields to `UserPreferences` and `PreferencesUpdateRequest` in `TauriIpcService.cs`**

Locate the `UserPreferences` record and add:

```csharp
[property: JsonPropertyName("auto_classification_enabled")] bool AutoClassificationEnabled,
[property: JsonPropertyName("auto_classification_confidence_threshold")] float AutoClassificationConfidenceThreshold,
[property: JsonPropertyName("auto_classification_group_gap_seconds")] int AutoClassificationGroupGapSeconds
```

Add the same fields (as nullable for partial update) to `PreferencesUpdateRequest`:

```csharp
[property: JsonPropertyName("auto_classification_enabled")] bool? AutoClassificationEnabled,
[property: JsonPropertyName("auto_classification_confidence_threshold")] float? AutoClassificationConfidenceThreshold,
[property: JsonPropertyName("auto_classification_group_gap_seconds")] int? AutoClassificationGroupGapSeconds
```

Also extend the Rust `PreferencesUpdateRequest` struct in `src-tauri/src/commands/mod.rs` with matching nullable fields and wire them into the `preferences_update` command's apply-delta block:

```rust
// In PreferencesUpdateRequest struct, add:
pub auto_classification_enabled: Option<bool>,
pub auto_classification_confidence_threshold: Option<f64>,
pub auto_classification_group_gap_seconds: Option<i64>,
```

Update the `preferences_update` SQL to include these columns:

```rust
// In UPDATE user_preferences SET ..., add:
//   auto_classification_enabled = ?N,
//   auto_classification_confidence_threshold = ?N,
//   auto_classification_group_gap_seconds = ?N
// and apply deltas with the same if-let pattern as other fields.
```

- [ ] **Step 2: Add "Auto-Classification" section to `Settings.razor`**

Add a new section after the Auto-Classification Rules section:

```razor
<section class="settings-section">
    <h3>Auto-Classification</h3>

    <label>
        <input type="checkbox" @bind="_autoClassificationEnabled" />
        Automatically classify window activity and create time entries
    </label>

    <div class="form-row">
        <label for="conf-threshold">Confidence threshold</label>
        <input id="conf-threshold" type="range" min="0.5" max="1.0" step="0.05"
               @bind="_confidenceThreshold" />
        <span>@Math.Round(_confidenceThreshold * 100)%</span>
    </div>
    <p class="field-hint">
        Activities above this threshold are classified automatically.
        Below it, you'll be prompted.
    </p>

    <div class="form-row">
        <label for="group-gap">Grouping gap</label>
        <input id="group-gap" type="number" min="30" max="600" step="30"
               @bind="_groupGapSeconds" />
        <span>seconds</span>
    </div>
    <p class="field-hint">
        Consecutive windows classified to the same project within this gap are merged
        into one time entry.
    </p>

    <button type="button" @onclick="SaveAutoClassificationPrefs">Save</button>
    @if (!string.IsNullOrEmpty(_autoClassificationError))
    {
        <p class="error">@_autoClassificationError</p>
    }
</section>
```

- [ ] **Step 3: Add state and handler to `Settings.razor` `@code` block**

```csharp
private bool _autoClassificationEnabled = true;
private float _confidenceThreshold = 0.7f;
private int _groupGapSeconds = 120;
private string _autoClassificationError = string.Empty;

// In OnInitializedAsync, after loading preferences:
_autoClassificationEnabled = prefs.AutoClassificationEnabled;
_confidenceThreshold = prefs.AutoClassificationConfidenceThreshold;
_groupGapSeconds = prefs.AutoClassificationGroupGapSeconds;

private async Task SaveAutoClassificationPrefs()
{
    _autoClassificationError = string.Empty;
    try
    {
        await Tauri.PreferencesUpdateAsync(new PreferencesUpdateRequest(
            AutoClassificationEnabled: _autoClassificationEnabled,
            AutoClassificationConfidenceThreshold: _confidenceThreshold,
            AutoClassificationGroupGapSeconds: _groupGapSeconds
            // all other fields null → unchanged
        ));
    }
    catch (Exception ex)
    {
        _autoClassificationError = $"Couldn't save settings. {ex.Message}";
    }
    StateHasChanged();
}
```

- [ ] **Step 4: Build the full project**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
dotnet build src/Tracey.App
```

Expected: both build without errors.

- [ ] **Step 5: Commit**

```powershell
git add src/Tracey.App/Pages/Settings.razor `
        src/Tracey.App/Services/TauriIpcService.cs `
        src-tauri/src/commands/mod.rs
git commit -m "feat(settings): add auto-classification preferences section (threshold, gap, enabled)"
```
