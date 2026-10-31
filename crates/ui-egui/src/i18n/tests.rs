use super::catalog::{parse_entries, placeholders};
use super::*;

const JA: fn() -> Lang = || Lang::from_code("ja").expect("ja registered");
const ZH: fn() -> Lang = || Lang::from_code("zh-hant").expect("zh-hant registered");
const CS: fn() -> Lang = || Lang::from_code("cs").expect("cs registered");
const ID: fn() -> Lang = || Lang::from_code("id").expect("id registered");
const VI: fn() -> Lang = || Lang::from_code("vi").expect("vi registered");
const HI: fn() -> Lang = || Lang::from_code("hi").expect("hi registered");

#[test]
fn hindi_tags_resolve() {
    for tag in ["hi", "hi-IN", "hi_IN", "hi_IN.UTF-8"] {
        assert_eq!(lang_from_tag(tag), Some(HI()), "{tag}");
    }
    assert_eq!(HI().name(), "हिन्दी");
    assert_eq!(Lang::from_pref("hi"), HI());
    assert_eq!(Lang::from_pref("HI"), HI());
}

#[test]
fn indonesian_tags_resolve() {
    for tag in ["id", "id-ID", "id_ID", "id_ID.UTF-8"] {
        assert_eq!(lang_from_tag(tag), Some(ID()), "{tag}");
    }
    assert_eq!(ID().name(), "Bahasa Indonesia");
}

#[test]
fn vietnamese_tags_resolve() {
    for tag in ["vi", "vi-VN", "vi_VN", "vi_VN.UTF-8"] {
        assert_eq!(lang_from_tag(tag), Some(VI()), "{tag}");
    }
    assert_eq!(VI().name(), "Tiếng Việt");
    assert_eq!(Lang::from_pref("vi"), VI());
    assert_eq!(Lang::from_pref("VI"), VI());
}

#[test]
fn picker_lists_english_first_then_names_alphabetically() {
    let order = Lang::for_picker();
    assert_eq!(order.first().map(|l| l.code()), Some("en"));
    assert_eq!(order.len(), Lang::all().count());
    let keys: Vec<String> = order[1..].iter().map(|l| name_sort_key(l.name())).collect();
    assert!(keys.windows(2).all(|w| w[0] <= w[1]), "{keys:?}");
    let pos = |code: &str| order.iter().position(|l| l.code() == code).unwrap_or_else(|| panic!("{code} registered"));
    assert!(pos("cs") < pos("de"), "Čeština sorts under C, not after Z");
    assert!(pos("it") < pos("hu"), "Magyar sorts under M, not under its code");
    assert!(pos("tr") < pos("el"), "Latin names come before Greek");
    assert!(pos("uk") < pos("ja"), "Cyrillic names come before CJK");
    assert_eq!(name_sort_key("Tiếng Việt"), "tieng viet");
    assert_eq!(name_sort_key("Türkçe"), "turkce");
}

#[test]
fn tags_map_to_languages() {
    western_and_legacy_tags_resolve();
    chinese_tags_resolve_by_region_and_script();
}

fn western_and_legacy_tags_resolve() {
    assert_eq!(lang_from_tag("ja_JP.UTF-8"), Some(JA()));
    assert_eq!(lang_from_tag("ja-JP"), Some(JA()));
    assert_eq!(lang_from_tag("en_US.UTF-8"), Some(Lang::EN));
    assert_eq!(lang_from_tag("C"), Some(Lang::EN));
    assert_eq!(lang_from_tag("POSIX"), Some(Lang::EN));
    assert_eq!(lang_from_tag("cs_CZ.UTF-8"), Some(CS()));
    assert_eq!(lang_from_tag("cs-CZ"), Some(CS()));
    assert_eq!(lang_from_tag("fr_FR"), Lang::from_code("fr"));
    assert_eq!(lang_from_tag("de_DE"), Lang::from_code("de"));
    assert_eq!(lang_from_tag("de-AT"), Lang::from_code("de"));
    assert_eq!(lang_from_tag("it_IT.UTF-8"), Lang::from_code("it"));
    assert_eq!(lang_from_tag("it-CH"), Lang::from_code("it"));
    assert_eq!(lang_from_tag("nl_NL.UTF-8"), Lang::from_code("nl"));
    assert_eq!(lang_from_tag("nl-BE"), Lang::from_code("nl"));
    assert_eq!(lang_from_tag("hu_HU.UTF-8"), Lang::from_code("hu"));
    assert_eq!(lang_from_tag("hu-HU"), Lang::from_code("hu"));
    assert_eq!(lang_from_tag("tr_TR.UTF-8"), Lang::from_code("tr"));
    assert_eq!(lang_from_tag("tr-TR"), Lang::from_code("tr"));
    assert_eq!(lang_from_tag("tr"), Lang::from_code("tr"));
}

fn chinese_tags_resolve_by_region_and_script() {
    // Traditional Chinese: by region, by script, and with a region after the script.
    assert_eq!(lang_from_tag("zh_TW.UTF-8"), Some(ZH()));
    assert_eq!(lang_from_tag("zh-TW"), Some(ZH()));
    assert_eq!(lang_from_tag("zh-HK"), Some(ZH()));
    assert_eq!(lang_from_tag("zh_MO"), Some(ZH()));
    assert_eq!(lang_from_tag("zh-Hant"), Some(ZH()));
    assert_eq!(lang_from_tag("zh-Hant-TW"), Some(ZH()));
    assert_eq!(lang_from_tag("zh-Hant-HK"), Some(ZH()));
    // Simplified Chinese locales never pick up the Traditional catalog: they resolve to `zh-hans`.
    for tag in ["zh-CN", "zh_CN.UTF-8", "zh_SG", "zh-Hans", "zh-Hans-CN", "zh"] {
        assert_ne!(lang_from_tag(tag), Some(ZH()), "{tag}");
        assert_eq!(lang_from_tag(tag), Lang::from_code("zh-hans"), "{tag}");
    }
    assert_eq!(lang_from_tag(""), None);
    assert_eq!(lang_from_tag("_"), None);
}

#[test]
fn candidates_walk_from_specific_to_general() {
    assert_eq!(candidates("pt_BR.UTF-8"), ["pt-br", "pt"]);
    assert_eq!(candidates("zh_TW"), ["zh-tw", "zh-hant", "zh"]);
    assert_eq!(candidates("zh-CN"), ["zh-cn", "zh-hans", "zh"]);
    assert_eq!(candidates("zh-Hant-HK"), ["zh-hant-hk", "zh-hant", "zh"]);
    assert_eq!(candidates("nn_NO.UTF-8"), ["nn-no", "nb", "nn"]);
    assert_eq!(candidates("no"), ["nb", "no"]);
}

#[test]
fn preferences_resolve_with_fallback() {
    assert_eq!(Lang::from_pref("ja"), JA());
    assert_eq!(Lang::from_pref("JA"), JA());
    assert_eq!(Lang::from_pref("zh-hant"), ZH());
    assert_eq!(Lang::from_pref("ZH-Hant"), ZH());
    assert_eq!(Lang::from_pref("en"), Lang::EN);
    // `auto` and unknown codes follow the system (English under test).
    assert_eq!(Lang::from_pref("auto"), Lang::EN);
    assert_eq!(Lang::from_pref("xx-unknown"), Lang::EN);
}

#[test]
fn simplified_chinese_resolves_from_preferences() {
    let zh = Lang::from_code("zh-hans").expect("zh-hans registered");
    assert_eq!(Lang::from_pref("ZH-Hans"), zh);
    assert_eq!(tr(zh, "no such label"), "no such label");
}

#[test]
fn spanish_resolves_and_pluralises() {
    let es = Lang::from_code("es").expect("es registered");
    for tag in ["es", "es_ES.UTF-8", "es-MX", "es-419"] {
        assert_eq!(lang_from_tag(tag), Some(es), "{tag}");
    }
    let forms: Vec<usize> = [0, 1, 3].into_iter().map(es.0.plural).collect();
    assert_eq!(forms, [1, 0, 1]);
}

#[test]
fn swedish_resolves_and_pluralises() {
    let sv = Lang::from_code("sv").expect("sv registered");
    assert_eq!(sv.name(), "Svenska");
    for tag in ["sv", "SV", "sv-SE", "sv_SE.UTF-8", "sv-FI", "sv_FI.UTF-8", "sv-AX"] {
        assert_eq!(lang_from_tag(tag), Some(sv), "{tag}");
        assert_eq!(Lang::from_pref(tag), sv, "{tag}");
    }
    let forms: Vec<usize> = [0, 1, 2, 21].into_iter().map(sv.0.plural).collect();
    assert_eq!(forms, [1, 0, 1, 1]);
    assert_eq!(tr(sv, "File"), "Arkiv");
    assert_eq!(trn(sv, 1, "{n} layer", "{n} layers"), "1 lager");
    assert_eq!(trn(sv, 3, "{n} layer", "{n} layers"), "3 lager");
}

