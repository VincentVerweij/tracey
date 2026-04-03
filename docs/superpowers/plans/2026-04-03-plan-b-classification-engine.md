# Classification Engine — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A working on-device classification engine (Phase 1: heuristic rules; Phase 2: TF-IDF model) that classifies window activity into Client/Project/Task. Includes commands to manage heuristic rules and store labeled training samples.

**Architecture:** A `classification/` service module holds `FeatureExtractor`, `HeuristicEngine`, and `TfIdfModel`. `ClassificationService` wraps both phases and is held in `ClassificationState` inside `AppState`. `ClassifierTrainer` retrains the TF-IDF model on the background when ≥ 10 new labeled samples accumulate. Phase 2 activates once 20 total samples exist.

**Tech Stack:** Rust, rusqlite, serde_json, tokio. No external ML libraries — TF-IDF implemented from scratch. Tests run with `cargo test --features test`.

**Depends on:** Plan A (reads `screenshots.ocr_text` to enrich feature text).

---

### Task 1: Add SQLite migration 006

**Files:**
- Create: `src-tauri/src/db/migrations/006_classification_engine.sql`
- Modify: `src-tauri/src/db/migrations.rs`

- [ ] **Step 1: Create migration SQL**

```sql
-- Migration 006: Classification engine tables and columns

-- labeled_samples: user-confirmed or user-corrected training data for TF-IDF
CREATE TABLE labeled_samples (
    id            TEXT PRIMARY KEY NOT NULL,
    feature_text  TEXT NOT NULL,   -- normalized "process_name window_title ocr_text"
    process_name  TEXT NOT NULL,
    modified_at   TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
    window_title  TEXT NOT NULL,
    client_id     TEXT,
    project_id    TEXT,
    task_id       TEXT,
    source        TEXT NOT NULL CHECK (source IN ('user_confirmed', 'user_corrected', 'auto')),
    device_id     TEXT NOT NULL,
    created_at    TEXT NOT NULL,
    synced_at     TEXT            -- NULL = pending sync to external DB
);

CREATE INDEX idx_labeled_samples_created_at
    ON labeled_samples (created_at);

CREATE INDEX idx_labeled_samples_synced
    ON labeled_samples (synced_at)
    WHERE synced_at IS NULL;

-- classifier_model: serialized TF-IDF model weights (JSON blob)
CREATE TABLE classifier_model (
    id           TEXT PRIMARY KEY NOT NULL,
    model_json   TEXT NOT NULL,
    trained_at   TEXT NOT NULL,
    sample_count INTEGER NOT NULL,
    device_id    TEXT NOT NULL
);

-- classification_rules_json: user-editable heuristic rules (JSON array of HeuristicRule)
ALTER TABLE user_preferences
    ADD COLUMN classification_rules_json TEXT;
-- NULL = no rules configured; the engine treats NULL the same as an empty array.
```

- [ ] **Step 2: Register in `migrations.rs`**

Add after the `005` entry:

```rust
    (
        "006_classification_engine",
        include_str!("migrations/006_classification_engine.sql"),
    ),
```

- [ ] **Step 3: Build to verify**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 4: Commit**

```powershell
git add src-tauri/src/db/migrations/006_classification_engine.sql `
        src-tauri/src/db/migrations.rs
git commit -m "feat(db): add labeled_samples, classifier_model, classification_rules_json (migration 006)"
```

---

### Task 2: Create the `classification` service module — shared types and feature extractor

**Files:**
- Create: `src-tauri/src/services/classification/mod.rs`
- Create: `src-tauri/src/services/classification/feature_extractor.rs`
- Modify: `src-tauri/src/services/mod.rs`

- [ ] **Step 1: Create `services/classification/mod.rs`** (module root + shared types)

```rust
//! Auto-classification engine: heuristics (Phase 1) + TF-IDF (Phase 2).

pub mod feature_extractor;
pub mod heuristic;
pub mod tfidf;
pub mod trainer;

use serde::{Deserialize, Serialize};

/// Normalized features extracted from a window activity record + screenshot.
#[derive(Debug, Clone)]
pub struct Features {
    pub process_name: String,
    pub window_title: String,
    pub ocr_text: Option<String>,
    /// Concatenated, lowercased feature text for TF-IDF.
    pub combined_text: String,
}

/// A single classification prediction (one candidate result).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationPrediction {
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub confidence: f32, // 0.0–1.0
    pub source: ClassificationSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationSource {
    Heuristic,
    TfIdf,
    Unclassified,
}

/// Top result + up to 2 additional ranked suggestions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub top: ClassificationPrediction,
    pub suggestions: Vec<ClassificationPrediction>,
}

