"""Regenerate bundled icons: uv run --with fonttools==4.68.0 scripts/subset-icons.py.

The full upstream font and license remain in assets/fonts. This optional
maintainer command is not needed for builds or the manual project checker.
"""
from pathlib import Path
import re

from fontTools import subset
from fontTools.ttLib import TTFont

ROOT = Path(__file__).resolve().parents[1]
font = TTFont(ROOT / 'assets/fonts/FontAwesome-Solid.ttf', recalcTimestamp=False)
codepoints = {int(value, 16) for value in re.findall(
    r"\\u\{([fF][0-9a-fA-F]{3})\}", (ROOT / 'src/theme.rs').read_text())}
if not codepoints:
    raise ValueError('No icon glyphs found')
missing = codepoints - font.getBestCmap().keys()
if missing:
    raise ValueError(f'Missing upstream glyphs: {missing}')
options = subset.Options()
options.recalc_timestamp = False
subsetter = subset.Subsetter(options=options)
subsetter.populate(unicodes=codepoints)
subsetter.subset(font)
# Rename the modified font; retain all upstream copyright/license records.
for record in font['name'].names:
    replacements = {1: 'Reclaim Icons', 2: 'Regular', 3: 'Reclaim Icons Regular',
                    4: 'Reclaim Icons', 6: 'ReclaimIcons-Regular',
                    16: 'Reclaim Icons', 17: 'Regular'}
    if record.nameID in replacements:
        record.string = replacements[record.nameID].encode(record.getEncoding())
output = ROOT / 'assets/fonts/Reclaim-Icons.ttf'
font.save(output)
assert codepoints <= TTFont(output).getBestCmap().keys()
print(f'{len(codepoints)} icon glyphs; {output.stat().st_size} bytes')
