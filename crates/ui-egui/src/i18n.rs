//! UI localisation uses the upstream craft-i18n lookup and CLDR plural rules.
//! The fork always embeds compressed catalogs, including its Qwen/upscaler labels.
//! Catalog rows combine the exact `craft-i18n.rev` input with fork overrides by
//! context/source key. Completeness, malformed-input and round-trip tests remain gates.
//! Command ids, control responses and stored history labels remain English.

mod casing;
mod catalog;
use catalog::Catalog;
mod system;

#[cfg(test)]
pub(crate) use system::with_decimal_separator;
pub use system::{decimal_separator, system_lang};

use std::cell::Cell;
use std::collections::HashMap;
use std::sync::{LazyLock, OnceLock};

use craft_i18n::I18n;

// Keep the fork's translations available even without an optional build checkout.
// These catalogs merge craft-i18n.rev with the fork's context/source-key overrides.
pub(crate) static CRAFT_I18N: LazyLock<Vec<(&'static str, &'static str, &'static str)>> = LazyLock::new(|| {
    LANGUAGES
        .iter()
        .filter(|l| l.code != "en")
        .map(|l| {
            let text: &'static str = Box::leak(l.text().into_boxed_str());
            (l.code, "", text)
        })
        .collect()
});

/// The embedded catalogs, offering exactly the languages in [`LANGUAGES`]. Built on first use; a
/// locale is parsed on its first lookup.
static I18N: LazyLock<I18n> = LazyLock::new(|| {
    let offered: Vec<&str> = LANGUAGES.iter().map(|l| l.code).collect();
    let (i18n, problems) = I18n::new(&CRAFT_I18N, &offered);
    for problem in problems {
        log::warn!("interface catalogs: {problem}");
    }
    i18n
});

/// One supported UI language.
pub struct LangInfo {
    /// BCP 47 code, lowercase (`ja`, `zh-hans`, `pt-br`). Also the `interface.language` value.
    pub code: &'static str,
    /// The language's name in itself, shown in the Preferences dropdown.
    pub name: &'static str,
    /// Catalog file contents, raw-deflated by build.rs (empty for the built-in English); see
    /// [`LangInfo::text`].
    pub source: &'static [u8],
    /// Plural form index for a count (English: 0 = one, 1 = other; Japanese, Chinese and Hungarian: always 0;
    /// Czech: 0 = one, 1 = few (2–4), 2 = other; Russian, Ukrainian and Bosnian: 0 = one (1, 21 …), 1 = few (2–4, 22–24 …), 2 = other; French: 0 = one (0 and 1), 1 = other). A catalog's `@plural` entries list one form per
    /// index.
    pub plural: fn(u64) -> usize,
    /// Must the catalog cover every menu string? (checked by the tests)
    pub complete_menus: bool,
    catalog: OnceLock<Catalog>,
}

/// A script's reading direction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dir {
    Ltr,
    Rtl,
}

/// Languages written right to left, by primary subtag: Arabic, Sorani Kurdish, Dhivehi, Persian,
/// Hebrew, Pashto, Sindhi, Uyghur, Urdu, Yiddish.
const RTL_LANGUAGES: [&str; 10] = ["ar", "ckb", "dv", "fa", "he", "ps", "sd", "ug", "ur", "yi"];

/// The reading direction of a language code (`ar`, `fa-ir`, `he`; any case).
pub fn dir_for_code(code: &str) -> Dir {
    let primary = code.split(['-', '_']).next().unwrap_or("");
    if RTL_LANGUAGES.iter().any(|l| l.eq_ignore_ascii_case(primary)) { Dir::Rtl } else { Dir::Ltr }
}

const LANGUAGE_COUNT: usize = 35;
fn plural_one_other(n: u64) -> usize {
    usize::from(n != 1)
}

fn plural_none(_: u64) -> usize {
    0
}

fn plural_russian(n: u64) -> usize {
    match (n % 10, n % 100) {
        (1, 11..=19) => 2,
        (1, _) => 0,
        (2..=4, 11..=19) => 2,
        (2..=4, _) => 1,
        _ => 2,
    }
}

