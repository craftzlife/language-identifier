# language-identifier

A layered language identifier for short and multi-line text inputs. Returns a ranked list of BCP 47 language candidates with calibrated confidence and an ambiguity-aware status.

Designed for dictionary apps, language-learning tools, and editor text-selection flows where input may contain embedded segments of a second language inside a primary one (an English sentence quoting a Japanese phrase, a Vietnamese sentence with a Chinese term, …).

## Status

**v3** — implements Layers 0–7 + `segments` output + `mixed` status + SDD-strict
M3 behavior. See [`SOFTWARE_DESIGN.md`](./SOFTWARE_DESIGN.md) for the full
pipeline spec.

| Layer | Implemented |
| --- | --- |
| 0. Normalization (NFC, whitespace, control chars) | ✅ |
| 1. Character n-gram scoring (en/vi tie-breaker) | ✅ |
| 2. Script & Unicode signal | ✅ |
| 3. Orthographic rules (kana, VI diacritics, Hans/Hant markers) | ✅ |
| 4. Function words / particles / stopwords | ✅ |
| 5. Dictionary / lexicon matching (10 K words × 6 langs) | ✅ |
| 6. Morphology / tokenization hints (per-token EN/VI attribution + CJK endings) | ✅ |
| 7. Context window scoring (per-sentence winners + intra-sentence Han runs) | ✅ |
| 8. User preference / app state | ❌ out of scope (see [SDD §7.1](./SOFTWARE_DESIGN.md#71-layer-notes)) |
| 9. Lightweight ML classifier | ⏳ v4 |
| 10. LLM resolver | ⏳ v4 |
| Final calibration & ambiguity handling (resolved / ambiguous / **mixed** / unknown / unsupported, with context-driven downgrade) | ✅ |

Supported languages: `en-US`, `ja`, `zh-Hans`, `zh-Hant`, `zh` (variant-unclear umbrella tag for embedded ambiguous Han spans), `vi-VN`, `ko`.

## Build

```bash
cargo build --workspace
cargo test --workspace
```

Requires Rust 1.75+.

## Library usage

```rust
use language_identifier::{identify, identify_lines, Status};

let r = identify("giáo viên đại học");
assert_eq!(r.status, Status::Resolved);
assert_eq!(r.primary_language.as_deref(), Some("vi-VN"));

let r = identify_lines(&[
    "田中先生は大学で日本語を教えています。",
    "学生たちは毎日授業に参加し、新しい言葉や文法を学んでいます。",
]);
assert_eq!(r.primary_language.as_deref(), Some("ja"));
```

The `IdentifyResult` implements `serde::Serialize`, so callers can produce the JSON shape defined in §5 of the SDD directly:

```rust
let json = serde_json::to_string_pretty(&identify("Teacher")).unwrap();
```

## CLI usage

```bash
# single argument
cargo run -q -p language-identifier-cli -- "先生"

# pretty-print
cargo run -q -p language-identifier-cli -- --pretty "giáo viên đại học"

# read array-of-lines input from stdin (use '-' as the argument)
printf '田中先生は大学で日本語を教えています。\n学生たちは毎日授業に参加し、新しい言葉や文法を学んでいます。\n' \
  | cargo run -q -p language-identifier-cli -- --pretty -
```

Example output for a mixed-language input:

```bash
$ cargo run -q -p language-identifier-cli -- --pretty "Hello こんにちは world ありがとう"
```

```json
{
  "candidates": [
    { "language": "en-US", "confidence": 0.61 },
    { "language": "ja",    "confidence": 0.37 },
    { "language": "vi-VN", "confidence": 0.02 }
  ],
  "primaryLanguage": "en-US",
  "status": "mixed",
  "reasons": [
    "Script counts — latin:10, hiragana:5, katakana:0, han:0, hangul:0",
    "Japanese kana present — Han characters interpreted as kanji",
    "Function-word hits — ja:3",
    "Dictionary — 3 recognized tokens (2 ambiguous across lexicons)",
    "2 candidates each hold ≥0.20 confidence — input contains multiple languages as first-class content"
  ],
  "segments": [
    { "language": "en-US", "start": 0,  "end": 5,  "text": "Hello" },
    { "language": "ja",    "start": 6,  "end": 21, "text": "こんにちは" },
    { "language": "en-US", "start": 22, "end": 27, "text": "world" }
  ],
  "normalizedText": "Hello こんにちは world"
}
```

`normalizedText` is the form of the input that the pipeline actually
analyzed: NFC-composed, whitespace-collapsed, control-stripped, trimmed.
Segment `start` / `end` byte offsets index into `normalizedText`, and each
segment's `text` is the slice between those offsets.

`segments` is always present; for a fully single-language input it contains
one span covering the whole text. For `unknown` / `unsupported` status it is
an empty array.

## Output schema

```jsonc
{
  "candidates": [
    { "language": "<BCP 47 tag>", "confidence": 0.0 }
  ],
  "primaryLanguage": "<BCP 47 tag>",          // omitted when status is "unknown" / "unsupported"
  "status": "resolved | ambiguous | mixed | unknown | unsupported",
  "reasons": ["...", "..."],                    // array of explanation lines (always an array)
  "segments": [                                 // always present; `[]` only when status is unknown/unsupported
    { "language": "<BCP 47 tag>", "start": 0, "end": 0, "text": "..." }
  ],
  "normalizedText": "..."                        // the analyzed form (NFC, whitespace-collapsed)
}
```

See [`SOFTWARE_DESIGN.md` §5](./SOFTWARE_DESIGN.md#5-output-schema) for field semantics and status definitions.

## Repository layout

```
.
├── Cargo.toml                              # workspace
├── NOTICE                                  # wordfreq + OpenCC attribution
├── crates/
│   ├── language-identifier/                # library crate
│   │   ├── src/
│   │   │   ├── lib.rs                      # public API
│   │   │   ├── types.rs                    # Status, Candidate, Segment, IdentifyResult, Reason
│   │   │   ├── pipeline.rs                 # orchestrates layers → aggregate → calibrate → segments
│   │   │   ├── aggregate.rs                # script + orthography + Layers 4/5 → candidate distribution
│   │   │   ├── calibration.rs              # gap-based + Mixed status decision
│   │   │   ├── segments.rs                 # span computation
│   │   │   ├── data/
│   │   │   │   ├── SOURCES.md              # lexicon provenance and license
│   │   │   │   └── lexicon_{en,ja,zh_hans,zh_hant,vi,ko}.txt  # 10 K words each
│   │   │   └── layers/
│   │   │       ├── normalize.rs            # Layer 0
│   │   │       ├── ngram.rs                # Layer 1
│   │   │       ├── script.rs               # Layer 2
│   │   │       ├── orthography.rs          # Layer 3
│   │   │       ├── function_words.rs       # Layer 4
│   │   │       └── dictionary.rs           # Layer 5
│   │   └── tests/
│   │       ├── golden.rs                   # SDD cases W1, W2, P1-P4, G1-G4, M1-M7
│   │       ├── layer4.rs                   # function-word integration tests
│   │       ├── layer5.rs                   # dictionary integration tests
│   │       ├── segments.rs                 # span-output tests
│   │       └── mixed.rs                    # Mixed status tests
│   └── language-identifier-cli/            # binary crate
│       └── src/main.rs
├── SOFTWARE_DESIGN.md                      # full design document
└── language-identifier-library.drawio      # diagram source of truth
```

## Known v3 limitations

- **`HanAmbiguous` spans still surface as `zh`** (the variant-unclear umbrella
  tag). Disambiguating an isolated Han snippet to `ja` vs `zh-Hans` vs
  `zh-Hant` *without* kana or markers requires Layer 9/10 (ML / LLM).
- **Sentence splitting on Latin abbreviations** (`Mr.`, `Dr.`) over-splits
  sentences. Per-token attribution still aggregates correctly to the same
  language, so this is cosmetic in the `reason` notes but doesn't change
  the verdict.
- **BCP 47 codes**: v3 emits `zh-Hans` / `zh-Hant` (BCP 47-correct). The SDD's
  test outputs use `cn-Hans` / `cn-Hant`; that divergence is intentional and
  noted in §12 of the SDD.
- **Segment offsets are into normalized text**, not the caller's original
  string. Mapping back through NFC + whitespace collapse is a v4 concern.

## License

MIT OR Apache-2.0 for the source code. The bundled word-frequency lists in
`crates/language-identifier/src/data/lexicon_*.txt` are derived from
[wordfreq](https://github.com/rspeer/wordfreq/) and redistributed under
Creative Commons Attribution-ShareAlike 4.0 — see [`NOTICE`](./NOTICE) for full
attribution.
