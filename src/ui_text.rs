//! Localized Slint properties. The English catalog is the message ID source.
use crate::{AppWindow, Locale, Strings};
use reclaim::i18n;
use slint::ComponentHandle;
const WINDOW_TITLE: &str = "Reclaim - File Recovery";
const BRAND: &str = "Reclaim";
const RECOVERY_STEPS: &str = "1. Stop using the drive you lost files from. Don't save, download or install anything to it.\n\n2. Choose that drive or a raw disk image, then scan its unused space. Stop any time and keep partial results.\n\n3. Inspect and select files, then save them to a different physical drive.";

/// One table drives both the Slint setters and the catalog-coverage test, so a
/// label cannot be added to the interface without also being checked.
macro_rules! ui_strings {
    ($($setter:ident => $text:literal,)*) => {
        #[cfg(test)]
        pub const UI_MESSAGES: &[&str] = &[WINDOW_TITLE, BRAND, RECOVERY_STEPS, $($text),*];
        fn apply_labels(strings: &Strings<'_>) {
            $(strings.$setter(i18n::t($text).into());)*
        }
    };
}

ui_strings! {
    set_no_source_selected => "No source selected",
    set_select_a_file_to_inspect => "Select a file to inspect its details.",
    set_reclaim => "reclaim",
    set_a_second_chance_for_your => "A second chance for your files",
    set_sources => "Sources",
    set_recovered_files => "Recovered files",
    set_settings_help => "Settings & help",
    set_read_only_by_design => "Read-only by design",
    set_your_source_stays_untouched_save => "Your source stays untouched. Save recovered files to another drive.",
    set_choose_a_drive_or_disk => "Choose a drive or disk image to search for deleted files.",
    set_inspect_candidates_choose_your_files => "Inspect candidates, choose your files, and recover them safely.",
    set_appearance_updates_and_a_little => "Appearance, updates, and a little guidance.",
    set_available_sources => "AVAILABLE SOURCES",
    set_refresh => "Refresh",
    set_open_disk_image => "Open disk image",
    set_no_drives_found_open_a => "No drives found. Open a disk image or connect a drive and refresh.",
    set_selected_source => "SELECTED SOURCE",
    set_start_scanning => "Start scanning",
    set_search_name_or_offset => "Search name or offset…",
    set_all_files => "All files",
    set_pictures => "Pictures",
    set_documents => "Documents",
    set_archives => "Archives",
    set_audio => "Audio",
    set_video => "Video",
    set_applications => "Applications",
    set_icons => "Icons",
    set_select_all => "Select all",
    set_choose_a_drive => "Choose a drive",
    set_minimize => "Minimize",
    set_maximize => "Maximize",
    set_restore => "Restore",
    set_close => "Close",
    set_save_to_a_different_physical_drive => "Save to a different physical drive",
    set_separator => " · ",
    set_searching_candidates_will_appear_here => "Searching… candidates will appear here.",
    set_no_matching_files_yet_start => "No matching files yet. Start a scan from Sources.",
    set_file_inspector => "FILE INSPECTOR",
    set_candidates_may_be_incomplete_verify => "Candidates may be incomplete. Verify recovered files before relying on them.",
    set_recover_selected => "Recover selected",
    set_system_language => "System language",
    set_light => "Light",
    set_dark => "Dark",
    set_system => "System",
    set_general => "General",
    set_about => "About",
    set_version => "Version",
    set_recover_in_three_steps => "Recover in three steps",
    set_supported_formats => "Supported formats",
    set_photos_documents_archives_audio_video => "Photos, documents, archives, audio, video, applications, and small NTFS resident files. Unused-space scanning on FAT, exFAT, and NTFS volumes.",
    set_check_for_updates => "Check for updates",
    set_project_website => "Project website",
    set_built_with_rust_and_slint => "Built with Rust and Slint · MIT licensed application",
    set_stop => "Stop",
    set_language => "Language",
    set_appearance => "Appearance",
}

pub fn apply(ui: &AppWindow) {
    ui.global::<Locale>().set_rtl(i18n::is_rtl());
    let strings = ui.global::<Strings>();
    apply_labels(&strings);
    strings.set_recovery_step_list(slint::ModelRc::new(slint::VecModel::from(recovery_steps())));
    strings.set_window_title(
        window_title(i18n::t(WINDOW_TITLE), i18n::is_rtl(), draws_own_title_bar()).into(),
    );
    ui.set_version(env!("CARGO_PKG_VERSION").into());
}

/// The catalog keeps the steps as one numbered paragraph; the interface draws its own
/// number badges, so each step is passed without its "1. " prefix.
fn recovery_steps() -> Vec<slint::SharedString> {
    i18n::t(RECOVERY_STEPS)
        .split("\n\n")
        .map(|step| {
            step.trim_start_matches(|c: char| c.is_ascii_digit() || c == '.')
                .trim()
                .into()
        })
        .collect()
}

/// On Wayland the toolkit draws the title bar itself, one glyph at a time from a single
/// font: no font fallback, no shaping and no right-to-left ordering. Hebrew and Arabic
/// titles come out as empty boxes there, so those sessions show the brand name alone.
fn window_title(localized: String, rtl: bool, draws_own_title_bar: bool) -> String {
    if rtl && draws_own_title_bar {
        i18n::t(BRAND)
    } else {
        localized
    }
}

fn draws_own_title_bar() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some_and(|v| !v.is_empty())
}

#[cfg(test)]
mod tests {
    use super::window_title;

    #[test]
    fn rtl_title_falls_back_to_brand_only_where_the_title_bar_cannot_draw_it() {
        let hebrew = "Reclaim - שחזור קבצים".to_string();
        assert_eq!(window_title(hebrew.clone(), true, true), "Reclaim");
        assert_eq!(window_title(hebrew.clone(), true, false), hebrew);
        let english = "Reclaim - File Recovery".to_string();
        assert_eq!(window_title(english.clone(), false, true), english);
    }
}