/// Czech: 1 → one, 2–4 → few, everything else (0, 5+) → other.
fn plural_cs(n: u64) -> usize {
    match n {
        1 => 0,
        2..=4 => 1,
        _ => 2,
    }
}

/// French: 0 and 1 take the singular, everything else the plural.
fn plural_fr(n: u64) -> usize {
    usize::from(n > 1)
}

/// Portuguese: 0 and 1 take the singular, everything else the plural.
fn plural_pt(n: u64) -> usize {
    usize::from(n > 1)
}

/// Romanian (CLDR): 1 → one; 0, or n % 100 in 1–19 → few (these take the bare plural noun, e.g.
/// "2 straturi", "0 straturi", "119 straturi"); everything else (20, 100, 121, ...) → other (these
/// take "de" plus the plural noun, e.g. "20 de straturi", "121 de straturi"), which matches
/// Romanian's grammatical rule for when counted nouns need "de" inserted before them.
fn plural_romanian(n: u64) -> usize {
    if n == 1 {
        0
    } else if n == 0 || (1..=19).contains(&(n % 100)) {
        1
    } else {
        2
    }
}

/// Polish: 1 → one; 2–4, except 12–14 → few; everything else → many.
fn plural_polish(n: u64) -> usize {
    let last = n % 10;
    let last_two = n % 100;
    if n == 1 {
        0
    } else if (2..=4).contains(&last) && !(12..=14).contains(&last_two) {
        1
    } else {
        2
    }
}

/// Arabic (CLDR): 0 → zero, 1 → one, 2 → two, n % 100 in 3–10 → few, in 11–99 → many, else other.
fn plural_arabic(n: u64) -> usize {
    match (n, n % 100) {
        (0, _) => 0,
        (1, _) => 1,
        (2, _) => 2,
        (_, 3..=10) => 3,
        (_, 11..=99) => 4,
        _ => 5,
    }
}

/// A catalog file as packed by build.rs (`pack_catalogs`; the binary would otherwise carry ~2.2 MB
/// of translations, which the web build's size gate can't afford). See [`unpack`].
macro_rules! catalog {
    ($code:literal) => {
        include_bytes!(concat!(env!("OUT_DIR"), "/i18n/", $code, ".tsv.deflate"))
    };
}

/// Inflated catalogs are bounded, so corrupt data can't exhaust memory (the largest is ~0.2 MB).
const MAX_CATALOG_BYTES: u64 = 16 << 20;

/// Inflate a catalog. Bad data gives an error (and the language falls back to English), never a panic.
fn inflate(source: &[u8]) -> Result<String, String> {
    use std::io::Read as _;
    let mut text = String::new();
    if source.is_empty() {
        return Ok(text);
    }
    flate2::read::DeflateDecoder::new(source).take(MAX_CATALOG_BYTES + 1).read_to_string(&mut text).map_err(|e| e.to_string())?;
    if text.len() as u64 > MAX_CATALOG_BYTES {
        return Err(format!("over {MAX_CATALOG_BYTES} bytes"));
    }
    Ok(text)
}

/// Every catalog entry's `context<TAB>source`, stored once for all catalogs (build.rs).
static KEYS: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/i18n/keys.deflate"));

/// Restore a catalog file from its packed form (build.rs `pack_catalogs`): entry lines refer to
/// [`KEYS`] instead of repeating the English source. Bad data gives an error (and the language
/// falls back to English), never a panic.
fn unpack(source: &[u8]) -> Result<String, String> {
    static KEY_TEXT: OnceLock<Result<String, String>> = OnceLock::new();
    if source.is_empty() {
        return Ok(String::new());
    }
    let keys: Vec<&str> = KEY_TEXT.get_or_init(|| inflate(KEYS)).as_ref().map_err(|e| format!("catalog keys: {e}"))?.split('\n').collect();
    expand(&inflate(source)?, &keys)
}

