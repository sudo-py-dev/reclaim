//! Loads the repository once and turns every Rust file into a flat list of events
//! (calls, literals, macros, indexing, `unsafe`) that the rules filter.

use proc_macro2::{TokenStream, TokenTree};
use std::{
    collections::BTreeSet,
    fmt,
    path::{Path, PathBuf},
    sync::Mutex,
};
use syn::{
    Attribute, Expr, ExprCall, ExprIndex, ExprMethodCall, ImplItemFn, ItemFn, ItemImpl, ItemMod,
    ItemStatic, Macro, StaticMutability, visit::Visit,
};

pub(crate) struct Finding {
    pub rule: &'static str,
    pub cwe: Option<u32>,
    pub file: String,
    pub line: usize,
    pub message: String,
}

impl Finding {
    pub(crate) fn new(
        rule: &'static str,
        cwe: Option<u32>,
        file: &str,
        line: usize,
        message: impl Into<String>,
    ) -> Self {
        Self {
            rule,
            cwe,
            file: file.into(),
            line,
            message: message.into(),
        }
    }
}

impl fmt::Display for Finding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let cwe = self.cwe.map_or_default(|id| format!(" CWE-{id}"));
        write!(
            f,
            "[{}{cwe}] {}:{}: {}",
            self.rule, self.file, self.line, self.message
        )
    }
}

/// Fails the calling test with every finding, one per line.
pub(crate) fn report(findings: &[Finding]) {
    if findings.is_empty() {
        return;
    }
    let lines: Vec<String> = findings.iter().map(ToString::to_string).collect();
    panic!(
        "{} audit finding(s):\n  {}\n\nFix the code, or accept a finding next to it with `// audit-allow(RULE): reason`.",
        lines.len(),
        lines.join("\n  ")
    );
}

/// Where a string literal appears, which decides whether it can be interface text.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum Context {
    Code,
    /// Arguments of `log::*` macros: diagnostics for the log file, never shown.
    Log,
    /// `assert!`, `panic!`, `unreachable!` and `.expect()` messages.
    Assertion,
    /// Attribute arguments such as `#[expect(..., reason = "...")]`.
    Attribute,
    /// Technical detail for errors: `diagnostic("...")`, `invalid("...")` and
    /// `io::Error` messages. Shown in English after a translated summary.
    Diagnostic,
}

#[derive(Debug)]
pub(crate) enum Event {
    Unsafe {
        line: usize,
        kind: &'static str,
    },
    StaticMut {
        line: usize,
    },
    /// A function call by path, such as `std::env::temp_dir()`.
    Call {
        line: usize,
        path: String,
        args: Vec<String>,
    },
    /// A method call; `receiver` is the receiver expression as written.
    Method {
        line: usize,
        name: String,
        receiver: String,
    },
    Macro {
        line: usize,
        name: String,
        body: String,
    },
    Index,
    Str {
        line: usize,
        value: String,
        context: Context,
    },
    Ident {
        line: usize,
        name: String,
    },
}

pub(crate) struct RustFile {
    pub path: String,
    pub text: String,
    /// Events outside `#[cfg(test)]` code; test files have none.
    pub production: Vec<Event>,
    /// Events in test code, including whole test files.
    pub tests: Vec<Event>,
}

pub(crate) struct TextFile {
    pub path: String,
    pub text: String,
}

struct Suppression {
    file: String,
    line: usize,
    rule: String,
}

pub(crate) struct Workspace {
    pub root: PathBuf,
    pub rust: Vec<RustFile>,
    /// Every text file the repository ships (code, configuration, docs, site source).
    pub text: Vec<TextFile>,
    suppressions: Vec<Suppression>,
    used: Mutex<BTreeSet<usize>>,
    malformed: Vec<Finding>,
}

const SKIP_DIRS: &[&str] = &[
    "target",
    ".git",
    // Git-ignored tool state, including other sessions' worktrees of this repository.
    ".claude",
    "node_modules",
    "dist",
    ".astro",
    ".vercel",
    "corpus",
    "artifacts",
];
const TEXT_EXTENSIONS: &[&str] = &[
    "rs", "slint", "toml", "json", "md", "yml", "yaml", "ts", "astro", "mjs", "css", "html", "txt",
    "iss", "plist", "desktop", "pub", "svg",
];

