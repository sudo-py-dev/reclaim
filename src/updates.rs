//! Slint-independent updater state; all downloads and installation run on a worker.
// Hardened module: no unchecked indexing or arithmetic in shipped code (CWE-125, CWE-190).
#![cfg_attr(
    not(test),
    deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)
)]
use reclaim::updater::{self, Release};
use std::{
    fmt::Write as _,
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};
enum Message {
    Checked(anyhow::Result<Option<Release>>),
    Progress(u64, u64),
    Phase(&'static str),
    Finished(anyhow::Result<Option<PathBuf>>),
}
#[derive(Default)]
pub struct Updates {
    receiver: Option<Receiver<Message>>,
    release: Option<Release>,
    downloaded: Option<PathBuf>,
    checked: bool,
    manual_check: bool,
    installing: bool,
    dismissed: Option<semver::Version>,
    dismissal_path: Option<PathBuf>,
    pub notice_visible: bool,
    pub status: String,
    pub notes: Vec<NoteBlock>,
    pub progress: f32,
    pub restart: bool,
}
impl Updates {
    pub fn new() -> Self {
        let dismissal_path = crate::update_runtime::cache_root()
            .ok()
            .map(|folder| folder.join("dismissed-version"));
        Self::with_dismissal_path(dismissal_path)
    }
    fn with_dismissal_path(dismissal_path: Option<PathBuf>) -> Self {
        let dismissed = dismissal_path.as_ref().and_then(|path| {
            let mut value = String::new();
            std::fs::File::open(path)
                .ok()?
                .take(128)
                .read_to_string(&mut value)
                .ok()?;
            value.trim().parse().ok()
        });
        Self {
            dismissal_path,
            dismissed,
            status: if crate::update_runtime::snap_managed() {
                "Updates are managed by the Snap Store.".into()
            } else if crate::update_runtime::flatpak_managed() {
                "Updates are managed by Flatpak.".into()
            } else {
                String::new()
            },
            ..Self::default()
        }
    }
    pub fn installing(&self) -> bool {
        self.installing
    }
    pub fn check_automatically(&mut self) {
        if !self.checked {
            self.start_check(false);
        }
    }
    pub fn dismiss(&mut self) {
        self.notice_visible = false;
        let Some(release) = &self.release else {
            return;
        };
        self.dismissed = Some(release.version.clone());
        let Some(path) = &self.dismissal_path else {
            return;
        };
        let save = || -> std::io::Result<()> {
            let Some(folder) = path.parent() else {
                return Ok(());
            };
            let mut file = tempfile::NamedTempFile::new_in(folder)?;
            file.write_all(release.version.to_string().as_bytes())?;
            file.as_file().sync_all()?;
            file.persist(path).map_err(|error| error.error)?;
            Ok(())
        };
        if let Err(error) = save() {
            log::warn!("Could not save dismissed update version: {error}");
        }
    }
    pub fn title(&self) -> String {
        self.release.as_ref().map_or_default(|release| {
            reclaim::i18n::message("Version {0} is available.", &[&release.version.to_string()])
        })
    }
    pub fn busy(&self) -> bool {
        self.receiver.is_some()
    }
    pub fn available(&self) -> bool {
        self.release.is_some() || self.downloaded.is_some()
    }
    pub fn label(&self) -> &'static str {
        if self.downloaded.is_some() {
            "Open installer"
        } else if crate::update_runtime::supported() {
            "Update and restart"
        } else {
            "Download update"
        }
    }
    pub fn check(&mut self) {
        self.start_check(true);
    }
    fn start_check(&mut self, manual: bool) {
        if crate::update_runtime::package_managed() || self.busy() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.checked = true;
        self.manual_check = manual;
        self.receiver = Some(rx);
        if manual {
            self.status = "Checking for updates…".into();
        }
        std::thread::spawn(move || {
            tx.send(Message::Checked(updater::check())).ok();
        });
    }
    pub fn apply(&mut self) {
        if crate::update_runtime::package_managed() || self.busy() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        if let Some(path) = self.downloaded.clone() {
            self.installing = true;
            self.notice_visible = false;
            self.receiver = Some(rx);
            std::thread::spawn(move || {
                tx.send(Message::Finished(
                    crate::installer::open_installer(&path).map(|()| None),
                ))
                .ok();
            });
            return;
        }
        let Some(release) = self.release.clone() else {
            return;
        };
        self.status = "Preparing update…".into();
        self.progress = 0.0;
        self.installing = true;
        self.notice_visible = false;
        self.receiver = Some(rx);
        std::thread::spawn(move || {
            let progress = |done, total: u64| {
                tx.send(Message::Progress(done, total)).ok();
            };
            let result = if crate::update_runtime::supported() {
                crate::update_runtime::update(&release, progress, |phase| {
                    tx.send(Message::Phase(phase)).ok();
                })
                .map(|()| None)
            } else if updater::installer_download(&release).is_some() {
                crate::update_runtime::stage_installer(&release, progress).map(Some)
            } else {
                updater::install_with_progress(&release, progress).map(|()| None)
            };
            tx.send(Message::Finished(result)).ok();
        });
    }
    pub fn poll(&mut self) {
        while let Some(receiver) = &self.receiver {
            let message = match receiver.try_recv() {
                Ok(message) => message,
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.receiver = None;
                    if self.manual_check || self.installing {
                        self.status = "Update worker stopped. Try again.".into();
                    }
                    self.installing = false;
                    break;
                }
            };
            match message {
                Message::Checked(result) => {
                    self.receiver = None;
                    match result {
                        Ok(release) => {
                            if self.release.as_ref().map(|r| &r.version)
                                != release.as_ref().map(|r| &r.version)
                            {
                                self.downloaded = None;
                            }
                            self.notice_visible = release
                                .as_ref()
                                .is_some_and(|r| self.dismissed.as_ref() != Some(&r.version));
                            self.status = release.as_ref().map_or_else(
                                || "You’re up to date.".into(),
                                |r| format!("Version {} is available.", r.version),
                            );
                            self.notes =
                                release.as_ref().map_or_default(|r| format_notes(&r.notes));
                            self.release = release;
                        }
                        Err(e) => {
                            if self.manual_check {
                                self.status =
                                    reclaim::i18n::explain("Could not check for updates: {0}", e);
                            } else {
                                log::debug!("Background update check failed: {e}");
                            }
                        }
                    }
                }
                Message::Progress(done, total) => {
                    #[expect(
                        clippy::cast_precision_loss,
                        reason = "a progress bar fraction; lost low bits cannot show"
                    )]
                    let progress = done as f32 / total.max(1) as f32;
                    self.progress = progress;
                }
                Message::Phase(phase) => self.status = phase.into(),
                Message::Finished(result) => {
                    self.receiver = None;
                    self.installing = false;
                    match result {
                        Ok(Some(path)) => {
                            self.downloaded = Some(path);
                            self.status = "Update verified. Open the installer to finish.".into();
                        }
                        Ok(None) => {
                            self.restart = crate::update_runtime::supported();
                            self.status =
                                "Update finished. Close and reopen Reclaim if needed.".into();
                            self.release = None;
                        }
                        Err(e) => {
                            self.status =
                                reclaim::i18n::explain("The update could not be installed: {0}", e);
                        }
                    }
                }
            }
        }
    }
}

