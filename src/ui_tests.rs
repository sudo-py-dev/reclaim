use super::*;
use egui::{Pos2, Rect};
use std::io::Write;

fn input(size: [f32; 2]) -> egui::RawInput {
    egui::RawInput {
        screen_rect: Some(Rect::from_min_size(Pos2::ZERO, size.into())),
        ..Default::default()
    }
}

#[test]
fn pages_render_at_minimum_and_default_sizes_in_both_themes() {
    for dark in [false, true] {
        for size in [[540.0, 580.0], [920.0, 740.0]] {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            ctx.set_theme(if dark {
                egui::Theme::Dark
            } else {
                egui::Theme::Light
            });
            let mut app = Reclaim::default();
            if drives::AVAILABLE {
                app.drives = vec![
                    drives::Drive {
                        path: "/dev/nvme0n1".into(),
                        name: "Internal SSD".into(),
                        name_is_message: false,
                        size: 512_000_000_000,
                        kind: "Entire drive".into(),
                        location: "Contains mounted volumes".into(),
                        mounted: true,
                        encrypted: false,
                        sector_size: 1,
                    },
                    drives::Drive {
                        path: "/dev/sdb".into(),
                        name: "Kingston USB drive".into(),
                        name_is_message: false,
                        size: 64_000_000_000,
                        kind: "USB drive".into(),
                        location: "Contains mounted volumes".into(),
                        mounted: true,
                        encrypted: false,
                        sector_size: 1,
                    },
                    drives::Drive {
                        path: "/dev/sdb1".into(),
                        name: "My Photos".into(),
                        name_is_message: false,
                        size: 63_000_000_000,
                        kind: "Volume".into(),
                        location: "Mounted at /media/me/My Photos".into(),
                        mounted: true,
                        encrypted: false,
                        sector_size: 1,
                    },
                ];
                app.source = Some("/dev/sdb1".into());
            }
            for page in 0..6 {
                app.page = if page >= 3 { 1 } else { page };
                if page == 1 {
                    app.scanned_source = Some(PathBuf::from("/media/Backup/drive.img"));
                    app.status = "Scan complete. 200 file candidates found.".into();
                    app.results = (0..200)
                        .map(|i| Found {
                            offset: i * 4096,
                            size: 184256,
                            kind: "png",
                            selected: i % 2 == 0,
                            validation: reclaim::Validation::ImageDecoded,
                            original_name: None,
                            resident_data: None,
                            ntfs: None,
                        })
                        .collect();
                }
                if page >= 3 {
                    app.busy = page != 5;
                    app.exporting = false;
                    app.results.clear();
                    app.total = if page == 3 { 1_000_000_000 } else { 0 };
                    app.scanned = if page == 3 { 250_000_000 } else { 0 };
                    app.source_handle = if page == 3 {
                        Some(std::sync::Arc::new(
                            std::fs::File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"))
                                .unwrap(),
                        ))
                    } else {
                        None
                    };
                    app.started =
                        Some(std::time::Instant::now() - std::time::Duration::from_secs(10));
                    app.status = if page == 3 {
                        "Scanning for recoverable files…"
                    } else if page == 5 {
                        "Scan complete. No file candidates found."
                    } else {
                        "Opening source…"
                    }
                    .into();
                }
                let mut output = None;
                for _ in 0..3 {
                    let mut frame = ctx.run_ui(input(size), |ui| app.render(ui));
                    frame.textures_delta.clear();
                    output = Some(frame);
                }
                let output = output.unwrap();
                assert!(!output.shapes.is_empty());
                assert!(
                    output.viewport_output[&egui::ViewportId::ROOT]
                        .commands
                        .is_empty()
                );
                if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
                    if std::env::var_os("RECLAIM_PREVIEW_LOADING_ONLY").is_some() && page < 3 {
                        continue;
                    }
                    let filename = format!(
                        "{dir}/page-{page}-{}-{}.ppm",
                        if dark { "dark" } else { "light" },
                        size[0]
                    );
                    rasterize(&ctx, output, size, &filename);
                }
            }
        }
    }
}

// CPU-only inspection of the actual egui meshes. This deliberately does not
// pretend to test a compositor, native dragging, or desktop window management.
fn rasterize(ctx: &egui::Context, output: egui::FullOutput, size: [f32; 2], path: &str) {
    let texture = ctx.fonts(|fonts| fonts.image());
    let (width, height) = (size[0] as usize, size[1] as usize);
    let mut pixels = vec![[246.0f32; 3]; width * height];
    let primitives = ctx.tessellate(output.shapes, output.pixels_per_point);
    for primitive in primitives {
        let egui::epaint::Primitive::Mesh(mesh) = primitive.primitive else {
            continue;
        };
        assert_eq!(mesh.texture_id, egui::TextureId::Managed(0));
        for tri in mesh.indices.chunks(3) {
            let v = [
                mesh.vertices[tri[0] as usize],
                mesh.vertices[tri[1] as usize],
                mesh.vertices[tri[2] as usize],
            ];
            let edge =
                |a: Pos2, b: Pos2, p: Pos2| (b.x - a.x) * (p.y - a.y) - (b.y - a.y) * (p.x - a.x);
            let area = edge(v[0].pos, v[1].pos, v[2].pos);
            if area.abs() < 0.00001 {
                continue;
            }
            let x0 = v
                .iter()
                .map(|v| v.pos.x)
                .fold(f32::INFINITY, f32::min)
                .max(primitive.clip_rect.left())
                .max(0.0)
                .floor() as usize;
            let y0 = v
                .iter()
                .map(|v| v.pos.y)
                .fold(f32::INFINITY, f32::min)
                .max(primitive.clip_rect.top())
                .max(0.0)
                .floor() as usize;
            let x1 = v
                .iter()
                .map(|v| v.pos.x)
                .fold(0.0, f32::max)
                .min(primitive.clip_rect.right())
                .min(width as f32)
                .ceil() as usize;
            let y1 = v
                .iter()
                .map(|v| v.pos.y)
                .fold(0.0, f32::max)
                .min(primitive.clip_rect.bottom())
                .min(height as f32)
                .ceil() as usize;
            for y in y0..y1 {
                for x in x0..x1 {
                    let p = egui::pos2(x as f32 + 0.5, y as f32 + 0.5);
                    if !primitive.clip_rect.contains(p) {
                        continue;
                    }
                    let weights = [
                        edge(v[1].pos, v[2].pos, p) / area,
                        edge(v[2].pos, v[0].pos, p) / area,
                        edge(v[0].pos, v[1].pos, p) / area,
                    ];
                    if weights.iter().any(|w| *w < -0.00001) {
                        continue;
                    }
                    let uv = (0..3).fold(egui::Vec2::ZERO, |sum, i| {
                        sum + v[i].uv.to_vec2() * weights[i]
                    });
                    let tx = (uv.x * texture.size[0] as f32)
                        .clamp(0.0, texture.size[0] as f32 - 1.0)
                        as usize;
                    let ty = (uv.y * texture.size[1] as f32)
                        .clamp(0.0, texture.size[1] as f32 - 1.0)
                        as usize;
                    let tex = texture.pixels[ty * texture.size[0] + tx].to_array();
                    let vertex: [f32; 4] = std::array::from_fn(|c| {
                        (0..3)
                            .map(|i| v[i].color.to_array()[c] as f32 * weights[i])
                            .sum()
                    });
                    let alpha = vertex[3] * tex[3] as f32 / (255.0 * 255.0);
                    for c in 0..3 {
                        pixels[y * width + x][c] = vertex[c] * tex[c] as f32 / 255.0
                            + pixels[y * width + x][c] * (1.0 - alpha);
                    }
                }
            }
        }
    }
    let mut file = std::fs::File::create(path).unwrap();
    write!(file, "P6\n{width} {height}\n255\n").unwrap();
    for pixel in pixels {
        file.write_all(&pixel.map(|c| c.clamp(0.0, 255.0) as u8))
            .unwrap();
    }
}

