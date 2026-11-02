use eframe::egui::{self, Color32, CornerRadius, Stroke};
use reclaim::i18n::{self, LocalizedResponse, LocalizedUi};

pub const ACCENT: Color32 = Color32::from_rgb(53, 112, 218);
pub const TEAL: Color32 = Color32::from_rgb(30, 156, 143);

fn blend(a: Color32, b: Color32, amount: f32) -> Color32 {
    egui::lerp(egui::Rgba::from(a)..=egui::Rgba::from(b), amount).into()
}
fn transition(ui: &egui::Ui, id: egui::Id, on: bool) -> f32 {
    ui.ctx()
        .animate_bool_with_time(id, on, ui.style().animation_time)
}

/// Compact identity mark; motion runs only while work is active.
pub fn recovery_emblem(ui: &mut egui::Ui, busy: bool) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(48.0, 48.0), egui::Sense::hover());
    let center = rect.center();
    let painter = ui.painter();
    painter.circle_filled(center, 23.0, blend(ui.visuals().window_fill, ACCENT, 0.16));
    let moving = busy && ui.style().animation_time > 0.0;
    let phase = if moving {
        ui.input(|i| i.time as f32) * 1.8
    } else {
        -std::f32::consts::FRAC_PI_2
    };
    let points = (0..32)
        .map(|i| {
            let angle = phase + i as f32 / 31.0 * std::f32::consts::PI * 1.5;
            center + egui::vec2(angle.cos(), angle.sin()) * 18.0
        })
        .collect();
    painter.add(egui::Shape::line(points, Stroke::new(2.0, ACCENT)));
    let disk = egui::Rect::from_center_size(center, egui::vec2(18.0, 15.0));
    painter.rect_stroke(
        disk,
        3,
        Stroke::new(1.7, ui.visuals().strong_text_color()),
        egui::StrokeKind::Inside,
    );
    painter.circle_filled(center + egui::vec2(4.0, 3.0), 2.0, TEAL);
    if moving {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
}
pub fn setup(ctx: &egui::Context) {
    i18n::setup_fonts(ctx);
    for theme in [egui::Theme::Light, egui::Theme::Dark] {
        let mut s = (*ctx.style_of(theme)).clone();
        let dark = theme == egui::Theme::Dark;
        s.animation_time = 0.18;
        s.spacing.item_spacing = egui::vec2(8.0, 8.0);
        s.spacing.button_padding = egui::vec2(14.0, 9.0);
        s.spacing.interact_size.y = 36.0;
        s.spacing.tooltip_width = 320.0;
        s.spacing.scroll = egui::style::ScrollStyle::solid();
        s.spacing.scroll.bar_width = 10.0;
        s.spacing.scroll.foreground_color = true;
        s.spacing.scroll.handle_min_length = 40.0;
        s.spacing.scroll.bar_inner_margin = 8.0;
        s.spacing.scroll.bar_outer_margin = 2.0;
        s.visuals.panel_fill = if dark {
            Color32::from_rgb(27, 30, 36)
        } else {
            Color32::from_rgb(245, 247, 250)
        };
        s.visuals.window_fill = if dark {
            Color32::from_rgb(35, 39, 46)
        } else {
            Color32::WHITE
        };
        s.visuals.weak_text_color = Some(if dark {
            Color32::from_gray(196)
        } else {
            Color32::from_gray(82)
        });
        let body_text = if dark {
            Color32::from_gray(224)
        } else {
            Color32::from_gray(48)
        };
        s.visuals.widgets.noninteractive.fg_stroke.color = body_text;
        s.visuals.widgets.inactive.fg_stroke.color = body_text;
        s.visuals.extreme_bg_color = if dark {
            Color32::from_rgb(22, 25, 31)
        } else {
            Color32::WHITE
        };
        let control = if dark {
            Color32::from_rgb(48, 53, 62)
        } else {
            Color32::from_rgb(231, 235, 242)
        };
        s.visuals.widgets.inactive.weak_bg_fill = control;
        s.visuals.widgets.inactive.bg_fill = control;
        s.visuals.widgets.hovered.weak_bg_fill = blend(control, ACCENT, 0.18);
        s.visuals.widgets.hovered.bg_fill = blend(control, ACCENT, 0.18);
        s.visuals.widgets.active.weak_bg_fill = blend(control, ACCENT, 0.28);
        s.visuals.widgets.active.bg_fill = blend(control, ACCENT, 0.28);
        s.visuals.selection.bg_fill = ACCENT;
        s.visuals.selection.stroke = Stroke::new(1.0, Color32::WHITE);
        for w in [
            &mut s.visuals.widgets.inactive,
            &mut s.visuals.widgets.hovered,
            &mut s.visuals.widgets.active,
        ] {
            w.corner_radius = CornerRadius::same(8);
        }
        s.text_styles
            .insert(egui::TextStyle::Heading, egui::FontId::proportional(24.0));
        s.text_styles
            .insert(egui::TextStyle::Body, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Button, egui::FontId::proportional(14.0));
        s.text_styles
            .insert(egui::TextStyle::Small, egui::FontId::proportional(12.0));
        s.visuals.widgets.noninteractive.bg_stroke = Stroke::new(
            1.0,
            if dark {
                Color32::from_gray(65)
            } else {
                Color32::from_gray(218)
            },
        );
        ctx.set_style_of(theme, s);
    }
}
pub fn card(ui: &egui::Ui) -> egui::Frame {
    egui::Frame::new()
        .fill(ui.visuals().window_fill)
        .corner_radius(10)
        .stroke(Stroke::new(
            1.0,
            ui.visuals().widgets.noninteractive.bg_stroke.color,
        ))
        .inner_margin(20.0)
}
pub fn primary(ui: &egui::Ui, text: impl Into<String>) -> egui::Button<'static> {
    let text = text.into();
    let color = if text == "Recover files…" {
        Color32::from_rgb(21, 120, 108)
    } else {
        ACCENT
    };
    egui::Button::new(i18n::widget(
        ui,
        egui::RichText::new(text).color(Color32::WHITE).strong(),
        false,
    ))
    .fill(color)
}
pub fn bytes(n: u64) -> String {
    let n = n as f64;
    if n >= 1073741824.0 {
        format!("{:.1} GiB", n / 1073741824.0)
    } else if n >= 1048576.0 {
        format!("{:.1} MiB", n / 1048576.0)
    } else if n >= 1024.0 {
        format!("{:.1} KiB", n / 1024.0)
    } else {
        format!("{n:.0} B")
    }
}

