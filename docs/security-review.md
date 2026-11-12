# Security review, 2026-11-01

This pass reviewed carving and validation, NTFS resident records, recovery output,
settings persistence, updater metadata/downloads, and native drive discovery.
It is a focused source review, not a security certification.

## Changes

- The library forbids unsafe Rust at compile time. This covers first-party
  library code, not dependency internals or the separate GUI executable.
- Recovery uses a randomly named private temporary file in the destination
  directory. It checks completeness and image validity through that open handle,
  syncs it, then publishes without replacing an existing destination. Failures
  clean up the temporary file instead of deleting a potentially replaced final
  path. Unix recovered files have mode 0600.
- Settings use a private temporary file and atomic replacement instead of a
  predictable `.tmp` pathname that could follow a pre-existing symlink.
- Original and generated recovery filenames are sanitized. Unicode direction
  controls are replaced to reduce extension/name spoofing.
- Update URLs must match the exact project, release tag, and platform asset.
  Download verification independently enforces size and digest bounds, including
  when callers construct public release metadata directly.
- Updates install only when a Minisign (Ed25519) signature made by the release
  key verifies over the downloaded bytes. The public key is compiled in from
  `packaging/update-signing.pub`; the signed trusted comment must read
  `reclaim-update file:<asset> version:<version>`, so a signature cannot be
  moved to another file or reused to re-publish an older build. Unsigned or
  mismatched releases are not offered. The release workflow refuses to publish
  without the `UPDATE_SIGNING_KEY` secret.
- Downloaded installers live in private `pending-*` folders in the per-user
  cache. They are deleted after installing, on any failure, or (for installers
  still running when Reclaim exits) on the next launch. Cleanup never follows
  symbolic links and only removes `pending-*` folders.

## Verification

`cargo test --locked --all-targets --offline`: 64 passed, 2 host-dependent tests ignored.

`cargo clippy --locked --all-targets --offline -- -D warnings -D clippy::correctness -D clippy::suspicious -D clippy::undocumented_unsafe_blocks`: passed.

Regression tests cover incomplete/invalid export cleanup, existing symlink
protection, Unix permissions, hostile filenames, unexpected release paths,
and invalid update sizes including u64::MAX.

## Remaining boundaries

- Native Windows/macOS discovery still needs unsafe OS FFI. Those paths were
  inspected but were not built or exercised on this Linux host.
- Update signatures protect against a compromised release page or token, but
  not against theft of the signing key itself. Keep the private key offline and
  in the repository secret only; rotating it requires shipping a release signed
  with the old key that contains the new public key.
- Update signing is separate from OS code signing (Windows Authenticode, Apple
  Developer ID). Those certificates are not configured, so Windows SmartScreen
  and macOS Gatekeeper still treat a fresh download as unsigned.
- Use an output/config directory controlled by the user. Atomic publication
  does not protect against an attacker who can rename ancestor directories or
  modify files with the same account's privileges.
- Image validation is not malware scanning. Recovered documents, archives and
  executables remain untrusted content. Parser and decoder limits bound memory
  in common paths, but do not establish a hard CPU-time limit for hostile input.

## Production tooling follow-up

Tools installed and run locally:

- `cargo-audit 0.22.2`: zero known vulnerabilities in `Cargo.lock`. Three
  unmaintained-package advisories remain: RUSTSEC-2026-0370 (`zvariant`),
  RUSTSEC-2026-0371 (`zbus_names`), and RUSTSEC-2026-0372 (`zvariant_derive`).
- `cargo-deny 0.20.2`: dependency sources and bans pass. The strict advisory
  check originally failed on those three maintenance advisories. They now have
  reviewed exceptions restricted to the exact locked versions, expiring on
  2026-12-15. The project audit (`cargo test --test audit`) enforces those bounds
  alongside the dependency scan. Unknown vulnerability advisories remain errors.
- `cargo-fuzz 0.13.2` with libFuzzer: the final two-minute run completed
  **334 inputs in 121 seconds**, exit status 0, with AddressSanitizer and
  LeakSanitizer enabled. No crash, address-sanitizer, or leak finding was reported.
  It used a debug build with overflow checks/assertions, real-format/NTFS seeds,
  the format dictionary, 64 KiB input and 2 GiB RSS limits, and a 10-second
  per-input timeout. The fuzz lockfile matches production dependency versions
  and checksums; CI verifies that match.
- Miri `0.1.0 (7189d39d9c 2026-10-22)`: four targeted tests passed, covering
  failed-export cleanup, aligned source reads/seek bounds, and updater checksum,
  truncation, oversized-download rejection, and hostile/torn/truncated NTFS
  record rejection. The full security-test target
  cannot run under Miri because it does not emulate `renameat2` used by
  `tempfile::persist_noclobber`; this is an interpreter limitation, not a detected
  undefined-behavior finding. Publication tests pass natively.

The RustSec database used for cargo-audit was fetched on 2026-11-01 at commit
`2295466c4e9df26647fd67d8fa1c705ac2fc7f81` (1,395 advisories).

The maintenance advisories describe crates merged into zbus 6. Our direct zbus
5 dependency and the accessibility stack both depend on the old crates. The
latest accessibility adapter inspected (`accesskit_unix 0.24.1`) still depends
on zbus 5.19. Upgrading only our direct dependency would retain the affected
crates and add another D-Bus implementation version. Accessibility was retained;
a coordinated upstream migration remains necessary. These are maintenance
findings, not reported exploitable vulnerabilities.