#[test]
#[cfg(not(target_os = "macos"))]
fn close_button_is_not_intercepted_by_the_title_drag_region() {
    let ctx = egui::Context::default();
    theme::setup(&ctx);
    let mut app = Reclaim::default();
    for _ in 0..3 {
        ctx.run_ui(input([920.0, 740.0]), |ui| app.render(ui))
            .drop_without_applying_deltas();
    }
    let pos = egui::pos2(892.0, 28.0);
    for pressed in [true, false] {
        let mut frame_input = input([920.0, 740.0]);
        frame_input.events = vec![
            egui::Event::PointerMoved(pos),
            egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed,
                modifiers: Default::default(),
            },
        ];
        let mut output = ctx.run_ui(frame_input, |ui| app.render(ui));
        output.textures_delta.clear();
        if !pressed {
            assert_eq!(
                output.viewport_output[&egui::ViewportId::ROOT].commands,
                vec![egui::ViewportCommand::Close]
            );
        }
    }
}

#[test]
fn drive_rows_activate_from_the_whole_row_and_keyboard() {
    let ctx = egui::Context::default();
    theme::setup(&ctx);
    let mut center = Pos2::ZERO;
    let mut render = |events: Vec<egui::Event>| {
        let mut clicked = false;
        let mut frame_input = input([600.0, 400.0]);
        frame_input.events = events;
        let output = ctx.run_ui(frame_input, |ui| {
            let response = theme::drive_row(
                ui,
                "USB Drive",
                "Not mounted",
                "USB drive",
                "64 GiB",
                false,
                true,
            );
            center = response.rect.right_center() - egui::vec2(10.0, 0.0);
            clicked = response.clicked();
        });
        output.drop_without_applying_deltas();
        (clicked, center)
    };
    let (_, pos) = render(vec![]);
    render(vec![
        egui::Event::PointerMoved(pos),
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: true,
            modifiers: Default::default(),
        },
    ]);
    assert!(
        render(vec![egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed: false,
            modifiers: Default::default()
        }])
        .0
    );
    let key = |key| egui::Event::Key {
        key,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    };
    // Standard keyboard traversal must reach the row and allow activation.
    render(vec![key(egui::Key::Tab)]);
    assert!(render(vec![key(egui::Key::Space)]).0);
}

#[test]
fn hebrew_pages_use_real_glyphs_and_preserve_data_and_queries() {
    reclaim::i18n::with_language(reclaim::i18n::Language::Hebrew, || {
        for width in [540.0, 920.0] {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            ctx.set_theme(egui::Theme::Dark);
            let mut app = Reclaim::default();
            app.drives = vec![drives::Drive {
                path: "/dev/sdb1".into(),
                name: "תמונות 2026".into(),
                name_is_message: false,
                size: 64_000_000_000,
                kind: "Volume".into(),
                location: "Mounted at /media/user/Photos".into(),
                mounted: false,
                encrypted: false,
                sector_size: 1,
            }];
            app.source = Some("/dev/sdb1".into());
            for page in 0..3 {
                app.page = page;
                if page == 1 {
                    app.scanned_source = app.source.clone();
                    app.results = vec![Found {
                        offset: 123456,
                        size: 1024,
                        kind: "png",
                        selected: true,
                        validation: reclaim::Validation::ImageDecoded,
                        original_name: Some("תמונה 12.png".into()),
                        resident_data: None,
                        ntfs: None,
                    }];
                    app.status = "Scan complete. 1 file candidates found.".into();
                    app.filter = "תמונה".into();
                }
                let mut output = None;
                for _ in 0..3 {
                    let mut frame = ctx.run_ui(input([width, 740.0]), |ui| app.render(ui));
                    frame.textures_delta.clear();
                    output = Some(frame);
                }
                let output = output.unwrap();
                // epaint 0.36's has_glyph compares face IDs with the replacement
                // face, yielding false negatives for a primary DejaVu face.
                assert!(ctx.fonts_mut(|fonts| {
                    fonts
                        .layout_no_wrap(
                            "שלום".into(),
                            egui::FontId::proportional(14.0),
                            egui::Color32::WHITE,
                        )
                        .size()
                        .x
                        > 20.0
                }));
                assert_eq!(app.drives[0].name, "תמונות 2026");
                if page == 1 {
                    assert_eq!(app.filter, "תמונה");
                    assert_eq!(
                        app.results[0].original_name.as_deref(),
                        Some("תמונה 12.png")
                    );
                }
                if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
                    rasterize(
                        &ctx,
                        output,
                        [width, 740.0],
                        &format!("{dir}/he-page-{page}-{width}.ppm"),
                    );
                }
            }
        }
    });
}

