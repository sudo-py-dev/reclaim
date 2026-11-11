# Bundled fonts

`DejaVuSans.ttf` retains the complete text font for Hebrew and filename rendering.
`FontAwesome-Solid.ttf` is the original Font Awesome Free 6.7.2 Solid source.
The app embeds `Reclaim-Icons.ttf`, a renamed subset containing every icon used
by `src/theme.rs`, including both directions of RTL-aware icons.

Regenerate the subset after adding an icon:

```sh
uv run --with fonttools==4.68.0 scripts/subset-icons.py
```

The generated font is committed, so normal builds need no Python dependencies.
All original copyright/license records remain in the subset. The applicable
licenses are `FontAwesome-LICENSE.txt` and `DejaVu-LICENSE.txt`; packaging continues
to include both license files.

The Slint interface embeds DejaVu Sans regular and bold for consistent Latin, Hebrew, and Arabic rendering. Both are covered by `DejaVu-LICENSE.txt`.
