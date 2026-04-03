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
