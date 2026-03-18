using Microsoft.JSInterop;
using System.Text.Json.Serialization;

namespace Tracey.App.Services;

/// <summary>
/// Typed C# wrappers for all Tauri IPC commands.
/// All calls go via window.__TAURI_INTERNALS__.invoke (Tauri 2.0 JS bridge).
/// All methods are async — IPC is never synchronous.
/// Source of truth: specs/001-window-activity-tracker/contracts/ipc-commands.md
/// </summary>
public class TauriIpcService
{
    private readonly IJSRuntime _js;

    public TauriIpcService(IJSRuntime js) => _js = js;

    // ── Health ────────────────────────────────────────────────────────────

    public Task<HealthResponse> HealthGetAsync() =>
        Invoke<HealthResponse>("health_get");

    // ── Preferences ───────────────────────────────────────────────────────

    public Task<UserPreferences> PreferencesGetAsync() =>
        Invoke<UserPreferences>("preferences_get");

    public Task<UserPreferences> PreferencesUpdateAsync(PreferencesUpdateRequest update) =>
        Invoke<UserPreferences>("preferences_update", new { update });

    // ── Timer ─────────────────────────────────────────────────────────────

    public Task<TimerStartResponse> TimerStartAsync(TimerStartRequest request) =>
        Invoke<TimerStartResponse>("timer_start", new { request });

    public Task<TimerStopResponse> TimerStopAsync() =>
        Invoke<TimerStopResponse>("timer_stop");

    public Task<ActiveTimerResponse> TimerGetActiveAsync() =>
        Invoke<ActiveTimerResponse>("timer_get_active");

    // ── Time Entries ──────────────────────────────────────────────────────

    public Task<TimeEntryListResponse> TimeEntryListAsync(TimeEntryListRequest request) =>
        Invoke<TimeEntryListResponse>("time_entry_list", new { request });

    public Task<IdResponse> TimeEntryCreateManualAsync(TimeEntryCreateManualRequest request) =>
        Invoke<IdResponse>("time_entry_create_manual", new { request });

    public Task<TimerStartResponse> TimeEntryContinueAsync(string sourceEntryId) =>
        Invoke<TimerStartResponse>("time_entry_continue", new { request = new { source_entry_id = sourceEntryId } });

    public Task<ModifiedAtResponse> TimeEntryUpdateAsync(TimeEntryUpdateRequest request) =>
        Invoke<ModifiedAtResponse>("time_entry_update", new { request });

    public Task TimeEntryDeleteAsync(string id) =>
        Invoke<object>("time_entry_delete", new { id });

    public Task<TimeEntryAutocompleteResponse> TimeEntryAutocompleteAsync(TimeEntryAutocompleteRequest request) =>
        Invoke<TimeEntryAutocompleteResponse>("time_entry_autocomplete", new { request });

    // ── Clients ───────────────────────────────────────────────────────────

    public Task<ClientListResponse> ClientListAsync(bool includeArchived = false) =>
        Invoke<ClientListResponse>("client_list", new { includeArchived });

    public Task<IdResponse> ClientCreateAsync(ClientCreateRequest request) =>
        Invoke<IdResponse>("client_create", new { request });

    public Task<ModifiedAtResponse> ClientUpdateAsync(ClientUpdateRequest request) =>
        Invoke<ModifiedAtResponse>("client_update", new { request });

    public Task<ModifiedAtResponse> ClientArchiveAsync(string id) =>
        Invoke<ModifiedAtResponse>("client_archive", new { id });

    public Task<ModifiedAtResponse> ClientUnarchiveAsync(string id) =>
        Invoke<ModifiedAtResponse>("client_unarchive", new { id });

    public Task<ClientDeleteResponse> ClientDeleteAsync(string id) =>
        Invoke<ClientDeleteResponse>("client_delete", new { id });

    // ── Projects ──────────────────────────────────────────────────────────

    public Task<ProjectListResponse> ProjectListAsync(string? clientId = null, bool includeArchived = false) =>
        Invoke<ProjectListResponse>("project_list", new { clientId, includeArchived });

    public Task<IdResponse> ProjectCreateAsync(ProjectCreateRequest request) =>
        Invoke<IdResponse>("project_create", new { request });

