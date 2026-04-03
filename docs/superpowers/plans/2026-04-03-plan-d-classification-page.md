# Classification Page — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A dedicated "Classification" page in the main nav that shows a live feed of the current window/classification and a paginated history list with inline correction support.

**Architecture:** A new `classification_event_list` Tauri command pages through `classification_events` ordered by `created_at DESC`. The Blazor page subscribes to `tracey://classification-needed` and `tracey://screenshot-captured` events to refresh the live panel. Inline corrections from history rows call the existing `classification_submit_label` command.

**Tech Stack:** Blazor, C#, TauriIpcService, TauriEventService (all existing patterns).

**Depends on:** Plan C (classification_events table, submit_label command, classification-needed event).

---

### Task 1: Add `classification_event_list` Tauri command

**Files:**
- Modify: `src-tauri/src/commands/classification.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Write the failing test**

Add to `src-tauri/src/commands/classification.rs` (in `#[cfg(test)]` at the end of the file):

```rust
#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("
            CREATE TABLE classification_events (
                id TEXT PRIMARY KEY, war_id TEXT NOT NULL,
                process_name TEXT NOT NULL, window_title TEXT NOT NULL,
                client_id TEXT, project_id TEXT, task_id TEXT,
                confidence REAL NOT NULL DEFAULT 0.0,
                classification_source TEXT NOT NULL DEFAULT 'unclassified',
                outcome TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL
            );
        ").unwrap();
        conn.execute_batch("
            INSERT INTO classification_events VALUES
              ('e1','w1','Code','tracey',NULL,'p1',NULL,0.9,'heuristic','auto','2026-01-01T10:00:00Z'),
              ('e2','w2','Slack','general',NULL,'p2',NULL,0.4,'tf_idf','pending','2026-01-01T10:01:00Z');
        ").unwrap();
        conn
    }

    #[test]
    fn list_returns_events_descending() {
        let conn = test_db();
        let mut stmt = conn.prepare(
            "SELECT id, process_name, confidence FROM classification_events \
             ORDER BY created_at DESC LIMIT 50 OFFSET 0"
        ).unwrap();
        let rows: Vec<(String, String, f64)> = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
            .unwrap().filter_map(|r| r.ok()).collect();
        assert_eq!(rows.len(), 2);
        // Most recent first
        assert_eq!(rows[0].0, "e2");
        assert_eq!(rows[1].0, "e1");
    }
}
```

- [ ] **Step 2: Run the test to verify it fails (event list not yet implemented)**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features test classification::tests::list_returns_events_descending 2>&1
```

Expected: test passes (it only tests SQL logic inline, not the command — this verifies the query design).

- [ ] **Step 3: Add the `ClassificationEventItem` type, `classification_event_list`, and `fuzzy_match_projects` commands**

Add to `src-tauri/src/commands/classification.rs`:

```rust
// ── Classification event list (for Classification page) ───────────────────────

#[derive(Serialize)]
pub struct ClassificationEventItem {
    pub id: String,
    pub war_id: String,
    pub process_name: String,
    pub window_title: String,
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub confidence: f64,
    pub classification_source: String,
    pub outcome: String,
    pub ocr_text: Option<String>,
    pub created_at: String,
}

#[derive(Deserialize)]
pub struct ClassificationEventListRequest {
    pub page: i64,      // 0-based
    pub page_size: i64, // e.g. 50
}

#[derive(Serialize)]
pub struct ClassificationEventListResponse {
    pub items: Vec<ClassificationEventItem>,
    pub total: i64,
}