/// A bounded, non-interactive representation of the release Markdown.
#[derive(Clone, Debug, PartialEq)]
pub struct NoteBlock {
    pub text: String,
    // 0 = paragraph, 1 = heading, 2 = list item, 3 = code.
    pub kind: i32,
}

fn format_notes(notes: &str) -> Vec<NoteBlock> {
    use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

    if notes.trim() == "No release notes." {
        return Vec::new();
    }
    let bounded: String = notes
        .chars()
        .take(6000)
        .collect::<String>()
        .lines()
        .take(40)
        .collect::<Vec<_>>()
        .join("\n");
    let mut blocks = Vec::new();
    let mut text = String::new();
    let mut kind = 0;
    let mut lists: Vec<Option<u64>> = Vec::new();
    let flush = |blocks: &mut Vec<NoteBlock>, text: &mut String, kind| {
        let value = if kind == 2 || kind == 3 {
            text.trim_matches('\n').to_owned()
        } else {
            text.trim().to_owned()
        };
        if !value.is_empty() {
            blocks.push(NoteBlock { text: value, kind });
        }
        text.clear();
    };
    for event in Parser::new_ext(
        &bounded,
        Options::ENABLE_TASKLISTS | Options::ENABLE_STRIKETHROUGH,
    ) {
        match event {
            Event::Start(Tag::Heading { .. }) => {
                flush(&mut blocks, &mut text, kind);
                kind = 1;
            }
            Event::Start(Tag::CodeBlock(_)) => {
                flush(&mut blocks, &mut text, kind);
                kind = 3;
            }
            Event::Start(Tag::List(start)) => {
                flush(&mut blocks, &mut text, kind);
                lists.push(start);
            }
            Event::Start(Tag::Item) => {
                flush(&mut blocks, &mut text, kind);
                kind = 2;
                text.push_str(&"  ".repeat(lists.len().saturating_sub(1)));
                if let Some(Some(number)) = lists.last_mut() {
                    // Writing to a String cannot fail.
                    write!(text, "{number}. ").ok();
                    *number = number.saturating_add(1);
                } else {
                    text.push_str("• ");
                }
            }
            Event::End(
                TagEnd::Heading(_) | TagEnd::CodeBlock | TagEnd::Item | TagEnd::Paragraph,
            ) => {
                flush(&mut blocks, &mut text, kind);
                kind = if lists.is_empty() { 0 } else { 2 };
            }
            Event::End(TagEnd::List(_)) => {
                flush(&mut blocks, &mut text, kind);
                lists.pop();
                kind = if lists.is_empty() { 0 } else { 2 };
            }
            Event::Text(value)
            | Event::Code(value)
            | Event::Html(value)
            | Event::InlineHtml(value) => text.push_str(&value),
            Event::SoftBreak => text.push(' '),
            Event::HardBreak => text.push('\n'),
            Event::Rule => {
                flush(&mut blocks, &mut text, kind);
            }
            Event::TaskListMarker(checked) => text.push_str(if checked { "☑ " } else { "☐ " }),
            Event::Start(_)
            | Event::End(_)
            | Event::InlineMath(_)
            | Event::DisplayMath(_)
            | Event::FootnoteReference(_) => {}
        }
    }
    flush(&mut blocks, &mut text, kind);
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    fn release(version: &str) -> Release {
        Release {
            version: version.parse().unwrap(),
            asset: String::new(),
            notes: "## Improvements\n- Faster scanning".into(),
            url: String::new(),
            digest: String::new(),
            size: 0,
            signature_url: String::new(),
            signature: String::new(),
        }
    }
    fn checked(updates: &mut Updates, result: anyhow::Result<Option<Release>>) {
        let (tx, rx) = mpsc::channel();
        updates.receiver = Some(rx);
        tx.send(Message::Checked(result)).unwrap();
        updates.poll();
    }
    #[test]
    fn dismissal_hides_the_notice_but_keeps_manual_install_available() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("dismissed-version");
        let mut updates = Updates {
            dismissal_path: Some(path.clone()),
            ..Updates::default()
        };
        checked(&mut updates, Ok(Some(release("2.0.0"))));
        assert!(updates.notice_visible);
        assert!(!updates.installing());
        updates.dismiss();
        assert!(!updates.notice_visible);
        assert!(updates.available());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "2.0.0");
        let mut reopened = Updates::with_dismissal_path(Some(path));
        checked(&mut reopened, Ok(Some(release("2.0.0"))));
        assert!(!reopened.notice_visible);
        checked(&mut updates, Ok(Some(release("2.0.0"))));
        assert!(!updates.notice_visible);
        checked(&mut updates, Ok(Some(release("2.0.1"))));
        assert!(updates.notice_visible);
    }
    #[test]
    fn automatic_failures_are_quiet_and_checks_do_not_count_as_installation() {
        let mut updates = Updates::default();
        let (_tx, rx) = mpsc::channel();
        updates.receiver = Some(rx);
        assert!(updates.busy());
        assert!(!updates.installing());
        checked(&mut updates, Err(anyhow::anyhow!("offline")));
        assert_eq!(updates.status, "");
        assert!(!updates.notice_visible);
        updates.checked = true;
        updates.check_automatically();
        assert!(
            !updates.busy(),
            "a completed automatic check is not repeated"
        );
        updates.manual_check = true;
        checked(&mut updates, Err(anyhow::anyhow!("offline")));
        assert_ne!(updates.status, "");
    }
    #[test]
    fn release_notes_render_markdown_with_bounds() {
        let notes = format_notes(
            "## Fixes\n\n- Handle **files** safely\n- See [details](https://example.com)\n\n```rust\nlet x = 1;\n```",
        );
        assert_eq!(
            notes
                .iter()
                .map(|n| (n.kind, n.text.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (1, "Fixes"),
                (2, "• Handle files safely"),
                (2, "• See details"),
                (3, "let x = 1;")
            ]
        );
        assert_eq!(format_notes("No release notes."), Vec::<NoteBlock>::new());
        assert!(format_notes(&"א".repeat(10000))[0].text.chars().count() <= 6000);
        assert!(format_notes(&"line\n\n".repeat(100)).len() <= 40);
    }
}