    public Task<ModifiedAtResponse> ProjectUpdateAsync(ProjectUpdateRequest request) =>
        Invoke<ModifiedAtResponse>("project_update", new { request });

    public Task<ModifiedAtResponse> ProjectArchiveAsync(string id) =>
        Invoke<ModifiedAtResponse>("project_archive", new { id });

    public Task<ModifiedAtResponse> ProjectUnarchiveAsync(string id) =>
        Invoke<ModifiedAtResponse>("project_unarchive", new { id });

    public Task<ProjectDeleteResponse> ProjectDeleteAsync(string id) =>
        Invoke<ProjectDeleteResponse>("project_delete", new { id });

    // ── Tasks ─────────────────────────────────────────────────────────────

    public Task<TaskListResponse> TaskListAsync(string projectId) =>
        Invoke<TaskListResponse>("task_list", new { projectId });

    public Task<IdResponse> TaskCreateAsync(TaskCreateRequest request) =>
        Invoke<IdResponse>("task_create", new { request });

    public Task<ModifiedAtResponse> TaskUpdateAsync(TaskUpdateRequest request) =>
        Invoke<ModifiedAtResponse>("task_update", new { request });

    public Task<AffectedEntriesResponse> TaskDeleteAsync(string id) =>
        Invoke<AffectedEntriesResponse>("task_delete", new { id });

    // ── Tags ──────────────────────────────────────────────────────────────

    public Task<TagListResponse> TagListAsync() =>
        Invoke<TagListResponse>("tag_list");

    public Task<IdResponse> TagCreateAsync(string name) =>
        Invoke<IdResponse>("tag_create", new { name });

    public Task<AffectedEntriesResponse> TagDeleteAsync(string id) =>
        Invoke<AffectedEntriesResponse>("tag_delete", new { id });

    /// Partial update — sends only id + tag_ids; all other fields preserved by Rust.
    /// Safe to call on a running entry (ended_at = NULL is preserved).
    public Task<ModifiedAtResponse> TimeEntryUpdateTagsAsync(string entryId, string[] tagIds) =>
        Invoke<ModifiedAtResponse>("time_entry_update", new { request = new { id = entryId, tag_ids = tagIds } });

    // ── Fuzzy Match ───────────────────────────────────────────────────────

    public Task<FuzzyMatchProjectsResponse> FuzzyMatchProjectsAsync(string query, int limit = 8) =>
        Invoke<FuzzyMatchProjectsResponse>("fuzzy_match_projects", new { query, limit });

    public Task<FuzzyMatchTasksResponse> FuzzyMatchTasksAsync(string projectId, string query, int limit = 8) =>
        Invoke<FuzzyMatchTasksResponse>("fuzzy_match_tasks", new { projectId, query, limit });

    // ── Screenshots ───────────────────────────────────────────────────────

    public Task<ScreenshotItem[]> ScreenshotListAsync(string from, string to) =>
        Invoke<ScreenshotItem[]>("screenshot_list", new { request = new { from, to } });

    public Task<DeletedCountResponse> ScreenshotDeleteExpiredAsync() =>
        Invoke<DeletedCountResponse>("screenshot_delete_expired");

    // ── Idle Detection ────────────────────────────────────────────────────

    public Task<IdleStatusResponse> IdleGetStatusAsync() =>
        Invoke<IdleStatusResponse>("idle_get_status");

    public Task<IdleResolveResponse> IdleResolveAsync(IdleResolveRequest request) =>
        Invoke<IdleResolveResponse>("idle_resolve", new { request });

    // ── Sync ──────────────────────────────────────────────────────────────

    public Task<SyncStatusResponse> SyncGetStatusAsync() =>
        Invoke<SyncStatusResponse>("sync_get_status");

    public Task<SyncConfigureResponse> SyncConfigureAsync(SyncConfigureRequest request) =>
        Invoke<SyncConfigureResponse>("sync_configure", request);

    public Task<SyncTriggerResponse> SyncTriggerAsync() =>
        Invoke<SyncTriggerResponse>("sync_trigger");

    // ── Private helper ────────────────────────────────────────────────────