#[test]
fn hebrew_header_settings_button_opens_settings_directly() {
    i18n::with_language(i18n::Language::Hebrew, || {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut app = Reclaim::default();
        let size = [1440.0, 900.0];
        for _ in 0..3 {
            ctx.run_ui(input(size), |ui| app.render(ui))
                .drop_without_applying_deltas();
        }
        // macOS uses native traffic lights, so the RTL Settings button is the
        // leftmost header action. Custom decorations reserve 120 px before it.
        let x = if window::CUSTOM_DECORATIONS {
            148.0
        } else {
            30.0
        };
        let pos = egui::pos2(x, 26.0);
        for pressed in [true, false] {
            let mut frame_input = input(size);
            frame_input.events = vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ];
            ctx.run_ui(frame_input, |ui| app.render(ui))
                .drop_without_applying_deltas();
        }
        let mut output = ctx.run_ui(input(size), |ui| app.render(ui));
        output.textures_delta.clear();
        assert!(
            app.settings_open,
            "header button should open settings directly"
        );
        if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
            rasterize(&ctx, output, size, &format!("{dir}/he-header-settings.ppm"));
        }
    });
}

#[test]
#[cfg(not(target_os = "macos"))]
fn hebrew_close_control_is_at_the_outer_left_edge() {
    i18n::with_language(i18n::Language::Hebrew, || {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut app = Reclaim::default();
        for _ in 0..3 {
            ctx.run_ui(input([1120.0, 780.0]), |ui| app.render(ui))
                .drop_without_applying_deltas();
        }
        let pos = egui::pos2(28.0, 28.0);
        for pressed in [true, false] {
            let mut frame_input = input([1120.0, 780.0]);
            frame_input.events = vec![
                egui::Event::PointerMoved(pos),
                egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ];
            let output = ctx.run_ui(frame_input, |ui| app.render(ui));
            if !pressed {
                assert!(
                    output.viewport_output[&egui::ViewportId::ROOT]
                        .commands
                        .contains(&egui::ViewportCommand::Close)
                );
            }
            output.drop_without_applying_deltas();
        }
    });
}

#[test]
fn long_drive_list_keeps_disk_image_action_visible() {
    i18n::with_language(i18n::Language::English, || {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut app = Reclaim::default();
        app.drives = (0..60)
            .map(|index| drives::Drive {
                path: format!("/dev/drive{index}").into(),
                name: format!("USB Drive {index}"),
                name_is_message: false,
                size: 64_000_000_000,
                kind: "USB drive".into(),
                location: "Not mounted".into(),
                mounted: false,
                encrypted: false,
                sector_size: 1,
            })
            .collect();
        let mut output = None;
        for _ in 0..3 {
            let mut frame = ctx.run_ui(input([540.0, 580.0]), |ui| app.render(ui));
            frame.textures_delta.clear();
            output = Some(frame);
        }
        let output = output.unwrap();
        let action = output
            .shapes
            .iter()
            .find_map(|shape| {
                if let egui::Shape::Text(text) = &shape.shape {
                    (text.galley.job.text == "\u{f56d}").then_some((text.pos, shape.clip_rect))
                } else {
                    None
                }
            })
            .expect("disk image action is rendered");
        assert!(action.0.y < 500.0, "drive list displaced the fixed action");
        assert!(action.1.contains(action.0), "disk image action is clipped");
    });
}

#[test]
fn settings_dialog_renders_in_both_languages_and_closes_with_escape() {
    for language in [i18n::Language::English, i18n::Language::Hebrew] {
        i18n::with_language(language, || {
            for (dark, size, category) in [false, true]
                .into_iter()
                .flat_map(|dark| [[540.0, 580.0], [1120.0, 780.0]].map(move |size| (dark, size)))
                .flat_map(|(dark, size)| (0..3).map(move |category| (dark, size, category)))
            {
                let ctx = egui::Context::default();
                theme::setup(&ctx);
                ctx.set_theme(if dark {
                    egui::Theme::Dark
                } else {
                    egui::Theme::Light
                });
                let mut app = Reclaim::default();
                app.settings_open = true;
                app.settings_page = category;
                app.updates = update_ui::Updates::preview_available();
                let mut output = None;
                for step in 0..4 {
                    let mut frame_input = input(size);
                    frame_input.time = Some(step as f64 * 0.25);
                    let mut frame = ctx.run_ui(frame_input, |ui| app.render(ui));
                    frame.textures_delta.clear();
                    output = Some(frame);
                }
                assert!(app.settings_open);
                if category == 2 {
                    let position = |label: &str| {
                        output
                            .as_ref()
                            .unwrap()
                            .shapes
                            .iter()
                            .find_map(|shape| {
                                if let egui::Shape::Text(text) = &shape.shape {
                                    (text.galley.job.text == label).then_some(text.pos.y)
                                } else {
                                    None
                                }
                            })
                            .unwrap_or_else(|| panic!("Missing About text: {label}"))
                    };
                    let version_y = position(env!("CARGO_PKG_VERSION"));
                    let license_y = position("MIT");
                    assert!(
                        license_y - version_y < 140.0,
                        "License row must stay near the description"
                    );
                    if language == i18n::Language::English {
                        assert!(
                            position("Project website") - license_y < 60.0,
                            "Website must follow the license without a large gap"
                        );
                    }
                }
                if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
                    rasterize(
                        &ctx,
                        output.unwrap(),
                        size,
                        &format!(
                            "{dir}/settings-{language:?}-{}-{}-{category}.ppm",
                            if dark { "dark" } else { "light" },
                            size[0]
                        ),
                    );
                }
                let mut frame_input = input(size);
                frame_input.events.push(egui::Event::Key {
                    key: egui::Key::Escape,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: Default::default(),
                });
                ctx.run_ui(frame_input, |ui| app.render(ui))
                    .drop_without_applying_deltas();
                assert!(!app.settings_open);
            }
        });
    }
}

