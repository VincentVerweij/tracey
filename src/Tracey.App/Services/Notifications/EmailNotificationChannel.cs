namespace Tracey.App.Services.Notifications;

/// <summary>
/// Email notification channel.
///
/// NOTE — WASM LIMITATION:
/// SMTP requires raw TCP socket connections, which are NOT available in the
/// Blazor WebAssembly browser runtime (WebView2). This channel is a stub that
/// throws <see cref="NotSupportedException"/> when <see cref="SendAsync"/> is called.
///
/// MailKit 4.15.1 is already referenced in the project. Full SMTP delivery will be
/// wired through a Tauri IPC command to a Rust SMTP relay in a future phase.
///
/// Config keys (stored in preferences, editable in Settings UI):
///   smtp_host, smtp_port, smtp_user, smtp_pass, smtp_from, smtp_to
/// </summary>
public class EmailNotificationChannel : INotificationChannel
{
    public string ChannelId => "email";

    public NotificationChannelSettings DefaultSettings => new(
        Enabled: false,
        Config: new Dictionary<string, string>
        {
            ["smtp_host"] = "",
            ["smtp_port"] = "587",
            ["smtp_user"] = "",
            ["smtp_pass"] = "",
            ["smtp_from"] = "",
            ["smtp_to"] = ""
        });

    /// <inheritdoc />
    /// <remarks>
    /// Always throws <see cref="NotSupportedException"/> — SMTP sockets are unavailable
    /// in the Blazor WASM browser environment. See class-level documentation.
    /// </remarks>
    public Task SendAsync(
        NotificationMessage message,
        NotificationChannelSettings settings,
        CancellationToken cancellationToken = default)
    {
        throw new NotSupportedException(
            "Email notifications require the Tauri (Rust) desktop process. " +
            "SMTP raw socket connections are not available in the Blazor WASM browser runtime. " +
            "Tauri IPC routing for email is planned for a future phase.");
    }
}
