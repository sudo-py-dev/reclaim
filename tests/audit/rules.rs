//! Security and code-quality rules over the parsed source.

use crate::workspace::{Context, Event, Finding, Workspace};
use std::collections::BTreeMap;

/// Files that talk to the operating system and therefore need `unsafe`. The
/// library crate (`src/lib.rs` and everything it includes) forbids it outright.
const UNSAFE_FILES: &[&str] = &[
    "src/diagnostics.rs",    // Windows exception filter and message box
    "src/drives/macos.rs",   // DiskArbitration and IOKit
    "src/drives/windows.rs", // volume and device IOCTLs
    "src/installer.rs",      // ShellExecuteW
    "src/native_process.rs", // posix_spawn / CreateProcessW
    "src/preview.rs",        // reopening a Windows handle, extended volume reads
    "src/update_runtime.rs", // geteuid for the elevated update helper
];

/// The only places that may start another program, and what they may call.
const PROCESS_APIS: &[(&str, &[&str])] = &[
    (
        "src/native_process.rs",
        &["posix_spawn", "CreateProcessW", "ShellExecuteExW"],
    ),
    ("src/installer.rs", &["ShellExecuteW"]),
];
const PROCESS_IDENTS: &[&str] = &[
    "posix_spawn",
    "posix_spawnp",
    "CreateProcessW",
    "CreateProcessA",
    "ShellExecuteW",
    "ShellExecuteA",
    "ShellExecuteExW",
    "WinExec",
    "execv",
    "execve",
    "execvp",
    "popen",
];

fn production(ws: &Workspace) -> impl Iterator<Item = (&str, &Event)> {
    ws.rust
        .iter()
        .flat_map(|f| f.production.iter().map(move |e| (f.path.as_str(), e)))
}

fn everywhere(ws: &Workspace) -> impl Iterator<Item = (&str, &Event)> {
    ws.rust.iter().flat_map(|f| {
        f.production
            .iter()
            .chain(f.tests.iter())
            .map(move |e| (f.path.as_str(), e))
    })
}

/// CWE-676 (dangerous functions) and CWE-119 (memory safety): `unsafe` only in the
/// platform layer, and every block explains why it is sound.
pub(crate) fn unsafe_code(ws: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    for file in &ws.rust {
        let lines: Vec<&str> = file.text.lines().collect();
        for event in file.production.iter().chain(file.tests.iter()) {
            let Event::Unsafe { line, kind } = event else {
                continue;
            };
            if !UNSAFE_FILES.contains(&file.path.as_str()) {
                findings.push(Finding::new(
                    "SEC-UNSAFE-SCOPE",
                    Some(676),
                    &file.path,
                    *line,
                    format!("{kind} outside the platform layer; keep `unsafe` in the files listed in the audit"),
                ));
                continue;
            }
            let start = line.saturating_sub(8);
            let documented = lines.get(start..*line).is_some_and(|window| {
                window
                    .iter()
                    .any(|l| l.contains("SAFETY:") || l.contains("# Safety"))
            });
            if !documented {
                findings.push(Finding::new(
                    "SEC-UNSAFE-DOC",
                    Some(119),
                    &file.path,
                    *line,
                    format!("{kind} needs a `// SAFETY:` comment explaining why it is sound"),
                ));
            }
        }
    }
    ws.unsuppressed(findings)
}

/// CWE-362, CWE-843, CWE-401 and CWE-908: global mutable state, type punning,
/// leaks and uninitialised memory.
pub(crate) fn memory_hazards(ws: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (file, event) in everywhere(ws) {
        match event {
            Event::StaticMut { line } => findings.push(Finding::new(
                "SEC-STATIC-MUT",
                Some(362),
                file,
                *line,
                "`static mut` is a data race waiting to happen; use an atomic, OnceLock or Mutex",
            )),
            Event::Call { line, path, .. } => {
                let last = path.rsplit("::").next().unwrap_or(path);
                let (rule, cwe, message) = match last {
                    "transmute" | "transmute_copy" => (
                        "SEC-TRANSMUTE",
                        843,
                        "`transmute` reinterprets memory unchecked; use a safe conversion",
                    ),
                    "forget" if path.contains("mem") => (
                        "SEC-LEAK",
                        401,
                        "`mem::forget` skips destructors and leaks resources",
                    ),
                    "leak" if path.contains("Box") => (
                        "SEC-LEAK",
                        401,
                        "`Box::leak` leaks memory for the life of the process",
                    ),
                    "zeroed" | "uninitialized" if path.contains("mem") => (
                        "SEC-UNINIT",
                        908,
                        "zeroed or uninitialised memory needs a reviewed reason",
                    ),
                    _ => continue,
                };
                findings.push(Finding::new(rule, Some(cwe), file, *line, message));
            }
            Event::Method { line, name, .. } if name == "assume_init" => {
                findings.push(Finding::new(
                    "SEC-UNINIT",
                    Some(908),
                    file,
                    *line,
                    "`assume_init` needs a reviewed reason",
                ));
            }
            Event::Unsafe { .. }
            | Event::Method { .. }
            | Event::Macro { .. }
            | Event::Index
            | Event::Str { .. }
            | Event::Ident { .. } => {}
        }
    }
    ws.unsuppressed(findings)
}

