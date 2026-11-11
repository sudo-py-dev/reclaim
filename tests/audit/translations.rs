//! Translation catalogs against the code: same keys in every language, every
//! interface message translated, no keys left behind by deleted code.

use crate::workspace::{Context, Event, Finding, Workspace};
use serde::de::{Deserialize, Deserializer, MapAccess, Visitor};
use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
};

const LANGUAGES: &[&str] = &["en", "he", "ar", "es", "fr", "de", "ru"];

/// Ordered key/value pairs, keeping duplicates so they can be reported.
struct Pairs(Vec<(String, String)>);

impl<'de> Deserialize<'de> for Pairs {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct PairsVisitor;
        impl<'de> Visitor<'de> for PairsVisitor {
            type Value = Pairs;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object of strings")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Pairs, A::Error> {
                let mut pairs = Vec::new();
                while let Some(entry) = map.next_entry::<String, String>()? {
                    pairs.push(entry);
                }
                Ok(Pairs(pairs))
            }
        }
        deserializer.deserialize_map(PairsVisitor)
    }
}

struct Catalog {
    path: String,
    pairs: Vec<(String, String)>,
}

fn load(ws: &Workspace) -> Vec<Catalog> {
    LANGUAGES
        .iter()
        .map(|code| {
            let path = format!("locales/{code}.json");
            let Pairs(pairs) =
                serde_json::from_str(ws.file(&path)).unwrap_or_else(|e| panic!("{path}: {e}"));
            Catalog { path, pairs }
        })
        .collect()
}

fn line_of_key(ws: &Workspace, path: &str, key: &str) -> usize {
    let quoted = serde_json::to_string(key).unwrap_or_default();
    ws.file(path)
        .lines()
        .position(|l| l.trim_start().starts_with(&quoted))
        .map(|i| i + 1)
        .unwrap_or(1)
}

/// `{0}`, `{e}`, `{}` and `{code:08X}` are all one placeholder to the translator,
/// which matches the text around them.
pub fn normalize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        out.push(c);
        if c == '{' {
            for inner in chars.by_ref() {
                if inner == '}' {
                    out.push('}');
                    break;
                }
            }
        }
    }
    out
}

fn placeholders(text: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find('{') {
        let Some(end) = rest[start..].find('}') else {
            break;
        };
        found.push(rest[start..start + end + 1].to_owned());
        rest = &rest[start + end + 1..];
    }
    found.sort();
    found
}

/// Catalog integrity: the same keys everywhere, no duplicates, nothing empty,
/// placeholders preserved, English values equal to their keys.
pub fn catalogs(ws: &Workspace) -> Vec<Finding> {
    let catalogs = load(ws);
    let mut findings = Vec::new();
    let english: BTreeSet<&str> = catalogs[0].pairs.iter().map(|(k, _)| k.as_str()).collect();
    for catalog in &catalogs {
        let mut seen = BTreeSet::new();
        for (key, value) in &catalog.pairs {
            let line = line_of_key(ws, &catalog.path, key);
            if !seen.insert(key.as_str()) {
                findings.push(Finding::new(
                    "I18N-DUPLICATE",
                    None,
                    &catalog.path,
                    line,
                    format!("duplicate key {key:?}"),
                ));
            }
            if value.trim().is_empty() {
                findings.push(Finding::new(
                    "I18N-EMPTY",
                    None,
                    &catalog.path,
                    line,
                    format!("empty translation for {key:?}"),
                ));
            }
            if placeholders(key) != placeholders(value) {
                findings.push(Finding::new(
                    "I18N-PLACEHOLDER",
                    None,
                    &catalog.path,
                    line,
                    format!("placeholders differ from the key in {key:?}"),
                ));
            }
            if catalog.path.ends_with("/en.json") && key != value {
                findings.push(Finding::new(
                    "I18N-ENGLISH",
                    None,
                    &catalog.path,
                    line,
                    format!("English text must equal its key: {key:?}"),
                ));
            }
            if value.contains(['\u{202a}', '\u{202b}', '\u{202c}', '\u{202d}', '\u{202e}']) {
                findings.push(Finding::new(
                    "I18N-BIDI",
                    Some(451),
                    &catalog.path,
                    line,
                    format!("bidi override character in {key:?}; use isolates"),
                ));
            }
        }
        let keys: BTreeSet<&str> = catalog.pairs.iter().map(|(k, _)| k.as_str()).collect();
        for missing in english.difference(&keys) {
            findings.push(Finding::new(
                "I18N-MISSING",
                None,
                &catalog.path,
                1,
                format!("no translation for {missing:?}"),
            ));
        }
        for extra in keys.difference(&english) {
            let line = line_of_key(ws, &catalog.path, extra);
            findings.push(Finding::new(
                "I18N-DEAD",
                None,
                &catalog.path,
                line,
                format!("{extra:?} is not an English key"),
            ));
        }
    }
    findings
}

/// Whether a literal reads like a sentence meant for people rather than an
/// identifier, format, path or file signature.
fn looks_like_message(text: &str) -> bool {
    let words: Vec<&str> = text
        .split_whitespace()
        .filter(|w| w.chars().filter(|c| c.is_alphabetic()).count() >= 2)
        .collect();
    text.trim() == text
        && text.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        && words.len() >= 2
        && text.chars().any(|c| c.is_lowercase())
        && !text.chars().any(char::is_control)
        && !["://", "::", "\\", "=", "%", "/dev/", "$"]
            .iter()
            .any(|bad| text.contains(bad))
}

