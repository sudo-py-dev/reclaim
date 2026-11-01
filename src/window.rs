use eframe::egui::{self, Color32, Pos2, Rect, ResizeDirection, Stroke, ViewportCommand};
use reclaim::i18n::{self, LocalizedResponse, LocalizedUi};

// macOS keeps its native traffic lights and window behavior.
pub const CUSTOM_DECORATIONS: bool = !cfg!(target_os = "macos");

#[derive(Clone, Copy)]
enum Control {
    Minimize,
    Maximize,
    Restore,
    Close,
}
impl Control {
    fn label(self) -> &'static str {
        match self {
            Self::Minimize => "Minimize",
            Self::Maximize => "Maximize",
            Self::Restore => "Restore window",
            Self::Close => "Close",
        }
    }
}
fn control(ui: &mut egui::Ui, control: Control) -> egui::Response {
    let response = ui.add_sized(
        [32.0, 32.0],
        egui::Button::new("").frame(false).corner_radius(4),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, true, i18n::t(control.label()))
    });
    let response = response.t_on_hover_text(control.label());
    let icon = match control {
        Control::Minimize => crate::theme::Icon::Minimize,
        Control::Maximize => crate::theme::Icon::Maximize,
        Control::Restore => crate::theme::Icon::Restore,
        Control::Close => crate::theme::Icon::Close,
    };
    crate::theme::paint_symbol(
        ui,
        response.rect,
        icon,
        13.0,
        ui.style().interact(&response).fg_stroke.color,
    );
    response
}

pub fn controls(ui: &mut egui::Ui) {
    if !CUSTOM_DECORATIONS {
        return;
    }
    let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
    if control(ui, Control::Minimize).clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
    }
    if control(
        ui,
        if maximized {
            Control::Restore
        } else {
            Control::Maximize
        },
    )
    .clicked()
    {
        ui.ctx()
            .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
    }
    if control(ui, Control::Close).clicked() {
        ui.ctx().send_viewport_cmd(ViewportCommand::Close);
    }
}

pub fn title(ui: &mut egui::Ui, rect: Rect, title: &str) {
    if CUSTOM_DECORATIONS {
        let response = ui.interact(
            rect,
            ui.id().with("title_drag"),
            egui::Sense::click_and_drag(),
        );
        if response.double_clicked() {
            let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
        } else if response.drag_started() {
            ui.ctx().send_viewport_cmd(ViewportCommand::StartDrag);
        }
        response.context_menu(|ui| {
            ui.with_layout(i18n::column(), |ui| {
                let maximized = ui.input(|i| i.viewport().maximized.unwrap_or(false));
                if ui
                    .t_button(if maximized {
                        "Restore window"
                    } else {
                        "Maximize"
                    })
                    .clicked()
                {
                    ui.ctx()
                        .send_viewport_cmd(ViewportCommand::Maximized(!maximized));
                    ui.close();
                }
                if ui.t_button("Minimize").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Minimized(true));
                    ui.close();
                }
                ui.separator();
                if ui.t_button("Close").clicked() {
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                    ui.close();
                }
            });
        });
    }
    let text = i18n::widget(
        ui,
        egui::RichText::new(title)
            .size(15.0)
            .color(ui.visuals().strong_text_color()),
        false,
    );
    let galley = text.into_galley(
        ui,
        Some(egui::TextWrapMode::Extend),
        f32::INFINITY,
        egui::TextStyle::Body,
    );
    ui.painter().galley(
        rect.center() - galley.size() / 2.0,
        galley,
        ui.visuals().strong_text_color(),
    );
}

