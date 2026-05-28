use super::normalize::Normalized;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Script {
    Latin,
    Hiragana,
    Katakana,
    Han,
    Hangul,
    Other,
}

#[derive(Debug, Clone, Default)]
pub struct ScriptCounts {
    pub latin: usize,
    pub hiragana: usize,
    pub katakana: usize,
    pub han: usize,
    pub hangul: usize,
    pub other: usize,
}

impl ScriptCounts {
    pub fn supported_total(&self) -> usize {
        self.latin + self.hiragana + self.katakana + self.han + self.hangul
    }

    pub fn kana(&self) -> usize {
        self.hiragana + self.katakana
    }
}

pub fn classify(c: char) -> Script {
    let cp = c as u32;
    // Basic Latin + Latin-1 + Latin Extended (covers Vietnamese precomposed letters)
    if (0x0041..=0x005A).contains(&cp)
        || (0x0061..=0x007A).contains(&cp)
        || (0x00C0..=0x024F).contains(&cp)
        || (0x1E00..=0x1EFF).contains(&cp)
    {
        return Script::Latin;
    }
    // Hiragana
    if (0x3040..=0x309F).contains(&cp) {
        return Script::Hiragana;
    }
    // Katakana (incl. phonetic extensions)
    if (0x30A0..=0x30FF).contains(&cp) || (0x31F0..=0x31FF).contains(&cp) {
        return Script::Katakana;
    }
    // CJK Unified Ideographs (BMP + extensions A/B sampled)
    if (0x4E00..=0x9FFF).contains(&cp) || (0x3400..=0x4DBF).contains(&cp) {
        return Script::Han;
    }
    // Hangul Syllables + Jamo
    if (0xAC00..=0xD7AF).contains(&cp)
        || (0x1100..=0x11FF).contains(&cp)
        || (0x3130..=0x318F).contains(&cp)
    {
        return Script::Hangul;
    }
    Script::Other
}

pub fn count_scripts(input: &Normalized) -> ScriptCounts {
    let mut counts = ScriptCounts::default();
    for &c in &input.chars {
        if c.is_whitespace() {
            continue;
        }
        match classify(c) {
            Script::Latin => counts.latin += 1,
            Script::Hiragana => counts.hiragana += 1,
            Script::Katakana => counts.katakana += 1,
            Script::Han => counts.han += 1,
            Script::Hangul => counts.hangul += 1,
            Script::Other => counts.other += 1,
        }
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layers::normalize::normalize;

    #[test]
    fn classifies_latin_extended() {
        assert_eq!(classify('a'), Script::Latin);
        assert_eq!(classify('é'), Script::Latin);
        assert_eq!(classify('ế'), Script::Latin);
        assert_eq!(classify('Đ'), Script::Latin);
    }

    #[test]
    fn classifies_kana_and_han() {
        assert_eq!(classify('の'), Script::Hiragana);
        assert_eq!(classify('カ'), Script::Katakana);
        assert_eq!(classify('先'), Script::Han);
    }

    #[test]
    fn classifies_hangul() {
        assert_eq!(classify('한'), Script::Hangul);
    }

    #[test]
    fn counts_mixed_text() {
        let n = normalize("先生は大学で日本語を教えています。");
        let c = count_scripts(&n);
        assert!(c.han > 0);
        assert!(c.hiragana > 0);
    }
}
