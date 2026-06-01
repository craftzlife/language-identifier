//! End-to-end tests for French (`fr`) support.

use language_identifier::{identify, identify_lines, Status};

fn top_lang(r: &language_identifier::IdentifyResult) -> &str {
    r.candidates
        .first()
        .map(|c| c.language.as_str())
        .unwrap_or("")
}

fn has_lang(r: &language_identifier::IdentifyResult, lang: &str) -> bool {
    r.candidates.iter().any(|c| c.language == lang)
}

#[test]
fn pure_french_phrase_resolves_to_fr() {
    // Use a phrase whose content words are clearly French (not en/fr
    // overlaps like "chat"/"table"). "manger", "fenêtre", "matin" are
    // unambiguous fr tokens.
    let r = identify("je voudrais manger près de la fenêtre ce matin");
    assert_eq!(r.status, Status::Resolved, "{r:?}");
    assert_eq!(top_lang(&r), "fr", "{r:?}");
}

#[test]
fn french_phrase_with_loanword_overlap_keeps_fr_primary() {
    // "le chat est sur la table" — every content word is an en/fr loanword
    // pair, so the dictionary signal alone splits ambiguously. The fr
    // function-word density (le/est/sur/la) should still pick fr as
    // primary even if the status downgrades to Mixed.
    let r = identify("le chat est sur la table");
    assert_eq!(top_lang(&r), "fr", "{r:?}");
}

#[test]
fn french_with_cedilla_resolves_to_fr() {
    // ç is a strong French marker among supported Latin langs.
    let r = identify("garçon français");
    assert_eq!(top_lang(&r), "fr", "{r:?}");
}

#[test]
fn french_with_oe_ligature_resolves_to_fr() {
    let r = identify("le cœur de l'œuvre");
    assert_eq!(top_lang(&r), "fr", "{r:?}");
}

#[test]
fn french_paragraph_resolves_to_fr() {
    let r = identify_lines(&[
        "Le professeur enseigne à l'université.",
        "Les étudiants viennent en classe tous les jours et apprennent du nouveau vocabulaire.",
        "Il est très gentil et répond toujours aux questions difficiles.",
    ]);
    assert_eq!(r.status, Status::Resolved, "{r:?}");
    assert_eq!(top_lang(&r), "fr", "{r:?}");
}

#[test]
fn english_with_embedded_french_phrase_keeps_english_primary() {
    // English carrier, short French embedded phrase — French should still
    // surface as a candidate but English remains primary.
    let r = identify(
        "When you read the phrase je ne sais quoi in an English book, the meaning is borrowed from French.",
    );
    assert_eq!(top_lang(&r), "en", "{r:?}");
    assert!(has_lang(&r, "fr"), "expected fr candidate: {r:?}");
}

#[test]
fn balanced_french_english_is_mixed_or_resolved_fr() {
    // Roughly equal English and French content — the carrier varies by
    // sentence. We accept either status, but fr must be among candidates.
    let r = identify_lines(&[
        "This is an English sentence about programming.",
        "Voici une phrase française qui parle de la programmation.",
    ]);
    assert!(has_lang(&r, "fr"), "expected fr candidate: {r:?}");
    assert!(has_lang(&r, "en"), "expected en candidate: {r:?}");
}

#[test]
fn french_does_not_misattribute_pure_vietnamese() {
    // Vietnamese must not get misclassified as fr now that fr is in the
    // lexicon set.
    let r = identify("giáo viên đại học tiếng Việt");
    assert_eq!(top_lang(&r), "vi", "{r:?}");
}

#[test]
fn french_does_not_misattribute_pure_english() {
    let r = identify("university teacher and student of english");
    assert_eq!(top_lang(&r), "en", "{r:?}");
}
