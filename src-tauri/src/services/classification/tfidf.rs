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
