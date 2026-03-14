# IPC Command Contracts: Tauri ↔ Blazor

**Phase**: 1 — Design & Contracts  
**Branch**: `001-window-activity-tracker`  
**Date**: 2026-03-14

These are the Tauri IPC commands exposed from the Rust backend to the Blazor WebView2 frontend. Commands are versioned (v1). All inputs are validated in Rust; all outputs are typed.

The Blazor frontend calls these via `IJSRuntime.InvokeAsync<T>("window.__TAURI__.invoke", commandName, args)` wrapped in a C# `TauriIpcService`.

---

## Conventions

- All datetimes are **ISO 8601 UTC strings** (`2026-03-14T09:00:00Z`).
- All IDs are **ULID strings** (26 chars, lexicographically sortable UUID variant).
- Errors are returned as `{ "error": "message" }` JSON.
- Rust command names use `snake_case`; C# wrapper methods use `PascalCase`.
- All commands are `async` in Rust (invoked with `invoke` not `invoke_without_handler`).

---

## Timer & Time Entry Commands

### `timer_start`
Start a new timer. Automatically stops and saves any currently running timer.

**Input**:
```json
{
  "description": "string (required, max 500 chars)",
  "project_id":  "string | null (ULID)",
  "task_id":     "string | null (ULID)",
  "tag_ids":     ["string (ULID)", "..."]
}
```

**Output**:
```json
{
  "id":          "string (ULID)",
  "started_at":  "string (UTC ISO 8601)",
  "stopped_entry": null | { "id": "string", "ended_at": "string" }
}
```

**Errors**: `"invalid_description"`, `"project_not_found"`, `"task_not_found"`, `"tag_not_found"`

---

### `timer_stop`
Stop the currently running timer.

**Input**: _(none)_

**Output**:
```json
{
  "id":        "string (ULID)",
  "ended_at":  "string (UTC ISO 8601)"
}
```

**Errors**: `"no_active_timer"`

---

### `timer_get_active`
Returns the currently running timer, or null if none.

**Input**: _(none)_

**Output**:
```json
{
  "id":          "string | null",
  "description": "string",
  "started_at":  "string",
  "project_id":  "string | null",
  "task_id":     "string | null",
  "tag_ids":     ["string"]
}
```

---

### `time_entry_list`
Paginated list of completed time entries, grouped by date, descending.

**Input**:
```json
{
  "page":      "number (1-based)",
  "page_size": "number (1–200)"
}
```

**Output**:
```json
{
  "entries": [
    {
      "id":          "string",
      "description": "string",
      "started_at":  "string",
      "ended_at":    "string",
      "project_id":  "string | null",
      "project_name": "string | null",
      "client_name": "string | null",
      "task_id":     "string | null",
      "task_name":   "string | null",
      "tag_ids":     ["string"],
      "tag_names":   ["string"],
      "is_break":    "boolean"
    }
  ],
  "total_count": "number",
  "has_more":    "boolean"
}
```

---

### `time_entry_create_manual`
Manually create a completed time entry (no running timer created).

**Input**:
```json
{
  "description": "string",
  "started_at":  "string (UTC ISO 8601)",
  "ended_at":    "string (UTC ISO 8601)",
  "project_id":  "string | null",
  "task_id":     "string | null",
  "tag_ids":     ["string"]
}
```

**Output**:
```json
{ "id": "string" }
```

**Errors**: `"invalid_time_range"`, `"overlap_detected"` (warning; requires `force: true` to override)

---

### `time_entry_continue`
Start a new timer copying description, project, task, and tags from a past entry.

**Input**:
```json
{ "source_entry_id": "string (ULID)" }
```

**Output**: Same as `timer_start`.

---

### `time_entry_autocomplete`
Return autocomplete suggestions for the description field (historical entries, fuzzy-matched).

**Input**:
```json
{
  "query": "string (partial description)",
  "limit": "number (default 10, max 20)"
}
```

**Output**:
```json
{
  "suggestions": [
    {
      "description": "string",
      "project_id":  "string | null",
      "project_name":"string | null",
      "task_id":     "string | null",
      "task_name":   "string | null",
      "tag_ids":     ["string"]
    }
  ]
}
```

---

## Client / Project / Task Commands

### `client_list`
**Input**:
```json
{ "include_archived": "boolean (default false)" }
```

**Output**:
```json
{
  "clients": [
    {
      "id": "string", "name": "string", "color": "string",
      "logo_path": "string | null", "is_archived": "boolean"
    }
  ]
}
```

---

### `client_create`
**Input**:
```json
{ "name": "string", "color": "string (#RRGGBB)", "logo_path": "string | null" }
```
**Output**: `{ "id": "string" }`  
**Errors**: `"name_conflict"`, `"invalid_color"`, `"logo_not_found"`

---

### `client_update`
**Input**:
```json
{ "id": "string", "name": "string", "color": "string", "logo_path": "string | null" }
```
**Output**: `{ "modified_at": "string" }`

---

### `client_archive` / `client_unarchive`
**Input**: `{ "id": "string" }`  
**Output**: `{ "modified_at": "string" }`