`.github/workflows/security.yml` adds dependency checks, AddressSanitizer fuzzing,
and the supported Miri checks for pushes, pull requests, manual runs, and weekly
runs. Action references are pinned to verified immutable hashes, tool releases
and nightly compilers are pinned, and workflow permissions are read-only. The dependency job
accepts only the reviewed maintenance notices until the deadline;
all other advisory failures remain blocking. Both nightly jobs pin
`nightly-2026-10-23` (compiler commit `7189d39d9`), which passed the local
checks. The moving 2026-10-31 nightly produced E0391 in `zvariant_utils`
before either fuzzing or Miri could run. The replacement workflow must complete
on GitHub before its remote result can be confirmed.

`deny.toml` rejects unknown Git/registry sources, wildcard dependencies, yanked
versions, stale offline advisory databases, and unmaintained packages outside
the reviewed, version-bound, expiring exceptions. See
`fuzz/README.md` for the reproducible fuzzing commands and scope limits. Local
machine-readable audit/policy reports and test logs are under
`target/security-reports/` (ignored build artifacts).

These checks do not prove the absence of vulnerabilities, and the two-minute
fuzz budget is a smoke check, not a sustained fuzzing campaign.

The initial sandboxed fuzz run completed 408 inputs but its LeakSanitizer
shutdown failed because process inspection was blocked. An address-only rerun
completed 318 inputs in 125 seconds. A fixed-fixture replay and then the final
full sanitizer run succeeded outside that sandbox, without changing kernel
settings or disabling leak detection. No application fix or crash suppression
was needed for that tooling limitation.


## CI dependency review, 2026-11-11

The desktop build matrix passed its tests, but cargo-deny found two additional
maintenance notices in the locked Slint 1.18.1 dependency graph:

- [RUSTSEC-2026-0441](https://rustsec.org/advisories/RUSTSEC-2026-0441):
  `pin-utils 0.1.1`, required by `spin_on 0.1.1` in `slint-build` and
  `slint-macros`. Upstream must replace its pinning macros with maintained
  alternatives such as `core::pin::pin!`.
- [RUSTSEC-2026-0192](https://rustsec.org/advisories/RUSTSEC-2026-0192):
  `ttf-parser 0.25.1`, required by `owned_ttf_parser` / `ab_glyph` through
  winit's `sctk-adwaita` window decorations. Migration to a maintained font
  parser belongs in that dependency stack.

Both advisories are informational unmaintained notices with no patched version.
The exact locked versions are temporarily accepted until 2026-12-15, with
version and expiry enforcement in `tests/audit/supply_chain.rs`. Vulnerability,
yanked-crate, unknown-source, and all other advisory checks remain blocking.

Slint's winit backend also unconditionally depends on `webbrowser 1.2.7` for
its platform `open_url` implementation. The cargo-deny ban now permits only
`i-slint-backend-winit` as its immediate wrapper; other dependents remain
banned. Reclaim does not invoke this URL helper, and the project audit still
bans a direct `webbrowser` dependency. This exception does not authorize
application code to launch browser helpers.

## Hardening pass, 2026-11-12

### Changes

- More Clippy lints are now `deny` for the whole crate: lossy integer casts
  (`cast_possible_truncation`, `cast_possible_wrap`, `cast_sign_loss`),
  `unwrap_in_result`, `panic_in_result_fn`, `fallible_impl_from`,
  `get_unwrap`, `exit` and `let_underscore_must_use`. About 100 casts on sizes
  and offsets read from scanned data now go through `bytes::to_usize` or
  `try_from`. A compile-time check makes a build for a target narrower than 64
  bits fail instead of relying on the fallback.
- Every ignored `Result` is now explicit. A scan whose window has closed now
  cancels itself instead of continuing with nobody reading its results.
- These modules now deny unchecked indexing and arithmetic in shipped code
  (`clippy::indexing_slicing`, `clippy::arithmetic_side_effects`): the updater,
  gzip, xar and ZIP parsing, damaged-media reads, i18n, Linux drive discovery,
  the drive monitor, diagnostics, the installer launcher, native process
  launching, preview, update state and runtime, and UI text. Download loops
  also refuse a reader that reports more bytes than its buffer holds. The audit
  baseline drops by 35 unchecked indexes.
- Release builds keep integer overflow checks on (`overflow-checks = true`), so
  a wrap that slips past checked arithmetic aborts instead of producing a wrong
  length.
- Windows: Reclaim loads DLLs only from System32 and its own folder
  (`SetDefaultDllDirectories`), so a DLL planted in the current folder or on
  `PATH` is ignored. MSVC builds enable Control Flow Guard.
- CI: every checkout uses `persist-credentials: false`, and the release build
  no longer restores a build cache that pull requests could have written.

### Verification

`cargo fmt --all -- --check`, `cargo test --locked` (all suites, including the
audit and headless UI renders) and
`cargo clippy --locked --all-targets -- -D warnings` pass on Linux.

### Not verified

- Windows and macOS were not built on this Linux host: no Windows C
  cross-compiler for `ring`, no macOS SDK. Their cast fixes and the DLL
  search restriction were reviewed by hand. Run CI and smoke-test the Windows
  build (file dialogs, previews, drive list, update) before releasing.
- The larger parsers (`formats.rs`, `lib.rs`, `validation.rs`, `allocation/`,
  `ntfs.rs`, `detection.rs`, `source.rs`, `cfb.rs`, Windows and macOS drive
  discovery) still contain unchecked indexing and arithmetic, mostly after
  explicit bounds checks. The audit ratchet keeps their counts from growing.