#[test]
fn recovery_exports_only_selected_files_matching_current_filters() {
    for (case, category, query, expected) in [
        (0, 1, "", vec!["photo.png"]),
        (1, 3, "", vec!["backup.zip"]),
        (2, 0, "PHOTO", vec!["photo.png"]),
        (3, 3, "photo", vec![]),
        (4, 0, "", vec!["backup.zip", "photo.png"]),
        (5, 2, "", vec![]),
    ] {
        let directory = std::env::temp_dir().join(format!(
            "reclaim-filter-export-{}-{case}",
            std::process::id()
        ));
        std::fs::create_dir_all(&directory).unwrap();
        let source = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml"));
        let mut app = Reclaim::default();
        app.scanned_source = Some(source.clone());
        app.source_handle = Some(Arc::new(std::fs::File::open(source).unwrap()));
        app.category = category;
        app.filter = query.into();
        app.results = [
            ("png", "photo.png", true),
            ("zip", "backup.zip", true),
            ("pdf", "notes.pdf", false),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (kind, name, selected))| Found {
            offset: i as u64 * 4096,
            size: 4,
            kind,
            selected,
            validation: reclaim::Validation::MetadataChecked,
            original_name: Some(name.into()),
            resident_data: Some(Arc::from(&b"data"[..])),
            ntfs: None,
        })
        .collect();
        app.export(directory.clone());
        if expected.is_empty() {
            assert!(!app.busy);
            assert!(app.receiver.is_none());
        } else {
            let result = app
                .receiver
                .as_ref()
                .unwrap()
                .recv_timeout(std::time::Duration::from_secs(5))
                .unwrap();
            assert!(matches!(result, Message::Export(Ok(count)) if count == expected.len()));
        }
        let mut actual: Vec<_> = std::fs::read_dir(&directory)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        actual.sort();
        assert_eq!(actual, expected, "category={category}, query={query}");
        for name in actual {
            assert_eq!(std::fs::read(directory.join(name)).unwrap(), b"data");
        }
        std::fs::remove_dir_all(directory).unwrap();
    }
}

#[test]
fn guide_next_steps_finish_at_source_selection() {
    i18n::with_language(i18n::Language::English, || {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut app = Reclaim::default();
        app.page = 2;
        for step in 0..4 {
            let mut output = None;
            for _ in 0..3 {
                let mut frame = ctx.run_ui(input([540.0, 580.0]), |ui| app.render(ui));
                frame.textures_delta.clear();
                output = Some(frame);
            }
            assert_eq!(app.guide_step, step);
            let output = output.unwrap();
            if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
                // Capture separately below because rasterization consumes the frame.
                let mut frame = ctx.run_ui(input([540.0, 580.0]), |ui| app.render(ui));
                frame.textures_delta.clear();
                rasterize(
                    &ctx,
                    frame,
                    [540.0, 580.0],
                    &format!("{dir}/guide-step-{step}.ppm"),
                );
            }
            let label = if step == 3 {
                "Choose a drive"
            } else {
                "Next step"
            };
            let pos = output
                .shapes
                .iter()
                .find_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        (text.galley.job.text == label)
                            .then_some(text.pos + text.galley.size() / 2.0)
                    } else {
                        None
                    }
                })
                .expect("step action is visible");
            assert!(pos.y < 580.0);
            for pressed in [true, false] {
                let mut frame_input = input([540.0, 580.0]);
                frame_input.events = vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ];
                ctx.run_ui(frame_input, |ui| app.render(ui))
                    .drop_without_applying_deltas();
            }
        }
        assert_eq!(app.page, 0);
    });
}

#[test]
fn animation_switch_supports_keyboard_activation_in_both_directions() {
    for language in [i18n::Language::English, i18n::Language::Hebrew] {
        i18n::with_language(language, || {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let mut enabled = true;
            ctx.run_ui(input([540.0, 580.0]), |ui| {
                theme::switch(ui, &mut enabled, "Enable animations").request_focus();
            })
            .drop_without_applying_deltas();
            let mut frame_input = input([540.0, 580.0]);
            frame_input.events.push(egui::Event::Key {
                key: egui::Key::Space,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            });
            ctx.run_ui(frame_input, |ui| {
                assert!(theme::switch(ui, &mut enabled, "Enable animations").changed());
            })
            .drop_without_applying_deltas();
            assert!(!enabled);
        });
    }
}

#[test]
fn drive_list_keeps_visible_rows_when_selecting_and_after_stopping() {
    i18n::with_language(i18n::Language::English, || {
        for size in [[540.0, 580.0], [1120.0, 780.0]] {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let mut app = Reclaim::default();
            app.drives = (0..12)
                .map(|index| drives::Drive {
                    path: format!("/dev/test{index}").into(),
                    name: format!("Test drive {index}"),
                    name_is_message: false,
                    size: 64_000_000_000,
                    kind: "USB drive".into(),
                    location: "Contains mounted volumes".into(),
                    mounted: true,
                    encrypted: false,
                    sector_size: 512,
                })
                .collect();
            let mut initial_rows = None;
            for state in 0..3 {
                if state > 0 {
                    app.source = Some(app.drives[0].path.clone());
                }
                if state == 2 {
                    app.scanned_source = app.source.clone();
                    app.scan_stopped = true;
                    app.status = "Scan stopped. You can recover partial results.".into();
                }
                let mut output = None;
                for _ in 0..3 {
                    let mut frame = ctx.run_ui(input(size), |ui| app.render(ui));
                    frame.textures_delta.clear();
                    output = Some(frame);
                }
                let output = output.unwrap();
                let rows = output
                    .shapes
                    .iter()
                    .filter(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            text.galley.job.text.starts_with("Test drive ")
                                && text.pos.y > 100.0
                                && text.pos.y < size[1] - 100.0
                                && shape.clip_rect.contains(text.pos)
                        } else {
                            false
                        }
                    })
                    .count();
                assert!(
                    rows >= 2,
                    "too few visible drives at {size:?}, state {state}"
                );
                assert_eq!(
                    *initial_rows.get_or_insert(rows),
                    rows,
                    "selection/stopping collapsed the drive list"
                );
                if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
                    rasterize(
                        &ctx,
                        output,
                        size,
                        &format!("{dir}/drive-state-{state}-{}.ppm", size[0]),
                    );
                }
            }
        }
    });
}

