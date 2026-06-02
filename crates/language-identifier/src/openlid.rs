//! OpenLID v3 label → BCP 47 mapping.
//!
//! OpenLID v3 emits `iso639-3_Script` labels (e.g. `eng_Latn`,
//! `cmn_Hans`, `yue_Hant`) — the NLLB-200 / FLORES-200 family of tags.
//! This module is the single source of truth that translates those
//! labels into the library's canonical BCP 47 tag set.
//!
//! ## Two-tier mapping
//!
//! 1. **Static lookup.** A hand-curated table maps each OpenLID label
//!    to its canonical BCP 47 tag (prefer two-letter ISO 639-1 where
//!    one exists; fall back to ISO 639-3 / BCP 47-permitted forms for
//!    the rest). Labels outside the table return `None` and that
//!    prediction is silently dropped from Layer 9 output.
//!
//! 2. **Sinitic Tier-2 delegation.** `cmn_Hans` and `cmn_Hant` are
//!    forwarded to [`crate::bcp47::resolve_zh`], which can promote to
//!    `yue` / `lzh` / `zh-Hant-HK` / `zh-Hant-TW` based on the text
//!    itself. This keeps the existing Han-script disambiguation
//!    pipeline load-bearing even though OpenLID also ships dedicated
//!    `yue_Hant` and `lzh_Hani` labels — when those fire directly,
//!    they map straight through.
//!
//! ## Supported tier
//!
//! Of the 194 OpenLID labels, fourteen — `en`, `fr`, `vi`, `ja`, `ko`,
//! `zh-Hans`, `zh-Hant`, `zh-Hant-HK`, `zh-Hant-TW`, `yue`, `lzh`,
//! `nan`, `hak`, `wuu` — get the full multi-layer pipeline (scripts,
//! lexicons, morphology, orthography markers). The remaining 180 are
//! **ML-only**: they appear as candidates when OpenLID is confident
//! but have no segment-level attribution. Calibration may report
//! `Status::Ambiguous` on ML-only inputs because the Layer 9 bonus is
//! capped — see SOFTWARE_DESIGN.md §6 for the limitation.

use crate::bcp47;

/// Map an OpenLID v3 label (e.g. `eng_Latn`, `cmn_Hans`, `__label__yue_Hant`)
/// to the library's canonical BCP 47 tag. Tolerates the `__label__`
/// prefix so callers can pass fastText predictions verbatim.
///
/// `text` is the normalized input the classifier saw. It is consulted
/// only for the Sinitic Tier-2 delegation (`cmn_Hans` / `cmn_Hant`).
///
/// Returns `None` for labels outside the curated set.
pub(crate) fn from_openlid_label(label: &str, text: &str) -> Option<&'static str> {
    let stripped = strip_label_prefix(label);
    match stripped {
        // Sinitic Tier-2 delegation: let resolve_zh inspect the text
        // and pick the most specific tag the input supports.
        "cmn_Hans" | "cmn_Hant" => Some(bcp47::resolve_zh(text)),
        other => static_lookup(other),
    }
}

/// Whether `label` is in the curated 194-row set — i.e. whether
/// [`from_openlid_label`] would return `Some` for some input.
///
/// Used by `OpenLidClassifier::unsupported_signal` to decide when the
/// classifier's top label warrants short-circuiting the pipeline to
/// [`crate::Status::Unsupported`]. For a 194-language model trained on
/// the labels in this table, this always returns `true` in practice —
/// the check is defense-in-depth for off-list output.
pub(crate) fn is_supported_openlid_label(label: &str) -> bool {
    let stripped = strip_label_prefix(label);
    matches!(stripped, "cmn_Hans" | "cmn_Hant") || static_lookup(stripped).is_some()
}

fn strip_label_prefix(s: &str) -> &str {
    s.strip_prefix("__label__").unwrap_or(s)
}

