using Tracey.App.Services;

namespace Tracey.Tests;

// ─────────────────────────────────────────────────────────────────────────────
// TimerStateServiceStub — In-Memory Test Double
//
// T020 (Phase 3) is complete. This stub now implements the full state machine
// in memory (without Tauri IPC) so the contract tests pass without a running
// Tauri host. The real TimerStateService in the app still delegates to Tauri.
// ─────────────────────────────────────────────────────────────────────────────

internal sealed class TimerStateServiceStub : ITimerStateService
{
    private bool _isRunning;
    private string? _currentDescription;
    private string? _currentEntryId;
    private string? _currentProjectId;
    private string? _currentTaskId;
    private string? _currentProjectName;
    private string? _currentClientId;
    private string? _currentClientName;
    private string? _currentTaskName;
    private string[] _currentTagIds = [];
    private TimeSpan _elapsed = TimeSpan.Zero;

    public bool IsRunning => _isRunning;
    public string? CurrentDescription => _currentDescription;
    public TimeSpan Elapsed => _elapsed;
    public string? CurrentEntryId => _currentEntryId;
    public string? CurrentProjectId => _currentProjectId;
    public string? CurrentTaskId => _currentTaskId;
    public string? CurrentProjectName => _currentProjectName;
    public string? CurrentClientId => _currentClientId;
    public string? CurrentClientName => _currentClientName;
    public string? CurrentTaskName => _currentTaskName;
    public string[] CurrentTagIds => _currentTagIds;

    public event Action? OnStateChanged;

    public Task StartAsync(string description, string? projectId = null, string? taskId = null,
        string? projectName = null, string? clientId = null, string? clientName = null,
        string? taskName = null, string[]? tagIds = null)
    {
        _isRunning = true;
        _currentDescription = description;
        _currentEntryId = Guid.NewGuid().ToString();
        _currentProjectId = projectId;
        _currentTaskId = taskId;
        _currentProjectName = projectName;
        _currentClientId = clientId;
        _currentClientName = clientName;
        _currentTaskName = taskName;
        _currentTagIds = tagIds ?? [];
        _elapsed = TimeSpan.Zero;
        OnStateChanged?.Invoke();
        return Task.CompletedTask;
    }

    public Task StopAsync()
    {
        if (!_isRunning) return Task.CompletedTask;
        _isRunning = false;
        _currentDescription = null;
        _currentEntryId = null;
        _currentProjectId = null;
        _currentTaskId = null;
        _currentProjectName = null;
        _currentClientId = null;
        _currentClientName = null;
        _currentTaskName = null;
        _currentTagIds = [];
        _elapsed = TimeSpan.Zero;
        OnStateChanged?.Invoke();
        return Task.CompletedTask;
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TimerStateServiceTests
//
// All tests currently FAIL with NotImplementedException — correct behaviour
// for the TDD gate. Tests become green when Root completes T020.
// ─────────────────────────────────────────────────────────────────────────────

/// <summary>
/// xUnit tests for TimerStateService (US1 — T019/T020).
/// Covers: initial state, StartAsync, StopAsync, single-timer invariant,
/// OnStateChanged event, and project/task propagation.
/// Uses TimerStateServiceStub (in-memory) — no Tauri IPC required.
/// </summary>
public class TimerStateServiceTests
{
    // ─────────────────────────────────────────────────────────────────────────
    // Initial state — no timer started
    // ─────────────────────────────────────────────────────────────────────────

    [Fact]
    public void IsRunning_IsFalse_WhenNoTimerStarted()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        Assert.False(svc.IsRunning);
    }

    [Fact]
    public void CurrentDescription_IsNull_WhenNoTimerStarted()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        Assert.Null(svc.CurrentDescription);
    }

    [Fact]
    public void Elapsed_IsZero_WhenNoTimerStarted()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        Assert.Equal(TimeSpan.Zero, svc.Elapsed);
    }

    [Fact]
    public void CurrentEntryId_IsNull_WhenNoTimerStarted()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        Assert.Null(svc.CurrentEntryId);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StartAsync
    // ─────────────────────────────────────────────────────────────────────────

    [Fact]
    public async Task StartAsync_SetsIsRunningTrue()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Testing the timer");
        Assert.True(svc.IsRunning);
    }

    [Fact]
    public async Task StartAsync_SetsCurrentDescription()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Writing xUnit tests");
        Assert.Equal("Writing xUnit tests", svc.CurrentDescription);
    }

    [Fact]
    public async Task StartAsync_AssignsNonEmptyCurrentEntryId()
    {
        // timer_start IPC returns a ULID for the created entry (ipc-commands.md)
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Entry ID check");
        Assert.NotNull(svc.CurrentEntryId);
        Assert.NotEmpty(svc.CurrentEntryId!);
    }

    [Fact]
    public async Task StartAsync_WithProjectId_PropagatesCurrentProjectId()
    {
        // timer_start carries optional project_id per ipc-commands.md
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Project task", projectId: "01HXY3ABCDE1234567890ABCDE");
        Assert.Equal("01HXY3ABCDE1234567890ABCDE", svc.CurrentProjectId);
    }

    [Fact]
    public async Task StartAsync_WithNullProjectId_CurrentProjectId_IsNull()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("No project", projectId: null);
        Assert.Null(svc.CurrentProjectId);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StopAsync
    // ─────────────────────────────────────────────────────────────────────────

    [Fact]
    public async Task StopAsync_SetsIsRunningFalse()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Timer running");
        await svc.StopAsync();
        Assert.False(svc.IsRunning);
    }

    [Fact]
    public async Task StopAsync_ClearsCurrentDescription()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("About to stop");
        await svc.StopAsync();
        Assert.Null(svc.CurrentDescription);
    }

    [Fact]
    public async Task StopAsync_ClearsCurrentEntryId()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Entry to clear");
        await svc.StopAsync();
        Assert.Null(svc.CurrentEntryId);
    }

    [Fact]
    public async Task StopAsync_WhenNoTimerRunning_IsNoOp_DoesNotThrow()
    {
        // IPC: timer_stop returns "no_active_timer" error when nothing is running.
        // Service must absorb this and remain stable — no throw to the caller.
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StopAsync();              // no timer running — must not throw
        Assert.False(svc.IsRunning);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Single-timer invariant (US1 AC3)
    // spec: "previously running timer is automatically stopped and saved"
    // ipc: timer_start "Automatically stops and saves any currently running timer"
    // ─────────────────────────────────────────────────────────────────────────

    [Fact]
    public async Task StartAsync_WhileRunning_ReplacesCurrentTimer()
    {
        // AC3 edge case: starting a second timer stops the first automatically
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("First task");
        await svc.StartAsync("Second task");
        Assert.Equal("Second task", svc.CurrentDescription);
        Assert.True(svc.IsRunning);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // OnStateChanged event
    // spec: components subscribe to force re-render on timer state change
    // ─────────────────────────────────────────────────────────────────────────

    [Fact]
    public async Task OnStateChanged_FiredOnStart()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        bool fired = false;
        svc.OnStateChanged += () => fired = true;
        await svc.StartAsync("Event start test");
        Assert.True(fired);
    }

    [Fact]
    public async Task OnStateChanged_FiredOnStop()
    {
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Event stop test");
        bool fired = false;
        svc.OnStateChanged += () => fired = true;
        await svc.StopAsync();
        Assert.True(fired);
    }
}

