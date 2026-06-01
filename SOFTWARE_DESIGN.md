# Language Identifier Library — Software Design Document

**Status:** Draft
**Date:** 2026-05-28
**Diagram source of truth:** [`language-identifier-library.drawio`](./language-identifier-library.drawio)

> Sections derived directly from the diagram are unmarked. Sections that go beyond the diagram are explicitly marked **(inferred)** so a reader can tell specification from extrapolation.

---

## 1. Overview

The Language Identifier Library identifies the natural language(s) of an arbitrary text input. It accepts a single string (a word, a phrase) or a `string[]` array where each element is one line of a paragraph, normalizes and combines the content, then runs the text through a multi-layer scoring pipeline. The library returns a ranked list of candidate languages with calibrated confidence scores, a primary language, and a status that distinguishes between confident results and ambiguous / mixed / unknown cases.

The library is designed to be robust on real-world text such as dictionary lookups, language-learning apps, and editor text-selection flows where input may contain embedded segments of a second language inside a primary one (for example, an English sentence quoting a Japanese phrase).

---

## 2. Goals

- Accept word, phrase, and multi-line paragraph (`string[]`) inputs.
- Identify language(s) using BCP 47 language tags (`en`, `fr`, `vi`, `ja`, `ko`, `zh-Hans`, `zh-Hant`, plus the Chinese variants `zh-Hant-HK`, `zh-Hant-TW`, `yue`, `lzh`, `nan`, `hak`, `wuu`). Locale subtags (`en-US`, `vi-VN`, `zh-CN`, …) are deliberately out of scope — see §3. The two HK/TW Hant subtags are the only locale-tagged exceptions.
- Return a ranked candidate list with per-candidate `confidence`, a `primaryLanguage`, and a `status` of `resolved | ambiguous | mixed | unknown | unsupported`.
- Disambiguate visually overlapping scripts (e.g. Han characters shared by Japanese and Chinese; Latin script shared by English and Vietnamese) using script signals, orthographic rules, dictionaries, and surrounding context.
- Surface a `reasons` array that explains the decision so the result is auditable.

## 3. Non-goals *(inferred)*

- Not a translator.
- Not a script converter (e.g. Hans ↔ Hant, romanization).
- Not a general-purpose NLP pipeline (no POS, parsing, NER beyond what language ID needs).
- Not a per-token segmentation API in v1; the schema returns a single `primaryLanguage`. Per-segment / embedded-segment output is listed under Open Questions.
- Not a preference-aware ranker. The library reports what is in the input text; user preferences (preferred / target language, app locale, lookup history) are an application concern and are applied on top of the returned `candidates` by the consumer — see §7.1 on Layer 8.

---

## 4. Input

The library accepts either:

| Input shape | Example |
| --- | --- |
| Single string | `"先生"`, `"university teacher"` |
| `string[]` (lines) | `["田中先生は大学で日本語を教えています。", "学生たちは毎日授業に参加し、新しい言葉や文法を学んでいます。", ...]` |

> *From the diagram:* "The input can be a word, a phrase, or a piece of text (a string array, where each element represents a line of text). The library will normalize and combine all of this to determine the language of the input content."

---

## 5. Output schema

```json
{
  "candidates": [
    { "language": "ja", "confidence": 0.52 },
    { "language": "zh-Hans", "confidence": 0.48 }
  ],
  "primaryLanguage": "ja",
  "status": "resolved|ambiguous|mixed|unknown|unsupported",
  "reasons": ["...", "..."]
}
```

### Fields

| Field | Type | Description |
| --- | --- | --- |
| `candidates` | array of `{ language: string, confidence: number }` | Ranked candidates with calibrated confidence in `[0, 1]`. |
| `primaryLanguage` | BCP 47 string | The selected primary language. May still be present when `status = "ambiguous"` (the highest-confidence candidate is reported even when the gap to the next is small). |
| `status` | enum | See below. |
| `reasons` | array of strings | Human-readable justification — one explanation line per element. Always an array, even for trivial cases (a one-element array). |

### `status` values