fn resize_direction(rect: Rect, pos: Pos2) -> Option<(ResizeDirection, egui::CursorIcon)> {
    if !rect.contains(pos) {
        return None;
    }
    let edge = 5.0;
    let corner = 14.0;
    let (left, right) = (pos.x < rect.left() + edge, pos.x > rect.right() - edge);
    let (top, bottom) = (pos.y < rect.top() + edge, pos.y > rect.bottom() - edge);
    let (near_left, near_right) = (pos.x < rect.left() + corner, pos.x > rect.right() - corner);
    let (near_top, near_bottom) = (pos.y < rect.top() + corner, pos.y > rect.bottom() - corner);
    use ResizeDirection as D;
    use egui::CursorIcon as C;
    if (top && near_left) || (left && near_top) {
        Some((D::NorthWest, C::ResizeNwSe))
    } else if (top && near_right) || (right && near_top) {
        Some((D::NorthEast, C::ResizeNeSw))
    } else if (bottom && near_left) || (left && near_bottom) {
        Some((D::SouthWest, C::ResizeNeSw))
    } else if (bottom && near_right) || (right && near_bottom) {
        Some((D::SouthEast, C::ResizeNwSe))
    } else if left {
        Some((D::West, C::ResizeHorizontal))
    } else if right {
        Some((D::East, C::ResizeHorizontal))
    } else if top {
        Some((D::North, C::ResizeVertical))
    } else if bottom {
        Some((D::South, C::ResizeVertical))
    } else {
        None
    }
}
pub fn resize(ui: &mut egui::Ui) {
    if !CUSTOM_DECORATIONS
        || ui.input(|i| {
            i.viewport().maximized.unwrap_or(false) || i.viewport().fullscreen.unwrap_or(false)
        })
    {
        return;
    }
    let rect = ui.ctx().viewport_rect();
    let direction = ui
        .input(|i| i.pointer.hover_pos())
        .and_then(|p| resize_direction(rect, p));
    if let Some((direction, cursor)) = direction {
        ui.ctx().set_cursor_icon(cursor);
        if ui.input(|i| i.pointer.primary_pressed()) {
            ui.ctx()
                .send_viewport_cmd(ViewportCommand::BeginResize(direction));
        }
    }
    // A subtle boundary separates undecorated windows from adjacent surfaces.
    ui.painter().rect_stroke(
        rect,
        0,
        Stroke::new(1.0, Color32::from_black_alpha(35)),
        egui::StrokeKind::Inside,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resize_edges_corners_and_content() {
        let rect = Rect::from_min_size(Pos2::ZERO, egui::vec2(900.0, 700.0));
        for (p, expected) in [
            ([2.0, 2.0], ResizeDirection::NorthWest),
            ([898.0, 2.0], ResizeDirection::NorthEast),
            ([2.0, 698.0], ResizeDirection::SouthWest),
            ([898.0, 698.0], ResizeDirection::SouthEast),
            ([450.0, 2.0], ResizeDirection::North),
            ([450.0, 698.0], ResizeDirection::South),
            ([2.0, 350.0], ResizeDirection::West),
            ([898.0, 350.0], ResizeDirection::East),
        ] {
            assert_eq!(
                resize_direction(rect, p.into()).map(|v| v.0),
                Some(expected)
            );
        }
        assert!(resize_direction(rect, egui::pos2(450.0, 350.0)).is_none());
        assert!(resize_direction(rect, egui::pos2(-1.0, 0.0)).is_none());
    }
}

#[cfg(all(test, not(target_os = "macos")))]
mod interaction_tests {
    use super::*;
    fn run(ctx: &egui::Context, maximized: bool, events: Vec<egui::Event>) -> egui::FullOutput {
        let mut input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 400.0))),
            events,
            ..Default::default()
        };
        input
            .viewports
            .get_mut(&egui::ViewportId::ROOT)
            .unwrap()
            .maximized = Some(maximized);
        let mut output = ctx.run_ui(input, |ui| {
            ui.spacing_mut().item_spacing.x = 6.0;
            ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), controls);
        });
        output.textures_delta.clear();
        output
    }
    #[test]
    fn window_buttons_send_the_right_commands() {
        for (x, maximized, expected) in [
            (16.0, false, ViewportCommand::Minimized(true)),
            (54.0, false, ViewportCommand::Maximized(true)),
            (54.0, true, ViewportCommand::Maximized(false)),
            (92.0, false, ViewportCommand::Close),
        ] {
            let ctx = egui::Context::default();
            crate::theme::setup(&ctx);
            run(&ctx, maximized, vec![]).drop_without_applying_deltas();
            let pos = egui::pos2(x, 200.0);
            run(
                &ctx,
                maximized,
                vec![
                    egui::Event::PointerMoved(pos),
                    egui::Event::PointerButton {
                        pos,
                        button: egui::PointerButton::Primary,
                        pressed: true,
                        modifiers: Default::default(),
                    },
                ],
            )
            .drop_without_applying_deltas();
            let output = run(
                &ctx,
                maximized,
                vec![egui::Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: false,
                    modifiers: Default::default(),
                }],
            );
            assert!(
                output.viewport_output[&egui::ViewportId::ROOT]
                    .commands
                    .contains(&expected),
                "Missing command {expected:?}"
            );
        }
    }
    fn title_frame(ctx: &egui::Context, time: f64, events: Vec<egui::Event>) -> egui::FullOutput {
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 400.0))),
            time: Some(time),
            events,
            ..Default::default()
        };
        let mut output = ctx.run_ui(input, |ui| {
            title(
                ui,
                Rect::from_min_size(Pos2::ZERO, egui::vec2(600.0, 50.0)),
                "Reclaim",
            )
        });
        output.textures_delta.clear();
        output
    }
    fn pointer(pos: Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton {
            pos,
            button: egui::PointerButton::Primary,
            pressed,
            modifiers: Default::default(),
        }
    }
    #[test]
    fn title_double_click_maximizes_and_drag_starts_native_move() {
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        let pos = egui::pos2(250.0, 25.0);
        title_frame(&ctx, 0.0, vec![]).drop_without_applying_deltas();
        title_frame(
            &ctx,
            0.05,
            vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
        )
        .drop_without_applying_deltas();
        title_frame(&ctx, 0.10, vec![pointer(pos, false)]).drop_without_applying_deltas();
        title_frame(&ctx, 0.15, vec![pointer(pos, true)]).drop_without_applying_deltas();
        let output = title_frame(&ctx, 0.20, vec![pointer(pos, false)]);
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&ViewportCommand::Maximized(true))
        );
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        title_frame(&ctx, 0.0, vec![]).drop_without_applying_deltas();
        title_frame(
            &ctx,
            0.05,
            vec![egui::Event::PointerMoved(pos), pointer(pos, true)],
        )
        .drop_without_applying_deltas();
        let output = title_frame(
            &ctx,
            0.10,
            vec![egui::Event::PointerMoved(pos + egui::vec2(20.0, 0.0))],
        );
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&ViewportCommand::StartDrag)
        );
    }
    #[test]
    fn window_buttons_support_keyboard_activation() {
        let ctx = egui::Context::default();
        crate::theme::setup(&ctx);
        run(&ctx, false, vec![]).drop_without_applying_deltas();
        let key = |key| egui::Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Default::default(),
        };
        run(&ctx, false, vec![key(egui::Key::Tab)]).drop_without_applying_deltas();
        let output = run(&ctx, false, vec![key(egui::Key::Space)]);
        assert!(
            output.viewport_output[&egui::ViewportId::ROOT]
                .commands
                .contains(&ViewportCommand::Minimized(true))
        );
    }
}