    private async Task<T> Invoke<T>(string command, object? args = null)
    {
        try
        {
            return await _js.InvokeAsync<T>(
                "window.__TAURI_INTERNALS__.invoke",
                command,
                args);
        }
        catch (JSException ex) when (ex.Message.Contains("__TAURI_INTERNALS__"))
        {
            // Running in a plain browser without the Tauri host — return default gracefully.
            return default!;
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared response shapes
// ─────────────────────────────────────────────────────────────────────────────

public record IdResponse(
    [property: JsonPropertyName("id")] string Id);

public record ModifiedAtResponse(
    [property: JsonPropertyName("modified_at")] string ModifiedAt);

public record AffectedEntriesResponse(
    [property: JsonPropertyName("affected_entries")] long AffectedEntries);

public record DeletedCountResponse(
    [property: JsonPropertyName("deleted_count")] long DeletedCount);

// ─────────────────────────────────────────────────────────────────────────────
// Health
// ─────────────────────────────────────────────────────────────────────────────

public record HealthResponse(
    [property: JsonPropertyName("running")] bool Running,
    [property: JsonPropertyName("last_write_at")] string? LastWriteAt,
    [property: JsonPropertyName("events_per_sec")] double EventsPerSec,
    [property: JsonPropertyName("memory_mb")] double MemoryMb,
    [property: JsonPropertyName("active_errors")] string[] ActiveErrors,
    [property: JsonPropertyName("pending_sync_count")] long PendingSyncCount);

// ─────────────────────────────────────────────────────────────────────────────
// Preferences
// ─────────────────────────────────────────────────────────────────────────────

public record UserPreferences(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("inactivity_timeout_seconds")] long InactivityTimeoutSeconds,
    [property: JsonPropertyName("screenshot_interval_seconds")] long ScreenshotIntervalSeconds,
    [property: JsonPropertyName("screenshot_retention_days")] long ScreenshotRetentionDays,
    [property: JsonPropertyName("screenshot_storage_path")] string? ScreenshotStoragePath,
    [property: JsonPropertyName("local_timezone")] string Timezone,
    [property: JsonPropertyName("page_size")] long EntriesPerPage,
    [property: JsonPropertyName("process_deny_list_json")] string ProcessDenyListJson,
    [property: JsonPropertyName("external_db_enabled")] bool ExternalDbEnabled,
    [property: JsonPropertyName("timer_notification_threshold_hours")] double TimerNotificationThresholdHours,
    [property: JsonPropertyName("modified_at")] string ModifiedAt);

public record PreferencesUpdateRequest(
    [property: JsonPropertyName("inactivity_timeout_seconds")] long? InactivityTimeoutSeconds = null,
    [property: JsonPropertyName("screenshot_interval_seconds")] long? ScreenshotIntervalSeconds = null,
    [property: JsonPropertyName("screenshot_retention_days")] long? ScreenshotRetentionDays = null,
    [property: JsonPropertyName("screenshot_storage_path")] string? ScreenshotStoragePath = null,
    [property: JsonPropertyName("local_timezone")] string? Timezone = null,
    [property: JsonPropertyName("page_size")] long? EntriesPerPage = null,
    [property: JsonPropertyName("process_deny_list_json")] string? ProcessDenyListJson = null,
    [property: JsonPropertyName("external_db_enabled")] bool? ExternalDbEnabled = null,
    [property: JsonPropertyName("timer_notification_threshold_hours")] double? TimerNotificationThresholdHours = null);

// ─────────────────────────────────────────────────────────────────────────────
// Timer
// ─────────────────────────────────────────────────────────────────────────────

public record TimerStartRequest(
    [property: JsonPropertyName("description")] string Description,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("tag_ids")] string[] TagIds);

public record TimerStartResponse(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("started_at")] string StartedAt,
    [property: JsonPropertyName("stopped_entry")] StoppedEntryInfo? StoppedEntry);

public record StoppedEntryInfo(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("ended_at")] string EndedAt);

public record TimerStopResponse(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("ended_at")] string EndedAt);

public record ActiveTimerResponse(
    [property: JsonPropertyName("id")] string? Id,
    [property: JsonPropertyName("description")] string Description,
    [property: JsonPropertyName("started_at")] string StartedAt,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("project_name")] string? ProjectName,
    [property: JsonPropertyName("client_id")] string? ClientId,
    [property: JsonPropertyName("client_name")] string? ClientName,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("task_name")] string? TaskName,
    [property: JsonPropertyName("tag_ids")] string[] TagIds);

