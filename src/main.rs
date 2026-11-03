#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]
mod diagnostics;
mod drives;
mod installer;
#[cfg(any(target_os = "linux", windows))]
mod native_process;
mod preview;
mod theme;
mod update_runtime;
mod update_ui;
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
    updates: update_ui::Updates,
    system_theme: Option<theme::SystemThemeWatcher>,
    details_file: Option<usize>,
    thumbnails: std::collections::HashMap<usize, Option<egui::TextureHandle>>,
    thumbnail_receiver: Option<(usize, mpsc::Receiver<Result<egui::ColorImage, String>>)>,
    settings_open: bool,
    settings_page: usize,
    settings_error: Option<String>,
    link_receiver: Option<mpsc::Receiver<Result<(), String>>>,
    source: Option<PathBuf>,
    source_handle: Option<Arc<std::fs::File>>,
    source_alignment: usize,
    drives: Vec<drives::Drive>,
    drive_monitor: Option<drives::DriveMonitor>,
    drive_receiver: Option<mpsc::Receiver<Result<Vec<drives::Drive>, String>>>,
    drives_loading: bool,
    drives_error: Option<String>,
    scanned_source: Option<PathBuf>,
    results: Vec<Found>,
    receiver: Option<mpsc::Receiver<Message>>,
    cancel: Arc<AtomicBool>,
    scan_activity: Option<(u64, &'static str, usize, std::time::Instant)>,
    last_scan_update: Option<std::time::Instant>,
    scan_check_summary: Option<(&'static str, usize)>,
    unused_scan: bool,
    allocation_ready: bool,
    scanned: u64,
    total: u64,
    source_size: Option<u64>,
    busy: bool,
    scan_stopped: bool,
    scan_error: Option<String>,
    status: String,
    filter: String,
    page: usize,
    guide_return: usize,
    guide_step: usize,
    category: usize,
    exporting: bool,
    started: Option<std::time::Instant>,
    source_stamp: Option<(u64, Option<std::time::SystemTime>)>,
    recovery_folder: Option<(PathBuf, drives::Destination)>,
}
impl Default for Reclaim {
    fn default() -> Self {
        Self {
            updates: update_ui::Updates::default(),
            system_theme: None,
            details_file: None,
            thumbnails: std::collections::HashMap::new(),
            thumbnail_receiver: None,
            settings_open: false,
            settings_page: 0,
            settings_error: None,
            link_receiver: None,
            source: None,
            source_handle: None,
            source_alignment: 1,
            drives: Vec::new(),
            drive_monitor: None,
            drive_receiver: None,
            drives_loading: false,
            drives_error: None,
            scanned_source: None,
            results: vec![],
            receiver: None,
            cancel: Arc::new(AtomicBool::new(false)),
            scan_activity: None,
            last_scan_update: None,
            scan_check_summary: None,
            unused_scan: false,
            allocation_ready: true,
            scanned: 0,
            total: 0,
            source_size: None,
            busy: false,
            scan_stopped: false,
            scan_error: None,
            status: "Choose a drive or disk image to get started.".into(),
            filter: String::new(),
            page: 0,
            guide_return: 0,
            guide_step: 0,
            category: 0,
            exporting: false,
            started: None,
            source_stamp: None,
            recovery_folder: None,
        }
    }
}
impl Reclaim {
    fn start(&mut self) {
        let Some(path) = self.source.clone() else {
            return;
        };
        self.thumbnails.clear();
        self.thumbnail_receiver = None;
        self.results.clear();
        self.details_file = None;
        self.scan_stopped = false;
        self.scan_error = None;
        self.page = 1;
        self.exporting = false;
        self.started = Some(std::time::Instant::now());
        self.source_stamp = path.metadata().ok().map(|m| (m.len(), m.modified().ok()));
        self.scan_activity = None;
        self.scan_check_summary = None;
        self.last_scan_update = Some(std::time::Instant::now());
        self.unused_scan = true;
        self.allocation_ready = false;
        self.scanned = 0;
        self.total = 0;
        let source_size = self
            .drives
            .iter()
            .find(|d| d.path == path)
            .map(|d| d.size)
            .unwrap_or_else(|| path.metadata().map(|m| m.len()).unwrap_or(0));
        self.source_size = (source_size > 0).then_some(source_size);
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
        let source_size = self.source_size;
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
                let size = source_size.ok_or_else(|| {
                    std::io::Error::other(
                        "The volume size is unknown; unused-space scanning cannot start.",
                    )
                })?;
                reclaim::scan_unused_reader(&mut input, size, &cancel, |e| {
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
                    && (query.is_empty()
                        || format!(
                            "{} {} {:x} {}",
                            f.recovery_name(i + 1),
                            f.original_name.as_deref().unwrap_or(""),
                            f.offset,
                            f.offset
                        )
                        .to_lowercase()
                        .contains(&query))
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
    fn choose_recovery_folder(&mut self) {
        let Some(directory) = rfd::FileDialog::new()
            .set_title(i18n::t("Save to a different physical drive"))
            .pick_folder()
        else {
            return;
        };
        self.review_recovery_folder(directory);
    }
    /// Saves straight away only when the folder provably cannot overwrite the
    /// scanned data; otherwise the destination dialog explains the risk.
    fn review_recovery_folder(&mut self, directory: PathBuf) {
        let Some(source) = &self.scanned_source else {
            return;
        };
        match drives::destination(source, &directory) {
            drives::Destination::Separate => self.export(directory),
            destination => self.recovery_folder = Some((directory, destination)),
        }
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
        let source_size = self.source_size;
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
        if self
            .drive_monitor
            .as_ref()
            .is_some_and(|monitor| monitor.request_refresh())
        {
            return;
        }
        let (tx, rx) = mpsc::channel();
        self.drive_receiver = Some(rx);
        std::thread::spawn(move || {
            let _ = tx.send(drives::discover());
        });
    }
    fn apply_drive_update(&mut self, result: Result<Vec<drives::Drive>, String>) {
        self.drives_loading = false;
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
                self.drives_error = None;
            }
            Err(e) => self.drives_error = Some(e),
        }
    }
    fn poll(&mut self) {
        if let Some(result) = self
            .drive_monitor
            .as_mut()
            .and_then(|monitor| monitor.take_update())
        {
            self.apply_drive_update(result);
        }
        if let Some(result) = self
            .drive_receiver
            .as_ref()
            .and_then(|rx| rx.try_recv().ok())
        {
            self.drive_receiver = None;
            self.apply_drive_update(result);
        }
        if let Some(rx) = &self.receiver {
            for message in rx.try_iter().take(512) {
                match message {
                    Message::Opened(handle) => {
                        self.source_handle = Some(handle);
                        self.status = if self.unused_scan {
                            "Reading unused-space map…"
                        } else {
                            "Scanning for recoverable files…"
                        }
                        .into();
                    }
                    Message::Scan(Event::UnusedSpace { bytes, .. }) => {
                        self.unused_scan = true;
                        self.allocation_ready = true;
                        self.total = bytes;
                        self.scanned = 0;
                        self.last_scan_update = Some(std::time::Instant::now());
                    }
                    Message::Scan(Event::Found(f)) => self.results.push(f),
                    Message::Scan(Event::Progress(n)) => {
                        if !self.unused_scan || n != self.scanned {
                            self.scan_activity = None;
                        }
                        self.last_scan_update = Some(std::time::Instant::now());
                        self.scanned = n;
                        if !self.cancel.load(Ordering::Relaxed) {
                            self.status = "Scanning for recoverable files…".into();
                        }
                    }
                    Message::Scan(Event::Checking {
                        offset,
                        kind,
                        bytes,
                    }) => {
                        let now = std::time::Instant::now();
                        let since = self
                            .scan_activity
                            .filter(|(at, previous, _, _)| *at == offset && *previous == kind)
                            .map(|(_, _, _, since)| since)
                            .unwrap_or(now);
                        self.scan_activity = Some((offset, kind, bytes, since));
                        self.last_scan_update = Some(now);
                        if !self.unused_scan {
                            self.scanned = self.scanned.max(offset);
                        }
                        if !self.cancel.load(Ordering::Relaxed) {
                            self.status = format!(
                                "{} · {} · {} · {}",
                                i18n::t("Checking file contents…"),
                                kind.to_uppercase(),
                                theme::bytes(bytes as u64),
                                theme::bytes(offset)
                            );
                        }
                    }
                    Message::Done(result) => {
                        self.scan_activity = None;
                        self.busy = false;
                        self.scan_stopped = self.cancel.load(Ordering::Relaxed);
                        self.status = match result {
                            Ok(()) if self.cancel.load(Ordering::Relaxed) => {
                                "Scan stopped. You can recover partial results.".into()
                            }
                            Ok(())
                                if self.unused_scan && self.allocation_ready && self.total == 0 =>
                            {
                                "No unused space is available on this volume.".into()
                            }
                            Ok(()) => {
                                self.scanned = self.total.max(self.scanned);
                                format!(
                                    "Scan complete. {} file candidates found.",
                                    self.results.len()
                                )
                            }
                            Err(e) => {
                                self.scan_error = Some(e.clone());
                                "Scan failed. See the message above.".into()
                            }
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
                    if ui
                        .add_enabled_ui(!self.busy, |ui| {
                            theme::icon_text_button(
                                ui,
                                theme::Icon::DiskImage,
                                "Open a Disk Image…",
                                false,
                                egui::vec2(176.0, 36.0),
                            )
                        })
                        .inner
                        .clicked()
                    {
                        self.choose_source();
                    }
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
        // Keep one compact toolbar below the list in every selection state.
        let list_height = (ui.available_height() - 64.0).max(68.0);
        theme::card(ui).inner_margin(8.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if drives::AVAILABLE {
                if self.drives_loading && self.drives.is_empty() {
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), ui.spacing().interact_size.y), i18n::row(), |ui| { if !self.busy { ui.spinner(); } ui.t_label("Looking for drives…"); });
                } else if let Some(error) = &self.drives_error && self.drives.is_empty() { ui.t_label(error); }
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
                                if theme::drive_row(ui, &drive.display_name(), &subtitle, &drive.kind, &theme::bytes(drive.size), self.source.as_ref() == Some(&drive.path), !self.busy && !drive.encrypted).raw_on_hover_text(format!("{}\n{}\n{} · {}", drive.display_name(), subtitle, theme::bytes(drive.size), drive.path.display())).clicked() {
                                    self.source = Some(drive.path.clone());
                                }
                            });
                        }
                        });
                    });
                }
            }
        });
        let mounted = self
            .source
            .as_ref()
            .and_then(|path| self.drives.iter().find(|drive| drive.path == *path))
            .is_some_and(|drive| drive.mounted);
        let message = if mounted {
            "For best results, stop using this drive or scan it from another computer."
        } else {
            "Read-only access may require permission from your operating system."
        };
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 40.0), i18n::row(), |ui| {
            let (rect, response) =
                ui.allocate_exact_size(egui::vec2(28.0, 28.0), egui::Sense::hover());
            theme::paint_symbol(
                ui,
                rect,
                if mounted {
                    theme::Icon::Warning
                } else {
                    theme::Icon::Shield
                },
                15.0,
                ui.visuals().weak_text_color(),
            );
            response.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Label, true, i18n::t(message))
            });
            ui.add(
                egui::Label::new(i18n::widget(
                    ui,
                    egui::RichText::new(message).small().weak(),
                    true,
                ))
                .wrap(),
            );
        });
    }
    fn stop_operation(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
        self.status = if self.exporting {
            "Stopping recovery…"
        } else {
            "Stopping scan…"
        }
        .into();
    }
    fn thumbnail(&mut self, index: usize, ctx: &egui::Context) -> Option<egui::TextureHandle> {
        if let Some(texture) = self.thumbnails.get(&index) {
            return texture.clone();
        }
        let file = &self.results[index];
        if self.thumbnail_receiver.is_none()
            && preview::supported(file.kind)
            && (file.resident_data.is_some() || self.source_handle.is_some())
        {
            let found = file.clone();
            let handle = self.source_handle.clone();
            let alignment = self.source_alignment;
            let (tx, rx) = mpsc::channel();
            self.thumbnail_receiver = Some((index, rx));
            let ctx = ctx.clone();
            std::thread::spawn(move || {
                let _ = tx.send(preview::load(&found, handle, alignment));
                ctx.request_repaint();
            });
        }
        None
    }

    fn results_view(&mut self, ui: &mut egui::Ui) {
        if let Some((index, receiver)) = &self.thumbnail_receiver {
            let result = match receiver.try_recv() {
                Ok(result) => Some(result.ok()),
                Err(mpsc::TryRecvError::Disconnected) => Some(None),
                Err(mpsc::TryRecvError::Empty) => None,
            };
            if let Some(image) = result {
                let texture = image.map(|image| {
                    ui.ctx().load_texture(
                        format!("thumbnail_{index}"),
                        image,
                        egui::TextureOptions::LINEAR,
                    )
                });
                self.thumbnails.insert(*index, texture);
                self.thumbnail_receiver = None;
            }
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
            .raw_on_hover_text(p.display().to_string());
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
                    ui.t_weak(if visible.len() == 1 {
                        "1 file".into()
                    } else {
                        format!("{} files", visible.len())
                    });
                });
            },
        );
        let mut inspect = None;
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
                } else if self.results.is_empty() && self.scan_error.is_some() {
                    "Could not scan this drive"
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
                } else if self.results.is_empty() && self.scan_error.is_some() {
                    self.scan_error.as_deref().unwrap_or_default()
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
                        // Visible indices are sorted in source-result order.
                        // Evict once per frame rather than once for every row.
                        if self.thumbnails.len() > 256 {
                            self.thumbnails.retain(|index, _| {
                                visible[range.clone()].binary_search(index).is_ok()
                            });
                        }
                        for row in range {
                            let thumbnail = self.thumbnail(visible[row], ui.ctx());
                            let f = &mut self.results[visible[row]];
                            ui.push_id(visible[row], |ui| {
                                ui.allocate_ui_with_layout(
                                    egui::vec2(ui.available_width(), ui.spacing().interact_size.y),
                                    i18n::row(),
                                    |ui| {
                                        if let Some(texture) = &thumbnail {
                                            ui.allocate_ui_with_layout(
                                                egui::vec2(40.0, 40.0),
                                                egui::Layout::centered_and_justified(
                                                    egui::Direction::LeftToRight,
                                                ),
                                                |ui| {
                                                    ui.add(
                                                        egui::Image::new(texture)
                                                            .fit_to_exact_size(egui::vec2(
                                                                40.0, 40.0,
                                                            ))
                                                            .maintain_aspect_ratio(true)
                                                            .alt_text(i18n::t("Image preview")),
                                                    );
                                                },
                                            );
                                        } else {
                                            theme::file_badge(ui, f.kind);
                                        }
                                        let filename_width =
                                            (ui.available_width() - 140.0).max(120.0);
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
                                            if theme::icon_button(
                                                ui,
                                                theme::Icon::Info,
                                                "File details",
                                                false,
                                                egui::vec2(32.0, 36.0),
                                            )
                                            .clicked()
                                            {
                                                inspect = Some(visible[row]);
                                            }
                                            ui.t_weak(theme::bytes(f.size));
                                        });
                                    },
                                );
                            });
                        }
                    });
            }
        });
        if let Some(index) = inspect {
            self.details_file = Some(index);
        }
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
        ui.t_strong("Recovery steps");
        ui.t_weak("A few careful steps can preserve more of your data.");
        ui.add_space(8.0);
        let titles = steps.map(|(title, _, _, _)| title);
        if let Some(step) = theme::guide_steps(ui, &titles, self.guide_step) {
            self.guide_step = step;
        }
        ui.add_space(12.0);
        let (title, summary, body, details) = steps[self.guide_step];
        theme::card(ui).inner_margin(24.0).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.with_layout(i18n::column(), |ui| {
                ui.t_label(
                    egui::RichText::new(i18n::t(&format!(
                        "Step {} of {}",
                        self.guide_step + 1,
                        steps.len()
                    )))
                        .small()
                        .color(theme::ACCENT),
                );
                ui.t_heading(title);
                ui.add_space(8.0);
                let body_height = ui.available_height().max(80.0);
                egui::ScrollArea::vertical()
                    .id_salt(("guide_content", self.guide_step))
                    .max_height(body_height)
                    .auto_shrink([false, true])
                    .show(ui, |ui| {
                        ui.with_layout(i18n::column(), |ui| {
                            ui.t_strong(summary);
                            ui.t_label(body);
                            ui.add_space(8.0);
                            ui.separator();
                            theme::disclosure(ui, "More details", |ui| {
                                ui.t_label(details);
                                if self.guide_step >= 2 {
                                    ui.add_space(12.0);
                                    ui.t_strong("About file names");
                                    ui.t_label("Small deleted NTFS files can retain their original names and contents inside an MFT record. When a checked record is available, its filename is used. Other results are found by file signatures and receive generated names such as Image 0001.png. Original folders are not reconstructed. Names are made portable when saving, and duplicate names get a numeric suffix.");
                                }
                            });
                            theme::disclosure(ui, "Know the limits", |ui| {
                                ui.t_label("Original filenames are available only for supported deleted NTFS resident files. Larger NTFS files, original folders, general fragmented-file reconstruction, FAT/exFAT, APFS, and ext4 metadata recovery are not supported. Overwritten, TRIM-erased, or encrypted data without keys cannot be recovered by this scanner.");
                            });
                        });
                    });
            });
        });
    }
    fn guide_navigation(&mut self, ui: &mut egui::Ui) {
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
    }
}
impl Reclaim {
    fn file_details_view(&mut self, ctx: &egui::Context) {
        let Some(index) = self.details_file else {
            return;
        };
        let Some(file) = self.results.get(index) else {
            self.details_file = None;
            return;
        };
        let mut close = false;
        let modal = egui::Modal::new(egui::Id::new("file_details"))
            .frame(egui::Frame::popup(&ctx.global_style()).inner_margin(20.0).corner_radius(14))
            .show(ctx, |ui| {
                ui.set_width((ctx.viewport_rect().width() - 80.0).clamp(260.0, 440.0));
                ui.with_layout(i18n::column(), |ui| {
                    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 40.0), i18n::row(), |ui| {
                        theme::file_badge(ui, file.kind);
                        ui.t_heading("File details");
                        ui.with_layout(i18n::trailing_row(), |ui| { close = theme::icon_button(ui, theme::Icon::Close, "Close", false, egui::vec2(32.0,32.0)).clicked(); });
                    });
                    let name = file.recovery_name(index + 1);
                    ui.add(egui::Label::new(i18n::raw_widget(ui, &name, egui::FontId::proportional(16.0), ui.visuals().strong_text_color(), ui.available_width())).wrap());
                    ui.add_space(8.0);
                    egui::ScrollArea::vertical().id_salt("file_details_scroll").max_height((ctx.viewport_rect().height() - 200.0).max(160.0)).auto_shrink([false, true]).show(ui, |ui| {
                        ui.with_layout(i18n::column(), |ui| {
                            theme::detail_row(ui, "File type", &file.kind.to_uppercase());
                            theme::detail_row(ui, "Size", &format!("{} ({} {})", theme::bytes(file.size), file.size, i18n::t("bytes")));
                            theme::detail_row(ui, "Recovery check", &i18n::t(file.validation.label()));
                            let deleted = file.ntfs.is_some();
                            theme::detail_row(ui, "Deletion status", &i18n::t(if deleted { "Deleted NTFS record" } else { "Deletion not confirmed" }));
                            theme::detail_row(ui, "File name", &i18n::t(if file.original_name.is_some() { "Original name recovered" } else { "Generated filename" }));
                            ui.separator();
                            if let Some(metadata) = &file.ntfs {
                                for (label, value) in [("Created", metadata.created), ("Last modified", metadata.modified), ("Last accessed", metadata.accessed), ("Record changed", metadata.record_changed)] {
                                    theme::detail_row(ui, label, &theme::file_date(value));
                                }
                                theme::detail_row(ui, "NTFS record", &format!("{} · {} {}", metadata.record_number, i18n::t("Sequence"), metadata.sequence_number));
                            } else { ui.t_label("Dates are not available for files found by signature scanning."); }
                            theme::detail_row(ui, "Deletion date", &i18n::t("Not available"));
                            ui.t_label(egui::RichText::new("File dates do not indicate when a file was deleted. This scanner does not recover deletion timestamps.").small().weak());
                            ui.separator();
                            if let Some(source) = &self.scanned_source { theme::detail_row(ui, "Source", &source.display().to_string()); }
                            theme::detail_row(ui, "Source offset", &format!("{} {} · 0x{:X}", file.offset, i18n::t("bytes"), file.offset));
                            ui.t_label(egui::RichText::new("Recovery checks do not guarantee complete or usable contents. Open saved files to verify them.").small().weak());
                        });
                    });
                });
            });
        if close || modal.should_close() {
            self.details_file = None;
        }
    }
    fn recovery_folder_view(&mut self, ctx: &egui::Context) {
        enum Choice {
            Save,
            Another,
            Cancel,
        }
        let Some((directory, destination)) = self.recovery_folder.clone() else {
            return;
        };
        // A proven match leaves nothing for the user to confirm, so saving is
        // offered only when the drive could not be identified.
        let same = destination == drives::Destination::SameDrive;
        let mut choice = None;
        let modal = egui::Modal::new(egui::Id::new("recovery_folder"))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .inner_margin(20.0)
                    .corner_radius(14),
            )
            .show(ctx, |ui| {
                ui.set_width((ctx.viewport_rect().width() - 80.0).clamp(260.0, 440.0));
                ui.with_layout(i18n::column(), |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 40.0),
                        i18n::row(),
                        |ui| {
                            let (rect, _) =
                                ui.allocate_exact_size(egui::vec2(32.0, 32.0), egui::Sense::hover());
                            theme::paint_symbol(
                                ui,
                                rect,
                                theme::Icon::Warning,
                                20.0,
                                ui.visuals().warn_fg_color,
                            );
                            ui.t_heading(if same {
                                "This folder is on the drive you scanned"
                            } else {
                                "Protect your recoverable data"
                            });
                        },
                    );
                    ui.t_label(if same {
                        "Saving here can overwrite the deleted files you are trying to recover and damage them permanently. Choose a folder on a different physical drive, such as a USB drive."
                    } else {
                        "Saving recovered files to the same physical drive you scanned can overwrite and permanently damage data you are trying to recover. Choose a folder on a different physical drive. Continue only if this destination is on another drive."
                    });
                    theme::detail_row(ui, "Destination", &directory.display().to_string());
                    ui.add_space(8.0);
                    ui.with_layout(i18n::trailing_row(), |ui| {
                        if ui
                            .add(theme::primary(ui, "Choose another folder").min_size(egui::vec2(0.0, 36.0)))
                            .clicked()
                        {
                            choice = Some(Choice::Another);
                        }
                        if !same && ui.t_button("Save files").clicked() {
                            choice = Some(Choice::Save);
                        }
                        if ui.t_button("Cancel").clicked() {
                            choice = Some(Choice::Cancel);
                        }
                    });
                });
            });
        if choice.is_none() && modal.should_close() {
            choice = Some(Choice::Cancel);
        }
        let Some(choice) = choice else {
            return;
        };
        self.recovery_folder = None;
        match choice {
            Choice::Save => self.export(directory),
            Choice::Another => self.choose_recovery_folder(),
            Choice::Cancel => {}
        }
    }
    fn open_link(&mut self, ctx: &egui::Context, url: &'static str) {
        if self.link_receiver.is_some() {
            return;
        }
        self.settings_error = None;
        let (tx, rx) = mpsc::channel();
        self.link_receiver = Some(rx);
        let ctx = ctx.clone();
        std::thread::spawn(move || {
            let _ = tx.send(installer::open_url(url));
            ctx.request_repaint();
        });
    }
    fn general_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.t_strong("Appearance & language");
        theme::card(ui)
            .fill(ui.visuals().extreme_bg_color)
            .inner_margin(20.0)
            .show(ui, |ui| {
                ui.with_layout(i18n::column(), |ui| {
                    theme::preference_row(ui, "Appearance", |ui| {
                        let mut preference = ctx.options(|options| options.theme_preference);
                        let previous = preference;
                        theme::setting_choices(
                            ui,
                            "appearance",
                            &mut preference,
                            &[
                                (egui::ThemePreference::System, "System"),
                                (egui::ThemePreference::Light, "Light"),
                                (egui::ThemePreference::Dark, "Dark"),
                            ],
                        );
                        if preference != previous {
                            ctx.set_theme(preference);
                        }
                    });
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    theme::preference_row(ui, "Language", |ui| {
                        let previous = i18n::preference();
                        let mut language = previous;
                        let width = ui.available_width();
                        theme::choice_dropdown(
                            ui,
                            "language",
                            &mut language,
                            &[
                                (i18n::Preference::System, "System"),
                                (i18n::Preference::English, "English"),
                                (i18n::Preference::Hebrew, "Hebrew"),
                            ],
                            width,
                        );
                        if language != previous {
                            self.settings_error = i18n::set_preference(language)
                                .err()
                                .map(|e| format!("Language preference could not be saved: {e}"));
                            i18n::setup_fonts(ctx);
                            ctx.send_viewport_cmd(egui::ViewportCommand::Title(i18n::t(
                                "Reclaim — File Recovery",
                            )));
                            ctx.request_repaint();
                        }
                    });
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    let mut animations = ui.style().animation_time > 0.0;
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 48.0),
                        i18n::row(),
                        |ui| {
                            let width = (ui.available_width() - 64.0).max(100.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(width, 48.0),
                                i18n::column(),
                                |ui| {
                                    ui.set_min_width(width);
                                    ui.t_strong("Animations");
                                    ui.t_label(
                                        egui::RichText::new(
                                            "Smooth selection and loading effects.",
                                        )
                                        .small()
                                        .weak(),
                                    );
                                },
                            );
                            if theme::switch(ui, &mut animations, "Enable animations").changed() {
                                for theme in [egui::Theme::Light, egui::Theme::Dark] {
                                    let mut style = (*ctx.style_of(theme)).clone();
                                    style.animation_time = if animations { 0.22 } else { 0.0 };
                                    ctx.set_style_of(theme, style);
                                }
                            }
                        },
                    );
                });
            });
    }
    fn settings_content(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        theme::page_fade(ui, "settings_content", self.settings_page);
        egui::ScrollArea::vertical()
            .id_salt(("settings_scroll", self.settings_page))
            .auto_shrink([false, false])
            .max_height((ctx.viewport_rect().height() - 240.0).clamp(160.0, 500.0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                match self.settings_page {
                    0 => self.general_settings(ui, ctx),
                    1 => {
                        ui.t_strong("Updates");
                        ui.add_space(8.0);
                        theme::card(ui)
                            .fill(ui.visuals().extreme_bg_color)
                            .inner_margin(20.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                self.updates.view(ui, self.busy || self.exporting);
                            });
                    }
                    _ => {
                        ui.t_strong("About Reclaim");
                        ui.add_space(8.0);
                        theme::card(ui)
                            .fill(ui.visuals().extreme_bg_color)
                            .inner_margin(20.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.t_heading("Reclaim");
                                theme::detail_row(ui, "Version", env!("CARGO_PKG_VERSION"));
                                ui.t_label("Read-only disk image carving and file recovery");
                                ui.add_space(8.0);
                                ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), i18n::row(), |ui| {
                                    ui.t_label("License");
                                    if ui.link("MIT").clicked() {
                                        self.open_link(ctx, "https://github.com/sudo-py-dev/reclaim/blob/main/LICENSE");
                                    }
                                });
                                ui.add_space(8.0);
                                if ui.link(i18n::t("Project website")).clicked() {
                                    self.open_link(ctx, "https://github.com/sudo-py-dev/reclaim");
                                }
                            });
                        ui.add_space(16.0);
                        ui.t_strong("Supported files");
                        theme::card(ui)
                            .fill(ui.visuals().extreme_bg_color)
                            .inner_margin(20.0)
                            .show(ui, |ui| {
                                ui.set_width(ui.available_width());
                                ui.t_label(reclaim::FORMAT_SUMMARY);
                            });
                    }
                }
                if let Some(error) = &self.settings_error {
                    ui.t_label(error);
                }
            });
    }
    fn settings_view(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut done = false;
        let modal = egui::Modal::new(egui::Id::new("settings_dialog"))
            .frame(
                egui::Frame::popup(&ctx.global_style())
                    .inner_margin(20.0)
                    .corner_radius(14),
            )
            .show(ctx, |ui| {
                ui.set_width((ctx.viewport_rect().width() - 80.0).clamp(260.0, 960.0));
                ui.spacing_mut().item_spacing = egui::vec2(8.0, 8.0);
                ui.with_layout(i18n::column(), |ui| {
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 48.0),
                        i18n::row(),
                        |ui| {
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::hover());
                            ui.painter()
                                .rect_filled(rect, 10, theme::ACCENT.gamma_multiply(0.15));
                            theme::paint_symbol(
                                ui,
                                rect,
                                theme::Icon::Settings,
                                19.0,
                                ui.visuals().strong_text_color(),
                            );
                            ui.t_heading("Settings");
                            ui.with_layout(i18n::trailing_row(), |ui| {
                                done |= theme::icon_button(
                                    ui,
                                    theme::Icon::Close,
                                    "Close",
                                    false,
                                    egui::vec2(32.0, 32.0),
                                )
                                .clicked();
                            });
                        },
                    );
                    ui.add_space(8.0);
                    let categories = ["General", "Updates", "About"];
                    if ui.available_width() >= 680.0 {
                        ui.with_layout(i18n::row().with_cross_align(egui::Align::Min), |ui| {
                            ui.allocate_ui_with_layout(
                                egui::vec2(160.0, 360.0),
                                i18n::column(),
                                |ui| {
                                    ui.set_width(160.0);
                                    for (index, label) in categories.iter().enumerate() {
                                        if theme::settings_category(
                                            ui,
                                            index,
                                            label,
                                            self.settings_page == index,
                                        )
                                        .clicked()
                                        {
                                            self.settings_page = index;
                                        }
                                    }
                                },
                            );
                            ui.add_space(16.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(ui.available_width(), 360.0),
                                i18n::column(),
                                |ui| {
                                    self.settings_content(ui, ctx);
                                },
                            );
                        });
                    } else {
                        theme::setting_choices(
                            ui,
                            "settings_categories",
                            &mut self.settings_page,
                            &[(0, "General"), (1, "Updates"), (2, "About")],
                        );
                        ui.add_space(8.0);
                        self.settings_content(ui, ctx);
                    }
                    ui.add_space(12.0);
                    ui.separator();
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), 40.0),
                        i18n::row(),
                        |ui| {
                            if theme::icon_button(
                                ui,
                                theme::Icon::Guide,
                                "Recovery guide",
                                false,
                                egui::vec2(40.0, 40.0),
                            )
                            .clicked()
                            {
                                if self.page != 2 {
                                    self.guide_return = self.page;
                                }
                                self.page = 2;
                                done = true;
                            }
                            ui.with_layout(i18n::trailing_row(), |ui| {
                                done |= theme::icon_button(
                                    ui,
                                    theme::Icon::Check,
                                    "Done",
                                    true,
                                    egui::vec2(52.0, 40.0),
                                )
                                .clicked();
                            });
                        },
                    );
                });
            });
        if done || modal.should_close() {
            self.settings_open = false;
        }
    }
    fn render(&mut self, ui: &mut egui::Ui) {
        self.poll();
        if let Some(receiver) = &self.link_receiver {
            match receiver.try_recv() {
                Ok(result) => {
                    self.settings_error = result.err();
                    self.link_receiver = None;
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.settings_error = Some("Could not open the browser.".into());
                    self.link_receiver = None;
                }
                Err(mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.busy || self.drives_loading {
            ui.ctx().request_repaint_after(Duration::from_millis(100));
        }
        if self.updates.available() && !self.settings_open {
            egui::Panel::bottom("update_notice").show(ui, |ui| {
                ui.with_layout(i18n::row(), |ui| {
                    ui.t_label("An update is available.");
                    if ui.button(i18n::t("What’s new")).clicked() {
                        self.settings_page = 1;
                        self.settings_open = true;
                    }
                });
            });
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
                            let has_results = self.scanned_source.is_some();
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
                                let label = if self.busy && self.cancel.load(Ordering::Relaxed) {
                                    "Stopping…"
                                } else if self.busy {
                                    "Stop"
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
                                    .add_enabled_ui(
                                        (self.busy || self.source.is_some())
                                            && !(self.busy && self.cancel.load(Ordering::Relaxed)),
                                        |ui| {
                                            theme::icon_text_button(
                                                ui,
                                                if self.busy {
                                                    theme::Icon::Stop
                                                } else {
                                                    theme::Icon::Search
                                                },
                                                label,
                                                true,
                                                egui::vec2(164.0, 38.0),
                                            )
                                        },
                                    )
                                    .inner
                                    .clicked()
                                {
                                    if self.busy {
                                        self.stop_operation();
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
            egui::Panel::bottom("results_bar")
                .frame(
                    egui::Frame::new()
                        .fill(theme::header_color(ui))
                        .inner_margin(egui::Margin::symmetric(24, 12)),
                )
                .show(ui, |ui| {
                    let selected = self
                        .visible_result_indices()
                        .into_iter()
                        .filter(|&index| self.results[index].selected)
                        .count();
                    let recovering = !self.busy && selected > 0;
                    let stopping = self.busy && self.cancel.load(Ordering::Relaxed);
                    let fraction = if self.total > 0 {
                        (self.scanned as f64 / self.total as f64).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    // Keep the most recent sustained check visible between candidates.
                    // This is historical information, never a claim that a finished check is active.
                    if let Some((_, kind, bytes, since)) = self.scan_activity
                        && since.elapsed().as_secs_f32() >= 1.5
                    {
                        self.scan_check_summary = Some((kind, bytes));
                    }
                    let status_height = if self.busy { 116.0 } else { 44.0 };
                    ui.allocate_ui_with_layout(
                        egui::vec2(ui.available_width(), status_height),
                        i18n::row(),
                        |ui| {
                            let reserve = 128.0 + if recovering { 152.0 } else { 0.0 };
                            let width = (ui.available_width() - reserve).max(100.0);
                            ui.allocate_ui_with_layout(
                                egui::vec2(width, status_height),
                                i18n::column(),
                                |ui| {
                                    ui.set_min_width(width);
                                    if self.busy {
                                        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 20.0), i18n::row(), |ui| {
                                            theme::activity_indicator(ui, false, stopping);
                                            ui.t_strong(if stopping {
                                                "Stopping safely…"
                                            } else if self.exporting {
                                                "Recovering selected files…"
                                            } else if !self.allocation_ready {
                                                "Reading unused-space map…"
                                            } else if self.source_handle.is_none() {
                                                "Preparing scan"
                                            } else {
                                                "Looking for your files…"
                                            });
                                        });
                                        let info = if !self.allocation_ready {
                                            i18n::t("Finding unused space before scanning")
                                        } else if self.exporting {
                                            i18n::t("Stop after this file")
                                        } else {
                                            let files =
                                                i18n::t(&format!("{} files", self.results.len()));
                                            if self.total > 0 {
                                                format!(
                                                    "{files} · {:.1}%",
                                                    fraction * 100.0
                                                )
                                            } else {
                                                format!("{files} · {}", theme::bytes(self.scanned))
                                            }
                                        };
                                        ui.add(
                                            egui::Label::new(i18n::widget(
                                                ui,
                                                egui::RichText::new(info).small(),
                                                false,
                                            ))
                                            .truncate(),
                                        );
                                        if !self.exporting && self.allocation_ready && self.total > 0 {
                                            let disk = format!("{} {} · {} {} · {} {}", theme::bytes(self.scanned.min(self.total)), i18n::t("scanned"), theme::bytes(self.total.saturating_sub(self.scanned)), i18n::t("remaining"), theme::bytes(self.total), i18n::t(if self.unused_scan {"unused total"} else {"total"}));
                                            ui.add(egui::Label::new(i18n::widget(ui, egui::RichText::new(&disk).small(), false)).truncate()).t_on_hover_text(&disk);
                                        }
                                        let waiting = self.last_scan_update.is_some_and(|last| last.elapsed().as_secs() >= 15);
                                        let detail = if stopping {
                                            i18n::t("Finishing the current step. Your results will stay here.")
                                        } else if self.exporting {
                                            i18n::t("Saving your selected files to the recovery folder.")
                                        } else if waiting {
                                            i18n::t("No new progress yet. The drive or file check may be taking longer.")
                                        } else if let Some((kind, bytes)) = self.scan_check_summary {
                                            format!("{} · {} · {}", i18n::t("Latest file check"), kind.to_uppercase(), theme::bytes(bytes as u64))
                                        } else if !self.allocation_ready {
                                            i18n::t("Reading the filesystem map. Occupied files will be skipped.")
                                        } else if self.source_handle.is_none() {
                                            i18n::t("Connecting to your drive. This can take a moment.")
                                        } else {
                                            i18n::t("Reading and checking files…")
                                        };
                                        ui.add(egui::Label::new(i18n::widget(ui, egui::RichText::new(&detail).small().weak(), false)).truncate()).t_on_hover_text(&detail);
                                        if !self.exporting {
                                            let note = i18n::t(if self.unused_scan {"Only unused space is searched. You can stop and keep your results."} else {"Found files appear above. You can stop and keep your results."});
                                            ui.add(egui::Label::new(i18n::widget(ui, egui::RichText::new(&note).small().weak(), false)).truncate()).t_on_hover_text(&note);
                                        }
                                    } else {
                                        ui.t_strong(format!("{selected} selected"));
                                        ui.add(
                                            egui::Label::new(i18n::widget(
                                                ui,
                                                egui::RichText::new(&self.status).small(),
                                                false,
                                            ))
                                            .truncate(),
                                        )
                                        .t_on_hover_text(&self.status);
                                    }
                                },
                            );
                            ui.with_layout(i18n::trailing_row(), |ui| {
                                let label = if stopping {
                                    "Stopping…"
                                } else if self.busy {
                                    "Stop"
                                } else {
                                    "Scan again"
                                };
                                if ui
                                    .add_enabled_ui(
                                        !stopping && (self.busy || self.scanned_source.is_some()),
                                        |ui| {
                                            theme::icon_text_button(
                                                ui,
                                                if self.busy {
                                                    theme::Icon::Stop
                                                } else {
                                                    theme::Icon::Refresh
                                                },
                                                label,
                                                false,
                                                egui::vec2(120.0, 38.0),
                                            )
                                        },
                                    )
                                    .inner
                                    .clicked()
                                {
                                    if self.busy {
                                        self.stop_operation();
                                    } else {
                                        self.source = self.scanned_source.clone();
                                        self.start();
                                    }
                                }
                                if recovering
                                    && theme::icon_text_button(
                                        ui,
                                        theme::Icon::Recover,
                                        "Recover files…",
                                        true,
                                        egui::vec2(152.0, 38.0),
                                    )
                                    .clicked()
                                {
                                    self.choose_recovery_folder();
                                }
                            });
                        },
                    );
                    if self.busy {
                        let known = (!self.exporting
                            && self.total > 0
                            && self.source_handle.is_some()
                            && self.allocation_ready)
                            .then_some(fraction as f32);
                        theme::operation_progress(
                            ui,
                            ("operation_progress", self.started),
                            known,
                            stopping,
                        );
                    }
                });
        }
        if self.page == 2 {
            egui::Panel::bottom("guide_navigation")
                .frame(
                    egui::Frame::new()
                        .fill(theme::header_color(ui))
                        .inner_margin(egui::Margin::symmetric(24, 12)),
                )
                .show(ui, |ui| self.guide_navigation(ui));
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
                            theme::page_fade(ui, "main_page", self.page);
                            if self.page == 1 {
                                self.results_view(ui);
                            } else if self.page == 0 {
                                ui.add_space(8.0);
                                self.source_view(ui);
                            } else {
                                self.guide_view(ui);
                            }
                        },
                    );
                });
            });
        window::resize(ui);
        self.settings_view(ui.ctx());
        self.file_details_view(ui.ctx());
        self.recovery_folder_view(ui.ctx());
    }
}
impl eframe::App for Reclaim {
    fn raw_input_hook(&mut self, _ctx: &egui::Context, input: &mut egui::RawInput) {
        if let Some(watcher) = &self.system_theme {
            watcher.apply_input(input);
        }
    }
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.updates.poll(ui.ctx());
        self.render(ui);
        if self.updates.restart_requested() && !self.busy && !self.exporting {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if self.updates.updating() || self.updates.restart_requested() {
            egui::Modal::new(egui::Id::new("automatic_update_progress")).show(ui.ctx(), |ui| {
                ui.set_width(320.0);
                ui.t_heading("Updating Reclaim…");
                self.updates.progress_view(ui);
            });
        }
        static FIRST_FRAME: std::sync::Once = std::sync::Once::new();
        FIRST_FRAME.call_once(|| diagnostics::write("First application frame completed"));
    }
}
impl Drop for Reclaim {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::Relaxed);
    }
}
// Prevent process-backed D-Bus addresses (including unixexec) from reaching
// our clients or desktop dependencies. A session socket is required so the
// native dialog library cannot fall back to autolaunching a bus helper.
#[cfg(target_os = "linux")]
fn native_bus_addresses() -> Result<(), String> {
    let session = std::env::var("DBUS_SESSION_BUS_ADDRESS").map_err(|_| {
        "A native desktop D-Bus session is required; helper autolaunch is disabled.".to_owned()
    })?;
    native_bus_address(&session)?;
    for name in ["DBUS_SYSTEM_BUS_ADDRESS", "AT_SPI_BUS_ADDRESS"] {
        if let Some(address) = std::env::var_os(name) {
            native_bus_address(address.to_str().ok_or("Invalid native bus address")?)?;
        }
    }
    Ok(())
}
#[cfg(target_os = "linux")]
fn native_bus_address(value: &str) -> Result<(), String> {
    if value.contains(';') {
        return Err("D-Bus address fallback lists are disabled.".into());
    }
    let address: zbus::Address = value.parse().map_err(|_| "Invalid native D-Bus address")?;
    if !matches!(address.transport(), zbus::address::Transport::Unix(_)) {
        return Err("Only native Unix-socket D-Bus addresses are allowed; process-backed transports are disabled.".into());
    }
    Ok(())
}
#[cfg(all(test, target_os = "linux"))]
#[test]
fn native_bus_transport_rejects_external_programs() {
    assert!(native_bus_address("unix:path=/run/user/1000/bus").is_ok());
    assert!(native_bus_address("unix:abstract=desktop-bus").is_ok());
    for address in [
        "unixexec:path=/bin/sh,argv1=-c,argv2=echo",
        "autolaunch:",
        "tcp:host=localhost,port=1234",
        "unix:path=/tmp/bus;unixexec:path=sh",
    ] {
        assert!(native_bus_address(address).is_err(), "{address}");
    }
}

