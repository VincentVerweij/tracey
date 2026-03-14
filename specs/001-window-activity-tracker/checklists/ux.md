# UX Requirements Quality Checklist: Window Activity Timetracking Tool

**Purpose**: Validate that UX and interaction requirements are complete, clear, consistent, and measurable before UI implementation begins. Tests the quality of requirements written in English — not whether the UI works.
**Created**: March 15, 2026
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md)
**Depth**: Thorough | **Audience**: Reviewer (pre-implementation gate) | **Focus**: Interaction completeness, state coverage, visual clarity

---

## Requirement Completeness

- [ ] CHK001 Are loading state requirements defined for all asynchronous data surfaces — the time entry list, screenshot timeline, and fuzzy-match dropdown? [Completeness, Gap]
- [ ] CHK002 Are empty-state requirements defined for the time entry list when no entries exist at all (first launch, or after a data wipe)? [Completeness, Gap]
- [ ] CHK003 Are empty-state requirements defined for the screenshot timeline when no screenshots have been captured yet? [Completeness, Gap]
- [ ] CHK004 Are empty-state requirements defined for the project/task picker when no projects exist — specifically, is a call-to-action or guidance message required? [Completeness, Gap]
- [ ] CHK005 Are system tray icon requirements defined — including available context menu actions (e.g., show/hide window, quit, show active timer)? [Completeness, Gap]
- [ ] CHK006 Are requirements defined for the running timer display — specifically its format (HH:MM:SS vs. relative), refresh rate, and its position in the UI relative to the entry list? [Completeness, Spec §FR-022]
- [ ] CHK007 Are in-app notification requirements defined for the scenario where the screenshot storage folder becomes full or inaccessible — including where and how the message appears? [Completeness, Spec §Edge Cases]
- [ ] CHK008 Are visual requirements defined for orphaned time entries (entries whose project or task was subsequently deleted) — for example, a visual indicator or fallback label? [Completeness, Gap]
- [ ] CHK009 Are the contents of the "delete all my data" confirmation dialog specified — including the warning copy, the list of what will be deleted, and the post-deletion confirmation message? [Completeness, Spec §FR-070]

---

## Requirement Clarity

- [ ] CHK010 Is "persistent input bar at the top" (FR-034) sufficiently defined — is there a fixed height, padding, or always-on-top behavior when the window is scrolled or resized? [Clarity, Spec §FR-034]
- [ ] CHK011 Is the visual appearance of the fuzzy-match dropdown specified — including maximum visible row count, scroll behavior within the dropdown, and how matched characters are highlighted? [Clarity, Spec §FR-038]
- [ ] CHK012 Is the visual distinction between the four idle-return prompt options (Break, Meeting, Specify, Keep Timer Running) specified — including layout, button hierarchy, and whether any option is the default/focused choice? [Clarity, Spec §FR-004]
- [ ] CHK013 Is the disambiguation dropdown (triggered when a project name matches multiple clients) visually specified — how it appears relative to the quick-entry bar, and how client identifiers are displayed alongside project names? [Clarity, Spec §FR-041]
- [ ] CHK014 Are segment-locking visual cues defined for the quick-entry bar — how does the UI indicate that a project or task segment has been confirmed versus still being typed? [Clarity, Spec §FR-039]
- [ ] CHK015 Is "always visible" (FR-034) sufficient to define how the quick-entry bar behaves at minimum window sizes — or does the spec need to define a minimum window width below which truncation or scrolling applies? [Clarity, Spec §FR-034]
- [ ] CHK016 Is the UX feedback mechanism for sync status defined — how and where the user sees synced/pending/failed states, the specific labels used, and whether this indicator is persistent or contextual? [Clarity, Spec §SC-008, §FR-059]
- [ ] CHK017 Is the date grouping display format for the time entry list specified — for example, whether group headers use relative dates ("Today", "Yesterday"), absolute dates, or both? [Clarity, Spec §FR-031]
- [ ] CHK018 Is the timestamp display format for screenshots in the timeline defined — including precision (HH:MM vs. HH:MM:SS) and whether the user's configured local timezone applies? [Clarity, Spec §FR-016, §FR-025]
- [ ] CHK019 Is it clear from the spec how break time entries are visually distinguished from regular work entries in the time entry list — given that `is_break` is a distinct field in the IPC output? [Clarity, Spec §FR-006, IPC contracts]

---

## Requirement Consistency

