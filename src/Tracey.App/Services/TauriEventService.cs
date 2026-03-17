using Microsoft.JSInterop;
using System.Text.Json.Serialization;

namespace Tracey.App.Services;

/// <summary>
/// Subscribes to Tauri event emissions from the Rust backend.
/// Uses window.__TAURI_INTERNALS__.listen (Tauri 2.0).
///
/// NOTE: Full callback wiring requires a JS shim that bridges the Tauri JS event
/// API to a DotNetObjectReference. The Listen&lt;T&gt; stub registers intent but does
/// not yet route payloads. Wire the JS shim before activating consumers.
/// Tracked as: decisions/inbox/root-t015-t016-t017.md
/// </summary>
public class TauriEventService
{
    private readonly IJSRuntime _js;

    public TauriEventService(IJSRuntime js) => _js = js;

    // Events match contract: specs/001-window-activity-tracker/contracts/ipc-commands.md
    public event Action<TimerTickPayload>? OnTimerTick;
    public event Action<IdleDetectedPayload>? OnIdleDetected;
    public event Action<IdleResolvedPayload>? OnIdleResolved;
    public event Action<ScreenshotCapturedPayload>? OnScreenshotCaptured;
    public event Action<SyncStatusPayload>? OnSyncStatusChanged;
    public event Action<NotificationSentPayload>? OnNotificationSent;
    public event Action<ErrorPayload>? OnError;

    public async Task InitializeAsync()
    {
        await Listen<TimerTickPayload>("tracey://timer-tick", p => OnTimerTick?.Invoke(p));
        await Listen<IdleDetectedPayload>("tracey://idle-detected", p => OnIdleDetected?.Invoke(p));
        await Listen<IdleResolvedPayload>("tracey://idle-resolved", p => OnIdleResolved?.Invoke(p));
        await Listen<ScreenshotCapturedPayload>("tracey://screenshot-captured", p => OnScreenshotCaptured?.Invoke(p));
        await Listen<SyncStatusPayload>("tracey://sync-status-changed", p => OnSyncStatusChanged?.Invoke(p));
        await Listen<NotificationSentPayload>("tracey://notification-sent", p => OnNotificationSent?.Invoke(p));
        await Listen<ErrorPayload>("tracey://error", p => OnError?.Invoke(p));
    }

    private async Task Listen<T>(string eventName, Action<T> handler)
    {
        // Tauri 2.0: window.__TAURI_INTERNALS__.listen(event, callback)
        // Blazor WASM cannot pass a C# delegate directly to JS.
        // A JS shim must call DotNetObjectReference.invokeMethodAsync with a
        // serialized payload, which then deserializes to T and invokes handler.
        // TODO: implement JS shim in Final Phase (see decisions inbox).
        await Task.CompletedTask;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Event payload types — shapes match contract Tauri Events table
// ─────────────────────────────────────────────────────────────────────────────

/// <summary>tracey://timer-tick — emitted every second while a timer runs.</summary>
public record TimerTickPayload(
    [property: JsonPropertyName("elapsed_seconds")] long ElapsedSeconds);

/// <summary>tracey://idle-detected — emitted when idle threshold is crossed.</summary>
public record IdleDetectedPayload(
    [property: JsonPropertyName("idle_since")] string IdleSince,
    [property: JsonPropertyName("had_active_timer")] bool HadActiveTimer);

/// <summary>tracey://idle-resolved — emitted after idle resolution is saved.</summary>
public record IdleResolvedPayload(
    [property: JsonPropertyName("created_entry_id")] string? CreatedEntryId);

/// <summary>tracey://screenshot-captured — emitted after each screenshot is saved.</summary>
public record ScreenshotCapturedPayload(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("captured_at")] string CapturedAt);

/// <summary>tracey://sync-status-changed — emitted when sync state changes.</summary>
public record SyncStatusPayload(
    [property: JsonPropertyName("connected")] bool Connected,
    [property: JsonPropertyName("pending")] long Pending);

/// <summary>tracey://notification-sent — emitted when a notification fires.</summary>
public record NotificationSentPayload(
    [property: JsonPropertyName("channel_id")] string ChannelId,
    [property: JsonPropertyName("message")] string Message);

/// <summary>tracey://error — emitted on recoverable errors requiring user notice.</summary>
public record ErrorPayload(
    [property: JsonPropertyName("component")] string Component,
    [property: JsonPropertyName("event")] string Event,
    [property: JsonPropertyName("error")] string Error);
