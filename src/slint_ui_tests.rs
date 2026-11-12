use super::*;

/// A phone connected for file transfer, as Linux discovery reports it.
fn phone() -> drives::Drive {
    drives::Drive {
        path: "/sys/bus/usb/devices/1-2".into(),
        name: "Unihertz F21 Pro".into(),
        name_is_message: false,
        size: 0,
        kind: "Phone or camera".into(),
        location: "File transfer only".into(),
        mounted: false,
        encrypted: false,
        sector_size: 512,
        phone: true,
    }
}

#[test]
fn phones_are_listed_unavailable_with_an_explanation() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .ok();
    let ui = AppWindow::new().unwrap();
    let mut state = Reclaim::default();
    state.sync_ui(&ui);
    assert!(!ui.get_phone_connected());
    state.drives.push(phone());
    state.sync_ui(&ui);
    assert!(ui.get_phone_connected());
    let row = ui.get_drives().row_data(0).unwrap();
    assert!(!row.available);
    assert_eq!(row.icon, DriveIcon::Phone);
    assert_eq!(row.size, "");
    assert_eq!(row.device, "");
    assert!(row.detail.contains("File transfer only"));
}

#[test]
fn slint_workspace_renders_all_pages_and_filters_recovery_selection() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    let mut state = Reclaim::default();
    state.drives.push(drives::Drive {
        path: "/sample/disk.img".into(),
        name: "Travel archive".into(),
        name_is_message: false,
        size: 128 << 30,
        kind: "USB drive".into(),
        location: "External · NTFS".into(),
        mounted: false,
        encrypted: false,
        sector_size: 512,
        phone: false,
    });
    // Other kinds render their own icon and device tag in the previews.
    for (path, name, name_is_message, kind, location, encrypted) in [
        (
            "/dev/nvme0n1p2",
            "Linux system",
            true,
            "Volume",
            "Mounted at /",
            false,
        ),
        (
            "/dev/sdb",
            "Internal drive",
            true,
            "Entire drive",
            "Not mounted",
            false,
        ),
        (
            "/dev/sdc1",
            "Volume",
            true,
            "Volume",
            "Encrypted: unlock in Disks, then refresh",
            true,
        ),
    ] {
        state.drives.push(drives::Drive {
            path: path.into(),
            name: name.into(),
            name_is_message,
            size: 512 << 30,
            kind: kind.into(),
            location: location.into(),
            mounted: false,
            encrypted,
            sector_size: 512,
            phone: false,
        });
    }
    state.drives.push(phone());
    state.source = Some(state.drives[0].path.clone());
    state.results = vec![
        Found {
            offset: 4096,
            size: 2048,
            kind: "png",
            selected: true,
            original_name: Some("Summer.png".into()),
            resident_data: None,
            ntfs: None,
            extents: None,
            validation: reclaim::Validation::StructureChecked,
        },
        Found {
            offset: 8192,
            size: 4096,
            kind: "pdf",
            selected: true,
            original_name: Some("Notes.pdf".into()),
            resident_data: None,
            ntfs: None,
            extents: None,
            validation: reclaim::Validation::StructureChecked,
        },
    ];
    state.sync_ui(&ui);
    assert!(ui.get_can_scan());
    assert_eq!(ui.get_files().row_count(), 2);
    state.filter = "summer".into();
    assert_eq!(state.recovery_files().len(), 1);
    state.category = 2;
    assert!(state.recovery_files().is_empty());
    state.filter.clear();
    state.category = 0;
    state.sync_ui(&ui);
    state.inspected = Some(0);
    let original_path = state.source.clone();
    let original_names: Vec<_> = state
        .results
        .iter()
        .map(|f| f.original_name.clone())
        .collect();
    ui.set_query("תמונות report العربية".into());
    ui.show().unwrap();
    for (code, language) in [
        ("en", i18n::Language::English),
        ("he", i18n::Language::Hebrew),
        ("ar", i18n::Language::Arabic),
        ("es", i18n::Language::Spanish),
        ("fr", i18n::Language::French),
        ("de", i18n::Language::German),
        ("ru", i18n::Language::Russian),
    ] {
        if let Ok(locales) = std::env::var("RECLAIM_PREVIEW_LOCALES")
            && !locales.split(',').any(|locale| locale == code)
        {
            continue;
        }
        i18n::with_language(language, || {
            ui_text::apply(&ui);
            state.sync_ui(&ui);
            assert_eq!(ui.global::<Locale>().get_rtl(), i18n::is_rtl());
            assert_eq!(ui.global::<Strings>().get_sources(), i18n::t("Sources"));
            assert_eq!(ui.get_query(), "תמונות report العربية");
            assert_eq!(state.source, original_path);
            assert_eq!(
                state
                    .results
                    .iter()
                    .map(|f| f.original_name.clone())
                    .collect::<Vec<_>>(),
                original_names
            );
            for dark in [true, false] {
                ui.global::<Palette>().set_theme(if dark { 2 } else { 1 });
                assert_eq!(ui.global::<Palette>().get_dark(), dark);
                for (width, height) in [(1160, 780), (900, 640)] {
                    if width == 900 && !matches!(code, "en" | "he" | "ar") {
                        continue;
                    }
                    ui.window()
                        .set_size(slint::PhysicalSize::new(width, height));
                    for page in 0..3 {
                        ui.set_page(page);
                        std::thread::sleep(Duration::from_millis(180));
                        slint::platform::update_timers_and_animations();
                        if page == 1 {
                            ui.set_category(0);
                            let choice =
                                i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                                    &ui,
                                    &i18n::t("All files"),
                                )
                                .next()
                                .expect("localized category dropdown");
                            choice.invoke_accessible_default_action();
                            for key in [
                                slint::platform::Key::DownArrow,
                                slint::platform::Key::Return,
                            ] {
                                ui.window().dispatch_event(
                                    slint::platform::WindowEvent::KeyPressed { text: key.into() },
                                );
                                ui.window().dispatch_event(
                                    slint::platform::WindowEvent::KeyReleased { text: key.into() },
                                );
                            }
                            assert_eq!(
                                ui.get_category(),
                                1,
                                "keyboard dropdown selection in {code}"
                            );
                            ui.set_category(0);
                        }
                        let snapshot = ui.window().take_snapshot().unwrap();
                        assert_eq!((snapshot.width(), snapshot.height()), (width, height));
                        // The sidebar must physically switch edges, not just align its text.
                        let edge = if i18n::is_rtl() { width - 2 } else { 1 };
                        let pixel = snapshot.as_slice()[(300 * width + edge) as usize];
                        let surface = ui.global::<Palette>().get_surface();
                        assert_eq!(
                            (pixel.r, pixel.g, pixel.b),
                            (surface.red(), surface.green(), surface.blue()),
                            "sidebar edge: {code}, {width}, {page}"
                        );
                        if let Ok(directory) = std::env::var("RECLAIM_PREVIEW_DIR") {
                            std::fs::create_dir_all(&directory).unwrap();
                            image::save_buffer(
                                format!(
                                    "{directory}/{code}-{width}-{page}-{}.png",
                                    if dark { "dark" } else { "light" }
                                ),
                                snapshot.as_bytes(),
                                snapshot.width(),
                                snapshot.height(),
                                image::ColorType::Rgba8,
                            )
                            .unwrap();
                        }
                    }
                }
            }
        });
    }
}