/// Run full classification: Phase 2 → Phase 1 → suggestions → unclassified.
pub fn classify(
    features: &Features,
    rules: &[heuristic::HeuristicRule],
    model: Option<&tfidf::TfIdfModel>,
) -> ClassificationResult {
    // Phase 2: TF-IDF (only if model is loaded)
    if let Some(m) = model {
        if let Some(candidates) = m.predict(&features.combined_text) {
            if let Some((best, confidence)) = candidates.first() {
                if *confidence >= 0.70 {
                    let top = ClassificationPrediction {
                        client_id: best.client_id.clone(),
                        project_id: best.project_id.clone(),
                        task_id: best.task_id.clone(),
                        confidence: *confidence,
                        source: ClassificationSource::TfIdf,
                    };
                    let suggestions = candidates.iter().skip(1).map(|(l, c)| ClassificationPrediction {
                        client_id: l.client_id.clone(),
                        project_id: l.project_id.clone(),
                        task_id: l.task_id.clone(),
                        confidence: *c,
                        source: ClassificationSource::TfIdf,
                    }).collect();
                    return ClassificationResult { top, suggestions };
                }
            }
        }
    }

    // Phase 1: heuristic rules
    if let Some(hit) = heuristic::apply_heuristics(rules, features) {
        return ClassificationResult { top: hit, suggestions: vec![] };
    }

    // Low-confidence TF-IDF suggestions for active learning toast
    if let Some(m) = model {
        if let Some(candidates) = m.predict(&features.combined_text) {
            if let Some((best, confidence)) = candidates.first() {
                let top = ClassificationPrediction {
                    client_id: best.client_id.clone(),
                    project_id: best.project_id.clone(),
                    task_id: best.task_id.clone(),
                    confidence: *confidence,
                    source: ClassificationSource::TfIdf,
                };
                let suggestions = candidates.iter().skip(1).map(|(l, c)| ClassificationPrediction {
                    client_id: l.client_id.clone(),
                    project_id: l.project_id.clone(),
                    task_id: l.task_id.clone(),
                    confidence: *c,
                    source: ClassificationSource::TfIdf,
                }).collect();
                return ClassificationResult { top, suggestions };
            }
        }
    }

    // Unclassified
    ClassificationResult {
        top: ClassificationPrediction {
            client_id: None,
            project_id: None,
            task_id: None,
            confidence: 0.0,
            source: ClassificationSource::Unclassified,
        },
        suggestions: vec![],
    }
}
```

- [ ] **Step 2: Create `feature_extractor.rs`**

```rust
//! Extracts and normalizes text features from window activity data.

use super::Features;

/// Build a `Features` struct from raw activity data.
/// `ocr_text` may be `None` if OCR has not run or produced no output.
pub fn extract(process_name: &str, window_title: &str, ocr_text: Option<&str>) -> Features {
    let combined_text = normalize(&format!(
        "{} {} {}",
        process_name,
        window_title,
        ocr_text.unwrap_or("")
    ));
    Features {
        process_name: process_name.to_string(),
        window_title: window_title.to_string(),
        ocr_text: ocr_text.map(|s| s.to_string()),
        combined_text,
    }
}

/// Lowercase + collapse whitespace. Strips non-alphanumeric chars except spaces.
pub fn normalize(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_combines_fields() {
        let f = extract("Code", "tracey — Visual Studio Code", Some("fn main"));
        assert!(f.combined_text.contains("code"));
        assert!(f.combined_text.contains("tracey"));
        assert!(f.combined_text.contains("fn main"));
    }

    #[test]
    fn extract_handles_no_ocr() {
        let f = extract("Slack", "general | Slack", None);
        assert!(f.combined_text.contains("slack"));
        assert!(f.combined_text.contains("general"));
    }

    #[test]
    fn normalize_strips_punctuation_and_lowercases() {
        assert_eq!(normalize("Hello, World!"), "hello world");
    }
}
```

- [ ] **Step 3: Register `classification` sub-module in `services/mod.rs`**

```rust
pub mod activity_tracker;
pub mod classification;
pub mod idle_service;
pub mod logger;
pub mod ocr_service;
pub mod screenshot_service;
pub mod sync_service;
pub mod timer_tick;
```

- [ ] **Step 4: Build and run tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features test 2>&1
```

Expected: `feature_extractor` tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/services/classification/ `
        src-tauri/src/services/mod.rs
git commit -m "feat(classification): add classification module, Features type, and FeatureExtractor"
```

---

### Task 3: Implement `heuristic.rs` and `tfidf.rs`

**Files:**
- Create: `src-tauri/src/services/classification/heuristic.rs`
- Create: `src-tauri/src/services/classification/tfidf.rs`

- [ ] **Step 1: Write the failing tests for heuristics**

Create `src-tauri/src/services/classification/heuristic.rs` with only tests first:

```rust
//! Phase 1: rule-based classification.

use serde::{Deserialize, Serialize};
use super::{ClassificationPrediction, ClassificationSource, Features};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicRule {
    pub app_contains: Option<String>,    // case-insensitive substring match on process_name
    pub title_contains: Option<String>,  // case-insensitive substring match on window_title
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
}

/// Evaluate rules in order; return the first match as a 100%-confidence prediction.
pub fn apply_heuristics(rules: &[HeuristicRule], features: &Features) -> Option<ClassificationPrediction> {
    for rule in rules {
        let app_ok = rule.app_contains.as_ref()
            .map(|a| features.process_name.to_lowercase().contains(&a.to_lowercase()))
            .unwrap_or(true);
        let title_ok = rule.title_contains.as_ref()
            .map(|t| features.window_title.to_lowercase().contains(&t.to_lowercase()))
            .unwrap_or(true);
        if app_ok && title_ok {
            return Some(ClassificationPrediction {
                client_id: rule.client_id.clone(),
                project_id: rule.project_id.clone(),
                task_id: rule.task_id.clone(),
                confidence: 1.0,
                source: ClassificationSource::Heuristic,
            });
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::classification::feature_extractor::extract;

    fn rule(app: Option<&str>, title: Option<&str>, project_id: &str) -> HeuristicRule {
        HeuristicRule {
            app_contains: app.map(|s| s.to_string()),
            title_contains: title.map(|s| s.to_string()),
            client_id: None,
            project_id: Some(project_id.to_string()),
            task_id: None,
        }
    }

    #[test]
    fn matches_on_app_and_title() {
        let rules = vec![rule(Some("Code"), Some("tracey"), "proj-tracey")];
        let f = extract("Code", "tracey — Visual Studio Code", None);
        let result = apply_heuristics(&rules, &f).unwrap();
        assert_eq!(result.project_id.as_deref(), Some("proj-tracey"));
        assert_eq!(result.confidence, 1.0);
        assert_eq!(result.source, ClassificationSource::Heuristic);
    }

    #[test]
    fn app_only_rule_matches_any_title() {
        let rules = vec![rule(Some("Slack"), None, "proj-comms")];
        let f = extract("Slack", "general | Slack", None);
        let result = apply_heuristics(&rules, &f).unwrap();
        assert_eq!(result.project_id.as_deref(), Some("proj-comms"));
    }

    #[test]
    fn no_match_returns_none() {
        let rules = vec![rule(Some("Code"), Some("tracey"), "proj-tracey")];
        let f = extract("Slack", "general | Slack", None);
        assert!(apply_heuristics(&rules, &f).is_none());
    }

    #[test]
    fn first_rule_wins() {
        let rules = vec![
            rule(Some("Code"), None, "proj-first"),
            rule(Some("Code"), None, "proj-second"),
        ];
        let f = extract("Code", "anything", None);
        let result = apply_heuristics(&rules, &f).unwrap();
        assert_eq!(result.project_id.as_deref(), Some("proj-first"));
    }
}
```

- [ ] **Step 2: Run heuristic tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features test classification::heuristic 2>&1
```

Expected: all 4 heuristic tests pass.

- [ ] **Step 3: Create `tfidf.rs`**

```rust
//! Phase 2: TF-IDF bag-of-words classifier.
//! Trains on `LabeledSample` data. Predicts top-3 (ClassLabel, confidence) pairs.

use std::collections::{HashMap, HashSet};
use serde::{Deserialize, Serialize};

const STOPWORDS: &[&str] = &[
    "the","a","an","and","or","but","in","on","at","to","for","of","with","by",
    "from","is","was","are","be","has","have","do","did","not","this","that","it","its",
];

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| t.len() > 1)
        .map(|t| t.to_string())
        .filter(|t| !STOPWORDS.contains(&t.as_str()))
        .collect()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassLabel {
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
}

pub struct TrainingSample {
    pub text: String,
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TfIdfModel {
    vocab: HashMap<String, usize>,
    idf: Vec<f32>,
    centroids: HashMap<String, Vec<f32>>,
    class_labels: HashMap<String, ClassLabel>,
}

impl TfIdfModel {
    /// Build model from training samples. Returns `None` if samples are empty
    /// or produce fewer than 2 distinct classes.
    pub fn train(samples: &[TrainingSample]) -> Option<Self> {
        if samples.is_empty() { return None; }

        // Build vocabulary
        let tokenized: Vec<Vec<String>> = samples.iter().map(|s| tokenize(&s.text)).collect();
        let mut vocab: HashMap<String, usize> = HashMap::new();
        for tokens in &tokenized {
            for t in tokens {
                if !vocab.contains_key(t) {
                    let idx = vocab.len();
                    vocab.insert(t.clone(), idx);
                }
            }
        }
        if vocab.is_empty() { return None; }
        let vocab_size = vocab.len();

        // IDF: log((N+1)/(df+1))+1  (smooth, add-1)
        let n = samples.len() as f32;
        let mut df = vec![0usize; vocab_size];
        for tokens in &tokenized {
            let unique: HashSet<&String> = tokens.iter().collect();
            for t in unique {
                if let Some(&i) = vocab.get(t) { df[i] += 1; }
            }
        }
        let idf: Vec<f32> = df.iter()
            .map(|&d| ((n + 1.0) / (d as f32 + 1.0)).ln() + 1.0)
            .collect();

        // Group samples by class key
        let mut class_vecs: HashMap<String, Vec<Vec<f32>>> = HashMap::new();
        let mut class_labels: HashMap<String, ClassLabel> = HashMap::new();
        for (sample, tokens) in samples.iter().zip(tokenized.iter()) {
            let key = format!(
                "{}|{}|{}",
                sample.client_id.as_deref().unwrap_or(""),
                sample.project_id.as_deref().unwrap_or(""),
                sample.task_id.as_deref().unwrap_or(""),
            );
            let mut counts = vec![0usize; vocab_size];
            for t in tokens { if let Some(&i) = vocab.get(t) { counts[i] += 1; } }
            let doc_len = tokens.len().max(1) as f32;
            let tfidf: Vec<f32> = counts.iter().enumerate()
                .map(|(i, &c)| (c as f32 / doc_len) * idf[i])
                .collect();
            class_vecs.entry(key.clone()).or_default().push(tfidf);
            class_labels.entry(key).or_insert(ClassLabel {
                client_id: sample.client_id.clone(),
                project_id: sample.project_id.clone(),
                task_id: sample.task_id.clone(),
            });
        }

        // Compute L2-normalized centroids
        let mut centroids: HashMap<String, Vec<f32>> = HashMap::new();
        for (key, vecs) in &class_vecs {
            let mut c = vec![0.0f32; vocab_size];
            for v in vecs { for (i, &x) in v.iter().enumerate() { c[i] += x; } }
            let n = vecs.len() as f32;
            for x in &mut c { *x /= n; }
            let norm: f32 = c.iter().map(|x| x * x).sum::<f32>().sqrt();
            if norm > 0.0 { for x in &mut c { *x /= norm; } }
            centroids.insert(key.clone(), c);
        }

        Some(TfIdfModel { vocab, idf, centroids, class_labels })
    }

