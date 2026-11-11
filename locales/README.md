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
