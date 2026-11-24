//! Localized Slint properties. The English catalog is the message ID source.
// Hardened module: no unchecked indexing or arithmetic in shipped code (CWE-125, CWE-190).
#![cfg_attr(
    not(test),
    deny(clippy::indexing_slicing, clippy::arithmetic_side_effects)
)]
use crate::{AppWindow, LanguageFlags, Locale, Strings};
use reclaim::i18n;
use slint::ComponentHandle;
const WINDOW_TITLE: &str = "Reclaim - File Recovery";
const BRAND: &str = "Reclaim";
const SUPPORTED_FORMATS: &str = "Photos, documents, archives, audio, video, applications, and small NTFS resident files. Unused-space scanning on {0} volumes.";
/// Filesystem names are the same in every language and always read left to right.
const FILESYSTEMS: &[&str] = &["FAT", "exFAT", "NTFS", "ext2/3/4", "HFS+"];
const RECOVERY_STEPS: &str = "1. Stop using the drive you lost files from. Don't save, download or install anything to it.\n\n2. Choose that drive or a raw disk image, then scan its unused space. Stop any time and keep partial results.\n\n3. Inspect and select files, then save them to a different physical drive.";

/// One table drives both the Slint setters and the catalog-coverage test, so a
/// label cannot be added to the interface without also being checked.
macro_rules! ui_strings {
    ($($setter:ident => $text:literal,)*) => {
        #[cfg(test)]
        pub const UI_MESSAGES: &[&str] = &[WINDOW_TITLE, BRAND, RECOVERY_STEPS, SUPPORTED_FORMATS, $($text),*];
        fn apply_labels(strings: &Strings<'_>) {
            $(strings.$setter(i18n::t($text).into());)*
        }
    };
}

ui_strings! {
    set_sector_viewer => "Sector viewer",
    set_view_bytes => "View bytes",
    set_partition_bytes => "View partition bytes",
    set_compare_image => "Compare image",
    set_close_comparison => "Close comparison",
    set_viewer_address => "Byte offset or LBA",
    set_jump => "Go",
    set_previous => "Previous",
    set_next => "Next",
    set_byte_legend => "Source on the left, comparison on the right. ^^ marks differences and -- marks missing bytes.",
    set_checking_drive => "Reading drive information…",
    set_health_details => "Device readings",
    set_inspect_partition => "Inspect partition",
    set_partition_table => "Partition table",
    set_drive_health => "SMART and NVMe health",
    set_quick_check => "Quick Drive Check",
    set_no_source_selected => "No source selected",
    set_select_a_file_to_inspect => "Select a file to inspect its details.",
    set_reclaim => "Reclaim",
    set_a_second_chance_for_your => "A second chance for your files",
    set_sources => "Sources",
    set_recovered_files => "Recovered files",
    set_settings_help => "Settings & help",
    set_read_only_by_design => "Read-only by design",
    set_scanning_only_reads => "Scanning only reads the drive.",
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
    set_choose_a_source => "Choose a source",
    set_choose_a_source_above => "Choose a source above to start scanning.",
    set_scan_stopped_before_finding => "The scan stopped before finding any files. See the details below.",
    set_minimize => "Minimize",
    set_maximize => "Maximize",
    set_restore => "Restore",
    set_close => "Close",
    set_save_to_a_different_physical_drive => "Save to a different physical drive",
    set_separator => " · ",
    set_searching_candidates_will_appear_here => "Searching… candidates will appear here.",
    set_no_scan_yet => "No scan yet. Choose a source and start a scan.",
    set_no_files_were_found => "No files were found. Try another source.",
    set_no_files_match_filters => "No files match your search and category filters.",
    set_clear_filters => "Clear filters",
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
    set_check_for_updates => "Check for updates",
    set_restart_as_administrator => "Restart as administrator",
    set_whats_new => "What’s new",
    set_updates => "Updates",
    set_later => "Later",
    set_hide_details => "Hide details",
    set_update_when_ready => "You can keep working and update later.",
    set_update_after_work => "Finish the scan or recovery first, then update.",
    set_no_release_notes => "No release notes.",
    set_source_code => "Source code",
    set_report_a_problem => "Report a problem",
    set_open_an_issue_on_github => "Open an issue on GitHub",
    set_release_history => "Release history",
    set_every_version_with_its_notes => "Every version with its release notes",
    set_license => "License",
    set_read_the_mit_license => "Read the MIT License",
    set_built_with_rust_and_slint => "Built with Rust and Slint · MIT licensed application",
    set_stop_scan => "Stop scan",
    set_open_folder => "Open folder",
    set_tick_files_to_recover => "Tick the files you want to recover.",
    set_clear_search => "Clear search",
    set_stop_recovery => "Stop recovery",
    set_review_recovery_destination => "Review recovery destination",
    set_saving_here_can_overwrite => "Saving here can overwrite the deleted files you are trying to recover and damage them permanently. Choose a folder on a different physical drive, such as a USB drive.",
    set_saving_to_the_same_drive => "Saving recovered files to the same physical drive you scanned can overwrite and permanently damage data you are trying to recover. Choose a folder on a different physical drive. Continue only if this destination is on another drive.",
    set_choose_another_folder => "Choose another folder",
    set_save_here_anyway => "Save here anyway",
    set_cancel => "Cancel",
    set_phones_and_cameras_connect_for => "Phones and cameras connect for file transfer only, so Reclaim can’t search them for deleted files. If yours has a memory card, put the card in a card reader and scan it here. Also check its Trash or Recently deleted folder.",
    set_language => "Language",
    set_appearance => "Appearance",
    set_debug_logging => "Debug logging",
    set_debug_log_description => "Records scan and recovery checkpoints in reclaim-debug.log.",
    set_enable_logging => "Enable logging",
    set_disable_logging => "Disable logging",
    set_diagnostic_write_error => "The diagnostic log could not be written.",
    set_diagnostic_open_error => "The diagnostic log folder could not be opened.",
}