    /// Returns top-3 `(ClassLabel, confidence)` sorted descending by cosine similarity.
    /// Returns `None` if the query produces a zero vector.
    pub fn predict(&self, text: &str) -> Option<Vec<(ClassLabel, f32)>> {
        let tokens = tokenize(text);
        if tokens.is_empty() { return None; }

        let vocab_size = self.vocab.len();
        let mut counts = vec![0usize; vocab_size];
        for t in &tokens { if let Some(&i) = self.vocab.get(t) { counts[i] += 1; } }
        let doc_len = tokens.len() as f32;
        let mut query: Vec<f32> = counts.iter().enumerate()
            .map(|(i, &c)| (c as f32 / doc_len) * self.idf[i])
            .collect();
        let norm: f32 = query.iter().map(|x| x * x).sum::<f32>().sqrt();
        if norm == 0.0 { return None; }
        for x in &mut query { *x /= norm; }

        let mut scores: Vec<(String, f32)> = self.centroids.iter()
            .map(|(k, c)| {
                let sim: f32 = query.iter().zip(c.iter()).map(|(q, v)| q * v).sum();
                (k.clone(), sim.clamp(0.0, 1.0))
            })
            .collect();
        scores.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        let results: Vec<(ClassLabel, f32)> = scores.into_iter()
            .take(3)
            .filter_map(|(k, s)| self.class_labels.get(&k).map(|l| (l.clone(), s)))
            .collect();
        if results.is_empty() { None } else { Some(results) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(text: &str, project: &str) -> TrainingSample {
        TrainingSample {
            text: text.to_string(),
            client_id: None,
            project_id: Some(project.to_string()),
            task_id: None,
        }
    }

    #[test]
    fn train_returns_none_on_empty_samples() {
        assert!(TfIdfModel::train(&[]).is_none());
    }

    #[test]
    fn predict_returns_correct_class() {
        let samples = vec![
            sample("visual studio code tracey rust", "proj-tracey"),
            sample("visual studio code tracey rust", "proj-tracey"),
            sample("slack general channel messages", "proj-comms"),
            sample("slack general channel messages", "proj-comms"),
        ];
        let model = TfIdfModel::train(&samples).unwrap();
        let result = model.predict("tracey rust code").unwrap();
        assert_eq!(result[0].0.project_id.as_deref(), Some("proj-tracey"));
        assert!(result[0].1 > 0.5);
    }

    #[test]
    fn predict_returns_none_for_unknown_text() {
        let samples = vec![
            sample("code tracey", "proj-tracey"),
            sample("code tracey", "proj-tracey"),
        ];
        let model = TfIdfModel::train(&samples).unwrap();
        // Completely unknown tokens — zero TF-IDF vector
        let result = model.predict("xyzzy qqqqqq");
        // May return None or a low-confidence result — both are acceptable
        if let Some(r) = result {
            assert!(r[0].1 < 0.3);
        }
    }

    #[test]
    fn top_3_suggestions_returned() {
        let samples = vec![
            sample("code tracey rust", "proj-a"),
            sample("code tracey rust", "proj-a"),
            sample("code browser html", "proj-b"),
            sample("code browser html", "proj-b"),
            sample("slack chat team", "proj-c"),
            sample("slack chat team", "proj-c"),
        ];
        let model = TfIdfModel::train(&samples).unwrap();
        let results = model.predict("code tracey").unwrap();
        assert!(results.len() <= 3);
        // First result should be the most relevant
        assert_eq!(results[0].0.project_id.as_deref(), Some("proj-a"));
    }
}
```

- [ ] **Step 4: Run TF-IDF tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features test classification::tfidf 2>&1
```

Expected: all 4 TF-IDF tests pass.

- [ ] **Step 5: Commit**

```powershell
git add src-tauri/src/services/classification/heuristic.rs `
        src-tauri/src/services/classification/tfidf.rs `
        src-tauri/src/services/classification/mod.rs
git commit -m "feat(classification): implement heuristic engine and TF-IDF model"
```

---

### Task 4: Implement `trainer.rs` and `ClassificationState`

**Files:**
- Create: `src-tauri/src/services/classification/trainer.rs`
- Modify: `src-tauri/src/commands/mod.rs` (add `ClassificationState` to `AppState`)
- Modify: `src-tauri/src/lib.rs` (wire `ClassificationState` into Tauri)

- [ ] **Step 1: Create `trainer.rs`**

```rust
//! Loads labeled samples from SQLite, trains a TfIdfModel, and persists it.

use rusqlite::Connection;
use ulid::Ulid;
use chrono::Utc;
use super::tfidf::{TfIdfModel, TrainingSample};

/// Minimum total sample count to activate Phase 2.
pub const PHASE2_MIN_SAMPLES: i64 = 20;

/// Load all labeled samples from the database.
pub fn load_samples(conn: &Connection) -> Vec<TrainingSample> {
    let mut stmt = match conn.prepare(
        "SELECT feature_text, client_id, project_id, task_id FROM labeled_samples",
    ) {
        Ok(s) => s,
        Err(_) => return vec![],
    };
    stmt.query_map([], |row| {
        Ok(TrainingSample {
            text: row.get(0)?,
            client_id: row.get(1)?,
            project_id: row.get(2)?,
            task_id: row.get(3)?,
        })
    })
    .unwrap_or_else(|_| Box::new(std::iter::empty()))
    .filter_map(|r| r.ok())
    .collect()
}

/// Count labeled samples in the database.
pub fn count_samples(conn: &Connection) -> i64 {
    conn.query_row("SELECT COUNT(*) FROM labeled_samples", [], |r| r.get(0))
        .unwrap_or(0)
}

/// Train and persist the model. Returns the trained model on success.
/// Returns `None` if there are fewer than `PHASE2_MIN_SAMPLES` total samples.
pub fn retrain(conn: &Connection) -> Option<TfIdfModel> {
    let count = count_samples(conn);
    if count < PHASE2_MIN_SAMPLES { return None; }

    let samples = load_samples(conn);
    let model = TfIdfModel::train(&samples)?;

    // Serialize and persist
    let model_json = serde_json::to_string(&model).ok()?;
    let id = Ulid::new().to_string().to_lowercase();
    let now = Utc::now().to_rfc3339();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    // Replace any existing model row
    let _ = conn.execute("DELETE FROM classifier_model", []);
    let _ = conn.execute(
        "INSERT INTO classifier_model (id, model_json, trained_at, sample_count, device_id) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![id, model_json, now, count, device_id],
    );

    log::info!("[trainer] Model retrained on {} samples", count);
    Some(model)
}

/// Load the latest persisted model from the database (called at startup).
pub fn load_model(conn: &Connection) -> Option<TfIdfModel> {
    let model_json: String = conn.query_row(
        "SELECT model_json FROM classifier_model LIMIT 1",
        [],
        |r| r.get(0),
    ).ok()?;
    serde_json::from_str(&model_json).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("
            CREATE TABLE labeled_samples (
                id TEXT PRIMARY KEY, feature_text TEXT NOT NULL,
                process_name TEXT NOT NULL, window_title TEXT NOT NULL,
                client_id TEXT, project_id TEXT, task_id TEXT,
                source TEXT NOT NULL, device_id TEXT NOT NULL,
                created_at TEXT NOT NULL, modified_at TEXT NOT NULL DEFAULT (datetime('now')),
                synced_at TEXT
            );
            CREATE TABLE classifier_model (
                id TEXT PRIMARY KEY, model_json TEXT NOT NULL,
                trained_at TEXT NOT NULL, sample_count INTEGER NOT NULL, device_id TEXT NOT NULL
            );
        ").unwrap();
        conn
    }

    fn insert_sample(conn: &Connection, text: &str, project: &str) {
        conn.execute(
            "INSERT INTO labeled_samples (id,feature_text,process_name,window_title,client_id,project_id,task_id,source,device_id,created_at,modified_at)
             VALUES (?,?,?,?,NULL,?,NULL,'user_confirmed','dev',datetime('now'),datetime('now'))",
            rusqlite::params![Ulid::new().to_string(), text, "app", "title", project],
        ).unwrap();
    }

    #[test]
    fn retrain_returns_none_below_threshold() {
        let conn = open_test_db();
        for i in 0..19 {
            insert_sample(&conn, &format!("code tracey rust {i}"), "proj-a");
        }
        assert_eq!(count_samples(&conn), 19);
        assert!(retrain(&conn).is_none());
    }

    #[test]
    fn retrain_succeeds_at_threshold() {
        let conn = open_test_db();
        for _ in 0..10 {
            insert_sample(&conn, "code tracey rust", "proj-a");
            insert_sample(&conn, "slack general chat", "proj-b");
        }
        assert_eq!(count_samples(&conn), 20);
        let model = retrain(&conn);
        assert!(model.is_some());
        // Model should be persisted
        let loaded = load_model(&conn);
        assert!(loaded.is_some());
    }
}
```

- [ ] **Step 2: Run trainer tests**

```powershell
cargo test --manifest-path src-tauri/Cargo.toml --features test classification::trainer 2>&1
```

Expected: both trainer tests pass.

- [ ] **Step 3: Add `ClassificationState` to `AppState`**

In `src-tauri/src/commands/mod.rs`, add:

```rust
use crate::services::classification::tfidf::TfIdfModel;
use crate::services::classification::heuristic::HeuristicRule;

/// Shared classification state: loaded model + rule cache + sample counter.
pub struct ClassificationState {
    pub model: Option<TfIdfModel>,
    pub rules: Vec<HeuristicRule>,
    pub sample_count_at_last_train: i64,
}

impl Default for ClassificationState {
    fn default() -> Self {
        ClassificationState { model: None, rules: vec![], sample_count_at_last_train: 0 }
    }
}
```

Add `classification_state` field to `AppState`:

```rust
pub struct AppState {
    pub db: std::sync::Mutex<rusqlite::Connection>,
    pub platform: Arc<dyn PlatformHooks + Send + Sync>,
    pub sync_state: Arc<std::sync::Mutex<SyncState>>,
    pub sync_notify: Arc<tokio::sync::Notify>,
    pub classification_state: Arc<std::sync::Mutex<ClassificationState>>,
}
```

- [ ] **Step 4: Wire `ClassificationState` into `lib.rs`**

In `src-tauri/src/lib.rs`, initialize and load the model at startup:

```rust
use services::classification::trainer;

pub fn run() {
    env_logger::init();
    commands::init_health();
    let conn = db::open().expect("DB init failed");

    // Load persisted classification model and rules
    let classification_state = {
        let model = trainer::load_model(&conn);
        let rules_json: Option<String> = conn
            .query_row("SELECT classification_rules_json FROM user_preferences LIMIT 1", [], |r| r.get(0))
            .ok()
            .flatten();
        let rules: Vec<services::classification::heuristic::HeuristicRule> = rules_json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        Arc::new(std::sync::Mutex::new(commands::ClassificationState {
            model,
            rules,
            sample_count_at_last_train: trainer::count_samples(&conn),
        }))
    };

    // ... rest of existing setup ...
    tauri::Builder::default()
        .manage(AppState {
            db: std::sync::Mutex::new(conn),
            platform,
            sync_state,
            sync_notify,
            classification_state,
        })
        // ...
}
```

- [ ] **Step 5: Build to verify**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
```

Expected: builds without errors.

- [ ] **Step 6: Commit**

```powershell
git add src-tauri/src/services/classification/trainer.rs `
        src-tauri/src/commands/mod.rs `
        src-tauri/src/lib.rs
git commit -m "feat(classification): add ClassifierTrainer, ClassificationState in AppState, load model at startup"
```

---

### Task 5: Add Tauri commands for rules management and classification testing

**Files:**
- Create: `src-tauri/src/commands/classification.rs`
- Modify: `src-tauri/src/commands/mod.rs` (add `pub mod classification;`)
- Modify: `src-tauri/src/lib.rs` (register commands)

- [ ] **Step 1: Create `commands/classification.rs`**

```rust
//! Tauri commands for managing classification rules and testing the engine.

use tauri::State;
use serde::{Deserialize, Serialize};
use ulid::Ulid;
use chrono::Utc;

use crate::commands::AppState;
use crate::services::classification::{
    self,
    feature_extractor::extract,
    heuristic::HeuristicRule,
    trainer,
    ClassificationResult,
};

// ── Rules CRUD ────────────────────────────────────────────────────────────────

#[tauri::command]
pub fn classification_rules_get(state: State<'_, AppState>) -> Result<Vec<HeuristicRule>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let json: Option<String> = conn.query_row(
        "SELECT classification_rules_json FROM user_preferences LIMIT 1",
        [],
        |r| r.get(0),
    ).ok().flatten();
    Ok(json.and_then(|j| serde_json::from_str(&j).ok()).unwrap_or_default())
}

#[tauri::command]
pub fn classification_rules_update(
    state: State<'_, AppState>,
    rules: Vec<HeuristicRule>,
) -> Result<(), String> {
    let json = serde_json::to_string(&rules).map_err(|e| e.to_string())?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE user_preferences SET classification_rules_json = ?1 WHERE id = 1",
            rusqlite::params![json],
        ).map_err(|e| e.to_string())?;
    }
    // Refresh in-memory rules cache
    let mut cs = state.classification_state.lock().map_err(|e| e.to_string())?;
    cs.rules = rules;
    Ok(())
}

