//! English message IDs with Hebrew translations. Business state and filenames
//! remain language-neutral; translate only at presentation boundaries.
use eframe::egui::{self, Color32, FontFamily, FontId, TextFormat, WidgetText};
use std::{
    cell::Cell,
    collections::{BTreeMap, HashMap},
    io::{self, Read, Write},
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicU8, Ordering},
    },
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Language {
    English,
    Hebrew,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preference {
    System,
    English,
    Hebrew,
}
static PREFERENCE: AtomicU8 = AtomicU8::new(0);
thread_local! { static OVERRIDE: Cell<Option<Language>> = const { Cell::new(None) }; }
fn en() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    C.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/en.json")).expect("valid English catalog")
    })
}
fn he() -> &'static BTreeMap<String, String> {
    static C: OnceLock<BTreeMap<String, String>> = OnceLock::new();
    C.get_or_init(|| {
        serde_json::from_str(include_str!("../locales/he.json")).expect("valid Hebrew catalog")
    })
}
pub fn preference() -> Preference {
    match PREFERENCE.load(Ordering::Relaxed) {
        1 => Preference::English,
        2 => Preference::Hebrew,
        _ => Preference::System,
    }
}
pub fn language() -> Language {
    if let Some(language) = OVERRIDE.with(Cell::get) {
        return language;
    }
    match preference() {
        Preference::English => Language::English,
        Preference::Hebrew => Language::Hebrew,
        Preference::System => {
            static SYSTEM: OnceLock<Language> = OnceLock::new();
            *SYSTEM
                .get_or_init(|| language_for_locale(&sys_locale::get_locale().unwrap_or_default()))
        }
    }
}
pub fn is_rtl() -> bool {
    language() == Language::Hebrew
}
/// Logical layouts shared by pages, popups, and compound controls.
pub fn start_align() -> egui::Align {
    if is_rtl() {
        egui::Align::Max
    } else {
        egui::Align::Min
    }
}
pub fn column() -> egui::Layout {
    egui::Layout::top_down(start_align())
}
pub fn row() -> egui::Layout {
    if is_rtl() {
        egui::Layout::right_to_left(egui::Align::Center)
    } else {
        egui::Layout::left_to_right(egui::Align::Center)
    }
}
pub fn trailing_row() -> egui::Layout {
    if is_rtl() {
        egui::Layout::left_to_right(egui::Align::Center)
    } else {
        egui::Layout::right_to_left(egui::Align::Center)
    }
}

// egui's compound widgets place their indicator on the physical left. Keep
// standard button interaction/focus while placing Hebrew indicators at start.
fn rtl_choice(ui: &mut egui::Ui, selected: bool, text: WidgetText, radio: bool) -> egui::Response {
    let label = text.text().to_owned();
    let response = ui.add(egui::Button::new(text).right_text("      ").frame(false));
    let center = egui::pos2(
        response.rect.right() - ui.spacing().button_padding.x - 7.0,
        response.rect.center().y,
    );
    let stroke = ui.style().interact(&response).fg_stroke;
    if radio {
        ui.painter().circle_stroke(center, 7.0, stroke);
        if selected {
            ui.painter().circle_filled(center, 3.5, stroke.color);
        }
    } else {
        let rect = egui::Rect::from_center_size(center, egui::vec2(14.0, 14.0));
        ui.painter()
            .rect_stroke(rect, 2, stroke, egui::StrokeKind::Inside);
        if selected {
            ui.painter().add(egui::Shape::line(
                vec![
                    center + egui::vec2(-4.0, 0.0),
                    center + egui::vec2(-1.0, 3.0),
                    center + egui::vec2(4.0, -3.0),
                ],
                stroke,
            ));
        }
    }
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            if radio {
                egui::WidgetType::RadioButton
            } else {
                egui::WidgetType::Checkbox
            },
            ui.is_enabled(),
            selected,
            label.clone(),
        )
    });
    response
}
pub fn checkbox(ui: &mut egui::Ui, value: &mut bool, text: WidgetText) -> egui::Response {
    if !is_rtl() {
        return ui.checkbox(value, text);
    }
    let mut response = rtl_choice(ui, *value, text, false);
    if response.clicked() {
        *value = !*value;
        response.mark_changed();
    }
    response
}
fn language_for_locale(locale: &str) -> Language {
    if matches!(
        locale
            .split(['_', '-', '.'])
            .next()
            .unwrap_or("")
            .to_ascii_lowercase()
            .as_str(),
        "he" | "iw"
    ) {
        Language::Hebrew
    } else {
        Language::English
    }
}
fn settings_path() -> Option<PathBuf> {
    #[cfg(target_os = "windows")]
    let root = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let root =
        std::env::var_os("HOME").map(|p| PathBuf::from(p).join("Library/Application Support"));
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let root = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|p| PathBuf::from(p).join(".config")));
    root.map(|p| p.join("reclaim/language.json"))
}
pub fn init() {
    let Some(path) = settings_path() else {
        return;
    };
    if let Ok(file) = std::fs::File::open(path) {
        let mut value = String::new();
        if file.take(64).read_to_string(&mut value).is_ok() {
            let value = serde_json::from_str::<String>(&value).unwrap_or_default();
            PREFERENCE.store(
                match value.as_str() {
                    "en" => 1,
                    "he" => 2,
                    _ => 0,
                },
                Ordering::Relaxed,
            );
        }
    }
}
pub fn set_preference(value: Preference) -> io::Result<()> {
    let number = match value {
        Preference::System => 0,
        Preference::English => 1,
        Preference::Hebrew => 2,
    };
    PREFERENCE.store(number, Ordering::Relaxed);
    if let Some(path) = settings_path() {
        std::fs::create_dir_all(path.parent().unwrap())?;
        let mut temporary = tempfile::NamedTempFile::new_in(path.parent().unwrap())?;
        let text = match value {
            Preference::System => "\"system\"",
            Preference::English => "\"en\"",
            Preference::Hebrew => "\"he\"",
        };
        temporary.write_all(text.as_bytes())?;
        temporary.as_file().sync_all()?;
        temporary.persist(path).map_err(|e| e.error)?;
    }
    Ok(())
}