#[tauri::command]
pub fn classification_event_list(
    state: State<'_, AppState>,
    request: ClassificationEventListRequest,
) -> Result<ClassificationEventListResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let offset = request.page * request.page_size;

    let total: i64 = conn.query_row(
        "SELECT COUNT(*) FROM classification_events",
        [],
        |r| r.get(0),
    ).unwrap_or(0);

    let mut stmt = conn.prepare(
        "SELECT id, war_id, process_name, window_title, client_id, project_id, task_id, \
                confidence, classification_source, outcome, ocr_text, created_at \
         FROM classification_events \
         ORDER BY created_at DESC \
         LIMIT ?1 OFFSET ?2",
    ).map_err(|e| e.to_string())?;

    let items: Vec<ClassificationEventItem> = stmt.query_map(
        rusqlite::params![request.page_size, offset],
        |r| Ok(ClassificationEventItem {
            id:                    r.get(0)?,
            war_id:                r.get(1)?,
            process_name:          r.get(2)?,
            window_title:          r.get(3)?,
            client_id:             r.get(4)?,
            project_id:            r.get(5)?,
            task_id:               r.get(6)?,
            confidence:            r.get(7)?,
            classification_source: r.get(8)?,
            outcome:               r.get(9)?,
            ocr_text:              r.get(10)?,
            created_at:            r.get(11)?,
        }),
    ).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(ClassificationEventListResponse { items, total })
}

// ── Fuzzy project search (used by correction forms in Classification page + Toast) ─

#[derive(Deserialize)]
pub struct FuzzyMatchProjectsRequest {
    pub query: String,
    pub limit: i64,
}

#[derive(Serialize)]
pub struct FuzzyProjectItem {
    pub id: String,
    pub name: String,
    pub client_id: Option<String>,
}

#[derive(Serialize)]
pub struct FuzzyMatchProjectsResponse {
    pub projects: Vec<FuzzyProjectItem>,
}

#[tauri::command]
pub fn fuzzy_match_projects(
    state: State<'_, AppState>,
    query: String,
    limit: i64,
) -> Result<FuzzyMatchProjectsResponse, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let pattern = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT p.id, p.name, p.client_id \
         FROM projects p \
         WHERE lower(p.name) LIKE ?1 \
         ORDER BY p.name ASC \
         LIMIT ?2",
    ).map_err(|e| e.to_string())?;

    let projects: Vec<FuzzyProjectItem> = stmt.query_map(
        rusqlite::params![pattern, limit],
        |r| Ok(FuzzyProjectItem {
            id:        r.get(0)?,
            name:      r.get(1)?,
            client_id: r.get(2)?,
        }),
    ).map_err(|e| e.to_string())?
    .filter_map(|r| r.ok())
    .collect();

    Ok(FuzzyMatchProjectsResponse { projects })
}
```

- [ ] **Step 4: Register commands in `lib.rs`**

Add to `invoke_handler`:

```rust
commands::classification::classification_event_list,
commands::classification::fuzzy_match_projects,
```

- [ ] **Step 5: Build and run tests**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --features test 2>&1
```

Expected: builds and all tests pass.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/commands/classification.rs `
        src-tauri/src/lib.rs
git commit -m "feat(classification): add classification_event_list Tauri command"
```

---

### Task 2: Add `ClassificationEventListAsync` to `TauriIpcService.cs`

**Files:**
- Modify: `src/Tracey.App/Services/TauriIpcService.cs`

- [ ] **Step 1: Add response types and IPC method**

Add to `TauriIpcService.cs` in the Classification section:

```csharp
public record ClassificationEventItem(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("war_id")] string WarId,
    [property: JsonPropertyName("process_name")] string ProcessName,
    [property: JsonPropertyName("window_title")] string WindowTitle,
    [property: JsonPropertyName("client_id")] string? ClientId,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("confidence")] float Confidence,
    [property: JsonPropertyName("classification_source")] string ClassificationSource,
    [property: JsonPropertyName("outcome")] string Outcome,
    [property: JsonPropertyName("ocr_text")] string? OcrText,
    [property: JsonPropertyName("created_at")] string CreatedAt);

public record ClassificationEventListResponse(
    [property: JsonPropertyName("items")] ClassificationEventItem[] Items,
    [property: JsonPropertyName("total")] long Total);

public record FuzzyProjectItem(
    [property: JsonPropertyName("id")] string Id,
    [property: JsonPropertyName("name")] string Name,
    [property: JsonPropertyName("client_id")] string? ClientId);

public record FuzzyMatchProjectsResponse(
    [property: JsonPropertyName("projects")] FuzzyProjectItem[] Projects);

public Task<ClassificationEventListResponse> ClassificationEventListAsync(int page = 0, int pageSize = 50) =>
    Invoke<ClassificationEventListResponse>(
        "classification_event_list",
        new { request = new { page, page_size = pageSize } });

public Task<FuzzyMatchProjectsResponse> FuzzyMatchProjectsAsync(string query, int limit = 5) =>
    Invoke<FuzzyMatchProjectsResponse>(
        "fuzzy_match_projects",
        new { query, limit });
```

