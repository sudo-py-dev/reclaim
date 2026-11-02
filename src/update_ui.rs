use eframe::egui;
use reclaim::{
    i18n::LocalizedUi,
    updater::{self, Release},
};
use std::{
    path::PathBuf,
    sync::mpsc::{self, Receiver},
};

enum Message {
    Checked(Result<Option<Release>, String>),
    Installed(Result<(), String>),
    Progress(u64, u64),
    Opened(Result<(), String>),
    Downloaded(Result<Option<PathBuf>, String>),
}
#[derive(Default)]
pub struct Updates {
    started: bool,
    receiver: Option<Receiver<Message>>,
    release: Option<Release>,
    status: String,
    installed: bool,
    downloaded: Option<PathBuf>,
    progress: Option<(u64, u64)>,
}
impl Updates {
    fn check(&mut self, ctx: &egui::Context) {
        let (tx, rx) = mpsc::channel();
        self.receiver = Some(rx);
        self.status = "Checking for updates…".into();
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(Message::Checked(updater::check()));
            ctx.request_repaint();
        });
    }
    pub fn poll(&mut self, ctx: &egui::Context) {
        if !self.started {
            self.started = true;
            self.check(ctx);
        }
        loop {
            let message = self.receiver.as_ref().and_then(|rx| match rx.try_recv() {
                Ok(message) => Some(message),
                Err(mpsc::TryRecvError::Disconnected) => Some(Message::Installed(Err(
                    "Update worker stopped. Try again.".into(),
                ))),
                Err(mpsc::TryRecvError::Empty) => None,
            });
            let Some(message) = message else {
                break;
            };
            if let Message::Progress(done, total) = message {
                self.progress = Some((done, total));
                continue;
            }
            self.receiver = None;
            self.progress = None;
            match message {
                Message::Checked(Ok(release)) => {
                    self.status = if release.is_some() {
                        "An update is available."
                    } else {
                        "You’re up to date."
                    }
                    .into();
                    self.release = release;
                    self.downloaded = None;
                }
                Message::Installed(Ok(())) => {
                    self.installed = true;
                    self.status = "Update installed. Close and reopen Reclaim to use it.".into();
                }
                Message::Downloaded(Ok(Some(path))) => {
                    self.downloaded = Some(path);
                    self.status =
                        "Update downloaded and verified. Open the installer to finish updating."
                            .into();
                }
                Message::Opened(Ok(())) => {
                    self.status = "Installer opened. Close Reclaim to complete the update.".into()
                }
                Message::Downloaded(Ok(None)) => self.status = "Download cancelled.".into(),
                Message::Checked(Err(error))
                | Message::Installed(Err(error))
                | Message::Downloaded(Err(error))
                | Message::Opened(Err(error)) => self.status = error,
                Message::Progress(_, _) => unreachable!(),
            }
        }
    }
    pub fn available(&self) -> bool {
        self.release.is_some() && !self.installed
    }
    pub fn view(&mut self, ui: &mut egui::Ui, busy: bool) {
        ui.horizontal(|ui| {
            ui.t_strong("Updates");
            ui.weak(format!("Reclaim {}", env!("CARGO_PKG_VERSION")));
        });
        ui.t_label(&self.status);
        if let Some((done, total)) = self.progress {
            ui.add(egui::ProgressBar::new(done as f32 / total.max(1) as f32).show_percentage());
        } else if self.receiver.is_some() {
            ui.spinner();
        }
        if let Some(path) = &self.downloaded {
            ui.label(path.display().to_string());
            if ui
                .add_enabled(
                    !busy && self.receiver.is_none(),
                    egui::Button::new(reclaim::i18n::t("Open installer")),
                )
                .clicked()
            {
                let path = path.clone();
                let (tx, rx) = mpsc::channel();
                self.receiver = Some(rx);
                let ctx = ui.ctx().clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Message::Opened(crate::installer::open_installer(&path)));
                    ctx.request_repaint();
                });
            }
            ui.t_label("Close Reclaim before completing the installation.");
        }
        if let Some(release) = self.release.clone() {
            ui.with_layout(reclaim::i18n::row(), |ui| {
                ui.t_label("Available version");
                ui.label(release.version.to_string());
            });
            if busy {
                ui.t_label("Finish scanning or recovering before installing an update.");
            }
            let packaged = updater::installer_download(&release).is_some();
            let label = if packaged {
                "Download update"
            } else {
                "Install update"
            };
            if self.downloaded.is_none()
                && !self.installed
                && ui
                    .add_enabled(
                        !busy && self.receiver.is_none(),
                        crate::theme::primary(ui, label),
                    )
                    .clicked()
            {
                let (tx, rx) = mpsc::channel();
                self.receiver = Some(rx);
                self.status = "Downloading and verifying update…".into();
                let ctx = ui.ctx().clone();
                let release = release.clone();
                std::thread::spawn(move || {
                    if packaged {
                        let result = (|| {
                            let Some(path) = rfd::FileDialog::new()
                                .set_file_name(&release.asset)
                                .save_file()
                            else {
                                return Ok(None);
                            };
                            updater::download_installer(&release, &path, |done, total| {
                                let _ = tx.send(Message::Progress(done, total));
                                ctx.request_repaint();
                            })?;
                            Ok(Some(path))
                        })();
                        let _ = tx.send(Message::Downloaded(result));
                    } else {
                        let _ = tx.send(Message::Installed(updater::install(&release)));
                    }
                    ctx.request_repaint();
                });
            }
            crate::theme::disclosure(ui, "What’s new", |ui| {
                ui.label(&release.notes);
            });
        }
        if !self.installed
            && ui
                .add_enabled(
                    self.receiver.is_none(),
                    egui::Button::new(reclaim::i18n::t("Check for updates")),
                )
                .clicked()
        {
            self.check(ui.ctx());
        }
    }
}

#[cfg(test)]
impl Updates {
    pub(super) fn preview_available() -> Self {
        Self {
            started: true,
            status: "An update is available.".into(),
            release: Some(Release {
                version: semver::Version::new(99, 0, 0),
                asset: "reclaim_99.0.0_amd64.deb".into(),
                notes: "Improved updates and drive discovery.".into(),
                url: "https://github.com/sudo-py-dev/reclaim/releases/download/v99.0.0/reclaim_99.0.0_amd64.deb".into(),
                digest: "a".repeat(64), size: 1024,
            }),
            ..Default::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn progress_keeps_the_worker_and_verified_download_is_not_marked_installed() {
        let mut updates = Updates::preview_available();
        let (tx, rx) = mpsc::channel();
        updates.receiver = Some(rx);
        tx.send(Message::Progress(512, 1024)).unwrap();
        updates.poll(&egui::Context::default());
        assert_eq!(updates.progress, Some((512, 1024)));
        assert!(updates.receiver.is_some());
        tx.send(Message::Downloaded(Ok(Some(PathBuf::from(
            "/tmp/update.deb",
        )))))
        .unwrap();
        updates.poll(&egui::Context::default());
        assert!(updates.receiver.is_none());
        assert!(updates.progress.is_none());
        assert!(updates.downloaded.is_some());
        assert!(!updates.installed);
    }
}
