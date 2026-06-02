use super::normalize::Normalized;
use super::script::{classify, Script};

#[derive(Debug, Clone, Default)]
pub struct OrthoSignals {
    pub vi_markers: usize,
    pub fr_markers: usize,
    pub hans_markers: usize,
    pub hant_markers: usize,
    /// Cantonese-only Han characters (嘅 嚟 唔 喺 啲 咁 哋 …).
    pub yue_markers: usize,
    /// Classical / Literary Chinese function-word characters (之 乎 者 也 …).
    pub lzh_markers: usize,
    /// Hong Kong-specific written-Chinese characters (嘥 冚 嘜 …).
    pub hant_hk_markers: usize,
    /// Taiwan-preferred Hant forms (臺 etc.).
    pub hant_tw_markers: usize,
    /// Min Nan (Hokkien) diagnostic chars (𪜶 𪜶 …) — sparse signal.
    pub nan_markers: usize,
    /// Hakka diagnostic chars — sparse signal.
    pub hak_markers: usize,
    /// Wu (Shanghainese) diagnostic chars (儂 嗰 阿拉 …) — sparse signal.
    pub wuu_markers: usize,
}

pub fn detect(input: &Normalized) -> OrthoSignals {
    detect_chars(&input.chars)
}

/// Same as [`detect`] but operates on a slice of chars. Used by
/// `segments::extract` to compute per-Han-run signals when deciding
/// whether to override the global Han attribution for an embedded
/// Chinese run inside a kana-dominated paragraph.
pub(crate) fn detect_chars(chars: &[char]) -> OrthoSignals {
    let mut sig = OrthoSignals::default();
    for &c in chars {
        if classify(c) == Script::Latin && is_vi_marker(c) {
            sig.vi_markers += 1;
        }
        if classify(c) == Script::Latin && is_fr_marker_char(c) {
            sig.fr_markers += 1;
        }
        if is_hans_only(c) {
            sig.hans_markers += 1;
        }
        if is_hant_only(c) {
            sig.hant_markers += 1;
        }
        if is_yue_marker(c) {
            sig.yue_markers += 1;
        }
        if is_lzh_marker(c) {
            sig.lzh_markers += 1;
        }
        if is_hant_hk_marker(c) {
            sig.hant_hk_markers += 1;
        }
        if is_hant_tw_marker(c) {
            sig.hant_tw_markers += 1;
        }
        if is_nan_marker(c) {
            sig.nan_markers += 1;
        }
        if is_hak_marker(c) {
            sig.hak_markers += 1;
        }
        if is_wuu_marker(c) {
            sig.wuu_markers += 1;
        }
    }
    sig
}

pub fn is_vi_marker_char(c: char) -> bool {
    is_vi_marker(c)
}

/// Whether `c` is a French-only letter among the library's supported
/// Latin-script set (`en`, `fr`, `vi`). Currently the cedilla `ç` and
/// the historical ligatures `œ`/`æ` — these are essentially absent from
/// English and Vietnamese orthography. Shared diacritics like `é`, `à`,
/// `è` are NOT included because they are not French-exclusive.
pub fn is_fr_marker_char(c: char) -> bool {
    matches!(c, 'ç' | 'Ç' | 'œ' | 'Œ' | 'æ' | 'Æ')
}

#[cfg_attr(not(feature = "ml-openlid"), allow(dead_code))]
pub fn is_hant_only_char(c: char) -> bool {
    is_hant_only(c)
}

#[cfg_attr(not(feature = "ml-openlid"), allow(dead_code))]
pub fn is_hant_hk_marker_char(c: char) -> bool {
    is_hant_hk_marker(c)
}

#[cfg_attr(not(feature = "ml-openlid"), allow(dead_code))]
pub fn is_hant_tw_marker_char(c: char) -> bool {
    is_hant_tw_marker(c)
}

/// Whether `text` contains at least one Cantonese-only Han character.
/// Used by [`crate::bcp47`] Tier 2 to escalate a coarse `zh` label to
/// the `yue` tag.
#[cfg_attr(not(feature = "ml-openlid"), allow(dead_code))]
pub fn has_yue_marker(text: &str) -> bool {
    text.chars().any(is_yue_marker)
}

