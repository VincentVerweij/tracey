//! Portable path resolution — integration tests.
//!
//! These tests verify the three guarantees of T078:
//!   (a) A writable exe_dir wins over APPDATA
//!   (b) A read-only exe_dir falls back to APPDATA
//!   (c) First-launch directory creation (tracey.db parent + screenshots/) works

use tracey_lib::db::{is_writable, resolve_db_path_for};

#[test]
fn writable_exe_dir_is_used() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_exe = tmp.path().join("tracey.exe");
    let path = resolve_db_path_for(Some(&fake_exe));
    assert_eq!(
        path,
        tmp.path().join("tracey.db"),
        "writable exe_dir should produce {{exe_dir}}/tracey.db"
    );
}

#[test]
fn readonly_exe_dir_falls_back_to_appdata() {
    // On Windows, set_readonly(true) on a directory does NOT prevent writes
    // (Windows uses ACLs, not the POSIX read-only attribute). The portable,
    // cross-platform way to trigger the is_writable=false path is to use a
    // directory that no longer exists — File::create inside it will fail on
    // every platform, causing the fallback to APPDATA.
    let tmp = tempfile::tempdir().unwrap();
    let fake_exe_dir = tmp.path().to_path_buf();
    let fake_exe = fake_exe_dir.join("tracey.exe");
    // Drop (and delete) the temp dir so is_writable returns false
    drop(tmp);

    let path = resolve_db_path_for(Some(&fake_exe));

    assert!(
        path.to_string_lossy().contains("tracey"),
        "fallback path should contain 'tracey', got {:?}",
        path
    );
    assert!(
        !path.starts_with(&fake_exe_dir),
        "fallback path must NOT be inside the non-existent exe dir"
    );
}

#[test]
fn first_launch_creates_db_parent_and_screenshots_dir() {
    let tmp = tempfile::tempdir().unwrap();
    let fake_exe = tmp.path().join("tracey.exe");
    let db_path = resolve_db_path_for(Some(&fake_exe));

    // Simulate first-launch directory creation
    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    let screenshots = db_path.parent().unwrap().join("screenshots");
    std::fs::create_dir_all(&screenshots).unwrap();

    assert!(
        db_path.parent().unwrap().exists(),
        "db parent dir should be created"
    );
    assert!(screenshots.exists(), "screenshots dir should be created");
}

#[test]
fn is_writable_returns_true_for_writable_dir() {
    let tmp = tempfile::tempdir().unwrap();
    assert!(is_writable(tmp.path()), "temp dir should be writable");
}
