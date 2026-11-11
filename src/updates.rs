//! Slint-independent updater state; all downloads and installation run on a worker.
use reclaim::updater::{self, Release};
use std::{
    io::{Read, Write},
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};
enum Message {
    Checked(Result<Option<Release>, String>),
    Progress(u64, u64),
    Phase(&'static str),
    Finished(Result<Option<PathBuf>, String>),
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
    pub notes: String,
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
        self.release
            .as_ref()
            .map(|release| {
                reclaim::i18n::message("Version {0} is available.", &[&release.version.to_string()])
            })
            .unwrap_or_default()
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
        if self.busy() {
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
            let _ = tx.send(Message::Checked(updater::check()));
        });
    }
    pub fn apply(&mut self) {
        if self.busy() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        if let Some(path) = self.downloaded.clone() {
            self.installing = true;
            self.notice_visible = false;
            self.receiver = Some(rx);
            std::thread::spawn(move || {
                let _ = tx.send(Message::Finished(
                    crate::installer::open_installer(&path).map(|()| None),
                ));
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
                let _ = tx.send(Message::Progress(done, total));
            };
            let result = if crate::update_runtime::supported() {
                crate::update_runtime::update(&release, progress, |phase| {
                    let _ = tx.send(Message::Phase(phase));
                })
                .map(|()| None)
            } else if updater::installer_download(&release).is_some() {
                crate::update_runtime::stage_installer(&release, progress).map(Some)
            } else {
                updater::install_with_progress(&release, progress).map(|()| None)
            };
            let _ = tx.send(Message::Finished(result));
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
                            self.status = release
                                .as_ref()
                                .map(|r| format!("Version {} is available.", r.version))
                                .unwrap_or_else(|| "You’re up to date.".into());
                            self.notes = release
                                .as_ref()
                                .map(|r| clean_notes(&r.notes))
                                .unwrap_or_default();
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
                Message::Progress(done, total) => self.progress = done as f32 / total.max(1) as f32,
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

/// Release notes are plain text, bounded before they reach the layout engine.
/// Remove Markdown heading/code markers without interpreting HTML or URLs.
fn clean_notes(notes: &str) -> String {
    if notes.trim() == "No release notes." {
        return String::new();
    }
    let bounded: String = notes.chars().take(6000).collect();
    bounded
        .lines()
        .take(40)
        .filter(|line| !line.trim_start().starts_with("```"))
        .map(|line| line.trim().trim_start_matches('#').trim().replace('`', ""))
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_owned()
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
    fn checked(updates: &mut Updates, result: Result<Option<Release>, String>) {
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
        checked(&mut updates, Err("offline".into()));
        assert!(updates.status.is_empty());
        assert!(!updates.notice_visible);
        updates.checked = true;
        updates.check_automatically();
        assert!(
            !updates.busy(),
            "a completed automatic check is not repeated"
        );
        updates.manual_check = true;
        checked(&mut updates, Err("offline".into()));
        assert!(!updates.status.is_empty());
    }
    #[test]
    fn release_notes_are_bounded_plain_text() {
        assert_eq!(
            clean_notes("## Fixes\n- Handle `files` safely\n```"),
            "Fixes\n- Handle files safely"
        );
        assert_eq!(clean_notes("No release notes."), "");
        assert!(clean_notes(&"א".repeat(10000)).chars().count() <= 6000);
        assert!(clean_notes(&"line\n".repeat(100)).lines().count() <= 40);
    }
}
