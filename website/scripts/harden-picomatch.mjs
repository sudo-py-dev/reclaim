import { readFileSync, writeFileSync } from 'node:fs';

const lock = JSON.parse(readFileSync(new URL('../package-lock.json', import.meta.url), 'utf8'));
const copies = Object.entries(lock.packages).filter(([path]) => path.endsWith('/picomatch'));
const supportedVersions = new Set(['2.3.2', '4.0.7']);
const original = "  if (typeof opts.noext === 'boolean') {\n    opts.noextglob = opts.noext;\n  }";
const hardened = '  // Reclaim disables extglobs pending an upstream fix for CVE-2026-112366.\n  opts.noextglob = true;';

if (copies.length === 0) throw new Error('Recheck the Picomatch mitigation after dependency changes.');

for (const [path, { version }] of copies) {
  if (!supportedVersions.has(version)) {
    throw new Error(`Recheck the Picomatch mitigation for ${path}@${version}.`);
  }

  const parser = new URL(`../${path}/lib/parse.js`, import.meta.url);
  const source = readFileSync(parser, 'utf8');
  if (source.includes(hardened)) continue;
  if (source.split(original).length !== 2) {
    throw new Error(`Unexpected Picomatch parser at ${path}@${version}.`);
  }
  writeFileSync(parser, source.replace(original, hardened));
}