/// CWE-78: OS command injection. No shell, no `Command`, and native process APIs
/// only in the two reviewed launchers.
pub(crate) fn process_launching(ws: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (file, event) in production(ws) {
        match event {
            Event::Call { line, path, .. } if path.ends_with("Command::new") => {
                findings.push(Finding::new(
                    "SEC-PROCESS",
                    Some(78),
                    file,
                    *line,
                    "`std::process::Command` is not allowed in the app; use native_process",
                ));
            }
            Event::Ident { line, name } if PROCESS_IDENTS.contains(&name.as_str()) => {
                let allowed = PROCESS_APIS
                    .iter()
                    .any(|(path, apis)| *path == file && apis.contains(&name.as_str()));
                if !allowed {
                    findings.push(Finding::new(
                        "SEC-PROCESS",
                        Some(78),
                        file,
                        *line,
                        format!("`{name}` starts another program; only the reviewed launchers may do that"),
                    ));
                }
            }
            Event::Unsafe { .. }
            | Event::StaticMut { .. }
            | Event::Call { .. }
            | Event::Method { .. }
            | Event::Macro { .. }
            | Event::Index
            | Event::Str { .. }
            | Event::Ident { .. } => {}
        }
    }
    ws.unsuppressed(dedupe(findings))
}

/// CWE-319 (cleartext transport), CWE-295 (certificate validation) and CWE-327/328
/// (broken cryptography).
pub(crate) fn transport_and_crypto(ws: &Workspace) -> Vec<Finding> {
    const TLS_BYPASS: &[&str] = &[
        "danger",
        "insecure",
        "noverif",
        "no_verify",
        "accept_invalid",
        "skip_verif",
    ];
    const WEAK_CRYPTO: &[&str] = &["md4", "md5", "Md5", "sha1", "Sha1", "Rc4", "rc4"];
    let mut findings = Vec::new();
    for (file, event) in production(ws) {
        match event {
            Event::Str {
                line,
                value,
                context: Context::Code,
            } if value.starts_with("http://") => findings.push(Finding::new(
                "SEC-CLEARTEXT",
                Some(319),
                file,
                *line,
                "plain-HTTP URL; use HTTPS",
            )),
            Event::Ident { line, name } => {
                let lower = name.to_ascii_lowercase();
                if TLS_BYPASS.iter().any(|bad| lower.contains(bad)) {
                    findings.push(Finding::new(
                        "SEC-TLS",
                        Some(295),
                        file,
                        *line,
                        format!("`{name}` looks like a certificate-validation bypass"),
                    ));
                }
                if WEAK_CRYPTO.contains(&name.as_str()) {
                    findings.push(Finding::new(
                        "SEC-WEAK-CRYPTO",
                        Some(327),
                        file,
                        *line,
                        format!("`{name}` is a broken algorithm; use SHA-256 or better"),
                    ));
                }
            }
            Event::Unsafe { .. }
            | Event::StaticMut { .. }
            | Event::Call { .. }
            | Event::Method { .. }
            | Event::Macro { .. }
            | Event::Index
            | Event::Str { .. } => {}
        }
    }
    ws.unsuppressed(dedupe(findings))
}

/// CWE-798: credentials committed to the repository.
pub(crate) fn secrets(ws: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    for file in &ws.text {
        if file.path.starts_with("tests/audit/") {
            continue;
        }
        for (index, line) in file.text.lines().enumerate() {
            if let Some(kind) = secret_kind(line) {
                findings.push(Finding::new(
                    "SEC-SECRET",
                    Some(798),
                    &file.path,
                    index + 1,
                    format!("possible {kind} committed to the repository"),
                ));
            }
        }
    }
    ws.unsuppressed(findings)
}

