//! Apple `FoundationModels` backend for [`LlmResolver`].
//!
//! Available only when the crate is built with
//! `--features llm-apple-foundation` on macOS 26+ (Apple Silicon
//! required by the framework itself). The implementation talks to a
//! tiny Swift static library — see `bridge/LanguageModelBridge.swift`
//! and `build.rs` — that exposes Apple's `SystemLanguageModel` over a
//! synchronous C ABI.
//!
//! Per SDD §11 (privacy), the on-device model means inputs never leave
//! the device — opting into this resolver does not weaken the offline
//! posture the same way a remote-API resolver would.

use std::ffi::{c_char, c_void, CString};
use std::sync::Mutex;

use crate::bcp47;
use crate::layers::llm::LlmResolver;
use crate::types::Candidate;

// --- C ABI from `LanguageModelBridge.swift` ------------------------------

#[link(name = "LanguageModelBridge", kind = "static")]
extern "C" {
    fn lid_apple_fm_is_available() -> bool;
    fn lid_apple_fm_session_create(instructions: *const c_char) -> *mut c_void;
    fn lid_apple_fm_session_destroy(session: *mut c_void);
    fn lid_apple_fm_respond(
        session: *mut c_void,
        prompt: *const c_char,
        out_buffer: *mut c_char,
        out_cap: usize,
        out_len: *mut usize,
    ) -> i32;
}

// Mirrors `BridgeStatus` in LanguageModelBridge.swift. Keep them in
// lockstep — bridge tests deliberately don't cover this, since the
// Swift enum is the source of truth.
const STATUS_OK: i32 = 0;
const STATUS_BUFFER_TOO_SMALL: i32 = 4;

// --- Public API ----------------------------------------------------------

/// Returned when the OS-shipped model is not usable on this machine —
/// Apple Intelligence is off, the hardware doesn't support it, the
/// model assets haven't downloaded yet, etc. Callers should treat this
/// as a soft failure and proceed without an LLM resolver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppleFoundationUnavailable;

impl std::fmt::Display for AppleFoundationUnavailable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            "Apple FoundationModels system model is not available on this device \
             (Apple Intelligence may be off, downloading, or unsupported)",
        )
    }
}

impl std::error::Error for AppleFoundationUnavailable {}

/// Default [`LlmResolver`] backed by Apple's on-device `SystemLanguageModel`.
///
/// The resolver holds a single long-lived session and serializes
/// requests through a mutex — `FoundationModels` rejects concurrent
/// requests against the same session.
pub struct AppleFoundationResolver {
    session: Mutex<SessionPtr>,
}

// SAFETY: the underlying session is accessed only via the mutex below;
// the Swift side declares the type as `@unchecked Sendable`.
struct SessionPtr(*mut c_void);
unsafe impl Send for SessionPtr {}

impl Drop for SessionPtr {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: pointer was returned by `lid_apple_fm_session_create`
            // and has not been freed before — `Drop` runs exactly once.
            unsafe { lid_apple_fm_session_destroy(self.0) };
            self.0 = std::ptr::null_mut();
        }
    }
}

impl AppleFoundationResolver {
    /// Open a session against the default system model. Returns
    /// [`AppleFoundationUnavailable`] when the model isn't usable on
    /// this device.
    pub fn new() -> Result<Self, AppleFoundationUnavailable> {
        // SAFETY: FFI call with no inputs.
        if !unsafe { lid_apple_fm_is_available() } {
            return Err(AppleFoundationUnavailable);
        }
        let instructions =
            CString::new(SYSTEM_INSTRUCTIONS).expect("system instructions contain no interior NUL");
        // SAFETY: `instructions` is a valid NUL-terminated C string for
        // the duration of the call.
        let raw = unsafe { lid_apple_fm_session_create(instructions.as_ptr()) };
        if raw.is_null() {
            return Err(AppleFoundationUnavailable);
        }
        Ok(Self {
            session: Mutex::new(SessionPtr(raw)),
        })
    }

