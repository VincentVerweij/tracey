# Manual check: the Timeline main bar and overview

The main bar draws presence spans and picks captures by time (#124, rules from #108, #113 and #120). The overview bar below it pans and resizes the zoom range (#125, gestures from #107). The rules live in `Tracey.App/TimelineModel` and are unit-tested; this checklist covers what only the running app shows: a real lock, the live edge, the mouse and the drag. E2E can't run in CI (no Tauri there), so run it whenever `Timeline.razor` or the model changes.

## Setup

Build and start Tracey with `cargo tauri dev` from `src-tauri`, use the desktop for a few minutes so captures exist, and open **Timeline** on today.

## Checks

- [ ] **Spans.** The bar is green where you were active, red where you were away, and uncolored before Tracey started and after now. There are no dots.
- [ ] **Lock.** Press **Win+L**, wait about 30 s and unlock. A red span appears as soon as you're back, without waiting for the next tick (`presence-changed`).
- [ ] **Live edge.** Leave the page open on today. The right edge of the colored area moves every ~30 s. The zoom range does not follow it.
- [ ] **Hover in a locked span.** The dashed line's label reads `HH:MM · Away`, and the preview is the card `HH:MM · Away (locked) — no screenshots`, with no image.
- [ ] **Hover in an idle span.** Go idle past the threshold to get one. The label reads `HH:MM · Away`, and the preview shows the latest capture at or before the cursor.
- [ ] **Hover before Tracey started.** The label reads `HH:MM · Not tracked`, and the preview is the `HH:MM · Not tracked` card.
- [ ] **Pin.** Click a green stretch: the hovered capture is pinned, and a solid accent line sits at the capture's own time, not where you clicked.
- [ ] **Unpin.** Click again where the same capture is picked: the pin is gone.
- [ ] **Clear in a gap.** Pin something, then click a red locked stretch, an uncolored one, or a green stretch before the day's first screenshot: the pin is cleared.
- [ ] **Double-click.** With a capture pinned and the bar zoomed in, double-click anywhere: the zoom returns to the full day and the pin is unchanged. Repeat with nothing pinned: nothing becomes pinned.
- [ ] **Zoom.** The wheel zooms around the cursor, no further in than 15 min. The header shows `HH:MM–HH:MM` and **↺ full day**, which is hidden at full day. The activity-list badge shows the same range, and the list is filtered to it. Hour labels stay readable at every zoom level.
- [ ] **Overview.** Under the main bar, a 26 px mini-map of the whole day shows the same colors, ticks every 3 h labelled 00, 06, 12 and 18, and an accent rectangle with grips on both edges over the visible range. The rest of the day is dimmed.
- [ ] **Pan.** Drag the rectangle's body: the main bar, its labels, the range text and the activity-list badge follow during the drag. The list updates on release. The cursor is a hand, closed while dragging.
- [ ] **Resize.** Drag each edge: only that side moves, with an ↔ cursor. Dragging an edge past the other stops at 15 min.
- [ ] **Click to centre.** Press on the overview outside the rectangle: the range centres there, and keeping the button down and moving pans it.
- [ ] **Narrow.** Zoom to 15 min. The rectangle can still be panned by its body, and its edges are resized from the grab areas just outside it; a press there never centres.
- [ ] **Overview wheel.** The wheel over the overview zooms ×1.5 around the rectangle's centre; over the main bar it still zooms around the cursor.
- [ ] **Pin tick.** Pin a capture, then move the range away from it: a thin accent tick still marks it on the overview.
- [ ] **Live edge mid-drag.** On today, hold a drag across a ~30 s tick: the drag doesn't stutter, and the edge catches up on a later tick.
- [ ] **Rows.** Clicking an activity row still pins its capture, and ✕ still clears the pin.
- [ ] **Focus order.** From the date filter, Tab goes: Clean up → ↺ full day (only when zoomed) → main bar → overview → preview ✕ (when pinned) → activity list. Both bars show an accent focus ring.
- [ ] **Cursor keys.** Tab to the main bar: the dashed line appears on the pinned capture, else on the first capture in view. ←/→ step through captures, Shift+←/→ jump between span edges, and at the last one the cursor stays put. Stepping out of view pans the range just enough. Tab away: the line and label go and the pin stays.
- [ ] **Shared cursor.** Hover the bar, click, and press →: the cursor continues from where the mouse was.
- [ ] **Keyboard pin.** Enter or Space pins the capture under the cursor, again unpins it, and in a red locked or uncolored stretch clears the pin. Escape clears it.
- [ ] **Overview keys.** On the overview: ←/→ and ↓/↑ pan 15 min, Shift+arrows and Page Up/Down pan 1 h, Home/End go to 00:00/24:00, + and − zoom around the centre, 0 shows the full day. The page doesn't scroll.
- [ ] **↺ focus.** Zoom in, Tab to ↺ full day and press Enter: focus lands on the main bar.
- [ ] **Screen reader.** With Narrator (or NVDA): the main bar reads as a slider with `10:31 · Present · <window title>`, `… · Away (locked), no screenshots` or `… · Not tracked`, and after Shift+arrow `…, until HH:MM`, plus its key hint. The overview reads `HH:MM–HH:MM` on each key. Pinning by key, click, activity row or ✕ says `Pinned screenshot HH:MM` / `Pin cleared`. Dragging, the wheel and follow-pans announce nothing else.
- [ ] **Daylight saving.** On the day the clocks change, hour labels, the range text and the hover time match the preview's capture times after the change too.
- [ ] **Empty day.** Pick a date with neither spans nor screenshots: "No screenshots for this day." is shown. A past day with screenshots but no spans shows an uncolored bar.
