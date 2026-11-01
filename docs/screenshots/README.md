# Interface screenshots

These PNGs show the current Reclaim 0.1.0 interface with sample drives and file results. They are rendered from the app’s actual egui meshes using the existing headless preview test, not mockups or photographs of real recovered data.

To refresh them from the repository root:

```sh
mkdir -p /tmp/reclaim-readme-preview
RECLAIM_PREVIEW_DIR=/tmp/reclaim-readme-preview cargo test --locked pages_render_at_minimum_and_default_sizes_in_both_themes
```

Convert `page-0-dark-920.ppm` to `reclaim-drives.png`. For the mixed file-type results screenshot, run:

```sh
RECLAIM_PREVIEW_DIR=/tmp/reclaim-readme-preview cargo test --locked file_type_icons_open_details_with_dates_and_unknown_deletion_time
```

Convert `file-types-English.ppm` to `reclaim-results.png`, keeping the original image dimensions. A PPM-capable image editor or Python’s Pillow library can perform the conversion.

The fixtures are illustrative. They do not establish real-device recovery success or test native desktop window behavior.
