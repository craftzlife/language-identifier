//! Per-segment tokenization. Each segment carries a `language` tag, and
//! this module dispatches to the tokenizer best suited to that language:
//!
//! * `jieba-rs` for any Chinese-family tag (`zh-Hans` / `zh-Hant` /
//!   `zh-Hant-HK` / `zh-Hant-TW` / `yue` / `lzh` / `nan` / `hak` /
//!   `wuu` / bare `zh`),
//! * `lindera` (IPADIC) for `ja`,
//! * `icu_segmenter` for everything else, including `ko`.
//!
//! Backend selection happens at runtime based on the parent segment's
//! language — there is one cargo feature, `tokenize`, that turns the
//! whole subsystem on or off. With it disabled the dispatcher returns
//! empty vecs and `Segment.tokens` is skipped during JSON serialization.
//!
//! Token offsets are translated from the tokenizer's local frame
//! (bytes into `segment.text`) into the segment's frame (bytes into
//! the normalized input), so they share the coordinate system of
//! `Segment.start` / `Segment.end`.

use crate::types::{Segment, Token};

mod icu;
mod ja;
mod zh;

/// Populate `segment.tokens` by running the appropriate per-language
/// tokenizer over `segment.text`. Token offsets are lifted into the
/// segment's absolute byte frame.
pub fn tokenize(segment: &mut Segment) {
    if segment.text.is_empty() {
        return;
    }
    let pairs: Vec<(String, usize, usize)> = match segment.language.as_str() {
        "zh" | "zh-Hans" | "zh-Hant" | "zh-Hant-HK" | "zh-Hant-TW" | "yue" | "lzh" | "nan"
        | "hak" | "wuu" => zh::tokenize(&segment.text),
        "ja" => ja::tokenize(&segment.text),
        _ => icu::tokenize(&segment.text),
    };
    segment.tokens = pairs
        .into_iter()
        .map(|(text, rel_start, rel_end)| Token {
            text,
            start: segment.start + rel_start,
            end: segment.start + rel_end,
        })
        .collect();
}
