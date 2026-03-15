use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Client {
    pub id: String,
    pub name: String,
    pub color: String,
    pub logo_path: Option<String>, // local FS path — NEVER synced to external DB
    pub is_archived: bool,
    pub created_at: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub is_archived: bool,
    pub created_at: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub created_at: String,
    pub modified_at: String,
}

// SQL: no color column — tags table has id, name, created_at, modified_at only
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tag {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntry {
    pub id: String,
    pub description: String,
    pub started_at: String,
    pub ended_at: Option<String>,  // NULL = timer currently running
    pub project_id: Option<String>, // SET NULL on project delete
    pub task_id: Option<String>,    // SET NULL on task delete
    pub is_break: bool,
    pub device_id: String,
    pub created_at: String,
    pub modified_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeEntryTag {
    pub time_entry_id: String,
    pub tag_id: String,
}

// SQL: window_activity_records has window_handle + device_id, no time_entry_id or process_path
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowActivityRecord {
    pub id: String,
    pub process_name: String,
    pub window_title: String,
    pub window_handle: String,
    pub recorded_at: String,
    pub device_id: String,
    pub synced_at: Option<String>, // NULL = pending sync
}

// SQL: screenshots has trigger + device_id, no width/height
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screenshot {
    pub id: String,
    pub file_path: String,
    pub captured_at: String,
    pub window_title: String,
    pub process_name: String,
    pub trigger: String, // "interval" | "window_change"
    pub device_id: String,
}

// SQL: id is INTEGER singleton (always 1), no modified_at column
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPreferences {
    pub id: i64, // INTEGER PRIMARY KEY DEFAULT 1; CHECK (id = 1)
    pub local_timezone: String,
    pub inactivity_timeout_seconds: i64,
    pub screenshot_interval_seconds: i64,
    pub screenshot_retention_days: i64,
    pub screenshot_storage_path: Option<String>,
    pub timer_notification_threshold_hours: f64,
    pub page_size: i64,
    pub external_db_uri_stored: bool, // flag only; URI lives in OS keychain
    pub external_db_enabled: bool,
    pub notification_channels_json: Option<String>, // JSON array
    pub process_deny_list_json: String,             // JSON array of strings
}

// SQL: id is INTEGER AUTOINCREMENT (not TEXT)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncQueueEntry {
    pub id: i64,
    pub table_name: String,
    pub record_id: String,
    pub operation: String, // "upsert" | "delete"
    pub queued_at: String,
    pub attempts: i64,     // retry counter; incremented on each failed sync attempt
}