pub fn header_color(ui: &egui::Ui) -> Color32 {
    if ui.visuals().dark_mode {
        Color32::from_rgb(32, 36, 43)
    } else {
        Color32::from_rgb(235, 239, 245)
    }
}

/// A full-width, keyboard-accessible single-choice row in an Adwaita-style list.
pub fn drive_row(
    ui: &mut egui::Ui,
    title: &str,
    subtitle: &str,
    size: &str,
    selected: bool,
    enabled: bool,
) -> egui::Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .frame(false)
            .min_size(egui::vec2(ui.available_width(), 68.0)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::RadioButton,
            enabled,
            selected,
            format!("{title}, {size}, {subtitle}"),
        )
    });
    let rect = response.rect;
    let painter = ui.painter().with_clip_rect(rect);
    let selection = transition(ui, response.id.with("selection"), selected);
    let hover = transition(ui, response.id.with("hover"), enabled && response.hovered());
    let surface = ui.visuals().window_fill;
    let fill = blend(
        blend(surface, ACCENT, hover * 0.09),
        ACCENT,
        selection * 0.22,
    );
    painter.rect_filled(rect, 8, fill);
    if selection > 0.0 {
        let x = if i18n::is_rtl() {
            rect.right() - 3.0
        } else {
            rect.left() + 3.0
        };
        let marker = egui::Rect::from_center_size(
            egui::pos2(x, rect.center().y),
            egui::vec2(3.0, 36.0 * selection),
        );
        painter.rect_filled(marker, 2, ACCENT);
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            8,
            Stroke::new(2.0, ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    let foreground = if enabled {
        ui.visuals().strong_text_color()
    } else {
        ui.visuals().weak_text_color()
    };
    let muted = if selected && ui.visuals().dark_mode {
        Color32::from_rgb(218, 229, 248)
    } else {
        ui.visuals().weak_text_color()
    };
    let indicator = if selected && ui.visuals().dark_mode {
        Color32::from_rgb(161, 201, 255)
    } else {
        ACCENT
    };
    let icon = egui::Rect::from_center_size(
        egui::pos2(
            if i18n::is_rtl() {
                rect.right() - 26.0
            } else {
                rect.left() + 26.0
            },
            rect.center().y,
        ),
        egui::vec2(26.0, 22.0),
    );
    let icon_color = if enabled {
        blend(TEAL, indicator, selection)
    } else {
        muted
    };
    painter.rect_filled(icon.expand(6.0), 7, blend(surface, icon_color, 0.14));
    paint_symbol(ui, icon, Icon::Drive, 20.0, icon_color);
    let text_x = rect.left() + if i18n::is_rtl() { 126.0 } else { 52.0 };
    let width = (rect.width() - 178.0).max(40.0);
    for (text, y, font, color) in [
        (title, rect.top() + 13.0, 15.0, foreground),
        (subtitle, rect.top() + 37.0, 13.0, muted),
    ] {
        let text = i18n::raw_widget(ui, text, egui::FontId::proportional(font), color, width);
        let galley = text.into_galley(
            ui,
            Some(egui::TextWrapMode::Truncate),
            width,
            egui::TextStyle::Body,
        );
        let x = text_x
            + if i18n::is_rtl() {
                (width - galley.size().x).max(0.0)
            } else {
                0.0
            };
        painter.galley(egui::pos2(x, y), galley, color);
    }
    painter.text(
        egui::pos2(
            if i18n::is_rtl() {
                rect.left() + 40.0
            } else {
                rect.right() - 40.0
            },
            rect.center().y,
        ),
        if i18n::is_rtl() {
            egui::Align2::LEFT_CENTER
        } else {
            egui::Align2::RIGHT_CENTER
        },
        size,
        egui::FontId::proportional(13.0),
        muted,
    );
    let dot = egui::pos2(
        if i18n::is_rtl() {
            rect.left() + 18.0
        } else {
            rect.right() - 18.0
        },
        rect.center().y,
    );
    painter.circle_stroke(
        dot,
        7.0,
        Stroke::new(1.5, blend(muted, indicator, selection)),
    );
    if selection > 0.0 {
        painter.circle_filled(dot, 3.5 * selection, indicator);
    }
    response
}

/// Keep the built-in disclosure's focus, keyboard, and expanded-state behavior.
pub fn disclosure(ui: &mut egui::Ui, title: &str, contents: impl FnOnce(&mut egui::Ui)) {
    let mut state = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        ui.make_persistent_id(title),
        false,
    );
    let response = ui.add_sized(
        [ui.available_width(), 38.0],
        egui::Button::new("").frame(false),
    );
    let text = i18n::widget_width(
        ui,
        title,
        false,
        response.rect.width() - 52.0,
        i18n::start_align(),
    );
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Truncate),
        (response.rect.width() - 52.0).max(1.0),
        egui::TextStyle::Button,
    );
    let x = if i18n::is_rtl() {
        response.rect.right() - 36.0 - galley.size().x
    } else {
        response.rect.left() + 36.0
    };
    ui.painter().galley(
        egui::pos2(x, response.rect.center().y - galley.size().y / 2.0),
        galley,
        ui.style().interact(&response).text_color(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::CollapsingHeader,
            ui.is_enabled(),
            state.is_open(),
            i18n::t(title),
        )
    });
    if response.clicked() {
        state.toggle(ui);
    }
    let openness = transition(ui, response.id.with("arrow"), state.is_open());
    let center = egui::pos2(
        if i18n::is_rtl() {
            response.rect.right() - 19.0
        } else {
            response.rect.left() + 19.0
        },
        response.rect.center().y,
    );
    let direction = if i18n::is_rtl() { -1.0 } else { 1.0 };
    let color = ui.style().interact(&response).fg_stroke.color;
    let galley = ui.painter().layout_no_wrap(
        Icon::Chevron.glyph().to_string(),
        egui::FontId::new(12.0, egui::FontFamily::Name("reclaim-icons".into())),
        color,
    );
    let pos = center - galley.size() / 2.0;
    ui.painter().add(
        egui::epaint::TextShape::new(pos, galley, color).with_angle_and_anchor(
            direction * openness * std::f32::consts::FRAC_PI_2,
            egui::Align2::CENTER_CENTER,
        ),
    );
    state.show_body_unindented(ui, |ui| {
        let margin = if i18n::is_rtl() {
            egui::Margin {
                right: 36,
                left: 12,
                top: 4,
                bottom: 8,
            }
        } else {
            egui::Margin {
                left: 36,
                right: 12,
                top: 4,
                bottom: 8,
            }
        };
        egui::Frame::new().inner_margin(margin).show(ui, |ui| {
            ui.with_layout(i18n::column(), contents);
        });
    });
}