    /// Drive the bridge with an explicit prompt. Exposed for tests and
    /// advanced callers who want to bypass the candidate-tag templating
    /// in `resolve`.
    pub fn respond(&self, prompt: &str) -> Option<String> {
        let c_prompt = CString::new(prompt).ok()?;
        let guard = self.session.lock().ok()?;
        respond_with_growing_buffer(guard.0, c_prompt.as_ptr())
    }
}

impl LlmResolver for AppleFoundationResolver {
    fn resolve(&self, normalized_text: &str, candidates: &[Candidate]) -> Option<String> {
        let prompt = build_prompt(normalized_text, candidates);
        let raw_reply = self.respond(&prompt)?;
        parse_response(&raw_reply, candidates)
    }
}

// --- Prompt + response handling ------------------------------------------

const SYSTEM_INSTRUCTIONS: &str = "\
You are a language-identification assistant. Given a snippet of text and a \
short list of candidate BCP 47 language tags, pick the single tag that best \
identifies the primary carrier language of the text. Respond with the tag \
only — no punctuation, no quotes, no explanation. If none of the candidates \
fit, respond with the word `none`.";

/// Cap on text fed to the model. Apple's on-device model has a finite
/// context window and Layer 10 is a tie-breaker for short ambiguous
/// inputs, not a long-form analyzer — truncating keeps latency bounded.
const MAX_TEXT_CHARS: usize = 2_000;

fn build_prompt(text: &str, candidates: &[Candidate]) -> String {
    let mut tags: Vec<&str> = candidates
        .iter()
        .map(|c| c.language.as_str())
        .filter(|t| !t.is_empty())
        .collect();
    tags.dedup();

    let truncated: String = text.chars().take(MAX_TEXT_CHARS).collect();
    let candidates_line = if tags.is_empty() {
        String::from("(no candidates provided)")
    } else {
        tags.join(", ")
    };
    format!(
        "Candidates: {candidates_line}\n\
         Text:\n{truncated}\n\
         Answer with one of the candidate tags only."
    )
}

fn parse_response(raw: &str, candidates: &[Candidate]) -> Option<String> {
    let trimmed = raw
        .trim()
        .trim_matches(|c: char| c == '`' || c == '"' || c == '\'' || c == '.' || c == ',');
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return None;
    }
    // Take only the first token — the model occasionally adds a brief
    // suffix despite the instruction.
    let head = trimmed
        .split(|c: char| c.is_whitespace())
        .next()
        .unwrap_or(trimmed);

    // Prefer an exact case-sensitive match against the candidate set,
    // since BCP 47 tags are case-significant (e.g. `zh-Hans` vs `zh-hans`).
    if candidates.iter().any(|c| c.language == head) {
        return Some(head.to_string());
    }
    // Fallback: case-insensitive match, normalizing back to the
    // canonical tag from the candidate list.
    if let Some(c) = candidates
        .iter()
        .find(|c| c.language.eq_ignore_ascii_case(head))
    {
        return Some(c.language.clone());
    }
    // Last resort: hand the model's ISO 639 code through the central
    // BCP 47 mapper so e.g. `zh` gets disambiguated using Tier 2.
    bcp47::from_iso639(head, "").map(|t| t.to_string())
}

