# Translations

English message IDs live in `en.json`; `he.json` contains Hebrew translations.
Keep the same keys and format placeholders in both catalogs. Catalog parity and
placeholder preservation are checked by `cargo test`. Translate app-owned text
at its presentation boundary with `i18n::widget`, `LocalizedUi`, or `i18n::t`.
Do not translate filenames, paths, model names, search queries, or exported bytes.

The application menu offers System, English and Hebrew. System recognizes `he`
and legacy `iw` locales and otherwise falls back to English. The choice persists
in `reclaim/language.json` under the platform's application configuration folder.
Native dialog buttons follow the operating system language.

Hebrew uses bundled DejaVu Sans, licensed in `assets/fonts/DejaVu-LICENSE.txt`.
Static labels handle bidirectional runs and wrapping. egui 0.36 does not provide
complete bidirectional text editing: mixed Hebrew/Latin search caret movement and
selection still need upstream support and live desktop validation. The interface
is not completely mirrored.