fn secret_kind(line: &str) -> Option<&'static str> {
    if line.contains("-----BEGIN") && line.contains("PRIVATE KEY") {
        return Some("private key");
    }
    if line.contains("untrusted comment: minisign encrypted secret key")
        || line.contains("rsign encrypted secret key")
    {
        return Some("signing key");
    }
    let prefixed: &[(&str, usize, &str)] = &[
        ("ghp_", 36, "GitHub token"),
        ("gho_", 36, "GitHub token"),
        ("github_pat_", 60, "GitHub token"),
        ("AKIA", 16, "AWS access key"),
        ("xoxb-", 24, "Slack token"),
        ("sk_live_", 24, "Stripe key"),
        ("AIza", 35, "Google API key"),
    ];
    for (prefix, length, kind) in prefixed {
        let mut rest = line;
        while let Some(at) = rest.find(prefix) {
            let tail = &rest[at + prefix.len()..];
            let run = tail
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                .count();
            if run >= *length {
                return Some(kind);
            }
            rest = tail;
        }
    }
    None
}

/// CWE-377 and CWE-379: predictable names in the shared temporary directory.
pub(crate) fn temporary_files(ws: &Workspace) -> Vec<Finding> {
    let findings = everywhere(ws)
        .filter_map(|(file, event)| match event {
            Event::Call { line, path, .. } if path.ends_with("temp_dir") => Some(Finding::new(
                "SEC-TEMP",
                Some(377),
                file,
                *line,
                "`env::temp_dir()` paths are predictable; use `tempfile` for a private, unique directory",
            )),
            Event::Unsafe { .. } | Event::StaticMut { .. } | Event::Call { .. } | Event::Method { .. } | Event::Macro { .. } | Event::Index | Event::Str { .. } | Event::Ident { .. } => None,
        })
        .collect();
    ws.unsuppressed(findings)
}

/// CWE-400, CWE-770 and CWE-789: reads and allocations sized by untrusted input
/// must have an explicit upper bound.
pub(crate) fn resource_limits(ws: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (file, event) in production(ws) {
        match event {
            Event::Method {
                line,
                name,
                receiver,
            } if name == "read_to_end" || name == "read_to_string" => {
                if !receiver.contains("take") && !receiver.contains("limit") {
                    findings.push(Finding::new(
                        "DOS-READ",
                        Some(400),
                        file,
                        *line,
                        format!(
                            "`{name}` without `.take(n)` or `.limit(n)` reads until the source ends"
                        ),
                    ));
                }
            }
            Event::Call { line, path, .. }
                if path.ends_with("fs::read") || path.ends_with("fs::read_to_string") =>
            {
                findings.push(Finding::new(
                    "DOS-READ",
                    Some(400),
                    file,
                    *line,
                    "whole-file read has no size limit; open the file and use `.take(limit)`",
                ));
            }
            Event::Call { line, path, args } if path.ends_with("with_capacity") => {
                if let Some(size) = args.first().filter(|size| !bounded(size)) {
                    findings.push(Finding::new(
                        "DOS-ALLOC",
                        Some(789),
                        file,
                        *line,
                        format!("capacity `{size}` has no visible upper bound"),
                    ));
                }
            }
            Event::Macro { line, name, body } if name == "vec" => {
                if let Some((_, size)) = body.rsplit_once(';') {
                    let size = size.trim();
                    if !bounded(size) {
                        findings.push(Finding::new(
                            "DOS-ALLOC",
                            Some(789),
                            file,
                            *line,
                            format!("buffer length `{size}` has no visible upper bound"),
                        ));
                    }
                }
            }
            Event::Unsafe { .. }
            | Event::StaticMut { .. }
            | Event::Call { .. }
            | Event::Method { .. }
            | Event::Macro { .. }
            | Event::Index
            | Event::Str { .. }
            | Event::Ident { .. } => {}
        }
    }
    ws.unsuppressed(findings)
}

/// A size expression the audit can see is bounded: a constant expression, a named
/// constant, an explicit `min`/`clamp`, or the length of data already in memory.
fn bounded(expression: &str) -> bool {
    let constant = expression.chars().all(|c| {
        c.is_ascii_digit() || " _*+-<()".contains(c) || c.is_ascii_uppercase() || c == ':'
    });
    constant
        || expression.contains("min (")
        || expression.contains("clamp (")
        || expression.contains(". min")
        || expression.contains(". clamp")
        || (expression.contains(". len ()") && !expression.contains('*'))
}

