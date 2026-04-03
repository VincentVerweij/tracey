//! Phase 1: rule-based classification. (stub — implemented in Task 3)

use serde::{Deserialize, Serialize};
use super::{ClassificationPrediction, Features};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeuristicRule {
    pub app_contains: Option<String>,
    pub title_contains: Option<String>,
    pub client_id: Option<String>,
    pub project_id: Option<String>,
    pub task_id: Option<String>,
}

pub fn apply_heuristics(_rules: &[HeuristicRule], _features: &Features) -> Option<ClassificationPrediction> {
    None
}
