use rusqlite::{Connection, Result as SqlResult};

/// All migrations embedded at compile time.
/// Lexicographic order = application order.
const MIGRATIONS: &[(&str, &str)] = &[
    (
        "001_initial_schema",
        include_str!("migrations/001_initial_schema.sql"),
    ),
    (
        "002_add_schema_migrations_table",
        include_str!("migrations/002_add_schema_migrations_table.sql"),
    ),
    (
        "003_sync_queue_additions",
        include_str!("migrations/003_sync_queue_additions.sql"),
    ),
    (
        "004_add_device_id_columns",
        include_str!("migrations/004_add_device_id_columns.sql"),
    ),
    (
        "005_add_ocr_text_to_screenshots",
        include_str!("migrations/005_add_ocr_text_to_screenshots.sql"),
    ),
    (
        "006_classification_engine",
        include_str!("migrations/006_classification_engine.sql"),
    ),
    (
        "007_auto_classification_loop",
        include_str!("migrations/007_auto_classification_loop.sql"),
    ),
];

pub fn run(conn: &Connection) -> SqlResult<()> {
    // Bootstrap the tracking table before we query it.
    // Migration 002 also creates it with IF NOT EXISTS, so the two are idempotent.
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    TEXT PRIMARY KEY NOT NULL,
            applied_at TEXT NOT NULL
        );",
    )?;

    for (version, sql) in MIGRATIONS {
        let already_applied: bool = conn.query_row(
            "SELECT COUNT(*) > 0 FROM schema_migrations WHERE version = ?1",
            [version],
            |row| row.get(0),
        )?;

        if !already_applied {
            log::info!("Applying migration: {}", version);

            // Run in a transaction — rollback on any failure
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
                rusqlite::params![version, chrono::Utc::now().to_rfc3339()],
            )?;
            tx.commit()?;

            log::info!("Applied migration: {}", version);
        }
    }

    Ok(())
}
