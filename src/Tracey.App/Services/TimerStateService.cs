using System.Text.Json.Serialization;
using Microsoft.JSInterop;

namespace Tracey.App.Services;

/// Defined by Shaw (T019) — this is the contract our tests assert against.
public interface ITimerStateService
{
    bool IsRunning { get; }
    string? CurrentDescription { get; }
    TimeSpan Elapsed { get; }
    string? CurrentEntryId { get; }
    string? CurrentProjectId { get; }
    string? CurrentTaskId { get; }

    Task StartAsync(string description, string? projectId = null, string? taskId = null);
    Task StopAsync();
    event Action? OnStateChanged;
}

public class TimerStateService : ITimerStateService
{
    private readonly TauriIpcService _tauri;
    private bool _isRunning;
    private string? _currentDescription;
    private string? _currentEntryId;
    private string? _currentProjectId;
    private string? _currentTaskId;
    private string? _startedAt; // UTC ISO string from Rust
    private long _elapsedSeconds; // updated by timer-tick events
    private System.Threading.PeriodicTimer? _localTicker;
    private CancellationTokenSource? _tickerCts;

    public bool IsRunning => _isRunning;
    public string? CurrentDescription => _currentDescription;
    public string? CurrentEntryId => _currentEntryId;
    public string? CurrentProjectId => _currentProjectId;
    public string? CurrentTaskId => _currentTaskId;

    public TimeSpan Elapsed => TimeSpan.FromSeconds(_elapsedSeconds);

    public event Action? OnStateChanged;

    public TimerStateService(TauriIpcService tauri)
    {
        _tauri = tauri;
    }

    /// Called by TauriEventService when tracey://timer-tick fires
    public void HandleTimerTick(long elapsedSeconds)
    {
        _elapsedSeconds = elapsedSeconds;  // Rust is ground truth — sync to its value
        OnStateChanged?.Invoke();
    }

    private void StartLocalTicker()
    {
        StopLocalTicker();
        _tickerCts = new CancellationTokenSource();
        var cts = _tickerCts;
        _localTicker = new System.Threading.PeriodicTimer(TimeSpan.FromSeconds(1));
        var ticker = _localTicker;
        _ = Task.Run(async () =>
        {
            try
            {
                while (await ticker.WaitForNextTickAsync(cts.Token))
                {
                    _elapsedSeconds++;
                    OnStateChanged?.Invoke();
                }
            }
            catch (OperationCanceledException) { }
        });
    }

    private void StopLocalTicker()
    {
        _tickerCts?.Cancel();
        _tickerCts?.Dispose();
        _tickerCts = null;
        _localTicker?.Dispose();
        _localTicker = null;
    }

    /// Load active timer from Rust on app startup (restores state across restarts)
    public async Task InitializeAsync()
    {
        try
        {
            var active = await _tauri.TimerGetActiveAsync();
            if (active.Id != null)
            {
                _isRunning = true;
                _currentEntryId = active.Id;
                _currentDescription = active.Description;
                _currentProjectId = active.ProjectId;
                _currentTaskId = active.TaskId;
                _startedAt = active.StartedAt;
                if (DateTime.TryParse(active.StartedAt, null,
                    System.Globalization.DateTimeStyles.RoundtripKind, out var start))
                {
                    _elapsedSeconds = (long)(DateTime.UtcNow - start).TotalSeconds;
                }
                if (_isRunning) StartLocalTicker();
                OnStateChanged?.Invoke();
            }
        }
        catch (Exception ex)
        {
            Console.Error.WriteLine($"[TimerStateService] InitializeAsync failed: {ex.Message}");
        }
    }

    public async Task StartAsync(string description, string? projectId = null, string? taskId = null)
    {
        var result = await _tauri.TimerStartAsync(new TimerStartRequest(
            description,
            projectId,
            taskId,
            []
        ));

        _isRunning = true;
        _currentEntryId = result.Id;
        _currentDescription = description;
        _currentProjectId = projectId;
        _currentTaskId = taskId;
        _startedAt = result.StartedAt;
        _elapsedSeconds = 0;
        StartLocalTicker();

        OnStateChanged?.Invoke();
    }

    public async Task StopAsync()
    {
        if (!_isRunning) return; // no-op per Shaw's test
        StopLocalTicker();

        try
        {
            await _tauri.TimerStopAsync();
        }
        catch (Exception ex) when (ex.Message.Contains("no_active_timer"))
        {
            // Already stopped — sync local state
        }

        _isRunning = false;
        _currentEntryId = null;
        _currentDescription = null;
        _currentProjectId = null;
        _currentTaskId = null;
        _startedAt = null;
        _elapsedSeconds = 0;

        OnStateChanged?.Invoke();
    }
}
