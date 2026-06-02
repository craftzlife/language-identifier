//! Japanese tokenization via `lindera` with the embedded IPADIC
//! dictionary. IPADIC is the de-facto MeCab-compatible dictionary for
//! general-purpose Japanese segmentation; tokens are returned with
//! native byte offsets which we forward unchanged into the segment's
//! relative frame.

#[cfg(feature = "tokenize")]
pub fn tokenize(text: &str) -> Vec<(String, usize, usize)> {
    use std::sync::OnceLock;
    use lindera::dictionary::{load_embedded_dictionary, DictionaryKind};
    use lindera::mode::Mode;
    use lindera::segmenter::Segmenter;
    use lindera::tokenizer::Tokenizer;

    static TOKENIZER: OnceLock<Option<Tokenizer>> = OnceLock::new();
    let tokenizer = TOKENIZER.get_or_init(|| {
        let dict = load_embedded_dictionary(DictionaryKind::IPADIC).ok()?;
        let segmenter = Segmenter::new(Mode::Normal, dict, None);
        Some(Tokenizer::new(segmenter))
    });
    let Some(tokenizer) = tokenizer.as_ref() else {
        return Vec::new();
    };

    let Ok(tokens) = tokenizer.tokenize(text) else {
        return Vec::new();
    };
    let mut out = Vec::with_capacity(tokens.len());
    for t in tokens {
        let bs = t.byte_start;
        let be = t.byte_end;
        if bs >= be || be > text.len() {
            continue;
        }
        let slice = &text[bs..be];
        if slice.chars().all(char::is_whitespace) {
            continue;
        }
        out.push((slice.to_string(), bs, be));
    }
    out
}

#[cfg(not(feature = "tokenize"))]
pub fn tokenize(_text: &str) -> Vec<(String, usize, usize)> {
    Vec::new()
}
