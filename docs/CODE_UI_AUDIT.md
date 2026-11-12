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

Everything runs with the normal Rust toolchain; there are no script dependencies.

```sh
cargo test --locked                      # all tests, including the project audit
cargo test --locked --test audit         # the audit on its own
cargo clippy --locked --all-targets -- -D warnings
```

The project audit (`tests/audit`) parses the source and checks, with each rule
tagged by CWE: `unsafe` confined to the platform layer and documented; no
`static mut`, `transmute`, leaks or unreviewed uninitialised memory; programs
started only by the two native launchers; no plain HTTP, TLS bypasses, weak
hashes or committed secrets; private temporary files; explicit bounds on reads
and allocations sized by scanned data; a ratchet on unchecked indexing; no debug
leftovers, silenced warnings, orphan files or em dashes in user-facing text;
every interface message translated, no dead catalog keys, matching placeholders,
and `ui/app.slint` defaults in step with their translations; exact dependency
versions, current advisory exceptions and a fuzz lockfile that matches
production. A finding can be accepted only next to its code with
`// audit-allow(RULE): reason`, and unused acceptances fail too.

`Cargo.toml` sets the compiler-enforced half (`[lints]`: no `unwrap`, `expect`,
`panic`, stray prints or undocumented `unsafe` in shipped code), and CI runs
`cargo-deny` against the RustSec advisory database. `RECLAIM_PREVIEW_DIR` with
`cargo test --bin reclaim slint_workspace` still writes headless UI renderings.

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

The Rust project audit enforces the file/catalog rules. Tests and compiler checks
cover selected behavior. The architectural rules above still require code review;
text matching cannot prove UI responsiveness or memory safety.

## Verification and limits

Headless previews cover minimum/default sizes, both appearances, Hebrew layouts,
settings, scan progress, and results.

Automatic layout checks (`src/layout_checks.rs`, run by
`layout_is_clear_and_steady_on_every_page_language_and_size`) go further than
previews: on every page, with no source, during a scan with long file names, and
with the update notice open, in English, Hebrew, Arabic and German, at
900x640 and 1160x780, every button, checkbox, dropdown and text field must lie
inside the window and must not overlap another control. Controls inside a
scroll view are judged by the part that is visible. The scan status headline
must keep the same size and position whether it says "scanning" or names a
slow check. Representative source selection, results,
Hebrew, and active scan previews were visually inspected. Automated interaction
coverage includes keyboard controls, modal behavior, filtering/export, and window
commands. Hardware/device tests and the live HTTPS test remain opt-in.

A headless render does not verify the actual compositor, native dialogs, screen
readers, authentication, or Windows/macOS hardware behavior. The file list
updates incrementally: a refresh appends new results and patches rows whose
selection changed, and rebuilds only when the search, category or result list
changes; a test streams 2,000 results and checks the list always equals a full
rebuild. Image
previews remain sequential background work with bounded decoding limits.