fn fields(template: &str) -> Vec<(usize, usize, &str)> {
    let mut result = Vec::new();
    let mut at = 0;
    while let Some(start) = template[at..].find('{') {
        let start = at + start;
        let Some(end) = template[start..].find('}') else {
            break;
        };
        let end = start + end + 1;
        result.push((start, end, &template[start + 1..end - 1]));
        at = end;
    }
    result
}
fn capture<'a>(template: &str, text: &'a str) -> Option<Vec<&'a str>> {
    let tokens = fields(template);
    if tokens.is_empty() {
        return None;
    }
    let mut at = 0;
    let mut source = 0;
    let mut values = Vec::new();
    for (i, (start, end, _)) in tokens.iter().enumerate() {
        let literal = &template[source..*start];
        if !text[at..].starts_with(literal) {
            return None;
        }
        at += literal.len();
        let next = tokens
            .get(i + 1)
            .map(|(n, _, _)| *n)
            .unwrap_or(template.len());
        let after = &template[*end..next];
        let count = if after.is_empty() {
            text.len() - at
        } else {
            text[at..].find(after)?
        };
        values.push(&text[at..at + count]);
        at += count;
        source = *end;
    }
    if text[at..] != template[source..] {
        return None;
    }
    Some(values)
}
pub fn t(text: &str) -> String {
    translate(text, language(), 0)
}
fn translate(text: &str, language: Language, depth: usize) -> String {
    if language == Language::English || depth > 5 {
        return text.into();
    }
    if let Some(value) = he().get(text) {
        return value.clone();
    }
    // Longer patterns take precedence over generic count labels.
    let mut patterns: Vec<_> = en().keys().filter(|key| key.contains('{')).collect();
    patterns.sort_by_key(|key| std::cmp::Reverse(key.len()));
    for key in patterns {
        let Some(values) = capture(key, text) else {
            continue;
        };
        if matches!(
            key.as_str(),
            "{} files" | "{} file" | "{} selected" | "{} selected (one)"
        ) && values[0].parse::<usize>().is_err()
        {
            continue;
        }
        let single = values.first().is_some_and(|n| *n == "1");
        let singular = if key.as_str() == "{} selected" {
            "{} selected (one)".into()
        } else {
            key.replace("files", "file")
                .replace("candidates", "candidate")
        };
        let translated = if single {
            he().get(&singular).or_else(|| he().get(key))
        } else {
            he().get(key)
        };
        let Some(translated) = translated else {
            continue;
        };
        let tokens = fields(key);
        let output_tokens = fields(translated);
        let mut used = HashMap::new();
        let mut output = String::new();
        let mut at = 0;
        for (start, end, token) in output_tokens {
            output.push_str(&translated[at..start]);
            let index = used.entry(token).or_insert(0usize);
            if let Some((i, _)) = tokens
                .iter()
                .enumerate()
                .filter(|(_, (_, _, t))| *t == token)
                .nth(*index)
            {
                output.push_str(&translate(values[i], language, depth + 1));
            }
            *index += 1;
            at = end;
        }
        output.push_str(&translated[at..]);
        return output;
    }
    text.into()
}

