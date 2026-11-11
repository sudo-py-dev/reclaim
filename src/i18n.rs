//! English message IDs with bundled translations. Business state and filenames
//! remain language-neutral; translate only at presentation boundaries.
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    io::{self, Read, Write},
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Hebrew,
    Spanish,
    French,
    German,
    Arabic,
    Russian,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    System,
    English,
    Hebrew,
    Spanish,
    French,
    German,
    Arabic,
    Russian,
}
static PREFERENCE: AtomicU8 = AtomicU8::new(0);
thread_local! { static OVERRIDE: Cell<Option<Language>> = const { Cell::new(None) }; }
fn en() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    C.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/en.json")).expect("valid English catalog")
    })
}
fn he() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    C.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/he.json")).expect("valid Hebrew catalog")
    })
}
fn catalog(language: Language) -> &'static BTreeMap<String, String> {
    macro_rules! bundled {
        ($file:literal) => {{
            static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
            C.get_or_init(|| {
                serde_json::from_str(include_str!($file)).expect("valid translation catalog")
            })
        }};
    }
    match language {
        Language::English => en(),
        Language::Hebrew => he(),
        Language::Spanish => bundled!("../locales/es.json"),
        Language::French => bundled!("../locales/fr.json"),
        Language::German => bundled!("../locales/de.json"),
        Language::Arabic => bundled!("../locales/ar.json"),
        Language::Russian => bundled!("../locales/ru.json"),
    }
}
pub fn preference() -> Preference {
    match PREFERENCE.load(Ordering::Relaxed) {
        1 => Preference::English,
        2 => Preference::Hebrew,
        3 => Preference::Spanish,
        4 => Preference::French,
        5 => Preference::German,
        6 => Preference::Arabic,
        7 => Preference::Russian,

        _ => Preference::System,
    }
}
pub fn language() -> Language {
    if let Some(language) = OVERRIDE.with(Cell::get) {
        return language;
    }
    match preference() {
        Preference::English => Language::English,
        Preference::Hebrew => Language::Hebrew,
        Preference::Spanish => Language::Spanish,
        Preference::French => Language::French,
        Preference::German => Language::German,
        Preference::Arabic => Language::Arabic,
        Preference::Russian => Language::Russian,
        Preference::System => {
            static SYSTEM: OnceLock<Language> = OnceLock::new();
            *SYSTEM
                .get_or_init(|| language_for_locale(&sys_locale::get_locale().unwrap_or_default()))
        }
    }
}
pub fn is_rtl() -> bool {
    matches!(language(), Language::Hebrew | Language::Arabic)
}
fn language_for_locale(locale: &str) -> Language {
    match locale
        .split(['_', '-', '.'])
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "he" | "iw" => Language::Hebrew,
        "es" => Language::Spanish,
        "fr" => Language::French,
        "de" => Language::German,
        "ar" => Language::Arabic,
        "ru" => Language::Russian,
        _ => Language::English,
    }
}
fn settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    root.map(|p| p.join("reclaim/language.json"))
}
pub fn init() {
    let Some(path) = settings_path() else {
        return;
    };
    if let Ok(file) = std::fs::File::open(path) {
        let mut value = String::new();
        if file.take(64).read_to_string(&mut value).is_ok() {
            let value = serde_json::from_str::<String>(&value).unwrap_or_default();
            PREFERENCE.store(
                match value.as_str() {
                    "en" => 1,
                    "he" => 2,
                    "es" => 3,
                    "fr" => 4,
                    "de" => 5,
                    "ar" => 6,
                    "ru" => 7,

                    _ => 0,
                },
                Ordering::Relaxed,
            );
        }
    }
}
pub fn set_preference(value: Preference) -> io::Result<()> {
    let number = match value {
        Preference::System => 0,
        Preference::English => 1,
        Preference::Hebrew => 2,
        Preference::Spanish => 3,
        Preference::French => 4,
        Preference::German => 5,
        Preference::Arabic => 6,
        Preference::Russian => 7,
    };
    PREFERENCE.store(number, Ordering::Relaxed);
    if let Some(path) = settings_path() {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        let text = match value {
            Preference::System => "\"system\"",
            Preference::English => "\"en\"",
            Preference::Hebrew => "\"he\"",
            Preference::Spanish => "\"es\"",
            Preference::French => "\"fr\"",
            Preference::German => "\"de\"",
            Preference::Arabic => "\"ar\"",
            Preference::Russian => "\"ru\"",
        };
        temporary.write_all(text.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|e| e.error)?;
    }
    Ok(())
}