// ─────────────────────────────────────────────────────────────────────────────
// Time Entries
// ─────────────────────────────────────────────────────────────────────────────

public record TimeEntryListRequest(
    [property: JsonPropertyName("page")] int Page,
    [property: JsonPropertyName("page_size")] int PageSize);

public record TimeEntryListResponse(
    [property: JsonPropertyName("entries")] TimeEntryItem[] Entries,
    [property: JsonPropertyName("total_count")] long TotalCount,
    [property: JsonPropertyName("has_more")] bool HasMore);

public record TimeEntryItem(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("description")] string Description,
    [property: JsonPropertyName("started_at")] string StartedAt,
    [property: JsonPropertyName("ended_at")] string EndedAt,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("project_name")] string? ProjectName,
    [property: JsonPropertyName("client_name")] string? ClientName,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("task_name")] string? TaskName,
    [property: JsonPropertyName("tag_ids")] string[] TagIds,
    [property: JsonPropertyName("tag_names")] string[] TagNames,
    [property: JsonPropertyName("is_break")] bool IsBreak);

public record TimeEntryCreateManualRequest(
    [property: JsonPropertyName("description")] string Description,
    [property: JsonPropertyName("started_at")] string StartedAt,
    [property: JsonPropertyName("ended_at")] string EndedAt,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("tag_ids")] string[] TagIds);

public record TimeEntryUpdateRequest(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("description")] string Description,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("tag_ids")] string[]? TagIds,
    [property: JsonPropertyName("started_at")] string StartedAt,
    [property: JsonPropertyName("ended_at")] string EndedAt,
    [property: JsonPropertyName("force")] bool Force);

public record TimeEntryAutocompleteRequest(
    [property: JsonPropertyName("query")] string Query,
    [property: JsonPropertyName("limit")] int Limit = 10);

public record TimeEntryAutocompleteResponse(
    [property: JsonPropertyName("suggestions")] AutocompleteSuggestion[] Suggestions);

/// <summary>
/// Autocomplete suggestion. is_orphaned = true when the linked project/task was deleted.
/// Per architectural decision 2026-03-15 (IPC Contract Amendment).
/// </summary>
public record AutocompleteSuggestion(
    [property: JsonPropertyName("description")] string Description,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("project_name")] string? ProjectName,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("task_name")] string? TaskName,
    [property: JsonPropertyName("tag_ids")] string[] TagIds,
    [property: JsonPropertyName("is_orphaned")] bool IsOrphaned);

// ─────────────────────────────────────────────────────────────────────────────
// Clients
// ─────────────────────────────────────────────────────────────────────────────

public record ClientListResponse(
    [property: JsonPropertyName("clients")] ClientItem[] Clients);

public record ClientItem(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("color")] string Color,
    [property: JsonPropertyName("logo_path")] string? LogoPath,
    [property: JsonPropertyName("is_archived")] bool IsArchived);

public record ClientCreateRequest(
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("color")] string Color,
    [property: JsonPropertyName("logo_path")] string? LogoPath);

public record ClientUpdateRequest(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("color")] string Color,
    [property: JsonPropertyName("logo_path")] string? LogoPath);

public record ClientDeleteResponse(
    [property: JsonPropertyName("deleted_projects")] long DeletedProjects,
    [property: JsonPropertyName("deleted_tasks")] long DeletedTasks,
    [property: JsonPropertyName("orphaned_entries")] long OrphanedEntries);

// ─────────────────────────────────────────────────────────────────────────────
// Projects
// ─────────────────────────────────────────────────────────────────────────────

public record ProjectListResponse(
    [property: JsonPropertyName("projects")] ProjectItem[] Projects);

public record ProjectItem(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("client_id")] string? ClientId,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("is_archived")] bool IsArchived);

public record ProjectCreateRequest(
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("client_id")] string? ClientId);

public record ProjectUpdateRequest(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("client_id")] string? ClientId);

public record ProjectDeleteResponse(
    [property: JsonPropertyName("deleted_tasks")] long DeletedTasks,
    [property: JsonPropertyName("orphaned_entries")] long OrphanedEntries);