- [ ] **Step 2: Build frontend**

```powershell
dotnet build src/Tracey.App
```

Expected: builds without errors.

- [ ] **Step 3: Commit**

```powershell
git add src/Tracey.App/Services/TauriIpcService.cs
git commit -m "feat(frontend): add ClassificationEventListAsync IPC method and response types"
```

---

### Task 3: Create `Pages/Classification.razor`

**Files:**
- Create: `src/Tracey.App/Pages/Classification.razor`
- Create: `src/Tracey.App/Pages/Classification.razor.css`

- [ ] **Step 1: Create the page**

```razor
@page "/classification"
@inject TauriIpcService Tauri
@inject TauriEventService Events
@implements IDisposable

<div class="classification-page">

    <section class="live-panel">
        <h2 class="section-title">Current activity</h2>
        @if (_liveEvent != null)
        {
            <div class="live-card">
                <div class="live-app">@_liveEvent.ProcessName</div>
                <div class="live-title" title="@_liveEvent.WindowTitle">@_liveEvent.WindowTitle</div>
                <div class="live-meta">
                    <span class="source-badge source-@_liveEvent.ClassificationSource">
                        @FormatSource(_liveEvent.ClassificationSource)
                    </span>
                    <span class="confidence-bar-wrap">
                        <span class="confidence-bar" style="width: @(Math.Round(_liveEvent.Confidence * 100))%"></span>
                    </span>
                    <span class="confidence-value">@Math.Round(_liveEvent.Confidence * 100)%</span>
                    <span class="outcome-badge outcome-@_liveEvent.Outcome">@_liveEvent.Outcome</span>
                </div>
                @if (!string.IsNullOrEmpty(_liveEvent.ProjectId))
                {
                    <div class="live-assignment">→ @FormatAssignment(_liveEvent)</div>
                }
                @if (!string.IsNullOrEmpty(_liveEvent.OcrText))
                {
                    <details class="live-ocr">
                        <summary>OCR text</summary>
                        <pre class="live-ocr-text">@_liveEvent.OcrText</pre>
                    </details>
                }
            </div>
        }
        else
        {
            <p class="empty-state">No classification data yet. The engine starts after a window change.</p>
        }
    </section>

    <section class="history-section">
        <h2 class="section-title">History</h2>

        @if (_loading)
        {
            <p>Loading entries…</p>
        }
        else if (_items.Length == 0)
        {
            <p class="empty-state">No classification events yet.</p>
        }
        else
        {
            <table class="history-table">
                <thead>
                    <tr>
                        <th>Time</th>
                        <th>App</th>
                        <th>Window</th>
                        <th>Assignment</th>
                        <th>Confidence</th>
                        <th>Outcome</th>
                        <th></th>
                    </tr>
                </thead>
                <tbody>
                    @foreach (var item in _items)
                    {
                        <tr class="history-row @(item.Id == _correctingId ? "correcting" : "")">
                            <td class="col-time">@FormatTime(item.CreatedAt)</td>
                            <td class="col-app">@item.ProcessName</td>
                            <td class="col-title" title="@item.WindowTitle">@TruncateTitle(item.WindowTitle)</td>
                            <td class="col-assignment">@FormatAssignment(item)</td>
                            <td class="col-confidence">@Math.Round(item.Confidence * 100)%</td>
                            <td class="col-outcome">
                                <span class="outcome-badge outcome-@item.Outcome">@item.Outcome</span>
                            </td>
                            <td class="col-actions">
                                <button type="button" class="btn-correct"
                                        @onclick="() => StartCorrection(item)">Correct</button>
                            </td>
                        </tr>
                        @if (item.Id == _correctingId)
                        {
                            <tr class="correction-row">
                                <td colspan="7">
                                    <div class="correction-form">
                                        <span>Assign to:</span>
                                        <input type="text" @bind="_correctionQuery"
                                               placeholder="Search projects…"
                                               @oninput="OnCorrectionInput" />
                                        @if (_correctionMatches.Length > 0)
                                        {
                                            <div class="correction-matches">
                                                @foreach (var m in _correctionMatches)
                                                {
                                                    <button type="button"
                                                            class="correction-match-btn @(_correctionProjectId == m.Id ? "selected" : "")"
                                                            @onclick="() => { _correctionProjectId = m.Id; _correctionQuery = m.Name; _correctionMatches = []; StateHasChanged(); }">
                                                        @m.Name
                                                    </button>
                                                }
                                            </div>
                                        }
                                        <button type="button" @onclick="() => SubmitCorrection(item)">Save</button>
                                        <button type="button" @onclick="CancelCorrection">Cancel</button>
                                        @if (!string.IsNullOrEmpty(_correctionError))
                                        {
                                            <span class="error">@_correctionError</span>
                                        }
                                    </div>
                                </td>
                            </tr>
                        }
                    }
                </tbody>
            </table>

            <div class="pagination">
                <button type="button" @onclick="PrevPage" disabled="@(_page == 0)">← Previous</button>
                <span>Page @(_page + 1) of @TotalPages</span>
                <button type="button" @onclick="NextPage" disabled="@(_page >= TotalPages - 1)">Next →</button>
            </div>
        }
    </section>
</div>

@code {
    private const int PageSize = 50;
    private bool _loading = true;
    private ClassificationEventItem[] _items = [];
    private long _total = 0;
    private int _page = 0;
    private ClassificationEventItem? _liveEvent;
    private string? _correctingId;
    private string _correctionQuery = string.Empty;
    private string _correctionError = string.Empty;
    private string? _correctionProjectId;
    private FuzzyProjectItem[] _correctionMatches = [];

    private int TotalPages => (int)Math.Ceiling((double)_total / PageSize);

    protected override async Task OnInitializedAsync()
    {
        Events.OnClassificationNeeded += OnClassificationNeeded;
        await LoadPage();
    }

    private async Task LoadPage()
    {
        _loading = true;
        StateHasChanged();
        var resp = await Tauri.ClassificationEventListAsync(_page, PageSize);
        _items = resp?.Items ?? [];
        _total = resp?.Total ?? 0;
        _liveEvent ??= _items.FirstOrDefault();
        _loading = false;
        StateHasChanged();
    }

    private void OnClassificationNeeded(ClassificationNeededPayload payload)
    {
        // Refresh the list to show the new pending event
        InvokeAsync(LoadPage);
    }

    private async Task PrevPage()
    {
        if (_page > 0) { _page--; await LoadPage(); }
    }

    private async Task NextPage()
    {
        if (_page < TotalPages - 1) { _page++; await LoadPage(); }
    }

    private void StartCorrection(ClassificationEventItem item)
    {
        _correctingId = item.Id;
        _correctionQuery = string.Empty;
        _correctionError = string.Empty;
        StateHasChanged();
    }

    private void CancelCorrection()
    {
        _correctingId = null;
        StateHasChanged();
    }

    private void OnCorrectionInput(ChangeEventArgs e)
    {
        _correctionQuery = e.Value?.ToString() ?? string.Empty;
        InvokeAsync(async () =>
        {
            if (_correctionQuery.Length >= 2)
            {
                var results = await Tauri.FuzzyMatchProjectsAsync(_correctionQuery, 5);
                _correctionMatches = results?.Projects ?? [];
            }
            else
            {
                _correctionMatches = [];
            }
            StateHasChanged();
        });
    }

    private async Task SubmitCorrection(ClassificationEventItem item)
    {
        if (string.IsNullOrWhiteSpace(_correctionProjectId))
        {
            _correctionError = "Select a project to assign.";
            StateHasChanged();
            return;
        }
        await Tauri.ClassificationSubmitLabelAsync(new ClassificationSubmitLabelRequest(
            WarId: item.WarId,
            EventId: item.Id,
            ProcessName: item.ProcessName,
            WindowTitle: item.WindowTitle,
            OcrText: null,
            ClientId: item.ClientId,
            ProjectId: _correctionProjectId,
            TaskId: item.TaskId,
            RecordedAt: item.CreatedAt,
            Source: "user_corrected"
        ));
        _correctingId = null;
        await LoadPage();
    }

    private static string FormatTime(string iso) =>
        DateTimeOffset.TryParse(iso, out var dt)
            ? dt.LocalDateTime.ToString("HH:mm:ss")
            : iso;

    private static string TruncateTitle(string title) =>
        title.Length > 50 ? title[..47] + "…" : title;

    private static string FormatAssignment(ClassificationEventItem item) =>
        string.Join(" / ", new[] { item.ClientId, item.ProjectId, item.TaskId }
            .Where(x => !string.IsNullOrEmpty(x)));

    private static string FormatSource(string source) => source switch {
        "heuristic" => "Rule",
        "tf_idf" => "Model",
        _ => "–"
    };

    public void Dispose()
    {
        Events.OnClassificationNeeded -= OnClassificationNeeded;
    }
}
```

