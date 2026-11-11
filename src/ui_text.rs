//! Localized Slint properties. The English catalog is the message ID source.
use crate::{AppWindow, Locale, Strings};
use reclaim::i18n;
use slint::ComponentHandle;
#[cfg(test)]
pub const UI_MESSAGES: &[&str] = &[
    "No source selected",
    "Select a file to inspect its details.",
    "reclaim",
    "A second chance for your files",
    "Sources",
    "Recovered files",
    "Settings & help",
    "Read-only by design",
    "Your source stays untouched. Save recovered files to another drive.",
    "RECLAIM  /  0.1.11",
    "Choose a drive or disk image to search for deleted files.",
    "Inspect candidates, choose your files, and recover them safely.",
    "Appearance, updates, and a little guidance.",
    "AVAILABLE SOURCES",
    "Refresh",
    "Open disk image",
    "No drives found. Open a disk image or connect a drive and refresh.",
    "SELECTED SOURCE",
    "Start scanning",
    "Search name or offset…",
    "All files",
    "Pictures",
    "Documents",
    "Archives",
    "Audio",
    "Video",
    "Applications",
    "Icons",
    "Select all",
    "Clear",
    " · ",
    "Searching… candidates will appear here.",
    "No matching files yet. Start a scan from Sources.",
    "FILE INSPECTOR",
    "Candidates may be incomplete. Verify recovered files before relying on them.",
    "Recover selected",
    "System language",
    "Light",
    "Dark",
    "System",
    "General",
    "About",
    "Version",
    "Recover in three steps",
    "1. Choose the original drive or a raw disk image.\n\n2. Scan unused space. Stop any time and keep partial results.\n\n3. Inspect and select candidates, then save to a different physical drive.",
    "Supported formats",
    "Photos, documents, archives, audio, video, applications, and small NTFS resident files. FAT and NTFS unused-space scanning.",
    "Check for updates",
    "Project website",
    "Built with Rust and Slint · MIT licensed application",
    "Stop",
    "Reclaim — File Recovery",
    "Language",
    "Appearance",
];
pub fn apply(ui: &AppWindow) {
    ui.global::<Locale>().set_rtl(i18n::is_rtl());
    let strings = ui.global::<Strings>();
    strings.set_no_source_selected(i18n::t("No source selected").into());
    strings.set_select_a_file_to_inspect(i18n::t("Select a file to inspect its details.").into());
    strings.set_reclaim(i18n::t("reclaim").into());
    strings.set_a_second_chance_for_your(i18n::t("A second chance for your files").into());
    strings.set_sources(i18n::t("Sources").into());
    strings.set_recovered_files(i18n::t("Recovered files").into());
    strings.set_settings_help(i18n::t("Settings & help").into());
    strings.set_read_only_by_design(i18n::t("Read-only by design").into());
    strings.set_your_source_stays_untouched_save(
        i18n::t("Your source stays untouched. Save recovered files to another drive.").into(),
    );
    strings.set_reclaim_0_1_11(i18n::t("RECLAIM  /  0.1.11").into());
    strings.set_choose_a_drive_or_disk(
        i18n::t("Choose a drive or disk image to search for deleted files.").into(),
    );
    strings.set_inspect_candidates_choose_your_files(
        i18n::t("Inspect candidates, choose your files, and recover them safely.").into(),
    );
    strings.set_appearance_updates_and_a_little(
        i18n::t("Appearance, updates, and a little guidance.").into(),
    );
    strings.set_available_sources(i18n::t("AVAILABLE SOURCES").into());
    strings.set_refresh(i18n::t("Refresh").into());
    strings.set_open_disk_image(i18n::t("Open disk image").into());
    strings.set_no_drives_found_open_a(
        i18n::t("No drives found. Open a disk image or connect a drive and refresh.").into(),
    );
    strings.set_selected_source(i18n::t("SELECTED SOURCE").into());
    strings.set_start_scanning(i18n::t("Start scanning").into());
    strings.set_search_name_or_offset(i18n::t("Search name or offset…").into());
    strings.set_all_files(i18n::t("All files").into());
    strings.set_pictures(i18n::t("Pictures").into());
    strings.set_documents(i18n::t("Documents").into());
    strings.set_archives(i18n::t("Archives").into());
    strings.set_audio(i18n::t("Audio").into());
    strings.set_video(i18n::t("Video").into());
    strings.set_applications(i18n::t("Applications").into());
    strings.set_icons(i18n::t("Icons").into());
    strings.set_select_all(i18n::t("Select all").into());
    strings.set_clear(i18n::t("Clear").into());
    strings.set_separator(i18n::t(" · ").into());
    strings.set_searching_candidates_will_appear_here(
        i18n::t("Searching… candidates will appear here.").into(),
    );
    strings.set_no_matching_files_yet_start(
        i18n::t("No matching files yet. Start a scan from Sources.").into(),
    );
    strings.set_file_inspector(i18n::t("FILE INSPECTOR").into());
    strings.set_candidates_may_be_incomplete_verify(
        i18n::t("Candidates may be incomplete. Verify recovered files before relying on them.")
            .into(),
    );
    strings.set_recover_selected(i18n::t("Recover selected").into());
    strings.set_system_language(i18n::t("System language").into());
    strings.set_light(i18n::t("Light").into());
    strings.set_dark(i18n::t("Dark").into());
    strings.set_system(i18n::t("System").into());
    strings.set_general(i18n::t("General").into());
    strings.set_about(i18n::t("About").into());
    strings.set_version(i18n::t("Version").into());
    strings.set_recovery_step_list(slint::ModelRc::new(slint::VecModel::from(recovery_steps())));
    ui.set_version(env!("CARGO_PKG_VERSION").into());
    strings.set_recover_in_three_steps(i18n::t("Recover in three steps").into());
    strings.set_supported_formats(i18n::t("Supported formats").into());
    strings.set_photos_documents_archives_audio_video(i18n::t("Photos, documents, archives, audio, video, applications, and small NTFS resident files. FAT and NTFS unused-space scanning.").into());
    strings.set_check_for_updates(i18n::t("Check for updates").into());
    strings.set_project_website(i18n::t("Project website").into());
    strings.set_built_with_rust_and_slint(
        i18n::t("Built with Rust and Slint · MIT licensed application").into(),
    );
    strings.set_stop(i18n::t("Stop").into());
    strings.set_window_title(
        window_title(
            i18n::t("Reclaim — File Recovery"),
            i18n::is_rtl(),
            draws_own_title_bar(),
        )
        .into(),
    );
    strings.set_language(i18n::t("Language").into());
    strings.set_appearance(i18n::t("Appearance").into());
}

/// The catalog keeps the steps as one numbered paragraph; the interface draws its own
/// number badges, so each step is passed without its "1. " prefix.
fn recovery_steps() -> Vec<slint::SharedString> {
    i18n::t("1. Choose the original drive or a raw disk image.\n\n2. Scan unused space. Stop any time and keep partial results.\n\n3. Inspect and select candidates, then save to a different physical drive.")
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
        i18n::t("Reclaim")
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
        let hebrew = "Reclaim — שחזור קבצים".to_string();
        assert_eq!(window_title(hebrew.clone(), true, true), "Reclaim");
        assert_eq!(window_title(hebrew.clone(), true, false), hebrew);
        let english = "Reclaim — File Recovery".to_string();
        assert_eq!(window_title(english.clone(), false, true), english);
    }
}