/// Center a measured empty-state group, including wrapped text and its action.
pub fn empty_state(
    ui: &mut egui::Ui,
    title: &str,
    description: &str,
    height: f32,
    clear_filters: bool,
    scanning: bool,
) -> bool {
    let width = (ui.available_width() - 32.0).clamp(100.0, 560.0);
    let title = i18n::widget_width(
        ui,
        egui::RichText::new(title).heading().strong(),
        true,
        width,
        egui::Align::Center,
    );
    let description = i18n::widget_width(
        ui,
        egui::RichText::new(description).weak(),
        true,
        width,
        egui::Align::Center,
    );
    let title_height = title
        .clone()
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Wrap),
            width,
            egui::TextStyle::Heading,
        )
        .size()
        .y;
    let body_height = description
        .clone()
        .into_galley(
            ui,
            Some(egui::TextWrapMode::Wrap),
            width,
            egui::TextStyle::Body,
        )
        .size()
        .y;
    let spacing = ui.spacing().item_spacing.y;
    let radar_size = if scanning {
        (height - title_height - body_height - spacing * 3.0).clamp(0.0, 96.0)
    } else {
        0.0
    };
    let radar_height = if radar_size >= 32.0 {
        radar_size + spacing + 4.0
    } else {
        0.0
    };
    let content_height = radar_height
        + title_height
        + spacing
        + body_height
        + if clear_filters {
            spacing + ui.spacing().interact_size.y
        } else {
            0.0
        };
    ui.set_min_height(height);
    let mut clear = false;
    ui.vertical_centered(|ui| {
        ui.add_space(((height - content_height) / 2.0).max(0.0));
        if radar_height > 0.0 {
            scan_radar(ui, radar_size);
            ui.add_space(4.0);
        }
        ui.add_sized(
            [width, title_height],
            egui::Label::new(title).halign(egui::Align::Center).wrap(),
        );
        ui.add_sized(
            [width, body_height],
            egui::Label::new(description)
                .halign(egui::Align::Center)
                .wrap(),
        );
        if clear_filters {
            clear = ui.t_button("Clear Filters").clicked();
        }
    });
    clear
}

