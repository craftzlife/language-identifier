use language_identifier::{identify, identify_with, IdentifyOptions, MlClassifier};

/// Trivial `MlClassifier` that ignores the input and returns a fixed
/// score list. Useful for verifying the L9 hook routes through
/// aggregate.
struct FixedMl(Vec<(String, f32)>);
impl MlClassifier for FixedMl {
    fn classify(&self, _: &str) -> Vec<(String, f32)> {
        self.0.clone()
    }
}

#[test]
fn default_options_match_identify() {
    for input in [
        "",
        "Hello",
        "giáo viên đại học",
        "田中先生は大学で日本語を教えています。",
        "学",
        "Hello こんにちは world",
    ] {
        let a = identify(input);
        let b = identify_with(input, &IdentifyOptions::default());
        assert_eq!(a.status, b.status, "status diverged for {input:?}");
        assert_eq!(
            a.primary_language, b.primary_language,
            "primary diverged for {input:?}"
        );
        let aa: Vec<_> = a.candidates.iter().map(|c| &c.language).collect();
        let bb: Vec<_> = b.candidates.iter().map(|c| &c.language).collect();
        assert_eq!(aa, bb, "candidate order diverged for {input:?}");
        assert_eq!(a.reasons, b.reasons, "reasons diverged for {input:?}");
    }
}

#[test]
fn ml_classifier_influences_candidate_ordering() {
    // W1-style ambiguous Han input: with no markers, script aggregation
    // splits Han ~evenly across ja / zh-Hans / zh-Hant. A strong ML
    // signal toward zh-Hans should reorder the candidates.
    let input = "学";
    let baseline = identify(input);
    assert!(
        matches!(baseline.primary_language.as_deref(), Some("ja") | Some("zh")),
        "unexpected baseline primary: {:?}",
        baseline.primary_language
    );

    let ml = FixedMl(vec![("zh-Hans".into(), 1.0)]);
    let opts = IdentifyOptions {
        ml_classifier: Some(&ml),
    };
    let with_ml = identify_with(input, &opts);
    assert_eq!(
        with_ml.primary_language.as_deref(),
        Some("zh"),
        "ML bonus failed to make zh-Hans push zh to primary: {:?}",
        with_ml.candidates
    );
    assert_eq!(
        with_ml.primary_variant.as_deref(),
        Some("zh-Hans"),
        "ML bonus failed to surface zh-Hans as primary variant: {:?}",
        with_ml
    );
    assert!(
        with_ml
            .reasons
            .iter()
            .any(|r| r.contains("Layer 9 (ML classifier)")),
        "expected Layer 9 reason, got: {:?}",
        with_ml.reasons
    );
}