fn english(ws: &Workspace) -> BTreeMap<String, String> {
    load(ws)
        .into_iter()
        .next()
        .map(|c| {
            c.pairs
                .into_iter()
                .map(|(k, _)| (normalize(&k), k))
                .collect()
        })
        .unwrap_or_default()
}

/// String literals in `ui/app.slint` outside the `Strings` global, which the app
/// cannot translate.
fn slint_literals(ws: &Workspace) -> Vec<(usize, String)> {
    let text = ws.file("ui/app.slint");
    let mut out = Vec::new();
    let mut in_strings = false;
    for (index, line) in text.lines().enumerate() {
        if line.starts_with("export global Strings") {
            in_strings = true;
        } else if in_strings && line.starts_with('}') {
            in_strings = false;
            continue;
        }
        // Font family names are not interface text.
        if in_strings || line.trim_start().starts_with("//") || line.contains("font-family") {
            continue;
        }
        let mut rest = line;
        while let Some(start) = rest.find('"') {
            let Some(end) = rest[start + 1..].find('"') else {
                break;
            };
            out.push((index + 1, rest[start + 1..start + 1 + end].to_owned()));
            rest = &rest[start + end + 2..];
        }
    }
    out
}

/// Interface text in the code that has no catalog entry is shown in English to
/// everyone. Log lines, assertions and attribute text are not interface text.
pub fn missing(ws: &Workspace) -> Vec<Finding> {
    let keys = english(ws);
    let mut findings = Vec::new();
    for file in ws.rust.iter().filter(|f| f.path.starts_with("src/")) {
        for event in &file.production {
            let Event::Str {
                line,
                value,
                context: Context::Code,
            } = event
            else {
                continue;
            };
            if looks_like_message(value) && !keys.contains_key(&normalize(value)) {
                findings.push(Finding::new(
                    "I18N-MISSING",
                    None,
                    &file.path,
                    *line,
                    format!("interface text has no translation: {value:?}"),
                ));
            }
        }
    }
    for (line, value) in slint_literals(ws) {
        if looks_like_message(&value) {
            findings.push(Finding::new(
                "I18N-HARDCODED",
                None,
                "ui/app.slint",
                line,
                format!("text outside the Strings global cannot be translated: {value:?}"),
            ));
        }
    }
    ws.unsuppressed(findings)
}

/// Keys no production code refers to. The translator derives singular forms
/// (`files` to `file`), so those count as used when their plural is.
pub fn dead(ws: &Workspace) -> Vec<Finding> {
    let mut used: BTreeSet<String> = BTreeSet::new();
    for file in ws.rust.iter().filter(|f| f.path.starts_with("src/")) {
        for event in &file.production {
            if let Event::Str {
                value,
                context: Context::Code | Context::Assertion | Context::Diagnostic,
                ..
            } = event
            {
                used.insert(normalize(value));
            }
        }
    }
    let slint = ws.file("ui/app.slint");
    let derived: BTreeSet<String> = used
        .iter()
        .flat_map(|key| {
            let singular = key
                .replace("files", "file")
                .replace("candidates", "candidate");
            let one = if key == "{} selected" {
                Some("{} selected (one)".to_owned())
            } else {
                None
            };
            std::iter::once(singular).chain(one)
        })
        .collect();
    let path = "locales/en.json";
    english(ws)
        .into_iter()
        .filter(|(normalized, key)| {
            !used.contains(normalized)
                && !derived.contains(normalized)
                && !slint.contains(&serde_json::to_string(key).unwrap_or_default())
        })
        .map(|(_, key)| {
            Finding::new(
                "I18N-DEAD",
                None,
                path,
                line_of_key(ws, path, &key),
                format!("no code uses {key:?}; remove it from every catalog"),
            )
        })
        .collect()
}

/// Every translatable property in the `Strings` global has a setter in
/// `src/ui_text.rs`, and its built-in English default is the same text.
pub fn slint_strings(ws: &Workspace) -> Vec<Finding> {
    let slint = ws.file("ui/app.slint");
    let ui_text = ws.file("src/ui_text.rs");
    let mut setters: BTreeMap<String, String> = BTreeMap::new();
    for line in ui_text.lines() {
        let trimmed = line.trim();
        if let Some((name, text)) = trimmed
            .strip_prefix("set_")
            .and_then(|r| r.split_once(" => "))
            && let Ok(text) = serde_json::from_str::<String>(text.trim_end_matches(','))
        {
            setters.insert(name.replace('_', "-"), text);
        }
    }
    let mut findings = Vec::new();
    let mut in_strings = false;
    for (index, line) in slint.lines().enumerate() {
        if line.starts_with("export global Strings") {
            in_strings = true;
            continue;
        }
        if in_strings && line.starts_with('}') {
            break;
        }
        let Some(rest) = line.trim().strip_prefix("in-out property <string> ") else {
            continue;
        };
        let Some((name, default)) = rest.split_once(':') else {
            continue;
        };
        let name = name.trim();
        let default = default.trim().trim_end_matches(';');
        let Ok(default) = serde_json::from_str::<String>(default) else {
            continue;
        };
        match setters.get(name) {
            Some(text) if *text == default => {}
            Some(text) => findings.push(Finding::new(
                "I18N-SLINT",
                None,
                "ui/app.slint",
                index + 1,
                format!("`{name}` defaults to {default:?} but its setter uses {text:?}"),
            )),
            None if !ui_text.contains(&format!("set_{}(", name.replace('-', "_"))) => findings
                .push(Finding::new(
                    "I18N-SLINT",
                    None,
                    "ui/app.slint",
                    index + 1,
                    format!("`{name}` is never translated by src/ui_text.rs"),
                )),
            None => {}
        }
    }
    findings
}
