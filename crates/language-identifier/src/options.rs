//! Caller-supplied configuration for [`crate::identify_with`] and
//! [`crate::identify_lines_with`].
//!
//! v4 introduces this struct as the single place to opt into the
//! probabilistic layers (Layer 9 ML classifier, Layer 10 LLM resolver).
//! The default value disables both — `identify(input)` is equivalent to
//! `identify_with(input, &IdentifyOptions::default())` and is v3-stable.
//!
//! Per SDD §11, this struct also satisfies the "configuration to
//! disable Layer 10" requirement: opting out is the default.

use crate::layers::ml::MlClassifier;
use crate::llm::LlmResolver;

/// Optional per-call configuration.
///
/// Trait objects are borrowed for the duration of the call, so a
/// short-lived options value built per-request allocates nothing.
/// Callers that want to stash configuration in a long-lived struct
/// should hold their classifier/resolver behind their own
/// `Box` / `Arc` and re-borrow when building the options.
#[derive(Default)]
pub struct IdentifyOptions<'a> {
    /// Layer 9 hook. When `Some`, the classifier is consulted once and
    /// its scores are folded into the candidate distribution via a
    /// capped bonus (see `aggregate.rs`). When `None`, Layer 9 is
    /// skipped — the v3 behavior.
    pub ml_classifier: Option<&'a dyn MlClassifier>,

    /// Layer 10 hook. When `Some`, the resolver is consulted **only
    /// when** calibration produces `Status::Ambiguous`. When `None`,
    /// Layer 10 is skipped — ambiguous inputs stay ambiguous.
    pub llm_resolver: Option<&'a dyn LlmResolver>,
}