// ── Test-classify (dev/debug) ─────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct ClassifyTestRequest {
    pub process_name: String,
    pub window_title: String,
    pub ocr_text: Option<String>,
}

#[tauri::command]
pub fn classification_classify_test(
    state: State<'_, AppState>,
    request: ClassifyTestRequest,
) -> Result<ClassificationResult, String> {
    let features = extract(&request.process_name, &request.window_title, request.ocr_text.as_deref());
    let cs = state.classification_state.lock().map_err(|e| e.to_string())?;
    Ok(classification::classify(&features, &cs.rules, cs.model.as_ref()))
}

// ── Labeled sample submit ─────────────────────────────────────────────────────

#[derive(Deserialize)]
pub struct LabeledSampleSubmitRequest {
    pub process_name: String,
    pub window_title: String,
    pub ocr_text: Option<String>,
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
    pub source: String, // "user_confirmed" | "user_corrected"
}

#[tauri::command]
pub fn labeled_sample_submit(
    state: State<'_, AppState>,
    request: LabeledSampleSubmitRequest,
) -> Result<(), String> {
    let features = extract(&request.process_name, &request.window_title, request.ocr_text.as_deref());
    let id = Ulid::new().to_string().to_lowercase();
    let now = Utc::now().to_rfc3339();
    let device_id = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "local".to_string());

    let (should_retrain, sample_count) = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        conn.execute(
            "INSERT INTO labeled_samples \
             (id, feature_text, process_name, window_title, client_id, project_id, task_id, \
              source, device_id, created_at, modified_at) \
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10)",
            rusqlite::params![
                id, features.combined_text,
                features.process_name, features.window_title,
                request.client_id, request.project_id, request.task_id,
                request.source, device_id, now,
            ],
        ).map_err(|e| e.to_string())?;

        let total = trainer::count_samples(&conn);
        let cs = state.classification_state.lock().map_err(|e| e.to_string())?;
        let new_since_last = total - cs.sample_count_at_last_train;
        (new_since_last >= 10 && total >= trainer::PHASE2_MIN_SAMPLES, total)
    }; // db lock released

    if should_retrain {
        let app_state = state.inner().clone();
        tauri::async_runtime::spawn(async move {
            let conn_guard = app_state.db.lock();
            if let Ok(conn) = conn_guard {
                if let Some(model) = trainer::retrain(&conn) {
                    drop(conn); // release db lock before locking classification_state
                    if let Ok(mut cs) = app_state.classification_state.lock() {
                        cs.model = Some(model);
                        cs.sample_count_at_last_train = sample_count;
                        log::info!("[classification] Model retrained and loaded");
                    }
                }
            }
        });
    }

    Ok(())
}
```

- [ ] **Step 2: Register the module**

In `src-tauri/src/commands/mod.rs`, add `pub mod classification;`.

- [ ] **Step 3: Register commands in `lib.rs`**

Add to the `invoke_handler`:

```rust
commands::classification::classification_rules_get,
commands::classification::classification_rules_update,
commands::classification::classification_classify_test,
commands::classification::labeled_sample_submit,
```

- [ ] **Step 4: Add C# wrappers to `TauriIpcService.cs`**

Add to the Classification section:

```csharp
public record ClassifyTestRequest(
    [property: JsonPropertyName("process_name")] string ProcessName,
    [property: JsonPropertyName("window_title")] string WindowTitle,
    [property: JsonPropertyName("ocr_text")] string? OcrText);

