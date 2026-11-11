# Interface previews

The screenshots show the actual Slint components with sample source and result data. Generate software-rendered previews with:

```sh
RECLAIM_PREVIEW_DIR=/tmp/reclaim-previews cargo test --locked --bin reclaim slint_workspace
```

This renders Sources, Recovered files, and Settings & help across all seven languages in light and dark themes. English, Hebrew, and Arabic are also rendered at the compact window size. Filenames include locale, width, page, and theme. Native dialogs and physical device access need desktop testing.
