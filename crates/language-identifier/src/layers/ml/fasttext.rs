//! `FastTextClassifier` — Layer 9 default implementation.
//!
//! Wraps Meta/FAIR's `lid.176.bin` (full-precision 176-language
//! language-identification model) via the pure-Rust [`fasttext`] crate.
//!
//! ## What this does and does not contribute
//!
//! The classifier delegates ISO 639 → BCP 47 translation to
//! [`crate::bcp47`], which holds the canonical mapping table and the
//! Tier 2 disambiguation logic (e.g. `zh` → `zh-Hans` / `zh-Hant`).
//! Predictions whose label is outside the supported set are filtered
//! out — see the module doc on `crate::bcp47` for the supported tags
//! and the extension model.

use std::path::PathBuf;

use ::fasttext::FastText;

use super::{MlClassifier, UnsupportedSignal};
use crate::bcp47;

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
        // fastText is single-line; the Tier 2 disambiguator needs the
        // original normalized text so its Hant-character scan sees
        // exactly what every other layer sees.
        self.model
            .predict(&cleaned, self.top_k, 0.0)
            .into_iter()
            .filter_map(|p| {
                bcp47::from_iso639(&p.label, normalized_text).map(|tag| (tag.to_string(), p.prob))
            })
            .collect()
    }

    fn unsupported_signal(&self, normalized_text: &str) -> Option<UnsupportedSignal> {
        let cleaned = scrub_newlines(normalized_text);
        let top = self.model.predict(&cleaned, 1, 0.0).into_iter().next()?;
        // `zh` IS in the supported set — variant disambiguation belongs
        // to `bcp47::from_iso639`. Treating it as "unsupported" here
        // would wrongly downgrade Han inputs.
        if bcp47::is_supported_iso639(&top.label) {
            None
        } else {
            let code = top.label.strip_prefix("__label__").unwrap_or(&top.label);
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

#[cfg(test)]
mod tests {
    use super::*;

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
