//! Layer 9 — Lightweight ML classifier.
//!
//! v4 ships the trait only. A default implementation backed by
//! `fastText lid.176.bin` will land in v4.1 behind the `ml-fasttext`
//! cargo feature. Callers can already plug in their own classifier via
//! [`crate::IdentifyOptions::ml_classifier`].
//!
//! The classifier is consulted once per call, after Layers 0–7 have
//! produced their signals and before aggregation. Its scores are folded
//! into the candidate distribution as a capped bonus (see
//! `aggregate.rs`), so a well-calibrated ML model can outweigh the
//! lexical layers but cannot single-handedly override a strong script
//! signal.

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
}