/// Direction-aware dropdown, using egui's popup dismissal and keyboard handling.
pub fn category_dropdown(ui: &mut egui::Ui, category: &mut usize, width: f32) {
    let choices: Vec<_> = reclaim::FILE_CATEGORIES
        .iter()
        .enumerate()
        .map(|(i, label)| (i, *label))
        .collect();
    choice_dropdown(ui, "category", category, &choices, width);
}

pub fn choice_dropdown<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut T,
    choices: &[(T, &str)],
    width: f32,
) {
    let selected = choices
        .iter()
        .find(|(alternative, _)| *alternative == *value)
        .map(|(_, label)| *label)
        .unwrap_or_default();
    if !i18n::is_rtl() {
        egui::ComboBox::from_id_salt(id)
            .width(width)
            .selected_text(i18n::widget(ui, selected, false))
            .show_ui(ui, |ui| {
                for (alternative, label) in choices {
                    ui.t_selectable_value(value, *alternative, *label);
                }
            });
        return;
    }
    let text = i18n::widget(ui, selected, false);
    let response = ui
        .push_id(id, |ui| {
            ui.add_sized([width, 40.0], egui::Button::new("    ").right_text(text))
        })
        .inner;
    let center = egui::pos2(response.rect.left() + 19.0, response.rect.center().y);
    paint_symbol(
        ui,
        egui::Rect::from_center_size(center, egui::vec2(16.0, 16.0)),
        Icon::ChevronDown,
        12.0,
        ui.style().interact(&response).text_color(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(
            egui::WidgetType::ComboBox,
            ui.is_enabled(),
            i18n::t(selected),
        )
    });
    egui::Popup::menu(&response).width(width).show(|ui| {
        ui.with_layout(i18n::column().with_cross_justify(true), |ui| {
            ui.set_min_width(width - 16.0);
            for (alternative, label) in choices {
                if ui.t_selectable_value(value, *alternative, *label).clicked() {
                    ui.close();
                }
            }
        });
    });
}

/// Keep wide preference controls beside their labels; stack them on small windows.
pub fn preference_row<R>(
    ui: &mut egui::Ui,
    label: &str,
    controls: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    if ui.available_width() < 540.0 {
        ui.t_strong(label);
        return controls(ui);
    }
    let width = ui.available_width();
    let controls_width = (width * 0.55).min(360.0);
    ui.allocate_ui_with_layout(egui::vec2(width, 44.0), i18n::row(), |ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(width - controls_width - 8.0, 44.0),
            i18n::row(),
            |ui| {
                ui.set_min_width(width - controls_width - 8.0);
                ui.t_strong(label);
            },
        );
        ui.allocate_ui_with_layout(egui::vec2(controls_width, 44.0), i18n::row(), |ui| {
            ui.set_min_width(controls_width);
            controls(ui)
        })
        .inner
    })
    .inner
}

/// Equal-width choices with stable IDs and standard keyboard activation.
pub fn setting_choices<T: Copy + PartialEq>(
    ui: &mut egui::Ui,
    id: &str,
    value: &mut T,
    choices: &[(T, &str)],
) {
    ui.push_id(id, |ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        ui.spacing_mut().button_padding = egui::vec2(8.0, 8.0);
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 40.0), i18n::row(), |ui| {
            let width = (ui.available_width()
                - ui.spacing().item_spacing.x * (choices.len() - 1) as f32)
                / choices.len() as f32;
            for (alternative, label) in choices {
                let text = i18n::widget(ui, *label, false);
                let response = if id == "appearance" {
                    let icon = match *label {
                        "Light" => Icon::Sun,
                        "Dark" => Icon::Moon,
                        _ => Icon::System,
                    };
                    icon_button(
                        ui,
                        icon,
                        label,
                        *value == *alternative,
                        egui::vec2(width, 40.0),
                    )
                } else {
                    ui.add_sized(
                        [width, 40.0],
                        egui::Button::new(text).selected(*value == *alternative),
                    )
                };
                response.widget_info(|| {
                    egui::WidgetInfo::selected(
                        egui::WidgetType::RadioButton,
                        ui.is_enabled(),
                        *value == *alternative,
                        i18n::t(label),
                    )
                });
                if response.clicked() {
                    *value = *alternative;
                }
            }
        });
    });
}

