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
  2026-12-15. `scripts/check_advisory_policy.py` enforces those bounds before the
  dependency scan. Unknown vulnerability advisories remain errors.
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
accepts only the three reviewed maintenance notices until the deadline;
all other advisory failures remain blocking. Both nightly jobs pin
`nightly-2026-10-23` (compiler commit `7189d39d9`), which passed the local
checks. The moving 2026-10-31 nightly produced E0391 in `zvariant_utils`
before either fuzzing or Miri could run. The replacement workflow must complete
on GitHub before its remote result can be confirmed.

`deny.toml` rejects unknown Git/registry sources, wildcard dependencies, yanked
versions, stale offline advisory databases, and unmaintained packages outside
the three reviewed, version-bound, expiring exceptions. See
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