- [ ] **Step 2: Create the CSS file**

```css
.classification-page {
    padding: 1.5rem;
    display: flex;
    flex-direction: column;
    gap: 2rem;
}

.section-title {
    font-size: 1rem;
    font-weight: 600;
    margin-bottom: 0.75rem;
    color: var(--text-primary, #222);
}

.live-card {
    background: var(--card-bg, #f9f9f9);
    border: 1px solid var(--border-color, #e0e0e0);
    border-radius: 8px;
    padding: 1rem;
    display: flex;
    flex-direction: column;
    gap: 0.375rem;
}

.live-app { font-weight: 600; }
.live-title { color: var(--text-muted, #555); font-size: 0.875rem; }
.live-assignment { font-size: 0.8125rem; color: var(--accent, #0070f3); }

.live-meta {
    display: flex;
    align-items: center;
    gap: 0.75rem;
    font-size: 0.8125rem;
}

.confidence-bar-wrap {
    width: 80px;
    height: 6px;
    background: var(--bg-subtle, #eee);
    border-radius: 3px;
    overflow: hidden;
}

.confidence-bar {
    display: block;
    height: 100%;
    background: var(--accent, #0070f3);
    border-radius: 3px;
}

.source-badge, .outcome-badge {
    font-size: 0.75rem;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    background: var(--bg-subtle, #eee);
}

.outcome-badge.outcome-auto { background: #d4edda; color: #155724; }
.outcome-badge.outcome-pending { background: #fff3cd; color: #856404; }
.outcome-badge.outcome-unclassified { background: #f8d7da; color: #721c24; }
.outcome-badge.outcome-user_confirmed { background: #cce5ff; color: #004085; }

.history-table {
    width: 100%;
    border-collapse: collapse;
    font-size: 0.875rem;
}

.history-table th {
    text-align: left;
    padding: 0.5rem 0.75rem;
    border-bottom: 2px solid var(--border-color, #e0e0e0);
    color: var(--text-muted, #555);
    font-weight: 600;
}

.history-table td {
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--border-color, #f0f0f0);
}

.history-row:hover td { background: var(--bg-hover, #f5f5f5); }
.history-row.correcting td { background: var(--bg-selected, #eef4ff); }

.btn-correct {
    background: none;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 4px;
    padding: 0.2rem 0.5rem;
    cursor: pointer;
    font-size: 0.75rem;
}

.correction-form {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0.5rem;
    background: var(--bg-selected, #eef4ff);
}

.correction-form input {
    flex: 1;
    padding: 0.3rem 0.5rem;
    border: 1px solid var(--border-color, #ccc);
    border-radius: 4px;
    font-size: 0.875rem;
}

.empty-state {
    color: var(--text-muted, #888);
    font-size: 0.875rem;
}

.pagination {
    display: flex;
    align-items: center;
    gap: 1rem;
    margin-top: 1rem;
    font-size: 0.875rem;
}
```