#[test]
fn stopping_scan_preserves_selected_partial_results() {
    let mut app = Reclaim::default();
    app.busy = true;
    app.page = 1;
    app.cancel.store(true, Ordering::Relaxed);
    let (tx, rx) = mpsc::sync_channel(4);
    app.receiver = Some(rx);
    tx.send(Message::Scan(Event::Found(Found {
        offset: 0,
        size: 4,
        kind: "png",
        selected: true,
        validation: reclaim::Validation::MetadataChecked,
        original_name: Some("photo.png".into()),
        resident_data: Some(Arc::from(&b"data"[..])),
        ntfs: None,
    })))
    .unwrap();
    tx.send(Message::Done(Ok(()))).unwrap();
    app.poll();
    assert!(!app.busy);
    assert!(app.scan_stopped);
    assert_eq!(app.recovery_files().len(), 1);
    assert!(app.status.starts_with("Scan stopped."));
}

#[test]
fn scan_footer_has_one_stop_control_then_enables_recovery() {
    i18n::with_language(i18n::Language::English, || {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut app = Reclaim::default();
        app.page = 1;
        app.busy = true;
        app.total = 100;
        app.scanned = 25;
        app.scanned_source = Some("/dev/example".into());
        app.source_handle = Some(Arc::new(
            std::fs::File::open(concat!(env!("CARGO_MANIFEST_DIR"), "/Cargo.toml")).unwrap(),
        ));
        app.results.push(Found {
            offset: 0,
            size: 4,
            kind: "png",
            selected: true,
            validation: reclaim::Validation::MetadataChecked,
            original_name: Some("photo.png".into()),
            resident_data: Some(Arc::from(&b"data"[..])),
            ntfs: None,
        });
        let size = [540.0, 580.0];
        let frame = |app: &mut Reclaim| {
            let mut output = None;
            for _ in 0..3 {
                let mut frame = ctx.run_ui(input(size), |ui| app.render(ui));
                frame.textures_delta.clear();
                output = Some(frame);
            }
            output.unwrap()
        };
        let text_positions = |output: &egui::FullOutput, label: &str| {
            output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        (text.galley.job.text == label)
                            .then_some(text.pos + text.galley.size() / 2.0)
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>()
        };
        let output = frame(&mut app);
        let stops = text_positions(&output, "Stop");
        assert_eq!(stops.len(), 1);
        assert!(text_positions(&output, "Recover files…").is_empty());
        assert!(stops[0].y > 490.0 && stops[0].y < 580.0);
        if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
            rasterize(&ctx, output, size, &format!("{dir}/scan-footer-active.ppm"));
        }
        for pressed in [true, false] {
            let mut frame_input = input(size);
            frame_input.events = vec![
                egui::Event::PointerMoved(stops[0]),
                egui::Event::PointerButton {
                    pos: stops[0],
                    button: egui::PointerButton::Primary,
                    pressed,
                    modifiers: Default::default(),
                },
            ];
            ctx.run_ui(frame_input, |ui| app.render(ui))
                .drop_without_applying_deltas();
        }
        assert!(app.cancel.load(Ordering::Relaxed));
        app.busy = false;
        app.scan_stopped = true;
        app.status = "Scan stopped. You can recover partial results.".into();
        let output = frame(&mut app);
        assert_eq!(text_positions(&output, "Scan again").len(), 1);
        assert_eq!(text_positions(&output, "Recover files…").len(), 1);
        assert!(text_positions(&output, "Stop").is_empty());
        if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
            rasterize(
                &ctx,
                output,
                size,
                &format!("{dir}/scan-footer-stopped.ppm"),
            );
        }
    });
}

#[test]
fn file_type_icons_open_details_with_dates_and_unknown_deletion_time() {
    for language in [i18n::Language::English, i18n::Language::Hebrew] {
        i18n::with_language(language, || {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let mut app = Reclaim::default();
            app.page = 1;
            app.scanned_source = Some("/media/Backup/drive.img".into());
            app.status = "Scan complete. 6 file candidates found.".into();
            app.results = ["png", "pdf", "docx", "zip", "wav", "avi"]
                .into_iter()
                .enumerate()
                .map(|(index, kind)| Found {
                    offset: index as u64 * 4096,
                    size: 4096,
                    kind,
                    selected: index < 3,
                    validation: reclaim::Validation::Unverified,
                    original_name: None,
                    resident_data: None,
                    ntfs: None,
                })
                .collect();
            app.results[1].original_name = Some("Project notes.pdf".into());
            app.results[1].validation = reclaim::Validation::MetadataChecked;
            app.results[1].ntfs = Some(reclaim::NtfsMetadata {
                created: Some(1_704_067_200),
                modified: Some(1_704_153_600),
                accessed: None,
                record_changed: Some(1_704_240_000),
                record_number: 82,
                sequence_number: 4,
            });
            let size = [1120.0, 780.0];
            let mut output = None;
            for _ in 0..3 {
                let mut frame = ctx.run_ui(input(size), |ui| app.render(ui));
                frame.textures_delta.clear();
                output = Some(frame);
            }
            let output = output.unwrap();
            let positions: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        (text.galley.job.text == "\u{f05a}")
                            .then_some(text.pos + text.galley.size() / 2.0)
                    } else {
                        None
                    }
                })
                .collect();
            assert_eq!(positions.len(), 6);
            if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
                rasterize(
                    &ctx,
                    output,
                    size,
                    &format!("{dir}/file-types-{language:?}.ppm"),
                );
            }
            let pos = positions[1];
            for pressed in [true, false] {
                let mut frame_input = input(size);
                frame_input.events = vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ];
                ctx.run_ui(frame_input, |ui| app.render(ui))
                    .drop_without_applying_deltas();
            }
            assert_eq!(app.details_file, Some(1));
            let mut output = None;
            for step in 0..4 {
                let mut frame_input = input(size);
                frame_input.time = Some(2.0 + step as f64 * 0.25);
                let mut frame = ctx.run_ui(frame_input, |ui| app.render(ui));
                frame.textures_delta.clear();
                output = Some(frame);
            }
            let output = output.unwrap();
            assert_eq!(
                theme::file_date(app.results[1].ntfs.as_ref().unwrap().created),
                "2024-01-01 00:00:00 UTC"
            );
            if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
                rasterize(
                    &ctx,
                    output,
                    size,
                    &format!("{dir}/file-details-{language:?}.ppm"),
                );
            }
            let mut frame_input = input(size);
            frame_input.events.push(egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: Default::default(),
            });
            ctx.run_ui(frame_input, |ui| app.render(ui))
                .drop_without_applying_deltas();
            assert_eq!(app.details_file, None);
        });
    }
}

