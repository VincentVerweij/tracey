using System.Text.Json.Serialization;

namespace Tracey.App.Services.Notifications;

/// <summary>
/// Represents a single outbound notification channel (e.g., email, Telegram).
/// Implement this interface to add new channels without modifying any existing code (FR-054, SC-010).
/// </summary>
public interface INotificationChannel
{
    /// <summary>Unique identifier for this channel (e.g., "email", "telegram").</summary>
    string ChannelId { get; }

    /// <summary>
    /// Send a notification message through this channel using the provided settings.
    /// Throws <see cref="NotSupportedException"/> if the channel cannot run in the current runtime.
    /// </summary>
    Task SendAsync(NotificationMessage message, NotificationChannelSettings settings, CancellationToken cancellationToken = default);

    /// <summary>
    /// Default (empty/disabled) settings returned when no user config exists for this channel.
    /// </summary>
    NotificationChannelSettings DefaultSettings { get; }
}

/// <summary>The notification message delivered to each channel when a threshold is exceeded.</summary>
/// <param name="Title">Short headline (e.g., "Timer Running for 8+ Hours")</param>
/// <param name="Body">Human-readable details including entry description and elapsed time.</param>
/// <param name="TriggeredAt">UTC moment the notification was triggered.</param>
/// <param name="Duration">Elapsed duration of the running timer at notification time.</param>
public record NotificationMessage(
    string Title,
    string Body,
    DateTimeOffset TriggeredAt,
    TimeSpan Duration);

/// <summary>
/// Runtime configuration for a notification channel, loaded from user preferences.
/// Stored in <c>notification_channels_json</c> as part of the preferences array.
/// </summary>
/// <param name="Enabled">Whether the channel should receive notifications.</param>
/// <param name="Config">Channel-specific key/value settings (e.g., bot_token, smtp_host).</param>
public record NotificationChannelSettings(
    bool Enabled,
    IReadOnlyDictionary<string, string> Config)
{
    /// <summary>Convenience instance representing a disabled channel with empty config.</summary>
    public static readonly NotificationChannelSettings Disabled =
        new(Enabled: false, Config: new Dictionary<string, string>());

    /// <summary>
    /// Gets a config value or returns the provided default if the key is missing or blank.
    /// </summary>
    public string Get(string key, string defaultValue = "") =>
        Config.TryGetValue(key, out var v) && !string.IsNullOrWhiteSpace(v) ? v : defaultValue;
}

/// <summary>
/// Wire format for one entry in the <c>notification_channels_json</c> preferences array.
/// Used for JSON serialisation / deserialisation only.
/// </summary>
public record NotificationChannelConfigEntry(
    [property: JsonPropertyName("channel_id")] string ChannelId,
    [property: JsonPropertyName("enabled")] bool Enabled,
    [property: JsonPropertyName("config")] Dictionary<string, string> Config);