- [ ] **Step 3: Build frontend**

```powershell
dotnet build src/Tracey.App
```

Expected: builds without errors.

- [ ] **Step 4: Commit**

```powershell
git add src/Tracey.App/Pages/Classification.razor `
        src/Tracey.App/Pages/Classification.razor.css
git commit -m "feat(frontend): add Classification page with live panel and paginated history"
```

---

### Task 4: Add Classification nav link

**Files:**
- Modify: `src/Tracey.App/Layout/NavMenu.razor`

- [ ] **Step 1: Add the nav link**

In `src/Tracey.App/Layout/NavMenu.razor`, add the new nav item after "Timeline":

```razor
<div class="nav-item px-3">
    <NavLink class="nav-link" href="classification">Classification</NavLink>
</div>
```

The full nav section should now read:

```razor
<div class="nav-item px-3">
    <NavLink class="nav-link" href="" Match="NavLinkMatch.All">Dashboard</NavLink>
</div>
<div class="nav-item px-3">
    <NavLink class="nav-link" href="timeline">Timeline</NavLink>
</div>
<div class="nav-item px-3">
    <NavLink class="nav-link" href="classification">Classification</NavLink>
</div>
<div class="nav-item px-3">
    <NavLink class="nav-link" href="projects">Projects</NavLink>
