# Scanner fuzzing

Install the real Rust fuzzing tool and nightly compiler:

```sh
cargo install --locked --version 0.13.2 cargo-fuzz
rustup toolchain install nightly --profile minimal --component rust-src
cargo test --locked --test audit fuzz_harness
cargo xtask seed-corpus
cargo +nightly fuzz run scan --dev --target x86_64-unknown-linux-gnu -- -max_total_time=120 -max_len=65536 -timeout=10 -rss_limit_mb=2048 -dict=fuzz/scan.dict
cargo +nightly fuzz run source --dev --target x86_64-unknown-linux-gnu -- -max_total_time=120 -max_len=65536 -timeout=10 -rss_limit_mb=1024
cargo +nightly fuzz run allocation --dev --target x86_64-unknown-linux-gnu -- -max_total_time=120 -max_len=65536 -timeout=10 -rss_limit_mb=2048 -dict=fuzz/scan.dict
```

AddressSanitizer is enabled by default. Debug assertions and overflow checks are
also enabled. Use longer runs for deeper exploration; two minutes is a smoke
check. The explicit target avoids inheriting the platform on which a prebuilt
cargo-fuzz binary was compiled.

The target exercises signature parsing, NTFS resident records, image decoding,
aligned reads, and recovery filename generation. It checks that carved ranges
fit their input and names contain a single path component. Inputs are limited
to 64 KiB and exports are not written to disk. This does not exercise the GUI,
network updater, OS drive FFI, or megabyte-sized streaming boundaries.

`source` compares randomized seek/read sequences against a standard in-memory
cursor, with 1/512/4096/65536-byte alignments, negative seeks and offsets near
integer limits. `allocation` fuzzes the production unused-space API with FAT,
NTFS and exFAT inputs; malformed maps may return errors but must not panic,
hang, or produce out-of-bounds candidates. Its corpus includes a complete FAT12
volume with a recoverable file in a free cluster. These targets use synthetic
memory inputs and never open a physical drive or write recovery output.

`tests/security.rs` also uses a virtual disk to verify exact export bytes at
offsets above 4 GiB, above 8 EiB, and near `u64::MAX`. Those tests exercise high
offsets without allocating a giant image or writing to user devices.

If LeakSanitizer explicitly reports that the host sandbox prevents its ptrace
check, prefix the command with `ASAN_OPTIONS=detect_leaks=0`. This keeps
AddressSanitizer enabled but omits leak detection; record that limitation with
the results. Do not change host ptrace or other system security settings.

The Rust seed command copies the existing real image/archive fixtures and creates a
valid NTFS resident record and PDF. Corpus mutations, build artifacts, coverage,
and crashes are ignored by Git. Keep reproducible crashes as regression tests
before fixing them. Keep those artifacts locally; the routine CI workflow runs regression tests rather than a fuzzing campaign.

CI checks that the fuzz lockfile uses the production dependency versions and
checksums. Refresh both lockfiles together when updating production dependencies.

## Container probes

The lightweight probe target compiles the exact `src/detection.rs` production
module without the desktop dependency stack. It fuzzes SQLite headers/page
bounds and incremental Ogg framing using the shared production limit and result
type. It does not replace the full scanner target or test file export.

```sh
cargo test --locked --test audit fuzz_harness
cargo xtask seed-corpus
cargo +nightly fuzz run containers --fuzz-dir fuzz/containers --dev --target x86_64-unknown-linux-gnu -- -max_total_time=120 -max_len=65536 -timeout=10 -rss_limit_mb=1024 -dict=fuzz/scan.dict
```

Run both targets locally when changing their parsers. Seeds larger than 64 KiB are excluded; native regression
tests separately cover the SQLite 64 KiB page-size encoding with a larger file.
