using Tracey.App.Services;

namespace Tracey.Tests;

// ─────────────────────────────────────────────────────────────────────────────
// TimerStateServiceStub — Failing Test Double
//
// Every member throws NotImplementedException.
// Tests against this stub document the required contract.
// Root replaces this with a real implementation in T020 (Phase 3).
// ─────────────────────────────────────────────────────────────────────────────

internal sealed class TimerStateServiceStub : ITimerStateService
{
    private const string NotYetImplemented =
        "TimerStateService not yet implemented — T020 (Phase 3).";

    public bool IsRunning =>
        throw new NotImplementedException(NotYetImplemented);

    public string? CurrentDescription =>
        throw new NotImplementedException(NotYetImplemented);

    public TimeSpan Elapsed =>
        throw new NotImplementedException(NotYetImplemented);

    public string? CurrentEntryId =>
        throw new NotImplementedException(NotYetImplemented);

    public string? CurrentProjectId =>
        throw new NotImplementedException(NotYetImplemented);

    public string? CurrentTaskId =>
        throw new NotImplementedException(NotYetImplemented);

    public string? CurrentProjectName =>
        throw new NotImplementedException(NotYetImplemented);

    public string? CurrentClientId =>
        throw new NotImplementedException(NotYetImplemented);

    public string? CurrentClientName =>
        throw new NotImplementedException(NotYetImplemented);

    public string? CurrentTaskName =>
        throw new NotImplementedException(NotYetImplemented);

    public string[] CurrentTagIds =>
        throw new NotImplementedException(NotYetImplemented);

    public event Action? OnStateChanged
    {
        add => throw new NotImplementedException(NotYetImplemented);
        remove => throw new NotImplementedException(NotYetImplemented);
    }

    public Task StartAsync(string description, string? projectId = null, string? taskId = null,
        string? projectName = null, string? clientId = null, string? clientName = null,
        string? taskName = null, string[]? tagIds = null) =>
        throw new NotImplementedException(NotYetImplemented);

    public Task StopAsync() =>
        throw new NotImplementedException(NotYetImplemented);
}

// ─────────────────────────────────────────────────────────────────────────────
// TimerStateServiceTests
//
// All tests currently FAIL with NotImplementedException — correct behaviour
// for the TDD gate. Tests become green when Root completes T020.
// ─────────────────────────────────────────────────────────────────────────────

/// <summary>
/// xUnit tests for TimerStateService (US1 — T019).
/// Covers: initial state, StartAsync, StopAsync, single-timer invariant,
/// OnStateChanged event, and project/task propagation.
/// Written before implementation per TDD gate. EXPECTED: all fail.
/// </summary>
public class TimerStateServiceTests
{
    // ─────────────────────────────────────────────────────────────────────────
    // Initial state — no timer started
    // ─────────────────────────────────────────────────────────────────────────

    [Fact]
    public void IsRunning_IsFalse_WhenNoTimerStarted()
    {
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        Assert.False(svc.IsRunning);
    }

    [Fact]
    public void CurrentDescription_IsNull_WhenNoTimerStarted()
    {
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        Assert.Null(svc.CurrentDescription);
    }

    [Fact]
    public void Elapsed_IsZero_WhenNoTimerStarted()
    {
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        Assert.Equal(TimeSpan.Zero, svc.Elapsed);
    }

    [Fact]
    public void CurrentEntryId_IsNull_WhenNoTimerStarted()
    {
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        Assert.Null(svc.CurrentEntryId);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // StartAsync
    // ─────────────────────────────────────────────────────────────────────────

    [Fact]
    public async Task StartAsync_SetsIsRunningTrue()
    {
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Testing the timer");
        Assert.True(svc.IsRunning);
    }

    [Fact]
    public async Task StartAsync_SetsCurrentDescription()
    {
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Writing xUnit tests");
        Assert.Equal("Writing xUnit tests", svc.CurrentDescription);
    }

    [Fact]
    public async Task StartAsync_AssignsNonEmptyCurrentEntryId()
    {
        // timer_start IPC returns a ULID for the created entry (ipc-commands.md)
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Entry ID check");
        Assert.NotNull(svc.CurrentEntryId);
        Assert.NotEmpty(svc.CurrentEntryId!);
    }

    [Fact]
    public async Task StartAsync_WithProjectId_PropagatesCurrentProjectId()
    {
        // timer_start carries optional project_id per ipc-commands.md
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Project task", projectId: "01HXY3ABCDE1234567890ABCDE");
        Assert.Equal("01HXY3ABCDE1234567890ABCDE", svc.CurrentProjectId);
    }

    [Fact]
    public async Task StartAsync_WithNullProjectId_CurrentProjectId_IsNull()
    {
        // FAILS — NotImplementedException until T020
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
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Timer running");
        await svc.StopAsync();
        Assert.False(svc.IsRunning);
    }

    [Fact]
    public async Task StopAsync_ClearsCurrentDescription()
    {
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("About to stop");
        await svc.StopAsync();
        Assert.Null(svc.CurrentDescription);
    }

    [Fact]
    public async Task StopAsync_ClearsCurrentEntryId()
    {
        // FAILS — NotImplementedException until T020
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
        // FAILS — NotImplementedException until T020
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
        // FAILS — NotImplementedException until T020
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
        // FAILS — NotImplementedException when subscribing (until T020)
        ITimerStateService svc = new TimerStateServiceStub();
        bool fired = false;
        svc.OnStateChanged += () => fired = true;
        await svc.StartAsync("Event start test");
        Assert.True(fired);
    }

    [Fact]
    public async Task OnStateChanged_FiredOnStop()
    {
        // FAILS — NotImplementedException until T020
        ITimerStateService svc = new TimerStateServiceStub();
        await svc.StartAsync("Event stop test");
        bool fired = false;
        svc.OnStateChanged += () => fired = true;
        await svc.StopAsync();
        Assert.True(fired);
    }
}

