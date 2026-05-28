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
    pub language: String,
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Reason {
    Single(String),
    Lines(Vec<String>),
}

impl Reason {
    pub fn from_notes(mut notes: Vec<String>) -> Self {
        notes.retain(|n| !n.is_empty());
        match notes.len() {
            0 => Reason::Single(String::new()),
            1 => Reason::Single(notes.pop().unwrap()),
            _ => Reason::Lines(notes),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Segment {
    pub language: String,
    /// Byte offset into the normalized input text, inclusive.
    pub start: usize,
    /// Byte offset into the normalized input text, exclusive.
    pub end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentifyResult {
    pub candidates: Vec<Candidate>,
    #[serde(rename = "primaryLanguage", skip_serializing_if = "Option::is_none")]
    pub primary_language: Option<String>,
    pub status: Status,
    pub reason: Reason,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<Segment>,
}
