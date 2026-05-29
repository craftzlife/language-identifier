use super::normalize::Normalized;
use super::script::{classify, Script};

#[derive(Debug, Clone, Default)]
pub struct OrthoSignals {
    pub vi_markers: usize,
    pub hans_markers: usize,
    pub hant_markers: usize,
}

pub fn detect(input: &Normalized) -> OrthoSignals {
    let mut sig = OrthoSignals::default();
    for &c in &input.chars {
        if classify(c) == Script::Latin && is_vi_marker(c) {
            sig.vi_markers += 1;
        }
        if is_hans_only(c) {
            sig.hans_markers += 1;
        }
        if is_hant_only(c) {
            sig.hant_markers += 1;
        }
    }
    sig
}

pub fn is_vi_marker_char(c: char) -> bool {
    is_vi_marker(c)
}

pub fn is_hant_only_char(c: char) -> bool {
    is_hant_only(c)
}

fn is_vi_marker(c: char) -> bool {
    // Strong Vietnamese markers — chars that are essentially exclusive to Vietnamese
    // among the languages we support.
    let cp = c as u32;

    // Latin Extended Additional U+1E00..U+1EFF — almost entirely Vietnamese tone-marked vowels.
    if (0x1E00..=0x1EFF).contains(&cp) {
        return true;
    }

    matches!(
        c,
        // ă/Ă, đ/Đ, ơ/Ơ, ư/Ư — not used by other supported langs
        'ă' | 'Ă' | 'đ' | 'Đ' | 'ơ' | 'Ơ' | 'ư' | 'Ư'
    )
}

// Han characters that are Simplified-only AND not used in modern Japanese.
// Curated short list — extending it is straightforward.
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

// Han characters that are Traditional-only AND not used in modern Japanese
// (Japanese either uses a Shinjitai simplification or does not use the character).
fn is_hant_only(c: char) -> bool {
    matches!(
        c,
        '學' | '國' | '來' | '們' | '經' | '這' | '實' | '會' | '說' | '應'
            | '讓' | '與' | '沒' | '聽' | '寫' | '認' | '識' | '觀' | '議' | '態'
            | '資' | '產' | '風' | '調' | '請' | '記'
            // v4.1 extension. Each entry has a distinct Hans form
            // *and* a Japanese shinjitai (or no modern Japanese use),
            // so seeing one is a strong Hant signal.
            //
            //   Hant | Hans | ja (shinjitai or alt)
            //   ---- | ---- | ----------------------
            | '權' // 权    権
            | '體' // 体    体  (ja shares Hans form)
            | '龜' // 龟    亀
            | '歲' // 岁    歳
            | '數' // 数    数  (ja shares Hans form)
            | '關' // 关    関
            | '舊' // 旧    旧
            | '處' // 处    処
            | '辭' // 辞    辞
            | '戰' // 战    戦
            | '對' // 对    対
            | '氣' // 气    気
            | '灣' // 湾    湾
            | '黨' // 党    党
            | '齊' // 齐    斉
            | '豐' // 丰    豊
            | '滿' // 满    満
            | '辦' // 办    弁
            | '齒' // 齿    歯
            | '從' // 从    従
            | '總' // 总    総
            | '歷' // 历    歴
            | '當' // 当    当
            | '兒' // 儿    児
            | '兩' // 两    両
    )
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
        // Sanity check the v4.1 extension: each of these is Hant-only
        // and must register without bleeding into hans_markers.
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
        // 先生 alone has no Hans-only or Hant-only markers (both characters are shared).
        let n = normalize("先生");
        let s = detect(&n);
        assert_eq!(s.hans_markers, 0);
        assert_eq!(s.hant_markers, 0);
    }

    #[test]
    fn vi_marker_includes_breve_and_horn() {
        let n = normalize("ăn cơm");
        let s = detect(&n);
        // ă (breve) and ơ (horn) — both VI-specific letters.
        assert!(s.vi_markers >= 2);
    }

    #[test]
    fn french_diacritics_do_not_register_as_vi() {
        // é à ç are Latin-1 Supplement, shared across many European languages.
        let n = normalize("café résumé naïve");
        let s = detect(&n);
        assert_eq!(s.vi_markers, 0);
    }

    #[test]
    fn shinjitai_kanji_does_not_register_as_hans_only() {
        // 学, 国, 来, 会 are Shinjitai forms shared with simplified Chinese — not Hans-only markers
        // by our definition (otherwise pure Japanese text would be misclassified as Chinese).
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
}
