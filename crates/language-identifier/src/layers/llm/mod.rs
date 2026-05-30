//! Layer 10 — LLM resolver.
//!
//! This module owns the [`LlmResolver`] trait plus optional, feature-gated
//! default implementations. Implementations live in per-backend submodules
//! (currently `apple_foundation`); the trait is the only public surface
//! the pipeline depends on.
//!
//! Per SDD §7.2, the resolver is consulted **only when calibration produces
//! [`Status::Ambiguous`](crate::Status::Ambiguous)** — i.e., the confidence
//! gap between the top two candidates is small. For `Resolved`, `Mixed`,
//! `Unknown`, and `Unsupported` results the resolver is skipped entirely.
//!
//! The resolver may refine `primaryLanguage` but does **not** change
//! `status`. Even after LLM refinement, the result remains `Ambiguous` —
//! the LLM's opinion is recorded as one more reason in the `reasons`
//! array.

use crate::types::Candidate;

#[cfg(all(target_os = "macos", feature = "llm-apple-foundation"))]
pub mod apple_foundation;

/// Disambiguator for ambiguous candidate sets.
///
/// Per SDD §10's fallback rule, a resolver that fails or returns no
/// opinion leaves the pre-LLM ranking intact. Errors and timeouts should
/// be handled inside the implementation and surfaced as `None` — the
/// library does not expose an error channel from this trait.
pub trait LlmResolver: Send + Sync {
    /// Choose a primary language from the calibrated candidate set, or
    /// return `None` to defer to the pre-LLM ranking.
    ///
    /// `candidates` is the post-calibration ranking, sorted by confidence
    /// descending. Implementors typically inspect the top 2–3 candidates
    /// and the input text to break the tie.
    ///
    /// Returning a language tag outside `candidates` is permitted but
    /// discouraged — it surfaces in the output without an accompanying
    /// confidence value from the deterministic layers.
    fn resolve(&self, normalized_text: &str, candidates: &[Candidate]) -> Option<String>;
}
