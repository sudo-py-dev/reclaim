//! Generate missing UI translations through the free MyMemory API.
use crate::Result;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    fs,
    io::Write,
    path::Path,
    process::{Command, Stdio},
};

type Catalog = BTreeMap<String, String>;
const MAX_QUERY_BYTES: usize = 500;
const ENDPOINT: &str = "https://api.mymemory.translated.net/get";

pub(crate) fn run(directory: &Path, requested: &[String]) -> Result {
    let languages = fs::read_to_string(directory.join("languages.txt"))?;
    let targets: Vec<_> = languages
        .lines()
        .filter_map(|row| row.split_ascii_whitespace().nth(1))
        .filter(|code| *code != "en")
        .collect();
    if requested == ["--list"] {
        println!("{}", targets.join(" "));
        return Ok(());
    }
    for code in requested {
        if !targets.contains(&code.as_str()) {
            return Err(format!("Unknown target language {code:?}; use translate --list").into());
        }
    }

    let source: Catalog = serde_json::from_slice(&fs::read(directory.join("en.json"))?)?;
    let progress = directory.join(".translation-progress");
    fs::create_dir_all(&progress)?;
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(progress.join("lock"))?;
    lock.try_lock()?;

    for code in targets {
        if requested.is_empty() || requested.iter().any(|requested| requested == code) {
            translate_catalog(directory, &source, code, |text| request(code, text))
                .map_err(|error| format!("{code}: {error}"))?;
        }
    }
    Ok(())
}

fn load(path: &Path) -> Result<Catalog> {
    match fs::read(path) {
        Ok(bytes) => Ok(serde_json::from_slice(&bytes)?),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(Catalog::new()),
        Err(error) => Err(error.into()),
    }
}

fn save(path: &Path, catalog: &Catalog) -> Result {
    let mut temporary = tempfile::NamedTempFile::new_in(path.parent().ok_or("missing directory")?)?;
    serde_json::to_writer_pretty(&mut temporary, catalog)?;
    writeln!(temporary)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path)?;
    Ok(())
}

fn translate_catalog(
    directory: &Path,
    source: &Catalog,
    code: &str,
    translate: impl Fn(&str) -> Result<String>,
) -> Result {
    let destination = directory.join(format!("{code}.json"));
    let checkpoint = directory
        .join(".translation-progress")
        .join(format!("{code}.partial"));
    let published = load(&destination)?;
    let mut catalog = load(&checkpoint)?;
    catalog.extend(
        published
            .iter()
            .map(|(key, value)| (key.clone(), value.clone())),
    );
    catalog.retain(|key, value| source.contains_key(key) && valid(key, value));
    let missing: Vec<_> = source
        .keys()
        .filter(|key| !catalog.contains_key(*key))
        .collect();
    for key in missing {
        let value = match key.as_str() {
            "Reclaim" | "reclaim" => key.clone(),
            key if key.contains('\n') => {
                let lines = key
                    .split('\n')
                    .map(|line| translate_fragments(line, &translate))
                    .collect::<Result<Vec<_>>>()?;
                lines.join("\n")
            }
            key => {
                let candidate = translate(key)?;
                if valid(key, &candidate) {
                    candidate
                } else {
                    translate_fragments(key, &translate)?
                }
            }
        };
        if !valid(key, &value) {
            return Err(format!("Invalid translation for {key:?}").into());
        }
        catalog.insert(key.clone(), value);
        save(&checkpoint, &catalog)?;
    }
    if catalog != published {
        save(&destination, &catalog)?;
        println!("{code}: saved {} messages", catalog.len());
    }
    if checkpoint.exists() {
        fs::remove_file(checkpoint)?;
    }
    Ok(())
}

fn placeholders(mut text: &str) -> Vec<&str> {
    let mut fields = Vec::new();
    while let Some((_, rest)) = text.split_once('{') {
        let Some((field, tail)) = rest.split_once('}') else {
            break;
        };
        fields.push(field);
        text = tail;
    }
    fields.sort_unstable();
    fields
}

fn valid(key: &str, translation: &str) -> bool {
    !translation.trim().is_empty()
        && !translation
            .chars()
            .any(|c| matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}'))
        && placeholders(key) == placeholders(translation)
        && key.matches('\n').count() == translation.matches('\n').count()
}

