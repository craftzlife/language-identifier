use crate::types::{Candidate, Status};

// Tuned against the SDD test cases:
// - >= 0.95 dominance ⇒ resolved with a single candidate (G1-G4, P1-P4, W2).
// - gap >= 0.30 between #1 and #2 ⇒ resolved, keep top + supporting candidates (M1, M2).
// - otherwise ⇒ ambiguous (W1, M3, M4 expectations).
const DOMINANT_THRESHOLD: f32 = 0.95;
const RESOLVED_GAP: f32 = 0.30;
// 0.01 surfaces embedded snippets that round to 1% (matches the M4 case where
// Vietnamese markers cover ~1% of input and should still appear as a candidate).
const KEEP_FLOOR: f32 = 0.01;

pub struct Calibrated {
    pub status: Status,
    pub candidates: Vec<Candidate>,
    pub primary_language: Option<String>,
    pub note: String,
}

pub fn calibrate(mut ranked: Vec<Candidate>, total_visible: usize) -> Calibrated {
    if total_visible == 0 || ranked.is_empty() {
        return Calibrated {
            status: Status::Unknown,
            candidates: vec![],
            primary_language: None,
            note: "No supported script detected in input".into(),
        };
    }

    // Drop noise candidates.
    ranked.retain(|c| c.confidence >= KEEP_FLOOR);
    if ranked.is_empty() {
        return Calibrated {
            status: Status::Unknown,
            candidates: vec![],
            primary_language: None,
            note: "No candidate above confidence floor".into(),
        };
    }

    let top_lang = ranked[0].language.clone();
    let top_conf = ranked[0].confidence;
    let second_conf = ranked.get(1).map(|c| c.confidence).unwrap_or(0.0);
    let gap = top_conf - second_conf;

    if top_conf >= DOMINANT_THRESHOLD || gap >= RESOLVED_GAP {
        let note = if top_conf >= DOMINANT_THRESHOLD && second_conf < KEEP_FLOOR {
            format!("Single language detected: {}", top_lang)
        } else {
            format!(
                "{} is primary (confidence {:.2}); remaining candidates treated as embedded segments",
                top_lang, top_conf
            )
        };
        return Calibrated {
            status: Status::Resolved,
            candidates: ranked,
            primary_language: Some(top_lang),
            note,
        };
    }

    Calibrated {
        status: Status::Ambiguous,
        candidates: ranked,
        primary_language: Some(top_lang.clone()),
        note: format!(
            "Confidence gap is small ({:.2}); detection status is 'ambiguous'",
            gap
        ),
    }
}