#[test]
fn all_slint_labels_have_catalog_entries() {
    for key in ui_text::UI_MESSAGES {
        assert!(i18n::has_message(key), "missing UI message: {key}");
    }
}

#[test]
fn linux_app_id_matches_the_packaged_desktop_entry() {
    let entry = include_str!("../packaging/linux/io.github.reclaim.Reclaim.desktop");
    assert!(entry.contains(&format!("Icon={APP_ID}")));
    assert!(entry.contains(&format!("StartupWMClass={APP_ID}")));
    let tooling = include_str!("../tools/src/main.rs");
    assert!(tooling.contains(&format!("IDENTIFIER: &str = \"{APP_ID}\"")));
}

#[test]
fn recovery_scrollbar_keeps_controls_clear_and_reaches_the_last_file() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    ui.set_page(1);
    ui.set_files(ModelRc::new(VecModel::from(
        (0..200)
            .map(|index| FileItem {
                name: format!("candidate-{index:04}.png").into(),
                kind: "PNG".into(),
                size: "2 KiB".into(),
                selected: true,
                index,
            })
            .collect::<Vec<_>>(),
    )));
    ui.show().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(900, 640));
    for (code, rtl) in [("en", false), ("he", true)] {
        ui.global::<Locale>().set_rtl(rtl);
        for dark in [false, true] {
            ui.global::<Palette>().set_theme(if dark { 2 } else { 1 });
            ui.window().take_snapshot().unwrap();
            let list = i_slint_backend_testing::ElementHandle::find_by_element_id(
                &ui,
                "AppWindow::files-view",
            )
            .next()
            .expect("recovered file list");
            let position = list.absolute_position();
            let size = list.size();
            let rail = slint::LogicalPosition::new(
                position.x + if rtl { 7.0 } else { size.width - 7.0 },
                position.y + size.height / 2.0,
            );
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerPressed {
                    position: rail,
                    button: slint::platform::PointerEventButton::Left,
                });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::PointerReleased {
                    position: rail,
                    button: slint::platform::PointerEventButton::Left,
                });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyPressed {
                    text: slint::platform::Key::End.into(),
                });
            ui.window()
                .dispatch_event(slint::platform::WindowEvent::KeyReleased {
                    text: slint::platform::Key::End.into(),
                });
            let snapshot = ui.window().take_snapshot().unwrap();
            let last = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                "candidate-0199.png",
            )
            .find(|element| {
                let y = element.absolute_position().y;
                y >= position.y && y < position.y + size.height
            });
            let last = last.expect("last file reachable by keyboard");
            let last_position = last.absolute_position();
            assert!(
                last_position.y + last.size().height <= position.y + size.height + 1.0,
                "the last row must be fully visible"
            );
            let previous = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                "candidate-0198.png",
            )
            .next()
            .expect("previous visible file");
            assert!(
                (last_position.y - previous.absolute_position().y - 52.0).abs() < 1.0,
                "list rows must remain spaced and virtualized"
            );
            if rtl {
                assert!(last_position.x >= position.x + 14.0, "RTL scrollbar gutter");
            } else {
                assert!(
                    last_position.x + last.size().width <= position.x + size.width - 14.0,
                    "LTR scrollbar gutter"
                );
            }
            if let Ok(directory) = std::env::var("RECLAIM_PREVIEW_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                image::save_buffer(
                    format!(
                        "{directory}/scroll-{code}-{}.png",
                        if dark { "dark" } else { "light" }
                    ),
                    snapshot.as_bytes(),
                    snapshot.width(),
                    snapshot.height(),
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
        }
    }
}