// ─────────────────────────────────────────────────────────────────────────────
// Tasks
// ─────────────────────────────────────────────────────────────────────────────

public record TaskListResponse(
    [property: JsonPropertyName("tasks")] TaskItem[] Tasks);

public record TaskItem(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("project_id")] string ProjectId,
    [property: JsonPropertyName("name")] string Name);

public record TaskCreateRequest(
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("project_id")] string ProjectId);

public record TaskUpdateRequest(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("project_id")] string ProjectId);

// ─────────────────────────────────────────────────────────────────────────────
// Tags
// ─────────────────────────────────────────────────────────────────────────────

public record TagListResponse(
    [property: JsonPropertyName("tags")] TagItem[] Tags);

public record TagItem(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("created_at")] string CreatedAt,
    [property: JsonPropertyName("entry_count")] long EntryCount);

// ─────────────────────────────────────────────────────────────────────────────
// Fuzzy Match
// ─────────────────────────────────────────────────────────────────────────────

public record FuzzyMatchProjectsResponse(
    [property: JsonPropertyName("matches")] ProjectMatch[] Matches);

public record ProjectMatch(
    [property: JsonPropertyName("project_id")] string ProjectId,
    [property: JsonPropertyName("project_name")] string ProjectName,
    [property: JsonPropertyName("client_id")] string ClientId,
    [property: JsonPropertyName("client_name")] string ClientName,
    [property: JsonPropertyName("score")] double Score);

public record FuzzyMatchTasksResponse(
    [property: JsonPropertyName("matches")] TaskMatch[] Matches);

public record TaskMatch(
    [property: JsonPropertyName("task_id")] string TaskId,
    [property: JsonPropertyName("task_name")] string TaskName,
    [property: JsonPropertyName("score")] double Score);

// ─────────────────────────────────────────────────────────────────────────────
// Screenshots
// ─────────────────────────────────────────────────────────────────────────────

public record ScreenshotItem(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("file_path")] string FilePath,
    [property: JsonPropertyName("captured_at")] string CapturedAt,
    [property: JsonPropertyName("window_title")] string WindowTitle,
    [property: JsonPropertyName("process_name")] string ProcessName,
    [property: JsonPropertyName("trigger")] string Trigger);

// ─────────────────────────────────────────────────────────────────────────────
// Idle Detection
// ─────────────────────────────────────────────────────────────────────────────

public record IdleStatusResponse(
    [property: JsonPropertyName("is_idle")] bool IsIdle,
    [property: JsonPropertyName("idle_seconds")] long IdleSeconds,
    [property: JsonPropertyName("idle_since")] string? IdleSince);

public record IdleResolveRequest(
    [property: JsonPropertyName("resolution")] string Resolution,
    [property: JsonPropertyName("idle_started_at")] string IdleStartedAt,
    [property: JsonPropertyName("idle_ended_at")] string IdleEndedAt,
    [property: JsonPropertyName("entry_details")] IdleEntryDetails? EntryDetails);

public record IdleEntryDetails(
    [property: JsonPropertyName("description")] string Description,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("tag_ids")] string[] TagIds);

public record IdleResolveResponse(
    [property: JsonPropertyName("created_entry_id")] string? CreatedEntryId);

// ─────────────────────────────────────────────────────────────────────────────
// Sync
// ─────────────────────────────────────────────────────────────────────────────

public record SyncStatusResponse(
    [property: JsonPropertyName("enabled")] bool Enabled,
    [property: JsonPropertyName("connected")] bool Connected,
    [property: JsonPropertyName("pending_queue_size")] long PendingQueueSize,
    [property: JsonPropertyName("last_sync_at")] string? LastSyncAt,
    [property: JsonPropertyName("last_error")] string? LastError);

public record SyncConfigureRequest(
    [property: JsonPropertyName("connection_uri")] string ConnectionUri,
    [property: JsonPropertyName("enabled")] bool Enabled);

public record SyncConfigureResponse(
    [property: JsonPropertyName("connected")] bool Connected);

public record SyncTriggerResponse(
    [property: JsonPropertyName("synced_records")] long SyncedRecords,
    [property: JsonPropertyName("errors")] long Errors);
