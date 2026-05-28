use unicode_normalization::UnicodeNormalization;

#[derive(Debug, Clone)]
pub struct Normalized {
    pub chars: Vec<char>,
    pub visible_chars: usize,
}

pub fn normalize(input: &str) -> Normalized {
    let nfc: String = input.nfc().collect();

    let mut text = String::with_capacity(nfc.len());
    let mut last_was_space = true;
    for ch in nfc.chars() {
        if ch.is_control() && ch != '\n' && ch != '\t' {
            continue;
        }
        if ch.is_whitespace() {
            if !last_was_space {
                text.push(' ');
                last_was_space = true;
            }
        } else {
            text.push(ch);
            last_was_space = false;
        }
    }
    let chars: Vec<char> = text.trim().chars().collect();
    let visible_chars = chars
        .iter()
        .filter(|c| !c.is_whitespace() && !is_punctuation(**c))
        .count();

    Normalized {
        chars,
        visible_chars,
    }
}

fn is_punctuation(c: char) -> bool {
    matches!(
        c,
        '.' | ',' | '!' | '?' | ';' | ':' | '"' | '\'' | '`'
            | '(' | ')' | '[' | ']' | '{' | '}'
            | '|' | '/' | '\\' | '-' | '_' | '+' | '='
            | '*' | '&' | '^' | '%' | '$' | '#' | '@' | '~' | '<' | '>'
    ) || matches!(c as u32,
        // Common CJK punctuation we don't want to count toward script proportions
        0x3000..=0x303F |  // CJK Symbols and Punctuation
        0xFF00..=0xFF0F |  // Fullwidth ASCII punct
        0xFF1A..=0xFF20 |
        0xFF3B..=0xFF40 |
        0xFF5B..=0xFF65
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trims_and_collapses_whitespace() {
        let n = normalize("  hello   world  ");
        let joined: String = n.chars.iter().collect();
        assert_eq!(joined, "hello world");
    }

    #[test]
    fn nfc_combines_decomposed() {
        // "e" + combining acute -> "é"
        let n = normalize("cafe\u{0301}");
        let joined: String = n.chars.iter().collect();
        assert_eq!(joined, "café");
    }

    #[test]
    fn counts_visible_chars_excludes_punctuation() {
        let n = normalize("Hello, world!");
        assert_eq!(n.visible_chars, 10); // 'Hello' + 'world'
    }

    #[test]
    fn cjk_punctuation_excluded_from_visible() {
        let n = normalize("先生は大学で日本語を教えています。");
        // Trailing '。' is CJK punctuation and should not count.
        assert_eq!(n.visible_chars, n.chars.len() - 1);
    }

    #[test]
    fn collapses_tabs_and_newlines_to_single_space() {
        let n = normalize("hello\t\tworld\n\nfrom\trust");
        let joined: String = n.chars.iter().collect();
        assert_eq!(joined, "hello world from rust");
    }

    #[test]
    fn strips_non_newline_control_chars() {
        // Bell (U+0007) should be dropped entirely, not turned into whitespace.
        let n = normalize("ab\u{0007}cd");
        let joined: String = n.chars.iter().collect();
        assert_eq!(joined, "abcd");
    }

    #[test]
    fn empty_input_yields_no_chars() {
        let n = normalize("");
        assert!(n.chars.is_empty());
        assert_eq!(n.visible_chars, 0);
    }

    #[test]
    fn whitespace_only_input_yields_no_visible_chars() {
        let n = normalize("   \t\n  ");
        assert_eq!(n.visible_chars, 0);
    }
}
