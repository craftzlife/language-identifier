//! Caller-supplied configuration for [`crate::identify_with`] and
//! [`crate::identify_lines_with`].
//!
//! The struct is the single place to opt into the probabilistic layers
//! the library ships. The default value disables everything optional —
//! `identify(input)` is equivalent to
//! `identify_with(input, &IdentifyOptions::default())` and is v3-stable.

use crate::layers::ml::MlClassifier;

/// Optional per-call configuration.
///
/// Trait objects are borrowed for the duration of the call, so a
/// short-lived options value built per-request allocates nothing.
/// Callers that want to stash configuration in a long-lived struct
/// should hold their classifier behind their own `Box` / `Arc` and
/// re-borrow when building the options.
#[derive(Default)]
pub struct IdentifyOptions<'a> {
    /// Layer 9 hook. When `Some`, the classifier is consulted once and
    /// its scores are folded into the candidate distribution via a
    /// capped bonus (see `aggregate.rs`). When `None`, Layer 9 is
    /// skipped — the v3 behavior.
    pub ml_classifier: Option<&'a dyn MlClassifier>,
}
