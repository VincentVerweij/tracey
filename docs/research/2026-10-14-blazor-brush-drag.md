# Drag/resize in Blazor WASM: pointer capture, interop cost, and brush reuse

Research for [#109](https://github.com/VincentVerweij/tracey/issues/109), part of map [#104](https://github.com/VincentVerweij/tracey/issues/104).
Date: 2026-10-14. Plan-only: no production code was touched.

**Question.** The Timeline zoom drag runs every `mousemove` through C#
([`Timeline.razor`](../../src/Tracey.App/Pages/Timeline.razor), `HandleBarMouseMove` →
`HandleZoomRangeDragAsync`), awaits a JS interop call per move
(`traceyBridge.getTimelineBarWidth` in
[`tauri-bridge.js`](../../src/Tracey.App/wwwroot/js/tauri-bridge.js)), and ends the drag on
`@onmouseleave`. The planned overview bar has a brush: drag the body to pan, drag either edge to
resize. Where should that drag live, what does the per-move path cost, and is there a component to
reuse?

**Source policy.** Each claim is labelled:

- **Documented**: Microsoft Learn (sourced from `dotnet/AspNetCore.Docs`), MDN, the W3C Pointer
  Events spec, or Microsoft Edge/WebView2 docs.
- **Source code**: read in the owning repository at a named ref.
- **Inference**: my reasoning from the above. Not a fact about the platform.
- **GAP**: looked for, not found.

---

## Recommendation

**Run the brush drag in a small JS module that owns pointer capture and the live visual, and talk
to C# only at drag start and drag end (plus, optionally, a `requestAnimationFrame`-throttled
progress callback).**

1. **Pointer capture fixes the "drag dies on leave" bug, and Blazor can receive captured events.**
   But calling `setPointerCapture` from a C# handler is an async round-trip after `pointerdown`,
   so do it in JS inside the `pointerdown` listener. (Section 1.)
2. **Don't route every `pointermove` through .NET.** Microsoft's own guidance for
   `onmousemove` is to register a throttled JS listener that calls a `[JSInvokable]` method, and
   to suppress re-renders that aren't needed. Today's code pays an event dispatch, an awaited
   interop call and a full `Timeline` re-render per move. (Section 2.)
3. **Hand-roll it (~100 lines of JS) instead of adopting d3-brush.** d3-brush is SVG-only,
   uses mouse/touch events rather than pointer events, pulls in five d3 modules, and Tracey has no
   npm/bundler pipeline. `BbRangeSlider` (already shipped in BlazorBlueprint 3.5.2) has no
   body-drag/pan, so it can't be the brush. Its JS is a useful pattern to copy, though.
   (Section 3.)
4. **Keep JS from mutating attributes that Blazor renders.** Have JS write CSS custom properties
   (or `transform`) on an element whose `style` Blazor never renders, and commit the final range
   back to C#, which re-renders once.

Sketch of the contract (inference, for the implementing ticket to refine):

```
C#  → JS  initBrush(trackEl, dotNetRef, {start, end, min, max, minSpan})   // OnAfterRenderAsync(firstRender)
C#  → JS  setBrush(trackEl, start, end)                                     // when wheel-zoom / reset changes the range
JS  → C#  [JSInvokable] OnBrushPreview(start, end)   // optional, rAF-throttled, no heavy re-render
JS  → C#  [JSInvokable] OnBrushCommit(start, end)    // pointerup: C# sets _viewStartHours/_zoomHours, renders once
C#  → JS  disposeBrush(trackEl)                                              // DisposeAsync
```

---

## 1. Pointer events and pointer capture from Blazor

