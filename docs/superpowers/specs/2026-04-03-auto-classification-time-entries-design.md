# Auto-Classification of Time Entries — Design

**Date:** 2026-04-03  
**Status:** Approved  
**Topic:** Automatic time entry creation using on-device ML with active learning

---

## Problem Statement

Tracey captures rich window activity data but requires the user to manually create and classify time entries. This friction reduces adoption and completeness. The goal is to have the system automatically create time entries and assign them to Client/Project/Task using the window activity, screenshots, and OCR text — with active learning to handle low-confidence cases and continuously improve classification.

---

## Scope

- On-device classification only (no external ML APIs; data never leaves the machine except via the existing Postgres sync)
- Automatic time entry creation when confidence ≥ 70%
- Active learning (user prompt) when confidence < 70%
- A dedicated Classification page in the main nav for transparency and correction
- Screenshot pipeline upgraded to capture at full resolution for OCR, then downscale before storage
- Labeled training data synced to the external Postgres database for cross-device continuity

---

## Architecture

```
ActivityTracker (existing)
       │ window change event
       ▼
FeatureExtractor          ← window title + app name + OCR text (async, from screenshot)
       │ feature vector
       ├──► ClassificationPage (live feed of features + result)
       ▼
ClassificationService
       │ (prediction, confidence)
       ├─ confidence ≥ 70% ──► TimeEntryService.AutoCreate(entry)
       └─ confidence < 70%  ──► ActiveLearningQueue → toast in UI
                                       │ user confirms / corrects
                                       ▼
                               LabeledSampleStore (local SQLite + Postgres sync)
                                       │ when ≥ 10 new samples since last run
                                       ▼
                               ClassifierTrainer.Retrain() [background thread]
```

### New Rust components

| Component | Responsibility |
|---|---|
| `FeatureExtractor` | Extract normalized text features from window title, app name, OCR text |
| `OcrService` | Run OCR on full-resolution screenshot in background thread |
| `ClassificationService` | Phase 1 heuristics → Phase 2 TF-IDF classifier; returns `(prediction, confidence)` |
| `ClassifierTrainer` | Retrain TF-IDF model on labeled samples; serialize model to SQLite |
| `ActiveLearningQueue` | Hold low-confidence classifications pending user input; enforce snooze logic |

### New database tables (local SQLite + Postgres)

| Table | Contents |
|---|---|
| `labeled_samples` | `(id, feature_vector_json, client_id, project_id, task_id, source, device_id, created_at)` |
| `classifier_model` | `(id, model_blob, trained_at, sample_count, device_id)` — serialized TF-IDF weights |

Both tables are included in the existing sync queue and replicated to Postgres following the same offline-resilient pattern as all other entities.

---

## Screenshot Pipeline Change

**Current behavior:** Screenshot captured at 50% resolution.  
**New behavior:**
1. `ScreenshotService` captures at full resolution (Win32 GDI, in `spawn_blocking`)
2. `OcrService` receives the full-resolution image and extracts text (background thread)
3. Extracted OCR text is stored in a new `ocr_text` column on the `screenshots` table
4. The image is downscaled to the current reduced size before writing to disk

The full-resolution image is held in memory only for the duration of OCR extraction and is never written to disk. Storage footprint is unchanged.

**Privacy:** `OcrService` only receives screenshots produced by window activity that passed the existing deny-list check in `ActivityTracker`. Denied processes (password managers, etc.) never produce a screenshot and therefore never produce OCR text.

---

## Classification Engine

### Phase 1 — Heuristics (active from day one, no training data required)

A user-editable rule file maps patterns to `(client_id, project_id, task_id)`. Rules are evaluated in order; first match wins.

```
{ app: "Visual Studio Code", title_contains: "tracey" } → tracey / tracey / development
{ app: "Slack" }                                         → <unclassified>
```

Rules are configurable from the Settings page. Confidence for a matched rule: 100%. No match: 0% (triggers active learning).

### Phase 2 — TF-IDF classifier (activates after ≥ 20 labeled samples)

A bag-of-words TF-IDF model trained on concatenated `window_title + ocr_text` for each labeled sample. The target label is `(client_id, project_id, task_id)`.

- Implemented in Rust using a lightweight in-process library (no subprocess, no Python)
- Confidence: cosine similarity of the query vector to the nearest class centroid, normalized to [0, 1]
- Model is serialized to `classifier_model` in SQLite after each training run and loaded at startup
- Phase 2 activates once 20 total labeled samples exist in the database. Subsequent retrains are triggered when ≥ 10 new samples have accumulated since the last training run.

**Fallback chain:** Phase 2 classifier → Phase 1 heuristics → active learning prompt