/// CWE-125 and CWE-129: unchecked `a[i]` panics on malformed input, which in a
/// scanner is a denial of service. The count per file may only go down.
pub(crate) fn index_ratchet(ws: &Workspace) -> Vec<Finding> {
    let baseline_text = ws.read("tests/audit/baseline.toml");
    let baseline: toml::Table = baseline_text
        .parse()
        .expect("tests/audit/baseline.toml is valid TOML");
    let limits = baseline
        .get("index")
        .and_then(|v| v.as_table())
        .cloned()
        .unwrap_or_default();
    let mut counts: BTreeMap<&str, i64> = BTreeMap::new();
    for (file, event) in production(ws) {
        if matches!(event, Event::Index) {
            *counts.entry(file).or_default() += 1;
        }
    }
    let mut findings = Vec::new();
    for (file, count) in &counts {
        let limit = limits
            .get(*file)
            .and_then(toml::Value::as_integer)
            .unwrap_or(0);
        if *count > limit {
            findings.push(Finding::new(
                "OOB-INDEX",
                Some(125),
                file,
                1,
                format!("{count} unchecked index/slice expressions, baseline allows {limit}; use `.get()` for input-derived positions"),
            ));
        }
    }
    for (file, limit) in &limits {
        let limit = limit.as_integer().unwrap_or(0);
        let count = counts.get(file.as_str()).copied().unwrap_or(0);
        if count < limit {
            findings.push(Finding::new(
                "OOB-INDEX",
                Some(125),
                "tests/audit/baseline.toml",
                1,
                format!("{file} is down to {count} unchecked indexes (baseline {limit}); lower the baseline to lock it in"),
            ));
        }
    }
    findings
}

/// CWE-489 (leftover debug code), CWE-561 (dead code) and CWE-1164 (irrelevant
/// code): no debug macros, no silenced warnings, no stray files, no TODOs.
pub(crate) fn leftovers(ws: &Workspace) -> Vec<Finding> {
    const DEBUG_MACROS: &[&str] = &[
        "dbg",
        "todo",
        "unimplemented",
        "println",
        "print",
        "eprintln",
        "eprint",
    ];
    let mut findings = Vec::new();
    for (file, event) in production(ws) {
        if let Event::Macro { line, name, .. } = event
            && DEBUG_MACROS.contains(&name.as_str())
        {
            findings.push(Finding::new(
                "CODE-DEBUG",
                Some(489),
                file,
                *line,
                format!("`{name}!` in shipped code; use `log::` or return the error"),
            ));
        }
    }
    for file in &ws.text {
        let code = file.path.ends_with(".rs") || file.path.ends_with(".slint");
        if !code || file.path.starts_with("tests/audit/") || file.path.starts_with("website/") {
            continue;
        }
        for (index, line) in file.text.lines().enumerate() {
            let trimmed = line.trim_start();
            if (trimmed.starts_with("#[allow(") || trimmed.starts_with("#![allow("))
                && file.path.ends_with(".rs")
            {
                findings.push(Finding::new(
                    "CODE-ALLOW",
                    Some(1164),
                    &file.path,
                    index + 1,
                    "use `#[expect(lint, reason = \"...\")]` so the exception disappears with the code it excused",
                ));
            }
            if let Some(comment) = line.split_once("//").map(|(_, c)| c)
                && ["TODO", "FIXME", "XXX", "HACK"]
                    .iter()
                    .any(|marker| comment.contains(marker))
            {
                findings.push(Finding::new(
                    "CODE-TODO",
                    Some(1164),
                    &file.path,
                    index + 1,
                    "unfinished-work marker; track it as an issue instead",
                ));
            }
        }
    }
    // Text the user reads: no em dashes anywhere in interface copy or docs.
    for file in &ws.text {
        let copy = file.path.starts_with("locales/")
            || file.path.starts_with("ui/")
            || file.path.starts_with("website/src/")
            || file.path.ends_with(".md");
        if !copy {
            continue;
        }
        for (index, line) in file.text.lines().enumerate() {
            if line.contains('\u{2014}') {
                findings.push(Finding::new(
                    "STYLE-DASH",
                    None,
                    &file.path,
                    index + 1,
                    "em dash in user-facing text; rewrite with a colon, comma or full stop",
                ));
            }
        }
    }
    findings.extend(orphans(ws));
    ws.unsuppressed(findings)
}

