# Lexicon sources

Every `lexicon_*.txt` in this directory contains exactly 10 000 unique terms,
one per line, UTF-8 (NFC). Lists are frequency-ranked: the most common token
appears first.

## Provenance

Source: [`wordfreq`](https://github.com/rspeer/wordfreq/) 3.1.1 via
`wordfreq.top_n_list(language, 50000)`, then filtered and truncated to 10 000
usable terms per locale.

Chinese files are produced from `wordfreq`'s `zh` data converted with
[OpenCC Python](https://github.com/yichen0831/opencc-python/):

| File | Source | Conversion |
| --- | --- | --- |
| `lexicon_en.txt` | `wordfreq` `en` | none |
| `lexicon_ja.txt` | `wordfreq` `ja` | none; filter requires Hiragana / Katakana / Han |
| `lexicon_zh_hans.txt` | `wordfreq` `zh` | OpenCC `t2s` |
| `lexicon_zh_hant.txt` | `wordfreq` `zh` | OpenCC `s2t` |
| `lexicon_vi.txt` | `wordfreq` `vi` | none |
| `lexicon_ko.txt` | `wordfreq` `ko` | none; filter requires Hangul |

## Filtering

Each term is kept only if it is non-empty, contains at least one Unicode
letter, contains no digits, has no leading/trailing whitespace, is not already
present in the file, and uses only letters, combining marks, spaces,
apostrophes, curly apostrophes, or hyphens.

Script-specific filters additionally keep only terms containing the target
script (Hiragana/Katakana/Han for Japanese, Han for Chinese, Hangul for
Korean).

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
