# Translations

English message IDs live in `en.json`. Bundled translations cover Hebrew (`he`),
Spanish (`es`), French (`fr`), German (`de`), Arabic (`ar`), and Russian (`ru`).
Every catalog contains the same keys and format placeholders. Catalog parity,
placeholder preservation, locale detection, and nested messages are checked by
`cargo test`. Translate app-owned text at its presentation boundary with
`i18n::widget`, `LocalizedUi`, or `i18n::t`. Do not translate filenames, paths,
model names, search queries, or exported bytes.

Settings offers System and each supported language, with language names shown
in their own language. System recognizes the language prefix of regional locales
such as `es-MX`, `fr-CA`, and `ar-SA`, plus legacy Hebrew `iw`. Unsupported locales
fall back to English. The choice persists in `reclaim/language.json` under the
platform's application configuration folder. Native dialogs follow the operating
system language.

Hebrew and Arabic use right-to-left layouts and bundled DejaVu Sans, licensed in
`assets/fonts/DejaVu-LICENSE.txt`. Static labels handle bidirectional runs and
wrapping; egui provides Arabic shaping. Mixed RTL/Latin search caret movement
and selection still need upstream support and live desktop validation. The
interface is not completely mirrored. Translation wording should receive native
speaker review before a release.