#[test]
fn finnish_resolves_and_pluralises() {
    let fi = Lang::from_code("fi").expect("fi registered");
    assert_eq!(fi.name(), "Suomi");
    for tag in ["fi", "FI", "fi-FI", "fi_FI.UTF-8"] {
        assert_eq!(lang_from_tag(tag), Some(fi), "{tag}");
        assert_eq!(Lang::from_pref(tag), fi, "{tag}");
    }
    // Filipino and Swedish in Finland don't pick Finnish.
    assert_ne!(lang_from_tag("fil"), Some(fi));
    assert_ne!(lang_from_tag("sv-FI"), Some(fi));
    assert_eq!(tr(fi, "File"), "Tiedosto");
    // The partitive singular follows any number but 1.
    for (n, layer) in [(0, "tasoa"), (1, "taso"), (2, "tasoa"), (21, "tasoa")] {
        assert_eq!(trn(fi, n, "{n} layer", "{n} layers"), format!("{n} {layer}"));
    }
}

#[test]
fn danish_resolves_and_pluralises() {
    let da = Lang::from_code("da").expect("da registered");
    assert_eq!(da.name(), "Dansk");
    for tag in ["da", "DA", "da-DK", "da_DK.UTF-8", "da-GL"] {
        assert_eq!(lang_from_tag(tag), Some(da), "{tag}");
        assert_eq!(Lang::from_pref(tag), da, "{tag}");
    }
    assert_eq!(tr(da, "File"), "Filer");
    assert_eq!(trn(da, 1, "{n} document", "{n} documents"), "1 dokument");
    assert_eq!(trn(da, 3, "{n} document", "{n} documents"), "3 dokumenter");
}

#[test]
fn norwegian_resolves_and_pluralises() {
    let nb = Lang::from_code("nb").expect("nb registered");
    assert_eq!(nb.name(), "Norsk bokmål");
    for tag in ["nb", "NB", "nb-NO", "nb_NO.UTF-8", "no", "no-NO", "no_NO.UTF-8", "nn", "nn-NO", "nn_NO.UTF-8"] {
        assert_eq!(lang_from_tag(tag), Some(nb), "{tag}");
        assert_eq!(Lang::from_pref(tag), nb, "{tag}");
    }
    assert_eq!(tr(nb, "File"), "Fil");
    assert_eq!(trn(nb, 1, "{n} item", "{n} items"), "1 element");
    assert_eq!(trn(nb, 3, "{n} item", "{n} items"), "3 elementer");
}

#[test]
fn hungarian_resolves_and_keeps_the_singular_after_counts() {
    let hu = Lang::from_code("hu").expect("hu registered");
    for tag in ["hu", "hu_HU.UTF-8", "hu-HU"] {
        assert_eq!(lang_from_tag(tag), Some(hu), "{tag}");
    }
    assert_eq!(hu.name(), "Magyar");
    let forms: Vec<usize> = [0, 1, 3, 21].into_iter().map(hu.0.plural).collect();
    assert_eq!(forms, [0, 0, 0, 0]);
}

#[test]
fn brazilian_portuguese_resolves_and_pluralises() {
    let pt = Lang::from_code("pt-br").expect("pt-br registered");
    for tag in ["pt", "pt-BR", "pt_BR.UTF-8", "pt-PT", "pt_PT.UTF-8"] {
        assert_eq!(lang_from_tag(tag), Some(pt), "{tag}");
    }
    assert_eq!(candidates("pt_PT"), ["pt-pt", "pt-br", "pt"]);
    let forms: Vec<usize> = [0, 1, 2].into_iter().map(pt.0.plural).collect();
    assert_eq!(forms, [0, 0, 1]);
}

#[test]
fn lookups_fall_back_to_english() {
    assert_eq!(tr(JA(), "no such label"), "no such label");
    assert_eq!(tr(Lang::EN, "Layer"), "Layer");
    assert_eq!(tr(ZH(), "no such label"), "no such label");
    assert_eq!(tr_id(ZH(), "no.such.id", "Layer"), tr(ZH(), "Layer"));
    assert_eq!(tr_id(JA(), "no.such.id", "Layer"), tr(JA(), "Layer"));
    assert_eq!(tr_ctx(JA(), "no such context", "Layer"), tr(JA(), "Layer"));
    assert_eq!(trn(JA(), 3, "{n} no such message", "{n} no such messages"), "3 no such messages");
}

#[test]
fn russian_plural_rule() {
    let ru = Lang::from_code("ru").expect("ru registered");
    let forms: Vec<usize> = [0, 1, 2, 5, 11, 12, 14, 21, 22, 25, 101, 111, 112].into_iter().map(ru.0.plural).collect();
    assert_eq!(forms, [2, 0, 1, 2, 2, 2, 2, 0, 1, 2, 0, 2, 2]);
}

#[test]
fn ukrainian_resolves_locales_and_preferences() {
    let uk = Lang::from_code("uk").expect("uk registered");
    assert_eq!(uk.name(), "Українська");
    for tag in ["uk", "UK", "uk-UA", "uk_UA", "uk_UA.UTF-8", "uk-UA@euro", "uk-Cyrl-UA"] {
        assert_eq!(lang_from_tag(tag), Some(uk), "{tag}");
        assert_eq!(Lang::from_pref(tag), uk, "{tag}");
    }
    assert_eq!(tr(uk, "unknown translation"), "unknown translation");
}

#[test]
fn ukrainian_plural_rule_handles_teens_and_compound_counts() {
    let uk = Lang::from_code("uk").expect("uk registered");
    let counts = [0, 1, 2, 4, 5, 11, 12, 14, 19, 21, 22, 24, 25, 100, 101, 111, 112, 114, 121, 122, u64::MAX];
    let forms: Vec<usize> = counts.into_iter().map(uk.0.plural).collect();
    assert_eq!(forms, [2, 0, 1, 1, 2, 2, 2, 2, 2, 0, 1, 1, 2, 2, 0, 2, 2, 2, 0, 1, 2]);
}

#[test]
fn arabic_resolves_locales_and_preferences() {
    let ar = Lang::from_code("ar").expect("ar registered");
    assert_eq!(ar.name(), "العربية");
    for tag in ["ar", "AR", "ar-SA", "ar_EG", "ar_EG.UTF-8", "ar-AE", "ar-MA"] {
        assert_eq!(lang_from_tag(tag), Some(ar), "{tag}");
        assert_eq!(Lang::from_pref(tag), ar, "{tag}");
    }
    assert_eq!(dir_for_code("ar"), Dir::Rtl);
}

#[test]
fn arabic_plural_rule_has_six_forms() {
    let ar = Lang::from_code("ar").expect("ar registered");
    let counts = [0, 1, 2, 3, 10, 11, 99, 100, 101, 102, 103, 111, 199, 1000, u64::MAX];
    let forms: Vec<usize> = counts.into_iter().map(ar.0.plural).collect();
    assert_eq!(forms, [0, 1, 2, 3, 3, 4, 4, 5, 5, 5, 3, 4, 4, 5, 4]);
}

#[test]
fn serbian_resolves_locales_and_preferences() {
    let sr = Lang::from_code("sr").expect("sr registered");
    assert_eq!(sr.name(), "Srpski");
    for tag in ["sr", "SR", "sr-RS", "sr_RS", "sr_RS.UTF-8", "sr_RS@latin", "sr-Latn", "sr-Latn-RS", "sr-Cyrl-RS", "sr-ME"] {
        assert_eq!(lang_from_tag(tag), Some(sr), "{tag}");
        assert_eq!(Lang::from_pref(tag), sr, "{tag}");
    }
}

#[test]
fn serbian_plural_rule_handles_teens_and_compound_counts() {
    let sr = Lang::from_code("sr").expect("sr registered");
    let counts = [0, 1, 2, 4, 5, 11, 12, 14, 21, 22, 25, 101, 111, 112, 122];
    let forms: Vec<usize> = counts.into_iter().map(sr.0.plural).collect();
    assert_eq!(forms, [2, 0, 1, 1, 2, 2, 2, 2, 0, 1, 2, 0, 2, 2, 1]);
}

#[test]
fn catalog_kinds_are_parsed_and_looked_up() {
    let c = Catalog::parse("# c\n\tHello\tこんにちは\n@id\tfile.save\t保存する\nmenu\tWindows\tウィンドウ群\n@plural\t{n} file|{n} files\t{n} 個\n\n");
    assert_eq!(c.plain("Hello"), Some("こんにちは"));
    assert_eq!(c.id("file.save"), Some("保存する"));
    assert_eq!(c.contextual("menu", "Windows"), Some("ウィンドウ群"));
    assert_eq!(c.contextual("other", "Windows"), None);
    assert_eq!(c.plural("{n} file", "{n} files", 0), Some("{n} 個"));
    assert_eq!(c.plural("{n} file", "{n} files", 5), Some("{n} 個"), "an index past the forms clamps");
}