| Value | Meaning (derived from the diagram's test cases) |
| --- | --- |
| `resolved` | A single language clearly dominates; the confidence gap between #1 and the next candidate is significant. |
| `ambiguous` | Candidate confidences are close, or the input is too short to disambiguate (e.g. a single Han character valid in both Japanese and Chinese). |
| `mixed` | Multiple languages coexist in the input as first-class content (not just embedded snippets). |
| `unknown` | No supported language could be identified. |
| `unsupported` | The script or language detected is not in the supported set. |

---

## 6. Supported language codes

BCP 47 tags. The canonical set is:

```
en
fr           French (10K-entry lexicon + ç/œ/æ markers)
vi
ja
ko
zh-Hans      Simplified Chinese (mainland default)
zh-Hant      Traditional Chinese (generic)
zh-Hant-HK   Traditional Chinese, Hong Kong vocabulary
zh-Hant-TW   Traditional Chinese, Taiwan vocabulary
yue          Cantonese (written, colloquial particle 嘅嚟唔喺啲咁哋)
lzh          Classical / Literary Chinese (dense 之乎者也)
nan          Min Nan (Hokkien) — sparse signal
hak          Hakka — sparse signal
wuu          Wu (Shanghainese) — sparse signal
```

Locale subtags more generally (`en-US`, `en-GB`, `vi-VN`, `ja-JP`, `zh-CN`, …) are deliberately deferred — `zh-Hant-HK` and `zh-Hant-TW` are the only locale-tagged exceptions, because the HK/TW distinction is a routine ask for downstream apps and is decidable from a small character-level signal. Other locale tags will be added through the centralized `bcp47` module without changing call sites.

### 6.1 Detection precision per variant

The new variant tags are not equally robust. The deterministic Layer 3 / 4 signals available to each:

| Tag | Signal | Precision |
| --- | --- | --- |
| `zh-Hans` / `zh-Hant` | Hans-only / Hant-only character sets | high |
| `zh-Hant-HK` / `zh-Hant-TW` | Region-specific Hant chars (`嘥`/`冚`/`嘜` vs `臺`/`麵`/`裡`) | medium — most HK/TW text reads as plain `zh-Hant` because Standard Written Chinese in those regions is largely identical |
| `yue` | Cantonese-only particles (`嘅`/`嚟`/`唔`/`喺`/`啲`/`咁`/`哋`/`佢`/`係`/`咗`) | high for colloquial Cantonese, undefined for Mandarin written in Hant chars |
| `lzh` | Classical function-word density (≥3 of `之乎者也矣焉哉而於以…` over ≥4 Han chars) | medium — works on Analects-style text, may miss compact classical aphorisms |
| `nan` / `hak` / `wuu` | Sparse diagnostic characters (`阮`, `𠊎`, `儂` …) | low — most Min Nan / Hakka / Wu text is written using Han chars indistinguishable from Mandarin. The tag fires only when one of the diagnostic chars appears |

---

## 7. Architecture: layered pipeline

The library is a staged pipeline. Input flows from the top (text input) through every layer to a final calibration step, then out to the JSON output. Earlier layers are cheap and deterministic; later layers are progressively more expensive and probabilistic.

| # | Layer | Purpose (from diagram) |
| --- | --- | --- |
| 0 | Normalization | Normalize the text to reduce noise and make the input stable before analysis. |
| 1 | Character / byte n-gram scoring | Estimate candidate languages based on character or byte patterns commonly seen in each language. |
| 2 | Script & Unicode signal | Detect writing systems and Unicode blocks to quickly narrow down possible languages. |
| 3 | Orthographic / writing-system rules | Use language-specific writing clues such as kana, Vietnamese diacritics, simplified/traditional characters, or special punctuation. |
| 4 | Function words / particles / stopwords | Identify common function words, particles, or stop-words that reveal the natural structure of a language. |
| 5 | Dictionary / lexicon matching | Check whether words or phrases exist in one or more language dictionaries and detect shared ambiguous terms. |
| 6 | Morphology / tokenization hints | Analyze word forms and tokenization patterns to detect grammar-specific language signals. |
| 7 | Context window scoring | Use surrounding sentences, paragraphs, or page context to resolve ambiguous words or phrases. |
| 8 | User preference / app state | **Out of library scope** (see §7.1). The diagram lists this layer, but a content-only detector should not bias its verdict by user state. Consumers apply preferences on top of the returned `candidates`. |
| 9 | Lightweight ML classifier / embedding classifier | Use a lightweight model to combine features and rerank candidate languages. |
| 10 | LLM / Foundation Model resolver | **Out of library scope** (see §7.1). The diagram lists this layer for completeness, but exploration showed it didn't change the user-visible result on the canonical test cases and required a substantial platform-specific bridge. Consumers needing an LLM tie-breaker wrap `identify` in their app layer. |
| — | Final calibration & ambiguity handling | Calibrate the final confidence score and decide whether to return a language or mark the result as ambiguous. |

### 7.1 Layer notes

- **Layers 0–6** are deterministic, fast, and dictionary/rule-based. They produce the initial candidate distribution.
- **Layer 7 (Context window)** uses surrounding text to disambiguate single words shared across languages — critical for dictionary-app flows where a user selects one CJK character.
- **Layer 8 (User preference)** is **not implemented in this library and is not planned.** A language detector should faithfully report what is in the text. User preferences — preferred / target language, app locale, lookup history — are application-level concerns and belong in the consumer: rerank, filter, or hide candidates returned by `identify` according to the host app's policy. This keeps the library deterministic and content-only, with no hidden side channel into the verdict.
- **Layer 9 (Lightweight ML)** reranks the candidates produced by 0–8 using learned features.
- **Layer 10 (LLM)** is **deliberately out of library scope**, analogous to Layer 8. We prototyped an Apple `FoundationModels` backend in v4.2 and learned three things: (1) on the canonical `JA_ZH` (M3) test case the LLM only re-confirms the primary that Layers 0–9 already pick (`ja` at confidence 0.79), so the user-visible JSON output is unchanged with or without it; (2) on single shared CJK characters (W1) the LLM cannot beat the existing "stay Ambiguous" behavior, by design; (3) any concrete backend (Apple FoundationModels, llama.cpp, remote API) brings substantial platform-specific build/runtime complexity for marginal benefit. Consumers who need an LLM tie-breaker should wrap `identify` in their application layer where they can pick a backend appropriate to their privacy, latency, and platform constraints. The diagram retains the layer to preserve architectural fidelity.
- **v4.1 status**: Layer 9's default impl `FastTextClassifier` ships behind the `ml-fasttext` cargo feature, backed by Meta/FAIR's `lid.176.bin` (pure-Rust `fasttext` crate v0.8 — no C++ toolchain needed). The classifier intentionally drops fastText's `zh` label so Hans/Hant disambiguation stays in Layer 3.

### 7.2 Final calibration & ambiguity handling

After every active layer reports, the calibration step:

1. Combines scores into a single confidence distribution over candidate languages.
2. Measures the **confidence gap** between the top candidate and the next.
   - A **significant** gap → `status: "resolved"`, the top candidate becomes `primaryLanguage`, other languages present in text are reported as embedded segments (low confidence, but still listed).
   - A **small** gap → `status: "ambiguous"`. The highest-confidence candidate is still reported as `primaryLanguage`; consumers can apply their own tie-breaker (e.g. an LLM, user preference, lookup history) on top of `candidates`.
3. When multiple languages coexist as first-class content rather than embedded snippets → `status: "mixed"`.

---

## 8. Test cases

All inputs and outputs below are reproduced verbatim from the diagram. They define the expected behavior of the library.

### 8.1 Single language — word examples

#### Case W1: Ambiguous Han character shared by JA and ZH

**INPUT:** `先生 | 教師 | 先生 | 老师`

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "ja", "confidence": 0.50 },
    { "language": "zh", "confidence": 0.50 }
  ],
  "status": "ambiguous",
  "reasons": ["This word is used in all above languages"]
}
```

#### Case W2: Latin word resolves to English

**INPUT:** `Teacher | Profesor`

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "en", "confidence": 1.0 }
  ],
  "status": "resolved",
  "reasons": []
}
```