pub fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    // Keep private-use icon glyphs separate from translated text and bidi runs.
    fonts.font_data.insert(
        "reclaim-icons".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/FontAwesome-Solid.ttf"
        ))),
    );
    fonts.families.insert(
        FontFamily::Name("reclaim-icons".into()),
        vec!["reclaim-icons".into()],
    );
    fonts.font_data.insert(
        "reclaim-hebrew".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/DejaVuSans.ttf"
        ))),
    );
    for family in [FontFamily::Proportional, FontFamily::Monospace] {
        let list = fonts.families.entry(family).or_default();
        if is_rtl() {
            list.insert(0, "reclaim-hebrew".into());
        } else {
            list.push("reclaim-hebrew".into());
        }
    }
    // Separate format sections keep the shaper from merging opposite-direction runs.
    for name in ["reclaim-bidi-ltr", "reclaim-bidi-rtl"] {
        fonts
            .families
            .insert(FontFamily::Name(name.into()), vec!["reclaim-hebrew".into()]);
    }
    ctx.set_fonts(fonts);
}
fn hebrew(text: &str) -> bool {
    text.chars().any(|c| ('\u{0590}'..='\u{05ff}').contains(&c))
}
fn bidi_job(
    ui: &egui::Ui,
    text: &str,
    format: TextFormat,
    width: f32,
    wrap: bool,
    align: egui::Align,
) -> egui::text::LayoutJob {
    let mut lines = Vec::<String>::new();
    for paragraph in text.split('\n') {
        if !wrap {
            lines.push(paragraph.into());
            continue;
        }
        let mut line = String::new();
        for word in paragraph.split_inclusive(' ') {
            let candidate = format!("{line}{word}");
            let measured = ui
                .painter()
                .layout_no_wrap(candidate.clone(), format.font_id.clone(), format.color)
                .size()
                .x;
            if !line.is_empty() && measured > width - 4.0 {
                lines.push(line.trim_end().into());
                line = word.into();
            } else {
                line = candidate;
            }
        }
        lines.push(line.trim_end().into());
    }
    let mut job = egui::text::LayoutJob {
        halign: align,
        ..Default::default()
    };
    job.wrap.max_width = width;
    for (i, line) in lines.iter().enumerate() {
        if i > 0 {
            job.append("\n", 0.0, format.clone());
        }
        let bidi = unicode_bidi::BidiInfo::new(line, Some(unicode_bidi::Level::rtl()));
        if let Some(paragraph) = bidi.paragraphs.first() {
            let (_, runs) = bidi.visual_runs(paragraph, paragraph.range.clone());
            for run in runs {
                let mut run_format = format.clone();
                run_format.font_id.family = FontFamily::Name(
                    if bidi.levels[run.start].is_rtl() {
                        "reclaim-bidi-rtl"
                    } else {
                        "reclaim-bidi-ltr"
                    }
                    .into(),
                );
                job.append(&line[run], 0.0, run_format);
            }
        }
    }
    job
}
pub fn widget(ui: &egui::Ui, text: impl Into<WidgetText>, wrap: bool) -> WidgetText {
    widget_width(
        ui,
        text,
        wrap,
        ui.available_width().max(40.0),
        if is_rtl() {
            egui::Align::Max
        } else {
            egui::Align::Min
        },
    )
}
pub fn widget_width(
    ui: &egui::Ui,
    text: impl Into<WidgetText>,
    wrap: bool,
    width: f32,
    align: egui::Align,
) -> WidgetText {
    formatted_widget(ui, text.into(), wrap, width, align, true)
}
fn formatted_widget(
    ui: &egui::Ui,
    text: WidgetText,
    wrap: bool,
    width: f32,
    align: egui::Align,
    translate_text: bool,
) -> WidgetText {
    let logical = if translate_text {
        t(text.text())
    } else {
        text.text().to_owned()
    };
    let original = text.clone().into_layout_job(
        ui.style(),
        egui::FontSelection::Default,
        egui::Align::Center,
    );
    if !hebrew(&logical) {
        if logical == text.text() {
            return text;
        }
        return logical.into();
    }
    let format = original
        .sections
        .first()
        .map(|s| s.format.clone())
        .unwrap_or_default();
    let width = if wrap { width } else { f32::INFINITY };
    let job = bidi_job(
        ui,
        &logical,
        format,
        width,
        wrap,
        if wrap { align } else { egui::Align::Min },
    );
    WidgetText::Galley(ui.fonts_mut(|fonts| fonts.layout_job(job)))
}
pub fn raw_widget(
    ui: &egui::Ui,
    text: &str,
    font: FontId,
    color: Color32,
    width: f32,
) -> WidgetText {
    if !hebrew(text) {
        return egui::RichText::new(text).font(font).color(color).into();
    }
    let mut job = bidi_job(
        ui,
        text,
        TextFormat::simple(font, color),
        width,
        false,
        egui::Align::Min,
    );
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = true;
    WidgetText::Galley(ui.fonts_mut(|fonts| fonts.layout_job(job)))
}