#[test]
fn malformed_lines_are_reported_not_fatal() {
    let (entries, errors) = parse_entries("\tok\tはい\nno tabs here\n\tonly\n\ta\tb\tc\textra\n\t\tempty source\n");
    assert_eq!(entries.len(), 1);
    assert_eq!(errors.len(), 4, "{errors:?}");
    assert_eq!(parse_entries("\ta\\tb\tx\\ny\\\\z\n").0[0], (String::new(), "a\tb".into(), "x\ny\\z".into()));
}

#[test]
fn plurals_and_placeholders() {
    assert_eq!(trn(Lang::EN, 1, "{n} item", "{n} items"), "1 item");
    assert_eq!(trn(Lang::EN, 0, "{n} item", "{n} items"), "0 items");
    assert_eq!(trn(Lang::EN, 7, "{n} item", "{n} items"), "7 items");
    let cs = fixture_lang(plural_cs, "@plural\t{n} item|{n} items\t{n} one|{n} few|{n} other\n");
    for (n, expected) in [(0, "0 other"), (1, "1 one"), (3, "3 few"), (5, "5 other")] {
        assert_eq!(trn(cs, n, "{n} item", "{n} items"), expected);
    }
    let ja = fixture_lang(plural_none, "@plural\t{n} item|{n} items\t{n} only\n");
    assert_eq!(trn(ja, 1, "{n} item", "{n} items"), "1 only");
    assert_eq!(trn(ja, 7, "{n} item", "{n} items"), "7 only");
    assert_eq!(trn(ja, 7, "{n} file", "{n} files"), "7 files", "a missing row falls back to English");
    assert_eq!(fmt("{b} before {a}", &[("a", "x"), ("b", "y"), ("c", "z")]), "y before x");
    assert_eq!(fmt("{missing}", &[]), "{missing}");
    assert_eq!(placeholders("a {x} b {y} {"), ["x", "y"]);
}

#[test]
fn french_resolves_and_pluralises() {
    let fr = Lang::from_code("fr").expect("fr registered");
    for tag in ["fr", "fr_FR.UTF-8", "fr-CA", "fr_BE", "fr-CH"] {
        assert_eq!(lang_from_tag(tag), Some(fr), "{tag}");
    }
    let forms: Vec<usize> = [0, 1, 2, 5, 100, u64::MAX].into_iter().map(fr.0.plural).collect();
    assert_eq!(forms, [0, 0, 1, 1, 1, 1]);
}

/// A language whose catalog is `text`, for testing lookups without depending on a bundled
/// catalog's wording.
fn fixture_lang(plural: fn(u64) -> usize, text: &str) -> Lang {
    let catalog = OnceLock::new();
    let _ = catalog.set(Catalog::parse(text));
    Lang(Box::leak(Box::new(LangInfo { code: "xx", name: "x", source: b"", plural, complete_menus: false, catalog })))
}

#[test]
fn id_and_context_rows_win_over_the_plain_label() {
    let lang = fixture_lang(plural_one_other, "\tAll\tplain all\n@id\tselect.all\tid all\nnavigator\tZoom\tcontext zoom\n\tZoom\tplain zoom\n");
    assert_eq!(tr_id(lang, "select.all", "All"), "id all");
    assert_eq!(tr_id(lang, "edit.other", "All"), "plain all");
    assert_eq!(tr(lang, "All"), "plain all");
    assert_eq!(tr_ctx(lang, "navigator", "Zoom"), "context zoom");
    assert_eq!(tr_ctx(lang, "other", "Zoom"), "plain zoom");
    assert_eq!(tr_ctx(lang, "navigator", "Unlisted"), "Unlisted");
}

#[test]
fn polish_plural_rule() {
    let forms: Vec<usize> = [0, 1, 2, 4, 5, 12, 14, 21, 22, 25, 112, 122].into_iter().map(plural_polish).collect();
    assert_eq!(forms, [2, 0, 1, 1, 2, 2, 2, 2, 1, 2, 2, 1]);
}

#[test]
fn czech_plural_rule() {
    let forms: Vec<usize> = [0, 1, 2, 3, 4, 5, 11, 12, 21, 22, 100, u64::MAX].into_iter().map(plural_cs).collect();
    assert_eq!(forms, [2, 0, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2]);
}

/// The packed catalogs in the binary unpack to exactly the `.tsv` files.
#[test]
fn packed_catalogs_match_their_files() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/i18n");
    for l in LANGUAGES.iter().skip(1) {
        let file = std::fs::read_to_string(dir.join(format!("{}.tsv", l.code))).unwrap();
        assert_eq!(unpack(l.source).unwrap(), file, "{}", l.code);
        assert!(l.source.len() < file.len() / 2, "{}: stored deflated", l.code);
    }
    assert_eq!(LANGUAGES[0].text(), "", "English has no catalog");
}

/// Corrupt catalog data is an error (English is shown), not a panic.
#[test]
fn corrupt_catalog_data_is_an_error() {
    assert!(inflate(b"\xff\xff\xff\xff not deflate").is_err());
    let ja = LANGUAGES.iter().find(|l| l.code == "ja").unwrap();
    assert!(inflate(&ja.source[..ja.source.len() / 2]).is_err(), "truncated");
    let info: &'static LangInfo =
        Box::leak(Box::new(LangInfo { code: "xx", name: "x", source: b"\x00garbage", plural: plural_none, complete_menus: false, catalog: OnceLock::new() }));
    assert_eq!(info.text(), "");
    assert_eq!(tr(Lang(info), "Layer"), "Layer", "falls back to English");
}

/// Packed lines unpack to their keys; references to keys that don't exist are an error, not a
/// panic.
#[test]
fn packed_lines_unpack_and_bad_references_are_errors() {
    let keys = ["\tOpen", "@id\tfile.open", "\tClose"];
    let packed = "# c\n\x01Ouvrir\n\x02-1\tOuvrir\n\x02+1\tFermer\n\x03\x01x\n";
    assert_eq!(expand(packed, &keys).unwrap(), "# c\n\tOpen\tOuvrir\n\tOpen\tOuvrir\n\tClose\tFermer\n\x01x\n");
    for bad in ["\x02-1\tx", "\x02 3\tx", "\x023", "\x029223372036854775807\tx", "\x02-9223372036854775808\tx", "\x01a\n\x01b\n\x01c\n\x01d"] {
        assert!(expand(bad, &keys).is_err(), "{bad:?}");
    }
}

/// Every bundled catalog is well-formed and consistent with its sources.
#[test]
fn bundled_catalogs_are_consistent() {
    for l in &LANGUAGES {
        assert!(l.code == l.code.to_ascii_lowercase() && !l.name.is_empty(), "{}", l.code);
        let (entries, errors) = parse_entries(&l.text());
        assert!(errors.is_empty(), "{}: {errors:?}", l.code);
        let mut seen = std::collections::HashSet::new();
        for (ctx, src, tr) in &entries {
            assert!(seen.insert((ctx.clone(), src.clone())), "{}: duplicate {ctx:?} {src:?}", l.code);
            if ctx == "@plural" {
                let one_other: Vec<&str> = src.split('|').collect();
                assert_eq!(one_other.len(), 2, "{}: plural source must be `one|other`: {src:?}", l.code);
                let forms = (0..=1000).map(l.plural).max().unwrap_or(0) + 1;
                assert_eq!(tr.split('|').count(), forms, "{}: {forms} plural forms expected in {src:?}", l.code);
                for form in tr.split('|') {
                    let mut want = placeholders(one_other[1]);
                    let mut got = placeholders(form);
                    want.sort_unstable();
                    got.sort_unstable();
                    assert_eq!(want, got, "{}: placeholders differ in {src:?}", l.code);
                }
                continue;
            }
            let mut want = placeholders(src);
            let mut got = placeholders(tr);
            want.sort_unstable();
            got.sort_unstable();
            assert_eq!(want, got, "{}: placeholders differ in {src:?}", l.code);
            if ctx.is_empty() {
                assert_eq!(src.ends_with('…'), tr.ends_with('…'), "{}: ellipsis mismatch: {src:?}", l.code);
            }
            if ctx == "@id" {
                assert!(crate::menus::is_live(src) || crate::menu_catalog::CATALOG.iter().any(|m| m.3 == src), "{}: unknown command id {src:?}", l.code);
            }
        }
    }
}

