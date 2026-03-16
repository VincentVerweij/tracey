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