pub trait LocalizedUi {
    fn t_label(&mut self, text: impl Into<WidgetText>) -> egui::Response;
    fn t_heading(&mut self, text: impl Into<egui::RichText>) -> egui::Response;
    fn t_strong(&mut self, text: impl Into<egui::RichText>) -> egui::Response;
    fn t_weak(&mut self, text: impl Into<egui::RichText>) -> egui::Response;
    fn t_button(&mut self, text: impl Into<WidgetText>) -> egui::Response;
    fn t_radio_value<T: PartialEq>(
        &mut self,
        value: &mut T,
        alternative: T,
        text: impl Into<WidgetText>,
    ) -> egui::Response;
    fn t_selectable_value<T: PartialEq>(
        &mut self,
        value: &mut T,
        alternative: T,
        text: impl Into<WidgetText>,
    ) -> egui::Response;
}
impl LocalizedUi for egui::Ui {
    fn t_label(&mut self, text: impl Into<WidgetText>) -> egui::Response {
        let text = widget(self, text, true);
        self.label(text)
    }
    fn t_heading(&mut self, text: impl Into<egui::RichText>) -> egui::Response {
        let text = widget(self, text.into().heading().strong(), true);
        self.label(text)
    }
    fn t_strong(&mut self, text: impl Into<egui::RichText>) -> egui::Response {
        let text = widget(self, text.into().strong(), true);
        self.label(text)
    }
    fn t_weak(&mut self, text: impl Into<egui::RichText>) -> egui::Response {
        let text = widget(self, text.into().weak(), true);
        self.label(text)
    }
    fn t_button(&mut self, text: impl Into<WidgetText>) -> egui::Response {
        let text = widget(self, text, false);
        self.button(text)
    }
    fn t_radio_value<T: PartialEq>(
        &mut self,
        value: &mut T,
        alternative: T,
        text: impl Into<WidgetText>,
    ) -> egui::Response {
        let text = widget(self, text, false);
        if !is_rtl() {
            return self.radio_value(value, alternative, text);
        }
        let mut response = rtl_choice(self, *value == alternative, text, true);
        if response.clicked() && *value != alternative {
            *value = alternative;
            response.mark_changed();
        }
        response
    }
    fn t_selectable_value<T: PartialEq>(
        &mut self,
        value: &mut T,
        alternative: T,
        text: impl Into<WidgetText>,
    ) -> egui::Response {
        let text = widget(self, text, false);
        self.selectable_value(value, alternative, text)
    }
}
pub trait LocalizedResponse {
    fn t_on_hover_text(self, text: impl Into<WidgetText>) -> Self;
    fn raw_on_hover_text(self, text: impl Into<WidgetText>) -> Self;
}
impl LocalizedResponse for egui::Response {
    fn t_on_hover_text(self, text: impl Into<WidgetText>) -> Self {
        let text = text.into();
        self.on_hover_ui(|ui| {
            tooltip_content(ui, text, true);
        })
    }
    fn raw_on_hover_text(self, text: impl Into<WidgetText>) -> Self {
        let text = text.into();
        self.on_hover_ui(|ui| {
            tooltip_content(ui, text, false);
        })
    }
}