#[test]
fn update_notice_can_be_dismissed_and_release_notes_open_only_on_request() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    ui.set_update_available(true);
    ui.set_update_notes(
        Rc::new(VecModel::from(vec![
            ReleaseNoteBlock {
                text: "Improvements".into(),
                kind: 1,
            },
            ReleaseNoteBlock {
                text: "• Clearer recovery results".into(),
                kind: 2,
            },
            ReleaseNoteBlock {
                text: "• More reliable drive detection".into(),
                kind: 2,
            },
            ReleaseNoteBlock {
                text: "• Smaller downloads".into(),
                kind: 2,
            },
        ]))
        .into(),
    );
    let installed = Rc::new(std::cell::Cell::new(false));
    let flag = installed.clone();
    ui.on_apply_update(move || flag.set(true));
    let weak = ui.as_weak();
    ui.on_dismiss_update(move || weak.upgrade().unwrap().set_update_notice(false));
    ui.show().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(900, 640));
    for (code, language) in [
        ("en", i18n::Language::English),
        ("he", i18n::Language::Hebrew),
    ] {
        i18n::with_language(language, || {
            ui_text::apply(&ui);
            ui.set_update_title(i18n::message("Version {0} is available.", &["0.3.0"]).into());
            ui.set_update_label(i18n::t("Update and restart").into());
            for dark in [false, true] {
                ui.global::<Palette>().set_theme(if dark { 2 } else { 1 });
                ui.set_page(0);
                ui.set_update_notice(true);
                ui.set_update_details_open(false);
                let snapshot = ui.window().take_snapshot().unwrap();
                if let Ok(directory) = std::env::var("RECLAIM_PREVIEW_DIR") {
                    std::fs::create_dir_all(&directory).unwrap();
                    image::save_buffer(
                        format!("{directory}/update-notice-{code}-{dark}.png"),
                        snapshot.as_bytes(),
                        snapshot.width(),
                        snapshot.height(),
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
                let later = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                    &ui,
                    &i18n::t("Later"),
                )
                .next()
                .expect("dismiss update notice");
                later.invoke_accessible_default_action();
                assert!(!ui.get_update_notice());
                assert!(ui.get_update_available());
                assert!(!installed.get());
                ui.set_update_notice(true);
                ui.window().take_snapshot().unwrap();
                let details = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                    &ui,
                    &i18n::t("What’s new"),
                )
                .next()
                .expect("review release notes");
                details.invoke_accessible_default_action();
                assert_eq!(ui.get_page(), 0);
                assert!(ui.get_update_details_open());
                assert!(!installed.get());
                let snapshot = ui.window().take_snapshot().unwrap();
                if let Ok(directory) = std::env::var("RECLAIM_PREVIEW_DIR") {
                    image::save_buffer(
                        format!("{directory}/update-notes-{code}-{dark}.png"),
                        snapshot.as_bytes(),
                        snapshot.width(),
                        snapshot.height(),
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
                ui.set_page(0);
                ui.set_busy(true);
                ui.window().take_snapshot().unwrap();
                assert!(
                    i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                        &ui,
                        &i18n::t("Later")
                    )
                    .next()
                    .is_none(),
                    "no update notice during a scan"
                );
                ui.set_busy(false);
                ui.window().take_snapshot().unwrap();
                let apply = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                    &ui,
                    &i18n::t("Update and restart"),
                )
                .next()
                .expect("explicit update action");
                apply.invoke_accessible_default_action();
                assert!(installed.replace(false));
                ui.set_update_notice(false);
                ui.set_page(2);
                ui.set_update_details_open(false);
                ui.window().take_snapshot().unwrap();
                assert!(
                    i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                        &ui,
                        &i18n::t("Check for updates"),
                    )
                    .next()
                    .is_none(),
                    "install action replaces the update check"
                );
                let update_label = i18n::t("Update and restart");
                let mut actions = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                    &ui,
                    &update_label,
                );
                actions
                    .next()
                    .expect("settings update action")
                    .invoke_accessible_default_action();
                assert!(installed.replace(false));
                let settings_details =
                    i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                        &ui,
                        &i18n::t("What’s new"),
                    )
                    .next()
                    .expect("release notes remain available in settings");
                settings_details.invoke_accessible_default_action();
                assert!(ui.get_update_details_open());
                ui.window().take_snapshot().unwrap();
            }
        });
    }
}

