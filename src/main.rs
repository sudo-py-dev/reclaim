#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
mod drives;
mod theme;
mod window;
use eframe::egui;
use reclaim::i18n::{self, LocalizedResponse, LocalizedUi};
use reclaim::{Event, Found};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::Duration,
};
enum Message {
    Scan(Event),
    Opened(Arc<std::fs::File>),
    Done(Result<(), String>),
    Export(Result<usize, String>),
}
struct Reclaim {
    settings_open: bool,
    settings_error: Option<String>,
    source: Option<PathBuf>,
    source_handle: Option<Arc<std::fs::File>>,
    source_alignment: usize,
    drives: Vec<drives::Drive>,
    drive_receiver: Option<mpsc::Receiver<Result<Vec<drives::Drive>, String>>>,
    drives_loading: bool,
    drives_error: Option<String>,
    scanned_source: Option<PathBuf>,
    results: Vec<Found>,
    receiver: Option<mpsc::Receiver<Message>>,
    cancel: Arc<AtomicBool>,
    scanned: u64,
    total: u64,
    busy: bool,
    scan_stopped: bool,
    status: String,
    filter: String,
    page: usize,
    guide_return: usize,
    guide_step: usize,
    category: usize,
    exporting: bool,
    started: Option<std::time::Instant>,
    source_stamp: Option<(u64, Option<std::time::SystemTime>)>,
}
impl Default for Reclaim {
    fn default() -> Self {
        Self {
            settings_open: false,
            settings_error: None,
            source: None,
            source_handle: None,
            source_alignment: 1,
            drives: Vec::new(),
            drive_receiver: None,
            drives_loading: false,
            drives_error: None,
            scanned_source: None,
            results: vec![],
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            scanned: 0,
            total: 0,
            busy: false,
            scan_stopped: false,
            status: "Choose a drive or disk image to get started.".into(),
            filter: String::new(),
            page: 0,
            guide_return: 0,
            guide_step: 0,
            category: 0,
            exporting: false,
            started: None,
            source_stamp: None,
        }
    }
}
impl Reclaim {
    fn start(&mut self) {
        let Some(path) = self.source.clone() else {
            return;
        };
        self.results.clear();
        self.scan_stopped = false;
        self.page = 1;
        self.exporting = false;
        self.started = Some(std::time::Instant::now());
        self.source_stamp = path.metadata().ok().map(|m| (m.len(), m.modified().ok()));
        self.scanned = 0;
        self.total = self
            .drives
            .iter()
            .find(|d| d.path == path)
            .map(|d| d.size)
            .unwrap_or_else(|| path.metadata().map(|m| m.len()).unwrap_or(0));
        self.source_handle = None;
        self.source_alignment = self
            .drives
            .iter()
            .find(|d| d.path == path)
            .map(|d| d.sector_size)
            .unwrap_or_else(|| {
                if cfg!(target_os = "windows") && path.to_string_lossy().starts_with(r"\\.\") {
                    4096
                } else {
                    1
                }
            });
        let alignment = self.source_alignment;
        let source_size = (self.total > 0).then_some(self.total);
        self.scanned_source = Some(path.clone());
        self.busy = true;
        self.status =
            "Opening source… If Linux asks, authorize read-only access to the drive.".into();
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let (tx, rx) = mpsc::sync_channel(256);
        self.receiver = Some(rx);
        std::thread::spawn(move || {
            let result = (|| {
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let handle = Arc::new(drives::open_source(&path)?);
                if cancel.load(Ordering::Relaxed) {
                    return Ok(());
                }
                let mut input =
                    reclaim::SourceReader::new(handle.try_clone()?, alignment, source_size)?;
                tx.send(Message::Opened(handle))
                    .map_err(|_| std::io::Error::other("Window closed"))?;
                reclaim::scan_reader(&mut input, &cancel, |e| {
                    let _ = tx.send(Message::Scan(e));
                })
            })()
            .map_err(|e: std::io::Error| e.to_string());
            let _ = tx.send(Message::Done(result));
        });
    }
    fn visible_result_indices(&self) -> Vec<usize> {
        let query = self.filter.trim().to_lowercase();
        self.results
            .iter()
            .enumerate()
            .filter(|(i, f)| {
                let category = reclaim::category_index(f.kind);
                (self.category == 0 || category == self.category)
                    && format!(
                        "{} {} {:x} {}",
                        f.recovery_name(i + 1).to_lowercase(),
                        f.original_name.as_deref().unwrap_or("").to_lowercase(),
                        f.offset,
                        f.offset
                    )
                    .contains(&query)
            })
            .map(|(i, _)| i)
            .collect()
    }
    fn recovery_files(&self) -> Vec<(Found, usize)> {
        self.visible_result_indices()
            .into_iter()
            .filter(|&index| self.results[index].selected)
            .map(|index| (self.results[index].clone(), index + 1))
            .collect()
    }
    fn export(&mut self, directory: PathBuf) {
        let Some(source) = self.scanned_source.clone() else {
            return;
        };
        let files = self.recovery_files();
        if files.is_empty() {
            return;
        }
        let needs_source_bytes = files.iter().any(|(file, _)| file.resident_data.is_none());
        if needs_source_bytes
            && source.metadata().ok().map(|m| (m.len(), m.modified().ok())) != self.source_stamp
        {
            self.status = "Source changed since scanning. Scan it again before recovering.".into();
            return;
        }
        let Some(handle) = self.source_handle.clone() else {
            self.status = "Scan the selected source before recovering files.".into();
            return;
        };
        let (tx, rx) = mpsc::sync_channel(256);
        self.receiver = Some(rx);
        self.busy = true;
        self.status = "Recovering selected files…".into();
        self.exporting = true;
        self.cancel = Arc::new(AtomicBool::new(false));
        let cancel = self.cancel.clone();
        let alignment = self.source_alignment;
        let source_size = (self.total > 0).then_some(self.total);
        std::thread::spawn(move || {
            let result = (|| {
                let input = handle
                    .try_clone()
                    .map_err(|e| format!("Could not read the scanned source: {e}"))?;
                let mut input = reclaim::SourceReader::new(input, alignment, source_size)
                    .map_err(|e| e.to_string())?;
                let mut count = 0;
                for (f, number) in files {
                    if cancel.load(Ordering::Relaxed) {
                        return Err(format!("Recovery stopped. {count} files saved."));
                    }
                    reclaim::recover_unique(&mut input, &directory, &f, number)
                        .map_err(|e| format!("Recovered {count} files before error: {e}"))?;
                    count += 1;
                }
                Ok(count)
            })();
            let _ = tx.send(Message::Export(result));
        });
    }
}
impl Reclaim {
    fn refresh_drives(&mut self) {
        if self.drives_loading || !drives::AVAILABLE {
            return;
        }
        self.drives_loading = true;
        self.drives_error = None;
        let (tx, rx) = mpsc::channel();
        self.drive_receiver = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(drives::discover());
        });
    }
    fn poll(&mut self) {
        if let Some(result) = self
            .drive_receiver
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
        {
            self.drives_loading = false;
            self.drive_receiver = None;
            match result {
                Ok(drives) => {
                    let was_drive = self
                        .source
                        .as_ref()
                        .is_some_and(|p| self.drives.iter().any(|d| &d.path == p));
                    if was_drive && !drives.iter().any(|d| Some(&d.path) == self.source.as_ref()) {
                        self.source = None;
                    }
                    self.drives = drives;
                }
                Err(e) => self.drives_error = Some(e),
            }
        }
        if let Some(rx) = &self.receiver {
            for message in rx.try_iter().take(512) {
                match message {
                    Message::Opened(handle) => {
                        self.source_handle = Some(handle);
                        self.status = "Scanning for recoverable files…".into();
                    }
                    Message::Scan(Event::Found(f)) => self.results.push(f),
                    Message::Scan(Event::Progress(n)) => self.scanned = n,
                    Message::Done(result) => {
                        self.busy = false;
                        self.scan_stopped = self.cancel.load(Ordering::Relaxed);
                        self.status = match result {
                            Ok(()) if self.cancel.load(Ordering::Relaxed) => {
                                "Scan stopped. You can recover partial results.".into()
                            }
                            Ok(()) => {
                                self.scanned = self.total.max(self.scanned);
                                format!(
                                    "Scan complete. {} file candidates found.",
                                    self.results.len()
                                )
                            }
                            Err(e) => format!("Scan failed: {e}"),
                        };
                    }
                    Message::Export(result) => {
                        self.busy = false;
                        self.exporting = false;
                        self.status = match result {
                            Ok(n) => format!(
                                "Saved {n} files. Open the destination folder and verify their contents."
                            ),
                            Err(e) => e,
                        };
                    }
                }
            }
        }
    }
    fn choose_source(&mut self) {
        if let Some(path) = rfd::FileDialog::new()
            .set_title(i18n::t("Choose a raw disk image"))
            .pick_file()
        {
            self.source = Some(path);
        }
    }
    fn source_view(&mut self, ui: &mut egui::Ui) {
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 60.0), i18n::row(), |ui| {
            theme::recovery_emblem(ui, self.busy || self.drives_loading);
            let width = (ui.available_width() - 8.0).max(80.0);
            ui.allocate_ui_with_layout(egui::vec2(width, 56.0), i18n::column(), |ui| {
                ui.t_heading(egui::RichText::new("Recover Files").strong());
                ui.t_label(if drives::AVAILABLE {
                    "Choose the drive that contained your files."
                } else {
                    "Open a disk image to look for deleted files."
                });
            });
        });
        ui.add_space(12.0);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
            i18n::row(),
            |ui| {
                ui.t_strong(if drives::AVAILABLE {
                    "Available Drives"
                } else {
                    "Disk Image"
                });
                ui.with_layout(i18n::trailing_row(), |ui| {
                    if drives::AVAILABLE
                        && ui
                            .add_enabled_ui(!self.busy && !self.drives_loading, |ui| {
                                theme::icon_button(
                                    ui,
                                    theme::Icon::Refresh,
                                    "Refresh",
                                    false,
                                    egui::vec2(36.0, 36.0),
                                )
                            })
                            .inner
                            .clicked()
                    {
                        self.refresh_drives();
                    }
                });
            },
        );
        // Reserve the same compact safety area in every source-selection state.
        // Selecting a mounted drive or finishing a scan must not collapse the list.
        let list_height = (ui.available_height() - 152.0).max(68.0);
        theme::card(ui).inner_margin(8.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if drives::AVAILABLE {
                if self.drives_loading {
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), ui.spacing().interact_size.y), i18n::row(), |ui| { if !self.busy { ui.spinner(); } ui.t_label("Looking for drives…"); });
                } else if let Some(error) = &self.drives_error { ui.t_label(error); }
                else if self.drives.is_empty() { ui.t_label("No drives found. Connect a drive and select Refresh, or open a disk image below."); }
                else {
                    egui::ScrollArea::vertical()
                        .id_salt("drive_list")
                        .max_height(list_height)
                        .auto_shrink([false, true])
                        .show(ui, |ui| {
                        ui.with_layout(i18n::column(), |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for drive in &self.drives {
                            ui.push_id(&drive.path, |ui| {
                                let subtitle = format!("{} · {}", i18n::t(&drive.kind), i18n::t(&drive.location));
                                if theme::drive_row(ui, &drive.display_name(), &subtitle, &theme::bytes(drive.size), self.source.as_ref() == Some(&drive.path), !self.busy && !drive.encrypted).on_hover_text(drive.path.display().to_string()).clicked() {
                                    self.source = Some(drive.path.clone());
                                }
                            });
                        }
                        });
                    });
                }
                ui.separator();
            }
            if ui.add_enabled(!self.busy, egui::Button::new(i18n::widget(ui, "Open a Disk Image…", false)).frame(false).min_size(egui::vec2(ui.available_width(), 38.0))).clicked() { self.choose_source(); }
            if let Some(path) = &self.source && !self.drives.iter().any(|d| d.path == *path) {
                ui.add(egui::Label::new(egui::RichText::new(path.file_name().unwrap_or(path.as_os_str()).to_string_lossy()).strong()).truncate()).on_hover_text(path.display().to_string());
            }
        });
        ui.add_space(12.0);
        let mounted = self
            .source
            .as_ref()
            .and_then(|path| self.drives.iter().find(|drive| drive.path == *path))
            .is_some_and(|drive| drive.mounted);
        theme::card(ui).inner_margin(12.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.with_layout(i18n::column(), |ui| {
                ui.t_strong(if mounted {
                    "Drive in Use"
                } else {
                    "Read-only recovery"
                });
                ui.t_label(if mounted {
                    "For best results, stop using this drive or scan it from another computer."
                } else {
                    "Read-only access may require permission from your operating system."
                });
            });
        });
    }
    fn results_view(&mut self, ui: &mut egui::Ui) {
        if self.busy && !self.exporting && !self.results.is_empty() {
            ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 56.0), i18n::row(), |ui| {
                theme::scan_radar(ui, 52.0);
                ui.t_strong("Scanning for recoverable files…");
            });
            ui.add_space(4.0);
        }
        if let Some(p) = &self.scanned_source {
            let name = self
                .drives
                .iter()
                .find(|d| d.path == *p)
                .map(|d| d.display_name())
                .unwrap_or_else(|| {
                    p.file_name()
                        .unwrap_or(p.as_os_str())
                        .to_string_lossy()
                        .into_owned()
                });
            ui.add(
                egui::Label::new(i18n::raw_widget(
                    ui,
                    &name,
                    egui::TextStyle::Body.resolve(ui.style()),
                    ui.visuals().strong_text_color(),
                    ui.available_width(),
                ))
                .truncate(),
            )
            .on_hover_text(p.display().to_string());
        }
        ui.add_space(4.0);
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
            i18n::row(),
            |ui| {
                const HEIGHT: f32 = 40.0;
                const CATEGORY_WIDTH: f32 = 156.0;
                ui.spacing_mut().interact_size.y = HEIGHT;
                let width =
                    (ui.available_width() - CATEGORY_WIDTH - ui.spacing().item_spacing.x).max(80.0);
                ui.add_sized(
                    [width, HEIGHT],
                    egui::TextEdit::singleline(&mut self.filter)
                        .font(egui::TextStyle::Body)
                        .hint_text(i18n::t("Search files…"))
                        .horizontal_align(if i18n::is_rtl() {
                            egui::Align::Max
                        } else {
                            egui::Align::Min
                        })
                        .desired_width(width)
                        .min_size(egui::vec2(0.0, HEIGHT))
                        .margin(egui::Margin::symmetric(12, 0))
                        .vertical_align(egui::Align::Center),
                )
                .t_on_hover_text("Search by filename, file extension, or source offset")
                .widget_info(|| {
                    egui::WidgetInfo::labeled(
                        egui::WidgetType::TextEdit,
                        true,
                        i18n::t("Search files"),
                    )
                });
                theme::category_dropdown(ui, &mut self.category, CATEGORY_WIDTH);
            },
        );
        let visible = self.visible_result_indices();
        ui.allocate_ui_with_layout(
            egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
            i18n::row(),
            |ui| {
                ui.spacing_mut().item_spacing.x = 16.0;
                ui.set_min_height(36.0);
                if ui
                    .add_enabled(!visible.is_empty(), |ui: &mut egui::Ui| {
                        theme::icon_button(
                            ui,
                            theme::Icon::SelectAll,
                            "Select all shown",
                            false,
                            egui::vec2(36.0, 36.0),
                        )
                    })
                    .clicked()
                {
                    for i in &visible {
                        self.results[*i].selected = true;
                    }
                }
                if ui
                    .add_enabled(
                        self.results.iter().any(|f| f.selected),
                        |ui: &mut egui::Ui| {
                            theme::icon_button(
                                ui,
                                theme::Icon::DeselectAll,
                                "Deselect all",
                                false,
                                egui::vec2(36.0, 36.0),
                            )
                        },
                    )
                    .clicked()
                {
                    for f in &mut self.results {
                        f.selected = false;
                    }
                }
                ui.with_layout(i18n::trailing_row(), |ui| {
                    ui.t_weak(format!("{} files", visible.len()));
                });
            },
        );
        let height = (ui.available_height() - 8.0).max(100.0);
        theme::card(ui).inner_margin(8.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if visible.is_empty() {
                let scanning_empty = self.busy && !self.exporting && self.results.is_empty();
                let title = if scanning_empty {
                    if self.source_handle.is_some() {
                        "Scanning for files…"
                    } else {
                        "Preparing scan"
                    }
                } else if self.results.is_empty() && self.scan_stopped {
                    "Scan stopped"
                } else if self.results.is_empty() {
                    "No files found"
                } else {
                    "No matching files"
                };
                let description = if scanning_empty {
                    if self.source_handle.is_some() {
                        "Files will appear here as they are found."
                    } else {
                        "The scan will start when the source is ready."
                    }
                } else if self.results.is_empty() && self.scan_stopped {
                    "No files were found before stopping. You can scan this source again."
                } else if self.results.is_empty() {
                    "Try another drive or disk image, or check the recovery guide."
                } else {
                    "Try another search or file type."
                };
                if theme::empty_state(
                    ui,
                    title,
                    description,
                    height - 16.0,
                    !self.filter.is_empty() || self.category != 0,
                    scanning_empty,
                ) {
                    self.filter.clear();
                    self.category = 0;
                }
            } else {
                egui::ScrollArea::vertical()
                    .id_salt("results")
                    .max_height(height - 16.0)
                    .auto_shrink([false, false])
                    .show_rows(ui, 54.0, visible.len(), |ui, range| {
                        for row in range {
                            let f = &mut self.results[visible[row]];
                            ui.push_id(f.offset, |ui| {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                                    i18n::row(),
                                    |ui| {
                                        let filename_width =
                                            (ui.available_width() - 100.0).max(140.0);
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(filename_width, 54.0),
                                            i18n::column(),
                                            |ui| {
                                                ui.set_min_size(egui::vec2(filename_width, 54.0));
                                                ui.spacing_mut().item_spacing.y = 2.0;
                                                ui.style_mut().wrap_mode =
                                                    Some(egui::TextWrapMode::Truncate);
                                                let name = f.recovery_name(visible[row] + 1);
                                                let origin = f
                                                    .original_name
                                                    .as_ref()
                                                    .map(|name| {
                                                        format!("Filename from NTFS record: {name}")
                                                    })
                                                    .unwrap_or_else(|| "Generated filename".into());
                                                let label = i18n::raw_widget(
                                                    ui,
                                                    &name,
                                                    egui::TextStyle::Body.resolve(ui.style()),
                                                    ui.visuals().text_color(),
                                                    filename_width - 24.0,
                                                );
                                                i18n::checkbox(ui, &mut f.selected, label)
                                                .t_on_hover_text(format!(
                                                    "{origin} · Source offset: {} bytes (0x{:x})",
                                                    f.offset, f.offset
                                                ));
                                                ui.t_label(
                                                    egui::RichText::new(format!(
                                                        "{} · {}",
                                                        f.kind.to_uppercase(),
                                                        i18n::t(f.validation.label())
                                                    ))
                                                    .small()
                                                    .weak(),
                                                );
                                            },
                                        );
                                        ui.with_layout(i18n::trailing_row(), |ui| {
                                            ui.t_weak(theme::bytes(f.size));
                                        });
                                    },
                                );
                            });
                        }
                    });
            }
        });
    }
    fn guide_view(&mut self, ui: &mut egui::Ui) {
        let steps = [
            (
                "Protect your data",
                "Stop using the affected drive.",
                "Avoid downloads, updates, or installing software on it. Connect it to another computer if possible.",
                "New downloads, updates, and installing recovery software can overwrite deleted data. If possible, connect the drive to another computer.",
            ),
            (
                "Choose your source",
                "Select the drive where your files were stored.",
                "You can also open a disk image. Unlock encrypted volumes before scanning.",
                "Select the drive or volume where your files were stored, then choose Scan Drive. Linux can request read-only access through desktop authentication. Windows raw drives require administrator access; macOS may require Full Disk Access and administrator privileges. You can open a raw disk image instead. Encrypted volumes must be unlocked first.",
            ),
            (
                "Scan and review",
                "Start a scan and review the files as they appear.",
                "Use the type and search filters to narrow the list. Check the files you want to keep.",
                "The scanner finds pictures, documents, ZIP packages, audio, and single-segment AVI video, up to 64 MiB each. Office, OpenDocument, EPUB, JAR, and APK extensions are inferred from ZIP metadata. A signature match does not guarantee a usable file or prove that it was deleted.",
            ),
            (
                "Save and verify",
                "Recover selected files to another physical drive.",
                "Only selected files matching your current filters are saved. Open the recovered files to check their contents.",
                "Select candidates and save them to another physical drive. Existing files are never replaced. Open recovered files and check their contents.",
            ),
        ];
        ui.vertical_centered(|ui| {
            ui.t_heading("Recovery guide");
            ui.t_weak("A few careful steps can preserve more of your data.");
        });
        ui.add_space(20.0);
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 40.0), i18n::row(), |ui| {
            let width = (ui.available_width() - 3.0 * ui.spacing().item_spacing.x) / 4.0;
            for (index, (title, _, _, _)) in steps.iter().enumerate() {
                let response = ui.add_sized(
                    [width, 36.0],
                    egui::Button::new(format!("{:02}", index + 1))
                        .selected(self.guide_step == index),
                );
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::RadioButton,
                        true,
                        self.guide_step == index,
                        i18n::t(title),
                    )
                });
                if response.t_on_hover_text(*title).clicked() {
                    self.guide_step = index;
                }
            }
        });
        ui.add_space(12.0);
        let (title, summary, body, details) = steps[self.guide_step];
        theme::card(ui).inner_margin(24.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.with_layout(i18n::column(), |ui| {
                ui.t_label(
                    egui::RichText::new(format!("{} / 4", self.guide_step + 1))
                        .small()
                        .color(theme::ACCENT),
                );
                ui.t_heading(title);
                ui.add_space(8.0);
                ui.t_strong(summary);
                ui.t_label(body);
                ui.add_space(12.0);
                ui.separator();
                theme::disclosure(ui, "More details", |ui| {
                    ui.t_label(details);
                    if self.guide_step >= 2 {
                        ui.add_space(12.0);
                        ui.t_strong("About file names");
                        ui.t_label("Small deleted NTFS files can retain their original names and contents inside an MFT record. When a checked record is available, its filename is used. Other results are found by file signatures and receive generated names such as Image 0001.png. Original folders are not reconstructed. Names are made portable when saving, and duplicate names get a numeric suffix.");
                    }
                });
            });
        });
        ui.add_space(16.0);
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 40.0), i18n::row(), |ui| {
            if ui
                .add_enabled_ui(self.guide_step > 0, |ui| {
                    theme::icon_button(
                        ui,
                        theme::Icon::Back,
                        "Previous step",
                        false,
                        egui::vec2(40.0, 40.0),
                    )
                })
                .inner
                .clicked()
            {
                self.guide_step -= 1;
            }
            ui.with_layout(i18n::trailing_row(), |ui| {
                let label = if self.guide_step == 3 {
                    "Choose a drive"
                } else {
                    "Next step"
                };
                if ui
                    .add(theme::primary(ui, label).min_size(egui::vec2(140.0, 40.0)))
                    .clicked()
                {
                    if self.guide_step == 3 {
                        self.page = 0;
                    } else {
                        self.guide_step += 1;
                    }
                }
            });
        });
        ui.add_space(20.0);
        theme::disclosure(ui, "Know the limits", |ui| {
            ui.t_label("Original filenames are available only for supported deleted NTFS resident files. Larger NTFS files, original folders, general fragmented-file reconstruction, FAT/exFAT, APFS, and ext4 metadata recovery are not supported. Overwritten, TRIM-erased, or encrypted data without keys cannot be recovered by this scanner.");
        });
        ui.add_space(12.0);
        ui.t_weak("Reclaim 0.1 · MIT license · Built with Rust and egui");
    }
}
impl Reclaim {
    fn settings_view(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut done = false;
        let modal = egui::Modal::new(egui::Id::new("settings_dialog"))
            .frame(egui::Frame::popup(&ctx.global_style()).inner_margin(20.0).corner_radius(14))
            .show(ctx, |ui| {
                ui.set_width((ctx.viewport_rect().width() - 80.0).clamp(260.0, 440.0));
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                ui.with_layout(i18n::column(), |ui| {
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 48.0), i18n::row(), |ui| {
                        let (rect, _) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 10, theme::ACCENT.gamma_multiply(0.15));
                        theme::paint_symbol(ui, rect, theme::Icon::Settings, 19.0, ui.visuals().strong_text_color());
                        ui.t_heading("Settings");
                        ui.with_layout(i18n::trailing_row(), |ui| {
                            done |= theme::icon_button(ui, theme::Icon::Close, "Close", false, egui::vec2(32.0, 32.0)).clicked();
                        });
                    });
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical()
                        .id_salt("settings_scroll")
                        .auto_shrink([false, true])
                        .max_height((ctx.viewport_rect().height() - 184.0).max(160.0))
                        .show(ui, |ui| {
                            ui.with_layout(i18n::column(), |ui| {
                                theme::card(ui).fill(ui.visuals().extreme_bg_color).inner_margin(16.0).show(ui, |ui| {
                                    ui.with_layout(i18n::column(), |ui| {
                                        ui.t_strong("Appearance");
                                        let mut preference = ctx.options(|options| options.theme_preference);
                                        let previous = preference;
                                        theme::setting_choices(ui, "appearance", &mut preference, &[
                                            (egui::ThemePreference::System, "System"),
                                            (egui::ThemePreference::Light, "Light"),
                                            (egui::ThemePreference::Dark, "Dark"),
                                        ]);
                                        if preference != previous { ctx.set_theme(preference); }
                                        ui.add_space(4.0);
                                        ui.separator();
                                        ui.add_space(4.0);
                                        ui.t_strong("Language");
                                        let previous = i18n::preference();
                                        let mut language = previous;
                                        let width = ui.available_width();
                                        theme::choice_dropdown(ui, "language", &mut language, &[
                                            (i18n::Preference::System, "System"),
                                            (i18n::Preference::English, "English"),
                                            (i18n::Preference::Hebrew, "Hebrew"),
                                        ], width);
                                        if language != previous {
                                            self.settings_error = i18n::set_preference(language).err().map(|e| format!("Language preference could not be saved: {e}"));
                                            i18n::setup_fonts(ctx);
                                            ctx.send_viewport_cmd(egui::ViewportCommand::Title(i18n::t("Reclaim — File Recovery")));
                                            ctx.request_repaint();
                                        }
                                        ui.add_space(4.0);
                                        ui.separator();
                                        ui.add_space(4.0);
                                        let mut animations = ui.style().animation_time > 0.0;
                                        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 48.0), i18n::row(), |ui| {
                                            let width = (ui.available_width() - 64.0).max(100.0);
                                            ui.allocate_ui_with_layout(egui::vec2(width, 48.0), i18n::column(), |ui| {
                                                ui.set_min_width(width);
                                                ui.t_strong("Animations");
                                                ui.t_label(egui::RichText::new("Smooth selection and loading effects.").small().weak());
                                            });
                                            if theme::switch(ui, &mut animations, "Enable animations").changed() {
                                                for theme in [egui::Theme::Light, egui::Theme::Dark] {
                                                    let mut style = (*ctx.style_of(theme)).clone();
                                                    style.animation_time = if animations { 0.18 } else { 0.0 };
                                                    ctx.set_style_of(theme, style);
                                                }
                                            }
                                        });
                                    });
                                });
                                ui.add_space(4.0);
                                theme::disclosure(ui, "Supported files", |ui| {
                                    ui.t_label(egui::RichText::new(reclaim::FORMAT_SUMMARY).small());
                                });
                                if let Some(error) = &self.settings_error { ui.t_label(error); }
                            });
                        });
                    ui.add_space(12.0);
                    ui.separator();
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 40.0), i18n::row(), |ui| {
                        if theme::icon_button(ui, theme::Icon::Guide, "Recovery guide", false, egui::vec2(40.0, 40.0)).clicked() {
                            if self.page != 2 { self.guide_return = self.page; }
                            self.page = 2;
                            done = true;
                        }
                        ui.with_layout(i18n::trailing_row(), |ui| {
                            done |= theme::icon_button(ui, theme::Icon::Check, "Done", true, egui::vec2(52.0, 40.0)).clicked();
                        });
                    });
                });
            });
        if done || modal.should_close() {
            self.settings_open = false;
        }
    }
    fn render(&mut self, ui: &mut egui::Ui) {
        self.poll();
        if self.busy || self.drives_loading {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        egui::Panel::top("header")
            .frame(
                egui::Frame::new()
                    .fill(theme::header_color(ui))
                    .inner_margin(egui::Margin::symmetric(12, 8)),
            )
            .show(ui, |ui| {
                let rect = egui::Rect::from_min_size(
                    ui.cursor().min,
                    egui::vec2(ui.available_width(), 36.0),
                );
                window::title(
                    ui,
                    rect,
                    ["Reclaim", "Scan results", "Recovery guide"][self.page],
                );
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                    i18n::row(),
                    |ui| {
                        ui.set_min_height(36.0);
                        if self.page != 0
                            && theme::icon_button(
                                ui,
                                theme::Icon::Back,
                                "Back",
                                false,
                                egui::vec2(36.0, 36.0),
                            )
                            .clicked()
                        {
                            self.page = if self.page == 2 { self.guide_return } else { 0 };
                        }
                        ui.with_layout(i18n::trailing_row(), |ui| {
                            if window::CUSTOM_DECORATIONS {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(108.0, 32.0),
                                    i18n::row(),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = 6.0;
                                        window::controls(ui);
                                    },
                                );
                            }
                            if theme::icon_button(
                                ui,
                                theme::Icon::Settings,
                                "Settings",
                                false,
                                egui::vec2(36.0, 36.0),
                            )
                            .clicked()
                            {
                                self.settings_open = true;
                            }
                        });
                    },
                );
            });
        if self.page == 0 {
            egui::Panel::bottom("scan_bar")
                .frame(
                    egui::Frame::new()
                        .fill(theme::header_color(ui))
                        .inner_margin(egui::Margin::symmetric(24, 12)),
                )
                .show(ui, |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                        i18n::row(),
                        |ui| {
                            let has_results = !self.busy && self.scanned_source.is_some();
                            let label_width = (ui.available_width()
                                - 188.0
                                - if has_results { 44.0 } else { 0.0 })
                            .max(80.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(label_width, 38.0),
                                i18n::row(),
                                |ui| {
                                    ui.set_min_width(label_width);
                                    let name = self.source.as_ref().map(|p| {
                                        self.drives
                                            .iter()
                                            .find(|d| d.path == *p)
                                            .map(|d| d.display_name())
                                            .unwrap_or_else(|| {
                                                p.file_name()
                                                    .unwrap_or(p.as_os_str())
                                                    .to_string_lossy()
                                                    .into_owned()
                                            })
                                    });
                                    let label = if let Some(name) = name {
                                        i18n::raw_widget(
                                            ui,
                                            &name,
                                            egui::TextStyle::Body.resolve(ui.style()),
                                            ui.visuals().strong_text_color(),
                                            label_width,
                                        )
                                    } else {
                                        i18n::widget(
                                            ui,
                                            egui::RichText::new("Select a drive above").strong(),
                                            true,
                                        )
                                    };
                                    ui.add(egui::Label::new(label).truncate());
                                },
                            );
                            ui.with_layout(i18n::trailing_row(), |ui| {
                                let label = if self.busy {
                                    "View Scan"
                                } else if self.source.is_none() {
                                    "Scan"
                                } else if self
                                    .source
                                    .as_ref()
                                    .is_some_and(|p| self.drives.iter().any(|d| &d.path == p))
                                {
                                    "Scan Drive"
                                } else {
                                    "Scan Image"
                                };
                                if ui
                                    .add_enabled(
                                        self.busy || self.source.is_some(),
                                        theme::primary(ui, label).min_size(egui::vec2(164.0, 38.0)),
                                    )
                                    .clicked()
                                {
                                    if self.busy {
                                        self.page = 1;
                                    } else {
                                        self.start();
                                    }
                                }
                                if has_results
                                    && theme::icon_button(
                                        ui,
                                        theme::Icon::Results,
                                        "View Scan Results",
                                        false,
                                        egui::vec2(36.0, 38.0),
                                    )
                                    .clicked()
                                {
                                    self.page = 1;
                                }
                            });
                        },
                    );
                });
        }
        if self.page == 1 {
            egui::Panel::bottom("recover_bar")
                .frame(
                    egui::Frame::new()
                        .fill(theme::header_color(ui))
                        .inner_margin(egui::Margin::symmetric(24, 12)),
                )
                .show(ui, |ui| {
                    let count = self
                        .visible_result_indices()
                        .into_iter()
                        .filter(|&index| self.results[index].selected)
                        .count();
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                        i18n::row(),
                        |ui| {
                            ui.allocate_ui_with_layout(
                                egui::vec2((ui.available_width() - 172.0).max(80.0), 44.0),
                                i18n::column(),
                                |ui| {
                                    ui.set_min_width((ui.available_width()).max(80.0));
                                    ui.t_strong(format!("{count} selected"));
                                    ui.t_label(
                                        egui::RichText::new("Save to another physical drive")
                                            .small()
                                            .weak(),
                                    );
                                },
                            );
                            ui.with_layout(i18n::trailing_row(), |ui| {
                                if ui
                                    .add_enabled(
                                        !self.busy && count > 0,
                                        theme::primary(ui, "Recover files…")
                                            .min_size(egui::vec2(144.0, 38.0)),
                                    )
                                    .clicked()
                                    && let Some(dir) = rfd::FileDialog::new()
                                        .set_title(i18n::t("Save to a different physical drive"))
                                        .pick_folder()
                                {
                                    self.export(dir);
                                }
                            });
                        },
                    );
                });
        }
        if self.busy || (self.page == 1 && self.scanned_source.is_some()) {
            egui::Panel::bottom("status")
                .frame(
                    egui::Frame::new()
                        .fill(ui.visuals().panel_fill)
                        .inner_margin(egui::Margin::symmetric(24, 10)),
                )
                .show(ui, |ui| {
                    let determinate = self.busy
                        && !self.exporting
                        && self.total > 0
                        && self.source_handle.is_some();
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                        i18n::row().with_main_wrap(true),
                        |ui| {
                            if self.busy && !determinate {
                                ui.spinner();
                            }
                            ui.t_label(&self.status);
                            if self.busy
                                && ui
                                    .t_button(if self.exporting {
                                        "Stop after this file"
                                    } else {
                                        "Stop scan"
                                    })
                                    .clicked()
                            {
                                self.cancel.store(true, Ordering::Relaxed);
                                self.status = if self.exporting {
                                    "Stopping recovery…"
                                } else {
                                    "Stopping scan…"
                                }
                                .into();
                            }
                        },
                    );
                    if self.busy && !self.exporting && self.source_handle.is_some() {
                        let fraction = if self.total > 0 {
                            (self.scanned as f64 / self.total as f64).clamp(0.0, 1.0)
                        } else {
                            0.0
                        };
                        if determinate {
                            ui.add(
                                egui::ProgressBar::new(fraction as f32)
                                    .desired_width(ui.available_width())
                                    .desired_height(6.0)
                                    .corner_radius(3)
                                    .animate(false),
                            );
                        }
                        let seconds = self
                            .started
                            .map(|t| t.elapsed().as_secs_f64())
                            .unwrap_or(0.0)
                            .max(0.01);
                        ui.allocate_ui_with_layout(
                            egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                            i18n::row(),
                            |ui| {
                                ui.t_label(
                                    egui::RichText::new(format!(
                                        "{} scanned · {}/s",
                                        theme::bytes(self.scanned),
                                        theme::bytes((self.scanned as f64 / seconds) as u64)
                                    ))
                                    .small()
                                    .weak(),
                                );
                                if determinate {
                                    ui.with_layout(i18n::trailing_row(), |ui| {
                                        ui.t_label(
                                            egui::RichText::new(format!(
                                                "{:.1}%",
                                                fraction * 100.0
                                            ))
                                            .small()
                                            .weak(),
                                        );
                                    });
                                }
                            },
                        );
                    }
                });
        }
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(ui.visuals().panel_fill)
                    .inner_margin(24.0),
            )
            .show(ui, |ui| {
                let max_width = match self.page {
                    0 => 1120.0,
                    1 => 1280.0,
                    _ => 800.0,
                };
                let width = ui.available_width().min(max_width);
                let inset = ((ui.available_width() - width) / 2.0).max(0.0);
                ui.with_layout(i18n::row().with_cross_align(egui::Align::Min), |ui| {
                    ui.add_space(inset);
                    ui.allocate_ui_with_layout(
                        egui::vec2(width, ui.available_height()),
                        i18n::column(),
                        |ui| {
                            if self.page == 1 {
                                self.results_view(ui);
                            } else if self.page == 0 {
                                ui.add_space(8.0);
                                self.source_view(ui);
                            } else {
                                egui::ScrollArea::vertical()
                                    .id_salt(("page_scroll", self.page))
                                    .auto_shrink([false, false])
                                    .show(ui, |ui| {
                                        ui.add_space(20.0);
                                        self.guide_view(ui);
                                    });
                            }
                        },
                    );
                });
            });
        window::resize(ui);
        self.settings_view(ui.ctx());
    }
}
impl eframe::App for Reclaim {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render(ui);
    }
}
impl Drop for Reclaim {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
fn main() -> eframe::Result {
    i18n::init();
    eframe::run_native(
        &i18n::t("Reclaim — File Recovery"),
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_decorations(!window::CUSTOM_DECORATIONS)
                .with_app_id("io.github.reclaim.Reclaim")
                .with_inner_size([1120.0, 780.0])
                .with_min_inner_size([540.0, 580.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            theme::setup(&cc.egui_ctx);
            let mut app = Reclaim::default();
            app.refresh_drives();
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod ui_tests;