#[derive(Clone, Copy)]
pub enum Icon {
    Settings,
    Back,
    Refresh,
    Check,
    SelectAll,
    DeselectAll,
    System,
    Sun,
    Moon,
    Drive,
    Minimize,
    Maximize,
    Restore,
    Close,
    Chevron,
    ChevronDown,
    Guide,
    Results,
    DiskImage,
    Shield,
    Warning,
    Info,
    Picture,
    Document,
    Pdf,
    Word,
    Spreadsheet,
    Presentation,
    Archive,
    Audio,
    Video,
    Application,
}

/// Font Awesome Free 6.7.2 Solid; see the bundled font license.
impl Icon {
    fn glyph(self) -> char {
        match self {
            Self::Settings => '\u{f013}',
            Self::Back if i18n::is_rtl() => '\u{f061}',
            Self::Back => '\u{f060}',
            Self::Refresh => '\u{f021}',
            Self::Check => '\u{f00c}',
            Self::SelectAll => '\u{f14a}',
            Self::DeselectAll => '\u{f146}',
            Self::System => '\u{f390}',
            Self::Sun => '\u{f185}',
            Self::Moon => '\u{f186}',
            Self::Drive => '\u{f0a0}',
            Self::Minimize => '\u{f2d1}',
            Self::Maximize => '\u{f2d0}',
            Self::Restore => '\u{f2d2}',
            Self::Close => '\u{f00d}',
            Self::Chevron if i18n::is_rtl() => '\u{f053}',
            Self::Chevron => '\u{f054}',
            Self::ChevronDown => '\u{f078}',
            Self::Guide => '\u{f02d}',
            Self::Results => '\u{f03a}',
            Self::DiskImage => '\u{f56d}',
            Self::Shield => '\u{f3ed}',
            Self::Warning => '\u{f071}',
            Self::Info => '\u{f05a}',
            Self::Picture => '\u{f03e}',
            Self::Document => '\u{f15b}',
            Self::Pdf => '\u{f1c1}',
            Self::Word => '\u{f1c2}',
            Self::Spreadsheet => '\u{f1c3}',
            Self::Presentation => '\u{f1c4}',
            Self::Archive => '\u{f1c6}',
            Self::Audio => '\u{f1c7}',
            Self::Video => '\u{f1c8}',
            Self::Application => '\u{f1c9}',
        }
    }
}
pub fn paint_symbol(ui: &egui::Ui, rect: egui::Rect, icon: Icon, size: f32, color: Color32) {
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        icon.glyph(),
        egui::FontId::new(size, egui::FontFamily::Name("reclaim-icons".into())),
        color,
    );
}
fn paint_icon(ui: &egui::Ui, response: &egui::Response, icon: Icon) {
    paint_symbol(
        ui,
        response.rect,
        icon,
        16.0,
        ui.style().interact(response).fg_stroke.color,
    );
}

pub fn icon_button(
    ui: &mut egui::Ui,
    icon: Icon,
    label: &str,
    selected: bool,
    size: egui::Vec2,
) -> egui::Response {
    let response = ui.add_sized(size, egui::Button::new("").selected(selected));
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), i18n::t(label))
    });
    paint_icon(ui, &response, icon);
    response.t_on_hover_text(label)
}

/// A purely visual sweep: actual scan progress is always reported separately.
/// Static when animations are disabled, with no idle repaint loop.
pub fn scan_radar(ui: &mut egui::Ui, diameter: f32) {
    let (rect, _) = ui.allocate_exact_size(egui::vec2(diameter, diameter), egui::Sense::hover());
    if !ui.is_rect_visible(rect) {
        return;
    }
    let center = rect.center();
    let radius = diameter * 0.43;
    let p = ui.painter();
    let surface = ui.visuals().window_fill;
    let moving = ui.style().animation_time > 0.0;
    let phase = if moving {
        (ui.input(|i| i.time) % 3.6) as f32 / 3.6 * std::f32::consts::TAU
    } else {
        -0.8
    };
    p.circle_filled(center, radius, blend(surface, ACCENT, 0.06));
    let grid = Stroke::new(1.0, blend(surface, ACCENT, 0.25));
    for scale in [0.33, 0.66, 1.0] {
        p.circle_stroke(center, radius * scale, grid);
    }
    for direction in [egui::vec2(1.0, 0.0), egui::vec2(0.0, 1.0)] {
        p.line_segment(
            [center - direction * radius, center + direction * radius],
            grid,
        );
    }
    for segment in 0..24 {
        let angle = phase - (24 - segment) as f32 * 0.05;
        let a = center + egui::vec2(angle.cos(), angle.sin()) * radius;
        let b = center + egui::vec2((angle + 0.05).cos(), (angle + 0.05).sin()) * radius;
        let color = blend(surface, TEAL, 0.04 + segment as f32 / 24.0 * 0.22);
        p.add(egui::Shape::convex_polygon(
            vec![center, a, b],
            color,
            Stroke::NONE,
        ));
    }
    for (angle, distance) in [(0.6, 0.72), (2.8, 0.5), (4.9, 0.78)] {
        let age = (phase - angle).rem_euclid(std::f32::consts::TAU);
        let intensity = if moving { (-age * 1.5).exp() } else { 0.6 };
        let dot = center + egui::vec2(angle.cos(), angle.sin()) * radius * distance;
        p.circle_filled(dot, 5.0, blend(surface, TEAL, intensity * 0.14));
        p.circle_filled(dot, 2.2, blend(surface, TEAL, 0.3 + intensity * 0.7));
    }
    p.line_segment(
        [
            center,
            center + egui::vec2(phase.cos(), phase.sin()) * radius,
        ],
        Stroke::new(1.8, TEAL),
    );
    p.circle_filled(center, 2.5, ACCENT);
    if moving {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(16));
    }
}

