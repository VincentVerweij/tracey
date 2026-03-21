//! Performance benchmarks for Tracey's database layer
//!
//! Covers:
//!   - time_entry_list query at scale (100 pre-inserted rows, paged)
//!   - window_activity_records insert throughput
//!
//! Run with:
//!   cargo bench --bench db_benchmarks
//!
//! Performance budgets (from decisions.md):
//!   - Queries < 500 ms p95 at ≤ 1M events
//!   - Background tracing < 2% CPU over any 10-second window
//!
//! NOTE: These benchmarks use an in-memory SQLite DB — no disk I/O, so numbers
//! represent pure query/insert overhead, not worst-case production latency.
//! Production latency (WAL-mode on-disk) will be higher but still within budget
//! given that SQLite WAL read performance is close to in-memory for sequential reads.

use criterion::{criterion_group, criterion_main, Criterion};
use rusqlite::Connection;

/// Create a fresh in-memory SQLite DB with the minimal schema needed for benchmarks.
/// WAL mode and foreign keys match production `PRAGMA` settings (decisions.md).
fn setup_test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;

        CREATE TABLE IF NOT EXISTS time_entries (
            id           TEXT PRIMARY KEY NOT NULL,
            description  TEXT NOT NULL DEFAULT '',
            started_at   TEXT NOT NULL,
            ended_at     TEXT,
            project_id   TEXT,
            task_id      TEXT,
            is_break     INTEGER NOT NULL DEFAULT 0,
            device_id    TEXT NOT NULL DEFAULT 'bench',
            created_at   TEXT NOT NULL,
            modified_at  TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS window_activity_records (
            id            TEXT PRIMARY KEY NOT NULL,
            process_name  TEXT NOT NULL,
            window_title  TEXT NOT NULL,
            window_handle TEXT NOT NULL,
            recorded_at   TEXT NOT NULL,
            device_id     TEXT NOT NULL,
            synced_at     TEXT
        );
        ",
    )
    .unwrap();
    conn
}

// ---------------------------------------------------------------------------
// time_entry_list benchmarks
// ---------------------------------------------------------------------------

/// Baseline: 100 pre-inserted rows, paginated SELECT (LIMIT 25).
/// Mirrors the `time_entry_list` IPC command query pattern.
fn bench_time_entry_list_100(c: &mut Criterion) {
    let conn = setup_test_db();

    for i in 0..100u32 {
        // Use a fixed timestamp — avoids chrono overhead inside the hot loop
        let ts = format!("2026-01-{:02}T10:00:00Z", (i % 28) + 1);
        conn.execute(
            "INSERT INTO time_entries
                (id, description, started_at, ended_at, device_id, created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, 'bench', ?5, ?6)",
            rusqlite::params![
                format!("id-{i}"),
                format!("Entry {i}"),
                &ts,
                // Every other entry is completed (has ended_at); matches realistic data
                if i % 2 == 0 {
                    Some(ts.clone())
                } else {
                    None
                },
                &ts,
                &ts,
            ],
        )
        .unwrap();
    }

    c.bench_function("time_entry_list_100_rows_page1", |b| {
        b.iter(|| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, description, started_at, ended_at,
                            project_id, task_id, is_break, device_id
                     FROM   time_entries
                     WHERE  ended_at IS NOT NULL
                     ORDER  BY started_at DESC
                     LIMIT  25 OFFSET 0",
                )
                .unwrap();
            let rows: Vec<_> = stmt
                .query_map([], |_row| Ok(()))
                .unwrap()
                .collect();
            criterion::black_box(rows)
        })
    });
}

/// Scale test: 1 000 pre-inserted rows, same paginated SELECT.
/// This is the primary budget check — must complete < 500 ms p95.
fn bench_time_entry_list_1000(c: &mut Criterion) {
    let conn = setup_test_db();

    for i in 0..1000u32 {
        let ts = format!("2026-01-01T{:02}:{:02}:00Z", (i / 60) % 24, i % 60);
        conn.execute(
            "INSERT INTO time_entries
                (id, description, started_at, ended_at, device_id, created_at, modified_at)
             VALUES (?1, ?2, ?3, ?4, 'bench', ?5, ?6)",
            rusqlite::params![
                format!("id-{i}"),
                format!("Entry {i}"),
                &ts,
                Some(ts.clone()),
                &ts,
                &ts,
            ],
        )
        .unwrap();
    }

    c.bench_function("time_entry_list_1000_rows_page1", |b| {
        b.iter(|| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, description, started_at, ended_at,
                            project_id, task_id, is_break, device_id
                     FROM   time_entries
                     WHERE  ended_at IS NOT NULL
                     ORDER  BY started_at DESC
                     LIMIT  25 OFFSET 0",
                )
                .unwrap();
            let rows: Vec<_> = stmt
                .query_map([], |_row| Ok(()))
                .unwrap()
                .collect();
            criterion::black_box(rows)
        })
    });
}

// ---------------------------------------------------------------------------
// window_activity_records insert throughput
// ---------------------------------------------------------------------------

/// INSERT throughput for window activity records.
/// Each iteration inserts one record; criterion measures iterations/second.
/// Budget: window polling runs every 1 s — insert must be negligible overhead.
fn bench_window_activity_insert(c: &mut Criterion) {
    let conn = setup_test_db();
    let mut counter = 0u64;

    c.bench_function("window_activity_insert", |b| {
        b.iter(|| {
            counter += 1;
            conn.execute(
                "INSERT INTO window_activity_records
                    (id, process_name, window_title, window_handle, recorded_at, device_id)
                 VALUES (?1, 'chrome.exe', 'GitHub - Chrome', 'chrome.exe:GitHub - Chrome',
                         '2026-01-01T10:00:00Z', 'bench')",
                rusqlite::params![format!("rec-{counter}")],
            )
            .unwrap();
        })
    });
}

/// Batch SELECT of un-synced window activity records (the flush-to-external-DB query pattern).
/// 500 rows pre-inserted; measures the SELECT before the 30-second flush cycle.
fn bench_window_activity_unsynced_select(c: &mut Criterion) {
    let conn = setup_test_db();

    for i in 0..500u32 {
        conn.execute(
            "INSERT INTO window_activity_records
                (id, process_name, window_title, window_handle, recorded_at, device_id)
             VALUES (?1, 'chrome.exe', 'GitHub - Chrome', 'chrome.exe:GitHub - Chrome',
                     '2026-01-01T10:00:00Z', 'bench')",
            rusqlite::params![format!("rec-{i}")],
        )
        .unwrap();
    }

    c.bench_function("window_activity_unsynced_select_500", |b| {
        b.iter(|| {
            let mut stmt = conn
                .prepare(
                    "SELECT id, process_name, window_title, window_handle, recorded_at, device_id
                     FROM   window_activity_records
                     WHERE  synced_at IS NULL
                     ORDER  BY recorded_at ASC
                     LIMIT  200",
                )
                .unwrap();
            let rows: Vec<_> = stmt
                .query_map([], |_row| Ok(()))
                .unwrap()
                .collect();
            criterion::black_box(rows)
        })
    });
}

criterion_group!(
    benches,
    bench_time_entry_list_100,
    bench_time_entry_list_1000,
    bench_window_activity_insert,
    bench_window_activity_unsynced_select
);
criterion_main!(benches);
