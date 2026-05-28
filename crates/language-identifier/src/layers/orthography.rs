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
        '师' | '时' | '这' | '让' | '给' | '们' | '经' | '还' | '实' | '进'
            | '见' | '问' | '谁' | '长' | '说' | '应' | '听' | '个' | '观' | '议'
            | '态' | '资' | '产' | '风' | '调' | '语' | '请' | '认' | '识' | '记'
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