/// Keyboard-accessible switch with an animated thumb; no layout movement.
pub fn switch(ui: &mut egui::Ui, value: &mut bool, label: &str) -> egui::Response {
    let mut response = ui.add_sized([48.0, 36.0], egui::Button::new("").frame(false));
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Checkbox,
            ui.is_enabled(),
            *value,
            i18n::t(label),
        )
    });
    let amount = transition(ui, response.id.with("switch"), *value);
    let track = egui::Rect::from_center_size(response.rect.center(), egui::vec2(42.0, 24.0));
    let off = if ui.visuals().dark_mode {
        Color32::from_gray(90)
    } else {
        Color32::from_gray(150)
    };
    let color = blend(off, ACCENT, amount);
    ui.painter().rect_filled(track, 12, color);
    let start = if i18n::is_rtl() {
        track.right() - 12.0
    } else {
        track.left() + 12.0
    };
    let end = if i18n::is_rtl() {
        track.left() + 12.0
    } else {
        track.right() - 12.0
    };
    ui.painter().circle_filled(
        egui::pos2(egui::lerp(start..=end, amount), track.center().y),
        9.0,
        Color32::WHITE,
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            track.expand(3.0),
            14,
            Stroke::new(2.0, ui.visuals().selection.stroke.color),
            egui::StrokeKind::Outside,
        );
    }
    response.t_on_hover_text(label)
}

/// Cached portal state is injected before egui begins a frame, so styles and
/// native theme notifications use the same input path. 255 means not available.
pub struct SystemThemeWatcher {
    scheme: std::sync::Arc<std::sync::atomic::AtomicU8>,
}
impl SystemThemeWatcher {
    pub fn apply_input(&self, input: &mut egui::RawInput) {
        match self.scheme.load(std::sync::atomic::Ordering::Relaxed) {
            1 => input.system_theme = Some(egui::Theme::Dark),
            2 => input.system_theme = Some(egui::Theme::Light),
            0 => input.system_theme = None,
            _ => {} // No portal value: preserve any native backend input.
        }
    }
}
#[cfg(any(target_os = "linux", test))]
fn record_system_theme(ctx: &egui::Context, state: &SystemThemeWatcher, scheme: u32) {
    let scheme = match scheme {
        1 => 1,
        2 => 2,
        _ => 0,
    };
    if state
        .scheme
        .swap(scheme, std::sync::atomic::Ordering::Relaxed)
        != scheme
    {
        ctx.request_repaint();
    }
}

/// Linux winit reports no theme. Listen to the desktop portal on the session
/// bus instead; all connection/read/wait work happens off the UI thread.
/// Windows and macOS already deliver ThemeChanged through egui-winit.
pub fn watch_system_theme(ctx: &egui::Context) -> Option<SystemThemeWatcher> {
    #[cfg(target_os = "linux")]
    {
        let ctx = ctx.clone();
        let scheme = std::sync::Arc::new(std::sync::atomic::AtomicU8::new(255));
        let worker = SystemThemeWatcher {
            scheme: scheme.clone(),
        };
        let _ = std::thread::Builder::new()
            .name("system-appearance".into())
            .spawn(move || {
                loop {
                    let _ = listen_for_linux_theme(&ctx, &worker);
                    // Retry if a portal starts late, restarts, or the session bus
                    // reconnects. This is a background retry, not UI-frame polling.
                    std::thread::sleep(std::time::Duration::from_secs(5));
                }
            });
        Some(SystemThemeWatcher { scheme })
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = ctx;
        None
    }
}

#[cfg(any(target_os = "linux", test))]
#[derive(Default)]
struct DesktopAppearance {
    color_scheme: u32,
    gnome_scheme: String,
    gtk_theme: String,
}
#[cfg(any(target_os = "linux", test))]
impl DesktopAppearance {
    fn resolved_scheme(&self) -> u32 {
        match self.color_scheme {
            1 | 2 => self.color_scheme,
            _ => match self.gnome_scheme.as_str() {
                "prefer-dark" => 1,
                "prefer-light" => 2,
                _ if self.gtk_theme.to_ascii_lowercase().ends_with("dark") => 1,
                _ => 2, // Default desktop themes are light when no preference is set.
            },
        }
    }
}

