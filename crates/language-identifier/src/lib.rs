//! Language Identifier — layered identification of natural language from text input.
//!
//! See `SOFTWARE_DESIGN.md` at the repository root for the full design.
//! v1 implements Layers 0–3 + final calibration; later layers (dictionary, context,
//! ML, LLM) are stubs to be added without breaking the public API.

mod aggregate;
mod calibration;
mod layers;
mod pipeline;
mod segments;
mod types;

pub use types::{Candidate, IdentifyResult, Segment, Status};

/// Identify the language of a single string.
pub fn identify(input: &str) -> IdentifyResult {
    pipeline::run(input)
}

/// Identify the language of a multi-line input. Lines are joined with a single
/// newline before running through the pipeline; the pipeline's Layer 0 then
/// normalizes whitespace.
pub fn identify_lines<S: AsRef<str>>(lines: &[S]) -> IdentifyResult {
    let joined = lines
        .iter()
        .map(|s| s.as_ref())
        .collect::<Vec<_>>()
        .join("\n");
    pipeline::run(&joined)
}