#[test]
fn hindi_tags_resolve_fork_contract() {
    for tag in ["hi", "hi-IN", "hi_IN", "hi_IN.UTF-8"] {
        assert_eq!(lang_from_tag(tag), Some(HI()), "{tag}");
    }
    assert_eq!(HI().name(), "हिन्दी");
    assert_eq!(Lang::from_pref("hi"), HI());
    assert_eq!(Lang::from_pref("HI"), HI());
    assert_eq!(tr(HI(), "Layer"), "लेयर");
    assert_eq!(tr(HI(), "File"), "फ़ाइल");
    assert_eq!(trn(HI(), 1, "{n} layer", "{n} layers"), "1 लेयर");
    assert_eq!(trn(HI(), 3, "{n} layer", "{n} layers"), "3 लेयर्स");
}

#[test]
fn vietnamese_tags_resolve_fork_contract() {
    for tag in ["vi", "vi-VN", "vi_VN", "vi_VN.UTF-8"] {
        assert_eq!(lang_from_tag(tag), Some(VI()), "{tag}");
    }
    assert_eq!(VI().name(), "Tiếng Việt");
    assert!(VI().complete_menus(), "Vietnamese translates every source key");
    assert_eq!(Lang::from_pref("vi"), VI());
    assert_eq!(Lang::from_pref("VI"), VI());
}

#[test]
fn brush_size_and_hardness_shortcuts_are_translated_in_every_language() {
    let keys = [
        "Decrease Brush Size",
        "Increase Brush Size",
        "Decrease Brush Hardness",
        "Increase Brush Hardness",
        "Previous Brush",
        "Next Brush",
        "First Brush",
        "Last Brush",
    ];
    for lang in Lang::all().filter(|lang| *lang != Lang::EN) {
        for key in keys {
            assert!(has(lang, key), "{} has no translation for {key:?}", lang.code());
        }
    }
    let zh = Lang::from_code("zh-hans").expect("zh-hans registered");
    assert_eq!(tr(zh, "Decrease Brush Size"), "减小画笔大小");
    assert_eq!(tr(zh, "Increase Brush Size"), "增大画笔大小");
}

#[test]
fn simplified_chinese_covers_dynamic_shortcuts_and_layer_counts() {
    let zh = Lang::from_code("zh-hans").expect("zh-hans registered");
    assert!(zh.complete_menus(), "Simplified Chinese must participate in the coverage gates");
    assert_eq!(Lang::from_pref("ZH-Hans"), zh);
    assert_eq!(tr(zh, "Pixel Layer"), "像素图层");
    assert_eq!(tr(zh, "System Info"), "系统信息");
    for key in ["⌥", "Alt"] {
        assert_eq!(fmt(tr(zh, "Add a mask  (from the selection; {key} inverts)"), &[("key", key)]), format!("添加蒙版  （基于选区；{key} 反相）"));
    }
    for n in [0, 1, 3] {
        assert_eq!(trn(zh, n, "{n} layer", "{n} layers"), format!("{n} 个图层"));
    }
    assert_eq!(tr(zh, "no such label"), "no such label");
}

#[test]
fn korean_uses_photoshop_terminology() {
    let ko = Lang::from_code("ko").expect("ko registered");
    for (source, expected) in [
        ("Shape", "모양"),
        ("Stroke", "획"),
        ("Smudge Tool", "손가락 도구"),
        ("Eyedropper Tool", "스포이드 도구"),
        ("Rectangular Marquee Tool", "사각형 선택 윤곽 도구"),
        ("Elliptical Marquee Tool", "원형 선택 윤곽 도구"),
        ("Zoom Tool", "돋보기 도구"),
        ("Horizontal Type Tool", "수평 문자 도구"),
        ("Puppet Warp", "퍼펫 뒤틀기"),
        ("Liquify…", "픽셀 유동화…"),
        ("Gaussian Blur…", "가우시안 흐림 효과…"),
        ("Gaussian Blur", "가우시안 흐림 효과"),
        ("Adaptive Wide Angle…", "응용 광각…"),
        ("Render", "렌더"),
        ("Sharpen", "선명 효과"),
        ("Vibrance", "활기"),
        ("Layer Comps", "레이어 구성 요소"),
        ("Vivid Light", "선명한 라이트"),
        ("Hard Mix", "하드 혼합"),
    ] {
        assert_eq!(tr(ko, source), expected, "{source}");
    }
    assert_eq!(tr_id(ko, "filter.sharpen.sharpen", "Sharpen"), "선명하게");
}

#[test]
fn spanish_resolves_and_pluralises_fork_contract() {
    let es = Lang::from_code("es").expect("es registered");
    for tag in ["es", "es_ES.UTF-8", "es-MX", "es-419"] {
        assert_eq!(lang_from_tag(tag), Some(es), "{tag}");
    }
    assert_eq!(tr(es, "Layer"), "Capa");
    assert_eq!(trn(es, 1, "{n} item", "{n} items"), "1 elemento");
    assert_eq!(trn(es, 3, "{n} item", "{n} items"), "3 elementos");
}

#[test]
fn hungarian_resolves_and_keeps_the_singular_after_counts_fork_contract() {
    let hu = Lang::from_code("hu").expect("hu registered");
    for tag in ["hu", "hu_HU.UTF-8", "hu-HU"] {
        assert_eq!(lang_from_tag(tag), Some(hu), "{tag}");
    }
    assert!(hu.complete_menus());
    assert_eq!(hu.name(), "Magyar");
    assert_eq!(tr(hu, "Layer"), "Réteg");
    for (n, expected) in [(0, "0 réteg"), (1, "1 réteg"), (3, "3 réteg"), (21, "21 réteg")] {
        assert_eq!(trn(hu, n, "{n} layer", "{n} layers"), expected);
    }
    assert_eq!(tr_ctx(hu, "layerLabel", "Red"), "Piros");
    assert_eq!(tr(hu, "Red"), "Vörös");
}

#[test]
fn brazilian_portuguese_resolves_and_pluralises_fork_contract() {
    let pt = Lang::from_code("pt-br").expect("pt-br registered");
    for tag in ["pt", "pt-BR", "pt_BR.UTF-8", "pt-PT", "pt_PT.UTF-8"] {
        assert_eq!(lang_from_tag(tag), Some(pt), "{tag}");
    }
    assert_eq!(candidates("pt_PT"), ["pt-pt", "pt-br", "pt"]);
    assert_eq!(tr(pt, "Layer"), "Camada");
    assert_eq!(trn(pt, 0, "{n} item", "{n} items"), "0 item");
    assert_eq!(trn(pt, 1, "{n} item", "{n} items"), "1 item");
    assert_eq!(trn(pt, 2, "{n} item", "{n} items"), "2 itens");
}

#[test]
fn lookups_fall_back_to_english_fork_contract() {
    assert_eq!(tr(JA(), "no such label"), "no such label");
    assert_eq!(tr(Lang::EN, "Layer"), "Layer");
    assert_eq!(tr(JA(), "Layer"), "レイヤー");
    assert_eq!(tr(ZH(), "Layer"), "圖層");
    assert_eq!(tr(ZH(), "no such label"), "no such label");
    assert_eq!(tr_id(ZH(), "no.such.id", "Layer"), "圖層");
    assert_eq!(tr_id(JA(), "no.such.id", "Layer"), "レイヤー");
    assert_eq!(tr_ctx(JA(), "no such context", "Layer"), "レイヤー");
}

#[test]
fn russian_plural_rules() {
    let ru = || Lang::from_code("ru").expect("ru registered");
    assert_eq!(trn(ru(), 1, "{n} item", "{n} items"), "1 элемент");
    assert_eq!(trn(ru(), 2, "{n} item", "{n} items"), "2 элемента");
    assert_eq!(trn(ru(), 5, "{n} item", "{n} items"), "5 элементов");
    assert_eq!(trn(ru(), 11, "{n} item", "{n} items"), "11 элементов");
    assert_eq!(trn(ru(), 21, "{n} item", "{n} items"), "21 элемент");
    assert_eq!(trn(ru(), 22, "{n} item", "{n} items"), "22 элемента");
    assert_eq!(trn(ru(), 101, "{n} item", "{n} items"), "101 элемент");
    assert_eq!(trn(ru(), 111, "{n} item", "{n} items"), "111 элементов");
}

