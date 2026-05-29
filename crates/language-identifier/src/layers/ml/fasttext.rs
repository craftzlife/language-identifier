//! `FastTextClassifier` — Layer 9 default implementation.
//!
//! Wraps Meta/FAIR's `lid.176.bin` (full-precision 176-language
//! language-identification model) via the pure-Rust [`fasttext`] crate.
//!
//! ## What this does and does not contribute
//!
//! The classifier maps fastText's coarse ISO 639 labels into the
//! library's BCP 47 set:
//!
//! | fastText label    | mapped tag                             |
//! | ----------------- | -------------------------------------- |
//! | `__label__en`     | `en-US`                                |
//! | `__label__vi`     | `vi-VN`                                |
//! | `__label__ja`     | `ja`                                   |
//! | `__label__ko`     | `ko`                                   |
//! | `__label__zh`     | `zh-Hant` if input contains a known    |
//! |                   | Traditional-only character; otherwise  |
//! |                   | `zh-Hans` (the modern default).        |
//! | all 171 others    | *dropped*                              |
//!
//! lid.176 emits a single `__label__zh` label without splitting Hans
//! vs Hant. Rather than throw the signal away, we promote it to a
//! concrete BCP 47 tag using a tiny ISO 639 → BCP 47 disambiguation
//! check: scan the input for the Layer 3 `is_hant_only` character set
//! and pick `zh-Hant` when any matches, otherwise default to
//! `zh-Hans`. This is intentionally a coarse fallback — Layer 3's
//! marker-based path is still authoritative for inputs with explicit
//! Hans-only signals.

use std::path::PathBuf;

use ::fasttext::FastText;

use super::{MlClassifier, UnsupportedSignal};
use crate::layers::orthography;

/// fastText-backed [`MlClassifier`] for Layer 9. Construct via
/// [`FastTextClassifier::builder`].
pub struct FastTextClassifier {
    model: FastText,
    top_k: usize,
}

impl FastTextClassifier {
    /// Start configuring a classifier. The builder's only required
    /// step is [`FastTextClassifierBuilder::model_path`].
    pub fn builder() -> FastTextClassifierBuilder {
        FastTextClassifierBuilder {
            model_path: None,
            top_k: 5,
        }
    }
}

/// Builder for [`FastTextClassifier`]. Future tuning knobs
/// (probability threshold, dictionary lazy loading) will land here
/// without breaking the constructor.
pub struct FastTextClassifierBuilder {
    model_path: Option<PathBuf>,
    top_k: usize,
}

impl FastTextClassifierBuilder {
    /// Set the path to a fastText binary model file (e.g.
    /// `lid.176.bin`). Required.
    pub fn model_path<P: Into<PathBuf>>(mut self, p: P) -> Self {
        self.model_path = Some(p.into());
        self
    }

    /// How many top predictions to request from fastText. Default 5.
    /// Values below 1 are clamped to 1.
    pub fn top_k(mut self, k: u8) -> Self {
        self.top_k = usize::from(k.max(1));
        self
    }

    /// Load the model and finalize the classifier.
    pub fn build(self) -> Result<FastTextClassifier, LoadError> {
        let path = self.model_path.ok_or(LoadError::NoModelPath)?;
        let model = FastText::load_model(&path).map_err(|e| LoadError::FastText(e.to_string()))?;
        Ok(FastTextClassifier {
            model,
            top_k: self.top_k,
        })
    }
}

impl MlClassifier for FastTextClassifier {
    fn classify(&self, normalized_text: &str) -> Vec<(String, f32)> {
        let cleaned = scrub_newlines(normalized_text);
        let has_hant = contains_hant_char(normalized_text);
        self.model
            .predict(&cleaned, self.top_k, 0.0)
            .into_iter()
            .filter_map(|p| map_label(&p.label, has_hant).map(|tag| (tag.to_string(), p.prob)))
            .collect()
    }

    fn unsupported_signal(&self, normalized_text: &str) -> Option<UnsupportedSignal> {
        let cleaned = scrub_newlines(normalized_text);
        let top = self.model.predict(&cleaned, 1, 0.0).into_iter().next()?;
        let code = top.label.strip_prefix("__label__").unwrap_or(&top.label);
        // `zh` IS in the supported set — we drop it from `classify` for
        // a different reason (variant disambiguation belongs to Layer
        // 3). Treating it as "unsupported" here would wrongly downgrade
        // Han inputs.
        if is_supported_lid176_code(code) {
            None
        } else {
            Some(UnsupportedSignal {
                label: code.to_string(),
                confidence: top.prob,
            })
        }
    }
}

