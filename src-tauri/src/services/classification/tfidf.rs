//! Phase 2: TF-IDF bag-of-words classifier. (stub — implemented in Task 3)

use serde::{Deserialize, Serialize};

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
pub struct TfIdfModel {}

impl TfIdfModel {
    pub fn train(_samples: &[TrainingSample]) -> Option<Self> { None }
    pub fn predict(&self, _text: &str) -> Option<Vec<(ClassLabel, f32)>> { None }
}