pub fn apply(ui: &AppWindow) {
    ui.global::<Locale<'_>>().set_rtl(i18n::is_rtl());
    let strings = ui.global::<Strings<'_>>();
    apply_labels(&strings);
    let language_names: Vec<slint::SharedString> = std::iter::once(strings.get_system_language())
        .chain(i18n::LANGUAGE_NAMES.iter().copied().map(Into::into))
        .collect();
    ui.set_language_names(slint::ModelRc::new(slint::VecModel::from(language_names)));
    let flags = ui.global::<LanguageFlags<'_>>();
    let language_flags: Vec<_> = std::iter::once(flags.get_system())
        .chain(i18n::LANGUAGE_CODES.iter().map(|&code| match code {
            "en" => flags.get_gb(),
            "he" => flags.get_il(),
            "es" => flags.get_es(),
            "fr" => flags.get_fr(),
            "de" => flags.get_de(),
            "ar" => flags.get_sa(),
            "ru" => flags.get_ru(),
            "pt" => flags.get_pt(),
            "it" => flags.get_it(),
            "nl" => flags.get_nl(),
            _ => flags.get_system(),
        }))
        .collect();
    ui.set_language_flags(slint::ModelRc::new(slint::VecModel::from(language_flags)));
    strings.set_photos_documents_archives_audio_video(
        // Not `message`: it would merge the separately isolated names back into one run.
        i18n::t(SUPPORTED_FORMATS)
            .replace("{0}", &filesystem_list())
            .into(),
    );
    strings.set_recovery_step_list(slint::ModelRc::new(slint::VecModel::from(recovery_steps())));
    strings.set_window_title(
        window_title(i18n::t(WINDOW_TITLE), i18n::is_rtl(), draws_own_title_bar()).into(),
    );
    ui.set_version(env!("CARGO_PKG_VERSION").into());
}

/// Each filesystem name is its own left-to-right run, so the slashes of ext2/3/4 and
/// the plus of HFS+ stay put inside right-to-left text, while the commas between
/// names follow the sentence and the line can still wrap between them.
fn filesystem_list() -> String {
    FILESYSTEMS
        .iter()
        .map(|name| i18n::ltr(name))
        .collect::<Vec<_>>()
        .join(", ")
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
