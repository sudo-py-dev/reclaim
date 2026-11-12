//! Supply-chain policy (CWE-1104, CWE-1357): exact versions, a lockfile that
//! matches, reviewed and dated advisory exceptions, and a fuzz harness that
//! tests the same dependency versions the app ships.

use crate::workspace::{Finding, Workspace};
use std::collections::BTreeSet;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maintenance notices accepted in deny.toml, with the exact version reviewed.
/// A new version, or any other exception, needs a fresh review.
const REVIEWED_ADVISORIES: &[(&str, &str, &str)] = &[
    ("RUSTSEC-2026-0370", "zvariant", "5.15.1"),
    ("RUSTSEC-2026-0371", "zbus_names", "4.3.5"),
    ("RUSTSEC-2026-0372", "zvariant_derive", "5.15.1"),
    ("RUSTSEC-2026-0441", "pin-utils", "0.1.1"),
    ("RUSTSEC-2026-0192", "ttf-parser", "0.25.1"),
];

/// Crates that provide broken cryptography or launch other programs.
const BANNED_DIRECT: &[&str] = &[
    "md5",
    "md-5",
    "sha1",
    "sha-1",
    "rust-crypto",
    "open",
    "webbrowser",
    "self-replace",
];

fn table<'a>(value: &'a toml::Value, key: &str) -> Option<&'a toml::Table> {
    value.get(key).and_then(toml::Value::as_table)
}

fn dependency_tables(manifest: &toml::Value) -> Vec<(String, &toml::Table)> {
    let mut tables = Vec::new();
    for kind in ["dependencies", "build-dependencies", "dev-dependencies"] {
        if let Some(deps) = table(manifest, kind) {
            tables.push((kind.to_owned(), deps));
        }
    }
    if let Some(targets) = table(manifest, "target") {
        for (target, value) in targets {
            for kind in ["dependencies", "build-dependencies", "dev-dependencies"] {
                if let Some(deps) = table(value, kind) {
                    tables.push((format!("target.{target}.{kind}"), deps));
                }
            }
        }
    }
    tables
}

fn line_of(text: &str, needle: &str) -> usize {
    text.lines()
        .position(|l| l.contains(needle))
        .map(|i| i + 1)
        .unwrap_or(1)
}

pub fn manifest(ws: &Workspace) -> Vec<Finding> {
    let text = ws.file("Cargo.toml");
    let manifest = toml::Value::Table(text.parse().expect("Cargo.toml is valid TOML"));
    let mut findings = Vec::new();
    for (section, deps) in dependency_tables(&manifest) {
        for (name, spec) in deps {
            let version = spec
                .as_str()
                .or_else(|| spec.get("version").and_then(toml::Value::as_str));
            if !version.is_some_and(|v| v.starts_with('=')) {
                findings.push(Finding::new(
                    "DEP-PIN",
                    Some(1104),
                    "Cargo.toml",
                    line_of(text, &format!("{name} ="))
                        .max(line_of(text, &format!("[{section}.{name}]"))),
                    format!("[{section}] {name} must pin an exact reviewed version (\"=x.y.z\")"),
                ));
            }
            if spec.get("git").is_some() || spec.get("path").is_some() {
                findings.push(Finding::new(
                    "DEP-SOURCE",
                    Some(1357),
                    "Cargo.toml",
                    line_of(text, name),
                    format!("{name} must come from crates.io"),
                ));
            }
            if BANNED_DIRECT.contains(&name.as_str()) {
                findings.push(Finding::new(
                    "DEP-BANNED",
                    Some(327),
                    "Cargo.toml",
                    line_of(text, name),
                    format!("{name} is banned by project policy"),
                ));
            }
        }
    }
    let package = table(&manifest, "package").expect("[package]");
    let name = package
        .get("name")
        .and_then(toml::Value::as_str)
        .unwrap_or_default();
    let version = package
        .get("version")
        .and_then(toml::Value::as_str)
        .unwrap_or_default();
    let lock = toml::Value::Table(
        ws.file("Cargo.lock")
            .parse()
            .expect("Cargo.lock is valid TOML"),
    );
    let locked = lock
        .get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .any(|p| {
            p.get("name").and_then(toml::Value::as_str) == Some(name)
                && p.get("version").and_then(toml::Value::as_str) == Some(version)
        });
    if !locked {
        findings.push(Finding::new(
            "DEP-LOCK",
            Some(1104),
            "Cargo.lock",
            1,
            format!("lockfile does not contain {name} {version}; run cargo update -p {name}"),
        ));
    }
    findings
}