// Keep paragraph numbering and splice unchanged format fields back in when a
// translation model rewrites them. Keep surrounding whitespace outside the API.
fn translate_fragments(line: &str, translate: &impl Fn(&str) -> Result<String>) -> Result<String> {
    let (prefix, mut text) = line
        .split_once(". ")
        .filter(|(number, _)| !number.is_empty() && number.bytes().all(|b| b.is_ascii_digit()))
        .map_or(("", line), |(_, rest)| {
            (&line[..line.len() - rest.len()], rest)
        });
    let mut output = prefix.to_owned();
    loop {
        let field = text.find('{').and_then(|start| {
            let end = text[start..].find('}')? + start + 1;
            Some((&text[..start], &text[start..end], &text[end..]))
        });
        let (fragment, field, rest) = field.unwrap_or((text, "", ""));
        let trimmed = fragment.trim();
        if trimmed.is_empty() || !trimmed.chars().any(char::is_alphabetic) {
            output.push_str(fragment);
        } else {
            let start = fragment.len() - fragment.trim_start().len();
            output.push_str(&fragment[..start]);
            let translated = translate(trimmed)?;
            output.push_str(translated.trim());
            output.push_str(&fragment[start + trimmed.len()..]);
        }
        output.push_str(field);
        if rest.is_empty() {
            return Ok(output);
        }
        text = rest;
    }
}

fn request(target: &str, text: &str) -> Result<String> {
    if text.len() > MAX_QUERY_BYTES {
        return Err(format!("MyMemory allows at most {MAX_QUERY_BYTES} UTF-8 bytes per message; split the source message {text:?}").into());
    }

    // curl honors Retry-After and backs off on transient failures, including HTTP 429.
    let mut child = Command::new("curl")
        .args([
            "--disable",
            "--silent",
            "--show-error",
            "--fail",
            "--proto",
            "=https",
            "--retry",
            "3",
            "--retry-max-time",
            "120",
            "--max-time",
            "45",
            "--get",
            "--data-urlencode",
            "q@-",
            "--data-urlencode",
            &format!("langpair=en|{target}"),
            "--url",
            ENDPOINT,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let write_result = child
        .stdin
        .take()
        .ok_or("missing curl stdin")?
        .write_all(text.as_bytes());
    let output = child.wait_with_output()?;
    write_result?;
    if !output.status.success() {
        return Err("MyMemory request failed; rerun to resume from saved translations".into());
    }
    parse_response(&output.stdout)
}

fn parse_response(body: &[u8]) -> Result<String> {
    let response: Value = serde_json::from_slice(body)?;
    let status = &response["responseStatus"];
    let status = status.as_u64().or_else(|| status.as_str()?.parse().ok());
    if response["quotaFinished"].as_bool() == Some(true) || matches!(status, Some(429 | 439)) {
        return Err(
            "MyMemory quota exceeded; rerun after it resets to resume from saved translations"
                .into(),
        );
    }
    match status {
        Some(200) => response["responseData"]["translatedText"]
            .as_str()
            .filter(|text| !text.trim().is_empty())
            .map(str::to_owned)
            .ok_or_else(|| "MyMemory response is missing translated text".into()),
        Some(429 | 439) => Err(
            "MyMemory quota exceeded; rerun after it resets to resume from saved translations"
                .into(),
        ),
        Some(status) => Err(format!("MyMemory rejected the translation (status {status})").into()),
        None => Err("MyMemory response is missing a valid status".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mymemory_response_rejects_quota_messages_as_translations() -> Result {
        let success = br#"{"responseStatus":200,"responseData":{"translatedText":"File {0}"}}"#;
        assert_eq!(parse_response(success)?, "File {0}");
        let quota =
            br#"{"responseStatus":"429","responseData":{"translatedText":"QUOTA EXCEEDED"}}"#;
        assert!(
            parse_response(quota)
                .unwrap_err()
                .to_string()
                .contains("quota exceeded")
        );
        let quota_flag = br#"{"responseStatus":200,"quotaFinished":true,"responseData":{"translatedText":"QUOTA EXCEEDED"}}"#;
        assert!(parse_response(quota_flag).is_err());
        assert!(parse_response(br#"{"responseStatus":200,"responseData":{}}"#).is_err());
        Ok(())
    }

    #[test]
    fn checkpoints_resume_and_complete_catalogs_preserve_existing_words() -> Result {
        let dir = tempfile::tempdir()?;
        fs::create_dir(dir.path().join(".translation-progress"))?;
        let source = (0..=2)
            .map(|i| (format!("Message {i:02}"), String::new()))
            .collect();
        let existing = Catalog::from([("Message 00".into(), "Reviewed".into())]);
        let path = dir.path().join("it.json");
        save(&path, &existing)?;
        let mut extended: Catalog = source;
        extended.insert("zz {0}".into(), String::new());
        let result = translate_catalog(dir.path(), &extended, "it", |text| {
            if text == "zz {0}" {
                return Err("provider unavailable".into());
            }
            Ok(format!("Translated {text}"))
        });
        assert!(result.is_err());
        assert_eq!(load(&path)?, existing);

        translate_catalog(dir.path(), &extended, "it", |text| match text {
            "zz {0}" => Ok("lost field".into()),
            "zz" => Ok("testo".into()),
            _ => Err("unexpected repeat request".into()),
        })?;
        let completed = load(&path)?;
        assert_eq!(completed.len(), extended.len());
        assert_eq!(completed["Message 00"], "Reviewed");
        assert_eq!(completed["zz {0}"], "testo {0}");
        Ok(())
    }
}