### 8.2 Single language — phrase examples

#### Case P1: Japanese phrases

**INPUT:** `大学の先生 | 日本語の教師 | 高校の先生 | 英語を教える先生 | 尊敬される教授`

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "ja", "confidence": 1 }
  ],
  "status": "resolved",
  "reasons": ["Japanese kana is detected among Han's characters"],
  "primaryLanguage": "ja"
}
```

#### Case P2: Simplified Chinese phrases

**INPUT:** `大学老师 | 中文老师 | 高中老师 | 教英语的老师 | 受人尊敬的教授`

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "zh-Hans", "confidence": 1.0 }
  ],
  "status": "resolved",
  "reasons": ["Chinese simplified"],
  "primaryLanguage": "zh-Hans"
}
```

#### Case P3: English phrases

**INPUT:** `university teacher | Japanese language teacher | high school teacher | teacher of English | respected professor`

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "en", "confidence": 1.0 }
  ],
  "status": "resolved",
  "reasons": ["All English US detected"],
  "primaryLanguage": "en"
}
```

#### Case P4: Vietnamese phrases

**INPUT:** `giáo viên đại học | giáo viên tiếng Nhật | giáo viên trung học | thầy giáo dạy tiếng Anh | giáo sư được kính trọng`

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "vi", "confidence": 1.0 }
  ],
  "status": "resolved",
  "reasons": ["All Vietnamese detected"],
  "primaryLanguage": "vi"
}
```