#[test]
fn refresh_buttons_animate_even_when_results_do_not_change() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            mock_time: true,
            ..Default::default()
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    ui_text::apply(&ui);
    let drives = Rc::new(std::cell::Cell::new(0));
    let checks = Rc::new(std::cell::Cell::new(0));
    let count = drives.clone();
    ui.on_refresh(move || count.set(count.get() + 1));
    let count = checks.clone();
    ui.on_check_update(move || count.set(count.get() + 1));
    ui.show().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(900, 640));
    for (page, label, count) in [(0, "Refresh", drives), (2, "Check for updates", checks)] {
        ui.set_page(page);
        ui.window().take_snapshot().unwrap();
        let button =
            i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, &i18n::t(label))
                .next()
                .unwrap();
        button.invoke_accessible_default_action();
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert_eq!(count.get(), 1);
        assert!(!button.accessible_enabled().unwrap());
        button.invoke_accessible_default_action();
        assert_eq!(count.get(), 1, "coalesce repeated clicks while refreshing");
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(150));
        slint::platform::update_timers_and_animations();
        let first = ui.window().take_snapshot().unwrap();
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(150));
        slint::platform::update_timers_and_animations();
        let second = ui.window().take_snapshot().unwrap();
        assert!(
            first.as_bytes() != second.as_bytes(),
            "refresh icon should visibly rotate"
        );
        assert!(if page == 0 {
            ui.get_refresh_feedback()
        } else {
            ui.get_update_check_feedback()
        });
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(401));
        slint::platform::update_timers_and_animations();
        assert!(
            button.accessible_enabled().unwrap(),
            "a quick unchanged result still gets 700ms of feedback"
        );
        button.invoke_accessible_default_action();
        i_slint_backend_testing::mock_elapsed_time(Duration::ZERO);
        assert_eq!(
            count.get(),
            2,
            "each subsequent press restarts the animation"
        );
        if page == 0 {
            ui.set_drives_refreshing(true);
        } else {
            ui.set_update_busy(true);
        }
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(701));
        slint::platform::update_timers_and_animations();
        assert!(
            !button.accessible_enabled().unwrap(),
            "keep spinning until the real request finishes"
        );
        if page == 0 {
            ui.set_drives_refreshing(false);
        } else {
            ui.set_update_busy(false);
        }
        assert!(button.accessible_enabled().unwrap());
    }
}

/// A scan that finds thousands of files must not rebuild the list on every
/// tick. The incremental sync has to produce exactly the rows a full rebuild
/// would, through appends, selection changes, filters and a new scan.
#[test]
fn file_list_stays_exact_while_a_large_scan_streams_results() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    let candidate = |n: u64, kind: &'static str| Found {
        offset: n * 4096,
        size: 2048 + n,
        kind,
        selected: true,
        original_name: n.is_multiple_of(7).then(|| format!("Holiday {n}.{kind}")),
        resident_data: None,
        ntfs: None,
        extents: None,
        validation: reclaim::Validation::StructureChecked,
    };
    let assert_exact = |state: &Reclaim, ui: &AppWindow, step: &str| {
        let expected: Vec<_> = state
            .visible_result_indices()
            .into_iter()
            .map(|index| file_item(&state.results[index], index))
            .collect();
        let files = ui.get_files();
        let rows: Vec<_> = files.iter().collect();
        assert_eq!(rows, expected, "{step}");
        let selected = expected.iter().filter(|row| row.selected).count();
        assert_eq!(
            ui.get_all_selected(),
            !expected.is_empty() && selected == expected.len(),
            "{step}"
        );
    };
    let mut state = Reclaim::default();
    state.busy = true;
    state.sync_ui(&ui);
    // Results stream in uneven batches, with progress-only ticks in between.
    let mut n = 0u64;
    for batch in [1, 37, 0, 250, 3, 0, 1700, 9] {
        for _ in 0..batch {
            state.results.push(candidate(
                n,
                if n.is_multiple_of(3) { "pdf" } else { "jpg" },
            ));
            n += 1;
        }
        state.scanned = n;
        state.sync_ui(&ui);
        assert_exact(&state, &ui, &format!("after streaming {n} results"));
    }
    assert_eq!(ui.get_files().row_count(), 2000);

    // A selection change patches its row; the next tick keeps it.
    state.results[5].selected = false;
    state.sync_ui(&ui);
    assert_exact(&state, &ui, "after deselecting one file");
    assert!(!ui.get_all_selected());
    state.sync_ui(&ui);
    assert_exact(&state, &ui, "on an unchanged tick");

    // A category or search narrows the list, and later results respect it.
    state.category = reclaim::category_index("pdf");
    state.sync_ui(&ui);
    assert_exact(&state, &ui, "with the documents category");
    state.results.push(candidate(n, "pdf"));
    state.results.push(candidate(n + 1, "jpg"));
    n += 2;
    state.sync_ui(&ui);
    assert_exact(&state, &ui, "streaming under a category");
    state.category = 0;
    state.filter = "HOLIDAY".into();
    state.sync_ui(&ui);
    assert_exact(&state, &ui, "with a search");
    state.filter.clear();
    state.sync_ui(&ui);
    assert_exact(&state, &ui, "after clearing the search");

    // A new scan replaces the results, even with the same count.
    let count = state.results.len();
    state.results = (0..count as u64)
        .map(|i| candidate(i + 1_000_000, "png"))
        .collect();
    state.sync_ui(&ui);
    assert_exact(&state, &ui, "after a new scan with the same count");
    state.results.clear();
    state.sync_ui(&ui);
    assert_exact(&state, &ui, "after clearing the results");
    assert_eq!(ui.get_files().row_count(), 0);
    assert!(n > 2000);

    // Another state synced into the same window replaces the rows it does not
    // know about instead of appending to them.
    state.results = (0..30).map(|i| candidate(i, "jpg")).collect();
    state.sync_ui(&ui);
    let mut fresh = Reclaim::default();
    fresh.results = vec![candidate(7, "png")];
    fresh.sync_ui(&ui);
    assert_exact(
        &fresh,
        &ui,
        "a fresh state in a window that already shows rows",
    );
}

