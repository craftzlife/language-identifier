//! UniFFI bindings for `language-identifier`.
//!
//! Generates idiomatic Swift / Kotlin / Python / Ruby bindings from one
//! Rust source via UniFFI's proc-macro mode. The wire types mirror the
//! core crate's `IdentifyResult`, `Candidate`, `Segment`, and `Status`
//! so the underlying library stays UniFFI-unaware.
//!
//! Build artifacts (static + dynamic) live in `target/<triple>/release/`
//! once cross-compiled. The companion scripts under `scripts/` assemble
//! these into platform-appropriate packages (XCFramework for Apple,
//! `jniLibs/` for Android).

use language_identifier as li;

mod c_abi;

uniffi::setup_scaffolding!();

#[derive(uniffi::Enum)]
pub enum Status {
    Resolved,
    Ambiguous,
    Mixed,
    Unknown,
    Unsupported,
}

#[derive(uniffi::Record)]
pub struct Candidate {
    pub language: String,
    pub confidence: f32,
}

#[derive(uniffi::Record)]
pub struct Segment {
    pub language: String,
    pub start: u64,
    pub end: u64,
    pub text: String,
    pub tokens: Vec<Token>,
}

#[derive(uniffi::Record)]
pub struct Token {
    pub text: String,
    pub start: u64,
    pub end: u64,
}

#[derive(uniffi::Record)]
pub struct IdentifyResult {
    pub candidates: Vec<Candidate>,
    pub primary_language: Option<String>,
    pub status: Status,
    pub reasons: Vec<String>,
    pub segments: Vec<Segment>,
    pub normalized_text: String,
}

#[uniffi::export]
pub fn identify(input: String) -> IdentifyResult {
    li::identify(&input).into()
}

#[uniffi::export]
pub fn identify_lines(lines: Vec<String>) -> IdentifyResult {
    li::identify_lines(&lines).into()
}

impl From<li::Status> for Status {
    fn from(s: li::Status) -> Self {
        match s {
            li::Status::Resolved => Status::Resolved,
            li::Status::Ambiguous => Status::Ambiguous,
            li::Status::Mixed => Status::Mixed,
            li::Status::Unknown => Status::Unknown,
            li::Status::Unsupported => Status::Unsupported,
        }
    }
}

impl From<li::Candidate> for Candidate {
    fn from(c: li::Candidate) -> Self {
        Candidate {
            language: c.language,
            confidence: c.confidence,
        }
    }
}

impl From<li::Segment> for Segment {
    fn from(s: li::Segment) -> Self {
        Segment {
            language: s.language,
            start: s.start as u64,
            end: s.end as u64,
            text: s.text,
            tokens: s.tokens.into_iter().map(Into::into).collect(),
        }
    }
}

impl From<li::Token> for Token {
    fn from(t: li::Token) -> Self {
        Token {
            text: t.text,
            start: t.start as u64,
            end: t.end as u64,
        }
    }
}

impl From<li::IdentifyResult> for IdentifyResult {
    fn from(r: li::IdentifyResult) -> Self {
        IdentifyResult {
            candidates: r.candidates.into_iter().map(Into::into).collect(),
            primary_language: r.primary_language,
            status: r.status.into(),
            reasons: r.reasons,
            segments: r.segments.into_iter().map(Into::into).collect(),
            normalized_text: r.normalized_text,
        }
    }
}