#[cfg(target_os = "linux")]
fn listen_for_linux_theme(ctx: &egui::Context, state: &SystemThemeWatcher) -> zbus::Result<()> {
    use std::collections::HashMap;
    use zbus::{
        blocking::{Proxy, connection::Builder},
        zvariant::OwnedValue,
    };
    const APPEARANCE: &str = "org.freedesktop.appearance";
    const GNOME: &str = "org.gnome.desktop.interface";
    let connection = Builder::session()?
        .method_timeout(std::time::Duration::from_secs(3))
        .build()?;
    let proxy = Proxy::new(
        &connection,
        "org.freedesktop.portal.Desktop",
        "/org/freedesktop/portal/desktop",
        "org.freedesktop.portal.Settings",
    )?;
    // Subscribe before the snapshot. Observe both standard and GTK settings:
    // GNOME can change gtk-theme while the standard scheme remains zero.
    let signals = proxy.receive_signal("SettingChanged")?;
    let settings: HashMap<String, HashMap<String, OwnedValue>> =
        proxy.call("ReadAll", &(vec![APPEARANCE, GNOME],))?;
    let mut desktop = DesktopAppearance::default();
    if let Some(value) = settings
        .get(APPEARANCE)
        .and_then(|map| map.get("color-scheme"))
    {
        desktop.color_scheme = u32::try_from(value).unwrap_or(0);
    }
    if let Some(values) = settings.get(GNOME) {
        desktop.gnome_scheme = values
            .get("color-scheme")
            .and_then(|v| <&str>::try_from(v).ok())
            .unwrap_or_default()
            .into();
        desktop.gtk_theme = values
            .get("gtk-theme")
            .and_then(|v| <&str>::try_from(v).ok())
            .unwrap_or_default()
            .into();
    }
    record_system_theme(ctx, state, desktop.resolved_scheme());
    for signal in signals {
        let Ok((namespace, key, value)) =
            signal.body().deserialize::<(String, String, OwnedValue)>()
        else {
            continue;
        };
        match (namespace.as_str(), key.as_str()) {
            (APPEARANCE, "color-scheme") => {
                if let Ok(value) = u32::try_from(value) {
                    desktop.color_scheme = value;
                }
            }
            (GNOME, "color-scheme") => {
                if let Ok(value) = <&str>::try_from(&value) {
                    desktop.gnome_scheme = value.into();
                }
            }
            (GNOME, "gtk-theme") => {
                if let Ok(value) = <&str>::try_from(&value) {
                    desktop.gtk_theme = value.into();
                }
            }
            _ => continue,
        }
        record_system_theme(ctx, state, desktop.resolved_scheme());
    }
    Ok(())
}

/// File-type silhouettes and category colors make results easy to scan.
pub fn file_badge(ui: &mut egui::Ui, kind: &str) {
    let category = reclaim::category_index(kind);
    let icon = match kind {
        "pdf" => Icon::Pdf,
        "docx" | "docm" | "odt" => Icon::Word,
        "xlsx" | "xlsm" | "ods" | "csv" => Icon::Spreadsheet,
        "pptx" | "pptm" | "odp" => Icon::Presentation,
        _ => match category {
            1 | 7 => Icon::Picture,
            2 => Icon::Document,
            3 => Icon::Archive,
            4 => Icon::Audio,
            5 => Icon::Video,
            6 => Icon::Application,
            _ => Icon::Document,
        },
    };
    let base = match category {
        1 => TEAL,
        2 => ACCENT,
        3 => Color32::from_rgb(182, 121, 27),
        4 | 7 => Color32::from_rgb(146, 102, 209),
        5 => Color32::from_rgb(198, 88, 107),
        _ => Color32::from_rgb(98, 120, 148),
    };
    let color = if ui.visuals().dark_mode {
        blend(base, Color32::WHITE, 0.35)
    } else {
        base
    };
    let (rect, response) = ui.allocate_exact_size(egui::vec2(36.0, 40.0), egui::Sense::hover());
    ui.painter()
        .rect_filled(rect, 8, blend(ui.visuals().window_fill, base, 0.15));
    paint_symbol(ui, rect, icon, 19.0, color);
    response.raw_on_hover_text(kind.to_uppercase());
}
pub fn detail_row(ui: &mut egui::Ui, label: &str, value: &str) {
    ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), i18n::row(), |ui| {
        let label_width = (ui.available_width() * 0.34).clamp(88.0, 144.0);
        ui.allocate_ui_with_layout(egui::vec2(label_width, 24.0), i18n::column(), |ui| {
            ui.set_min_width(label_width);
            ui.add(
                egui::Label::new(i18n::widget(
                    ui,
                    egui::RichText::new(label).small().weak(),
                    false,
                ))
                .truncate(),
            );
        });
        let text = i18n::raw_widget(
            ui,
            value,
            egui::TextStyle::Body.resolve(ui.style()),
            ui.visuals().text_color(),
            ui.available_width(),
        );
        ui.add(egui::Label::new(text).truncate())
            .raw_on_hover_text(value);
    });
}
pub fn file_date(timestamp: Option<i64>) -> String {
    timestamp
        .and_then(|seconds| chrono::DateTime::from_timestamp(seconds, 0))
        .map(|date| date.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| i18n::t("Not available"))
}

