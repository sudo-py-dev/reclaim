//! Reclaim's project audit. Runs with the rest of the suite (`cargo test`), or on
//! its own with `cargo test --test audit`.
//!
//! The audit reads the source as a syntax tree, not as text, and checks rules that
//! the compiler and Clippy cannot express: where `unsafe` and process launching may
//! live, unbounded reads and allocations on untrusted input, leftover debug code,
//! translation catalogs against the code that uses them, and supply-chain policy.
//! Every rule names the CWE weakness it guards against.
//!
//! A finding can be accepted only next to the code it concerns, with a reason:
//!
//! ```text
//! // audit-allow(DOS-ALLOC): the length is a u16 field, at most 64 KiB.
//! ```
//!
//! Suppressions that no longer match a finding are reported too, so they cannot
//! outlive the code they excused.

#![expect(clippy::string_slice, reason = "the rules slice ASCII-delimited text")]
#![expect(
    clippy::case_sensitive_file_extension_comparisons,
    reason = "repository file names, whose case is fixed by the project"
)]

// Test-only code, so Clippy treats the harness helpers as tests on every
// toolchain: they report findings by panicking.
#[cfg(test)]
mod rules;
#[cfg(test)]
mod supply_chain;
#[cfg(test)]
mod translations;
#[cfg(test)]
mod workspace;

use workspace::{Workspace, report};

#[cfg(test)]
fn workspace() -> &'static Workspace {
    use std::sync::OnceLock;
    static WORKSPACE: OnceLock<Workspace> = OnceLock::new();
    WORKSPACE.get_or_init(Workspace::load)
}

#[test]
fn unsafe_code_is_confined_and_documented() {
    report(&rules::unsafe_code(workspace()));
}

#[test]
fn no_memory_or_concurrency_hazards() {
    report(&rules::memory_hazards(workspace()));
}

#[test]
fn processes_are_only_started_by_the_native_launcher() {
    report(&rules::process_launching(workspace()));
}

#[test]
fn network_and_crypto_use_secure_defaults() {
    report(&rules::transport_and_crypto(workspace()));
}

#[test]
fn no_secrets_in_the_repository() {
    report(&rules::secrets(workspace()));
}

#[test]
fn fixtures_carry_no_local_user_paths() {
    report(&rules::fixture_privacy(workspace()));
}

#[test]
fn temporary_files_are_unpredictable() {
    report(&rules::temporary_files(workspace()));
}

#[test]
fn untrusted_input_cannot_exhaust_memory() {
    report(&rules::resource_limits(workspace()));
}

#[test]
fn unchecked_indexing_does_not_grow() {
    report(&rules::index_ratchet(workspace()));
}

#[test]
fn no_debug_leftovers_or_dead_code_markers() {
    report(&rules::leftovers(workspace()));
}

#[test]
fn lint_policy_stays_strict() {
    report(&rules::lint_policy(workspace()));
}

#[test]
fn translation_catalogs_are_consistent() {
    report(&translations::catalogs(workspace()));
}

#[test]
fn every_interface_message_is_translated() {
    report(&translations::missing(workspace()));
}

#[test]
fn no_dead_translation_keys() {
    report(&translations::dead(workspace()));
}

#[test]
fn interface_strings_match_their_setters() {
    report(&translations::slint_strings(workspace()));
}

#[test]
fn dependencies_are_pinned_and_locked() {
    report(&supply_chain::manifest(workspace()));
}

#[test]
fn advisory_exceptions_are_reviewed_and_current() {
    report(&supply_chain::advisories(workspace()));
}

#[test]
fn fuzz_harness_uses_production_dependencies() {
    report(&supply_chain::fuzz_lock(workspace()));
}

/// Runs last by name: every rule above has recorded which suppressions it used.
#[test]
fn zz_every_suppression_is_used() {
    let ws = workspace();
    // Run each rule so the used-suppression set is complete regardless of test order.
    drop((
        rules::unsafe_code(ws),
        rules::memory_hazards(ws),
        rules::process_launching(ws),
        rules::transport_and_crypto(ws),
        rules::secrets(ws),
        rules::temporary_files(ws),
        rules::resource_limits(ws),
        rules::index_ratchet(ws),
        rules::leftovers(ws),
        translations::missing(ws),
    ));
    report(&ws.unused_suppressions());
}
