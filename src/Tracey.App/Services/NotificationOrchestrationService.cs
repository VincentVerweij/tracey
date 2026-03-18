using System.Text.Json;
using Microsoft.Extensions.Hosting;
using Tracey.App.Services.Notifications;

namespace Tracey.App.Services;

/// <summary>
/// Background service that monitors the running timer and sends notifications
/// through all enabled <see cref="INotificationChannel"/> implementations when
/// the elapsed time exceeds the configured threshold (default: 8 hours).
///
/// Satisfies: FR-052 (threshold monitoring), FR-054 (channel abstraction),
/// FR-055/FR-056 (email + telegram channels), FR-057 (all channels notified).
///
/// Runs as a hosted service in Blazor WASM (.NET 10+).
/// Registration: builder.Services.AddHostedService&lt;NotificationOrchestrationService&gt;()
/// </summary>
public class NotificationOrchestrationService : BackgroundService
{
    private const double DefaultThresholdHours = 8.0;
    private static readonly TimeSpan CheckInterval = TimeSpan.FromSeconds(60);

    private readonly IEnumerable<INotificationChannel> _channels;
    private readonly ITimerStateService _timerState;
    private readonly TauriIpcService _tauri;
    private readonly TauriEventService _events;

    // Track which timer entry we've already notified for, so we don't repeat every minute
    private string? _notifiedForEntryId;

    public NotificationOrchestrationService(
        IEnumerable<INotificationChannel> channels,
        ITimerStateService timerState,
        TauriIpcService tauri,
        TauriEventService events)
    {
        _channels   = channels;
        _timerState = timerState;
        _tauri      = tauri;
        _events     = events;
    }

    protected override async Task ExecuteAsync(CancellationToken stoppingToken)
    {
        Console.WriteLine($"[Notifications] ✅ BackgroundService started at {DateTimeOffset.UtcNow:HH:mm:ss}. " +
                          $"Poll interval: {CheckInterval.TotalSeconds}s.");
        using var timer = new System.Threading.PeriodicTimer(CheckInterval);
        try
        {
            while (await timer.WaitForNextTickAsync(stoppingToken))
            {
                await CheckAndNotifyAsync(stoppingToken);
            }
        }
        catch (OperationCanceledException)
        {
            // Normal shutdown — swallow
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine(
                $"[Notifications] ❌ Unhandled error in background loop: {ex.Message}");
        }
    }

