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
| 5. Dictionary / lexicon matching (10 K words × 14 tags) | ✅ |
| 6. Morphology / tokenization hints (per-token EN/VI attribution + CJK endings) | ✅ |
| 7. Context window scoring (per-sentence winners + intra-sentence Han runs) | ✅ |
| 8. User preference / app state | ❌ out of scope (see [SDD §7.1](./SOFTWARE_DESIGN.md#71-layer-notes)) |
| 9. Lightweight ML classifier | ✅ v4.2 (default impl `OpenLidClassifier` behind `ml-openlid` feature, OpenLID v3 model, 194 languages) |
| 10. LLM resolver | ❌ out of scope (see [SDD §7.1](./SOFTWARE_DESIGN.md#71-layer-notes)) |
| Final calibration & ambiguity handling (resolved / ambiguous / **mixed** / unknown / unsupported, with context-driven downgrade) | ✅ |

Supported languages come in two tiers:

- **Tier 1 — full pipeline** (scripts + lexicons + morphology + orthography markers): `en`, `fr`, `vi`, `ja`, `ko`, `zh-Hans`, `zh-Hant`, `zh-Hant-HK`, `zh-Hant-TW`, `yue` (Cantonese), `lzh` (Classical Chinese), `nan` (Min Nan), `hak` (Hakka), `wuu` (Wu), plus the `zh` umbrella tag for variant-unclear Han spans.
- **Tier 2 — Layer 9 ML-only** (enabled by the `ml-openlid` feature): the remaining ~180 languages covered by OpenLID v3 — Spanish, German, Russian, Arabic, Hindi, Thai, Portuguese, Italian, Dutch, Polish, Turkish, Indonesian, Bengali, etc. — surface as candidates when the classifier is confident but have no segment-level attribution. Calibration may report `Status::Ambiguous` for Tier 2 inputs because the L9 bonus is capped (see SOFTWARE_DESIGN.md §6).

See [SDD §6.1](./SOFTWARE_DESIGN.md#61-detection-precision-per-variant) for per-variant precision.

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
assert_eq!(r.primary_language.as_deref(), Some("vi"));

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

### Customizing detection (Layer 9)

`identify` / `identify_lines` run Layers 0–7 only. To opt into Layer 9
(a lightweight ML classifier), use `identify_with` /
`identify_lines_with` and supply a trait implementation through
`IdentifyOptions`:

```rust
use language_identifier::{identify_with, IdentifyOptions, MlClassifier};

struct MyClassifier;
impl MlClassifier for MyClassifier {
    fn classify(&self, text: &str) -> Vec<(String, f32)> {
        // Return per-language confidence over BCP 47 tags.
        vec![("en".into(), 0.92), ("vi".into(), 0.04)]
    }
}

let ml = MyClassifier;
let opts = IdentifyOptions {
    ml_classifier: Some(&ml),
};
let r = identify_with("先生", &opts);
```

Layer 9 (when provided) folds its scores into aggregation as a capped
bonus. A bundled `MlClassifier` —
[`OpenLidClassifier`](#layer-9-with-the-bundled-openlid-default-v42) —
ships behind the `ml-openlid` cargo feature; see below. Layer 10
(LLM tie-breaker) is deliberately out of library scope — see
[SDD §7.1](./SOFTWARE_DESIGN.md#71-layer-notes). Consumers needing
LLM-based disambiguation should wrap `identify` in their application
layer.

### Layer 9 with the bundled OpenLID default (v4.2)

Enable the `ml-openlid` feature to compile the default `MlClassifier`
backed by the [OpenLID v3](https://github.com/laurieburchell/open-lid-dataset)
fastText model (`openlid-v3.bin`, 194 languages). The
[`fasttext`](https://crates.io/crates/fasttext) crate v0.8 used
underneath is a pure-Rust port — no `clang` / `cmake` / C++ toolchain
needed.

Use the model from Rust:

```rust
use language_identifier::{
    identify_with, OpenLidClassifier, IdentifyOptions,
};

let classifier = OpenLidClassifier::builder()
    .model_path("./openlid-v3.bin")
    .build()?;
let opts = IdentifyOptions {
    ml_classifier: Some(&classifier),
};
let r = identify_with("Hola, ¿cómo estás?", &opts);
```

Or from the CLI:

```bash
cargo run --features ml-openlid -p language-identifier-cli -- \
  --ml-openlid ./openlid-v3.bin --pretty "Hola, ¿cómo estás?"
```

`OpenLidClassifier` maps OpenLID's `iso639-3_Script` labels into the
library's BCP 47 set. Examples:

| OpenLID label   | mapped tag                                |
| --------------- | ----------------------------------------- |
| `eng_Latn`      | `en`                                      |
| `fra_Latn`      | `fr`                                      |
| `vie_Latn`      | `vi`                                      |
| `jpn_Jpan`      | `ja`                                      |
| `kor_Hang`      | `ko`                                      |
| `yue_Hant`      | `yue`                                     |
| `lzh_Hani`      | `lzh`                                     |
| `cmn_Hans`      | `zh-Hans` (or `yue` / `lzh` / `zh-Hant-*` per text) |
| `cmn_Hant`      | `zh-Hant` (or `yue` / `zh-Hant-HK` / `zh-Hant-TW` per text) |
| `spa_Latn`      | `es` (Tier 2 — ML-only)                   |
| `rus_Cyrl`      | `ru` (Tier 2 — ML-only)                   |
| `arb_Arab`      | `ar` (Tier 2 — ML-only)                   |
| any other       | *dropped*                                 |

OpenLID's `cmn_Hans` / `cmn_Hant` labels are forwarded to
[`bcp47::resolve_zh`](./crates/language-identifier/src/bcp47.rs), which
inspects the text and promotes the prediction to the most specific tag
the input supports (Cantonese particles → `yue`, dense classical
function words → `lzh`, HK/TW region markers → `zh-Hant-HK` /
`zh-Hant-TW`). OpenLID's dedicated `yue_Hant` / `lzh_Hani` labels map
straight through with no text scan.

**Unsupported-language downgrade.** When `OpenLidClassifier` returns a
top label that is off our curated mapping table (shouldn't happen for
trained OpenLID v3 output, but defends against future model changes),
the pipeline returns `status: "unsupported"` instead of forcing a
verdict. The threshold is 0.50 confidence. Custom `MlClassifier`
impls can opt into this behavior by overriding the trait's
[`unsupported_signal`](https://docs.rs/language-identifier/latest/language_identifier/trait.MlClassifier.html#method.unsupported_signal)
method (default returns `None`).

OpenLID weights and license terms come from the upstream project; the
library code stays MIT/Apache-2.0.

## CLI usage

### Synopsis

```text
language-identifier [--pretty] [--ml-openlid <path>] <text>
language-identifier [--pretty] [--ml-openlid <path>] -
```

The CLI prints a single JSON-encoded `IdentifyResult` to stdout. It
exits `0` on success, `2` on argument errors, and `1` on serialization
errors.

### Arguments

| Argument | Description |
| --- | --- |
| `<text>` | One or more positional arguments are joined with single spaces and identified as a single string. |
| `-` | When the only positional argument is a single dash, the CLI reads one input per line from stdin and identifies them as a single multi-line input (equivalent to the library's `identify_lines`). |

### Options

| Option | Description |
| --- | --- |
| `--pretty` | Indent the JSON output (two-space indent). Without this flag the output is single-line minified JSON, convenient for piping into `jq`. |
| `--ml-openlid <path>` | Enable Layer 9 (the lightweight ML classifier) using the OpenLID v3 fastText model at `<path>`. Activates Tier 2 language detection (~180 additional languages beyond the Tier 1 full-pipeline set). Requires the binary to be built with `--features ml-openlid`; without that feature flag the option errors out. The path is to the `.bin` model file — see [Layer 9 with the bundled OpenLID default](#layer-9-with-the-bundled-openlid-default-v42). |

> The `-q` flag in the examples below is a `cargo run` option (short for
> `--quiet`) that suppresses cargo's own build-progress output so only
> the JSON result reaches stdout. It is not an option of the
> language-identifier CLI itself, and is unnecessary when invoking a
> pre-built binary directly (e.g. `./target/release/language-identifier
> ...`).

### Examples

**Default Tier 1 (no model needed)**

```bash
# Single argument — Vietnamese phrase
cargo run -q -p language-identifier-cli -- "giáo viên đại học"

# Pretty-print — Cantonese (yue) routed under the zh macro
cargo run -q -p language-identifier-cli -- --pretty "今日朝早，我喺旺角行過一條好熱鬧嘅街"

# Multi-line input via stdin
printf '田中先生は大学で日本語を教えています。\n学生たちは毎日授業に参加しています。\n' \
  | cargo run -q -p language-identifier-cli -- --pretty -
```

**With Layer 9 / OpenLID (Tier 1 + Tier 2)**

```bash
# Tier 2 (Spanish, Russian, Arabic, Hindi, Thai, …) — Layer 9 only
cargo run -q --features ml-openlid -p language-identifier-cli -- \
  --ml-openlid ./openlid-v3.bin --pretty "Hola, ¿cómo estás?"

# Tier 1 + Layer 9 boost (French) — model confirms the fr verdict
cargo run -q --features ml-openlid -p language-identifier-cli -- \
  --ml-openlid ./openlid-v3.bin --pretty "Bonjour le monde"

# Pipe into jq to extract just the top result
echo "Привет мир" | cargo run -q --features ml-openlid -p language-identifier-cli -- \
  --ml-openlid ./openlid-v3.bin - | jq '.primaryLanguage'
```

### Example output

```bash
$ cargo run -q -p language-identifier-cli -- --pretty "Hello こんにちは world ありがとう"
```

```json
{
  "candidates": [
    { "language": "en", "variants": ["en"], "confidence": 0.61 },
    { "language": "ja", "variants": ["ja"], "confidence": 0.37 },
    { "language": "vi", "variants": ["vi"], "confidence": 0.02 }
  ],
  "primaryLanguage": "en",
  "primaryVariant": "en",
  "status": "mixed",
  "reasons": [
    "Script counts — latin:10, hiragana:5, katakana:0, han:0, hangul:0",
    "Japanese kana present — Han characters interpreted as kanji",
    "Function-word hits — ja:3",
    "Dictionary — 3 recognized tokens (2 ambiguous across lexicons)",
    "2 candidates each hold ≥0.20 confidence — input contains multiple languages as first-class content"
  ],
  "segments": [
    { "language": "en", "start": 0,  "end": 5,  "text": "Hello" },
    { "language": "ja", "start": 6,  "end": 21, "text": "こんにちは" },
    { "language": "en", "start": 22, "end": 27, "text": "world" }
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

## Native bindings (Swift, Kotlin, C#, C++)

For non-Rust consumers, the [`language-identifier-ffi`](./crates/language-identifier-ffi/)
crate exposes the same `identify` / `identify_lines` API as a single
`.dylib` / `.so` / `.dll` plus generated language bindings. Two transports
ship in one artifact:

| Platform        | Language | Transport                                  | Build script |
| --------------- | -------- | ------------------------------------------ | ------------ |
| iOS, macOS      | Swift    | UniFFI → XCFramework                       | [`scripts/build-apple.sh`](./crates/language-identifier-ffi/scripts/build-apple.sh)     |
| Android         | Kotlin   | UniFFI → `jniLibs/` + `.kt`                | [`scripts/build-android.sh`](./crates/language-identifier-ffi/scripts/build-android.sh) |
| Windows         | C# / any | C ABI + cbindgen header (P/Invoke)         | [`scripts/build-windows.sh`](./crates/language-identifier-ffi/scripts/build-windows.sh) |
| Linux           | C++ / any| C ABI + cbindgen header                    | [`scripts/build-linux.sh`](./crates/language-identifier-ffi/scripts/build-linux.sh)     |

The C ABI surface is three functions — `lid_identify`, `lid_identify_lines`,
`lid_string_free` — returning the same JSON-encoded `IdentifyResult`
described under [Output schema](#output-schema). The header is generated
by `cbindgen` on every cargo build at
`target/c-header/language_identifier.h`.

UniFFI symbols and the C ABI coexist in the same compiled library, so the
artifact you ship can be consumed by either transport. The `ml-openlid`
feature is intentionally not enabled in the FFI crate — mobile apps can't
realistically bundle the OpenLID model, and desktop consumers can call
the Rust API directly. Each build script's header comment lists its
one-time prerequisites (rustup targets, Android NDK, MinGW, `cross`, …).

## Output schema

```jsonc
{
  "candidates": [
    {
      "language": "<macro BCP 47 tag>",         // zh, en, fr, vi, ja, ko
      "variants": ["<fine BCP 47 tag>", ...],   // fine tags the library can decide under this macro
      "confidence": 0.0
    }
  ],
  "primaryLanguage": "<macro BCP 47 tag>",      // omitted when status is "unknown" / "unsupported"
  "primaryVariant": "<fine BCP 47 tag>",        // top fine tag inside the primary macro; same as primaryLanguage for en/fr/vi/ja/ko
  "status": "resolved | ambiguous | mixed | unknown | unsupported",
  "reasons": ["...", "..."],                    // array of explanation lines (always an array)
  "segments": [                                 // fine-grained tags so apps can annotate per-segment
    { "language": "<fine BCP 47 tag>", "start": 0, "end": 0, "text": "..." }
  ],
  "normalizedText": "..."                       // the analyzed form (NFC, whitespace-collapsed)
}
```

`candidates` is macro-grouped for consumer apps (a single `zh` entry
covers `zh-Hans`/`zh-Hant`/`yue`/`lzh`/`nan`/`hak`/`wuu`/…). The fine
distinction is available via `primaryVariant` and `segments[*].language`.
See [`SOFTWARE_DESIGN.md` §5](./SOFTWARE_DESIGN.md#5-output-schema) for
the full field semantics, macro grouping table, and status definitions.

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
│   │   │   │   └── lexicon_{en,fr,ja,ko,vi,zh_hans,zh_hant,zh_hant_hk,zh_hant_tw,yue,lzh,nan,hak,wuu}.txt  # 10 K words each
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
│   ├── language-identifier-cli/            # binary crate
│   │   └── src/main.rs
│   └── language-identifier-ffi/            # native bindings (UniFFI + C ABI)
│       ├── src/
│       │   ├── lib.rs                      # UniFFI proc-macro exports (Swift / Kotlin)
│       │   ├── c_abi.rs                    # extern "C" surface (Windows / Linux / generic)
│       │   └── bin/uniffi-bindgen.rs
│       ├── build.rs + cbindgen.toml        # emits target/c-header/language_identifier.h
│       ├── uniffi.toml                     # Swift module + Kotlin package names
│       └── scripts/build-{apple,android,windows,linux}.sh
├── SOFTWARE_DESIGN.md                      # full design document
└── language-identifier-library.drawio      # diagram source of truth
```

## Known v3 limitations

- **`HanAmbiguous` spans still surface as `zh`** (the variant-unclear umbrella
  tag). Disambiguating an isolated Han snippet to `ja` vs `zh-Hans` vs
  `zh-Hant` *without* kana or markers requires Layer 9 (the bundled
  OpenLID classifier) or an application-layer LLM tie-breaker.
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