/// Errors produced while constructing a [`FastTextClassifier`].
#[derive(Debug)]
pub enum LoadError {
    /// [`FastTextClassifierBuilder::model_path`] was never called.
    NoModelPath,
    /// The underlying fastText library failed to load the model
    /// (file missing, corrupted, wrong format version, etc.).
    FastText(String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::NoModelPath => f.write_str("model_path is required"),
            LoadError::FastText(e) => write!(f, "fastText load failed: {e}"),
        }
    }
}

impl std::error::Error for LoadError {}

fn scrub_newlines(s: &str) -> String {
    s.chars()
        .map(|c| if c == '\n' || c == '\r' { ' ' } else { c })
        .collect()
}

fn map_label(raw: &str, has_hant: bool) -> Option<&'static str> {
    let code = raw.strip_prefix("__label__").unwrap_or(raw);
    match code {
        "en" => Some("en-US"),
        "vi" => Some("vi-VN"),
        "ja" => Some("ja"),
        "ko" => Some("ko"),
        // ISO 639 → BCP 47 disambiguation for the umbrella `zh` label.
        // See module doc-comment.
        "zh" => Some(if has_hant { "zh-Hant" } else { "zh-Hans" }),
        _ => None,
    }
}

fn contains_hant_char(text: &str) -> bool {
    text.chars().any(orthography::is_hant_only_char)
}

fn is_supported_lid176_code(code: &str) -> bool {
    matches!(code, "en" | "vi" | "ja" | "ko" | "zh")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_mapping_supported() {
        assert_eq!(map_label("__label__en", false), Some("en-US"));
        assert_eq!(map_label("__label__vi", false), Some("vi-VN"));
        assert_eq!(map_label("__label__ja", false), Some("ja"));
        assert_eq!(map_label("__label__ko", false), Some("ko"));
    }

    #[test]
    fn label_mapping_routes_zh_by_hant_signal() {
        // ISO 639 `zh` with no Hant character defaults to Hans (the
        // modern default per the design rule).
        assert_eq!(map_label("__label__zh", false), Some("zh-Hans"));
        // When the input contains a Hant-only character, prefer Hant.
        assert_eq!(map_label("__label__zh", true), Some("zh-Hant"));
    }

    #[test]
    fn label_mapping_drops_unrelated_languages() {
        assert_eq!(map_label("__label__fr", false), None);
        assert_eq!(map_label("__label__de", false), None);
        assert_eq!(map_label("__label__zh-cn", false), None);
        assert_eq!(map_label("__label__", false), None);
    }

    #[test]
    fn label_mapping_tolerates_missing_prefix() {
        assert_eq!(map_label("en", false), Some("en-US"));
    }

    #[test]
    fn contains_hant_char_detects_hant_only() {
        // `學` is in orthography's `is_hant_only` table.
        assert!(contains_hant_char("學生"));
    }

    #[test]
    fn contains_hant_char_misses_shared_or_hans_only() {
        // `学生` uses the shinjitai/Hans form `学` — not Hant-only.
        assert!(!contains_hant_char("学生"));
        // Pure ASCII: no Han chars at all.
        assert!(!contains_hant_char("Hello world"));
    }

    #[test]
    fn is_supported_treats_zh_as_supported() {
        assert!(is_supported_lid176_code("zh"));
    }

    #[test]
    fn is_supported_rejects_unsupported_codes() {
        assert!(!is_supported_lid176_code("fr"));
        assert!(!is_supported_lid176_code("de"));
        assert!(!is_supported_lid176_code("es"));
        assert!(!is_supported_lid176_code(""));
    }

    #[test]
    fn scrub_newlines_replaces_cr_and_lf() {
        assert_eq!(scrub_newlines("a\nb\rc\r\nd"), "a b c  d");
    }

    #[test]
    fn builder_without_path_errors() {
        let r = FastTextClassifier::builder().build();
        assert!(matches!(r, Err(LoadError::NoModelPath)));
    }

    #[test]
    fn builder_with_bogus_path_errors() {
        let r = FastTextClassifier::builder()
            .model_path("/nonexistent/path/to/lid.176.bin")
            .build();
        assert!(matches!(r, Err(LoadError::FastText(_))));
    }
}