// The curated OpenLID v3 label → BCP 47 table.
//
// Source: the NLLB-200 / FLORES-200 language identifier label set
// adopted by OpenLID v3. Refresh this table when adopting newer model
// versions; off-list labels fall through to `None` and are silently
// dropped from Layer 9 output.
fn static_lookup(label: &str) -> Option<&'static str> {
    Some(match label {
        // -- Tier 1 (full pipeline) ----------------------------------
        "eng_Latn" => "en",
        "fra_Latn" => "fr",
        "vie_Latn" => "vi",
        "jpn_Jpan" => "ja",
        "kor_Hang" => "ko",
        "yue_Hant" => "yue",
        "lzh_Hani" => "lzh",
        // OpenLID emits nan / hak / wuu in Han script. Latin
        // romanization variants (if shipped) collapse to the same tag.
        "nan_Hani" | "nan_Latn" => "nan",
        "hak_Hani" | "hak_Latn" => "hak",
        "wuu_Hans" | "wuu_Hant" => "wuu",

        // -- Tier 2 (ML-only) ---------------------------------------
        // Major world languages with two-letter ISO 639-1 codes.
        "afr_Latn" => "af",
        "amh_Ethi" => "am",
        "arb_Arab" | "arb_Latn" => "ar",
        "asm_Beng" => "as",
        "ast_Latn" => "ast",
        "ayr_Latn" => "ay",
        "azj_Latn" => "az",
        "bak_Cyrl" => "ba",
        "bel_Cyrl" => "be",
        "ben_Beng" => "bn",
        "bod_Tibt" => "bo",
        "bos_Latn" => "bs",
        "bul_Cyrl" => "bg",
        "cat_Latn" => "ca",
        "ceb_Latn" => "ceb",
        "ces_Latn" => "cs",
        "cym_Latn" => "cy",
        "dan_Latn" => "da",
        "deu_Latn" => "de",
        "dzo_Tibt" => "dz",
        "ell_Grek" => "el",
        "epo_Latn" => "eo",
        "est_Latn" => "et",
        "eus_Latn" => "eu",
        "ewe_Latn" => "ee",
        "fao_Latn" => "fo",
        "fij_Latn" => "fj",
        "fin_Latn" => "fi",
        "fon_Latn" => "fon",
        "gla_Latn" => "gd",
        "gle_Latn" => "ga",
        "glg_Latn" => "gl",
        "grn_Latn" => "gn",
        "guj_Gujr" => "gu",
        "hat_Latn" => "ht",
        "hau_Latn" => "ha",
        "heb_Hebr" => "he",
        "hin_Deva" => "hi",
        "hrv_Latn" => "hr",
        "hun_Latn" => "hu",
        "hye_Armn" => "hy",
        "ibo_Latn" => "ig",
        "ilo_Latn" => "ilo",
        "ind_Latn" => "id",
        "isl_Latn" => "is",
        "ita_Latn" => "it",
        "jav_Latn" => "jv",
        "kan_Knda" => "kn",
        "kat_Geor" => "ka",
        "kaz_Cyrl" => "kk",
        "khm_Khmr" => "km",
        "kik_Latn" => "ki",
        "kin_Latn" => "rw",
        "kir_Cyrl" => "ky",
        "kmr_Latn" => "kmr",
        "kon_Latn" => "kg",
        "lao_Laoo" => "lo",
        "lin_Latn" => "ln",
        "lit_Latn" => "lt",
        "ltz_Latn" => "lb",
        "lug_Latn" => "lg",
        "luo_Latn" => "luo",
        "lvs_Latn" => "lv",
        "mal_Mlym" => "ml",
        "mar_Deva" => "mr",
        "mkd_Cyrl" => "mk",
        "mlt_Latn" => "mt",
        "mri_Latn" => "mi",
        "mya_Mymr" => "my",
        "nld_Latn" => "nl",
        "nno_Latn" => "nn",
        "nob_Latn" => "nb",
        "npi_Deva" => "ne",
        "nso_Latn" => "nso",
        "nya_Latn" => "ny",
        "oci_Latn" => "oc",
        "ory_Orya" => "or",
        "pan_Guru" => "pa",
        "pbt_Arab" => "ps",
        "pes_Arab" => "fa",
        "plt_Latn" => "mg",
        "pol_Latn" => "pl",
        "por_Latn" => "pt",
        "prs_Arab" => "prs",
        "quy_Latn" => "qu",
        "ron_Latn" => "ro",
        "run_Latn" => "rn",
        "rus_Cyrl" => "ru",
        "sag_Latn" => "sg",
        "san_Deva" => "sa",
        "sin_Sinh" => "si",
        "slk_Latn" => "sk",
        "slv_Latn" => "sl",
        "smo_Latn" => "sm",
        "sna_Latn" => "sn",
        "snd_Arab" => "sd",
        "som_Latn" => "so",
        "sot_Latn" => "st",
        "spa_Latn" => "es",
        "srp_Cyrl" => "sr",
        "ssw_Latn" => "ss",
        "sun_Latn" => "su",
        "swe_Latn" => "sv",
        "swh_Latn" => "sw",
        "tam_Taml" => "ta",
        "tat_Cyrl" => "tt",
        "tel_Telu" => "te",
        "tgk_Cyrl" => "tg",
        "tgl_Latn" => "tl",
        "tha_Thai" => "th",
        "tir_Ethi" => "ti",
        "tsn_Latn" => "tn",
        "tso_Latn" => "ts",
        "tuk_Latn" => "tk",
        "tur_Latn" => "tr",
        "twi_Latn" => "tw",
        "uig_Arab" => "ug",
        "ukr_Cyrl" => "uk",
        "urd_Arab" => "ur",
        "uzn_Latn" => "uz",
        "war_Latn" => "war",
        "wol_Latn" => "wo",
        "xho_Latn" => "xh",
        "ydd_Hebr" => "yi",
        "yor_Latn" => "yo",
        "zsm_Latn" => "ms",
        "zul_Latn" => "zu",

        // Languages without an ISO 639-1 code — keep the ISO 639-3
        // form (BCP 47 allows it).
        "ace_Arab" | "ace_Latn" => "ace",
        "acm_Arab" => "acm",
        "acq_Arab" => "acq",
        "aeb_Arab" => "aeb",
        "ajp_Arab" => "ajp",
        "aka_Latn" => "ak",
        "apc_Arab" => "apc",
        "ars_Arab" => "ars",
        "ary_Arab" => "ary",
        "arz_Arab" => "arz",
        "awa_Deva" => "awa",
        "azb_Arab" => "azb",
        "bam_Latn" => "bm",
        "ban_Latn" => "ban",
        "bem_Latn" => "bem",
        "bho_Deva" => "bho",
        "bjn_Arab" | "bjn_Latn" => "bjn",
        "bug_Latn" => "bug",
        "cjk_Latn" => "cjk",
        "ckb_Arab" => "ckb",
        "crh_Latn" => "crh",
        "dik_Latn" => "dik",
        "dyu_Latn" => "dyu",
        "fur_Latn" => "fur",
        "fuv_Latn" => "fuv",
        "gaz_Latn" => "gaz",
        "hne_Deva" => "hne",
        "kab_Latn" => "kab",
        "kac_Latn" => "kac",
        "kam_Latn" => "kam",
        "kas_Arab" | "kas_Deva" => "ks",
        "kbp_Latn" => "kbp",
        "kea_Latn" => "kea",
        "khk_Cyrl" => "khk",
        "kmb_Latn" => "kmb",
        "knc_Arab" | "knc_Latn" => "knc",
        "lij_Latn" => "lij",
        "lim_Latn" => "li",
        "lmo_Latn" => "lmo",
        "ltg_Latn" => "ltg",
        "lua_Latn" => "lua",
        "lus_Latn" => "lus",
        "mag_Deva" => "mag",
        "mai_Deva" => "mai",
        "min_Latn" | "min_Arab" => "min",
        "mni_Beng" => "mni",
        "mos_Latn" => "mos",
        "nus_Latn" => "nus",
        "pag_Latn" => "pag",
        "pap_Latn" => "pap",
        "sat_Olck" => "sat",
        "scn_Latn" => "scn",
        "shn_Mymr" => "shn",
        "srd_Latn" => "sc",
        "szl_Latn" => "szl",
        "taq_Latn" | "taq_Tfng" => "taq",
        "tpi_Latn" => "tpi",
        "tum_Latn" => "tum",
        "tzm_Tfng" => "tzm",
        "umb_Latn" => "umb",
        "vec_Latn" => "vec",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tolerates_label_prefix() {
        assert_eq!(from_openlid_label("__label__eng_Latn", ""), Some("en"));
        assert_eq!(from_openlid_label("__label__yue_Hant", ""), Some("yue"));
    }

    #[test]
    fn tier1_round_trips() {
        for (label, tag) in [
            ("eng_Latn", "en"),
            ("fra_Latn", "fr"),
            ("vie_Latn", "vi"),
            ("jpn_Jpan", "ja"),
            ("kor_Hang", "ko"),
            ("yue_Hant", "yue"),
            ("lzh_Hani", "lzh"),
            ("nan_Hani", "nan"),
            ("hak_Hani", "hak"),
            ("wuu_Hans", "wuu"),
        ] {
            assert_eq!(from_openlid_label(label, ""), Some(tag), "{label}");
        }
    }

    #[test]
    fn sinitic_delegates_to_resolve_zh() {
        // Plain Han → defaults to Hans.
        assert_eq!(
            from_openlid_label("cmn_Hans", "我们今天去学校"),
            Some("zh-Hans")
        );
        assert_eq!(from_openlid_label("cmn_Hans", "Hello"), Some("zh-Hans"));

        // Hant-only chars → Hant (or HK/TW with region markers).
        assert_eq!(from_openlid_label("cmn_Hant", "學生"), Some("zh-Hant"));
        assert_eq!(
            from_openlid_label("cmn_Hant", "學生嘥時間"),
            Some("zh-Hant-HK")
        );
        assert_eq!(
            from_openlid_label("cmn_Hant", "臺灣的學生"),
            Some("zh-Hant-TW")
        );

        // Cantonese particles override the model's `cmn_Hant` guess.
        assert_eq!(from_openlid_label("cmn_Hant", "我嘅學生"), Some("yue"));

        // Dense classical function-word run promotes lzh.
        assert_eq!(
            from_openlid_label("cmn_Hans", "子曰：學而時習之，不亦說乎？"),
            Some("lzh")
        );
    }

    #[test]
    fn tier2_newcomers_lookup() {
        for (label, tag) in [
            ("spa_Latn", "es"),
            ("deu_Latn", "de"),
            ("rus_Cyrl", "ru"),
            ("arb_Arab", "ar"),
            ("tha_Thai", "th"),
            ("hin_Deva", "hi"),
            ("por_Latn", "pt"),
            ("ita_Latn", "it"),
            ("nld_Latn", "nl"),
            ("ckb_Arab", "ckb"),
            ("uig_Arab", "ug"),
        ] {
            assert_eq!(from_openlid_label(label, ""), Some(tag), "{label}");
        }
    }

    #[test]
    fn unknown_label_returns_none() {
        assert_eq!(from_openlid_label("xxx_Yyyy", ""), None);
        assert_eq!(from_openlid_label("", ""), None);
        assert_eq!(from_openlid_label("en", ""), None); // bare ISO-639-1 no longer recognized
    }

    #[test]
    fn is_supported_covers_known_labels() {
        for label in [
            "eng_Latn", "fra_Latn", "cmn_Hans", "cmn_Hant", "yue_Hant", "spa_Latn", "rus_Cyrl",
            "arb_Arab",
        ] {
            assert!(
                is_supported_openlid_label(label),
                "expected {label} supported"
            );
            assert!(is_supported_openlid_label(&format!("__label__{label}")));
        }
    }

    #[test]
    fn is_supported_rejects_off_list_labels() {
        for label in ["xxx_Yyyy", "en", ""] {
            assert!(
                !is_supported_openlid_label(label),
                "expected {label} unsupported"
            );
        }
    }
}