#[test]
fn short_hover_hints_use_compact_popups_in_both_languages() {
    for language in [i18n::Language::English, i18n::Language::Hebrew] {
        i18n::with_language(language, || {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            ctx.all_styles_mut(|style| {
                style.interaction.tooltip_delay = 0.0;
                style.interaction.show_tooltips_only_when_still = false;
            });
            let size = [540.0, 580.0];
            let mut button = Rect::NOTHING;
            ctx.run_ui(input(size), |ui| {
                ui.with_layout(i18n::column(), |ui| {
                    button = theme::icon_button(
                        ui,
                        theme::Icon::DeselectAll,
                        "Deselect all",
                        false,
                        egui::vec2(36.0, 36.0),
                    )
                    .rect;
                });
            })
            .drop_without_applying_deltas();
            let mut output = None;
            for step in 0..6 {
                let mut frame_input = input(size);
                frame_input.time = Some(1.0 + step as f64 * 0.25);
                frame_input
                    .events
                    .push(egui::Event::PointerMoved(button.center()));
                let mut frame = ctx.run_ui(frame_input, |ui| {
                    ui.with_layout(i18n::column(), |ui| {
                        theme::icon_button(
                            ui,
                            theme::Icon::DeselectAll,
                            "Deselect all",
                            false,
                            egui::vec2(36.0, 36.0),
                        );
                    });
                });
                frame.textures_delta.clear();
                output = Some(frame);
            }
            let output = output.unwrap();
            let mut rects = Vec::new();
            let mut text = None;
            fn inspect(
                shape: &egui::Shape,
                rects: &mut Vec<Rect>,
                text: &mut Option<Pos2>,
                label: &str,
            ) {
                match shape {
                    egui::Shape::Rect(shape) => rects.push(shape.rect),
                    egui::Shape::Text(shape)
                        if shape
                            .galley
                            .job
                            .text
                            .split_whitespace()
                            .collect::<Vec<_>>()
                            .join(" ")
                            == label =>
                    {
                        *text = Some(shape.pos + shape.galley.rect.center().to_vec2())
                    }
                    egui::Shape::Vec(shapes) => {
                        for shape in shapes {
                            inspect(shape, rects, text, label);
                        }
                    }
                    _ => {}
                }
            }
            for shape in &output.shapes {
                inspect(
                    &shape.shape,
                    &mut rects,
                    &mut text,
                    &i18n::t("Deselect all"),
                );
            }
            let position = text.expect("localized tooltip should appear");
            let popup = rects
                .into_iter()
                .filter(|rect| rect.contains(position))
                .min_by(|a, b| a.area().total_cmp(&b.area()))
                .expect("tooltip frame");
            assert!(
                popup.width() < 180.0,
                "short tooltip reserved too much width: {popup:?}"
            );
            assert!(
                popup.height() < 64.0,
                "short tooltip reserved too much height: {popup:?}"
            );
            if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
                rasterize(
                    &ctx,
                    output,
                    size,
                    &format!("{dir}/tooltip-{language:?}.ppm"),
                );
            }
        });
    }
}

#[test]
fn expanded_guide_keeps_step_buttons_and_navigation_fixed() {
    for language in [i18n::Language::English, i18n::Language::Hebrew] {
        i18n::with_language(language, || {
            let ctx = egui::Context::default();
            theme::setup(&ctx);
            let mut app = Reclaim::default();
            app.page = 2;
            app.guide_step = 2;
            let size = [540.0, 580.0];
            let position = |output: &egui::FullOutput, label: &str| {
                output
                    .shapes
                    .iter()
                    .find_map(|shape| {
                        if let egui::Shape::Text(text) = &shape.shape {
                            (text.galley.job.text == label)
                                .then_some(text.pos + text.galley.rect.center().to_vec2())
                        } else {
                            None
                        }
                    })
                    .unwrap_or_else(|| panic!("missing label {label}"))
            };
            let mut output = None;
            for _ in 0..3 {
                let mut frame = ctx.run_ui(input(size), |ui| app.render(ui));
                frame.textures_delta.clear();
                output = Some(frame);
            }
            let output = output.unwrap();
            let nav = position(&output, &i18n::t("Next step"));
            let step = position(&output, "3");
            let more = position(&output, &i18n::t("More details"));
            for pressed in [true, false] {
                let mut frame_input = input(size);
                frame_input.events = vec![
                    egui::Event::PointerMoved(more),
                    egui::Event::PointerButton {
                        pos: more,
                        button: egui::PointerButton::Primary,
                        pressed,
                        modifiers: Default::default(),
                    },
                ];
                ctx.run_ui(frame_input, |ui| app.render(ui))
                    .drop_without_applying_deltas();
            }
            let mut output = None;
            for index in 0..5 {
                let mut frame_input = input(size);
                frame_input.time = Some(2.0 + index as f64 * 0.25);
                let mut frame = ctx.run_ui(frame_input, |ui| app.render(ui));
                frame.textures_delta.clear();
                output = Some(frame);
            }
            let expanded = output.unwrap();
            assert_eq!(position(&expanded, &i18n::t("Next step")), nav);
            assert_eq!(position(&expanded, "3"), step);
            assert!(nav.y > 510.0 && nav.y < 580.0);
            let mut frame_input = input(size);
            frame_input.events = vec![
                egui::Event::PointerMoved(egui::pos2(270.0, 350.0)),
                egui::Event::MouseWheel {
                    unit: egui::MouseWheelUnit::Point,
                    phase: egui::TouchPhase::Move,
                    delta: egui::vec2(0.0, -250.0),
                    modifiers: Default::default(),
                },
            ];
            ctx.run_ui(frame_input, |ui| app.render(ui))
                .drop_without_applying_deltas();
            let mut frame = ctx.run_ui(input(size), |ui| app.render(ui));
            frame.textures_delta.clear();
            assert_eq!(position(&frame, &i18n::t("Next step")), nav);
            assert_eq!(position(&frame, "3"), step);
            if let Ok(dir) = std::env::var("RECLAIM_PREVIEW_DIR") {
                rasterize(
                    &ctx,
                    frame,
                    size,
                    &format!("{dir}/guide-expanded-{language:?}.ppm"),
                );
            }
        });
    }
}

