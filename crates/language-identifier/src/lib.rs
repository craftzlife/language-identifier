//! Language Identifier — layered identification of natural language from text input.
//!
//! See `SOFTWARE_DESIGN.md` at the repository root for the full design.
//! v1 implements Layers 0–3 + final calibration; later layers (dictionary, context,
//! ML, LLM) are stubs to be added without breaking the public API.

mod aggregate;
mod calibration;
mod layers;
mod llm;
mod options;
mod pipeline;
mod segments;
mod types;

pub use layers::ml::{MlClassifier, UnsupportedSignal};
#[cfg(feature = "ml-fasttext")]
pub use layers::ml::fasttext::{FastTextClassifier, LoadError as FastTextLoadError};
pub use llm::LlmResolver;
pub use options::IdentifyOptions;
pub use types::{Candidate, IdentifyResult, Segment, Status};

/// Identify the language of a single string using the default
/// configuration (Layers 0–7 only — Layer 9/10 disabled).
pub fn identify(input: &str) -> IdentifyResult {
    pipeline::run(input)
}

/// Identify the language of a single string with caller-supplied
/// [`IdentifyOptions`]. Use this to opt into Layer 9 (ML classifier) or
/// Layer 10 (LLM resolver) by providing trait implementations.
pub fn identify_with(input: &str, opts: &IdentifyOptions<'_>) -> IdentifyResult {
    pipeline::run_with(input, opts)
}

/// Identify the language of a multi-line input. Lines are joined with a single
/// newline before running through the pipeline; the pipeline's Layer 0 then
/// normalizes whitespace.
pub fn identify_lines<S: AsRef<str>>(lines: &[S]) -> IdentifyResult {
    identify_lines_with(lines, &IdentifyOptions::default())
}

/// Identify the language of a multi-line input with caller-supplied
/// [`IdentifyOptions`].
pub fn identify_lines_with<S: AsRef<str>>(
    lines: &[S],
    opts: &IdentifyOptions<'_>,
) -> IdentifyResult {
    let joined = lines
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>()
        .join("\n");
    pipeline::run_with(&joined, opts)
}
