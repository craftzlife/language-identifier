//! Chinese-family tokenization via `jieba-rs`. Used for every `zh*`
//! variant tag plus `yue` / `lzh` / `nan` / `hak` / `wuu` — jieba is
//! the most broadly available Hanzi segmenter and produces reasonable
//! output even for Cantonese / Classical text whose dedicated tools
//! are not on crates.io.

#[cfg(feature = "tokenize")]
pub fn tokenize(text: &str) -> Vec<(String, usize, usize)> {
    use std::sync::OnceLock;
    static JIEBA: OnceLock<jieba_rs::Jieba> = OnceLock::new();
    let jieba = JIEBA.get_or_init(jieba_rs::Jieba::new);

    // `tokenize()` reports `start` / `end` as character offsets. Map
    // them through a single char-to-byte index built from the source
    // so we end up with byte offsets, matching `Segment.start`/`end`.
    let char_to_byte: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .collect();

    let tokens = jieba.tokenize(text, jieba_rs::TokenizeMode::Default, true);
    let mut out = Vec::with_capacity(tokens.len());
    for t in tokens {
        let bs = char_to_byte[t.start];
        let be = char_to_byte[t.end];
        let word = &text[bs..be];
        if word.chars().all(char::is_whitespace) {
            continue;
        }
        out.push((word.to_string(), bs, be));
    }
    out
}

#[cfg(not(feature = "tokenize"))]
pub fn tokenize(_text: &str) -> Vec<(String, usize, usize)> {
    Vec::new()
}