// Measure before placing an RTL column: placing it in the popup's default
// 500-point area first makes a two-word hint reserve that entire width.
fn tooltip_content(ui: &mut egui::Ui, text: WidgetText, translate_text: bool) -> egui::Response {
    let limit = (ui.ctx().viewport_rect().width() - 32.0).clamp(40.0, 320.0);
    let measured = formatted_widget(
        ui,
        text.clone(),
        true,
        limit,
        egui::Align::Min,
        translate_text,
    )
    .into_galley(
        ui,
        Some(egui::TextWrapMode::Wrap),
        limit,
        egui::TextStyle::Body,
    );
    let width = (measured.size().x.ceil() + 6.0).clamp(1.0, limit);
    let content = formatted_widget(ui, text, true, width, start_align(), translate_text);
    let mut job =
        (*content.into_layout_job(ui.style(), egui::FontSelection::Default, start_align())).clone();
    job.wrap.max_width = width;
    job.wrap.max_rows = 10;
    job.wrap.break_anywhere = true;
    let galley = ui.fonts_mut(|fonts| fonts.layout_job(job));
    ui.set_width(width);
    ui.with_layout(column(), |ui| {
        ui.add(egui::Label::new(WidgetText::Galley(galley)))
    })
    .inner
}

/// Scope presentation to a language without changing the saved preference.
pub fn with_language<R>(language: Language, f: impl FnOnce() -> R) -> R {
    struct Reset(Option<Language>);
    impl Drop for Reset {
        fn drop(&mut self) {
            OVERRIDE.with(|v| v.set(self.0));
        }
    }
    let _reset = Reset(OVERRIDE.with(|v| v.replace(Some(language))));
    f()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn catalogs_cover_all_keys_and_keep_format_arguments() {
        assert_eq!(
            en().keys().collect::<Vec<_>>(),
            he().keys().collect::<Vec<_>>()
        );
        for (key, translation) in he() {
            assert!(!translation.trim().is_empty());
            let mut source: Vec<_> = fields(key).into_iter().map(|(_, _, key)| key).collect();
            let mut target: Vec<_> = fields(translation)
                .into_iter()
                .map(|(_, _, key)| key)
                .collect();
            source.sort();
            target.sort();
            assert_eq!(source, target, "Placeholder mismatch: {key}");
        }
    }
    #[test]
    fn locale_fallback_counts_and_nested_errors() {
        assert_eq!(language_for_locale("he_IL.UTF-8"), Language::Hebrew);
        assert_eq!(language_for_locale("iw-IL"), Language::Hebrew);
        assert_eq!(language_for_locale("unknown"), Language::English);
        assert_eq!(translate("2 files", Language::Hebrew, 0), "2 קבצים");
        assert_eq!(translate("1 files", Language::Hebrew, 0), "1 קובץ");
        assert_eq!(
            translate("Scan failed: Invalid source offset", Language::Hebrew, 0),
            "הסריקה נכשלה: מיקום הנתונים במקור אינו תקין"
        );
        assert_eq!(
            translate("/media/user/Photos", Language::Hebrew, 0),
            "/media/user/Photos"
        );
        assert_eq!(
            translate("Search files…", Language::English, 0),
            "Search files…"
        );
    }
    #[test]
    fn tooltip_width_follows_content_in_english_and_hebrew() {
        for language in [Language::English, Language::Hebrew] {
            with_language(language, || {
                for (text, short) in [
                    ("Deselect all".to_owned(), true),
                    (
                        "For best results, stop using this drive or scan it from another computer."
                            .repeat(12),
                        false,
                    ),
                    ("/media/תמונות/".repeat(80), false),
                ] {
                    let ctx = egui::Context::default();
                    setup_fonts(&ctx);
                    let mut bounds = egui::Vec2::ZERO;
                    ctx.run_ui(
                        egui::RawInput {
                            screen_rect: Some(egui::Rect::from_min_size(
                                egui::Pos2::ZERO,
                                egui::vec2(900.0, 700.0),
                            )),
                            ..Default::default()
                        },
                        |ui| {
                            tooltip_content(ui, text.clone().into(), short);
                            bounds = ui.min_rect().size();
                        },
                    )
                    .drop_without_applying_deltas();
                    assert!(bounds.x <= 321.0, "tooltip too wide: {bounds:?}");
                    assert!(bounds.y < 220.0, "tooltip too tall: {bounds:?}");
                    if short {
                        assert!(bounds.x < 160.0, "short hint stretched: {bounds:?}");
                    }
                }
            });
        }
    }
}
