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
| 9. Lightweight ML classifier | ✅ v4.1 (default impl `FastTextClassifier` behind `ml-fasttext` feature) |
| 10. LLM resolver | ❌ out of scope (see [SDD §7.1](./SOFTWARE_DESIGN.md#71-layer-notes)) |
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
        vec![("en-US".into(), 0.92), ("vi-VN".into(), 0.04)]
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
[`FastTextClassifier`](#layer-9-with-the-bundled-fasttext-default-v41) —
ships behind the `ml-fasttext` cargo feature; see below. Layer 10
(LLM tie-breaker) is deliberately out of library scope — see
[SDD §7.1](./SOFTWARE_DESIGN.md#71-layer-notes). Consumers needing
LLM-based disambiguation should wrap `identify` in their application
layer.

### Layer 9 with the bundled fastText default (v4.1)

Enable the `ml-fasttext` feature to compile the default `MlClassifier`
backed by Meta/FAIR's `lid.176.bin`. The
[`fasttext`](https://crates.io/crates/fasttext) crate v0.8 used
underneath is a pure-Rust port — no `clang` / `cmake` / C++ toolchain
needed.

Download the model file (~126 MB) once:

```bash
curl -L -o lid.176.bin \
  https://dl.fbaipublicfiles.com/fasttext/supervised-models/lid.176.bin
```

Use it from Rust:

```rust
use language_identifier::{
    identify_with, FastTextClassifier, IdentifyOptions,
};

let classifier = FastTextClassifier::builder()
    .model_path("./lid.176.bin")
    .build()?;
let opts = IdentifyOptions {
    ml_classifier: Some(&classifier),
};
let r = identify_with("Bonjour le monde", &opts);
```

Or use it from the CLI:

```bash
cargo run --features ml-fasttext -p language-identifier-cli -- \
  --ml-fasttext ./lid.176.bin --pretty "Bonjour le monde"
```

`FastTextClassifier` maps lid.176's labels into the library's BCP 47
set as follows:

| fastText label  | mapped tag |
| --------------- | ---------- |
| `__label__en`   | `en-US`    |
| `__label__vi`   | `vi-VN`    |
| `__label__ja`   | `ja`       |
| `__label__ko`   | `ko`       |
| `__label__zh`   | *dropped*  |
| any other       | *dropped*  |

`__label__zh` is intentionally dropped: lid.176 does not split
`zh-Hans` vs `zh-Hant`, and Hans/Hant disambiguation is the
deterministic job of Layer 3 (orthography markers). Layer 9 earns its
keep on cross-script ties, not on Han-variant decisions.

**Unsupported-language downgrade.** When `FastTextClassifier` is
confident the input is in a language outside the supported set (e.g.
French through this `{en, vi, ja, ko, zh-*}`-only library), the
pipeline returns `status: "unsupported"` instead of forcing the
verdict onto `en-US` via the Latin-script default. The threshold is
0.50 confidence; the `zh` umbrella label is treated as supported here
so pure-Han inputs are never downgraded. Custom `MlClassifier` impls
can opt into this behavior by overriding the trait's
[`unsupported_signal`](https://docs.rs/language-identifier/latest/language_identifier/trait.MlClassifier.html#method.unsupported_signal)
method (default returns `None`).

The `lid.176.bin` weights are licensed CC-BY-SA 3.0 by the upstream
authors; the library code stays MIT/Apache-2.0. See [`NOTICE`](./NOTICE).

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
artifact you ship can be consumed by either transport. The `ml-fasttext`
feature is intentionally not enabled in the FFI crate — mobile apps can't
realistically bundle the 126 MB model, and desktop consumers can call the
Rust API directly. Each build script's header comment lists its one-time
prerequisites (rustup targets, Android NDK, MinGW, `cross`, …).

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
  fastText classifier) or an application-layer LLM tie-breaker.
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
