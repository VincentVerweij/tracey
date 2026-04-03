# Cross-Device Sync for Classification Data — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `labeled_samples` and `classifier_model` are synced to the external Postgres database so the classification engine picks up training data from other devices and a new device is useful immediately after first sync.

**Architecture:** Extend `sync_service.rs` EXTERNAL_DDL with two new tables. In `run_sync_cycle_inline`, upsert `labeled_samples` (same last-write-wins pattern as all other tables). The `classifier_model` is always rebuilt from samples locally — it is stored in Postgres for audit/portability only. After a sync cycle that receives new samples, trigger a background retrain.

**Tech Stack:** Rust, tokio-postgres, rusqlite, sync_service.rs (existing patterns).

**Depends on:** Plan B (labeled_samples table and schema), Plan C (sync trigger after label submission — already done via the existing sync_notify mechanism).

---

### Task 1: Add `labeled_samples` and `classifier_model` to Postgres DDL

**Files:**
- Modify: `src-tauri/src/services/sync_service.rs`

The existing `EXTERNAL_DDL` const is the single source of truth for the Postgres schema. It is applied idempotently via `IF NOT EXISTS` guards.

- [ ] **Step 1: Increment `EXTERNAL_SCHEMA_VERSION`**

In `sync_service.rs`, change:

```rust
const EXTERNAL_SCHEMA_VERSION: i32 = 1;
```

to:

```rust
const EXTERNAL_SCHEMA_VERSION: i32 = 2;
```

- [ ] **Step 2: Append the new tables to `EXTERNAL_DDL`**

At the end of the `EXTERNAL_DDL` string constant (just before the closing `"#;`), add:

```sql

CREATE TABLE IF NOT EXISTS labeled_samples (
    id            TEXT PRIMARY KEY,
    feature_text  TEXT NOT NULL,
    process_name  TEXT NOT NULL,
    window_title  TEXT NOT NULL,
    client_id     TEXT,
    project_id    TEXT,
    task_id       TEXT,
    source        TEXT NOT NULL,
    device_id     TEXT NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL,
    modified_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    synced_at     TIMESTAMPTZ
);

DO $$ BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_indexes WHERE indexname = 'idx_labeled_samples_created'
    ) THEN
        CREATE INDEX idx_labeled_samples_created
            ON labeled_samples (device_id, created_at DESC);
    END IF;
END $$;

CREATE TABLE IF NOT EXISTS classifier_model (
    id           TEXT PRIMARY KEY,
    model_json   TEXT NOT NULL,
    trained_at   TIMESTAMPTZ NOT NULL,
    sample_count INTEGER NOT NULL,
    device_id    TEXT NOT NULL
);
```

- [ ] **Step 3: Build to verify DDL string compiles**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/services/sync_service.rs
git commit -m "feat(sync): add labeled_samples and classifier_model to Postgres DDL (schema v2)"
```

---

### Task 2: Sync `labeled_samples` in `run_sync_cycle_inline`

**Files:**
- Modify: `src-tauri/src/services/sync_service.rs`

The existing sync cycle already handles clients, projects, tasks, time_entries, and window_activity_records. Follow the same pattern for labeled_samples.

- [ ] **Step 1: Add a helper to read unsynced labeled samples from SQLite**

Find the section in `sync_service.rs` where other `read_*` helpers are defined (e.g., `read_clients`, `read_window_activity_records`). Add:

```rust
fn read_unsynced_labeled_samples(
    conn: &std::sync::MutexGuard<rusqlite::Connection>,
    limit: usize,
) -> Vec<(String, String, String, String, Option<String>, Option<String>, Option<String>, String, String, String, String)> {
    // Returns (id, feature_text, process_name, window_title, client_id, project_id,
    //          task_id, source, device_id, created_at, modified_at)
    let mut stmt = match conn.prepare(
        "SELECT id, feature_text, process_name, window_title, client_id, project_id, \
                task_id, source, device_id, created_at, modified_at \
         FROM labeled_samples WHERE synced_at IS NULL LIMIT ?1",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map(rusqlite::params![limit as i64], |r| {
        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?,
            r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?, r.get(8)?, r.get(9)?, r.get(10)?))
    })
    .unwrap_or_else(|_| Box::new(std::iter::empty()))
    .filter_map(|r| r.ok())
    .collect()
}
```

- [ ] **Step 2: Add `labeled_samples` upsert in `run_sync_cycle_inline`**

Inside `run_sync_cycle_inline`, after the existing time_entries sync block and before the function returns, add:

```rust
// ── labeled_samples sync ──────────────────────────────────────────────────────
let samples = {
    let conn = db.lock().map_err(|e| e.to_string())?;
    read_unsynced_labeled_samples(&conn, 200)
};

