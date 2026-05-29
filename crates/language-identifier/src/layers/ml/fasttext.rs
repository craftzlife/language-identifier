//! `FastTextClassifier` — Layer 9 default implementation.
//!
//! Wraps Meta/FAIR's `lid.176.bin` (full-precision 176-language
//! language-identification model) via the pure-Rust [`fasttext`] crate.
//!
//! ## What this does and does not contribute
//!
//! The classifier maps fastText's coarse language labels into the
//! library's BCP 47 set:
//!
//! | fastText label    | mapped tag |
//! | ----------------- | ---------- |
//! | `__label__en`     | `en-US`    |
//! | `__label__vi`     | `vi-VN`    |
//! | `__label__ja`     | `ja`       |
//! | `__label__ko`     | `ko`       |
//! | `__label__zh`     | *dropped*  |
//! | all 171 others    | *dropped*  |
//!
//! `__label__zh` is intentionally dropped — lid.176 does not split
//! `zh-Hans` vs `zh-Hant`, and Hans/Hant disambiguation is the
//! deterministic job of Layer 3 (orthography markers). The ML model
//! earns its keep on cross-script ties (en vs vi, ja vs zh in the
//! aggregate), not on variant decisions inside Han.

use std::path::PathBuf;

use ::fasttext::FastText;

use super::{MlClassifier, UnsupportedSignal};

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
        self.model
            .predict(&cleaned, self.top_k, 0.0)
            .into_iter()
            .filter_map(|p| map_label(&p.label).map(|tag| (tag.to_string(), p.prob)))
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

fn map_label(raw: &str) -> Option<&'static str> {
    let code = raw.strip_prefix("__label__").unwrap_or(raw);
    match code {
        "en" => Some("en-US"),
        "vi" => Some("vi-VN"),
        "ja" => Some("ja"),
        "ko" => Some("ko"),
        // `zh` intentionally dropped from classify; see module
        // doc-comment. It is still treated as "supported" by
        // `is_supported_lid176_code` so the unsupported-signal path
        // does not wrongly downgrade Han inputs.
        _ => None,
    }
}

fn is_supported_lid176_code(code: &str) -> bool {
    matches!(code, "en" | "vi" | "ja" | "ko" | "zh")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_mapping_supported() {
        assert_eq!(map_label("__label__en"), Some("en-US"));
        assert_eq!(map_label("__label__vi"), Some("vi-VN"));
        assert_eq!(map_label("__label__ja"), Some("ja"));
        assert_eq!(map_label("__label__ko"), Some("ko"));
    }

    #[test]
    fn label_mapping_drops_zh() {
        assert_eq!(map_label("__label__zh"), None);
    }

    #[test]
    fn label_mapping_drops_unrelated_languages() {
        assert_eq!(map_label("__label__fr"), None);
        assert_eq!(map_label("__label__de"), None);
        assert_eq!(map_label("__label__zh-cn"), None);
        assert_eq!(map_label("__label__"), None);
    }

    #[test]
    fn label_mapping_tolerates_missing_prefix() {
        assert_eq!(map_label("en"), Some("en-US"));
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