/// [`unpack`]'s second step: replace each packed entry line with its key and translation.
fn expand(packed: &str, keys: &[&str]) -> Result<String, String> {
    let mut text = String::with_capacity(packed.len().saturating_mul(2));
    let mut next = 0usize;
    for (n, line) in packed.split('\n').enumerate() {
        if n > 0 {
            text.push('\n');
        }
        let bad = || format!("packed line {}: bad key reference", n + 1);
        let (index, tr) = if let Some(tr) = line.strip_prefix('\x01') {
            (next, tr)
        } else if let Some(rest) = line.strip_prefix('\x02') {
            let (offset, tr) = rest.split_once('\t').ok_or_else(bad)?;
            let offset: i64 = offset.parse().map_err(|_| bad())?;
            let index = i64::try_from(next).ok().and_then(|n| n.checked_add(offset)).and_then(|i| usize::try_from(i).ok()).ok_or_else(bad)?;
            (index, tr)
        } else {
            text.push_str(line.strip_prefix('\x03').unwrap_or(line));
            continue;
        };
        text.push_str(keys.get(index).ok_or_else(bad)?);
        text.push('\t');
        text.push_str(tr);
        next = index.saturating_add(1);
        if text.len() as u64 > MAX_CATALOG_BYTES {
            return Err(format!("over {MAX_CATALOG_BYTES} bytes"));
        }
    }
    Ok(text)
}