### 8.3 Single language — paragraph examples (`string[]`)

#### Case G1: Japanese paragraph

**INPUT:**

```json
[
  "田中先生は大学で日本語を教えています。",
  "学生たちは毎日授業に参加し、新しい言葉や文法を学んでいます。",
  "先生はとても親切で、難しい質問にも丁寧に答えてくれます。"
]
```

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "ja", "confidence": 1 }
  ],
  "status": "resolved",
  "reasons": ["Japanese kana is detected among Han's characters"],
  "primaryLanguage": "ja"
}
```

#### Case G2: Simplified Chinese paragraph

**INPUT:**

```json
[
  "王老师在大学教中文。",
  "学生们每天来上课，学习新的词语和语法。",
  "老师很有耐心，也会认真回答学生的问题。"
]
```

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "zh-Hans", "confidence": 1.0 }
  ],
  "status": "resolved",
  "reasons": ["Chinese simplified"],
  "primaryLanguage": "zh-Hans"
}
```

#### Case G3: English paragraph

**INPUT:**

```json
[
  "Mr. Tanaka is a teacher at the university.",
  "His students attend class every day and learn new vocabulary and grammar.",
  "He is very kind and always answers difficult questions carefully."
]
```

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "en", "confidence": 1.0 }
  ],
  "status": "resolved",
  "reasons": ["All English US detected"],
  "primaryLanguage": "en"
}
```

#### Case G4: Vietnamese paragraph

**INPUT:**

```json
[
  "Thầy Tanaka là giáo viên ở trường đại học.",
  "Các sinh viên tham gia lớp học mỗi ngày và học thêm từ vựng cũng như ngữ pháp mới.",
  "Thầy rất tận tâm và luôn trả lời cẩn thận những câu hỏi khó của sinh viên."
]
```

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "vi", "confidence": 1.0 }
  ],
  "status": "resolved",
  "reasons": ["All Vietnamese detected"],
  "primaryLanguage": "vi"
}
```

### 8.4 Mixed language — paragraph examples (`string[]`)

#### Case M1: `EN_JA` — English carrier, Japanese embedded

**INPUT:**

```json
[
  "When the user selects the word 先生 from a Japanese article, the library should not only look at the word itself but also analyze the surrounding sentence, such as 先生は大学で日本語を教えています。",
  "This dictionary app should detect that the main sentence is English, while the embedded phrase 日本語を勉強する belongs to Japanese and provides useful context for the selected word.",
  "If a user writes I want to understand the meaning of 大学の先生 in this sentence, the detector should recognize that English is the main language and Japanese appears as an embedded phrase."
]
```

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "en", "confidence": 0.74 },
    { "language": "ja", "confidence": 0.05 }
  ],
  "status": "resolved",
  "reasons": [
    "There are English and Japanese (Han traditional combined using with Kana characters), so language code [en, ja] are picked candidates",
    "English words is 74.68%, Japanese Kana is 5.61%",
    "The candidate confidence gap is significant so en is picked as the primary language, Japanese text is just embedded language segment"
  ],
  "primaryLanguage": "en"
}
```

#### Case M2: `EN_ZH` — English carrier, Simplified Chinese embedded

**INPUT:**

```json
[
  "When the user selects the word 先生 from a Chinese article, the library should inspect the full sentence, such as 王先生今天在大学教中文, before deciding whether the text is Chinese or Japanese.",
  "This app should understand that the sentence is mainly English, but the phrase 中文老师在大学上课 is Chinese and should be treated as an embedded language segment.",
  "If the input says Please explain why 老师 and 先生 can both refer to a teacher or a respectful title in Chinese, the detector should mark English as the main language and Chinese as embedded content."
]
```

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "en", "confidence": 0.74 },
    { "language": "zh-Hans", "confidence": 0.05 }
  ],
  "status": "resolved",
  "reasons": [
    "There are English and Han simplified. The phrases embedded here (王先生今天在大学教中文 and 中文老师在大学上课) follow Chinese grammar rules directly without any Japanese particles. The characters '老师' (teacher), '学' (study/university), and '国' (implied in country terms) are explicitly written in their Simplified Chinese forms. No Japanese Kana Presence. therefor language code [en, zh-Hans] are picked candidates",
    "English words is 78.49%, Chinese Simplified is 4.37%",
    "The candidate confidence gap is significant so en is picked as the primary language, Chinese Simplified text is just embedded language segment"
  ],
  "primaryLanguage": "en"
}
```