fn packages(lock_text: &str) -> Vec<(String, String, String, String)> {
    let lock = toml::Value::Table(lock_text.parse().expect("lockfile is valid TOML"));
    lock.get("package")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .map(|p| {
            let field = |k: &str| {
                p.get(k)
                    .and_then(toml::Value::as_str)
                    .unwrap_or_default()
                    .to_owned()
            };
            (
                field("name"),
                field("version"),
                field("source"),
                field("checksum"),
            )
        })
        .collect()
}

/// Days since 1970-01-01 for a civil date (proleptic Gregorian).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let yoe = year - era * 400;
    let mp = (month + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn parse_date(text: &str) -> Option<i64> {
    let mut parts = text.split('-').map(|p| p.parse::<i64>().ok());
    let (year, month, day) = (parts.next()??, parts.next()??, parts.next()??);
    ((1..=12).contains(&month) && (1..=31).contains(&day))
        .then(|| days_from_civil(year, month, day))
}

/// cargo-deny records a reason but does not expire exceptions; this does.
pub fn advisories(ws: &Workspace) -> Vec<Finding> {
    let text = ws.file("deny.toml");
    let config = toml::Value::Table(text.parse().expect("deny.toml is valid TOML"));
    let today = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        // Days since 1970 are far below i64::MAX.
        .map(|d| (d.as_secs() / 86_400).cast_signed())
        .unwrap_or(0);
    let locked = packages(ws.file("Cargo.lock"));
    let mut findings = Vec::new();
    let ignores = config
        .get("advisories")
        .and_then(|a| a.get("ignore"))
        .and_then(toml::Value::as_array)
        .cloned()
        .unwrap_or_default();
    for entry in &ignores {
        let id = entry
            .get("id")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        let line = line_of(text, id);
        let Some((_, crate_name, version)) = REVIEWED_ADVISORIES
            .iter()
            .find(|(reviewed, ..)| *reviewed == id)
        else {
            findings.push(Finding::new(
                "DEP-ADVISORY",
                Some(1104),
                "deny.toml",
                line,
                format!("{id:?} is not a reviewed exception; fix the dependency instead"),
            ));
            continue;
        };
        let reason = entry
            .get("reason")
            .and_then(toml::Value::as_str)
            .unwrap_or_default();
        let expiry = reason
            .strip_prefix("Expires ")
            .and_then(|r| r.get(..10))
            .and_then(parse_date);
        match expiry {
            None => findings.push(Finding::new(
                "DEP-ADVISORY",
                Some(1104),
                "deny.toml",
                line,
                format!("{id} reason must start with \"Expires YYYY-MM-DD.\""),
            )),
            Some(day) if day <= today => findings.push(Finding::new(
                "DEP-ADVISORY",
                Some(1104),
                "deny.toml",
                line,
                format!("{id} exception has expired; re-review or remove it"),
            )),
            Some(_) => {}
        }
        let versions: BTreeSet<&str> = locked
            .iter()
            .filter(|p| p.0 == *crate_name)
            .map(|p| p.1.as_str())
            .collect();
        if versions != BTreeSet::from([*version]) {
            findings.push(Finding::new(
                "DEP-ADVISORY",
                Some(1104),
                "deny.toml",
                line,
                format!("{id} was reviewed for {crate_name} {version}, but the lockfile has {versions:?}"),
            ));
        }
    }
    findings
}

/// The fuzz harnesses have their own lockfiles; they must not drift to versions
/// the app does not ship, or the fuzzing proves nothing about the release.
pub fn fuzz_lock(ws: &Workspace) -> Vec<Finding> {
    const HARNESS_ONLY: &[&str] = &[
        "reclaim-fuzz",
        "reclaim-container-fuzz",
        "arbitrary",
        "libfuzzer-sys",
        "derive_arbitrary",
    ];
    let production: BTreeSet<_> = packages(ws.file("Cargo.lock")).into_iter().collect();
    let mut findings = Vec::new();
    for path in ["fuzz/Cargo.lock", "fuzz/containers/Cargo.lock"] {
        let Some(file) = ws.text.iter().find(|t| t.path == path) else {
            continue;
        };
        for package in packages(&file.text) {
            if !HARNESS_ONLY.contains(&package.0.as_str()) && !production.contains(&package) {
                findings.push(Finding::new(
                    "DEP-FUZZ-DRIFT",
                    Some(1104),
                    path,
                    line_of(&file.text, &format!("name = \"{}\"", package.0)),
                    format!(
                        "{} {} differs from the production lockfile",
                        package.0, package.1
                    ),
                ));
            }
        }
    }
    findings
}