fn main() {
    diagnostics::install();
    update_runtime::cleanup_staged_updates();
    if let Err(error) = run() {
        diagnostics::report("Reclaim could not open its window.", &format!("{error:?}"));
        std::process::exit(1);
    }
    diagnostics::write("Reclaim closed normally");
}

fn run() -> eframe::Result {
    #[cfg(target_os = "linux")]
    if let Err(error) = native_bus_addresses() {
        eprintln!("{error}");
        return Ok(());
    }
    i18n::init();
    eframe::run_native(
        &i18n::t("Reclaim — File Recovery"),
        eframe::NativeOptions {
            #[cfg(windows)]
            renderer: eframe::Renderer::Wgpu,
            #[cfg(windows)]
            wgpu_options: windows_graphics_options(),
            viewport: egui::ViewportBuilder::default()
                .with_decorations(!window::CUSTOM_DECORATIONS)
                .with_app_id("io.github.reclaim.Reclaim")
                .with_icon(
                    eframe::icon_data::from_png_bytes(include_bytes!(
                        "../assets/icons/reclaim.png"
                    ))
                    .expect("bundled application icon"),
                )
                .with_inner_size([1120.0, 780.0])
                .with_min_inner_size([540.0, 580.0]),
            ..Default::default()
        },
        Box::new(|cc| {
            diagnostics::write("Graphics initialized; creating application");
            theme::setup(&cc.egui_ctx);
            let mut app = Reclaim::default();
            app.system_theme = theme::watch_system_theme(&cc.egui_ctx);
            app.drive_monitor = drives::watch(&cc.egui_ctx);
            app.refresh_drives();
            Ok(Box::new(app))
        }),
    )
}

#[cfg(test)]
mod ui_tests;

// DirectX avoids the OpenGL 1.1-only context on many Windows VMs. Wgpu can
// select Microsoft's software adapter when accelerated graphics are unavailable.
#[cfg(windows)]
fn windows_graphics_options() -> eframe::WgpuConfiguration {
    let mut options = eframe::WgpuConfiguration::default();
    if let eframe::egui_wgpu::WgpuSetup::CreateNew(setup) = &mut options.wgpu_setup {
        setup.instance_descriptor.backends = eframe::wgpu::Backends::DX12;
        // FXC is supplied by Windows, unlike the optional DXC redistributable.
        setup
            .instance_descriptor
            .backend_options
            .dx12
            .shader_compiler = eframe::wgpu::Dx12Compiler::Fxc;
    }
    options
}
