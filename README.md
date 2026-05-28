# language-identifier

A layered language identifier for short and multi-line text inputs. Returns a ranked list of BCP 47 language candidates with calibrated confidence and an ambiguity-aware status.

Designed for dictionary apps, language-learning tools, and editor text-selection flows where input may contain embedded segments of a second language inside a primary one (an English sentence quoting a Japanese phrase, a Vietnamese sentence with a Chinese term, …).

## Status

**v1** — implements Layers 0–3 of the pipeline described in [`SOFTWARE_DESIGN.md`](./SOFTWARE_DESIGN.md):

| Layer | Implemented |
| --- | --- |
| 0. Normalization (NFC, whitespace, control chars) | ✅ |
| 1. Character n-gram scoring (en/vi tie-breaker) | ✅ |
| 2. Script & Unicode signal | ✅ |
| 3. Orthographic rules (kana, VI diacritics, Hans/Hant markers) | ✅ |
| 4. Function words / stopwords | ⏳ v2 |
| 5. Dictionary / lexicon matching | ⏳ v2 |
| 6. Morphology / tokenization hints | ⏳ v2 |
| 7. Context window scoring | ⏳ v2 |
| 8. User preference / app state | ⏳ v2 |
| 9. Lightweight ML classifier | ⏳ v2 |
| 10. LLM resolver | ⏳ v2 |
| Final calibration & ambiguity handling | ✅ |

Supported languages: `en-US`, `ja`, `zh-Hans`, `zh-Hant`, `zh` (variant-unclear), `vi-VN`, `ko`.

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

Example output:

```json
{
  "candidates": [
    { "language": "ja", "confidence": 0.5 },
    { "language": "zh", "confidence": 0.5 }
  ],
  "primaryLanguage": "ja",
  "status": "ambiguous",
  "reason": [
    "Script counts — latin:0, hiragana:0, katakana:0, han:2, hangul:0",
    "Confidence gap is small (0.00); detection status is 'ambiguous'"
  ]
}
```

## Output schema

```jsonc
{
  "candidates": [
    { "language": "<BCP 47 tag>", "confidence": 0.0 }
  ],
  "primaryLanguage": "<BCP 47 tag>",       // omitted when status is "unknown" / "unsupported"
  "status": "resolved | ambiguous | mixed | unknown | unsupported",
  "reason": "..." | ["...", "..."]          // single line or array of explanation lines
}
```

See [`SOFTWARE_DESIGN.md` §5](./SOFTWARE_DESIGN.md#5-output-schema) for field semantics and status definitions.

## Repository layout

```
.
├── Cargo.toml                              # workspace
├── crates/
│   ├── language-identifier/                # library crate
│   │   ├── src/
│   │   │   ├── lib.rs                      # public API
│   │   │   ├── types.rs                    # Status, Candidate, IdentifyResult, Reason
│   │   │   ├── pipeline.rs                 # orchestrates layers → aggregate → calibrate
│   │   │   ├── aggregate.rs                # script + orthography → candidate distribution
│   │   │   ├── calibration.rs              # gap-based status decision
│   │   │   └── layers/
│   │   │       ├── normalize.rs            # Layer 0
│   │   │       ├── ngram.rs                # Layer 1
│   │   │       ├── script.rs               # Layer 2
│   │   │       └── orthography.rs          # Layer 3
│   │   └── tests/golden.rs                 # SDD test cases W1, W2, P1-P4, G1-G4, M1-M4
│   └── language-identifier-cli/            # binary crate
│       └── src/main.rs
├── SOFTWARE_DESIGN.md                      # full design document
└── language-identifier-library.drawio      # diagram source of truth
```

## Known v1 limitations

The full mixed-language behavior described in the SDD requires later layers. v1 has the following gaps, all to be addressed in v2:

- **W1** (`先生 教師 先生 老师`): the SDD expects `ambiguous` between `ja` and `zh` based on dictionary overlap. v1 reaches `ambiguous` via conflicting Hans/Hant markers, which is the right answer for this input but the wrong reasoning. Pure-Han short inputs without conflicting markers will resolve to a Chinese variant instead of staying ambiguous.
- **M3** (`JA_ZH`): the SDD expects `ambiguous` with an LLM picking `ja` as primary. v1 returns `resolved ja` deterministically because kana dominance closes the gap.
- **BCP 47 codes**: v1 emits `zh-Hans` / `zh-Hant` (BCP 47-correct). The SDD's test outputs use `cn-Hans` / `cn-Hant`; that divergence is intentional and noted in §12 of the SDD.
- **`mixed` status**: declared in the enum but not produced by v1 (all multi-language inputs resolve to `resolved` or `ambiguous`).

## License

MIT OR Apache-2.0.
