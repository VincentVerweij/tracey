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