#[test]
fn automatic_drive_updates_preserve_selection_and_scan_results() {
    let drive = |name: &str| drives::Drive {
        path: format!("/dev/{name}").into(),
        name: name.into(),
        name_is_message: false,
        size: 64_000_000_000,
        kind: "USB drive".into(),
        location: "Not mounted".into(),
        mounted: false,
        encrypted: false,
        sector_size: 512,
    };
    let mut app = Reclaim::default();
    app.drives = vec![drive("usb1")];
    app.source = Some("/dev/usb1".into());
    app.scanned_source = app.source.clone();
    app.busy = true;
    app.scanned = 4096;
    app.apply_drive_update(Ok(vec![drive("usb1"), drive("usb2")]));
    assert_eq!(
        app.source.as_deref(),
        Some(std::path::Path::new("/dev/usb1"))
    );
    assert!(app.busy);
    assert_eq!(app.scanned, 4096);
    app.apply_drive_update(Err("temporary discovery error".into()));
    assert_eq!(app.drives.len(), 2);
    assert!(app.source.is_some());
    app.apply_drive_update(Ok(vec![drive("usb2")]));
    assert!(app.source.is_none());
    assert_eq!(
        app.scanned_source.as_deref(),
        Some(std::path::Path::new("/dev/usb1"))
    );
    assert!(app.busy); // The active reader owns its handle and reports completion/errors.
    assert!(app.drives_error.is_none());
    app.source = Some("/media/backup.img".into());
    app.apply_drive_update(Ok(vec![]));
    assert_eq!(
        app.source.as_deref(),
        Some(std::path::Path::new("/media/backup.img"))
    );
}

#[test]
fn a_failed_scan_displays_its_error_instead_of_no_files_found() {
    i18n::with_language(i18n::Language::English, || {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut app = Reclaim::default();
        app.page = 1;
        app.busy = true;
        let (tx, rx) = mpsc::channel();
        app.receiver = Some(rx);
        tx.send(Message::Done(Err("Read-only access was denied".into())))
            .unwrap();
        app.poll();
        assert!(!app.busy);
        let mut labels = Vec::new();
        for _ in 0..3 {
            let output = ctx.run_ui(input([1120.0, 780.0]), |ui| app.render(ui));
            labels = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        Some(text.galley.job.text.clone())
                    } else {
                        None
                    }
                })
                .collect();
            output.drop_without_applying_deltas();
        }
        assert!(
            labels
                .iter()
                .any(|label| label == "Could not scan this drive")
        );
        assert!(
            labels
                .iter()
                .any(|label| label.contains("Read-only access was denied"))
        );
        assert!(!labels.iter().any(|label| label == "No files found"));
    });
}

#[test]
fn allocation_error_is_translated_and_footer_stays_short_in_hebrew() {
    i18n::with_language(i18n::Language::Hebrew, || {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut app = Reclaim::default();
        app.page = 1;
        app.busy = true;
        let (tx, rx) = mpsc::channel();
        app.receiver = Some(rx);
        let error = "Unused-space scanning needs a supported formatted volume (NTFS, FAT12/16/32, or exFAT). Select the volume/partition rather than a whole-disk image. Occupied space was not scanned.";
        tx.send(Message::Done(Err(error.into()))).unwrap();
        app.poll();
        assert_eq!(app.status, "Scan failed. See the message above.");

        let mut output = ctx.run_ui(input([920.0, 740.0]), |ui| app.render(ui));
        output.textures_delta.clear();
        let labels: Vec<_> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        assert!(labels.iter().any(|label| label.contains("אמצעי אחסון")));
        assert!(labels.iter().any(|label| label.contains("פרטים מופיעים")));
        assert!(!labels.iter().any(|label| label.contains("Unused-space")));
    });
}

#[test]
fn scan_feedback_tracks_checks_and_preserves_the_stop_request() {
    let mut app = Reclaim::default();
    app.busy = true;
    let (tx, rx) = mpsc::sync_channel(8);
    app.receiver = Some(rx);
    tx.send(Message::Scan(Event::Checking {
        offset: 4096,
        kind: "png",
        bytes: 8192,
    }))
    .unwrap();
    app.poll();
    assert_eq!(app.scanned, 4096);
    let (_, kind, bytes, _) = app.scan_activity.unwrap();
    assert_eq!((kind, bytes), ("png", 8192));
    assert!(app.last_scan_update.is_some());
    tx.send(Message::Scan(Event::Progress(8192))).unwrap();
    app.poll();
    assert!(app.scan_activity.is_none());
    app.stop_operation();
    let stopping = app.status.clone();
    tx.send(Message::Scan(Event::Checking {
        offset: 8192,
        kind: "jpg",
        bytes: 16384,
    }))
    .unwrap();
    app.poll();
    assert_eq!(app.status, stopping);
    tx.send(Message::Done(Ok(()))).unwrap();
    app.poll();
    assert!(!app.busy);
    assert!(app.scan_activity.is_none());
    assert!(app.scan_stopped);
}