let mut samples_ok = 0i64;
let mut samples_err = 0i64;
let mut samples_ids: Vec<String> = Vec::new();

for (id, feature_text, process_name, window_title,
     client_id, project_id, task_id, source, device_id, created_at, modified_at) in &samples
{
    let result = pg_client.execute(
        "INSERT INTO labeled_samples \
         (id, feature_text, process_name, window_title, client_id, project_id, \
          task_id, source, device_id, created_at, modified_at) \
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10::TIMESTAMPTZ,$11::TIMESTAMPTZ) \
         ON CONFLICT (id) DO UPDATE SET \
             feature_text = EXCLUDED.feature_text, \
             client_id = EXCLUDED.client_id, \
             project_id = EXCLUDED.project_id, \
             task_id = EXCLUDED.task_id, \
             source = EXCLUDED.source, \
             modified_at = EXCLUDED.modified_at \
         WHERE EXCLUDED.modified_at > labeled_samples.modified_at",
        &[id, feature_text, process_name, window_title,
          client_id, project_id, task_id, source, device_id, created_at, modified_at],
    ).await;
    match result {
        Ok(_) => { samples_ok += 1; samples_ids.push(id.clone()); }
        Err(e) => {
            samples_err += 1;
            log::warn!("[sync] labeled_samples upsert error for {id}: {e}");
        }
    }
}

// Mark synced locally
if !samples_ids.is_empty() {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let now_str = chrono::Utc::now().to_rfc3339();
    let placeholders: String = samples_ids.iter().enumerate()
        .map(|(i, _)| format!("?{}", i + 1))
        .collect::<Vec<_>>().join(",");
    let sql = format!(
        "UPDATE labeled_samples SET synced_at = '{}' WHERE id IN ({})",
        now_str, placeholders,
    );
    let params: Vec<&dyn rusqlite::ToSql> = samples_ids.iter().map(|s| s as &dyn rusqlite::ToSql).collect();
    let _ = conn.execute(&sql, params.as_slice());
}

log::info!("[sync] labeled_samples: {} ok, {} errors", samples_ok, samples_err);
```

- [ ] **Step 3: Build**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/services/sync_service.rs
git commit -m "feat(sync): sync labeled_samples to Postgres (unsynced batches of 200)"
```

---

### Task 3: Pull `labeled_samples` from Postgres on sync and trigger retrain

When the sync cycle upserts data from Postgres back to local SQLite (the pull phase), any new `labeled_samples` received from other devices should trigger a model retrain.

**Files:**
- Modify: `src-tauri/src/services/sync_service.rs`
- Modify: `src-tauri/src/lib.rs` (pass `classification_state` to sync loop)

- [ ] **Step 1: Pass `classification_state` to the sync loop**

In `sync_service.rs`, `start_sync_loop` currently takes `app: AppHandle`. It already has access to all `AppState` via `app.state::<AppState>()`. No signature change needed.

- [ ] **Step 2: Add pull of `labeled_samples` from Postgres to local SQLite**

Inside `run_sync_cycle_inline`, after the push block for `labeled_samples`, add a pull block that fetches samples from Postgres that this device hasn't seen:

