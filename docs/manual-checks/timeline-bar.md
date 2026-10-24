# Manual check: the Timeline main bar

The main bar draws presence spans and picks captures by time (#124, rules from #108, #113 and #120). The rules live in `Tracey.App/TimelineModel` and are unit-tested; this checklist covers what only the running app shows: a real lock, the live edge and the mouse. E2E can't run in CI (no Tauri there), so run it whenever `Timeline.razor` or the model changes.

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
- [ ] **Clear in a gap.** Pin something, then click a red locked stretch or an uncolored one: the pin is cleared.
- [ ] **Double-click.** With a capture pinned and the bar zoomed in, double-click anywhere: the zoom returns to the full day and the pin is unchanged. Repeat with nothing pinned: nothing becomes pinned.
- [ ] **Zoom.** The wheel zooms around the cursor, no further in than 15 min. The header shows `HH:MM–HH:MM` and **↺ full day**, which is hidden at full day. The activity-list badge shows the same range, and the list is filtered to it. Hour labels stay readable at every zoom level.
- [ ] **Rows.** Clicking an activity row still pins its capture, and ✕ still clears the pin.
- [ ] **Empty day.** Pick a date with neither spans nor screenshots: "No screenshots for this day." is shown. A past day with screenshots but no spans shows an uncolored bar.