---

## Active Learning Loop

### Trigger

When `ClassificationService` returns confidence < 70%, the current activity is enqueued in `ActiveLearningQueue` and a Tauri event is emitted to the frontend.

### Toast behavior

- Appears non-intrusively (bottom-right corner)
- Shows: detected app name, window title snippet
- Shows up to 3 ranked suggestions (Project / Task + confidence %)
- "Something else…" option opens a mini picker (free-text search over all projects/tasks)
- Auto-dismisses after 30 seconds; entry remains as "pending" in the Classification page
- Snooze: user can dismiss without answering; same pattern is not re-asked within 10 minutes
- Snooze cap: if the same pattern is dismissed 3 times, the system stops prompting for it and marks entries as "unclassified"

### Feedback storage

Each user response (selected suggestion or free-input) creates a `LabeledSample`:

```
{
  feature_vector_json: <normalized title + app + ocr text>,
  client_id, project_id, task_id,
  source: "user_confirmed" | "user_corrected",
  device_id,
  created_at
}
```

Stored locally and queued for Postgres sync. When ≥ 10 new samples accumulate since the last training run, `ClassifierTrainer.Retrain()` is triggered on a background thread.

---

## Time Entry Automation

- When confidence ≥ 70%, `TimeEntryService.AutoCreate` creates a time entry with start time = start of the current window activity segment (already tracked by `ActivityTracker`)
- Entries created this way are tagged `source: "auto"` to distinguish them from manually created entries
- **Grouping:** Consecutive windows classified to the same project/task within a 2-minute gap are merged into a single time entry (configurable from the Settings page under a new "Auto-Classification" section)
- **Corrections:** Auto-created entries can be corrected from the Timeline page or Classification page; a correction updates the entry and creates a labeled sample, feeding the training loop
- **Timeline visibility:** Auto-created entries display a subtle "auto" badge so users can always distinguish them from manually created entries

---

## Classification Page

A dedicated page in the main navigation that provides transparency into how the system is classifying activity.

### Panels

**Live panel:**
- Current active window (app + title)
- Extracted features: raw OCR text snippet, normalized feature tokens
- Predicted project/task with confidence bar
- Classification source: "heuristic" | "tf-idf" | "pending"

**History list:**
- Recent classification events (paginated)
- Columns: time, app, title snippet, predicted project/task, confidence, outcome (auto / user-confirmed / user-corrected / unclassified)
- Each row is correctable inline → adds a labeled sample

---

## Cross-Device Sync

Both `labeled_samples` and `classifier_model` are included in the existing sync pipeline:

- On device A: samples are created → queued for Postgres sync
- On device B: samples arrive via sync → if ≥ 10 new samples since last retrain, `ClassifierTrainer.Retrain()` is triggered
- Cold start on a new device: labeled samples are pulled from Postgres on first sync, classifier is retrained locally before classification begins
- Conflict resolution: `labeled_samples` uses last-write-wins on `modified_at`; `classifier_model` is always rebuilt from samples, so no merge conflict

---

## Error Handling

| Scenario | Behavior |
|---|---|
| OCR failure (image unreadable) | Feature extraction degrades gracefully — classification runs on title + app only |
| Classifier not yet trained (< 20 samples) | Falls back to Phase 1 heuristics; active learning prompt if no rule matches |
| Postgres unreachable | Labeled samples queue in local SQLite; sync resumes on reconnect (existing pattern) |
| Retrain fails | Log error, keep using the previous model; do not block the UI |
| Conflicting samples across devices | Last-write-wins; classifier retrained after sync resolves |
| Pattern dismissed 3 times | Entry recorded as "unclassified"; visible in Classification page for later manual classification |

---

## Testing

### Unit tests
- `FeatureExtractor`: text normalization, OCR text parsing, empty/null handling
- `ClassificationService`: heuristic pattern matching, TF-IDF confidence scoring
- `ActiveLearningQueue`: snooze logic, deduplication, snooze cap enforcement
- `OcrService`: valid image → extracted text, unreadable image → graceful degradation

### Integration tests
- Full pipeline: simulated window change → feature extraction → classification → labeled sample stored → retrain triggered
- Phase 1 → Phase 2 transition: ≥ 20 samples trigger classifier activation
- Auto time entry creation and grouping logic

### Frontend tests
- Toast: renders suggestions, selection creates labeled sample, "Something else…" opens picker, auto-dismiss behavior
- Classification page: live panel updates on window change, history rows are correctable

### Cross-device sync test
- Labeled sample created on device A, synced to Postgres, received and applied on device B, classifier retrained on device B