fn fields(template: &str) -> Vec<(usize, usize, &str)> {
    let mut result = Vec::new();
    let mut at = 0;
    while let Some(start) = template[at..].find('{') {
        let start = at + start;
        let Some(end) = template[start..].find('}') else {
            break;
        };
        let end = start + end + 1;
        result.push((start, end, &template[start + 1..end - 1]));
        at = end;
    }
    result
}
fn capture<'a>(template: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let tokens = fields(template);
    if tokens.is_empty() {
        return None;
    }
    let mut at = 0;
    let mut source = 0;
    let mut values = Vec::new();
    for (i, (start, end, _)) in tokens.iter().enumerate() {
        let literal = &template[source..*start];
        if !text[at..].starts_with(literal) {
            return None;
        }
        at += literal.len();
        let next = tokens
            .get(i + 1)
            .map(|(n, _, _)| *n)
            .unwrap_or(template.len());
        let after = &template[*end..next];
        let count = if after.is_empty() {
            text.len() - at
        } else {
            text[at..].find(after)?
        };
        values.push(&text[at..at + count]);
        at += count;
        source = *end;
    }
    if text[at..] != template[source..] {
        return None;
    }
    Some(values)
}
pub fn t(text: &str) -> String {
    translate(text, language(), 0)
}
/// Isolate display data from the surrounding paragraph without changing stored data.
pub fn isolate(text: &str) -> String {
    let safe: String = text
        .chars()
        .filter(|c| !matches!(*c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        .collect();
    format!("\u{2068}{safe}\u{2069}")
}
/// Format a translated message with numbered, directionally isolated data values.
/// Values are never recursively translated or interpreted as formatting syntax.
pub fn message(key: &str, values: &[&str]) -> String {
    let template = t(key);
    let mut output = String::new();
    let mut at = 0;
    for (start, end, token) in fields(&template) {
        output.push_str(&template[at..start]);
        if let Some(value) = token
            .parse::<usize>()
            .ok()
            .and_then(|index| values.get(index))
        {
            output.push_str(&isolate(value));
        } else {
            output.push_str(&template[start..end]);
        }
        at = end;
    }
    output.push_str(&template[at..]);
    output
}
/// Catalog coverage for first-party UI messages.
pub fn has_message(key: &str) -> bool {
    en().contains_key(key)
}
fn translate(text: &str, language: Language, depth: usize) -> String {
    if language == Language::English || depth > 5 {
        return text.into();
    }
    let translations = catalog(language);
    if let Some(value) = translations.get(text) {
        return value.clone();
    }
    // Longer patterns take precedence over generic count labels.
    let mut patterns: Vec<_> = en().keys().filter(|key| key.contains('{')).collect();
    patterns.sort_by_key(|key| std::cmp::Reverse(key.len()));
    for key in patterns {
        let Some(values) = capture(key, text) else {
            continue;
        };
        if matches!(
            key.as_str(),
            "{} files" | "{} file" | "{} selected" | "{} selected (one)"
        ) && values[0].parse::<usize>().is_err()
        {
            continue;
        }
        let single = values.first().is_some_and(|n| *n == "1");
        let singular = if key.as_str() == "{} selected" {
            "{} selected (one)".into()
        } else {
            key.replace("files", "file")
                .replace("candidates", "candidate")
        };
        let translated = if single {
            translations
                .get(&singular)
                .or_else(|| translations.get(key))
        } else {
            translations.get(key)
        };
        let Some(translated) = translated else {
            continue;
        };
        let tokens = fields(key);
        let output_tokens = fields(translated);
        let mut used = HashMap::new();
        let mut output = String::new();
        let mut at = 0;
        for (start, end, token) in output_tokens {
            output.push_str(&translated[at..start]);
            let index = used.entry(token).or_insert(0usize);
            if let Some((i, _)) = tokens
                .iter()
                .enumerate()
                .filter(|(_, (_, _, t))| *t == token)
                .nth(*index)
            {
                output.push_str(&translate(values[i], language, depth + 1));
            }
            *index += 1;
            at = end;
        }
        output.push_str(&translated[at..]);
        return output;
    }
    text.into()
}

/// Scope presentation to a language without changing the saved preference.
pub fn with_language<R>(language: Language, f: impl FnOnce() -> R) -> R {
    struct Reset(Option<Language>);
    impl Drop for Reset {
        fn drop(&mut self) {
            OVERRIDE.with(|v| v.set(self.0));
        }
    }
    let _reset = Reset(OVERRIDE.with(|v| v.replace(Some(language))));
    f()
}

#[cfg(test)]
mod tests {
    use super::*;
    const LANGUAGES: [Language; 7] = [
        Language::English,
        Language::Hebrew,
        Language::Spanish,
        Language::French,
        Language::German,
        Language::Arabic,
        Language::Russian,
    ];

    #[test]
    fn catalogs_cover_all_keys_and_keep_format_arguments() {
        for language in LANGUAGES {
            let translations = catalog(language);
            assert_eq!(
                en().keys().collect::<Vec<_>>(),
                translations.keys().collect::<Vec<_>>(),
                "{language:?}"
            );
            for (key, translation) in translations {
                assert!(!translation.trim().is_empty());
                let mut source: Vec<_> = fields(key).into_iter().map(|(_, _, key)| key).collect();
                let mut target: Vec<_> = fields(translation)
                    .into_iter()
                    .map(|(_, _, key)| key)
                    .collect();
                source.sort();
                target.sort();
                assert_eq!(
                    source, target,
                    "Placeholder mismatch in {language:?}: {key}"
                );
            }
        }
    }

    #[test]
    fn new_locales_translate_nested_messages_and_preserve_paths() {
        for (locale, language) in [
            ("es_MX.UTF-8", Language::Spanish),
            ("fr-CA", Language::French),
            ("de_DE", Language::German),
            ("ar-SA", Language::Arabic),
            ("ru_RU", Language::Russian),
        ] {
            assert_eq!(language_for_locale(locale), language);
            assert_eq!(
                translate("/media/user/Photos", language, 0),
                "/media/user/Photos"
            );
            let error = translate("Scan failed: Invalid source offset", language, 0);
            assert!(!error.contains("Scan failed"));
            assert!(error.contains(&translate("Invalid source offset", language, 0)));
            assert!(translate("2 files", language, 0).contains('2'));
            with_language(language, || {
                assert_eq!(is_rtl(), language == Language::Arabic)
            });
        }
    }
    #[test]
    fn locale_fallback_counts_and_nested_errors() {
        assert_eq!(language_for_locale("he_IL.UTF-8"), Language::Hebrew);
        assert_eq!(language_for_locale("iw-IL"), Language::Hebrew);
        assert_eq!(language_for_locale("unknown"), Language::English);
        assert_eq!(translate("2 files", Language::Hebrew, 0), "2 קבצים");
        assert_eq!(translate("1 files", Language::Hebrew, 0), "1 קובץ");
        assert_eq!(
            translate("Scan failed: Invalid source offset", Language::Hebrew, 0),
            "הסריקה נכשלה: מיקום הנתונים במקור אינו תקין"
        );
        assert_eq!(
            translate("/media/user/Photos", Language::Hebrew, 0),
            "/media/user/Photos"
        );
        assert_eq!(
            translate("Search files…", Language::English, 0),
            "Search files…"
        );
    }
    #[test]
    fn formatted_data_is_isolated_without_recursive_translation_or_injection() {
        let path = "/media/תמונות/report {1}.png";
        for language in LANGUAGES {
            with_language(language, || {
                let value = message("Mounted at {0}", &[path]);
                assert!(value.contains(&isolate(path)));
                assert!(value.contains("{1}"));
                assert!(value.ends_with('\u{2069}'));
            });
        }
        assert_eq!(
            isolate("photo\u{202e}.exe\u{2069}"),
            "\u{2068}photo.exe\u{2069}"
        );
    }
}