fn respond_with_growing_buffer(session: *mut c_void, prompt: *const c_char) -> Option<String> {
    // Start with a reasonable buffer; grow on `bufferTooSmall`. The
    // model's classification responses are tiny (a single tag), so the
    // first allocation almost always wins.
    let mut cap: usize = 256;
    loop {
        let mut buf: Vec<u8> = vec![0; cap];
        let mut out_len: usize = 0;
        // SAFETY: `session` came from `lid_apple_fm_session_create`,
        // `prompt` is a valid C string for the duration of this call,
        // and `buf` is a writeable `cap`-byte allocation.
        let status = unsafe {
            lid_apple_fm_respond(
                session,
                prompt,
                buf.as_mut_ptr() as *mut c_char,
                cap,
                &mut out_len as *mut usize,
            )
        };
        match status {
            STATUS_OK => {
                buf.truncate(out_len);
                return String::from_utf8(buf).ok();
            }
            STATUS_BUFFER_TOO_SMALL => {
                if out_len <= cap {
                    // Bridge said too small but didn't ask for more —
                    // bail to avoid infinite loop.
                    return None;
                }
                cap = out_len;
            }
            _ => return None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cands(tags: &[&str]) -> Vec<Candidate> {
        tags.iter()
            .enumerate()
            .map(|(i, t)| Candidate {
                language: (*t).to_string(),
                confidence: 1.0 - (i as f32 * 0.1),
            })
            .collect()
    }

    #[test]
    fn prompt_lists_tags_then_text() {
        let p = build_prompt("hello world", &cands(&["ja", "zh-Hant"]));
        assert!(p.contains("Candidates: ja, zh-Hant"), "{p}");
        assert!(p.contains("hello world"), "{p}");
        assert!(p.contains("candidate tags only"), "{p}");
    }

    #[test]
    fn prompt_truncates_long_text() {
        // Mark the part that must survive vs the part that must be cut
        // with distinct characters so substring containment isn't
        // ambiguous.
        let kept: String = "あ".repeat(MAX_TEXT_CHARS);
        let dropped_marker = "SHOULD_NOT_APPEAR";
        let long = format!("{kept}{dropped_marker}");
        let p = build_prompt(&long, &cands(&["ja"]));
        assert!(p.contains(&kept));
        assert!(
            !p.contains(dropped_marker),
            "marker past MAX_TEXT_CHARS leaked into prompt: {p}"
        );
    }

    #[test]
    fn prompt_handles_empty_candidates() {
        let p = build_prompt("hi", &[]);
        assert!(p.contains("(no candidates provided)"), "{p}");
    }

    #[test]
    fn parse_exact_match_wins() {
        let got = parse_response("zh-Hant", &cands(&["ja", "zh-Hant"]));
        assert_eq!(got.as_deref(), Some("zh-Hant"));
    }

    #[test]
    fn parse_strips_quotes_and_punctuation() {
        let got = parse_response("`ja`.", &cands(&["ja", "zh-Hans"]));
        assert_eq!(got.as_deref(), Some("ja"));
    }

    #[test]
    fn parse_takes_first_token_only() {
        let got = parse_response("ja (Japanese)", &cands(&["ja", "zh-Hant"]));
        assert_eq!(got.as_deref(), Some("ja"));
    }

    #[test]
    fn parse_case_insensitive_match_normalizes_back() {
        let got = parse_response("zh-hant", &cands(&["ja", "zh-Hant"]));
        assert_eq!(got.as_deref(), Some("zh-Hant"));
    }

    #[test]
    fn parse_none_response_yields_none() {
        let got = parse_response("none", &cands(&["ja", "zh-Hant"]));
        assert_eq!(got, None);
    }

    #[test]
    fn parse_empty_response_yields_none() {
        assert_eq!(parse_response("", &cands(&["ja"])), None);
        assert_eq!(parse_response("   ", &cands(&["ja"])), None);
    }

    #[test]
    fn parse_unknown_iso_falls_through_to_bcp47() {
        // `en` isn't in the candidate set, but Tier 1 maps it to `en`,
        // so the central mapper's answer should pass through.
        let got = parse_response("en", &cands(&["ja", "zh-Hant"]));
        assert_eq!(got.as_deref(), Some("en"));
    }

    #[test]
    fn parse_unsupported_iso_yields_none() {
        let got = parse_response("fr", &cands(&["ja", "zh-Hant"]));
        assert_eq!(got, None);
    }
}