impl Workspace {
    pub(crate) fn load() -> Self {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let mut text = Vec::new();
        collect_text(&root, &root, &mut text);
        text.sort_by(|a, b| a.path.cmp(&b.path));

        let mut rust = Vec::new();
        for file in &text {
            let in_scope = file.path.ends_with(".rs")
                && (file.path.starts_with("src/") || file.path.starts_with("tests/") || file.path == "build.rs")
                // The audit's own rule tables name the very patterns it bans.
                && !file.path.starts_with("tests/audit/");
            if !in_scope {
                continue;
            }
            let ast = syn::parse_file(&file.text)
                .unwrap_or_else(|error| panic!("{}: cannot parse: {error}", file.path));
            let whole_file_is_test = file.path.starts_with("tests/")
                || file.path.ends_with("_tests.rs")
                || file.path.ends_with("/tests.rs");
            let mut walker = Walker {
                in_test: whole_file_is_test,
                context: Context::Code,
                production: vec![],
                tests: vec![],
            };
            walker.visit_file(&ast);
            rust.push(RustFile {
                path: file.path.clone(),
                text: file.text.clone(),
                production: walker.production,
                tests: walker.tests,
            });
        }

        let mut suppressions = Vec::new();
        let mut malformed = Vec::new();
        for file in text
            .iter()
            .filter(|f| f.path.ends_with(".rs") || f.path.ends_with(".slint"))
        {
            if file.path.starts_with("tests/audit/") {
                continue;
            }
            for (index, line) in file.text.lines().enumerate() {
                let Some(at) = line.find("audit-allow(") else {
                    continue;
                };
                let rest = &line[at + "audit-allow(".len()..];
                let (rule, reason) = match rest.split_once("):") {
                    Some((rule, reason)) => (rule.trim(), reason.trim()),
                    None => ("", ""),
                };
                if rule.is_empty() || reason.len() < 12 {
                    malformed.push(Finding::new(
                        "AUDIT-SUPPRESSION",
                        None,
                        &file.path,
                        index + 1,
                        "suppression needs `audit-allow(RULE): reason` with a real reason",
                    ));
                    continue;
                }
                suppressions.push(Suppression {
                    file: file.path.clone(),
                    line: index + 1,
                    rule: rule.into(),
                });
            }
        }
        Self {
            root,
            rust,
            text,
            suppressions,
            used: Mutex::new(BTreeSet::new()),
            malformed,
        }
    }

    /// Whether a finding is accepted by a suppression on its line or in the comment
    /// block directly above it.
    pub(crate) fn suppressed(&self, rule: &str, file: &str, line: usize) -> bool {
        let Some(text) = self.text.iter().find(|t| t.path == file) else {
            return false;
        };
        let lines: Vec<&str> = text.text.lines().collect();
        let mut candidates = vec![line];
        let mut above = line;
        while above > 1 {
            above -= 1;
            let trimmed = lines.get(above - 1).map_or("", |l| l.trim_start());
            if trimmed.starts_with("//") {
                candidates.push(above);
            } else {
                break;
            }
        }
        for (index, suppression) in self.suppressions.iter().enumerate() {
            if suppression.file == file
                && suppression.rule == rule
                && candidates.contains(&suppression.line)
            {
                self.used
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .insert(index);
                return true;
            }
        }
        false
    }

    /// Keeps findings that no suppression accepts.
    pub(crate) fn unsuppressed(&self, findings: Vec<Finding>) -> Vec<Finding> {
        findings
            .into_iter()
            .filter(|f| !self.suppressed(f.rule, &f.file, f.line))
            .collect()
    }

    pub(crate) fn unused_suppressions(&self) -> Vec<Finding> {
        let used = self
            .used
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let mut findings: Vec<Finding> = self
            .suppressions
            .iter()
            .enumerate()
            .filter(|(index, _)| !used.contains(index))
            .map(|(_, s)| {
                Finding::new(
                    "AUDIT-SUPPRESSION",
                    None,
                    &s.file,
                    s.line,
                    format!(
                        "`{}` suppression no longer matches a finding; remove it",
                        s.rule
                    ),
                )
            })
            .collect();
        findings.extend(
            self.malformed
                .iter()
                .map(|f| Finding::new(f.rule, f.cwe, &f.file, f.line, f.message.clone())),
        );
        findings
    }

    pub(crate) fn file(&self, path: &str) -> &str {
        self.text
            .iter()
            .find(|t| t.path == path)
            .map_or_else(|| panic!("{path} is missing"), |t| t.text.as_str())
    }

    pub(crate) fn read(&self, path: &str) -> String {
        std::fs::read_to_string(self.root.join(path)).unwrap_or_else(|e| panic!("{path}: {e}"))
    }
}