/// The registry. English first: it is the fallback and the source language.
pub static LANGUAGES: [LangInfo; 35] = [
    LangInfo { code: "en", name: "English", source: b"", plural: plural_one_other, complete_menus: false, catalog: OnceLock::new() },
    LangInfo { code: "ja", name: "日本語", source: catalog!("ja"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "zh-hans", name: "简体中文", source: catalog!("zh-hans"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
    // Traditional Chinese in the vocabulary used in Taiwan; `zh-TW`, `zh-HK`, `zh-MO` and `zh-Hant-*`
    // locales all resolve here (see `candidates`).
    LangInfo { code: "zh-hant", name: "繁體中文", source: catalog!("zh-hant"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "es", name: "Español", source: catalog!("es"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "ru", name: "Русский", source: catalog!("ru"), plural: plural_russian, complete_menus: true, catalog: OnceLock::new() },
    // Ukrainian has the same one/few/many rule for integer counts.
    LangInfo { code: "uk", name: "Українська", source: catalog!("uk"), plural: plural_russian, complete_menus: true, catalog: OnceLock::new() },
    // Serbian (Latin script, Ekavian) has the same one/few/many rule for integer counts. `sr-RS`,
    // `sr-Latn-*` and `sr_RS@latin` resolve here; so do Cyrillic `sr` locales, which read Latin
    // rather than English.
    LangInfo { code: "sr-latn", name: "Srpski", source: catalog!("sr-latn"), plural: plural_russian, complete_menus: true, catalog: OnceLock::new() },
    // Bosnian (Latin script, Ijekavian), the same one/few/other rule for integer counts. `bs-BA`,
    // `bs-Latn-*` and `bs_BA.UTF-8` resolve here; so do Cyrillic `bs` locales, which read Latin
    // rather than English.
    LangInfo { code: "bs", name: "Bosanski", source: catalog!("bs"), plural: plural_russian, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "cs", name: "Čeština", source: catalog!("cs"), plural: plural_cs, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "fr", name: "Français", source: catalog!("fr"), plural: plural_fr, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "id", name: "Bahasa Indonesia", source: catalog!("id"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "ko", name: "한국어", source: catalog!("ko"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "pl", name: "Polski", source: catalog!("pl"), plural: plural_polish, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "de", name: "Deutsch", source: catalog!("de"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Brazilian Portuguese; `pt`, `pt-BR` and `pt-PT` locales all resolve here (see `candidates`).
    LangInfo { code: "pt-br", name: "Português (Brasil)", source: catalog!("pt-br"), plural: plural_pt, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "el", name: "Ελληνικά", source: catalog!("el"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Dutch; `nl-NL` and `nl-BE` locales both resolve here.
    LangInfo { code: "nl", name: "Nederlands", source: catalog!("nl"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    LangInfo { code: "it", name: "Italiano", source: catalog!("it"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Hungarian: a count takes the singular noun ("3 réteg"), so one plural form.
    LangInfo { code: "hu", name: "Magyar", source: catalog!("hu"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
    // Turkish; `tr`, `tr-TR` and `tr_TR` locales resolve here.
    LangInfo { code: "tr", name: "Türkçe", source: catalog!("tr"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
    // Azerbaijani (Latin script); regional and explicit Latin-script tags resolve here.
    LangInfo { code: "az", name: "Azərbaycanca", source: catalog!("az"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Vietnamese; `vi-VN` resolves here.
    LangInfo { code: "vi", name: "Tiếng Việt", source: catalog!("vi"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
    // Thai; `th`, `th-TH` and `th_TH.UTF-8` locales resolve here. Thai has no plural inflection,
    // so counts use one form.
    LangInfo { code: "th", name: "ไทย", source: catalog!("th"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
    // Hindi; `hi`, `hi-IN` locales resolve here.
    LangInfo { code: "hi", name: "हिन्दी", source: catalog!("hi"), plural: plural_one_other, complete_menus: false, catalog: OnceLock::new() },
    // Arabic (Modern Standard); `ar-SA`, `ar-EG` and the other regional locales resolve here. The
    // chrome mirrors right to left (`dir_for_code`), and egui lays each label out right to left.
    LangInfo { code: "ar", name: "العربية", source: catalog!("ar"), plural: plural_arabic, complete_menus: true, catalog: OnceLock::new() },
    // Swedish; `sv-SE` and `sv-FI` locales resolve here.
    LangInfo { code: "sv", name: "Svenska", source: catalog!("sv"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Finnish; `fi-FI` resolves here (Swedish in Finland, `sv-FI`, stays Swedish). One/other: the
    // partitive singular follows any number but 1 ("1 taso", "2 tasoa").
    LangInfo { code: "fi", name: "Suomi", source: catalog!("fi"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Danish; `da-DK` and the other `da-*` locales resolve here.
    LangInfo { code: "da", name: "Dansk", source: catalog!("da"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Norwegian Bokmål; `nb-NO`, and the macrolanguage `no` and Nynorsk `nn` from POSIX locales,
    // resolve here (see `candidates`).
    LangInfo { code: "nb", name: "Norsk bokmål", source: catalog!("nb"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Hebrew; `he-il` and `he_IL` resolve here. The chrome mirrors right to left (`dir_for_code`),
    // like Arabic. Modern Hebrew software UI collapses CLDR's one/two/many/other
    // into a plain one|other, so this uses `plural_one_other`.
    LangInfo { code: "he", name: "עברית", source: catalog!("he"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Urdu; written right to left like Arabic (`dir_for_code`). CLDR plural: one|other.
    LangInfo { code: "ur", name: "اردو", source: catalog!("ur"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Romanian; `ro-RO` and `ro-MD` resolve here. Standard left-to-right; see `plural_romanian`
    // for the one/few/other rule.
    LangInfo { code: "ro", name: "Română", source: catalog!("ro"), plural: plural_romanian, complete_menus: true, catalog: OnceLock::new() },
    // Tamil; `ta-IN`, `ta-LK` and `ta_IN.UTF-8` resolve here. CLDR plural: one|other.
    LangInfo { code: "ta", name: "தமிழ்", source: catalog!("ta"), plural: plural_one_other, complete_menus: true, catalog: OnceLock::new() },
    // Georgian: nouns stay singular after a numeral (`შრე: 5`, `5 შრე`), so one form serves every count.
    LangInfo { code: "ka", name: "ქართული", source: catalog!("ka"), plural: plural_none, complete_menus: true, catalog: OnceLock::new() },
];

impl LangInfo {
    /// The catalog file's text (empty for English, or if the bundled data can't be inflated).
    pub fn text(&self) -> String {
        unpack(self.source).unwrap_or_else(|e| {
            log::error!("translation catalog {}: {e}; showing English", self.code);
            String::new()
        })
    }

    fn catalog(&self) -> &Catalog {
        self.catalog.get_or_init(|| Catalog::parse(&self.text()))
    }
}

/// Per language of [`LANGUAGES`], PhotoCraft's own catalog rows as written (escaped): (context,
/// source) to translation, read on first use ([`Lang::owns`]).
static OWN_ROWS: [OnceLock<HashMap<(&'static str, &'static str), &'static str>>; LANGUAGE_COUNT] = [const { OnceLock::new() }; LANGUAGE_COUNT];

/// A language the UI can be shown in: English, or one of [`LANGUAGES`] whose catalogs are
/// embedded.
#[derive(Clone, Copy)]
pub struct Lang {
    lang: craft_i18n::Lang,
    /// The catalogs it is looked up in: the embedded ones ([`I18N`]), or a test fixture's.
    catalogs: Option<&'static I18n>,
}

impl std::fmt::Debug for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lang({})", self.code())
    }
}

impl PartialEq for Lang {
    fn eq(&self, other: &Self) -> bool {
        self.code() == other.code()
    }
}

impl Eq for Lang {}

impl Lang {
    pub const EN: Lang = Lang { lang: craft_i18n::Lang::EN, catalogs: None };

    fn i18n(self) -> &'static I18n {
        self.catalogs.unwrap_or_else(|| LazyLock::force(&I18N))
    }

    fn info(self) -> Option<&'static LangInfo> {
        let code = self.code();
        LANGUAGES.iter().find(|l| l.code == code)
    }

    pub fn code(self) -> &'static str {
        self.i18n().code(self.lang)
    }

    /// A language by its exact code (any case): one of [`LANGUAGES`] whose catalogs are embedded.
    pub fn from_code(code: &str) -> Option<Lang> {
        let code = if code.eq_ignore_ascii_case("sr") { "sr-latn" } else { code };
        let info = LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(code))?;
        I18N.from_code(info.code).map(|lang| Lang { lang, catalogs: None })
    }

    /// Resolve the `interface.language` preference: a language code, or `auto` (and anything
    /// unknown, e.g. a code from a newer version) to follow the system locale.
    pub fn from_pref(pref: &str) -> Lang {
        if pref.eq_ignore_ascii_case("auto") {
            return system_lang();
        }
        Lang::from_code(pref).or_else(|| lang_from_tag(pref)).unwrap_or_else(system_lang)
    }

    /// Every language the interface can be shown in: [`LANGUAGES`] in order, those whose catalogs
    /// are embedded (English alone in a build without craft-i18n).
    pub fn all() -> impl Iterator<Item = Lang> {
        LANGUAGES.iter().filter_map(|l| Lang::from_code(l.code))
    }

    /// Every registered language in the order a picker lists them: English first (the source
    /// language and the fallback), then the rest by their own name with accents folded, so
    /// Čeština sits under C and Türkçe under T rather than after Z. Names in other scripts follow
    /// the Latin ones in code point order: Greek, Cyrillic, then the CJK languages.
    pub fn for_picker() -> Vec<Lang> {
        let mut rest: Vec<Lang> = Lang::all().filter(|l| l.code() != "en").collect();
        rest.sort_by_cached_key(|l| name_sort_key(l.name()));
        std::iter::once(Lang::EN).chain(rest).collect()
    }

    /// The language's name in itself (its `@id language.name` row), shown in the Preferences
    /// dropdown.
    pub fn name(self) -> &'static str {
        self.i18n().name(self.lang)
    }

    /// Does this language's catalog claim to cover every menu string and `tl!` literal?
    pub fn complete_menus(self) -> bool {
        self.info().is_some_and(|l| l.complete_menus)
    }

    /// The language's reading direction.
    pub fn dir(self) -> Dir {
        dir_for_code(self.code())
    }

    fn catalog(self) -> &'static Catalog {
        self.info().unwrap_or(&LANGUAGES[0]).catalog()
    }

    #[cfg(test)]
    fn legacy_info(self) -> &'static LangInfo {
        self.info().unwrap_or(&LANGUAGES[0])
    }

    /// Whether `translated`, the lookup's text for (`context`, `source`), comes from PhotoCraft's
    /// own catalog (`photocraft.tsv`, not the shared `common.tsv`). The shared catalog's `@id` rows
    /// are keyed by the other apps' command ids, which can name another command, so command-id
    /// lookups ask this; every `@id` row PhotoCraft had is in its own catalog. Comparing the text,
    /// not only the key, keeps a shared row out when craft-i18n skipped a malformed own row. A test
    /// fixture has no shared catalog: its rows are its own.
    fn owns(self, context: &str, source: &str, translated: &str) -> bool {
        if let Some(fixture) = self.catalogs {
            return fixture.get(self.lang, context, source) == Some(translated);
        }
        let code = self.code();
        let Some(slot) = LANGUAGES.iter().position(|l| l.code == code).and_then(|i| OWN_ROWS.get(i)) else { return false };
        let rows: &HashMap<(&str, &str), &str> = slot.get_or_init(|| {
            app_catalog(code)
                .lines()
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| {
                    let mut columns = l.split('\t');
                    Some(((columns.next()?, columns.next()?), columns.next()?))
                })
                .collect()
        });
        let escape = |s: &str| s.replace('\\', "\\\\").replace('\n', "\\n").replace('\t', "\\t");
        rows.get(&(escape(context).as_str(), escape(source).as_str())).is_some_and(|own| *own == escape(translated))
    }
}

impl Lang {
    /// The embedded catalog texts this language reads, the shared one then PhotoCraft's; none for
    /// English. For checks that the interface fonts draw every translation.
    #[cfg(test)]
    pub(crate) fn catalog_texts(self) -> impl Iterator<Item = &'static str> {
        let code = self.code();
        CRAFT_I18N.iter().filter(move |(c, ..)| c.eq_ignore_ascii_case(code)).flat_map(|(_, shared, app)| [*shared, *app])
    }
}

/// PhotoCraft's own embedded catalog text (`photocraft.tsv`) in the locale `code`; empty when
/// there is none.
fn app_catalog(code: &str) -> &'static str {
    CRAFT_I18N.iter().find(|(c, ..)| c.eq_ignore_ascii_case(code)).map_or("", |(_, _, app)| *app)
}

/// Whether the fork translation snapshot is embedded. All supported languages are mandatory.
pub fn catalogs_embedded() -> bool {
    !CRAFT_I18N.is_empty()
}

/// `name` lowercased with the Latin accents that occur in language names folded away, so a
/// byte-wise sort reads alphabetically. Other scripts are left as they are.
fn name_sort_key(name: &str) -> String {
    name.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à'..='å' | 'ā' | 'ă' | 'ą' => 'a',
            'ç' | 'ć' | 'č' => 'c',
            'è'..='ë' | 'ē' | 'ě' | 'ę' | 'ế' | 'ệ' => 'e',
            'ì'..='ï' | 'ī' => 'i',
            'ñ' | 'ń' | 'ň' => 'n',
            'ò'..='ö' | 'ø' | 'ő' => 'o',
            'ś' | 'š' => 's',
            'ù'..='ü' | 'ū' | 'ů' | 'ű' => 'u',
            'ý' | 'ÿ' => 'y',
            'ź' | 'ż' | 'ž' => 'z',
            c => c,
        })
        .collect()
}

/// Candidate language codes for a locale tag, most specific first: `zh_TW.UTF-8` →
/// `zh-tw`, `zh-hant`, `zh`.
fn candidates(tag: &str) -> Vec<String> {
    let base = tag.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_ascii_lowercase();
    let parts: Vec<&str> = base.split('-').filter(|p| !p.is_empty()).collect();
    let Some(&primary) = parts.first() else { return Vec::new() };
    let mut out = Vec::new();
    for n in (1..=parts.len()).rev() {
        out.push(parts[..n].join("-"));
    }
    if primary == "zh" && !parts.iter().any(|p| matches!(*p, "hans" | "hant")) {
        // Chinese by region when no script is given.
        let script = if parts.iter().any(|p| matches!(*p, "tw" | "hk" | "mo")) { "zh-hant" } else { "zh-hans" };
        out.insert(out.len() - 1, script.to_string());
    }
    if primary == "pt" && !out.iter().any(|c| c == "pt-br") {
        // The only Portuguese catalog is Brazilian; other regions use it rather than English.
        out.insert(out.len() - 1, "pt-br".to_string());
    }
    if primary == "sr" && !out.iter().any(|c| c == "sr-latn") {
        // The only Serbian catalog is Latin (`sr-latn`). Plain, regional (`sr-RS`, `sr-ME`) and
        // Cyrillic (`sr-Cyrl-RS`) tags read it rather than English, as when it was PhotoCraft's
        // `sr` catalog; craft-i18n's own matching never takes `sr` to a script it doesn't name.
        out.insert(out.len() - 1, "sr-latn".to_string());
    }
    if matches!(primary, "no" | "nn") && !out.iter().any(|c| c == "nb") {
        // The only Norwegian catalog is Bokmål; the macrolanguage code and Nynorsk use it rather
        // than English.
        out.insert(out.len() - 1, "nb".to_string());
    }
    out
}

/// The registered language for a locale tag such as `ja_JP.UTF-8`, `ja-JP`, `zh-TW`; `None` if
/// the language isn't supported. `C`/`POSIX` mean English.
pub fn lang_from_tag(tag: &str) -> Option<Lang> {
    let cands = candidates(tag);
    if matches!(cands.first().map(String::as_str), Some("c" | "posix")) {
        return Some(Lang::EN);
    }
    cands.iter().find_map(|c| Lang::from_code(c))
}

thread_local! {
    // Independent app/test threads must not change each other's drawing language.
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::EN) };
}

/// Set the UI language for drawing (the shell calls this once per frame from the preference), so
/// widgets can translate without every call site carrying a language around.
pub fn set_current(lang: Lang) {
    CURRENT.set(lang);
}

/// The language the UI is drawn in.
pub fn current() -> Lang {
    CURRENT.get()
}

/// Temporarily draw in another language, restoring the previous one even on unwinding.
/// Preferences previews use this without changing the saved setting or other dialogs.
pub fn with_language<R>(lang: Lang, draw: impl FnOnce() -> R) -> R {
    let _restore = language_scope(lang);
    draw()
}

/// Keep the language active until this guard is dropped on the drawing thread.
#[must_use]
pub fn language_scope(lang: Lang) -> impl Drop {
    struct Restore {
        previous: Lang,
        _thread: std::marker::PhantomData<std::rc::Rc<()>>,
    }
    impl Drop for Restore {
        fn drop(&mut self) {
            set_current(self.previous);
        }
    }
    let restore = Restore { previous: current(), _thread: std::marker::PhantomData };
    set_current(lang);
    restore
}

/// Apply a committed language to this context. Font caches are rebuilt only on a language
/// change, so Han glyphs follow the selected script even when the previous font covered them.
pub fn sync_context(ctx: &egui::Context, language: &str) {
    let lang = Lang::from_pref(language);
    set_current(lang);
    let id = egui::Id::unique("photocraft-ui-language");
    let changed = ctx.data(|data| data.get_temp::<Lang>(id) != Some(lang));
    if changed {
        ctx.data_mut(|data| data.insert_temp(id, lang));
        crate::theme::install_fonts(ctx);
        ctx.request_repaint();
    }
}

/// Does `lang` have a catalog entry for this plain string? (English never does: it is the source.)
pub fn has(lang: Lang, s: &str) -> bool {
    lang.i18n().get(lang.lang, "", s).is_some()
}

/// Translate an English UI string into the current language ([`tr`] with [`current`]).
pub fn t(s: &str) -> &str {
    tr(current(), s)
}

/// Translate an English UI string; unknown strings come back unchanged.
pub fn tr(lang: Lang, s: &str) -> &str {
    lang.i18n().t(lang.lang, s)
}

/// Keep history labels stable in the engine and reuse menu/tool translations at display time.
pub fn history_label(lang: Lang, label: &str) -> &str {
    if let Some(value) = lang.catalog().history(label) {
        return value;
    }
    let source = match label.trim().to_ascii_lowercase().as_str() {
        "select rectangle" => "Rectangular Marquee Tool",
        "select ellipse" => "Elliptical Marquee Tool",
        "move selected pixels" => "Move Tool",
        "new color fill layer" => "Solid Color…",
        "new gradient fill layer" => "Gradient…",
        "new pattern fill layer" => "Pattern…",
        "new group" => "Group…",
        "rotate canvas" => "Image Rotation",
        _ => label,
    };
    lang.catalog().history(source).unwrap_or(label)
}

/// Like [`tr`], for an English string that needs a disambiguating `context`.
pub fn tr_ctx<'a>(lang: Lang, context: &str, s: &'a str) -> &'a str {
    lang.i18n().tc(lang.lang, context, s)
}

/// A string keyed by its command id, falling back to the translation of the English `label`. Only
/// PhotoCraft's own `@id` rows count ([`own_id`]).
pub fn tr_id<'a>(lang: Lang, id: &str, label: &'a str) -> &'a str {
    if own_id(lang, id, label) { lang.i18n().tid(lang.lang, id, label) } else { tr(lang, label) }
}

/// Whether `lang` has PhotoCraft's own `@id` row for `id` ([`Lang::owns`]: command ids are each
/// app's own) and it keeps the `{placeholders}` and trailing `…` of the English `label`. craft-i18n
/// can't check that, because an `@id` row's source is the id, so a row that doesn't is skipped
/// here and the label's translation shows instead.
fn own_id(lang: Lang, id: &str, label: &str) -> bool {
    lang.i18n().get(lang.lang, "@id", id).is_some_and(|translated| lang.owns("@id", id, translated) && same_shape(translated, label))
}

/// Whether `translated` has the `{placeholders}` (in any order) and the trailing `…` of `english`,
/// the check craft-i18n makes on every row whose source is English text, and no `{` left open or
/// `}` without its `{`.
fn same_shape(translated: &str, english: &str) -> bool {
    let placeholders = |text: &str| -> Option<Vec<String>> {
        let mut names = text.split('{').skip(1).map(|p| p.split_once('}').map(|(name, _)| name.to_owned())).collect::<Option<Vec<_>>>()?;
        if text.matches('}').count() != names.len() {
            return None;
        }
        names.sort_unstable();
        Some(names)
    };
    translated.ends_with('…') == english.ends_with('…') && placeholders(translated).is_some_and(|names| Some(names) == placeholders(english))
}

/// Fill `{name}` placeholders. Unknown placeholders are left as written.
pub fn fmt(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = template.to_string();
    for (k, v) in args {
        out = out.replace(&format!("{{{k}}}"), v);
    }
    out
}

/// A plural-aware message: `one`/`other` are the English forms (with `{n}` where the count goes).
/// The form is the language's CLDR category; `{n}` is written in `0`–`9` without grouping or
/// isolate marks, as PhotoCraft always has.
pub fn trn(lang: Lang, n: u64, one: &str, other: &str) -> String {
    lang.i18n().trn_plain(lang.lang, n, one, other)
}

/// A test language whose PhotoCraft catalog is `source` (catalog format), so tests of lookup code
/// don't depend on the embedded catalogs' wording. `code` picks the CLDR plural rules.
#[cfg(test)]
pub(crate) fn fixture_lang(code: &'static str, source: &'static str) -> Lang {
    let embedded: &'static craft_i18n::Embedded = Box::leak(Box::new([(code, "", source)]));
    let (i18n, _) = I18n::new(embedded, &[code]);
    let i18n: &'static I18n = Box::leak(Box::new(i18n));
    let lang = i18n.from_code(code).unwrap_or(craft_i18n::Lang::EN);
    Lang { lang, catalogs: Some(i18n) }
}

/// Require embedded catalogs rather than silently skipping translation coverage.
#[cfg(test)]
pub(crate) fn needs_catalogs() -> bool {
    assert!(catalogs_embedded(), "the fork must embed its translation catalogs");
    true
}

#[cfg(test)]
mod fork_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod live_tests;

#[cfg(test)]
mod history_tests {
    use super::*;

    #[test]
    fn history_labels_reuse_korean_menus_tools_and_keep_engine_text_unchanged() {
        let ko = Lang::from_code("ko").unwrap();
        for (source, translated) in
            [("Open", "열기"), ("Brush Tool", "브러시 도구"), ("Lasso", "올가미"), ("Fill Path", "패스 채우기"), ("Prompt Generate", "프롬프트로 생성")]
        {
            assert_eq!(history_label(ko, source), translated);
            assert_eq!(history_label(Lang::EN, source), source);
        }
        assert_eq!(history_label(ko, "  select rectangle  "), history_label(ko, "Select Rectangle"));
        assert_eq!(history_label(ko, "OPEN…"), "열기");
        assert_eq!(history_label(ko, "user supplied layer name"), "user supplied layer name");
        for source in ["Sampling steps", "Translate prompt to English", "Prompt information", "AI Prompt History", "Original prompt", "English translation"] {
            assert_ne!(tr(ko, source), source);
        }
    }
}
