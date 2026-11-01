# Scanner fuzzing

Install the real Rust fuzzing tool and nightly compiler:

```sh
cargo install --locked --version 0.13.2 cargo-fuzz
rustup toolchain install nightly --profile minimal --component rust-src
python3 fuzz/check_lock.py
python3 fuzz/seed_corpus.py
cargo +nightly fuzz run scan --dev --target x86_64-unknown-linux-gnu -- -max_total_time=120 -max_len=65536 -timeout=10 -rss_limit_mb=2048 -dict=fuzz/scan.dict
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

The seed script copies the existing real image/archive fixtures and creates a
valid NTFS resident record and PDF. Corpus mutations, build artifacts, coverage,
and crashes are ignored by Git. Keep reproducible crashes as regression tests
before fixing them. CI uploads crash artifacts on failure.

CI checks that the fuzz lockfile uses the production dependency versions and
checksums. Refresh both lockfiles together when updating production dependencies.

## Container probes

The lightweight probe target compiles the exact `src/detection.rs` production
module without the desktop dependency stack. It fuzzes SQLite headers/page
bounds and incremental Ogg framing using the shared production limit and result
type. It does not replace the full scanner target or test file export.

```sh
python3 fuzz/check_lock.py
python3 fuzz/seed_corpus.py
cargo +nightly fuzz run containers --fuzz-dir fuzz/containers --dev --target x86_64-unknown-linux-gnu -- -max_total_time=120 -max_len=65536 -timeout=10 -rss_limit_mb=1024 -dict=fuzz/scan.dict
```

Both targets run in CI. Seeds larger than 64 KiB are excluded; native regression
tests separately cover the SQLite 64 KiB page-size encoding with a larger file.