/// Whether `text` looks like Classical / Literary Chinese (`lzh`).
/// Triggered when classical function-word characters reach a density
/// substantially higher than would be expected in modern Chinese prose.
#[cfg_attr(not(feature = "ml-openlid"), allow(dead_code))]
pub fn is_classical_chinese(text: &str) -> bool {
    let mut han_chars = 0usize;
    let mut lzh_hits = 0usize;
    for c in text.chars() {
        if classify(c) == Script::Han {
            han_chars += 1;
            if is_lzh_marker(c) {
                lzh_hits += 1;
            }
        }
    }
    // Need at least four Han chars to call density meaningfully, and at
    // least three classical markers — modern Chinese rarely strings 之/
    // 乎/者/也 together while Classical Chinese does so routinely.
    han_chars >= 4 && lzh_hits >= 3
}

fn is_vi_marker(c: char) -> bool {
    let cp = c as u32;
    if (0x1E00..=0x1EFF).contains(&cp) {
        return true;
    }
    matches!(
        c,
        'ă' | 'Ă' | 'đ' | 'Đ' | 'ơ' | 'Ơ' | 'ư' | 'Ư'
    )
}

// Han characters that are Simplified-only AND not used in modern Japanese.
fn is_hans_only(c: char) -> bool {
    matches!(
        c,
        '师' | '时'
            | '这'
            | '让'
            | '给'
            | '们'
            | '经'
            | '还'
            | '实'
            | '进'
            | '见'
            | '问'
            | '谁'
            | '长'
            | '说'
            | '应'
            | '听'
            | '个'
            | '观'
            | '议'
            | '态'
            | '资'
            | '产'
            | '风'
            | '调'
            | '语'
            | '请'
            | '认'
            | '识'
            | '记'
    )
}

// Han characters that are Traditional-only AND not used in modern Japanese.
fn is_hant_only(c: char) -> bool {
    matches!(
        c,
        '學' | '國' | '來' | '們' | '經' | '這' | '實' | '會' | '說' | '應'
            | '讓' | '與' | '沒' | '聽' | '寫' | '認' | '識' | '觀' | '議' | '態'
            | '資' | '產' | '風' | '調' | '請' | '記'
            | '權'
            | '體'
            | '龜'
            | '歲'
            | '數'
            | '關'
            | '舊'
            | '處'
            | '辭'
            | '戰'
            | '對'
            | '氣'
            | '灣'
            | '黨'
            | '齊'
            | '豐'
            | '滿'
            | '辦'
            | '齒'
            | '從'
            | '總'
            | '歷'
            | '當'
            | '兒'
            | '兩'
    )
}

// Cantonese-only Han characters: colloquial particles and pronouns
// that essentially do not appear in modern written Mandarin / Standard
// Chinese. A single hit escalates a coarse `zh` → `yue` in
// `bcp47::resolve_zh`.
fn is_yue_marker(c: char) -> bool {
    matches!(
        c,
        '嘅' | '嚟' | '唔' | '喺' | '啲' | '咁' | '哋' | '佢' | '係' | '咗' | '由' | '嘞' | '囉' | '仔'
    )
}

// Classical Chinese function-word characters. Density of these — not a
// single occurrence — marks Literary Chinese; the
// `is_classical_chinese` helper enforces the density gate.
fn is_lzh_marker(c: char) -> bool {
    matches!(
        c,
        '之' | '乎' | '者' | '也' | '矣' | '焉' | '哉' | '而' | '於' | '以' | '其' | '吾' | '汝' | '曰'
    )
}

// Hong Kong Standard-Written-Chinese characters. Deliberately disjoint
// from `is_yue_marker` (those are colloquial Cantonese particles).
fn is_hant_hk_marker(c: char) -> bool {
    matches!(c, '嘥' | '冚' | '嘜' | '攞' | '靚')
}

// Taiwan-preferred Hant forms. 臺 is the canonical 臺/台 split; the
// others are TW-flavor vocabulary.
fn is_hant_tw_marker(c: char) -> bool {
    matches!(c, '臺' | '麵' | '裡' | '著')
}

// Min Nan (Hokkien) diagnostic Han characters — sparse signal.
fn is_nan_marker(c: char) -> bool {
    matches!(c, '\u{2A736}' | '\u{3468}' | '阮' | '甲')
}

// Hakka diagnostic Han characters — sparse signal.
fn is_hak_marker(c: char) -> bool {
    matches!(c, '\u{2828E}' | '𢲷')
}