/// The scan status must not flip between sentences on every quick check: it
/// names a check only after it runs slowly, keeps it readable, and shows the
/// overall progress separately.
#[test]
fn scan_status_stays_steady_and_names_only_slow_checks() {
    i18n::with_language(i18n::Language::English, || {
        let start = std::time::Instant::now();
        let at = |ms: u64| start.checked_add(Duration::from_millis(ms)).unwrap();
        let mut state = Reclaim::default();
        state.busy = true;
        state.status = "Scanning for recoverable files…".into();
        state.total = 4 << 30;
        state.scanned = 1 << 30;

        let checking_zip = i18n::message("Checking a {0} file…", &["ZIP"]);
        // A quick check keeps the steady sentence.
        state.scan_activity = Some((4096, "zip", 1 << 20, start));
        let (status, detail) = state.scan_status(at(400));
        assert_eq!(status, "Scanning for recoverable files…");
        assert_eq!(detail, i18n::message("{0} of {1}", &["1.0 GiB", "4.0 GiB"]));

        // A slow one is named, and stays named while it runs.
        let (status, _) = state.scan_status(at(1600));
        assert_eq!(status, checking_zip);
        let (status, _) = state.scan_status(at(1900));
        assert_eq!(status, checking_zip);

        // A check that ends right after being named stays readable for a moment.
        state.scan_activity = None;
        let (status, _) = state.scan_status(at(2000));
        assert_eq!(status, checking_zip);
        let (status, _) = state.scan_status(at(3200));
        assert_eq!(status, "Scanning for recoverable files…");

        // A long check that was on screen for a while leaves as soon as it ends.
        state.scan_activity = Some((65536, "zip", 1 << 20, at(3200)));
        assert_eq!(state.scan_status(at(4800)).0, checking_zip);
        assert_eq!(state.scan_status(at(9000)).0, checking_zip);
        state.scan_activity = None;
        assert_eq!(
            state.scan_status(at(9100)).0,
            "Scanning for recoverable files…"
        );

        // Stopping always shows the stop message, never a check.
        state.scan_activity = Some((8192, "zip", 0, start));
        state.cancel.store(true, Ordering::Relaxed);
        state.status = "Stopping after the current step…".into();
        let (status, _) = state.scan_status(at(9000));
        assert_eq!(status, "Stopping after the current step…");

        // Nothing beside the status once the scan is over.
        state.busy = false;
        assert_eq!(state.scan_status(at(9100)).1, "");
    });
}

