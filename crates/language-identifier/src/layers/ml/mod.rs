//! Layer 9 — Lightweight ML classifier.
//!
//! The trait is always available. The default implementation
//! [`openlid::OpenLidClassifier`] (backed by the OpenLID v3 fastText
//! model, `openlid-v3.bin`, 194 languages with `iso639-3_Script`
//! labels) ships behind the `ml-openlid` cargo feature. Callers can
//! also plug in their own classifier via
//! [`crate::IdentifyOptions::ml_classifier`].
//!
//! The classifier is consulted once per call, after Layers 0–7 have
//! produced their signals and before aggregation. Its scores are folded
//! into the candidate distribution as a capped bonus (see
//! `aggregate.rs`), so a well-calibrated ML model can outweigh the
//! lexical layers but cannot single-handedly override a strong script
//! signal.

#[cfg(feature = "ml-openlid")]
pub mod openlid;

/// Per-language language-identification confidence over the supported
/// BCP 47 tag set.
///
/// Implementors return `(language, confidence)` pairs where `confidence`
/// is in `[0.0, 1.0]`. Languages outside the library's supported set
/// (see SDD §6) are silently dropped during aggregation; returning them
/// is harmless.
pub trait MlClassifier: Send + Sync {
    /// Classify the normalized input text. Called at most once per
    /// `identify_with` call.
    ///
    /// The input is the **normalized** form — NFC-composed,
    /// whitespace-collapsed, control-stripped — matching what every
    /// other layer sees. Empty input is filtered earlier in the
    /// pipeline and never reaches this method.
    fn classify(&self, normalized_text: &str) -> Vec<(String, f32)>;

    /// Optional hook: signal that the classifier is confident the
    /// input is in a language outside the library's supported set.
    ///
    /// Returning `Some(signal)` lets the pipeline downgrade the verdict
    /// to [`crate::Status::Unsupported`] instead of forcing the input
    /// into a supported tag.
    ///
    /// The default impl returns `None`. Existing custom classifiers
    /// keep working unchanged; only implementors that know their
    /// model's full label set need to override.
    fn unsupported_signal(&self, _normalized_text: &str) -> Option<UnsupportedSignal> {
        None
    }
}

/// Diagnostic value returned by [`MlClassifier::unsupported_signal`]
/// when a classifier is confident the input is in a language outside
/// the library's supported BCP 47 set.
#[derive(Debug, Clone)]
pub struct UnsupportedSignal {
    /// Label the classifier uses internally (e.g. `"xxx_Yyyy"`).
    /// Surfaced verbatim in the `reasons` array; the pipeline does not
    /// interpret it as a BCP 47 tag.
    pub label: String,
    /// Confidence in `[0.0, 1.0]`. The pipeline downgrades to
    /// `Unsupported` only when this is at or above the threshold
    /// declared in `pipeline.rs`.
    pub confidence: f32,
}
