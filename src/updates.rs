//! Slint-independent updater state; all downloads and installation run on a worker.
use reclaim::updater::{self, Release};
use std::{
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
    pub status: String,
    pub notes: String,
    pub progress: f32,
    pub restart: bool,
}
impl Updates {
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
        if self.busy() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.status = "Checking for updates…".into();
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
                (|| {
                    let Some(path) = rfd::FileDialog::new()
                        .set_file_name(&release.asset)
                        .save_file()
                    else {
                        return Ok(None);
                    };
                    updater::download_installer(&release, &path, progress)?;
                    Ok(Some(path))
                })()
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
                    self.status = "Update worker stopped. Try again.".into();
                    break;
                }
            };
            match message {
                Message::Checked(result) => {
                    self.receiver = None;
                    match result {
                        Ok(release) => {
                            self.status = release
                                .as_ref()
                                .map(|r| format!("Version {} is available.", r.version))
                                .unwrap_or_else(|| "You’re up to date.".into());
                            self.notes = release
                                .as_ref()
                                .map(|r| r.notes.clone())
                                .unwrap_or_default();
                            self.release = release;
                        }
                        Err(e) => self.status = e,
                    }
                }
                Message::Progress(done, total) => self.progress = done as f32 / total.max(1) as f32,
                Message::Phase(phase) => self.status = phase.into(),
                Message::Finished(result) => {
                    self.receiver = None;
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
                        Err(e) => self.status = e,
                    }
                }
            }
        }
    }
}
