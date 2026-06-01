use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    Resolved,
    Ambiguous,
    Mixed,
    Unknown,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candidate {
    /// Consumer-facing macro language tag (`zh`, `en`, `fr`, `vi`,
    /// `ja`, `ko`). Apps that don't care about Hans/Hant or Cantonese
    /// distinctions should branch on this field.
    pub language: String,
    /// Fine-grained BCP 47 tags this macro represents
    /// (`zh-Hans` / `zh-Hant` / `zh-Hant-HK` / `zh-Hant-TW` / `yue` /
    /// `lzh` / `nan` / `hak` / `wuu` for the `zh` macro; just
    /// `[language]` for monolithic macros). Lists what the library
    /// *can* decide, not necessarily what fired in this input — see
    /// [`crate::variants_of`].
    #[serde(default)]
    pub variants: Vec<String>,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Segment {
    pub language: String,
    /// Byte offset into the normalized input text, inclusive.
    pub start: usize,
    /// Byte offset into the normalized input text, exclusive.
    pub end: usize,
    /// The actual text of this segment, i.e. the slice of the normalized
    /// input between `start` and `end`. Carried in the output so callers can
    /// read the segment without holding the original normalized text.
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifyResult {
    pub candidates: Vec<Candidate>,
    /// Macro language tag (`zh`, `en`, `fr`, `vi`, `ja`, `ko`) of the
    /// primary candidate.
    #[serde(rename = "primaryLanguage", skip_serializing_if = "Option::is_none")]
    pub primary_language: Option<String>,
    /// Fine-grained BCP 47 tag (`zh-Hant`, `zh-Hant-HK`, `yue`, `lzh`,
    /// …) of the highest-confidence variant within the primary macro
    /// language. Equals `primary_language` for monolithic macros (e.g.
    /// `en`, `fr`). Omitted when status is `unknown` / `unsupported`.
    #[serde(rename = "primaryVariant", skip_serializing_if = "Option::is_none")]
    pub primary_variant: Option<String>,
    pub status: Status,
    pub reasons: Vec<String>,
    #[serde(default)]
    pub segments: Vec<Segment>,
    /// The NFC-composed, whitespace-collapsed, control-stripped form of the
    /// input that the pipeline actually analyzed. `Segment.start` and
    /// `Segment.end` index into this string.
    #[serde(rename = "normalizedText")]
    pub normalized_text: String,
}