public record LabeledSampleSubmitRequest(
    [property: JsonPropertyName("process_name")] string ProcessName,
    [property: JsonPropertyName("window_title")] string WindowTitle,
    [property: JsonPropertyName("ocr_text")] string? OcrText,
    [property: JsonPropertyName("client_id")] string? ClientId,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("task_id")] string? TaskId,
    [property: JsonPropertyName("source")] string Source);

public Task<ClassificationResult> ClassificationClassifyTestAsync(ClassifyTestRequest request) =>
    Invoke<ClassificationResult>("classification_classify_test", new { request });

public Task LabeledSampleSubmitAsync(LabeledSampleSubmitRequest request) =>
    Invoke<object>("labeled_sample_submit", new { request });
```

- [ ] **Step 5: Write tests for rules roundtrip and sample submit threshold**

Add to `src/Tracey.Tests/ClassificationCommandTests.cs`:

```csharp
// tests/Tracey.Tests/ClassificationCommandTests.cs — in-process Rust unit tests only
// These are Rust-side; add to src-tauri/src/commands/classification.rs #[cfg(test)] block
```

Add this `#[cfg(test)]` block at the bottom of `src-tauri/src/commands/classification.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn setup_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("
            CREATE TABLE user_preferences (
                id INTEGER PRIMARY KEY, classification_rules_json TEXT
            );
            INSERT INTO user_preferences (id) VALUES (1);
            CREATE TABLE labeled_samples (
                id TEXT PRIMARY KEY, feature_text TEXT NOT NULL,
                process_name TEXT NOT NULL, window_title TEXT NOT NULL,
                client_id TEXT, project_id TEXT, task_id TEXT,
                source TEXT NOT NULL, device_id TEXT NOT NULL,
                created_at TEXT NOT NULL, modified_at TEXT NOT NULL DEFAULT (datetime('now')),
                synced_at TEXT
            );
            CREATE TABLE classifier_model (
                id TEXT PRIMARY KEY, model_json TEXT NOT NULL,
                trained_at TEXT NOT NULL, sample_count INTEGER NOT NULL,
                device_id TEXT NOT NULL
            );
        ").unwrap();
        conn
    }

    #[test]
    fn rules_roundtrip_empty() {
        let conn = setup_db();
        let json: Option<String> = conn.query_row(
            "SELECT classification_rules_json FROM user_preferences LIMIT 1",
            [], |r| r.get(0),
        ).ok().flatten();
        let rules: Vec<HeuristicRule> = json
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        assert!(rules.is_empty(), "Should be empty on fresh DB");
    }

    #[test]
    fn rules_roundtrip_write_and_read() {
        let conn = setup_db();
        let rule = HeuristicRule {
            app_contains: Some("code".to_string()),
            title_contains: Some("tracey".to_string()),
            client_id: None,
            project_id: Some("proj-1".to_string()),
            task_id: None,
        };
        let json = serde_json::to_string(&[&rule]).unwrap();
        conn.execute(
            "UPDATE user_preferences SET classification_rules_json = ?1 WHERE id = 1",
            rusqlite::params![json],
        ).unwrap();

        let stored: Option<String> = conn.query_row(
            "SELECT classification_rules_json FROM user_preferences LIMIT 1",
            [], |r| r.get(0),
        ).ok().flatten();
        let loaded: Vec<HeuristicRule> = stored
            .and_then(|j| serde_json::from_str(&j).ok())
            .unwrap_or_default();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].project_id.as_deref(), Some("proj-1"));
    }

    #[test]
    fn labeled_sample_insert_includes_modified_at() {
        let conn = setup_db();
        conn.execute(
            "INSERT INTO labeled_samples \
             (id,feature_text,process_name,window_title,client_id,project_id,task_id,source,device_id,created_at,modified_at) \
             VALUES ('s1','feat','app','title',NULL,'p1',NULL,'user_confirmed','dev',datetime('now'),datetime('now'))",
            [],
        ).unwrap();
        let modified_at: String = conn.query_row(
            "SELECT modified_at FROM labeled_samples WHERE id = 's1'", [],
            |r| r.get(0),
        ).unwrap();
        assert!(!modified_at.is_empty(), "modified_at should be set");
    }
}
```

- [ ] **Step 6: Build and run all tests**

```powershell
cargo build --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml --features test 2>&1
```

Expected: all tests pass.

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/src/commands/classification.rs `
        src-tauri/src/commands/mod.rs `
        src-tauri/src/lib.rs `
        src/Tracey.App/Services/TauriIpcService.cs