#[test]
fn scan_status_renders_on_one_line_in_every_direction() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    ui.show().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(900, 640));
    for (code, language) in [
        ("en", i18n::Language::English),
        ("he", i18n::Language::Hebrew),
        ("de", i18n::Language::German),
    ] {
        i18n::with_language(language, || {
            ui_text::apply(&ui);
            let mut state = Reclaim::default();
            state.busy = true;
            state.status = "Scanning for recoverable files…".into();
            state.total = 6_120_000_000;
            state.scanned = 2_250_000_000;
            for (name, checking) in [("scanning", false), ("checking", true)] {
                state.scan_activity =
                    checking.then(|| (4096, "zip", 27 << 20, std::time::Instant::now()));
                state.long_check = checking.then(|| ("zip", std::time::Instant::now()));
                state.sync_ui(&ui);
                ui.set_page(1);
                let detail = ui.get_scan_detail();
                assert!(detail.contains("GiB"), "{code}: {detail}");
                let snapshot = ui.window().take_snapshot().unwrap();
                if let Ok(directory) = std::env::var("RECLAIM_PREVIEW_DIR") {
                    image::save_buffer(
                        format!("{directory}/status-{code}-{name}.png"),
                        snapshot.as_bytes(),
                        snapshot.width(),
                        snapshot.height(),
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
        });
    }
}

/// No control may leave the window or overlap another, on any page, in any
/// main state, in left-to-right, right-to-left and the longest language, at the
/// minimum and default window sizes. The scan status headline must not move or
/// resize between "scanning" and a named slow check.
#[test]
fn layout_is_clear_and_steady_on_every_page_language_and_size() {
    use crate::layout_checks::{assert_controls_clear, assert_steady};
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    ui.show().unwrap();
    let first_run = Reclaim::default;
    let with_phone = || {
        let mut state = Reclaim::default();
        state.drives.push(phone());
        state
    };
    let scanning = || {
        let mut state = Reclaim::default();
        state.source = Some("/dev/disk-with-a-long-descriptive-name".into());
        for n in 0..40u64 {
            state.results.push(Found {
                offset: n * 4096,
                size: 2048 + n,
                kind: if n.is_multiple_of(3) { "pdf" } else { "jpg" },
                selected: true,
                original_name: Some(format!(
                    "A very long holiday photo name number {n} from the summer trip.jpg"
                )),
                resident_data: None,
                ntfs: None,
                extents: None,
                validation: reclaim::Validation::StructureChecked,
            });
        }
        state.busy = true;
        state.status = "Scanning for recoverable files…".into();
        state.total = 6_120_000_000;
        state.scanned = 2_250_000_000;
        state
    };
    let recovering = || {
        let mut state = scanning();
        state.exporting = true;
        state.recovery_status = "Recovering selected files…".into();
        state
    };
    for (code, language) in [
        ("en", i18n::Language::English),
        ("he", i18n::Language::Hebrew),
        ("ar", i18n::Language::Arabic),
        ("de", i18n::Language::German),
    ] {
        i18n::with_language(language, || {
            ui_text::apply(&ui);
            ui.set_update_title(i18n::message("Version {0} is available.", &["0.3.0"]).into());
            ui.set_update_label(i18n::t("Update and restart").into());
            for (width, height) in [(900u32, 640u32), (1160, 780)] {
                ui.window()
                    .set_size(slint::PhysicalSize::new(width, height));
                let window = (width as f32, height as f32);
                for page in 0..3 {
                    ui.set_page(page);
                    for (name, make, notice) in [
                        ("first run", &first_run as &dyn Fn() -> Reclaim, false),
                        ("phone connected", &with_phone, false),
                        ("scanning", &scanning, false),
                        ("recovering while scanning", &recovering, false),
                        ("update notice", &first_run, true),
                    ] {
                        let mut state = make();
                        state.sync_ui(&ui);
                        ui.set_update_notice(notice);
                        ui.window().take_snapshot().unwrap();
                        assert_controls_clear(
                            &ui,
                            window,
                            &format!("{code} {width}x{height} page {page} {name}"),
                        );
                    }
                }
                ui.set_update_notice(false);
                ui.set_page(1);
                // The list header never elides, so counts in the hundreds of
                // thousands must still fit the narrow list column.
                scanning().sync_ui(&ui);
                let count = "250000";
                ui.set_summary(i18n::message("Found: {0} · Selected: {1}", &[count, count]).into());
                ui.window().take_snapshot().unwrap();
                assert_controls_clear(
                    &ui,
                    window,
                    &format!("{code} {width}x{height} page 1 large counts"),
                );
                // Both labels are at least as wide as their text, so text that
                // does not fit pokes out of the list column instead of eliding.
                let element = |id: &str| {
                    i_slint_backend_testing::ElementHandle::find_by_element_id(&ui, id)
                        .next()
                        .unwrap_or_else(|| panic!("{id}"))
                };
                let list = element("AppWindow::files-view");
                let (left, right) = (
                    list.absolute_position().x,
                    list.absolute_position().x + list.size().width,
                );
                for id in ["AppWindow::select-all-caption", "AppWindow::list-summary"] {
                    let label = element(id);
                    let start = label.absolute_position().x;
                    let end = start + label.size().width;
                    assert!(
                        start >= left - 0.5 && end <= right + 0.5,
                        "{code} {width}x{height}: {id} ({start}..{end}) is cut off by the list \
                         column ({left}..{right})"
                    );
                }
                let mut state = scanning();
                let mut steady = |ui: &AppWindow| {
                    state.sync_ui(ui);
                    ui.window().take_snapshot().unwrap();
                };
                let mut slow_check = |ui: &AppWindow| {
                    ui.set_status(i18n::message("Checking a {0} file…", &["ZIP"]).into());
                    ui.window().take_snapshot().unwrap();
                };
                let mut recovering_now = |ui: &AppWindow| {
                    recovering().sync_ui(ui);
                    ui.window().take_snapshot().unwrap();
                };
                assert_steady(
                    &ui,
                    "ScanStatus::headline",
                    &format!("{code} {width}x{height}"),
                    &mut [
                        ("scanning", &mut steady),
                        ("slow check", &mut slow_check),
                        ("recovering", &mut recovering_now),
                    ],
                );
                // Recovery messages replace the hint without moving anything.
                let message = |text: &'static str| {
                    move |ui: &AppWindow| {
                        let mut state = recovering();
                        state.recovery_status = text.into();
                        state.sync_ui(ui);
                        ui.window().take_snapshot().unwrap();
                    }
                };
                let mut hint = |ui: &AppWindow| {
                    scanning().sync_ui(ui);
                    ui.window().take_snapshot().unwrap();
                };
                assert_steady(
                    &ui,
                    "AppWindow::recovery-message",
                    &format!("{code} {width}x{height}"),
                    &mut [
                        ("hint", &mut hint),
                        ("recovering", &mut message("Recovering selected files…")),
                        (
                            "saved",
                            &mut message(
                                "Saved 1 file. Open the destination folder and verify their contents.",
                            ),
                        ),
                        (
                            "warning",
                            &mut message(
                                "Saving here can overwrite the deleted files you are trying to recover and damage them permanently. Choose a folder on a different physical drive, such as a USB drive.",
                            ),
                        ),
                    ],
                );
                // Selected files can be recovered mid-scan; the button then stops it.
                scanning().sync_ui(&ui);
                assert!(ui.get_can_recover(), "{code}: recover during a scan");
                recovering().sync_ui(&ui);
                ui.window().take_snapshot().unwrap();
                assert!(ui.get_can_recover() && ui.get_busy());
                assert!(
                    i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                        &ui,
                        &i18n::t("Stop recovery"),
                    )
                    .next()
                    .is_some(),
                    "{code}: the recover button offers to stop"
                );
                if let Ok(directory) = std::env::var("RECLAIM_PREVIEW_DIR") {
                    let mut state = recovering();
                    state.recovery_status =
                        "Saved 1 file. Open the destination folder and verify their contents."
                            .into();
                    state.sync_ui(&ui);
                    let snapshot = ui.window().take_snapshot().unwrap();
                    image::save_buffer(
                        format!("{directory}/recovering-{code}-{width}.png"),
                        snapshot.as_bytes(),
                        snapshot.width(),
                        snapshot.height(),
                        image::ColorType::Rgba8,
                    )
                    .unwrap();
                }
            }
        });
    }
}