fn collect_text(root: &Path, dir: &Path, out: &mut Vec<TextFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !SKIP_DIRS.contains(&name.as_str()) {
                collect_text(root, &path, out);
            }
            continue;
        }
        let extension = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let is_text =
            TEXT_EXTENSIONS.contains(&extension) || name == "Cargo.lock" || name == ".gitignore";
        if !is_text {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let relative = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        out.push(TextFile {
            path: relative,
            text,
        });
    }
}

fn line_of(span: proc_macro2::Span) -> usize {
    span.start().line
}

fn is_test_attr(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let path = attr.path();
        if path.is_ident("test") {
            return true;
        }
        if path.is_ident("cfg")
            && let syn::Meta::List(list) = &attr.meta
        {
            // Only cfg(test) and cfg(all(test, ...)) are test-only; code under
            // cfg(any(target_os = "linux", test)) ships on Linux.
            let predicate: String = list
                .tokens
                .to_string()
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect();
            return predicate == "test"
                || predicate
                    .strip_prefix("all(")
                    .and_then(|rest| rest.strip_suffix(')'))
                    .is_some_and(|inner| inner.split(',').any(|part| part == "test"));
        }
        false
    })
}

fn tokens(expr: &impl quote::ToTokens) -> String {
    quote::ToTokens::to_token_stream(expr).to_string()
}

struct Walker {
    in_test: bool,
    context: Context,
    production: Vec<Event>,
    tests: Vec<Event>,
}

impl Walker {
    fn push(&mut self, event: Event) {
        if self.in_test {
            self.tests.push(event);
        } else {
            self.production.push(event);
        }
    }

    fn scoped(&mut self, test: bool, f: impl FnOnce(&mut Self)) {
        let previous = self.in_test;
        self.in_test |= test;
        f(self);
        self.in_test = previous;
    }

    fn with_context(&mut self, context: Context, f: impl FnOnce(&mut Self)) {
        let previous = self.context;
        self.context = context;
        f(self);
        self.context = previous;
    }

    /// Macro bodies are token streams: collect their literals and identifiers.
    fn walk_tokens(&mut self, stream: TokenStream) {
        for tree in stream {
            match tree {
                TokenTree::Group(group) => self.walk_tokens(group.stream()),
                TokenTree::Ident(ident) => self.push(Event::Ident {
                    line: line_of(ident.span()),
                    name: ident.to_string(),
                }),
                TokenTree::Literal(literal) => {
                    let line = line_of(literal.span());
                    if let Ok(string) =
                        syn::parse2::<syn::LitStr>(TokenTree::Literal(literal).into())
                    {
                        let context = self.context;
                        self.push(Event::Str {
                            line,
                            value: string.value(),
                            context,
                        });
                    }
                }
                TokenTree::Punct(_) => {}
            }
        }
    }
}

const LOG_MACROS: &[&str] = &["trace", "debug", "info", "warn", "error", "log"];
/// `anyhow` macros whose arguments are parsed as expressions.
const ERROR_MACROS: &[&str] = &["anyhow", "bail", "ensure"];
const ASSERT_MACROS: &[&str] = &[
    "assert",
    "assert_eq",
    "assert_ne",
    "debug_assert",
    "debug_assert_eq",
    "debug_assert_ne",
    "panic",
    "unreachable",
    "todo",
    "unimplemented",
];

impl<'ast> Visit<'ast> for Walker {
    fn visit_attribute(&mut self, attr: &'ast Attribute) {
        self.with_context(Context::Attribute, |w| {
            if let syn::Meta::List(list) = &attr.meta {
                w.walk_tokens(list.tokens.clone());
            } else {
                syn::visit::visit_attribute(w, attr);
            }
        });
    }

    fn visit_item_mod(&mut self, node: &'ast ItemMod) {
        self.scoped(is_test_attr(&node.attrs), |w| {
            syn::visit::visit_item_mod(w, node);
        });
    }

    fn visit_item_fn(&mut self, node: &'ast ItemFn) {
        self.scoped(is_test_attr(&node.attrs), |w| {
            if node.sig.unsafety.is_some() {
                w.push(Event::Unsafe {
                    line: line_of(node.sig.fn_token.span),
                    kind: "unsafe fn",
                });
            }
            syn::visit::visit_item_fn(w, node);
        });
    }