git commit -m "feat(classification): add Tauri commands for rules CRUD, test-classify, and labeled sample submit"
```

---

### Task 6: Add heuristic rules management to the Settings page

The spec requires heuristic rules to be configurable from Settings. The backend commands (`classification_rules_get`, `classification_rules_update`) are already added in Task 5.

**Files:**
- Modify: `src/Tracey.App/Pages/Settings.razor`
- Modify: `src/Tracey.App/Services/TauriIpcService.cs`

- [ ] **Step 1: Add IPC wrappers for rules to `TauriIpcService.cs`**

Add to the Classification section in `TauriIpcService.cs`:

```csharp
public record HeuristicRule(
    [property: JsonPropertyName("app_contains")] string? AppContains,
    [property: JsonPropertyName("title_contains")] string? TitleContains,
    [property: JsonPropertyName("client_id")] string? ClientId,
    [property: JsonPropertyName("project_id")] string? ProjectId,
    [property: JsonPropertyName("task_id")] string? TaskId);

public Task<HeuristicRule[]> ClassificationRulesGetAsync() =>
    Invoke<HeuristicRule[]>("classification_rules_get");

public Task ClassificationRulesUpdateAsync(HeuristicRule[] rules) =>
    Invoke<object>("classification_rules_update", new { rules });