    private async Task CheckAndNotifyAsync(CancellationToken ct)
    {
        var now = DateTimeOffset.UtcNow;
        Console.WriteLine($"[Notifications] 🔍 Poll at {now:HH:mm:ss} — " +
                          $"IsRunning={_timerState.IsRunning}, " +
                          $"EntryId={_timerState.CurrentEntryId ?? "null"}, " +
                          $"Elapsed={_timerState.Elapsed.TotalMinutes:F1}min");
        try
        {
            if (!_timerState.IsRunning)
            {
                // Timer stopped — reset so next start is fresh
                _notifiedForEntryId = null;
                Console.WriteLine("[Notifications] ⏸ Timer not running — skipping.");
                return;
            }

            var entryId = _timerState.CurrentEntryId;
            if (entryId == null)
            {
                Console.WriteLine("[Notifications] ⚠ IsRunning=true but CurrentEntryId is null — skipping.");
                return;
            }

            // Already notified for this specific timer run
            if (_notifiedForEntryId == entryId)
            {
                Console.WriteLine($"[Notifications] 🔕 Already notified for entry {entryId} — skipping.");
                return;
            }

            // Load preferences to get threshold and channel configs
            UserPreferences prefs;
            try
            {
                prefs = await _tauri.PreferencesGetAsync();
            }
            catch (Exception ex)
            {
                Console.Error.WriteLine(
                    $"[Notifications] ❌ Failed to load preferences: {ex.Message}");
                return;
            }

            var thresholdHours = prefs.TimerNotificationThresholdHours > 0
                ? prefs.TimerNotificationThresholdHours
                : DefaultThresholdHours;

            var elapsed = _timerState.Elapsed;
            Console.WriteLine($"[Notifications] ⏱ Elapsed={elapsed.TotalMinutes:F1}min, " +
                              $"Threshold={thresholdHours * 60:F1}min ({thresholdHours}h), " +
                              $"ChannelsJson={(string.IsNullOrWhiteSpace(prefs.NotificationChannelsJson) ? "null/empty" : prefs.NotificationChannelsJson)}");

            if (elapsed.TotalHours < thresholdHours)
            {
                Console.WriteLine($"[Notifications] ⏳ Threshold not yet reached — {(thresholdHours - elapsed.TotalHours) * 60:F1}min remaining.");
                return;
            }

            Console.WriteLine($"[Notifications] 🚀 Threshold exceeded! Sending via all enabled channels...");

            // Threshold exceeded — build channel settings map from preferences JSON
            var channelSettingsMap = BuildChannelSettingsMap(prefs.NotificationChannelsJson);

            // Build the notification message
            var h = (int)elapsed.TotalHours;
            var m = elapsed.Minutes;
            var desc = string.IsNullOrWhiteSpace(_timerState.CurrentDescription)
                ? "a timer"
                : $"\"{_timerState.CurrentDescription}\"";

            var message = new NotificationMessage(
                Title:       $"Timer Running for {h}h {m}m",
                Body:        $"You have {desc} that has been running for {h} hours and {m} minutes.",
                TriggeredAt: DateTimeOffset.UtcNow,
                Duration:    elapsed);

            // Send via all enabled channels
            foreach (var channel in _channels)
            {
                var settings = channelSettingsMap.TryGetValue(channel.ChannelId, out var s)
                    ? s
                    : channel.DefaultSettings;

                Console.WriteLine($"[Notifications] 📡 Channel '{channel.ChannelId}': " +
                                  $"Enabled={settings.Enabled}, " +
                                  $"ConfigKeys=[{string.Join(", ", settings.Config.Keys)}], " +
                                  $"HasToken={!string.IsNullOrWhiteSpace(settings.Get("bot_token"))}");

                if (!settings.Enabled)
                {
                    Console.WriteLine($"[Notifications] ⏭ Channel '{channel.ChannelId}' is disabled — skipping.");
                    continue;
                }

                try
                {
                    await channel.SendAsync(message, settings, ct);
                    Console.WriteLine($"[Notifications] ✅ Sent via '{channel.ChannelId}'.");

                    // Raise the in-app event so UI can show a toast or indicator
                    _events.RaiseNotificationSent(new NotificationSentPayload(
                        ChannelId: channel.ChannelId,
                        Message:   message.Title));
                }
                catch (NotSupportedException nse)
                {
                    // Expected for email in WASM — log and continue
                    Console.WriteLine($"[Notifications] ℹ Channel '{channel.ChannelId}' not supported: {nse.Message}");
                }
                catch (Exception ex)
                {
                    Console.Error.WriteLine($"[Notifications] ❌ Channel '{channel.ChannelId}' FAILED: {ex.GetType().Name}: {ex.Message}");
                }
            }

            // Mark as notified so we don't re-send every minute for the same timer run
            _notifiedForEntryId = entryId;
            Console.WriteLine($"[Notifications] 🏁 Done. Will not re-notify for entry {entryId}.");
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[Notifications] ❌ CheckAndNotifyAsync error: {ex.GetType().Name}: {ex.Message}");
        }
    }

    private static Dictionary<string, NotificationChannelSettings> BuildChannelSettingsMap(
        string? notificationChannelsJson)
    {
        var map = new Dictionary<string, NotificationChannelSettings>(StringComparer.OrdinalIgnoreCase);
        if (string.IsNullOrWhiteSpace(notificationChannelsJson)) return map;

        try
        {
            var entries = JsonSerializer.Deserialize<List<NotificationChannelConfigEntry>>(
                notificationChannelsJson,
                new JsonSerializerOptions { PropertyNameCaseInsensitive = true });

            if (entries == null) return map;

            foreach (var entry in entries)
            {
                map[entry.ChannelId] = new NotificationChannelSettings(
                    Enabled: entry.Enabled,
                    Config:  entry.Config ?? new Dictionary<string, string>());
            }
        }
        catch (JsonException ex)
        {
            Console.Error.WriteLine(
                $"[NotificationOrchestrationService] Failed to parse notification_channels_json: {ex.Message}");
        }

        return map;
    }
}