---

### `client_delete`
**Input**: `{ "id": "string" }`  
**Output**: `{ "deleted_projects": "number", "deleted_tasks": "number", "orphaned_entries": "number" }`  
**Errors**: `"not_found"`

---

### `project_list`
**Input**:
```json
{ "client_id": "string | null", "include_archived": "boolean (default false)" }
```
**Output**: Array of project objects with `id`, `client_id`, `name`, `is_archived`.

---

### `project_create`, `project_update`, `project_archive`, `project_unarchive`, `project_delete`
Follow same pattern as client commands; scoped to `project_id`.

---

### `task_list`
**Input**: `{ "project_id": "string" }`  
**Output**: Array of task objects with `id`, `project_id`, `name`.

---

### `task_create`, `task_update`, `task_delete`
Follow same pattern; scoped to `task_id`.

---

## Tag Commands

### `tag_list`
**Input**: _(none)_  
**Output**: Array of `{ id, name, created_at }`.

---

### `tag_create`
**Input**: `{ "name": "string" }`  
**Output**: `{ "id": "string" }`  
**Errors**: `"name_conflict"`

---

### `tag_delete`
**Input**: `{ "id": "string" }`  
**Output**: `{ "affected_entries": "number" }`  
(Warns before confirming via UI; the confirmation is shown by Blazor before calling this command.)

---

## Quick-Entry Fuzzy Match Commands

### `fuzzy_match_projects`
**Input**:
```json
{ "query": "string", "limit": "number (default 8)" }
```
**Output**:
```json
{
  "matches": [
    { "project_id": "string", "project_name": "string", "client_id": "string", "client_name": "string", "score": "number" }
  ]
}
```

---

### `fuzzy_match_tasks`
**Input**:
```json
{ "project_id": "string", "query": "string", "limit": "number (default 8)" }
```
**Output**:
```json
{
  "matches": [
    { "task_id": "string", "task_name": "string", "score": "number" }
  ]
}
```

---

## Screenshot Commands

### `screenshot_list`
**Input**:
```json
{ "from": "string (UTC ISO 8601)", "to": "string (UTC ISO 8601)" }
```
**Output**: Array of `{ id, file_path, captured_at, window_title, process_name, trigger }`.

---

### `screenshot_delete_expired`
Manually trigger retention cleanup.  
**Input**: _(none)_  
**Output**: `{ "deleted_count": "number" }`

---

## Idle Detection Commands

### `idle_get_status`
**Input**: _(none)_  
**Output**:
```json
{
  "is_idle": "boolean",
  "idle_seconds": "number",
  "idle_since": "string | null (UTC ISO 8601)"
}
```

### `idle_resolve`
User has selected an option from the idle-return modal.  
**Input**:
```json
{
  "resolution": "break | meeting | specify | keep",
  "idle_started_at": "string (UTC ISO 8601)",
  "idle_ended_at":   "string (UTC ISO 8601)",
  "entry_details": null | {
    "description": "string",
    "project_id":  "string | null",
    "task_id":     "string | null",
    "tag_ids":     ["string"]
  }
}
```
**Output**:
```json
{ "created_entry_id": "string | null" }
```

---

## Sync Commands

### `sync_get_status`
**Input**: _(none)_  
**Output**:
```json
{
  "enabled": "boolean",
  "connected": "boolean",
  "pending_queue_size": "number",
  "last_sync_at": "string | null",
  "last_error": "string | null"
}
```

### `sync_configure`
**Input**:
```json
{ "connection_uri": "string", "enabled": "boolean" }
```
**Output**: `{ "connected": "boolean" }`  
**Notes**: Connection URI is stored in OS keychain; this command never returns the URI back.

### `sync_trigger`
Force an immediate sync attempt.  
**Input**: _(none)_  
**Output**: `{ "synced_records": "number", "errors": "number" }`

---

## Observability Commands

### `health_get`
(Local only; not exposed to external network)  
**Input**: _(none)_  
**Output**:
```json
{
  "running": "boolean",
  "last_write_at": "string | null",
  "events_per_sec": "number",
  "memory_mb": "number",
  "active_errors": ["string"],
  "pending_sync_count": "number"
}
```

---

## Tauri Events (Backend → Frontend push)

The Rust layer emits these events to all WebView2 windows; Blazor subscribes via JS interop.

| Event Name | Payload | When Emitted |
|------------|---------|--------------|
| `tracey://timer-tick` | `{ elapsed_seconds: number }` | Every second while a timer is running |
| `tracey://idle-detected` | `{ idle_since: string, had_active_timer: boolean }` | When idle threshold crossed |
| `tracey://idle-resolved` | `{ created_entry_id: string \| null }` | After idle resolution saved |
| `tracey://screenshot-captured` | `{ id: string, captured_at: string }` | After each screenshot saved |
| `tracey://sync-status-changed` | `{ connected: boolean, pending: number }` | When sync state changes |
| `tracey://notification-sent` | `{ channel_id: string, message: string }` | When a notification fires |
| `tracey://error` | `{ component: string, message: string }` | On recoverable errors requiring user notice |