#[cfg(test)]
mod system_theme_tests {
    use super::*;
    fn update(ctx: &egui::Context, state: &SystemThemeWatcher, scheme: u32) {
        record_system_theme(ctx, state, scheme);
        let mut input = egui::RawInput::default();
        state.apply_input(&mut input);
        ctx.run_ui(input, |_| {}).drop_without_applying_deltas();
    }
    #[test]
    fn live_system_changes_preserve_explicit_preferences() {
        let ctx = egui::Context::default();
        setup(&ctx);
        let state = SystemThemeWatcher {
            scheme: std::sync::Arc::new(std::sync::atomic::AtomicU8::new(255)),
        };
        ctx.set_theme(egui::ThemePreference::System);
        update(&ctx, &state, 2);
        assert_eq!(ctx.theme(), egui::Theme::Light);
        update(&ctx, &state, 1);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        ctx.set_theme(egui::ThemePreference::Light);
        update(&ctx, &state, 1);
        assert_eq!(ctx.theme(), egui::Theme::Light);
        ctx.set_theme(egui::ThemePreference::Dark);
        update(&ctx, &state, 2);
        assert_eq!(ctx.theme(), egui::Theme::Dark);
        ctx.set_theme(egui::ThemePreference::System);
        assert_eq!(ctx.theme(), egui::Theme::Light);
        update(&ctx, &state, 0);
        assert_eq!(ctx.system_theme(), None);
        update(&ctx, &state, 99);
        assert_eq!(ctx.system_theme(), None);
    }
    #[test]
    fn no_preference_follows_gtk_theme_and_explicit_scheme_wins() {
        let mut desktop = DesktopAppearance {
            gtk_theme: "Yaru-blue".into(),
            ..Default::default()
        };
        assert_eq!(desktop.resolved_scheme(), 2);
        desktop.gtk_theme = "Yaru-blue-dark".into();
        assert_eq!(desktop.resolved_scheme(), 1);
        desktop.gnome_scheme = "prefer-light".into();
        assert_eq!(desktop.resolved_scheme(), 2);
        desktop.color_scheme = 1;
        assert_eq!(desktop.resolved_scheme(), 1);
        desktop.color_scheme = 2;
        assert_eq!(desktop.resolved_scheme(), 2);
    }
    #[test]
    fn native_theme_events_follow_system_while_app_runs() {
        let ctx = egui::Context::default();
        setup(&ctx);
        ctx.set_theme(egui::ThemePreference::System);
        for system in [egui::Theme::Light, egui::Theme::Dark, egui::Theme::Light] {
            let input = egui::RawInput {
                system_theme: Some(system),
                ..Default::default()
            };
            ctx.run_ui(input, |ui| {
                assert_eq!(ui.visuals().dark_mode, system == egui::Theme::Dark)
            })
            .drop_without_applying_deltas();
        }
    }
}

#[cfg(all(test, target_os = "linux"))]
mod live_portal_test {
    #[test]
    #[ignore = "requires a desktop session bus"]
    fn read_live_desktop_color_scheme() {
        let connection = zbus::blocking::Connection::session().unwrap();
        let proxy = zbus::blocking::Proxy::new(
            &connection,
            "org.freedesktop.portal.Desktop",
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
        )
        .unwrap();
        let result: Result<
            std::collections::HashMap<
                String,
                std::collections::HashMap<String, zbus::zvariant::OwnedValue>,
            >,
            _,
        > = proxy.call(
            "ReadAll",
            &(vec![
                "org.freedesktop.appearance",
                "org.gnome.desktop.interface",
            ],),
        );
        match result {
            Ok(settings) => {
                let values = settings.get("org.gnome.desktop.interface");
                let desktop = super::DesktopAppearance {
                    color_scheme: settings
                        .get("org.freedesktop.appearance")
                        .and_then(|m| m.get("color-scheme"))
                        .and_then(|v| u32::try_from(v).ok())
                        .unwrap_or(0),
                    gnome_scheme: values
                        .and_then(|m| m.get("color-scheme"))
                        .and_then(|v| <&str>::try_from(v).ok())
                        .unwrap_or_default()
                        .into(),
                    gtk_theme: values
                        .and_then(|m| m.get("gtk-theme"))
                        .and_then(|v| <&str>::try_from(v).ok())
                        .unwrap_or_default()
                        .into(),
                };
                eprintln!(
                    "Desktop scheme: {}, GTK theme: {}, resolved: {}",
                    desktop.color_scheme,
                    desktop.gtk_theme,
                    desktop.resolved_scheme()
                );
            }
            Err(error) => panic!("Portal read failed: {error}"),
        }
    }
}