/// Source files and interface assets that nothing uses.
fn orphans(ws: &Workspace) -> Vec<Finding> {
    let mut findings = Vec::new();
    let all_rust: String = ws.rust.iter().map(|f| f.text.as_str()).collect();
    for file in ws.rust.iter().filter(|f| f.path.starts_with("src/")) {
        let stem = file
            .path
            .rsplit('/')
            .next()
            .unwrap_or("")
            .trim_end_matches(".rs");
        if matches!(stem, "main" | "lib") {
            continue;
        }
        let declared = all_rust.contains(&format!("mod {stem};"))
            || all_rust.contains(&format!("mod {stem} {{"));
        if !declared {
            findings.push(Finding::new(
                "CODE-ORPHAN",
                Some(561),
                &file.path,
                1,
                "no `mod` declaration includes this file, so it is never compiled",
            ));
        }
    }
    let slint = ws.file("ui/app.slint");
    for entry in std::fs::read_dir(ws.root.join("ui/icons"))
        .into_iter()
        .flatten()
        .flatten()
    {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !slint.contains(&format!("icons/{name}")) {
            findings.push(Finding::new(
                "CODE-ORPHAN",
                Some(561),
                &format!("ui/icons/{name}"),
                1,
                "icon is not used by the interface",
            ));
        }
    }
    findings
}

/// The compiler-enforced half of the audit: these lints must stay at `deny`, and
/// CI must keep running Clippy, the tests and the advisory scan.
pub(crate) fn lint_policy(ws: &Workspace) -> Vec<Finding> {
    const RUST: &[&str] = &[
        "unsafe_op_in_unsafe_fn",
        // Peak tier (references/peak-lints.md in the house skill).
        "unused_must_use",
        "unused_qualifications",
        "trivial_casts",
        "trivial_numeric_casts",
        "let_underscore_drop",
        "elided_lifetimes_in_paths",
        "missing_debug_implementations",
    ];
    const CLIPPY: &[&str] = &[
        "undocumented_unsafe_blocks",
        "unwrap_used",
        "expect_used",
        "panic",
        "todo",
        "unimplemented",
        "dbg_macro",
        "print_stdout",
        "print_stderr",
        "allow_attributes",
        "allow_attributes_without_reason",
        "string_slice",
        "mem_forget",
        "large_stack_arrays",
        "unreachable",
        // Peak tier: whole groups plus the bug-catching picks.
        "correctness",
        "suspicious",
        "perf",
        "complexity",
        "style",
        "pedantic",
        "wildcard_enum_match_arm",
        "map_err_ignore",
        "significant_drop_in_scrutinee",
        "iter_over_hash_type",
        "clone_on_ref_ptr",
        "collection_is_never_read",
        "read_zero_byte_vec",
        "multiple_unsafe_ops_per_block",
    ];
    let manifest: toml::Table = ws
        .file("Cargo.toml")
        .parse()
        .expect("Cargo.toml is valid TOML");
    let lints = manifest.get("lints").and_then(|v| v.as_table());
    let mut findings = Vec::new();
    for (group, names) in [("rust", RUST), ("clippy", CLIPPY)] {
        let table = lints.and_then(|l| l.get(group)).and_then(|v| v.as_table());
        for name in names {
            let level = table.and_then(|t| t.get(*name)).and_then(|v| {
                v.as_str()
                    .map(str::to_owned)
                    .or_else(|| v.get("level").and_then(|l| l.as_str()).map(str::to_owned))
            });
            if !matches!(level.as_deref(), Some("deny" | "forbid")) {
                findings.push(Finding::new(
                    "LINT-POLICY",
                    None,
                    "Cargo.toml",
                    1,
                    format!("[lints.{group}] {name} must be \"deny\""),
                ));
            }
        }
    }
    let ci = ws.file(".github/workflows/build.yml");
    for (needle, what) in [
        (
            "cargo test --locked",
            "the test suite (which runs this audit)",
        ),
        (
            "cargo clippy --locked --all-targets -- -D warnings",
            "Clippy with warnings as errors",
        ),
        ("cargo-deny", "the RustSec advisory and license scan"),
    ] {
        if !ci.contains(needle) {
            findings.push(Finding::new(
                "LINT-POLICY",
                None,
                ".github/workflows/build.yml",
                1,
                format!("CI must run {what}"),
            ));
        }
    }
    findings
}