#[test]
fn russian_status_messages_agree_with_counts() {
    let ru = Lang::from_code("ru").expect("ru registered");
    let cases = [
        (
            "Exported {n} swatch to {file}",
            "Exported {n} swatches to {file}",
            ["Экспортирован {n} образец в {file}", "Экспортированы {n} образца в {file}", "Экспортировано {n} образцов в {file}"],
        ),
        (
            "Imported {n} brush from {group}",
            "Imported {n} brushes from {group}",
            ["Импортирована {n} кисть из {group}", "Импортированы {n} кисти из {group}", "Импортировано {n} кистей из {group}"],
        ),
        (
            "Imported {n} swatch from {file}",
            "Imported {n} swatches from {file}",
            ["Импортирован {n} образец из {file}", "Импортированы {n} образца из {file}", "Импортировано {n} образцов из {file}"],
        ),
        ("Played {n} step", "Played {n} steps", ["Воспроизведён {n} шаг", "Воспроизведены {n} шага", "Воспроизведено {n} шагов"]),
        (
            "Updated {n} linked smart object changed on disk",
            "Updated {n} linked smart objects changed on disk",
            [
                "Обновлён {n} связанный смарт-объект, изменённый на диске",
                "Обновлены {n} связанных смарт-объекта, изменённых на диске",
                "Обновлено {n} связанных смарт-объектов, изменённых на диске",
            ],
        ),
        (
            "{n} layer recorded by this comp was deleted. Update the comp, or clear the warning.",
            "{n} layers recorded by this comp were deleted. Update the comp, or clear the warning.",
            [
                "Удалён {n} слой, записанный в этом варианте. Обновите вариант или сбросьте предупреждение.",
                "Удалены {n} слоя, записанных в этом варианте. Обновите вариант или сбросьте предупреждение.",
                "Удалено {n} слоёв, записанных в этом варианте. Обновите вариант или сбросьте предупреждение.",
            ],
        ),
        ("{n} sample point removed", "{n} sample points removed", ["Удалена {n} точка замера", "Удалены {n} точки замера", "Удалено {n} точек замера"]),
        (
            "Loaded {n} action from {file}",
            "Loaded {n} actions from {file}",
            ["Загружена {n} операция из {file}", "Загружены {n} операции из {file}", "Загружено {n} операций из {file}"],
        ),
        (
            "Saved {n} action to {file}",
            "Saved {n} actions to {file}",
            ["Сохранена {n} операция в {file}", "Сохранены {n} операции в {file}", "Сохранено {n} операций в {file}"],
        ),
        (
            "{n} step with no Photoshop equivalent left out",
            "{n} steps with no Photoshop equivalent left out",
            [
                "Пропущен {n} шаг без эквивалента в Photoshop",
                "Пропущены {n} шага без эквивалента в Photoshop",
                "Пропущено {n} шагов без эквивалента в Photoshop",
            ],
        ),
    ];
    // Include zero, teens, and compound counts as well as each basic form.
    for (n, form) in [(0, 2), (1, 0), (2, 1), (3, 1), (4, 1), (5, 2), (11, 2), (12, 2), (14, 2), (21, 0), (22, 1), (25, 2), (101, 0), (112, 2)] {
        for (one, other, forms) in cases {
            let expected = forms[form].replace("{n}", &n.to_string());
            assert_eq!(trn(ru, n, one, other), expected, "{one}, n={n}");
        }
    }
    assert_eq!(
        fmt(&trn(ru, 2, "Imported {n} brush from {group}", "Imported {n} brushes from {group}"), &[("group", "Кисти")]),
        "Импортированы 2 кисти из Кисти"
    );
    assert_eq!(
        fmt(&trn(ru, 22, "Loaded {n} action from {file}", "Loaded {n} actions from {file}"), &[("file", "actions.atn")]),
        "Загружены 22 операции из actions.atn"
    );
}

#[test]
fn russian_font_weights_and_reviewed_labels() {
    let ru = Lang::from_code("ru").expect("ru registered");
    assert_eq!(tr_ctx(ru, "fontWeight", "ExtraLight"), "Сверхтонкий");
    assert_eq!(tr_ctx(ru, "fontWeight", "Light"), "Лёгкий");
    assert_eq!(tr_ctx(ru, "fontWeight", "ExtraBold"), "Сверхжирный");
    assert_eq!(tr_ctx(ru, "fontWeight", "Black"), "Сверхжирный (Black)");
    assert_eq!(tr(ru, "Delete and Fill Selection"), "Удалить и залить выделение");
    assert_eq!(tr(ru, "Discard the embedded profile (don't color manage)"), "Не использовать встроенный профиль (без управления цветом)");
    assert_eq!(tr(ru, "No object found there — drag a rectangle around it"), "Объект не найден — обведите его прямоугольником");
}

#[test]
fn ukrainian_resolves_locales_and_preferences_fork_contract() {
    let uk = Lang::from_code("uk").expect("uk registered");
    assert_eq!(uk.name(), "Українська");
    assert!(uk.complete_menus());
    for tag in ["uk", "UK", "uk-UA", "uk_UA", "uk_UA.UTF-8", "uk-UA@euro", "uk-Cyrl-UA"] {
        assert_eq!(lang_from_tag(tag), Some(uk), "{tag}");
        assert_eq!(Lang::from_pref(tag), uk, "{tag}");
    }
    assert_eq!(tr(uk, "File"), "Файл");
    assert_eq!(tr(uk, "Layer"), "Шар");
    assert_eq!(tr(uk, "New document…"), "Новий документ…");
    assert_eq!(tr(uk, "No properties"), "Немає властивостей");
    assert_eq!(tr_id(uk, "select.all", "All"), "Виділити все");
    assert_eq!(tr_ctx(uk, "cameraRaw", "Light"), "Світло");
    assert_eq!(tr_ctx(uk, "fontWeight", "Light"), "Легкий");
    assert_eq!(fmt(tr(uk, "Camera Raw Filter ({layer})"), &[("layer", "Background")]), "Фільтр Camera Raw (Background)");
    for key in ["Alt", "⌥"] {
        assert_eq!(fmt(tr(uk, "Add a mask  (from the selection; {key} inverts)"), &[("key", key)]), format!("Додати маску  (із виділення; {key} інвертує)"));
    }
    assert_eq!(tr(uk, "unknown translation"), "unknown translation");
}

#[test]
fn ukrainian_plural_messages_handle_teens_and_compound_counts() {
    let uk = Lang::from_code("uk").expect("uk registered");
    for (n, item, layer) in [
        (0, "елементів", "шарів"),
        (1, "елемент", "шар"),
        (2, "елементи", "шари"),
        (4, "елементи", "шари"),
        (5, "елементів", "шарів"),
        (11, "елементів", "шарів"),
        (12, "елементів", "шарів"),
        (14, "елементів", "шарів"),
        (19, "елементів", "шарів"),
        (21, "елемент", "шар"),
        (22, "елементи", "шари"),
        (24, "елементи", "шари"),
        (25, "елементів", "шарів"),
        (100, "елементів", "шарів"),
        (101, "елемент", "шар"),
        (111, "елементів", "шарів"),
        (112, "елементів", "шарів"),
        (114, "елементів", "шарів"),
        (121, "елемент", "шар"),
        (122, "елементи", "шари"),
        (u64::MAX, "елементів", "шарів"),
    ] {
        assert_eq!(trn(uk, n, "{n} item", "{n} items"), format!("{n} {item}"));
        assert_eq!(trn(uk, n, "{n} layer", "{n} layers"), format!("{n} {layer}"));
        assert_eq!(trn(uk, n, "Group · {n} layer", "Group · {n} layers"), format!("Група · {n} {layer}"));
    }
}

#[test]
fn serbian_resolves_locales_and_preferences_fork_contract() {
    let sr = Lang::from_code("sr").expect("sr registered");
    assert_eq!(sr.name(), "Srpski");
    assert!(sr.complete_menus());
    for tag in ["sr", "SR", "sr-RS", "sr_RS", "sr_RS.UTF-8", "sr_RS@latin", "sr-Latn", "sr-Latn-RS", "sr-Cyrl-RS", "sr-ME"] {
        assert_eq!(lang_from_tag(tag), Some(sr), "{tag}");
        assert_eq!(Lang::from_pref(tag), sr, "{tag}");
    }
    assert_eq!(tr(sr, "File"), "Datoteka");
    assert_eq!(tr(sr, "Layer"), "Sloj");
    assert_eq!(tr(sr, "New document…"), "Novi dokument…");
    assert_eq!(tr_id(sr, "select.all", "All"), "Izaberi sve", "an id override wins over the plain label");
    assert_eq!(tr(sr, "All"), "Sve");
    assert_eq!(tr_ctx(sr, "cameraRaw", "Light"), "Svetlo");
    assert_eq!(tr_ctx(sr, "fontWeight", "Light"), "Lagano");
    assert_eq!(fmt(tr(sr, "Camera Raw Filter ({layer})"), &[("layer", "Background")]), "Camera Raw filter (Background)");
    for key in ["Alt", "⌥"] {
        assert_eq!(fmt(tr(sr, "Add a mask  (from the selection; {key} inverts)"), &[("key", key)]), format!("Dodaj masku  (iz selekcije; {key} invertuje)"));
    }
}