- [ ] CHK020 Are the auto-save-on-blur requirements made distinct enough to prevent misinterpretation — is it unambiguous that the in-place entry editor auto-saves on blur but the quick-entry bar requires explicit Enter? [Consistency, Spec §FR-030]
- [ ] CHK021 Are keyboard navigation requirements consistent across all dropdown types — fuzzy-match dropdown, disambiguation dropdown, tag picker, and auto-complete suggestions? [Consistency, Spec §FR-039]
- [ ] CHK022 Do the Continue button requirements (FR-024) and the auto-complete suggestion requirements (FR-029) specify the same set of copied fields — description, project, task, and tags — with no discrepancy? [Consistency, Spec §FR-024, §FR-029]
- [ ] CHK023 Are archive/unarchive visual feedback requirements consistent between clients and projects — same confirmation pattern or explicitly differentiated? [Consistency, Spec §FR-045, §FR-046]
- [ ] CHK024 Is the destructive-action confirmation dialog pattern consistent across all three occurrences: client delete (FR-047), tag delete (FR-051), and full data wipe (FR-070) — or are intentional differences between them specified? [Consistency, Spec §FR-047, §FR-051, §FR-070]

---

## Scenario Coverage

- [ ] CHK025 Are requirements defined for what happens when the user presses Escape in the quick-entry bar — does the partially-typed input clear, retain, or dismiss the dropdown while keeping the text? [Coverage, Gap]
- [ ] CHK026 Are requirements defined for the idle-return prompt's focus behavior — does the modal appear in the foreground regardless of which application the user returns to, or only when the Tracey window is focused? [Coverage, Gap]
- [ ] CHK027 Are requirements defined for the idle-return prompt's lifetime — does the modal persist indefinitely until the user responds, or is there a timeout after which it auto-dismisses? [Coverage, Gap]
- [ ] CHK028 Are requirements defined for the manual time entry overlap scenario — specifically, what the overlap warning displays and whether the user must correct or can force-confirm the overlapping entry? [Coverage, Spec §Edge Cases]
- [ ] CHK029 Are requirements defined for auto-complete suggestions when the matched historical entry's project or task has since been deleted — does the suggestion appear with a visual flag, or is it suppressed entirely? [Coverage, Spec §Edge Cases]
- [ ] CHK030 Are requirements defined for the screenshot timeline when the user scrolls to a period with no screenshots — for example, during an idle period or when the app was not running? [Coverage, Gap]
- [ ] CHK031 Are requirements defined for the settings screen when the user enters an invalid or unreachable connection URI — specifically the validation moment, error message copy, and whether the field retains or clears the invalid value? [Coverage, Gap]
- [ ] CHK032 Are requirements defined for notification channel configuration forms in the settings UI — including which fields are required, inline validation behavior, and feedback when a test notification is sent? [Completeness, Spec §FR-055, §FR-056]

---

## Edge Case Coverage

- [ ] CHK033 Are requirements defined for the quick-entry bar zero-match state — what is shown in the dropdown when the typed segment matches no existing project or task? [Edge Case, Spec §FR-037]
- [ ] CHK034 Are requirements defined for the time entry list when a timer is stopped mid-scroll — does the new completed entry appear without disrupting the user's current scroll position, or does the list jump to the top? [Edge Case, Spec §FR-033]
- [ ] CHK035 Are requirements defined for the client logo upload interaction — accepted file types, maximum file size, validation feedback, and what is displayed if the logo file is later moved or deleted? [Edge Case, Spec §FR-042]
- [ ] CHK036 Are requirements defined for the "delete all my data" operation's progress feedback — given the 60-second completion requirement (FR-070), is a progress indicator or status message required during the operation? [Edge Case, Spec §FR-070]

---

## Non-Functional UX Requirements

- [ ] CHK037 Are accessibility requirements defined for keyboard-only navigation of the idle-return prompt — including which option receives initial focus and how the user tabs through options? [Accessibility, Gap]
- [ ] CHK038 Are accessibility requirements defined for the screenshot timeline — including keyboard scrolling behavior and screen reader labels for captured images? [Accessibility, Gap]
- [ ] CHK039 Are ARIA or screen reader requirements specified for any interactive component — quick-entry bar, dropdown, timer display, or timeline? [Accessibility, Gap]
- [ ] CHK040 Is the startup UX defined for the 5-second initialization window (SC-006) — is a splash screen, skeleton layout, or loading indicator required, and what is shown if initialization exceeds 5 seconds? [Non-Functional, Spec §SC-006]

---

## Notes

- Focus: UX & interaction requirements quality only. Performance, security, and data-model quality are out of scope for this checklist.
- Depth: Thorough (40 items). Audience: pre-implementation reviewer gate.
- `[Gap]` = requirement is absent from the spec/plan and needs to be added or explicitly out-of-scoped.
- Resolve `[Gap]` items by either: (a) adding a requirement to spec.md, or (b) adding an explicit exclusion in the Assumptions section.
- Items marked `[Spec §FR-XXX]` reference existing spec sections — check that the referenced text is clear enough to implement without further guidance.
