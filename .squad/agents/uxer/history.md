# Project Context

- **Owner:** Vincent Verweij
- **Project:** Tracey — Window Activity Timetracking Tool
- **Stack:** Tauri 2.0 (Rust) + Blazor WebAssembly .NET 10 (C#, WebView2) + BlazorBlueprint.Components + SQLite
- **Created:** 2026-03-16

## Learnings

<!-- Append new learnings below. Each entry is something lasting about the project. -->

## Phase 5.5 — UI Design Pass (2026-03-16)

**What was fixed:**
- `.tracey-shell` lacked `display: flex` — sidebar and main were stacking vertically. Fixed in `MainLayout.razor.css` with full rewrite: flex layout, 240px sidebar with `#1a1a2e` deep navy background, `flex: 1` main-content.
- Old Bootstrap Blazor scaffold CSS (`top-row`, `.page`, etc.) removed entirely from `MainLayout.razor.css`.
- `NavMenu.razor` left untouched (dead but harmless).
- `app.css` now opens with all CSS custom properties (`--tracey-accent`, `--tracey-sidebar-bg`, etc.) and updates font-family to Inter.
- Inter (Google Fonts) added to `index.html` via preconnect + link tags.
- BlazorBlueprint CSS link added to `index.html` before Bootstrap so BB Tailwind layer is available.
- `AddBlazorBlueprintComponents()` added to `Program.cs`; `@using BlazorBlueprint.Components` + `@using BlazorBlueprint.Primitives` added to `_Imports.razor`.
- `BbPortalHost` + `BbDialogProvider` added to `MainLayout.razor` (required for BbDialog and overlay components).

**BlazorBlueprint components used:**
- `BbButton` (Variant: Default/Outline/Ghost/Destructive, Size: Small) — replaces raw buttons in QuickEntryBar, TimeEntryList, Projects, IdleReturnModal forms.
- `BbCard` — wraps each client in Projects.razor, providing shadcn-style bordered box.
- `BbAlert` + `BbAlertTitle` + `BbAlertDescription` (Variant: Danger) — replaces raw `div[role=alert]` in Projects.
- `BbDialog` + `BbDialogContent` + `BbDialogHeader` + `BbDialogTitle` + `BbDialogDescription` — replaces raw `<dialog>` in `IdleReturnModal.razor`.

**New scoped CSS files created:**
- `QuickEntryBar.razor.css` — spotlight-style entry bar with focus ring, elapsed display, autocomplete dropdown.
- `TimeEntryList.razor.css` — small-caps date headers, flex rows, monospace running timer, clean edit form.
- `Projects.razor.css` — card layout, rotating chevron (CSS transform), yellow inline-confirm warning, add-form styling.
- `Dashboard.razor.css` — max-width container, header with date subtitle.
- `IdleReturnModal.razor.css` — 2x2 grid idle option cards, hover accent border, specify form.

**Key design decisions:**
- Accent color `#6366f1` (indigo) — works on both dark sidebar and light content area.
- Sidebar brand in white 700w, nav links use `::deep` to pierce Blazor scoped boundary for NavLink `.active` class.
- `BbCard Class=""` — component parameters don't support inline Razor interpolation; must use `@($"...")` syntax.
- `BbPortalHost` generates warning RZ10012 due to package targeting net8.0 vs project's net10.0; build succeeds (0 errors). False positive — components resolve correctly at runtime.
- Idle option buttons kept as raw `<button>` (not BbButton) — they need card-like multi-line layout that BbButton's Tailwind classes would conflict with.
- BlazorBlueprint CSS loaded before Bootstrap in index.html so BB Tailwind reset is available, Bootstrap then overrides where needed, finally app.css and scoped CSS win.
- Stub pages (Tags, Timeline, Settings) given emoji empty states + descriptive text; PageTitle tags added.

## Phase 6 — Timeline Design Pass (2026-03-17)

**What was delivered:**
- `Timeline.razor.css` — full scoped CSS for the screenshot timeline page.
- `Timeline.razor` — stub replaced with proper HTML scaffold; `@implements IDisposable` and stub `@code {}` added so Root can wire C# logic without touching HTML.

## Feature 7 — Horizontal 24h Timeline CSS (2026-03-17)

**What was delivered:**
- `Timeline.razor.css` — complete rewrite for the 24h horizontal timeline UI.
- Old card-grid classes (`.screenshot-grid`, `.screenshot-item`, `.screenshot-meta`, etc.) fully removed.
- New classes: `.timeline-day-bar`, `.timeline-bar-inner` — full-width dark gradient track (80px height, crosshair cursor).
- `.timeline-hour-marker` + `.hour-label` — 24 absolute-positioned tick lines; nth-child rule hides crowded labels (shows every 6th = hours 0, 6, 12, 18).
- `.timeline-screenshot-dot` + `.timeline-dot-selected` — indigo circles absolutely positioned by left%, scale + glow on hover/selected, focus-visible ring for keyboard nav.
- `.timeline-hover-indicator` + `.hover-time-label` — 1px vertical hairline with monospace time bubble follows mouse; z-index 4 floats above dots.
- `.timeline-preview-area`, `.preview-header`, `.preview-header-hover`, `.preview-close`, `.preview-time`, `.preview-trigger`, `.preview-process`, `.preview-title-text`, `.preview-placeholder`, `.screenshot-img`, `.screenshot-img-hover` — full preview panel below the bar.
- `Projects.razor` error banner already uses `BbAlert` from Phase 5.5 — no change needed.
- Build: 0 errors (3 pre-existing warnings: RZ10012 BbPortalHost + 2 CS0649 stubs Root will fill).

**CSS design decisions:**
- Screenshot grid: `auto-fill` with `minmax(280px, 1fr)` — adapts from 1 column (narrow) to 3+ columns (wide) without media queries.
- `.screenshot-item.selected` — indigo border + 15% opacity ring (same accent hue) for clear but not garish selection state.
- `.screenshot-time` — monospace font stack, accent colour, 600w — visually anchors the card timestamp.
- `.screenshot-trigger` — pill badge (9999px radius), surface-alt background — distinguishes metadata from content without heavy colour.
- `.screenshot-preview` — `position: sticky; bottom: 1.5rem` — preview floats at viewport bottom while user scrolls the grid above.
- Error banner — custom hand-rolled `div.timeline-error-banner` (not BbAlert) — tighter left-accent-border style matching the design spec layout (dismiss button inline right).
- All muted colours reference `var(--tracey-text-muted)` and `var(--tracey-surface-alt)` — no hard-coded hex fallbacks needed since tokens are defined in app.css.

**Accessibility notes:**
- `.screenshot-item:focus-visible` — explicit 2px indigo outline at 2px offset; `outline: none` on base rule removes browser default only (CHK038 coverage).
- Grid container `role="list"`, items `role="listitem"` — semantic list for screen readers (CHK039).
- Error banner `role="alert"` — announced immediately on appearance.
- Empty state emoji is `aria-hidden="true"` per Phase 5.5 pattern.
- Date input has explicit `aria-label`; clean-up button has `AriaLabel` prop.

---

### 2026-03-17: Cross-Agent Note (from Shaw T042) — Selector Contracts UXer HTML Must Honour

Shaw's T042 tests require at least one matching selector from each group on UXer-owned Timeline.razor HTML:

| Element | Required selectors (at least one) |
|---|---|
| Empty state | `.empty-state-illustration`, `[data-testid="empty-state"]` |
| Screenshot item | `[data-testid="screenshot-item"]`, `[data-testid="screenshot-card"]` |
| Timestamp | `[data-testid="screenshot-timestamp"]`, `[class*="timestamp"]`, `[class*="time"]` |
| Process name | `[data-testid="process-name"]`, `[class*="process"]` |
| Window title | `[data-testid="window-title"]`, `[class*="window-title"]`, `[class*="title"]` |
| Trigger badge | `[data-testid="trigger-badge"]`, `[class*="trigger"]`, `[class*="badge"]` |
| Preview image | `img[src]`, `role="img"`, `[data-testid="screenshot-preview"]` |
| Error banner | `role="alert"` (already present in scaffold) |
| Dismiss button | `role="button"` name `/close\|dismiss\|×\|✕/i` or `aria-label*="close"`/`"dismiss"` |

Current scaffold CSS classes (`.screenshot-time`, `.screenshot-trigger`, `.empty-state-illustration`) already cover most groups. No `data-testid` attributes are required if class-based selectors match — but adding them is safe and reduces Shaw selector fragility.