**Blazor exposes all pointer events, including the capture events.** *(Documented + source code)*
The event-handling article lists `PointerEventArgs` for "Mouse pointer" events
([Learn: Built-in event arguments](https://learn.microsoft.com/en-us/aspnet/core/blazor/components/event-handling?view=aspnetcore-10.0#built-in-event-arguments)).
`EventHandlers.cs` on `release/10.0` registers `ongotpointercapture`, `onlostpointercapture`,
`onpointercancel`, `onpointerdown`, `onpointermove`, `onpointerup` (and enter/leave/out/over) with
`PointerEventArgs`
([source](https://github.com/dotnet/aspnetcore/blob/release/10.0/src/Components/Web/src/Web/EventHandlers.cs#L70-L80)),
and `PointerEventArgs` carries `PointerId`, `PointerType` and `IsPrimary`
([source](https://github.com/dotnet/aspnetcore/blob/release/10.0/src/Components/Web/src/Web/PointerEventArgs.cs)).

**Blazor has no `setPointerCapture` API. It has to be called through JS.** *(Source code; GAP in
docs)* Nothing in `Microsoft.AspNetCore.Components.Web` wraps it, and the docs don't mention
pointer capture. `ElementReference` can be passed to JS to call it
([Learn: Capture references to elements](https://learn.microsoft.com/en-us/aspnet/core/blazor/javascript-interoperability/call-javascript-from-dotnet?view=aspnetcore-10.0#capture-references-to-elements)).

**What capture does.** *(Documented)* `setPointerCapture` makes the element the target of later
events for that pointer "even when the pointer moves outside of its boundaries". Capture is
released explicitly or implicitly after `pointerup`/`pointercancel`
([MDN: setPointerCapture](https://developer.mozilla.org/en-US/docs/Web/API/Element/setPointerCapture);
[W3C Pointer Events: Implicit release](https://w3c.github.io/pointerevents/#implicit-release-of-pointer-capture)).
It is Baseline "widely available" since July 2020 (MDN), and WebView2 renders with Microsoft
Edge/Chromium on the Evergreen runtime
([Learn: WebView2 intro](https://learn.microsoft.com/en-us/microsoft-edge/webview2/)), so engine
support is not a concern.

**Captured events still reach `@onpointermove` handlers.** *(Source code + inference)* Blazor's
`EventDelegator` attaches one listener per event name on `document` and walks
`event.composedPath()` from the target, looking for elements with registered .NET handlers
([source](https://github.com/dotnet/aspnetcore/blob/release/10.0/src/Components/Web.JS/src/Rendering/Events/EventDelegator.ts)).
Capture retargets the event to the capturing element, which then bubbles to `document`. So an
`@onpointermove` on the capturing element fires even when the cursor is outside it. BlazorBlueprint
relies on exactly this setup (see §3).

**The catch: the C# handler is asynchronous to the browser event.** *(Source code + documented)*
The delegator dispatches to .NET via `invokeMethodAsync('DispatchEventAsync', …)`
([WebRendererInteropMethods.ts](https://github.com/dotnet/aspnetcore/blob/release/10.0/src/Components/Web.JS/src/Rendering/WebRendererInteropMethods.ts#L66-L71)),
and a C# → JS `setPointerCapture` call is another async hop. Per the spec, `setPointerCapture`
throws `NotFoundError` if the pointerId isn't an active pointer, and *silently does nothing* if
the pointer "is not in the active buttons state"
([W3C: Setting pointer capture](https://w3c.github.io/pointerevents/#setting-pointer-capture)).
Consequences (inference):
- Moves that happen between `pointerdown` and the moment capture lands are not captured. A fast
  flick off a 6 px edge handle can escape.
- A quick click (released before the hop completes) produces either a no-op or an exception that
  C# must swallow.
- `preventDefault` can't be decided in C# at runtime. Only the static
  `@onpointerdown:preventDefault` works, because the browser event has already returned
  ([Learn: Prevent default actions](https://learn.microsoft.com/en-us/aspnet/core/blazor/components/event-handling?view=aspnetcore-10.0#prevent-default-actions)).

Calling `setPointerCapture(e.pointerId)` synchronously inside a JS `pointerdown` listener avoids
all three.

**Also needed for pen/touch on Windows:** `touch-action: none` on the brush and handles. Otherwise
the browser can take the gesture for panning and send `pointercancel`
([MDN: touch-action](https://developer.mozilla.org/en-US/docs/Web/CSS/touch-action)). Handle
`pointercancel`/`lostpointercapture` as "abort or commit" so the brush can't get stuck mid-drag.
*(Documented + inference)*

**Answer to Q1.** Blazor *can* participate: `@onpointer*` handlers work, and capture set via JS
keeps events flowing outside the element. But capture must be set synchronously in JS, and per-move
work belongs in JS anyway (Q2). So the drag should live in JS.

## 2. Cost of per-move interop and re-render in Blazor WASM

**No documented latency number.** *(GAP)* Neither the JS-interop performance article nor the
rendering article gives per-call microseconds or frame budgets for WebAssembly. What Microsoft does
document is qualitative:

- *JS interop overhead.* "Calls between .NET and JavaScript require additional overhead because:
  Calls are asynchronous. Parameters and return values are JSON-serialized…" For WebAssembly,
  batching "usually only improves performance significantly if the component makes a large number
  of JS interop calls"
  ([Learn: JS interop performance](https://learn.microsoft.com/en-us/aspnet/core/blazor/performance/javascript-interoperability?view=aspnetcore-10.0)).
  A per-`pointermove` call is that large number.
- *Synchronous interop on WASM.* Casting to `IJSInProcessRuntime` "has slightly less overhead than
  making asynchronous calls and can result in fewer render cycles because there's no intermediate
  state while awaiting results". JS → .NET can use `DotNet.invokeMethod`
  ([same article, "Consider the use of synchronous calls"](https://learn.microsoft.com/en-us/aspnet/core/blazor/performance/javascript-interoperability?view=aspnetcore-10.0#consider-the-use-of-synchronous-calls)).
  Today's `await GetTimelineBarWidthAsync()` is an async hop. Inside `HandleBarMouseMove` it also
  produces an extra render: `ComponentBase` renders once after the handler's synchronous part and
  again when the returned task completes
  ([Learn: Component rendering §An asynchronous handler involves multiple asynchronous phases](https://learn.microsoft.com/en-us/aspnet/core/blazor/components/rendering?view=aspnetcore-10.0#an-asynchronous-handler-involves-multiple-asynchronous-phases)).
  *(Documented + inference)*
- *Rapid input specifically.* The (older, still-published) unmarshalled-interop section warns that
  "poor performance may result when JS interop calls are made on the basis of moving an input
  device, such as spinning a mouse wheel"
  ([Learn: call JS from .NET, .NET 5–6 moniker](https://learn.microsoft.com/en-us/aspnet/core/blazor/javascript-interoperability/call-javascript-from-dotnet?view=aspnetcore-6.0#unmarshalled-javascript-interop)).
  Its replacement on .NET 7+ is `[JSImport]`/`[JSExport]`, which "offers improved performance and
  stability". `IJSRuntime` remains "the recommended approach"
  ([Learn: JSImport/JSExport](https://learn.microsoft.com/en-us/aspnet/core/blazor/javascript-interoperability/import-export-interop?view=aspnetcore-10.0)).
  It also requires `AllowUnsafeBlocks`, which isn't worth it for two calls per drag. *(Documented +
  inference)*

**Microsoft's recommended pattern for exactly this.** *(Documented)* "Don't trigger events too
rapidly": "`onmousemove` and `onscroll` can fire tens or hundreds of times per second… If events
are triggered too rapidly, you may harm UI responsiveness or consume excessive CPU time. Rather
than use native events that rapidly fire, consider the use of JS interop to register a callback
that fires less frequently". The example registers a JS `mousemove` listener that calls a
`[JSInvokable]` method via a `DotNetObjectReference`, throttled with Lodash
([Learn: Rendering performance §Don't trigger events too rapidly](https://learn.microsoft.com/en-us/aspnet/core/blazor/performance/rendering?view=aspnetcore-10.0#dont-trigger-events-too-rapidly)).
For a visual drag, `requestAnimationFrame` is the natural throttle: its callback runs "before the
next repaint" and generally at display refresh rate
([MDN: requestAnimationFrame](https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame)).
Browsers may also coalesce pointer moves themselves
([W3C: Coalesced events](https://w3c.github.io/pointerevents/#coalesced-events)). *(Documented)*

**Re-render suppression.** *(Documented)* `ComponentBase` "automatically invokes
`StateHasChanged` after the component's event handlers are invoked". Use `IHandleEvent` to opt out,
or `ShouldRender` overrides in children, to avoid unneeded re-renders
([Learn: §Avoid rerendering after handling events without state changes](https://learn.microsoft.com/en-us/aspnet/core/blazor/performance/rendering?view=aspnetcore-10.0#avoid-rerendering-after-handling-events-without-state-changes)).
In Tracey, a re-render of `Timeline` re-runs the hour-marker loop and everything else in the
page. *(Inference)* If a live preview of the main bar is wanted during drag, isolate the parts that
depend on the range into child components and let the rest return `false` from `ShouldRender`. Or
don't preview in C# at all and commit on `pointerup`.

**DOM ownership.** *(Documented)* "Only mutate the DOM with JavaScript (JS) when the object
doesn't interact with Blazor… If an element rendered by Blazor is modified externally… the DOM may
no longer match Blazor's internal representation, which can result in undefined behavior"
([Learn: JS interop §Interaction with the DOM](https://learn.microsoft.com/en-us/aspnet/core/blazor/javascript-interoperability/?view=aspnetcore-10.0#interaction-with-the-dom)).
The brush moves visually in JS during drag, so (inference) either:
- let Blazor render the track and brush **without** a `style` attribute, and have JS set
  `--brush-start`/`--brush-end` custom properties that the CSS uses; on commit, C# pushes the
  value back with `setBrush`; or
- render an empty container from Blazor and let JS build the brush inside it.

The first is simpler and keeps markup and `aria-*` in Razor.

**Answer to Q2.** There's no published millisecond figure. The documented guidance is
unambiguous, though: don't send high-frequency pointer events through .NET. Handle them in JS,
call back through `[JSInvokable]` at a reduced rate (rAF or commit-only), prefer sync interop on
WASM for any remaining calls, and suppress needless re-renders with `IHandleEvent`/`ShouldRender`.

## 3. Reuse vs hand-roll

**Repo posture.** *(Source code)* `package.json` doesn't exist and `package-lock.json` has an
empty `packages` map. There's no npm or bundler step. JS is a single hand-written
`wwwroot/js/tauri-bridge.js` loaded by a `<script>` tag. The only vendored lib is Bootstrap. The
C# UI dependency is `BlazorBlueprint.Components` 3.5.2 (Apache-2.0). `CODING_STANDARDS.md` rule 15
requires any new dependency to be pinned.

**d3-brush.** *(Source code)* ISC. Last release v3.0.0 on 2021-06-10, last push 2023-10, not
archived ([repo](https://github.com/d3/d3-brush)). It depends on `d3-dispatch`, `d3-drag`,
`d3-interpolate`, `d3-selection` and `d3-transition`. It renders SVG (`brushX` for 1-D), listens to
`mousedown`/`mousemove`/`mouseup` and `touch*` rather than pointer events, and owns the DOM it
draws into ([src/brush.js](https://github.com/d3/d3-brush/blob/main/src/brush.js)). For Tracey
(inference) that would mean vendoring a d3 bundle, switching the overview bar to SVG, and giving
d3 a Blazor-empty container per the DOM-ownership rule. That's a lot of weight for a 1-D,
two-handle, single-pointer brush, and it still needs the same C# bridge. Not recommended.

**BlazorBlueprint `BbRangeSlider`** (already in the dependency tree). *(Source code)*
[`BbRangeSlider.razor`](https://github.com/blazorblueprintui/ui/blob/e347f4d4e9e764ecbca6e0a21450c66e34339d43/src/BlazorBlueprint.Components/Components/RangeSlider/BbRangeSlider.razor)
is a dual-thumb slider. Thumbs use `@onpointerdown` → C# → JS `startDrag(id, thumb, pointerId)`,
which calls `trackElement.setPointerCapture(pointerId)`. The JS module
(`_content/BlazorBlueprint.Components/js/range-slider.js`) then calls
`dotNetRef.invokeMethodAsync('UpdateValueFromPercentage', …)` on **every** `pointermove`, without
throttling. It has thumb drag and track click, but no body-drag to pan a fixed-width window. So:
- it can't be the brush without forking (no pan);
- it confirms that the C# → JS `setPointerCapture` route works in practice, and also shows its
  costs: an async capture after `pointerdown`, and an unthrottled .NET call per move.

Its ~150 lines of JS make a good template for Tracey's own module, with three changes: set capture
in the JS `pointerdown` listener, add a body-drag mode, and rAF-throttle or commit-only the
callback.

**Other Blazor brush/range libraries.** *(GAP)* A GitHub search for Blazor range sliders found
only commercial suites (Syncfusion, etc.) and unmaintained single-author repos (e.g. an Ion.RangeSlider
wrapper, last push 2020). I found no maintained, MIT/Apache Blazor *brush* (pan + resize)
component. I did not evaluate the commercial suites; their licensing alone rules them out for a
single interaction.

**Answer to Q3.** Hand-roll a small JS module beside `tauri-bridge.js`, modeled on
BlazorBlueprint's `range-slider.js`. Don't add d3.

## Proposed JS behaviour (inference, for the implementing ticket)

- `pointerdown` on a handle or the body: `e.preventDefault()`, `el.setPointerCapture(e.pointerId)`,
  and record the mode (`resize-start` | `resize-end` | `pan`), the start clientX, the start range
  and `track.getBoundingClientRect().width`. Read the width **once** per drag, not per move. That
  replaces `getTimelineBarWidth`.
- `pointermove` (same pointerId): compute the new range in JS (clamp to [0, 24h], `minSpan`
  0.5 h to match the wheel zoom clamp), write the CSS custom properties, and schedule at most one
  rAF-throttled `OnBrushPreview` if a live preview is wanted.
- `pointerup`: `OnBrushCommit(start, end)`. `pointercancel`/`lostpointercapture` without `pointerup`:
  revert or commit (product decision).
- Distinguish click from drag with a small movement threshold, which replaces `_didDragZoomRange`.
- CSS: `touch-action: none` on the brush, `cursor: ew-resize` on handles and `grab`/`grabbing` on
  the body.
- Keyboard and ARIA: give the handles `role="slider"` with arrow-key handlers in Razor. These are
  low-frequency, so plain C# `@onkeydown` is fine. *(Inference)*

## Sources

- Learn: [Blazor event handling](https://learn.microsoft.com/en-us/aspnet/core/blazor/components/event-handling?view=aspnetcore-10.0), [Rendering performance](https://learn.microsoft.com/en-us/aspnet/core/blazor/performance/rendering?view=aspnetcore-10.0), [JS interop performance](https://learn.microsoft.com/en-us/aspnet/core/blazor/performance/javascript-interoperability?view=aspnetcore-10.0), [JS interop overview](https://learn.microsoft.com/en-us/aspnet/core/blazor/javascript-interoperability/?view=aspnetcore-10.0), [Call JS from .NET](https://learn.microsoft.com/en-us/aspnet/core/blazor/javascript-interoperability/call-javascript-from-dotnet?view=aspnetcore-10.0), [Call .NET from JS](https://learn.microsoft.com/en-us/aspnet/core/blazor/javascript-interoperability/call-dotnet-from-javascript?view=aspnetcore-10.0), [JSImport/JSExport](https://learn.microsoft.com/en-us/aspnet/core/blazor/javascript-interoperability/import-export-interop?view=aspnetcore-10.0), [Component rendering](https://learn.microsoft.com/en-us/aspnet/core/blazor/components/rendering?view=aspnetcore-10.0), [WebView2 intro](https://learn.microsoft.com/en-us/microsoft-edge/webview2/)
- dotnet/aspnetcore `release/10.0`: [EventHandlers.cs](https://github.com/dotnet/aspnetcore/blob/release/10.0/src/Components/Web/src/Web/EventHandlers.cs), [PointerEventArgs.cs](https://github.com/dotnet/aspnetcore/blob/release/10.0/src/Components/Web/src/Web/PointerEventArgs.cs), [EventDelegator.ts](https://github.com/dotnet/aspnetcore/blob/release/10.0/src/Components/Web.JS/src/Rendering/Events/EventDelegator.ts), [EventTypes.ts](https://github.com/dotnet/aspnetcore/blob/release/10.0/src/Components/Web.JS/src/Rendering/Events/EventTypes.ts), [WebRendererInteropMethods.ts](https://github.com/dotnet/aspnetcore/blob/release/10.0/src/Components/Web.JS/src/Rendering/WebRendererInteropMethods.ts)
- W3C [Pointer Events](https://w3c.github.io/pointerevents/); MDN [setPointerCapture](https://developer.mozilla.org/en-US/docs/Web/API/Element/setPointerCapture), [requestAnimationFrame](https://developer.mozilla.org/en-US/docs/Web/API/Window/requestAnimationFrame), [touch-action](https://developer.mozilla.org/en-US/docs/Web/CSS/touch-action)
- [d3/d3-brush](https://github.com/d3/d3-brush); [blazorblueprintui/ui @ e347f4d](https://github.com/blazorblueprintui/ui/tree/e347f4d4e9e764ecbca6e0a21450c66e34339d43) (the commit in the 3.5.2 nuspec)
