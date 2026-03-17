using Microsoft.JSInterop;
using System.Text.Json;
using System.Text.Json.Serialization;

namespace Tracey.App.Services;

public class TauriEventService : IDisposable
{
    private readonly IJSRuntime _js;
    private DotNetObjectReference<TauriEventService>? _dotNetRef;

    private static readonly JsonSerializerOptions _jsonOptions = new()
    {
        PropertyNameCaseInsensitive = true
    };

    public TauriEventService(IJSRuntime js) => _js = js;

    public event Action<TimerTickPayload>? OnTimerTick;
    public event Action<IdleDetectedPayload>? OnIdleDetected;
    public event Action<IdleResolvedPayload>? OnIdleResolved;
    public event Action<ScreenshotCapturedPayload>? OnScreenshotCaptured;
    public event Action<SyncStatusPayload>? OnSyncStatusChanged;
    public event Action<NotificationSentPayload>? OnNotificationSent;
    public event Action<ErrorPayload>? OnError;

    public async Task InitializeAsync()
    {
        _dotNetRef = DotNetObjectReference.Create(this);
        try
        {
            await _js.InvokeVoidAsync("traceyBridge.initializeTauriBridge", _dotNetRef);
        }
        catch (JSException ex) when (ex.Message.Contains("__TAURI_INTERNALS__"))
        {
            // Running outside Tauri host (e.g., plain browser during dev) — events won't fire
            Console.WriteLine("[TauriEventService] Bridge not available (non-Tauri host)");
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[TauriEventService] InitializeAsync failed: {ex.Message}");
        }
    }

    [JSInvokable]
    public void RouteEvent(string eventName, string jsonPayload)
    {
        try
        {
            switch (eventName)
            {
                case "tracey://timer-tick":
                    var tick = JsonSerializer.Deserialize<TimerTickPayload>(jsonPayload, _jsonOptions);
                    if (tick != null) OnTimerTick?.Invoke(tick);
                    break;
                case "tracey://idle-detected":
                    var idle = JsonSerializer.Deserialize<IdleDetectedPayload>(jsonPayload, _jsonOptions);
                    if (idle != null) OnIdleDetected?.Invoke(idle);
                    break;
                case "tracey://idle-resolved":
                    var resolved = JsonSerializer.Deserialize<IdleResolvedPayload>(jsonPayload, _jsonOptions);
                    if (resolved != null) OnIdleResolved?.Invoke(resolved);
                    break;
                case "tracey://screenshot-captured":
                    var shot = JsonSerializer.Deserialize<ScreenshotCapturedPayload>(jsonPayload, _jsonOptions);
                    if (shot != null) OnScreenshotCaptured?.Invoke(shot);
                    break;
                case "tracey://sync-status-changed":
                    var sync = JsonSerializer.Deserialize<SyncStatusPayload>(jsonPayload, _jsonOptions);
                    if (sync != null) OnSyncStatusChanged?.Invoke(sync);
                    break;
                case "tracey://notification-sent":
                    var notif = JsonSerializer.Deserialize<NotificationSentPayload>(jsonPayload, _jsonOptions);
                    if (notif != null) OnNotificationSent?.Invoke(notif);
                    break;
                case "tracey://error":
                    var err = JsonSerializer.Deserialize<ErrorPayload>(jsonPayload, _jsonOptions);
                    if (err != null) OnError?.Invoke(err);
                    break;
                default:
                    Console.WriteLine($"[TauriEventService] Unknown event: {eventName}");
                    break;
            }
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[TauriEventService] RouteEvent failed for {eventName}: {ex.Message}");
        }
    }

    public void Dispose()
    {
        try { _js.InvokeVoidAsync("traceyBridge.disposeTauriBridge"); } catch { }
        _dotNetRef?.Dispose();
    }
}

// ─── Event payload types ───────────────────────────────────────────────────────

public record TimerTickPayload(
    [property: JsonPropertyName("elapsed_seconds")] long ElapsedSeconds);

public record IdleDetectedPayload(
    [property: JsonPropertyName("idle_since")] string IdleSince,
    [property: JsonPropertyName("had_active_timer")] bool HadActiveTimer);

public record IdleResolvedPayload(
    [property: JsonPropertyName("created_entry_id")] string? CreatedEntryId);

public record ScreenshotCapturedPayload(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("captured_at")] string CapturedAt);

public record SyncStatusPayload(
    [property: JsonPropertyName("connected")] bool Connected,
    [property: JsonPropertyName("pending")] long Pending);

public record NotificationSentPayload(
    [property: JsonPropertyName("channel_id")] string ChannelId,
    [property: JsonPropertyName("message")] string Message);

public record ErrorPayload(
    [property: JsonPropertyName("component")] string Component,
    [property: JsonPropertyName("event")] string Event,
    [property: JsonPropertyName("error")] string Error);
