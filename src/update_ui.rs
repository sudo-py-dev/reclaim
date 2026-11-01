use eframe::egui;
use reclaim::{
    i18n::LocalizedUi,
    updater::{self, Release},
};
use std::sync::mpsc::{self, Receiver};

enum Message {
    Checked(Result<Option<Release>, String>),
    Installed(Result<(), String>),
}
#[derive(Default)]
pub struct Updates {
    started: bool,
    receiver: Option<Receiver<Message>>,
    release: Option<Release>,
    status: String,
    installed: bool,
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
        let result = self.receiver.as_ref().and_then(|rx| match rx.try_recv() {
            Ok(message) => Some(message),
            Err(mpsc::TryRecvError::Disconnected) => Some(Message::Installed(Err(
                "Update worker stopped. Try again.".into(),
            ))),
            Err(mpsc::TryRecvError::Empty) => None,
        });
        if let Some(message) = result {
            self.receiver = None;
            match message {
                Message::Checked(Ok(release)) => {
                    self.status = if release.is_some() {
                        "An update is available."
                    } else {
                        "You’re up to date."
                    }
                    .into();
                    self.release = release;
                }
                Message::Installed(Ok(())) => {
                    self.installed = true;
                    self.status = "Update installed. Close and reopen Reclaim to use it.".into();
                }
                Message::Checked(Err(error)) | Message::Installed(Err(error)) => {
                    self.status = error
                }
            }
        }
    }
    pub fn available(&self) -> bool {
        self.release.is_some() && !self.installed
    }
    pub fn view(&mut self, ui: &mut egui::Ui, busy: bool) {
        ui.separator();
        ui.t_strong("Updates");
        ui.label(format!("Reclaim {}", env!("CARGO_PKG_VERSION")));
        ui.t_label(&self.status);
        if self.receiver.is_some() {
            ui.spinner();
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
        if let Some(release) = &self.release {
            ui.label(format!("Reclaim {}", release.version));
            ui.t_strong("What’s new");
            // Plain text deliberately: release notes cannot execute HTML or commands.
            ui.label(&release.notes);
            if let Some(url) = updater::installer_download(release) {
                ui.t_label("Download the new installer, close Reclaim, then run it to update.");
                ui.hyperlink_to(reclaim::i18n::t("Download installer"), url);
                return;
            }
            if busy {
                ui.t_label("Finish scanning or recovering before installing an update.");
            }
            if ui
                .add_enabled(
                    !busy && !self.installed && self.receiver.is_none(),
                    egui::Button::new(reclaim::i18n::t("Install update")),
                )
                .clicked()
            {
                let release = release.clone();
                let (tx, rx) = mpsc::channel();
                self.receiver = Some(rx);
                self.status = "Downloading and verifying update…".into();
                let ctx = ui.ctx().clone();
                std::thread::spawn(move || {
                    let _ = tx.send(Message::Installed(updater::install(&release)));
                    ctx.request_repaint();
                });
            }
        }
    }
}