</div>
<div class="nav-item px-3">
    <NavLink class="nav-link" href="tags">Tags</NavLink>
</div>
<div class="nav-item px-3">
    <NavLink class="nav-link" href="settings">Settings</NavLink>
</div>
```

- [ ] **Step 2: Build frontend**

```powershell
dotnet build src/Tracey.App
```

Expected: builds without errors.

- [ ] **Step 3: Commit**

```powershell
git add src/Tracey.App/Layout/NavMenu.razor
git commit -m "feat(nav): add Classification page link to nav menu"
```

---

### Task 5: Add "auto" source badge to time entries on the Dashboard

The spec requires: "Auto-created entries display a subtle 'auto' badge so users can always distinguish them from manually created entries."

**Files:**
- Modify: `src/Tracey.App/Services/TauriIpcService.cs` (add `source` to `TimeEntryItem`)
- Modify: `src/Tracey.App/Components/TimeEntryList.razor` (render the badge)
- Modify: `src-tauri/src/commands/timer.rs` (include `source` in time_entry_list SELECT)

- [ ] **Step 1: Add `source` to the Rust `time_entry_list` response**

In `src-tauri/src/commands/timer.rs`, find the `time_entry_list` SELECT query and add `source` to the column list and the row mapping:

```rust
// Add source to SELECT:
"SELECT id, description, started_at, ended_at, project_id, task_id, \
        is_break, device_id, created_at, modified_at, source \
 FROM time_entries ..."

// Add to row mapping:
source: row.get(10)?
```

Also update the response struct if there is one (or the inline serialization) to include `source`.

- [ ] **Step 2: Build Rust to verify**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 3: Add `Source` to `TimeEntryItem` in `TauriIpcService.cs`**

Locate the `TimeEntryItem` record (or similar response type) returned by `TimeEntryListAsync` in `TauriIpcService.cs` and add:

```csharp
[property: JsonPropertyName("source")] string Source  // "manual" | "auto" | "continued"
```

- [ ] **Step 4: Add auto badge to `TimeEntryList.razor`**

Find where time entries are rendered in `TimeEntryList.razor`. In the entry row, add a conditional badge after the entry description:

```razor
@if (entry.Source == "auto")
{
    <span class="auto-badge" title="Created automatically by classification">auto</span>
}
```

Add the CSS for the badge in `TimeEntryList.razor.css`:

```css
.auto-badge {
    font-size: 0.7rem;
    background: var(--bg-subtle, #eee);
    color: var(--text-muted, #888);
    padding: 0.1rem 0.35rem;
    border-radius: 3px;
    margin-left: 0.4rem;
    vertical-align: middle;
}
```

- [ ] **Step 5: Build frontend**

```powershell
dotnet build src/Tracey.App
```

Expected: builds without errors.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/commands/timer.rs `
        src/Tracey.App/Services/TauriIpcService.cs `
        src/Tracey.App/Components/TimeEntryList.razor `
        src/Tracey.App/Components/TimeEntryList.razor.css
git commit -m "feat(timeline): add auto source badge to auto-classified time entries"
```