#[test]
fn serbian_plural_messages_handle_teens_and_compound_counts() {
    let sr = Lang::from_code("sr").expect("sr registered");
    for (n, item, layer) in [
        (0, "stavki", "slojeva"),
        (1, "stavka", "sloj"),
        (2, "stavke", "sloja"),
        (4, "stavke", "sloja"),
        (5, "stavki", "slojeva"),
        (11, "stavki", "slojeva"),
        (12, "stavki", "slojeva"),
        (14, "stavki", "slojeva"),
        (21, "stavka", "sloj"),
        (22, "stavke", "sloja"),
        (25, "stavki", "slojeva"),
        (101, "stavka", "sloj"),
        (111, "stavki", "slojeva"),
        (112, "stavki", "slojeva"),
        (122, "stavke", "sloja"),
    ] {
        assert_eq!(trn(sr, n, "{n} item", "{n} items"), format!("{n} {item}"));
        assert_eq!(trn(sr, n, "{n} layer", "{n} layers"), format!("{n} {layer}"));
        assert_eq!(trn(sr, n, "Group · {n} layer", "Group · {n} layers"), format!("Grupa · {n} {layer}"));
    }
    for (n, expected) in [
        (1, "Uklonjena je 1 tačka uzorkovanja"),
        (3, "Uklonjene su 3 tačke uzorkovanja"),
        (5, "Uklonjeno je 5 tačaka uzorkovanja"),
        (13, "Uklonjeno je 13 tačaka uzorkovanja"),
        (21, "Uklonjena je 21 tačka uzorkovanja"),
    ] {
        assert_eq!(trn(sr, n, "{n} sample point removed", "{n} sample points removed"), expected);
    }
}

#[test]
fn plurals_and_placeholders_fork_contract() {
    assert_eq!(trn(Lang::EN, 1, "{n} item", "{n} items"), "1 item");
    assert_eq!(trn(Lang::EN, 0, "{n} item", "{n} items"), "0 items");
    assert_eq!(trn(Lang::EN, 7, "{n} item", "{n} items"), "7 items");
    assert_eq!(trn(JA(), 1, "{n} item", "{n} items"), "1 件");
    assert_eq!(trn(JA(), 7, "{n} item", "{n} items"), "7 件");
    assert_eq!(trn(ZH(), 1, "{n} item", "{n} items"), "1 個項目");
    assert_eq!(trn(ZH(), 7, "{n} item", "{n} items"), "7 個項目");
    assert_eq!(trn(CS(), 1, "{n} item", "{n} items"), "1 položka");
    assert_eq!(trn(CS(), 3, "{n} item", "{n} items"), "3 položky");
    assert_eq!(trn(CS(), 5, "{n} item", "{n} items"), "5 položek");
    assert_eq!(trn(CS(), 0, "{n} item", "{n} items"), "0 položek");
    assert_eq!(fmt("{b} before {a}", &[("a", "x"), ("b", "y"), ("c", "z")]), "y before x");
    assert_eq!(fmt("{missing}", &[]), "{missing}");
    assert_eq!(placeholders("a {x} b {y} {"), ["x", "y"]);
}

#[test]
fn french_resolves_and_pluralises_fork_contract() {
    let fr = Lang::from_code("fr").expect("fr registered");
    for tag in ["fr", "fr_FR.UTF-8", "fr-CA", "fr_BE", "fr-CH"] {
        assert_eq!(lang_from_tag(tag), Some(fr), "{tag}");
    }
    assert_eq!(tr(fr, "Layer"), "Calque");
    assert_eq!(tr_id(fr, "select.all", "All"), "Tout sélectionner", "an id override wins over the plain label");
    assert_eq!(tr(fr, "All"), "Tout");
    let forms: Vec<usize> = [0, 1, 2, 5, 100, u64::MAX].into_iter().map(plural_fr).collect();
    assert_eq!(forms, [0, 0, 1, 1, 1, 1]);
    assert_eq!(trn(fr, 0, "{n} item", "{n} items"), "0 élément");
    assert_eq!(trn(fr, 1, "{n} item", "{n} items"), "1 élément");
    assert_eq!(trn(fr, 3, "{n} item", "{n} items"), "3 éléments");
}

#[test]
fn simplified_chinese_counted_messages_are_translated() {
    let lang = Lang::from_code("zh-hans").expect("zh-hans registered");
    assert_eq!(trn(lang, 3, "Played {n} step", "Played {n} steps"), "已播放 3 个步骤");
    let brushes = trn(lang, 2, "Imported {n} brush from {group}", "Imported {n} brushes from {group}");
    assert_eq!(fmt(&brushes, &[("group", "水彩")]), "已从 水彩 导入 2 个画笔");
    assert_eq!(trn(lang, 5, "{n} slice", "{n} slices"), "5 个切片");
}

#[test]
fn korean_counted_messages_are_translated() {
    let lang = Lang::from_code("ko").expect("ko registered");
    assert_eq!(trn(lang, 3, "Played {n} step", "Played {n} steps"), "단계 3개를 재생했습니다");
    let brushes = trn(lang, 2, "Imported {n} brush from {group}", "Imported {n} brushes from {group}");
    assert_eq!(fmt(&brushes, &[("group", "수채화")]), "수채화에서 브러시 2개를 가져왔습니다");
    assert_eq!(trn(lang, 5, "{n} slice", "{n} slices"), "분할 영역 5개");
}

#[test]
fn indonesian_counted_messages_are_translated() {
    let lang = Lang::from_code("id").expect("id registered");
    assert_eq!(trn(lang, 3, "Played {n} step", "Played {n} steps"), "3 langkah diputar");
    let brushes = trn(lang, 2, "Imported {n} brush from {group}", "Imported {n} brushes from {group}");
    assert_eq!(fmt(&brushes, &[("group", "Cat Air")]), "2 kuas diimpor dari Cat Air");
    assert_eq!(trn(lang, 5, "{n} slice", "{n} slices"), "5 slice");
}

#[test]
fn polish_counted_messages_use_all_three_forms() {
    let pl = Lang::from_code("pl").expect("pl registered");
    let steps = |n| trn(pl, n, "Played {n} step", "Played {n} steps");
    assert_eq!(
        [steps(1), steps(3), steps(5), steps(12), steps(22)],
        ["Odtworzono 1 krok", "Odtworzono 3 kroki", "Odtworzono 5 kroków", "Odtworzono 12 kroków", "Odtworzono 22 kroki"]
    );
    let brushes = trn(pl, 2, "Imported {n} brush from {group}", "Imported {n} brushes from {group}");
    assert_eq!(fmt(&brushes, &[("group", "Akwarele")]), "Zaimportowano 2 pędzle z Akwarele");
    assert_eq!(trn(pl, 5, "{n} slice", "{n} slices"), "5 plasterków");
}

#[test]
fn czech_plural_rule_fork_contract() {
    let forms: Vec<usize> = [0, 1, 2, 3, 4, 5, 11, 12, 21, 22, 100, u64::MAX].into_iter().map(plural_cs).collect();
    assert_eq!(forms, [2, 0, 1, 1, 1, 2, 2, 2, 2, 2, 2, 2]);
    assert_eq!(tr(CS(), "Layer"), "Vrstva");
    assert_eq!(tr_id(CS(), "select.all", "All"), "Vybrat vše", "an id override wins over the plain label");
    assert_eq!(tr(CS(), "All"), "Vše");
}

#[test]
fn complete_languages_translate_every_menu_string() {
    let mut strings = std::collections::BTreeSet::new();
    for &(path, label, _, _) in crate::menu_catalog::CATALOG {
        strings.extend(path.iter().copied());
        strings.insert(label);
    }
    for &(_, label, path, _) in crate::menus::UI_COMMANDS {
        strings.extend(path.iter().copied());
        strings.insert(label);
    }
    for c in photocraft_engine::command_specs().iter().filter(|c| !c.menu.is_empty()) {
        strings.extend(c.menu.iter().copied());
        strings.insert(c.label);
    }
    strings.remove("---");
    for l in LANGUAGES.iter().filter(|l| l.complete_menus) {
        let cat = l.catalog();
        let missing: Vec<_> = strings.iter().filter(|s| cat.plain(s).is_none()).collect();
        assert!(missing.is_empty(), "{}: untranslated menu strings: {missing:#?}", l.code);
    }
}

