# Code and interface audit

Reviewed 2026-11-02. Scope: scanner and ZIP classification, result filtering,
thumbnail loading/caching, desktop event handling, and existing UI regression tests.

## Changes

| Finding | Correction |
| --- | --- |
| Empty searches constructed filenames and searchable strings for every result | Skip text construction for an empty query |
| Nonempty searches lowercased individual fields separately | Lowercase the combined search text once, preserving cross-field matching |
| Thumbnail cache eviction repeated inside the visible-row loop | Evict once per rendered row range; binary search sorted visible indices |
| Result widget IDs used source offsets, which need not be unique | Scope each row by its stable result index within the current scan |
| Resident thumbnail loading copied the complete compressed image | Borrow existing immutable bytes; own only bytes read from disk |
| ZIP package detection retained every archive entry name | Track only the fixed set of recognized package markers |
| ZIP classification searched arbitrary amounts of payload for the footer | Search only the final 65,557 bytes permitted for a classic ZIP footer and comment |
| ZIP classification repeated directory checks already completed by `valid_zip` | Reuse validated directory bounds |

These reduce work and allocations. No end-to-end throughput percentage has been
measured; disk access and decoding can still dominate performance.

## Repeatable checks

Requires Python 3.11+ and the project's Rust toolchain/dependencies.

```sh
python3 scripts/check_project.py
python3 scripts/check_project.py --full
python3 scripts/check_project.py --full --previews /tmp/reclaim-previews
```

The fast check parses Python scripts without executing them, rejects duplicate
translation keys, compares catalog keys and numbered placeholder counts, rejects
empty translations, and checks that the application version matches Cargo.lock.
The full check also runs Python rule tests, Rust formatting, all default Rust
tests, and Clippy with warnings treated as errors. The default Rust suite includes
UI behavior and layout regressions. `--previews` writes headless PPM renderings.
These audit checks run only when you invoke the script; no GitHub Actions steps were added.

## Rules for future changes

- Keep disk reads, decoding, downloads, and authentication off the render thread.
- Keep scan queues bounded and cancellation checks inside long-running loops.
- Preserve read-only source access and export without overwriting existing files.
- Use unique, stable widget IDs; preserve virtualization of long result lists.
- Avoid allocating per-result search strings when the search is empty.
- Preserve English/Hebrew catalogs, placeholders, keyboard controls, and RTL data.
- Check minimum and default window sizes in light and dark appearances.
- Keep image decoding and candidate parsing bounded; validate untrusted sizes.
- Prefer existing components and dependencies; simplify only with equivalent behavior.

The Python checks enforce the file/catalog rules. Rust tests and compiler checks
cover selected behavior. The architectural rules above still require code review;
text matching cannot prove UI responsiveness or memory safety.

## Verification and limits

Headless previews cover minimum/default sizes, both appearances, Hebrew layouts,
settings, scan progress, and results. Representative source selection, results,
Hebrew, and active scan previews were visually inspected. Automated interaction
coverage includes keyboard controls, modal behavior, filtering/export, and window
commands. Hardware/device tests and the live HTTPS test remain opt-in.

A headless render does not verify the actual compositor, native dialogs, screen
readers, authentication, or Windows/macOS hardware behavior. Result filtering is
still linear in the number of candidates each render; caching it would require
careful invalidation when scan results, filters, and filenames change. Image
previews remain sequential background work with bounded decoding limits.