#[test]
fn package_managed_updates_hide_the_native_installer_action() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    ui_text::apply(&ui);
    ui.set_page(2);
    ui.set_updates_managed(true);
    ui.set_update_status(i18n::t("Updates are managed by Flatpak.").into());
    ui.show().unwrap();
    ui.window().take_snapshot().unwrap();
    assert!(
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            &i18n::t("Check for updates"),
        )
        .next()
        .is_none()
    );
    ui.set_updates_managed(false);
    ui.window().take_snapshot().unwrap();
    assert!(
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(
            &ui,
            &i18n::t("Check for updates"),
        )
        .next()
        .is_some()
    );
}

#[test]
fn windows_drive_access_failure_has_an_actionable_aligned_restart_button() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .unwrap();
    let ui = AppWindow::new().unwrap();
    let restarted = Rc::new(std::cell::Cell::new(false));
    let flag = restarted.clone();
    ui.on_restart_as_administrator(move || flag.set(true));
    ui.show().unwrap();
    for (code, language) in [
        ("en", i18n::Language::English),
        ("he", i18n::Language::Hebrew),
    ] {
        i18n::with_language(language, || {
            ui_text::apply(&ui);
            ui.set_page(1);
            ui.set_show_status(true);
            ui.set_status(i18n::t("Scan failed. See the message above.").into());
            ui.set_error(i18n::t("Windows denied access. For physical drives, restart Reclaim with Run as administrator, or open a disk image. BitLocker volumes must be unlocked first.").into());
            ui.set_needs_administrator(true);
            ui.window().set_size(slint::PhysicalSize::new(900, 640));
            let snapshot = ui.window().take_snapshot().unwrap();
            crate::layout_checks::assert_controls_clear(&ui, (900., 640.), code);
            let action = i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                &ui,
                &i18n::t("Restart as administrator"),
            )
            .next()
            .expect("restart action for denied drive access");
            action.invoke_accessible_default_action();
            assert!(restarted.replace(false));
            if let Ok(directory) = std::env::var("RECLAIM_PREVIEW_DIR") {
                std::fs::create_dir_all(&directory).unwrap();
                image::save_buffer(
                    format!("{directory}/access-denied-{code}.png"),
                    snapshot.as_bytes(),
                    snapshot.width(),
                    snapshot.height(),
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }
            ui.set_needs_administrator(false);
            ui.window().take_snapshot().unwrap();
            assert!(
                i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                    &ui,
                    &i18n::t("Restart as administrator"),
                )
                .next()
                .is_none()
            );
        });
    }
}

#[test]
fn destination_review_offers_a_safe_choice_and_never_accepts_the_source_drive() {
    use crate::layout_checks::assert_controls_clear;
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .ok();
    let ui = AppWindow::new().unwrap();
    ui.show().unwrap();
    ui.set_page(1);
    let folder = PathBuf::from("/media/user/Backup drive with a long name/Recovered photos");
    let button = |ui: &AppWindow, label: &str| {
        i_slint_backend_testing::ElementHandle::find_by_accessible_label(ui, &i18n::t(label))
            .next()
            .is_some()
    };
    for (code, language) in [
        ("en", i18n::Language::English),
        ("he", i18n::Language::Hebrew),
        ("ar", i18n::Language::Arabic),
        ("de", i18n::Language::German),
        ("es", i18n::Language::Spanish),
        ("fr", i18n::Language::French),
        ("ru", i18n::Language::Russian),
    ] {
        i18n::with_language(language, || {
            ui_text::apply(&ui);
            for (width, height) in [(900u32, 640u32), (1160, 780)] {
                ui.window()
                    .set_size(slint::PhysicalSize::new(width, height));
                for (kind, destination) in [
                    ("same-drive", drives::Destination::SameDrive),
                    ("unknown", drives::Destination::Unknown),
                ] {
                    let mut state = Reclaim::default();
                    state.destination_review = Some((folder.clone(), destination));
                    state.sync_ui(&ui);
                    let snapshot = ui.window().take_snapshot().unwrap();
                    let context = format!("{code} {width}x{height} {kind} destination");
                    assert!(ui.get_destination_review(), "{context}");
                    assert!(ui.get_destination_path().contains("Recovered photos"));
                    assert!(button(&ui, "Choose another folder"), "{context}");
                    assert!(button(&ui, "Cancel"), "{context}");
                    assert_eq!(
                        button(&ui, "Save here anyway"),
                        destination == drives::Destination::Unknown,
                        "{context}: only an undetermined drive can be accepted"
                    );
                    assert_controls_clear(&ui, (width as f32, height as f32), &context);
                    // Dialog buttons never elide, so each must fit inside the card.
                    let card = i_slint_backend_testing::ElementHandle::find_by_element_id(
                        &ui,
                        "AppWindow::destination-card",
                    )
                    .next()
                    .expect("destination card");
                    let (left, right) = (
                        card.absolute_position().x,
                        card.absolute_position().x + card.size().width,
                    );
                    for label in ["Cancel", "Save here anyway", "Choose another folder"] {
                        for action in
                            i_slint_backend_testing::ElementHandle::find_by_accessible_label(
                                &ui,
                                &i18n::t(label),
                            )
                        {
                            let start = action.absolute_position().x;
                            let end = start + action.size().width;
                            assert!(
                                start >= left && end <= right,
                                "{context}: {label} ({start}..{end}) is outside the dialog ({left}..{right})"
                            );
                        }
                    }
                    if let Ok(directory) = std::env::var("RECLAIM_PREVIEW_DIR") {
                        image::save_buffer(
                            format!("{directory}/destination-{kind}-{code}-{width}.png"),
                            snapshot.as_bytes(),
                            snapshot.width(),
                            snapshot.height(),
                            image::ColorType::Rgba8,
                        )
                        .unwrap();
                    }
                }
            }
        });
    }
    // Escape cancels.
    let cancelled = std::rc::Rc::new(std::cell::Cell::new(false));
    ui.on_cancel_destination({
        let cancelled = cancelled.clone();
        move || cancelled.set(true)
    });
    ui.set_destination_review(false);
    ui.window().take_snapshot().unwrap();
    ui.set_destination_review(true);
    ui.window().take_snapshot().unwrap();
    let escape: slint::SharedString = slint::platform::Key::Escape.into();
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyPressed {
            text: escape.clone(),
        });
    ui.window()
        .dispatch_event(slint::platform::WindowEvent::KeyReleased { text: escape });
    assert!(cancelled.get(), "Escape closes the destination dialog");
    // "Save here anyway" never writes onto the scanned drive, even if invoked
    // for a same-drive folder; the dialog just closes.
    let mut state = Reclaim::default();
    state.destination_review = Some((folder.clone(), drives::Destination::SameDrive));
    state.save_to_unconfirmed_destination();
    assert!(!state.exporting && state.destination_review.is_none());
}