fn without_test_modules(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find("#[cfg(test)]\nmod ") {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let Some(open) = after.find('{') else { return out };
        let mut depth = 0usize;
        let mut end = after.len();
        for (i, c) in after[open..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        rest = &after[end..];
    }
    out.push_str(rest);
    out
}

#[test]
fn test_modules_are_cut_out_one_by_one() {
    let text = "a\n#[cfg(test)]\nmod t { fn x() { tl!(\"hidden\"); } }\nb tl!(\"seen\")\n#[cfg(test)]\nmod u {}\nc";
    let code = without_test_modules(text);
    assert!(code.contains("tl!(\"seen\")"), "code after the first test module is scanned: {code}");
    assert!(!code.contains("hidden"), "{code}");
    assert!(code.starts_with("a\n") && code.ends_with("\nc"), "{code}");
}

#[test]
fn czech_status_templates_preserve_details_and_plural_counts() {
    let cs = CS();
    let template = "Step {step} ({id}) failed: {error}";
    let args = [("step", "2"), ("id", "file.export"), ("error", "disk full: /tmp/image.psd")];
    assert_eq!(fmt(tr(cs, template), &args), "Krok 2 (file.export) selhal: disk full: /tmp/image.psd");
    assert_eq!(fmt(tr(Lang::EN, template), &args), "Step 2 (file.export) failed: disk full: /tmp/image.psd");
    // Languages without these dynamic templates keep the English source.
    assert_eq!(tr(JA(), template), template);
    for (n, expected) in [(0, "Odstraněno 0 bodů měření"), (1, "Odstraněn 1 bod měření"), (2, "Odstraněny 2 body měření"), (5, "Odstraněno 5 bodů měření")]
    {
        assert_eq!(trn(cs, n, "{n} sample point removed", "{n} sample points removed"), expected);
    }
    let one = "Imported {n} swatch from {file}";
    let other = "Imported {n} swatches from {file}";
    assert_eq!(fmt(&trn(cs, 3, one, other), &[("file", "English name.aco")]), "Importovány 3 vzorky ze souboru English name.aco");
    assert_eq!(tr(cs, "Preferences can't be saved"), "Předvolby nelze uložit");
}

#[test]
fn czech_dialog_and_hardware_labels_preserve_names() {
    let cs = CS();
    for label in [
        "New Crop Preset…",
        "Delete Crop Preset…",
        "Color Picker (Background Color)",
        "Color Picker (Foreground Color)",
        "Pro (Medium Gray)",
        "Grid view",
        "List view",
        "Scroll menu up",
        "Scroll menu down",
        "Commit transform",
        "Cancel transform",
    ] {
        assert_ne!(tr(cs, label), label, "{label}");
    }
    for kind in [
        crate::rasterize_prompt::Kind::Type,
        crate::rasterize_prompt::Kind::Shape,
        crate::rasterize_prompt::Kind::SmartObject,
        crate::rasterize_prompt::Kind::Fill,
    ] {
        assert_ne!(tr(cs, kind.message()), kind.message());
    }
    assert_eq!(fmt(tr(cs, "Driver: {driver}"), &[("driver", "Mesa 25.0")]), "Ovladač: Mesa 25.0");
    assert_eq!(
        fmt(tr(cs, "Monitor profile on {display}: {profile}"), &[("display", "DELL U2720Q"), ("profile", "sRGB IEC61966-2.1")]),
        "Profil monitoru DELL U2720Q: sRGB IEC61966-2.1"
    );
}

#[test]
fn czech_composed_labels_use_context_and_preserve_document_names() {
    let cs = CS();
    assert_eq!(tr(cs, "Zoom"), "Lupa");
    assert_eq!(tr_ctx(cs, "navigator", "Zoom"), "Zvětšení");
    assert_eq!(tr_ctx(Lang::EN, "navigator", "Zoom"), "Zoom");
    let template = "Data Set: {name}  ({index}/{total})";
    assert_eq!(fmt(tr(cs, template), &[("name", "Data Set 1"), ("index", "1"), ("total", "2")]), "Datová sada: Data Set 1  (1/2)");
    for (n, expected) in
        [(1, "Importován 1 přechod ze souboru Sunset"), (3, "Importovány 3 přechody ze souboru Sunset"), (5, "Importováno 5 přechodů ze souboru Sunset")]
    {
        assert_eq!(fmt(&trn(cs, n, "Imported {n} gradient from {file}", "Imported {n} gradients from {file}"), &[("file", "Sunset")]), expected);
    }
}

#[test]
fn russian_dynamic_status_hardware_and_dialog_labels_are_translated() {
    let ru = Lang::from_code("ru").expect("ru registered");
    for kind in [
        crate::rasterize_prompt::Kind::Type,
        crate::rasterize_prompt::Kind::Shape,
        crate::rasterize_prompt::Kind::SmartObject,
        crate::rasterize_prompt::Kind::Fill,
    ] {
        assert_ne!(tr(ru, kind.message()), kind.message(), "{:?}", kind);
    }
    assert_eq!(fmt(tr(ru, "Driver: {driver}"), &[("driver", "Mesa 25.0")]), "Драйвер: Mesa 25.0");
    assert_eq!(
        fmt(tr(ru, "Monitor profile on {display}: {profile}"), &[("display", "DELL U2720Q"), ("profile", "sRGB IEC61966-2.1")]),
        "Профиль монитора для DELL U2720Q: sRGB IEC61966-2.1"
    );
    assert_eq!(fmt(tr(ru, "Platform: {os} {arch}"), &[("os", "Linux"), ("arch", "x86_64")]), "Платформа: Linux x86_64");
    assert_eq!(tr(ru, "Graphics adapter"), "Графический адаптер");
    assert_eq!(tr(ru, "Image compositor: {processor}"), "Компоновщик изображения: {processor}");
    // Filter / adjustment parameter names.
    for label in [
        "Amount",
        "Radius",
        "Threshold",
        "Angle",
        "High Pass",
        "Smart Blur",
        "Reduce Noise",
        "Lens Correction",
        "Colored Pencil",
        "Angled Strokes",
        "Stained Glass",
        "Sumi-e",
        "Film Grain",
        "Unsharp Mask",
        "Lens Flare",
    ] {
        assert!(has(ru, label), "Russian filter label missing: {label}");
    }
    // Dynamic composed status templates.
    assert_eq!(
        fmt(tr(ru, "Step {step} ({id}) failed: {error}"), &[("step", "2"), ("id", "file.export"), ("error", "ok")]),
        "Шаг 2 (file.export) не выполнен: ok"
    );
    assert_eq!(fmt(tr(ru, "Couldn't open {name}: {error}"), &[("name", "scan.tif"), ("error", "denied")]), "Не удалось открыть scan.tif: denied");
    assert_eq!(fmt(tr(ru, "Exported {path} ({size})"), &[("path", "/tmp/a.png"), ("size", "12 KB")]), "Экспортировано: /tmp/a.png (12 KB)");
    assert_eq!(fmt(tr(ru, "Data Set: {name}  ({index}/{total})"), &[("name", "Set 1"), ("index", "1"), ("total", "2")]), "Набор данных: Set 1  (1/2)");
    // Document and layer names stay verbatim.
    let text = fmt(tr(ru, "Saved {path}; image assets: {n} in {dir}"), &[("path", "/tmp/a.pcraft"), ("n", "3"), ("dir", "/tmp/assets")]);
    assert!(text.contains("/tmp/a.pcraft") && text.contains("/tmp/assets"), "{text}");
}

#[test]
fn every_tl_literal_is_translated() {
    const KEEP_AS_IS: &[&str] = &[];
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut literals = std::collections::BTreeSet::new();
    let mut stack = vec![dir];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "rs") && !path.ends_with("lib.rs") {
                let text = std::fs::read_to_string(&path).unwrap_or_default().replace("\r\n", "\n");
                // Test modules aside, scan every `tl!("…")`. Each test *module* is cut out
                // on its own: cutting the file at the first one hid everything after a small
                // test module early in a file (panels.rs has one at ~line 540). A
                // `#[cfg(test)]` on a single item doesn't count.
                let code = without_test_modules(&text);
                let mut rest = code.as_str();
                while let Some(at) = rest.find("tl!(\"") {
                    rest = &rest[at + 5..];
                    let mut end = 0;
                    let bytes = rest.as_bytes();
                    while end < bytes.len() && !(bytes[end] == b'"' && (end == 0 || bytes[end - 1] != b'\\')) {
                        end += 1;
                    }
                    let lit = rest.get(..end).unwrap_or("").replace("\\\"", "\"");
                    if rest.get(end + 1..end + 2) == Some(")") {
                        literals.insert(lit);
                    }
                }
            }
        }
    }
    assert!(literals.len() > 300, "scan found only {} literals", literals.len());
    for l in LANGUAGES.iter().filter(|l| l.complete_menus) {
        let cat = l.catalog();
        let missing: Vec<_> = literals.iter().filter(|s| !KEEP_AS_IS.contains(&s.as_str()) && cat.plain(s).is_none()).collect();
        assert!(missing.is_empty(), "{}: untranslated tl! strings: {missing:#?}", l.code);
    }
}

#[test]
fn brush_section_names_are_translated() {
    for lang in Lang::all().filter(|l| l.complete_menus()) {
        for (name, _) in crate::brush_panel::SECTIONS {
            assert!(lang.0.catalog().plain(name).is_some(), "{} missing brush section: {name}", lang.code());
        }
    }
}

