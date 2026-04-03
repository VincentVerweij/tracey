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