```

- [ ] **Step 2: Add "Auto-Classification Rules" section to `Settings.razor`**

Locate the existing process deny-list section in `Settings.razor` (search for `process_deny_list` or "Process Deny List"). Add a new section directly below it:

```razor
@* ── Auto-Classification Rules ───────────────────────────────────────────── *@
<section class="settings-section">
    <h3>Auto-Classification Rules</h3>
    <p class="section-description">
        Rules are checked in order. First match wins at 100% confidence, skipping the model.
    </p>

    @foreach (var (rule, i) in _classificationRules.Select((r, i) => (r, i)))
    {
        <div class="rule-row">
            <input type="text" placeholder="App contains…"
                   value="@rule.AppContains"
                   @onchange="e => UpdateRule(i, rule with { AppContains = e.Value?.ToString() })" />
            <input type="text" placeholder="Title contains…"
                   value="@rule.TitleContains"
                   @onchange="e => UpdateRule(i, rule with { TitleContains = e.Value?.ToString() })" />
            <input type="text" placeholder="Project ID"
                   value="@rule.ProjectId"
                   @onchange="e => UpdateRule(i, rule with { ProjectId = e.Value?.ToString() })" />
            <button type="button" @onclick="() => RemoveRule(i)">Remove</button>
        </div>
    }

    <button type="button" @onclick="AddRule">+ Add rule</button>
    <button type="button" @onclick="SaveRules">Save rules</button>
    @if (!string.IsNullOrEmpty(_rulesError)) { <p class="error">@_rulesError</p> }
</section>
```

- [ ] **Step 3: Add rules state and handlers to `Settings.razor` `@code` block**

In the `@code` block, add:

```csharp
private List<HeuristicRule> _classificationRules = new();
private string _rulesError = string.Empty;

protected override async Task OnInitializedAsync()
{
    // ... existing initialization code ...
    var rules = await Tauri.ClassificationRulesGetAsync();
    _classificationRules = rules?.ToList() ?? new();
}

private void UpdateRule(int index, HeuristicRule updated)
{
    _classificationRules[index] = updated;
}

private void AddRule()
{
    _classificationRules.Add(new HeuristicRule(null, null, null, null, null));
    StateHasChanged();
}

private void RemoveRule(int index)
{
    _classificationRules.RemoveAt(index);
    StateHasChanged();
}

private async Task SaveRules()
{
    _rulesError = string.Empty;
    try
    {
        await Tauri.ClassificationRulesUpdateAsync(_classificationRules.ToArray());
    }
    catch (Exception ex)
    {
        _rulesError = $"Couldn't save rules. {ex.Message}";
    }
    StateHasChanged();
}
```

- [ ] **Step 4: Build frontend**

```powershell
dotnet build src/Tracey.App
```

Expected: builds without errors.

- [ ] **Step 5: Commit**

```powershell
git add src/Tracey.App/Pages/Settings.razor `
        src/Tracey.App/Services/TauriIpcService.cs
git commit -m "feat(settings): add heuristic classification rules management section"
```
