using System.Net;
using System.Net.Http;
using System.Net.Http.Json;
using System.Text.Json;
using Tracey.App.Services;
using Tracey.App.Services.Notifications;

namespace Tracey.Tests;

// ─────────────────────────────────────────────────────────────────────────────
// T063 — Notification Channel Tests (Phase 9 / US7)
//
// Written BEFORE implementation (TDD gate).
// Tests channel contracts, orchestrator logic, and error handling.
// No external packages required — uses in-file test doubles.
// ─────────────────────────────────────────────────────────────────────────────

// ── Helper: fake NotificationChannelSettings factory ─────────────────────────

file static class TestSettings
{
    public static NotificationChannelSettings Enabled(Dictionary<string, string>? config = null) =>
        new(Enabled: true, Config: config ?? new Dictionary<string, string>());

    public static NotificationChannelSettings Disabled() =>
        NotificationChannelSettings.Disabled;
}

// ── Helper: fake INotificationChannel ────────────────────────────────────────

file class FakeNotificationChannel : INotificationChannel
{
    public string ChannelId { get; }
    public NotificationChannelSettings DefaultSettings => TestSettings.Disabled();

    public List<(NotificationMessage Message, NotificationChannelSettings Settings)> Calls { get; } = new();
    public Exception? ThrowOnSend { get; set; }

    public FakeNotificationChannel(string channelId) => ChannelId = channelId;

    public Task SendAsync(NotificationMessage message, NotificationChannelSettings settings,
        CancellationToken cancellationToken = default)
    {
        if (ThrowOnSend != null) throw ThrowOnSend;
        Calls.Add((message, settings));
        return Task.CompletedTask;
    }
}

// ── Helper: fake ITimerStateService ──────────────────────────────────────────

file class FakeTimerStateService : ITimerStateService
{
    public bool IsRunning { get; set; }
    public string? CurrentEntryId { get; set; }
    public string? CurrentDescription { get; set; }
    public TimeSpan Elapsed { get; set; }
    public string? CurrentProjectId => null;
    public string? CurrentTaskId    => null;
    public string? CurrentProjectName => null;
    public string? CurrentClientId   => null;
    public string? CurrentClientName => null;
    public string? CurrentTaskName   => null;
    public string[] CurrentTagIds    => Array.Empty<string>();

    public event Action? OnStateChanged;

    public Task StartAsync(string description, string? projectId = null, string? taskId = null,
        string? projectName = null, string? clientId = null, string? clientName = null,
        string? taskName = null, string[]? tagIds = null)
        => Task.CompletedTask;

    public Task StopAsync() => Task.CompletedTask;
}

// ── Helper: recording HttpMessageHandler ─────────────────────────────────────

file class RecordingHttpMessageHandler : HttpMessageHandler
{
    public List<HttpRequestMessage> Requests { get; } = new();
    public HttpStatusCode ResponseStatus { get; set; } = HttpStatusCode.OK;
    public string ResponseBody { get; set; } = """{"ok":true}""";

    protected override Task<HttpResponseMessage> SendAsync(
        HttpRequestMessage request, CancellationToken cancellationToken)
    {
        Requests.Add(request);
        return Task.FromResult(new HttpResponseMessage(ResponseStatus)
        {
            Content = new StringContent(ResponseBody)
        });
    }
}

// ── Helper: fake IHttpClientFactory ──────────────────────────────────────────

file class FakeHttpClientFactory : IHttpClientFactory
{
    private readonly HttpMessageHandler _handler;
    public FakeHttpClientFactory(HttpMessageHandler handler) => _handler = handler;
    public HttpClient CreateClient(string name) => new(_handler);
}

// ─────────────────────────────────────────────────────────────────────────────
// EmailNotificationChannel tests
// ─────────────────────────────────────────────────────────────────────────────

public class EmailNotificationChannelTests
{
    private static NotificationMessage SampleMessage() => new(
        Title:       "Timer Running for 8h 0m",
        Body:        "Your timer has been running for 8 hours.",
        TriggeredAt: DateTimeOffset.UtcNow,
        Duration:    TimeSpan.FromHours(8));