// Wu / Shanghainese diagnostic Han characters — sparse signal.
fn is_wuu_marker(c: char) -> bool {
    matches!(c, '儂' | '弗' | '勿')
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::normalize::normalize;

    #[test]
    fn detects_vi_diacritics() {
        let n = normalize("giáo viên đại học");
        let s = detect(&n);
        assert!(s.vi_markers >= 3);
    }

    #[test]
    fn no_vi_markers_in_english() {
        let n = normalize("university teacher");
        let s = detect(&n);
        assert_eq!(s.vi_markers, 0);
    }

    #[test]
    fn detects_hans_markers() {
        let n = normalize("大学老师");
        let s = detect(&n);
        assert!(s.hans_markers >= 1);
        assert_eq!(s.hant_markers, 0);
    }

    #[test]
    fn detects_hant_markers() {
        let n = normalize("學國來");
        let s = detect(&n);
        assert!(s.hant_markers >= 3);
        assert_eq!(s.hans_markers, 0);
    }

    #[test]
    fn detects_extended_hant_markers() {
        for word in ["權限", "體會", "關係", "戰爭", "對話", "氣候", "臺灣"] {
            let n = normalize(word);
            let s = detect(&n);
            assert!(
                s.hant_markers >= 1,
                "expected at least one Hant marker in {word:?}: {s:?}"
            );
            assert_eq!(
                s.hans_markers, 0,
                "{word:?} must not register as Hans: {s:?}"
            );
        }
    }

    #[test]
    fn japanese_kanji_with_kana_no_strong_hans() {
        let n = normalize("先生");
        let s = detect(&n);
        assert_eq!(s.hans_markers, 0);
        assert_eq!(s.hant_markers, 0);
    }

    #[test]
    fn vi_marker_includes_breve_and_horn() {
        let n = normalize("ăn cơm");
        let s = detect(&n);
        assert!(s.vi_markers >= 2);
    }

    #[test]
    fn french_diacritics_do_not_register_as_vi() {
        let n = normalize("café résumé naïve");
        let s = detect(&n);
        assert_eq!(s.vi_markers, 0);
    }

    #[test]
    fn french_cedilla_and_ligatures_register_as_fr_markers() {
        let n = normalize("garçon œuvre cœur");
        let s = detect(&n);
        assert!(s.fr_markers >= 3, "expected fr markers: {s:?}");
        assert_eq!(s.vi_markers, 0);
    }

    #[test]
    fn fr_marker_check_is_disjoint_from_vi() {
        // VI tone marks must not register as fr markers and vice versa.
        let n = normalize("đại học");
        let s = detect(&n);
        assert_eq!(s.fr_markers, 0);
    }

    #[test]
    fn shinjitai_kanji_does_not_register_as_hans_only() {
        let n = normalize("学校で国語を勉強する");
        let s = detect(&n);
        assert_eq!(s.hans_markers, 0, "Shinjitai must not be Hans-only: {s:?}");
    }

    #[test]
    fn vi_and_hans_can_coexist_in_one_input() {
        let n = normalize("Tôi học 老师");
        let s = detect(&n);
        assert!(s.vi_markers >= 1);
        assert!(s.hans_markers >= 1);
    }

    #[test]
    fn detects_cantonese_markers() {
        let n = normalize("我嘅學生喺學校");
        let s = detect(&n);
        assert!(s.yue_markers >= 2, "expected yue markers: {s:?}");
    }

    #[test]
    fn classical_chinese_density_trips() {
        // Classic opening from the Analects — dense classical particles.
        assert!(is_classical_chinese("子曰：學而時習之，不亦說乎？"));
    }

    #[test]
    fn modern_chinese_with_one_classical_particle_does_not_trip() {
        // Modern Mandarin: "I" + 之 once is not enough to call it Classical.
        assert!(!is_classical_chinese("我之前去过北京"));
    }

    #[test]
    fn detects_hant_tw_marker_臺() {
        let n = normalize("臺灣");
        let s = detect(&n);
        assert!(s.hant_tw_markers >= 1);
    }

    #[test]
    fn detects_hant_hk_marker_嘥() {
        let n = normalize("嘥時間");
        let s = detect(&n);
        assert!(s.hant_hk_markers >= 1);
    }

    #[test]
    fn detects_wu_marker() {
        let n = normalize("儂好");
        let s = detect(&n);
        assert!(s.wuu_markers >= 1);
    }
}