#[test]
fn reveal_labels_are_translated() {
    for lang in Lang::all().filter(|l| l.complete_menus()) {
        let cat = lang.0.catalog();
        for label in ["Reveal in Finder", "Show in Explorer", "Show in Folder"] {
            let tr = cat.plain(label);
            assert!(tr.is_some(), "{} missing {label}", lang.code());
            if label != "Reveal in Finder" {
                assert!(!tr.unwrap_or_default().contains("Finder"), "{}: {label} names the Finder", lang.code());
            }
        }
    }
}

#[test]
fn layer_color_names_are_translated() {
    for lang in Lang::all().filter(|l| l.complete_menus()) {
        for color in photocraft_doc::LabelColor::ALL {
            let cat = lang.0.catalog();
            assert!(cat.contextual("layerLabel", color.label()).or_else(|| cat.plain(color.label())).is_some(), "{} missing {}", lang.code(), color.label());
        }
    }
    let de = lang_from_tag("de").unwrap();
    assert_eq!(tr_ctx(de, "layerLabel", "No Color"), "Keine Farbe");
    assert_eq!(tr_ctx(de, "layerLabel", "Seafoam"), "Meeresschaum");
    assert_eq!(tr(Lang::EN, "Seafoam"), "Seafoam");
}

#[test]
fn russian_dynamic_selection_theme_and_measurement_labels_are_translated() {
    let ru = Lang::from_code("ru").expect("ru registered");
    let (inverse_label, _) =
        crate::canvas_tool_menu::SELECTION_MENU.iter().flatten().find(|(_, id)| *id == "select.inverse").expect("selection menu has Select Inverse");
    assert_eq!(tr(ru, inverse_label), "Инвертировать выделение");
    for kind in [crate::theme::ThemeKind::ProLight, crate::theme::ThemeKind::ProLightGray] {
        assert!(has(ru, kind.label()), "Russian theme label missing {}", kind.label());
    }
    for (_, label) in photocraft_engine::analysis_cmds::COLUMNS {
        assert!(has(ru, label), "Russian measurement header missing {label}");
    }
    assert_eq!(tr(ru, "Pro (Light)"), "Pro (светлая)");
    assert_eq!(tr(ru, "Pro (Light Gray)"), "Pro (светло-серая)");
    assert_eq!(tr(ru, "Date and Time"), "Дата и время");
    assert_eq!(tr(ru, "Integrated Density"), "Интегральная плотность");
}

#[test]
fn font_weight_names_are_translated() {
    const TERMS: &[&str] = &["Thin", "ExtraLight", "Light", "Regular", "Medium", "SemiBold", "Bold", "ExtraBold", "Black", "Italic"];
    for lang in Lang::all().filter(|l| l.complete_menus()) {
        for t in TERMS {
            assert!(lang.0.catalog().contextual("fontWeight", t).is_some(), "{} missing font weight: {t}", lang.code());
        }
        assert_ne!(tr_ctx(lang, "cameraRaw", "Light"), tr_ctx(lang, "fontWeight", "Light"), "{}: Camera Raw Light vs the font weight", lang.code());
    }
}

#[test]
fn blend_mode_names_are_translated() {
    for l in LANGUAGES.iter().filter(|l| l.complete_menus) {
        for m in std::iter::once(photocraft_color::BlendMode::PassThrough).chain(photocraft_color::BlendMode::layer_modes()) {
            assert!(l.catalog().plain(m.label()).is_some(), "{}: blend mode {:?}", l.code, m.label());
        }
    }
}

#[test]
fn italian_names_the_dodge_tools_and_the_screen_blend_mode_as_photoshop_does() {
    let it = LANGUAGES.iter().find(|l| l.code == "it").expect("the Italian catalog is registered");
    for (source, translation) in [("Dodge", "Scherma"), ("Dodge Tool", "Strumento Scherma"), ("Dodge/Burn", "Scherma/Brucia"), ("Screen", "Scherma")] {
        assert_eq!(it.catalog().plain(source), Some(translation), "it: {source:?}");
    }
    // The tool family stays together, and the blend modes keep Adobe's wording.
    for (source, translation) in
        [("Burn", "Brucia"), ("Sponge", "Spugna"), ("Lighten", "Schiarisci"), ("Color Dodge", "Colore scherma"), ("Linear Dodge", "Scherma lineare")]
    {
        assert_eq!(it.catalog().plain(source), Some(translation), "it: {source:?}");
    }
}

#[test]
fn color_lookup_export_scope_labels_are_translated() {
    for lang in Lang::all().filter(|l| *l != Lang::EN) {
        for source in ["Scope", "Selected"] {
            assert!(lang.catalog().plain(source).is_some(), "{}: missing LUT export label {source:?}", lang.code());
            assert_ne!(tr(lang, source), source, "{}: untranslated LUT export label {source:?}", lang.code());
        }
    }
}

#[test]
fn mixer_brush_ui_strings_have_translations_in_every_registered_language() {
    const STRINGS: &[&str] = &[
        "Mixer Brush",
        "Mixer Brush Tool",
        "Wet",
        "Load",
        "Mix",
        "Flow",
        "Sample All Layers",
        "Current Brush Load",
        "Load Brush",
        "Clean Brush",
        "Load the brush after each stroke",
        "Clean the brush after each stroke",
    ];
    for lang in Lang::all() {
        for source in STRINGS {
            let translated = tr(lang, source);
            if lang == Lang::EN {
                assert_eq!(translated, *source, "English source string {source}");
            } else {
                assert_ne!(translated, *source, "{} is missing {source:?}", lang.code());
            }
        }
    }
}

#[test]
fn pattern_stamp_ui_strings_have_translations_in_every_registered_language() {
    const STRINGS: &[&str] = &["Pattern Stamp Tool", "Pattern Stamp", "Impressionist", "Aligned"];
    for lang in Lang::all() {
        for source in STRINGS {
            let translated = tr(lang, source);
            if lang == Lang::EN {
                assert_eq!(translated, *source, "English source string {source}");
            } else {
                assert_ne!(translated, *source, "{} is missing {source:?}", lang.code());
            }
        }
    }
}

#[test]
fn camera_raw_labels_are_translated_in_every_available_language() {
    let sources = [
        include_str!("../dialogs/camera_raw_ui.rs"),
        include_str!("../dialogs/camera_raw_scope_ui.rs"),
        include_str!("../dialogs/camera_raw_ui/controls.rs"),
        include_str!("../dialogs/camera_raw_ui/convert.rs"),
        include_str!("../dialogs/camera_raw_ui/menu.rs"),
        include_str!("../dialogs/camera_raw_ui/open.rs"),
        include_str!("../dialogs/camera_raw_ui/proxy.rs"),
        include_str!("../dialogs/camera_raw_ui/sidecar.rs"),
    ];
    let mut labels = std::collections::BTreeSet::new();
    for source in sources {
        let code = source.split("#[cfg(test)]").next().unwrap();
        for marker in ["tl!(\"", "row(ui, &mut dirty, \"", "row(ui, dirty, \"", "section(ui, \"", "wheel(ui, &mut dirty, \"", "=> \""] {
            for tail in code.split(marker).skip(1) {
                labels.insert(tail.split('"').next().unwrap());
            }
        }
    }
    let bands = sources[0].split("const BANDS:").nth(1).unwrap().split(" = ").nth(1).unwrap().split(';').next().unwrap();
    for band in bands.split('"').skip(1).step_by(2) {
        labels.insert(band);
    }
    assert!(labels.len() >= 66, "missing Camera Raw source labels: {labels:?}");
    for lang in Lang::all().filter(|l| *l != Lang::EN) {
        let catalog = lang.catalog();
        let missing: Vec<_> = labels.iter().filter(|s| catalog.contextual("cameraRaw", s).or_else(|| catalog.plain(s)).is_none()).collect();
        assert!(missing.is_empty(), "{}: Camera Raw labels: {missing:?}", lang.code());
        assert_ne!(tr_ctx(lang, "cameraRaw", "Highlights"), tr_ctx(lang, "cameraRaw", "Lights"), "{}: distinct curve regions", lang.code());
        assert_ne!(tr_ctx(lang, "cameraRaw", "Shadows"), tr_ctx(lang, "cameraRaw", "Darks"), "{}: distinct curve regions", lang.code());
    }
    let ru = Lang::from_code("ru").unwrap();
    assert_eq!(tr_ctx(ru, "cameraRaw", "Vibrance"), "Красочность");
    assert_eq!(tr_ctx(ru, "cameraRaw", "Aqua"), "Голубые");
    let text = fmt(tr(ru, "Camera Raw Filter ({layer})"), &[("layer", "{Background} 影像")]);
    assert_eq!(text, "Фильтр Camera Raw ({Background} 影像)", "user layer names are not translated");
}