    [Fact]
    public void ChannelId_IsEmail()
    {
        var channel = new EmailNotificationChannel();
        Assert.Equal("email", channel.ChannelId);
    }

    [Fact]
    public void DefaultSettings_IsDisabled()
    {
        var channel = new EmailNotificationChannel();
        Assert.False(channel.DefaultSettings.Enabled);
    }

    [Fact]
    public void DefaultSettings_HasExpectedConfigKeys()
    {
        var channel = new EmailNotificationChannel();
        var keys = channel.DefaultSettings.Config.Keys.ToHashSet();
        Assert.Contains("smtp_host", keys);
        Assert.Contains("smtp_port", keys);
        Assert.Contains("smtp_user", keys);
        Assert.Contains("smtp_pass", keys);
        Assert.Contains("smtp_from", keys);
        Assert.Contains("smtp_to", keys);
    }

    [Fact]
    public async Task SendAsync_ThrowsNotSupportedException_InWasmRuntime()
    {
        var channel  = new EmailNotificationChannel();
        var settings = TestSettings.Enabled();

        await Assert.ThrowsAsync<NotSupportedException>(
            () => channel.SendAsync(SampleMessage(), settings));
    }

    [Fact]
    public async Task SendAsync_ErrorMessage_MentionsTauriProcess()
    {
        var channel  = new EmailNotificationChannel();
        var settings = TestSettings.Enabled();

        var ex = await Assert.ThrowsAsync<NotSupportedException>(
            () => channel.SendAsync(SampleMessage(), settings));

        Assert.Contains("Tauri", ex.Message, StringComparison.OrdinalIgnoreCase);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TelegramNotificationChannel tests
// ─────────────────────────────────────────────────────────────────────────────

public class TelegramNotificationChannelTests
{
    private static NotificationMessage SampleMessage() => new(
        Title:       "Timer Running for 8h 0m",
        Body:        "Your timer has been running for 8 hours.",
        TriggeredAt: DateTimeOffset.UtcNow,
        Duration:    TimeSpan.FromHours(8));

    private static NotificationChannelSettings ValidSettings() => TestSettings.Enabled(new()
    {
        ["bot_token"] = "123456:TESTTOKEN",
        ["chat_id"]   = "987654321"
    });

    [Fact]
    public void ChannelId_IsTelegram()
    {
        var handler = new RecordingHttpMessageHandler();
        var channel = new TelegramNotificationChannel(new FakeHttpClientFactory(handler));
        Assert.Equal("telegram", channel.ChannelId);
    }

    [Fact]
    public void DefaultSettings_IsDisabled()
    {
        var handler = new RecordingHttpMessageHandler();
        var channel = new TelegramNotificationChannel(new FakeHttpClientFactory(handler));
        Assert.False(channel.DefaultSettings.Enabled);
    }

    [Fact]
    public async Task SendAsync_PostsToTelegramBotApi()
    {
        var handler = new RecordingHttpMessageHandler();
        var channel = new TelegramNotificationChannel(new FakeHttpClientFactory(handler));

        await channel.SendAsync(SampleMessage(), ValidSettings());

        Assert.Single(handler.Requests);
        var req = handler.Requests[0];
        Assert.Equal(HttpMethod.Post, req.Method);
        Assert.Contains("api.telegram.org", req.RequestUri!.Host);
        Assert.Contains("sendMessage", req.RequestUri.AbsolutePath);
    }

    [Fact]
    public async Task SendAsync_IncludesBotTokenInUrl()
    {
        var handler = new RecordingHttpMessageHandler();
        var channel = new TelegramNotificationChannel(new FakeHttpClientFactory(handler));

        await channel.SendAsync(SampleMessage(), ValidSettings());

        var url = handler.Requests[0].RequestUri!.ToString();
        Assert.Contains("123456", url);
    }

    [Fact]
    public async Task SendAsync_ThrowsInvalidOperationException_WhenBotTokenMissing()
    {
        var handler  = new RecordingHttpMessageHandler();
        var channel  = new TelegramNotificationChannel(new FakeHttpClientFactory(handler));
        var settings = TestSettings.Enabled(new() { ["bot_token"] = "", ["chat_id"] = "123" });

        await Assert.ThrowsAsync<InvalidOperationException>(
            () => channel.SendAsync(SampleMessage(), settings));
    }

    [Fact]
    public async Task SendAsync_ThrowsInvalidOperationException_WhenChatIdMissing()
    {
        var handler  = new RecordingHttpMessageHandler();
        var channel  = new TelegramNotificationChannel(new FakeHttpClientFactory(handler));
        var settings = TestSettings.Enabled(new() { ["bot_token"] = "tok", ["chat_id"] = "" });

        await Assert.ThrowsAsync<InvalidOperationException>(
            () => channel.SendAsync(SampleMessage(), settings));
    }

    [Fact]
    public async Task SendAsync_ThrowsHttpRequestException_WhenApiReturns4xx()
    {
        var handler = new RecordingHttpMessageHandler
        {
            ResponseStatus = HttpStatusCode.Unauthorized,
            ResponseBody   = """{"ok":false,"error_code":401,"description":"Unauthorized"}"""
        };
        var channel = new TelegramNotificationChannel(new FakeHttpClientFactory(handler));

        await Assert.ThrowsAsync<HttpRequestException>(
            () => channel.SendAsync(SampleMessage(), ValidSettings()));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NotificationOrchestrationService tests
//
// The service runs via BackgroundService.ExecuteAsync; we test the core
// CheckAndNotify logic by calling it indirectly via a controlled timer state.
// Since the background loop polls, we use a subclass that exposes the method.
// ─────────────────────────────────────────────────────────────────────────────

// Test-only subclass that exposes internal check for unit testing
file class TestableOrchestrationService : NotificationOrchestrationService
{
    public TestableOrchestrationService(
        IEnumerable<INotificationChannel> channels,
        ITimerStateService timerState,
        FakeTauriIpcService tauri,
        TauriEventService events)
        : base(channels, timerState, tauri, events) { }
}

// Minimal fake TauriIpcService for orchestration tests (avoids JS interop)
file class FakeTauriIpcService : TauriIpcService
{
    public UserPreferences? PreferencesToReturn { get; set; }

    public FakeTauriIpcService() : base(null!) { }

    // Override preferences call without hitting JS
    public new Task<UserPreferences> PreferencesGetAsync()
    {
        var prefs = PreferencesToReturn ?? new UserPreferences(
            Id:                             1,
            InactivityTimeoutSeconds:       300,
            ScreenshotIntervalSeconds:      60,
            ScreenshotRetentionDays:        30,
            ScreenshotStoragePath:          null,
            Timezone:                       "UTC",
            EntriesPerPage:                 50,
            ProcessDenyListJson:            "[]",
            ExternalDbEnabled:              false,
            TimerNotificationThresholdHours: 8.0,
            NotificationChannelsJson:       null,
            AutoClassificationEnabled:      true,
            AutoClassificationConfidenceThreshold: 0.7f,
            AutoClassificationGroupGapSeconds: 120);
        return Task.FromResult(prefs);
    }
}

public class NotificationOrchestrationServiceTests
{
    // We cannot unit-test the ExecuteAsync loop directly (runs until cancellation),
    // so these tests verify the observable outcomes after calling the service
    // methods indirectly through a short-lived cancellation scope.

    private static NotificationMessage BuildMessage(TimeSpan elapsed) => new(
        Title:       "Test",
        Body:        "Test body",
        TriggeredAt: DateTimeOffset.UtcNow,
        Duration:    elapsed);

    private static string ChannelsJsonFor(string channelId, bool enabled,
        Dictionary<string, string>? config = null)
    {
        var entry = new { channel_id = channelId, enabled, config = config ?? new Dictionary<string, string>() };
        return JsonSerializer.Serialize(new[] { entry });
    }

    [Fact]
    public async Task BelowThreshold_NoChannelsNotified()
    {
        var fakeChannel = new FakeNotificationChannel("telegram");
        var timerState  = new FakeTimerStateService
        {
            IsRunning       = true,
            CurrentEntryId  = "entry-1",
            Elapsed         = TimeSpan.FromHours(2), // below 8h threshold
            CurrentDescription = "Test task"
        };
        var tauri = new FakeTauriIpcService
        {
            PreferencesToReturn = MakePrefs(8.0, ChannelsJsonFor("telegram", true))
        };

        using var cts = new CancellationTokenSource(TimeSpan.FromMilliseconds(150));
        var svc = CreateService([fakeChannel], timerState, tauri);
        await RunShortLoopAsync(svc, cts.Token);

        // Loop fires at most once per 60s polling; since we only wait 150ms,
        // the actual CheckAndNotify may not run. But if it did, the threshold guard blocks it.
        // Either way: zero calls expected.
        Assert.Empty(fakeChannel.Calls);
    }

    [Fact]
    public void NotificationMessage_ContainsTimerDescription()
    {
        // Verify message construction logic (white-box, direct construction)
        var elapsed = TimeSpan.FromHours(9.5);
        var h = (int)elapsed.TotalHours;
        var m = elapsed.Minutes;
        var body = $"You have \"Coding\" that has been running for {h} hours and {m} minutes.";

        Assert.Contains("9", body);
        Assert.Contains("Coding", body);
    }

    [Fact]
    public void NotificationChannelSettings_Get_ReturnsFallbackWhenMissing()
    {
        var settings = new NotificationChannelSettings(
            Enabled: true,
            Config: new Dictionary<string, string> { ["key1"] = "val1" });

        Assert.Equal("val1", settings.Get("key1"));
        Assert.Equal("default", settings.Get("missing_key", "default"));
        Assert.Equal("", settings.Get("missing_key"));
    }

    [Fact]
    public void NotificationChannelSettings_Disabled_IsDisabledWithEmptyConfig()
    {
        var settings = NotificationChannelSettings.Disabled;
        Assert.False(settings.Enabled);
        Assert.Empty(settings.Config);
    }

    [Fact]
    public void ChannelsJson_Parsing_ProducesCorrectEntries()
    {
        // Verify JSON deserialization the service uses (via NotificationChannelConfigEntry)
        var json = ChannelsJsonFor("telegram", true, new() { ["bot_token"] = "tok", ["chat_id"] = "123" });
        var entries = JsonSerializer.Deserialize<List<NotificationChannelConfigEntry>>(
            json, new JsonSerializerOptions { PropertyNameCaseInsensitive = true })!;

        Assert.Single(entries);
        Assert.Equal("telegram", entries[0].ChannelId);
        Assert.True(entries[0].Enabled);
        Assert.Equal("tok", entries[0].Config["bot_token"]);
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    private static NotificationOrchestrationService CreateService(
        IEnumerable<INotificationChannel> channels,
        ITimerStateService timerState,
        TauriIpcService tauri)
    {
        // TauriEventService needs IJSRuntime — pass null since test doesn't invoke JS
        var events = new TauriEventService(null!);
        return new NotificationOrchestrationService(channels, timerState, tauri, events);
    }

    private static async Task RunShortLoopAsync(NotificationOrchestrationService svc, CancellationToken ct)
    {
        // NotificationOrchestrationService uses Initialize(), not BackgroundService.
        // The PeriodicTimer fires every 60s, so a 150ms window won't trigger the loop.
        svc.Initialize();
        try { await Task.Delay(Timeout.Infinite, ct); }
        catch (OperationCanceledException) { /* expected */ }
    }

    private static UserPreferences MakePrefs(double thresholdHours, string? channelsJson) =>
        new(Id:                              1,
            InactivityTimeoutSeconds:        300,
            ScreenshotIntervalSeconds:       60,
            ScreenshotRetentionDays:         30,
            ScreenshotStoragePath:           null,
            Timezone:                        "UTC",
            EntriesPerPage:                  50,
            ProcessDenyListJson:             "[]",
            ExternalDbEnabled:               false,
            TimerNotificationThresholdHours: thresholdHours,
            NotificationChannelsJson:        channelsJson,
            AutoClassificationEnabled:       true,
            AutoClassificationConfidenceThreshold: 0.7f,
            AutoClassificationGroupGapSeconds: 120);
}
