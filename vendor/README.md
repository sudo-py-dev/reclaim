# Slint core patch

`slint-core/` contains the published [i-slint-core 1.18.1 crate](https://crates.io/crates/i-slint-core/1.18.1).
The upstream archive SHA-256 is `ed2ea15059d70bc31b1a8b1eb4c34f2c5df04cb7e53f36179b43c0261231d646`.
The upstream commit is `372cf0ee5577c3dfec309a45e7b778ba4e81b734`.
License files and notices are preserved.

The changes to the published crate are in `Cargo.toml` and `date_time.rs`.
Slint still requires chrono, which is covered by
[RUSTSEC-2026-0543](https://rustsec.org/advisories/RUSTSEC-2026-0543.html).
The patch replaces chrono with Jiff for date parsing, formatting, calendar
arithmetic, and discovery of the system's local date.

Once Slint publishes a release that has removed chrono, upgrade the pinned
Slint crates together and remove this directory and both `[patch.crates-io]`
entries. Refresh the app and fuzz lockfiles and run the supply-chain checks.
