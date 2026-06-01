# Lexicon sources

Every `lexicon_*.txt` in this directory contains exactly 10 000 unique terms,
one per line, UTF-8 (NFC). Lists are frequency-ranked: the most common token
appears first.

## Provenance

Every supported BCP 47 tag now has a 10 000-entry lexicon. Sources fall
into three families:

1. **`wordfreq` 3.1.1** for languages with native frequency lists.
2. **`wordfreq` `zh` + OpenCC** for the Mandarin script / region variants.
3. **Wikipedia article-title dumps and the MOE Taiwan Hakka dictionary**
   for Sinitic siblings (`yue`, `lzh`, `nan`, `hak`, `wuu`) that
   `wordfreq` maps to generic `zh`.

| File | Source | Conversion / Filter |
| --- | --- | --- |
| `lexicon_en.txt` | `wordfreq` `en` | none |
| `lexicon_fr.txt` | `wordfreq` `fr` | none |
| `lexicon_ja.txt` | `wordfreq` `ja` | filter requires Hiragana / Katakana / Han |
| `lexicon_vi.txt` | `wordfreq` `vi` | none |
| `lexicon_ko.txt` | `wordfreq` `ko` | filter requires Hangul |
| `lexicon_zh_hans.txt` | `wordfreq` `zh` | OpenCC `t2s` |
| `lexicon_zh_hant.txt` | `wordfreq` `zh` | OpenCC `s2t` |
| `lexicon_zh_hant_hk.txt` | `wordfreq` `zh` | OpenCC `s2hk` |
| `lexicon_zh_hant_tw.txt` | `wordfreq` `zh` | OpenCC `s2twp` |
| `lexicon_yue.txt` | Cantonese Wikipedia `zh_yuewiki` title dump | plain-title filter |
| `lexicon_lzh.txt` | Classical Chinese Wikipedia `zh_classicalwiki` title dump | plain-title filter |
| `lexicon_nan.txt` | MOE Taiwan Min Nan dictionary headwords (`g0v/moedict-data-twblg`) | plain headword filter |
| `lexicon_hak.txt` | MOE Taiwan Hakka dictionary headwords (`Taiwanese-Corpus/moedict-data-hakka`) | plain headword filter |
| `lexicon_wuu.txt` | Wu Wikipedia `wuuwiki` title dump | plain-title filter |

External-source URLs (Wikipedia title dumps + MOE Hakka mirror) live in
`manifest.json` of the upstream dataset; see the "Regenerating" section.

> **Note** — variant lexicons sourced from Wikipedia title dumps are
> not strictly frequency-ranked; their top entries are sometimes rare
> CJK Extension characters rather than high-frequency particles. That is
> why the Layer-4 function-word tables for `yue` and `lzh` are still
> curated in `function_words_yue.txt` / `function_words_lzh.txt` rather
> than derived from the top of the lexicon.

## Filtering

Each term is kept only if it is non-empty, contains at least one Unicode
letter, contains no digits, has no leading/trailing whitespace, is not already
present in the file, and uses only letters, combining marks, spaces,
apostrophes, curly apostrophes, or hyphens.

Script-specific filters additionally keep only terms containing the target
script (Hiragana/Katakana/Han for Japanese, Han for Chinese, Hangul for
Korean).

## Layer-3 / Layer-4 marker sets

Layer 3 (orthographic exclusivity) and Layer 4 (curated function-word
tables for `yue` / `lzh`) work off small fixed character lists — a
dozen entries or so per language. Those live inline in
`src/layers/orthography.rs` (`is_hans_only`, `is_hant_only`,
`is_vi_marker`, `is_fr_marker_char`, `is_yue_marker`, `is_lzh_marker`,
`is_hant_hk_marker`, `is_hant_tw_marker`, `is_nan_marker`,
`is_hak_marker`, `is_wuu_marker`) and `src/layers/function_words.rs`
(`YUE_PARTICLES`, `LZH_PARTICLES`). Not enough content to warrant
external files.

## Licensing

`wordfreq` is Apache-2.0 and includes redistributable data under
Creative Commons Attribution-ShareAlike 4.0 and other attributed sources.
See the `NOTICE` file at the repository root for the required attribution.

## Regenerating

The generator script lives at the original dataset workspace:
`https://github.com/<unpublished>/raw-10000-words/generate_word_lists.py`.

To regenerate locally:

```sh
python -m venv .venv
.venv/bin/python -m pip install wordfreq==3.1.1 opencc-python-reimplemented==0.1.7
.venv/bin/python generate_word_lists.py
```

Then copy the produced files into this directory and rename `en-US.txt` →
`lexicon_en.txt`, etc.
