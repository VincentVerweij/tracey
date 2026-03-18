using System.Net.Http.Json;
using System.Text.Json.Serialization;

namespace Tracey.App.Services.Notifications;

/// <summary>
/// Telegram Bot API notification channel.
/// Uses <see cref="HttpClient"/> which in Blazor WASM is implemented via the browser's
/// fetch API — fully functional in WebView2 for outbound HTTPS calls.
///
/// Config keys (stored in preferences, editable in Settings UI):
///   bot_token — Telegram Bot API token (from @BotFather)
///   chat_id   — Target chat or channel ID (can be negative for groups/channels)
/// </summary>
public class TelegramNotificationChannel : INotificationChannel
{
    private readonly IHttpClientFactory _httpClientFactory;

    public TelegramNotificationChannel(IHttpClientFactory httpClientFactory)
    {
        _httpClientFactory = httpClientFactory;
    }

    public string ChannelId => "telegram";

    public NotificationChannelSettings DefaultSettings => new(
        Enabled: false,
        Config: new Dictionary<string, string>
        {
            ["bot_token"] = "",
            ["chat_id"]   = ""
        });

    /// <inheritdoc />
    public async Task SendAsync(
        NotificationMessage message,
        NotificationChannelSettings settings,
        CancellationToken cancellationToken = default)
    {
        var botToken = settings.Get("bot_token");
        var chatId   = settings.Get("chat_id");

        if (string.IsNullOrWhiteSpace(botToken))
            throw new InvalidOperationException("Telegram channel: bot_token is not configured.");
        if (string.IsNullOrWhiteSpace(chatId))
            throw new InvalidOperationException("Telegram channel: chat_id is not configured.");

        var text = $"*{EscapeMarkdown(message.Title)}*\n{EscapeMarkdown(message.Body)}";

        var payload = new TelegramSendMessageRequest(
            ChatId:    chatId,
            Text:      text,
            ParseMode: "MarkdownV2");

        var http = _httpClientFactory.CreateClient();

        // Build URL with bot token in path (standard Telegram Bot API pattern)
        var url = $"https://api.telegram.org/bot{Uri.EscapeDataString(botToken)}/sendMessage";

        var response = await http.PostAsJsonAsync(url, payload, cancellationToken);

        if (!response.IsSuccessStatusCode)
        {
            var body = await response.Content.ReadAsStringAsync(cancellationToken);
            throw new HttpRequestException(
                $"Telegram API returned {(int)response.StatusCode}: {body}");
        }
    }

    // Escape special characters required by Telegram MarkdownV2
    private static string EscapeMarkdown(string text) =>
        text.Replace("_", "\\_")
            .Replace("*", "\\*")
            .Replace("[", "\\[")
            .Replace("]", "\\]")
            .Replace("(", "\\(")
            .Replace(")", "\\)")
            .Replace("~", "\\~")
            .Replace("`", "\\`")
            .Replace(">", "\\>")
            .Replace("#", "\\#")
            .Replace("+", "\\+")
            .Replace("-", "\\-")
            .Replace("=", "\\=")
            .Replace("|", "\\|")
            .Replace("{", "\\{")
            .Replace("}", "\\}")
            .Replace(".", "\\.")
            .Replace("!", "\\!");
}

file record TelegramSendMessageRequest(
    [property: JsonPropertyName("chat_id")]    string ChatId,
    [property: JsonPropertyName("text")]       string Text,
    [property: JsonPropertyName("parse_mode")] string ParseMode);