#[test]
fn finished_scan_message_wraps_at_the_full_width() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            ..Default::default()
        },
    )))
    .ok();
    let ui = AppWindow::new().unwrap();
    ui.show().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(900, 640));
    ui.set_page(1);
    i18n::with_language(i18n::Language::German, || {
        ui_text::apply(&ui);
        let mut state = Reclaim::default();
        state.status = "Scan finished with unreadable areas. Some files could not be checked. The drive may be failing: recover the files you need now and avoid repeated scans.".into();
        state.sync_ui(&ui);
        let snapshot = ui.window().take_snapshot().unwrap();
        let headline =
            i_slint_backend_testing::ElementHandle::find_by_element_id(&ui, "ScanStatus::headline")
                .next()
                .expect("scan headline");
        // Wrapped at the row's width this is two or three lines, not a word per line.
        assert!(headline.size().width > 300.0, "{:?}", headline.size());
        assert!(headline.size().height < 70.0, "{:?}", headline.size());
        if let Ok(directory) = std::env::var("RECLAIM_PREVIEW_DIR") {
            image::save_buffer(
                format!("{directory}/status-de-finished.png"),
                snapshot.as_bytes(),
                snapshot.width(),
                snapshot.height(),
                image::ColorType::Rgba8,
            )
            .unwrap();
        }
    });
}

#[test]
fn stop_buttons_are_filled_red_in_light_and_dark() {
    slint::platform::set_platform(Box::new(i_slint_backend_testing::TestingBackend::new(
        i_slint_backend_testing::TestingBackendOptions {
            renderer_name: Some("software".into()),
            mock_time: true,
            ..Default::default()
        },
    )))
    .ok();
    let ui = AppWindow::new().unwrap();
    ui.show().unwrap();
    ui.window().set_size(slint::PhysicalSize::new(900, 640));
    ui_text::apply(&ui);
    ui.set_page(1);
    let mut state = Reclaim::default();
    state.results.push(Found {
        offset: 0,
        size: 2048,
        kind: "jpg",
        selected: true,
        original_name: None,
        resident_data: None,
        ntfs: None,
        extents: None,
        validation: reclaim::Validation::StructureChecked,
    });
    state.busy = true;
    state.exporting = true;
    state.status = "Scanning for recoverable files…".into();
    for dark in [false, true] {
        ui.global::<Palette>().set_theme(if dark { 2 } else { 1 });
        state.sync_ui(&ui);
        ui.window().take_snapshot().unwrap();
        // Let the button's background animation finish.
        i_slint_backend_testing::mock_elapsed_time(Duration::from_millis(300));
        slint::platform::update_timers_and_animations();
        let snapshot = ui.window().take_snapshot().unwrap();
        for label in ["Stop", "Stop recovery"] {
            let button =
                i_slint_backend_testing::ElementHandle::find_by_accessible_label(&ui, label)
                    .next()
                    .unwrap_or_else(|| panic!("{label} button"));
            // A background pixel inside the button, clear of its icon and caption.
            let position = button.absolute_position();
            #[expect(
                clippy::cast_possible_truncation,
                clippy::cast_sign_loss,
                reason = "an on-screen position in a 900x640 snapshot is small and positive"
            )]
            let (x, y) = ((position.x + 5.0) as u32, (position.y + 5.0) as u32);
            let offset = ((y * snapshot.width() + x) * 4) as usize;
            let pixel = &snapshot.as_bytes()[offset..offset + 3];
            let (red, green, blue) = (
                i32::from(pixel[0]),
                i32::from(pixel[1]),
                i32::from(pixel[2]),
            );
            assert!(
                red > green + 80 && red > blue + 80,
                "{label} is not filled red (dark: {dark}): {pixel:?}"
            );
        }
    }
}