    fn visit_impl_item_fn(&mut self, node: &'ast ImplItemFn) {
        self.scoped(is_test_attr(&node.attrs), |w| {
            if node.sig.unsafety.is_some() {
                w.push(Event::Unsafe {
                    line: line_of(node.sig.fn_token.span),
                    kind: "unsafe fn",
                });
            }
            syn::visit::visit_impl_item_fn(w, node);
        });
    }

    fn visit_item_impl(&mut self, node: &'ast ItemImpl) {
        self.scoped(is_test_attr(&node.attrs), |w| {
            if let Some(token) = node.unsafety {
                w.push(Event::Unsafe {
                    line: line_of(token.span),
                    kind: "unsafe impl",
                });
            }
            syn::visit::visit_item_impl(w, node);
        });
    }

    fn visit_item_static(&mut self, node: &'ast ItemStatic) {
        if matches!(node.mutability, StaticMutability::Mut(_)) {
            self.push(Event::StaticMut {
                line: line_of(node.static_token.span),
            });
        }
        syn::visit::visit_item_static(self, node);
    }

    fn visit_expr_unsafe(&mut self, node: &'ast syn::ExprUnsafe) {
        self.push(Event::Unsafe {
            line: line_of(node.unsafe_token.span),
            kind: "unsafe block",
        });
        syn::visit::visit_expr_unsafe(self, node);
    }

    fn visit_expr_call(&mut self, node: &'ast ExprCall) {
        if let Expr::Path(path) = &*node.func {
            let text = path
                .path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect::<Vec<_>>()
                .join("::");
            let line = path
                .path
                .segments
                .first()
                .map_or(0, |s| line_of(s.ident.span()));
            let diagnostic = matches!(text.rsplit("::").next(), Some("diagnostic" | "invalid"))
                || text.ends_with("Error::other")
                || text.ends_with("Error::new");
            self.push(Event::Call {
                line,
                path: text,
                args: node.args.iter().map(tokens).collect(),
            });
            if diagnostic {
                self.visit_expr(&node.func);
                self.with_context(Context::Diagnostic, |w| {
                    node.args.iter().for_each(|arg| w.visit_expr(arg));
                });
                return;
            }
        }
        syn::visit::visit_expr_call(self, node);
    }

    fn visit_expr_method_call(&mut self, node: &'ast ExprMethodCall) {
        let name = node.method.to_string();
        self.push(Event::Method {
            line: line_of(node.method.span()),
            name: name.clone(),
            receiver: tokens(&node.receiver),
        });
        if name == "expect" {
            self.visit_expr(&node.receiver);
            self.with_context(Context::Assertion, |w| {
                for arg in &node.args {
                    w.visit_expr(arg);
                }
            });
            return;
        }
        syn::visit::visit_expr_method_call(self, node);
    }

    fn visit_expr_index(&mut self, node: &'ast ExprIndex) {
        self.push(Event::Index);
        syn::visit::visit_expr_index(self, node);
    }

    fn visit_macro(&mut self, node: &'ast Macro) {
        let name = node
            .path
            .segments
            .last()
            .map_or_default(|s| s.ident.to_string());
        let line = node
            .path
            .segments
            .last()
            .map_or(0, |s| line_of(s.ident.span()));
        self.push(Event::Macro {
            line,
            name: name.clone(),
            body: node.tokens.to_string(),
        });
        let context = if LOG_MACROS.contains(&name.as_str()) && node.path.segments.len() <= 2 {
            Context::Log
        } else if ASSERT_MACROS.contains(&name.as_str()) {
            Context::Assertion
        } else {
            self.context
        };
        // Error macros take expressions, so `bail!(diagnostic("..."))` is read
        // like `Err(diagnostic("..."))` instead of as loose tokens.
        if ERROR_MACROS.contains(&name.as_str())
            && let Ok(args) = node.parse_body_with(
                syn::punctuated::Punctuated::<Expr, syn::Token![,]>::parse_terminated,
            )
        {
            self.with_context(context, |w| args.iter().for_each(|arg| w.visit_expr(arg)));
            return;
        }
        self.with_context(context, |w| w.walk_tokens(node.tokens.clone()));
    }

    fn visit_lit_str(&mut self, node: &'ast syn::LitStr) {
        let context = self.context;
        self.push(Event::Str {
            line: line_of(node.span()),
            value: node.value(),
            context,
        });
    }

    fn visit_ident(&mut self, node: &'ast proc_macro2::Ident) {
        self.push(Event::Ident {
            line: line_of(node.span()),
            name: node.to_string(),
        });
    }
}