```rust
// ── Pull labeled_samples from Postgres (other devices) ───────────────────────
let local_count_before: i64 = {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT COUNT(*) FROM labeled_samples", [], |r| r.get(0))
        .unwrap_or(0)
};

let device_id_str = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());
let remote_samples = match pg_client.query(
    "SELECT id, feature_text, process_name, window_title, client_id, project_id, \
            task_id, source, device_id, created_at, modified_at \
     FROM labeled_samples WHERE device_id <> $1",
    &[&device_id_str],
).await {
    Ok(rows) => rows,
    Err(e) => {
        log::warn!("[sync] labeled_samples pull from Postgres failed: {e}");
        vec![]
    }
};

for row in &remote_samples {
    let id: &str = row.get(0);
    let feature_text: &str = row.get(1);
    let process_name: &str = row.get(2);
    let window_title: &str = row.get(3);
    let client_id: Option<&str> = row.get(4);
    let project_id: Option<&str> = row.get(5);
    let task_id: Option<&str> = row.get(6);
    let source: &str = row.get(7);
    let device_id: &str = row.get(8);
    let created_at: chrono::DateTime<chrono::Utc> = row.get(9);
    let modified_at: chrono::DateTime<chrono::Utc> = row.get(10);
    let created_at_str = created_at.to_rfc3339();
    let modified_at_str = modified_at.to_rfc3339();
    let now = chrono::Utc::now().to_rfc3339();

    if let Ok(conn) = db.lock() {
        if let Err(e) = conn.execute(
            // Last-write-wins: only update if the remote sample is newer
            "INSERT INTO labeled_samples \
             (id,feature_text,process_name,window_title,client_id,project_id,task_id, \
              source,device_id,created_at,modified_at,synced_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)
             ON CONFLICT(id) DO UPDATE SET
                 feature_text = CASE WHEN excluded.modified_at > labeled_samples.modified_at
                                THEN excluded.feature_text ELSE labeled_samples.feature_text END,
                 client_id = CASE WHEN excluded.modified_at > labeled_samples.modified_at
                             THEN excluded.client_id ELSE labeled_samples.client_id END,
                 project_id = CASE WHEN excluded.modified_at > labeled_samples.modified_at
                              THEN excluded.project_id ELSE labeled_samples.project_id END,
                 task_id = CASE WHEN excluded.modified_at > labeled_samples.modified_at
                           THEN excluded.task_id ELSE labeled_samples.task_id END,
                 source = CASE WHEN excluded.modified_at > labeled_samples.modified_at
                          THEN excluded.source ELSE labeled_samples.source END,
                 modified_at = CASE WHEN excluded.modified_at > labeled_samples.modified_at
                               THEN excluded.modified_at ELSE labeled_samples.modified_at END",
            rusqlite::params![
                id, feature_text, process_name, window_title,
                client_id, project_id, task_id,
                source, device_id, created_at_str, modified_at_str, now,
            ],
        ) {
            log::warn!("[sync] Failed to upsert pulled labeled_sample {id}: {e}");
        }
    }
}

let local_count_after: i64 = {
    let conn = db.lock().map_err(|e| e.to_string())?;
    conn.query_row("SELECT COUNT(*) FROM labeled_samples", [], |r| r.get(0))
        .unwrap_or(0)
};

let new_samples_received = local_count_after - local_count_before;
```

- [ ] **Step 3: Trigger retrain if new samples were received**

After the pull block, check if retraining is warranted and trigger it:

```rust
if new_samples_received > 0 {
    log::info!("[sync] Received {} new labeled samples from other devices", new_samples_received);
    let app_state = app.state::<AppState>();
    if let Ok(conn) = app_state.db.lock() {
        if let Some(model) = crate::services::classification::trainer::retrain(&conn) {
            drop(conn);
            if let Ok(mut cs) = app_state.classification_state.lock() {
                cs.model = Some(model);
                log::info!("[sync] Classification model retrained from {} new samples", new_samples_received);
            }
        }
    }
}
```

> **Implementation note:** `run_sync_cycle_inline` must accept `app: &AppHandle` (in addition to the existing `db` and `uri` parameters) so it can access `classification_state`. Update the function signature and all call sites in `start_sync_loop` accordingly.

- [ ] **Step 4: Build**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 5: Run all tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features test 2>&1
dotnet test src/Tracey.Tests
```

Expected: all tests pass.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/services/sync_service.rs
git commit -m "feat(sync): pull labeled_samples from Postgres and trigger retrain on new samples"
```

---

### Task 4: Verify end-to-end sync with a manual test

This task has no automated test because it requires a live Postgres connection. Follow these steps to verify manually.

- [ ] **Step 1: Configure external DB sync in Settings**

Launch the app, navigate to Settings, enter a valid Postgres connection URI, save, and verify sync status shows "Connected."

- [ ] **Step 2: Trigger a labeled sample on device A**

On device A: switch to a window, confirm the classification toast (or use the Settings to submit a correction). The `labeled_samples` table should have one row with `synced_at IS NULL`.

- [ ] **Step 3: Trigger sync on device A**

```
Settings → Sync → Sync now (or wait for the 30-second auto-sync)
```

Verify in Postgres:

```sql
SELECT id, process_name, device_id, synced_at FROM labeled_samples;
```

Expected: the row is present in Postgres with the device A `device_id`.

- [ ] **Step 4: Verify device B receives the sample**

On device B, trigger a sync. After sync, query local SQLite:

```sql
SELECT id, process_name, synced_at FROM labeled_samples;
```

Expected: the row from device A is present locally on device B.

- [ ] **Step 5: Commit any fixups found during manual testing**

```powershell
git add -A
git commit -m "fix(sync): fixups from end-to-end sync verification"
```