#[test]
fn scan_note_stays_visible_between_file_checks() {
    i18n::with_language(i18n::Language::English, || {
        let ctx = egui::Context::default();
        theme::setup(&ctx);
        let mut app = Reclaim::default();
        app.page = 1;
        app.busy = true;
        app.total = 1024;
        app.scan_activity = Some((
            0,
            "zip",
            512,
            std::time::Instant::now() - Duration::from_secs(2),
        ));
        for _ in 0..3 {
            ctx.run_ui(input([920.0, 740.0]), |ui| app.render(ui))
                .drop_without_applying_deltas();
        }
        for checking in [true, false, true, false] {
            if !checking {
                app.scan_activity = None;
            }
            let mut output = ctx.run_ui(input([920.0, 740.0]), |ui| app.render(ui));
            output.textures_delta.clear();
            let labels: Vec<_> = output
                .shapes
                .iter()
                .filter_map(|shape| {
                    if let egui::Shape::Text(text) = &shape.shape {
                        Some(text.galley.job.text.as_str())
                    } else {
                        None
                    }
                })
                .collect();
            assert!(
                labels
                    .iter()
                    .any(|text| text.contains("Found files appear above."))
            );
            assert!(labels.iter().any(|text| text.contains("Latest file check")));
            assert_eq!(app.scan_check_summary, Some(("zip", 512)));
            output.drop_without_applying_deltas();
        }
    });
}

#[test]
fn unused_space_progress_does_not_jump_to_the_physical_disk_offset() {
    let mut app = Reclaim::default();
    app.busy = true;
    let (tx, rx) = mpsc::sync_channel(8);
    app.receiver = Some(rx);
    tx.send(Message::Scan(Event::UnusedSpace {
        bytes: 4096,
        filesystem: "NTFS",
    }))
    .unwrap();
    tx.send(Message::Scan(Event::Progress(512))).unwrap();
    tx.send(Message::Scan(Event::Checking {
        offset: 50 * 1024 * 1024,
        kind: "zip",
        bytes: 1024,
    }))
    .unwrap();
    app.poll();
    assert!(app.unused_scan && app.allocation_ready);
    assert_eq!(app.total, 4096);
    assert_eq!(app.scanned, 512);
    let since = app.scan_activity.unwrap().3;
    tx.send(Message::Scan(Event::Progress(512))).unwrap();
    tx.send(Message::Scan(Event::Checking {
        offset: 50 * 1024 * 1024,
        kind: "zip",
        bytes: 2048,
    }))
    .unwrap();
    app.poll();
    assert_eq!(app.scan_activity.unwrap().3, since);
}

#[test]
fn scan_total_stays_unknown_until_unused_space_is_mapped() {
    let source = tempfile::NamedTempFile::new().unwrap();
    source.as_file().set_len(8192).unwrap();
    let mut app = Reclaim::default();
    app.source = Some(source.path().to_owned());
    app.start();
    assert_eq!(app.source_size, Some(8192));
    assert_eq!(app.total, 0);
    assert!(!app.allocation_ready);
    app.stop_operation();
}

#[test]
fn recovery_reads_offsets_beyond_the_unused_byte_total() {
    let mut source = tempfile::tempfile().unwrap();
    source.write_all(&[0; 4096]).unwrap();
    source.write_all(b"data").unwrap();
    let directory = tempfile::tempdir().unwrap();
    let mut app = Reclaim::default();
    app.scanned_source = Some("test-volume".into());
    app.source_handle = Some(Arc::new(source));
    app.source_size = Some(4100);
    app.total = 512; // Free space is smaller than the physical file offset.
    app.results = vec![Found {
        offset: 4096,
        size: 4,
        kind: "txt",
        selected: true,
        validation: reclaim::Validation::Unverified,
        original_name: Some("recovered.txt".into()),
        resident_data: None,
        ntfs: None,
    }];
    app.export(directory.path().to_owned());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while app.busy && std::time::Instant::now() < deadline {
        app.poll();
        std::thread::yield_now();
    }
    assert!(!app.busy, "export did not finish");
    assert_eq!(
        std::fs::read(directory.path().join("recovered.txt")).unwrap(),
        b"data"
    );
}

#[test]
fn recovery_folder_dialog_blocks_saving_onto_the_scanned_drive() {
    let size = [920.0, 740.0];
    let ctx = egui::Context::default();
    theme::setup(&ctx);
    let mut app = Reclaim::default();
    app.scanned_source = Some("/dev/example".into());
    let labels = |app: &mut Reclaim| {
        let mut output = ctx.run_ui(input(size), |ui| app.render(ui));
        output.textures_delta.clear();
        let texts: Vec<String> = output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(text.galley.job.text.clone()),
                _ => None,
            })
            .collect();
        texts
    };

    app.recovery_folder = Some(("/media/scanned".into(), drives::Destination::SameDrive));
    // Modals measure themselves on their first frame.
    labels(&mut app);
    let texts = labels(&mut app);
    assert!(
        texts
            .iter()
            .any(|t| t == "This folder is on the drive you scanned")
    );
    assert!(texts.iter().any(|t| t == "Choose another folder"));
    assert!(!texts.iter().any(|t| t == "Save files"));

    app.recovery_folder = Some(("/media/other".into(), drives::Destination::Unknown));
    let texts = labels(&mut app);
    assert!(texts.iter().any(|t| t == "Protect your recoverable data"));
    assert!(texts.iter().any(|t| t == "Save files"));

    // Escape dismisses without saving.
    let mut escape = input(size);
    escape.events = vec![egui::Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Default::default(),
    }];
    ctx.run_ui(escape, |ui| app.render(ui))
        .drop_without_applying_deltas();
    assert!(app.recovery_folder.is_none());
    assert!(!app.exporting);

    // A disk image cannot be overwritten by saving beside it, so no dialog.
    let image = std::env::temp_dir().join(format!("reclaim-dialog-{}.img", std::process::id()));
    std::fs::write(&image, b"image").unwrap();
    app.scanned_source = Some(image.clone());
    app.review_recovery_folder(std::env::temp_dir());
    let _ = std::fs::remove_file(&image);
    assert!(app.recovery_folder.is_none());
}
