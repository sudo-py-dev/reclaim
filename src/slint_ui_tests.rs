use super::*;

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
        });
    }
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
