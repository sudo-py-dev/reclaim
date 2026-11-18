# Translations and text direction

English message IDs live in `en.json`, with complete matching catalogs for Hebrew
(`he`), Arabic (`ar`), Spanish (`es`), French (`fr`), German (`de`), and Russian
(`ru`). `ui/app.slint` exposes named localized properties; `src/ui_text.rs` fills
them when the language changes. Status messages, file details, previews, drive
metadata, and recovery warnings are localized at their presentation boundaries.
Release notes and operating-system error details retain their original language.

Hebrew and Arabic mirror the sidebar, brand header, action bars, source and result
rows, file inspector, and category/language dropdowns. Labels and search input
align to the reading direction. Text uses bundled DejaVu Sans regular and bold,
including Hebrew and Arabic glyphs and bidirectional shaping. Icons use semantic
colors for light and dark themes; the sidebar theme action uses sun/moon SVGs.

Use `i18n::t` for app-owned messages and `i18n::message` for numbered placeholders
with data arguments. The latter isolates inserted values and never recursively
translates them. `i18n::isolate` protects mixed-direction filenames, paths, offsets,
and dates for display. Stored paths, original names, search queries, scan results,
and recovered bytes remain unchanged. Brand names, format identifiers, and binary
size units remain language-neutral.

Settings offers System and each supported language, using their native names.
System recognizes regional prefixes such as `es-MX`, `fr-CA`, and `ar-SA`, plus
legacy Hebrew `iw`. Unsupported locales fall back to English. The preference is
saved in `reclaim/language.json` in the platform configuration directory. Native
file pickers use operating-system controls and language.

Catalog parity, placeholder preservation, static UI coverage, mixed-direction data,
keyboard dropdown selection, and all pages in both themes are checked by tests.
RTL layouts also render at the compact window size. To inspect the renders:

```sh
RECLAIM_PREVIEW_DIR=/tmp/reclaim-previews cargo test --locked --bin reclaim slint_workspace
```

Names include locale, window width, page number, and theme, for example
`he-900-1-light.png`. Live desktop input-method behavior still needs platform
validation; translation wording benefits from native-speaker review.

## Generate catalogs

The language list in `languages.txt` includes 20 targets counting English.
A locale appears in Settings and is recognized by System after its JSON catalog
is added and the app is rebuilt. The translation command fills missing entries,
keeps valid existing translations, and writes the catalog once every English
message has a translation.

Run with `curl` on your PATH. The generator uses MyMemory's free public
translation endpoint without an API key. Anonymous use is limited to 5,000
characters per day, with a maximum of 500 UTF-8 bytes per request. Generating
all 13 new catalogs will take multiple quota resets. [API specification](https://mymemory.translated.net/doc/spec.php) · [Usage limits](https://mymemory.translated.net/doc/usagelimits.php)

```sh
cargo xtask translate --list
cargo xtask translate it pt
cargo xtask translate
```

The last command processes all configured non-English locales. Each translated
message is saved to the ignored `.translation-progress` directory. Transient
HTTP failures use bounded retries with `Retry-After` support. If MyMemory
reports that the daily quota is exhausted, rerun after the reset to resume.
Review the generated wording, especially recovery warnings, before
committing the catalogs. Run `cargo test --locked --lib i18n` to check message
coverage and placeholders.