fn dedupe(mut findings: Vec<Finding>) -> Vec<Finding> {
    findings.sort_by(|a, b| (&a.file, a.line, a.rule).cmp(&(&b.file, b.line, b.rule)));
    findings.dedup_by(|a, b| a.file == b.file && a.line == b.line && a.rule == b.rule);
    findings
}

/// Largest fixture file read, and most bytes inflated from a gzip fixture.
const MAX_FIXTURE_BYTES: u64 = 64 << 20;
const MAX_INFLATED_BYTES: u64 = 256 << 20;

/// Path prefixes that put a local user name into a file: a home directory, a
/// desktop's removable-media mount point (which disk images record as their
/// last mount), and the Windows and macOS profile folders.
const USER_PATH_PREFIXES: &[&[u8]] = &[b"/home/", b"/run/media/", b"/Users/", b"\\Users\\"];

/// The user name that follows `prefix` at `at`, if it looks like one.
fn user_after(bytes: &[u8], at: usize, prefix: &[u8]) -> Option<String> {
    let rest = bytes.get(at + prefix.len()..)?;
    let length = rest
        .iter()
        .take(64)
        .take_while(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-'))
        .count();
    let separator = rest.get(length)?;
    (length > 0 && matches!(separator, b'/' | b'\\' | 0))
        .then(|| String::from_utf8_lossy(&rest[..length]).into_owned())
}

/// Every local user path in `bytes`.
fn user_paths(bytes: &[u8]) -> Vec<String> {
    let mut found = Vec::new();
    for prefix in USER_PATH_PREFIXES {
        for at in bytes
            .windows(prefix.len())
            .enumerate()
            .filter(|(_, window)| window == prefix)
            .map(|(at, _)| at)
        {
            if let Some(user) = user_after(bytes, at, prefix) {
                let path = format!("{}{user}", String::from_utf8_lossy(prefix));
                if !found.contains(&path) {
                    found.push(path);
                }
            }
        }
    }
    found
}

/// Test fixtures, disk images most of all, must not carry the local user
/// name of whoever made them (CWE-538). Mounting an ext image, for example,
/// records `/run/media/<user>/...` in its superblock and journal.
pub(crate) fn fixture_privacy(ws: &Workspace) -> Vec<Finding> {
    use std::io::Read;
    let mut findings = Vec::new();
    let mut pending = vec![ws.root.join("tests/fixtures")];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let Ok(kind) = entry.file_type() else {
                continue;
            };
            if kind.is_dir() {
                pending.push(path);
                continue;
            }
            let relative = path
                .strip_prefix(&ws.root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let Ok(file) = std::fs::File::open(&path) else {
                continue;
            };
            let mut bytes = Vec::new();
            if file
                .take(MAX_FIXTURE_BYTES + 1)
                .read_to_end(&mut bytes)
                .is_err()
                || bytes.len() as u64 > MAX_FIXTURE_BYTES
            {
                findings.push(Finding::new(
                    "PRIV-FIXTURE",
                    Some(538),
                    &relative,
                    1,
                    "fixture is unreadable or larger than the audit reads; keep fixtures small",
                ));
                continue;
            }
            let mut contents = user_paths(&bytes);
            if bytes.starts_with(&[0x1f, 0x8b]) {
                let mut inflated = Vec::new();
                if let Err(e) = flate2::read::MultiGzDecoder::new(bytes.as_slice())
                    .take(MAX_INFLATED_BYTES)
                    .read_to_end(&mut inflated)
                {
                    // What did inflate is still searched below.
                    findings.push(Finding::new(
                        "PRIV-FIXTURE",
                        Some(538),
                        &relative,
                        1,
                        format!("gzip fixture could not be fully checked: {e}"),
                    ));
                }
                for path in user_paths(&inflated) {
                    if !contents.contains(&path) {
                        contents.push(path);
                    }
                }
            }
            for path in contents {
                findings.push(Finding::new(
                    "PRIV-FIXTURE",
                    Some(538),
                    &relative,
                    1,
                    format!("contains the local user path {path:?}; scrub it before committing"),
                ));
            }
        }
    }
    findings
}

#[test]
fn user_paths_are_found_in_raw_bytes() {
    let image = b"\0\0/run/media/alex/1234-ABCD\0\0C:\\Users\\Sam\\Desktop\0word/media/x";
    assert_eq!(user_paths(image), vec!["/run/media/alex", "\\Users\\Sam"]);
    assert_eq!(
        user_paths(b"word/media/image1.png /home/ alone"),
        Vec::<String>::new()
    );
}