#### Case M3: `JA_ZH` — Japanese carrier with Chinese examples, ambiguity broken by LLM

**INPUT:**

```json
[
  "日本語の文の中に 中文老师在大学教中文 という中国語の例文が含まれている場合、システムは日本語を主言語として扱い、中国語部分を別のセグメントとして検出する必要があります。",
  "この入力では 先生 という言葉が日本語にも中国語にも存在するため、王先生今天不在 という中国語の文脈を使って判断することが重要です。",
  "日本語では 先生は大学で日本語を教えています と言えますが、中国語では 王老师在大学教中文 のように表現するため、両方の言語が混在していることを検出する必要があります。"
]
```

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "ja", "confidence": 0.39 },
    { "language": "zh-Hant", "confidence": 0.30 },
    { "language": "zh-Hans", "confidence": 0.09 }
  ],
  "status": "ambiguous",
  "reasons": [
    "The high presence of Kana (39.37%) spread evenly across the entire text acts as the structural spine, overall sentence grammar and carrier language is Japanese (picked as the primaryLanguage). Words like 先生 or 大学 make up 30.43% of the text and are valid in Chinese and Japanese. Characters 老师 is strictly Simplified Chinese and never used in Japanese. Therefor language code [ja, zh-Hant, zh-Hans] are picked candidates",
    "Japanese Kana is 39.37%, Chinese Traditional is 30.43%, Chinese Simplified is 9.42%",
    "The candidate confidence gap is small, so detection status is 'ambiguous'. Local LLM is used to validated the context of input text, Japanese is primaryLanguage, Chinese text is just embedded language segment"
  ],
  "primaryLanguage": "en"
}
```

> **Note:** The `primaryLanguage` value `"en"` in this case appears inconsistent with the reason text, which says "Japanese is primaryLanguage". Preserved verbatim from the diagram; should be reconciled (likely should be `"ja"`).

#### Case M4: `EN_JA_ZH_VI` — four-language meta-discussion

**INPUT:**

```json
[
  "This language detection library should support English, tiếng Việt, 日本語, and 中文 in the same input, especially when a user explains that 先生 can appear in both Japanese and Chinese contexts.",
  "When the input says Tôi muốn compare the Japanese sentence 先生は大学で日本語を教えています with the Chinese sentence 王老师在大学教中文, the detector should return a mixed-language result instead of forcing one language.",
  "A real user may write Please translate câu này sang tiếng Việt: 田中先生は大学で日本語を教えています, so the library needs to detect English, Vietnamese, and Japanese in one line."
]
```

**OUTPUT:**

```json
{
  "candidates": [
    { "language": "en", "confidence": 0.66 },
    { "language": "vi", "confidence": 0.06 },
    { "language": "zh-Hant", "confidence": 0.05 },
    { "language": "ja", "confidence": 0.02 },
    { "language": "zh-Hans", "confidence": 0.00 }
  ],
  "status": "ambiguous",
  "reasons": [
    "The input acts as a meta-language discussion where English functions as the carrier script across all three segments. Vietnamese is injected through short colloquial phrases sharing the Latin alphabet framework but identified by exclusive diacritic combinations (tiếng Việt, Tôi muốn). The CJK cluster contains overlapping Hanzi/Kanji (先生, 大学) which natively maps to both Japanese and Traditional Chinese, though specific Kana indicators (は, で, を) anchor the target examples to Japanese, while a single instance of 老师 targets Simplified Chinese. Therefore, [en, vi, zh-Hant, ja] are selected as the primary candidates.",
    "English is 66.42%, Vietnamese is 6.42%, Chinese Traditional/Kanji is 5.87%, Japanese Kana is 2.94%, Chinese Simplified is 0.37%",
    "Multiple distinct language systems coexist within single-sentence boundaries, rendering a single-result classification invalid. Contextual mapping verifies that English handles the primary syntax framework, while Vietnamese, Japanese, and Chinese serve strictly as nested, embedded reference segments."
  ],
  "primaryLanguage": "en"
}
```

### 8.5 Mixed language inputs — pending expected outputs

These mix combinations appear as INPUT examples in the diagram (page 1 / page 5 corpus block) but have no paired OUTPUT block. They are listed here as known scenarios the library must handle once expected outputs are defined.

#### `EN_VI`

```json
[
  "The user may write an English sentence like I want to translate giáo viên tiếng Nhật into Japanese, so the detector should identify English as the main language and Vietnamese as an embedded phrase.",
  "In a language learning app, a sentence such as Please explain the difference between thầy giáo, giáo viên, and giáo sư contains English structure but several Vietnamese terms.",
  "When the input contains I am building a feature that helps users understand cụm từ tiếng Việt trong câu dài, the detector should recognize both English and Vietnamese."
]
```

#### `VI_JA`

```json
[
  "Tôi muốn hệ thống nhận diện được rằng câu này chủ yếu là tiếng Việt, nhưng cụm 先生は大学で日本語を教えています là tiếng Nhật và cần được xử lý như một đoạn nhúng.",
  "Khi người dùng chọn từ 先生 trong câu tiếng Nhật như 先生はとても親切です, ứng dụng nên dùng cả ngữ cảnh xung quanh thay vì chỉ dựa vào một từ đơn lẻ.",
  "Ứng dụng học ngôn ngữ cần hiểu rằng câu Tôi đang học cách dùng cụm 大学の先生 trong tiếng Nhật có cả tiếng Việt và một cụm tiếng Nhật."
]
```

#### `VI_ZH`

```json
[
  "Tôi muốn thư viện phát hiện rằng câu này chủ yếu là tiếng Việt, nhưng cụm 王先生今天在大学教中文 là tiếng Trung và không nên bị nhận nhầm thành tiếng Nhật.",
  "Khi người dùng nhập câu Hãy giải thích sự khác nhau giữa 老师 và 先生 trong tiếng Trung, hệ thống nên nhận diện tiếng Việt là ngôn ngữ chính và tiếng Trung là nội dung được nhúng.",
  "Ứng dụng nên xử lý tốt những câu như Tôi đang đọc một ví dụ tiếng Trung: 这位老师很有耐心, trong đó phần đầu là tiếng Việt còn phần sau là tiếng Trung."
]
```

---

## 9. Non-functional considerations *(inferred)*

The diagram is silent on hard numbers. The points below are reasonable expectations implied by the layered architecture; they should be confirmed before implementation.

| Concern | Expectation |
| --- | --- |
| **Latency** | Layers 0–6 are constant-time over input length and should be < 1 ms for short input. Layer 7 (context) scales with surrounding text. Layer 9 (ML) is bounded. Layer 10 is out of scope (see §7.1). |
| **Determinism** | Layers 0–8 are deterministic. Layers 9–10 are probabilistic; the calibration step should produce stable outputs given the same model versions. |
| **Memory** | Layer 5 (dictionaries) dominates static memory; should be lazy-loaded per supported language. |
| **Throughput** | The fast path (layers 0–6 only) is expected to be the common case; the Layer 9 path is acceptable for interactive use (word lookup, paragraph at a time), not high-throughput batch. |
| **Offline operation** | Layers 0–9 work offline. The library performs no network I/O at all. |

---

## 10. Error handling *(inferred)*

Behavior for edge cases not covered explicitly in the diagram:

| Condition | Result |
| --- | --- |
| Empty string or empty array | `{ status: "unknown", candidates: [] }` |
| Whitespace-only / punctuation-only input | `{ status: "unknown", candidates: [] }` |
| Script not in supported set (e.g. Arabic, Thai when only en/vi/ja/ko/zh are configured) | `{ status: "unsupported", candidates: [] }` |
| Latin-script input in an unsupported language (e.g. French, German, Spanish) | When a Layer 9 `MlClassifier` opts into [`unsupported_signal`](#71-layer-notes) and reports a confidence ≥ 0.50 on a non-supported tag, `{ status: "unsupported", candidates: [] }` with a reason naming the detected language. Without a classifier — or with a classifier that doesn't implement the signal — the Latin script defaults to `en` per Layer 3's vi-density rule, which is a known sharp edge (v3 behavior). |
| Conflicting strong signals across layers | `status: "ambiguous"` per the diagram's "confidence gap is small" rule. |

---

## 11. Security & privacy *(inferred)*

- **Input data exposure:** The library performs no network I/O. All computation is local and deterministic; Layer 10 (which would otherwise be the only network path) is deliberately out of scope (see §7.1). Consumers that add their own LLM tie-breaker on top of `identify` are responsible for any data-exposure decisions that entails.
- **User preference:** Layer 8 is intentionally out of scope (see §7.1) — the library never reads user preference, app locale, or history, so there is no preference data to protect or transmit.
- **No persistence:** The library should not retain input text after a call returns.
- **Logging:** Reasons returned in the output may quote portions of input. Consumers logging the output should be aware that input fragments may surface.

---

## 12. Open questions & future work *(inferred)*

### Resolved in v2 of the implementation

1. ~~**Confidence-gap threshold.**~~ Set in `calibration.rs`: `DOMINANT_THRESHOLD = 0.95`, `RESOLVED_GAP = 0.30`, `MIXED_FLOOR = 0.20`, `MIXED_TOP_CEILING = 0.70`, `MIXED_MIN_VISIBLE = 4`. Tuned against the SDD test cases.
4. ~~**Per-segment output.**~~ `IdentifyResult.segments: Vec<Segment>` added in v2. Byte offsets index into the **normalized** input text (NFC, whitespace collapsed). The list is omitted when the input is single-language or unknown/unsupported.
6. ~~**Status `"mixed"`.**~~ Produced when ≥2 candidates each hold ≥0.20 confidence, the top candidate is below 0.70, and the input has ≥4 visible chars. Below the size threshold the status falls back to `Ambiguous` (single shared characters are not "mixed-language content").

### Resolved in v3 of the implementation

5. ~~**Inconsistent `primaryLanguage` in Case M3.**~~ v3 implements the diagram's stated behavior: M3 returns `status: "ambiguous"` with `primaryLanguage: "ja"`. The diagram's `"en"` value in the JSON output is treated as a typo. Layer 7 (context window) detects the embedded Chinese phrases via an intra-sentence Han-run signal and downgrades Resolved → Ambiguous while keeping `ja` as the primary.
8. ~~**Dictionary-aware Latin sub-segmentation.**~~ Layer 6 (morphology / tokenization hints) now attributes each Latin token to `en` or `vi` using lexicon membership + VI syllable shape. Aggregate uses these per-token attributions to split the Latin script proportionally, producing genuine EN+VI `Mixed` status when both languages have meaningful content.

### Resolved by design (not implemented)

3. ~~**API surface for Layer 8.**~~ Layer 8 is **out of library scope** (see §7.1). A language detector should report what is in the input; biasing the verdict with user state (preferred language, app locale, lookup history) makes the library less truthful as a primitive and adds a hidden side channel into the result. Consumers apply preferences on top of the returned `candidates` — rerank, filter, or surface them in the UI according to host-app policy. There is no Layer 8 API and none is planned.
2. ~~**Layer 10 deployment (LLM bundled vs remote).**~~ Resolved by dropping Layer 10 from library scope entirely (see §7.1). A v4.2 prototype using Apple's on-device `FoundationModels` confirmed it didn't move the user-visible result on M3 (Layers 0–9 already pick `ja`) and required a substantial platform-specific bridge. Consumers needing an LLM tie-breaker wrap `identify` in their application layer where the bundled-vs-remote choice is theirs to make against their own privacy and latency budget.

### Still open

7. **Expected outputs for `EN_VI`, `VI_JA`, `VI_ZH`.** Listed in §8.5 as pending — fill in once decided.
9. **Segment offsets are into normalized text, not the caller's original input.** Callers needing to highlight spans in the original string need a v4 mapping back through NFC + whitespace collapse.
10. **Sentence splitting on Latin abbreviations** (`Mr.`, `Dr.`, `etc.`) over-splits sentences. Cosmetic only — per-token attributions still aggregate correctly to the same language — but the `reason` notes can be misleading.

---

## 13. Source

The authoritative diagram is [`language-identifier-library.drawio`](./language-identifier-library.drawio) in this repository. When this document and the diagram disagree, treat the diagram as the source for **architecture** and this document as the source for **prose specification** of input/output, status semantics, and edge cases.
