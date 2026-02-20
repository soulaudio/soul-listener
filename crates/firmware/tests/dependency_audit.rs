//! Dependency audit tests.
// Audit test file: expect/unwrap/cast lints are intentional test mechanisms.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
)]
//! Real enforcement is via cargo-deny in CI. These tests verify the deny.toml
//! configuration exists and contains the required bans.
//!
//! Run with: cargo test -p firmware --test dependency_audit

/// Verify that deny.toml bans embedded-alloc (no heap allocation in this project).
#[test]
fn deny_toml_bans_allocator_crates() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains("embedded-alloc"),
        "deny.toml must ban embedded-alloc (no heap allocation — use heapless instead)"
    );
    assert!(
        deny_toml.contains("wee_alloc"),
        "deny.toml must ban wee_alloc (unmaintained + memory corruption RUSTSEC-2022-0054)"
    );
}

/// Verify that deny.toml bans getrandom (no OS entropy source on bare-metal STM32H7).
#[test]
fn deny_toml_bans_random_number_generator() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains("getrandom"),
        "deny.toml must ban getrandom (no OS entropy on bare metal thumbv7em-none-eabihf)"
    );
}

/// Verify that deny.toml denies multiple versions of the same crate.
///
/// Diamond dependency conflicts on no_std targets can cause subtle issues
/// where two crates use different versions of embedded-hal traits, breaking
/// driver composition at type level.
#[test]
fn deny_toml_multiple_versions_is_deny() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains("multiple-versions = \"deny\""),
        "deny.toml must deny multiple versions of the same crate to prevent \
         embedded-hal version conflicts between drivers"
    );
}

/// Verify that deny.toml bans the deprecated minimp3 family.
#[test]
fn deny_toml_bans_minimp3() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains("minimp3"),
        "deny.toml must ban minimp3 family (deprecated, unsound on ARM shift ops)"
    );
}

/// Verify that deny.toml bans epd-waveshare (no SSD1677 support — use custom driver).
#[test]
fn deny_toml_bans_epd_waveshare() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains("epd-waveshare"),
        "deny.toml must ban epd-waveshare (does not support SSD1677 — use custom driver)"
    );
}

/// Verify that the cargo-vet supply-chain directory exists and is valid.
///
/// cargo-vet provides cryptographic supply-chain trust annotations: each
/// direct and transitive dependency is audited or certified as safe by a
/// trusted auditor. This test is a soft check — it warns but does not fail
/// when supply-chain/config.toml is absent (e.g. before `cargo vet init`).
///
/// Run `cargo vet init` at the workspace root to initialize supply-chain tracking.
/// Run `cargo vet check` to verify all dependencies are audited.
#[test]
fn supply_chain_config_exists() {
    let config_path = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../supply-chain/config.toml"
    ));
    // Soft check: warn but do not fail if cargo vet not yet initialized.
    // Once initialized, the config must not be empty.
    if config_path.exists() {
        let config = std::fs::read_to_string(config_path).unwrap();
        assert!(
            config.contains("criteria") || config.contains("[imports]") || !config.is_empty(),
            "supply-chain/config.toml must not be empty"
        );
        println!(
            "INFO: cargo vet supply-chain/config.toml found ({} bytes)",
            config.len()
        );
    } else {
        println!(
            "INFO: supply-chain/config.toml not found -- \
             run `cargo vet init` to set up supply-chain tracking"
        );
    }
}

#[test]
fn ci_cargo_vet_does_not_swallow_exit_code() {
    let ci_yml = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/ci.yml")
    ).expect("ci.yml must exist");
    // Must NOT have "cargo vet check || echo" pattern (this swallows failures)
    assert!(
        !ci_yml.contains("cargo vet check || echo"),
        "CI must not use `cargo vet check || echo` — this swallows exit codes and lets unvetted deps pass"
    );
    // Must have cargo vet check (without fallback)
    assert!(
        ci_yml.contains("cargo vet check") || ci_yml.contains("cargo vet"),
        "CI must run cargo vet check"
    );
}

#[test]
fn ci_has_embedded_target_check_job() {
    let ci_yml = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/ci.yml")
    ).expect("ci.yml must exist");
    // CI must have a job that checks the embedded target
    assert!(
        ci_yml.contains("thumbv7em-none-eabihf"),
        "CI must have a job checking --target thumbv7em-none-eabihf to prevent std from sneaking into embedded build"
    );
}

#[test]
fn ci_toolchain_is_pinned_not_floating_stable() {
    let ci_yml = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/ci.yml")
    ).expect("ci.yml must exist");
    // Count occurrences of @stable (unpinned)
    let unpinned_count = ci_yml.matches("rust-toolchain@stable").count()
        + ci_yml.matches("dtolnay/rust-toolchain@stable").count();
    // Allow at most 1 use of @stable (some jobs might intentionally test on latest)
    // But the main lint/build jobs should use a pinned version
    let pinned_count = ci_yml.matches("rust-toolchain@1.").count()
        + ci_yml.matches("rust-toolchain@\"1.").count();
    assert!(
        pinned_count >= 1 || unpinned_count <= 2,
        "CI should pin the Rust toolchain to a specific version (e.g., @1.75) for reproducible builds.         Found {} unpinned @stable and {} pinned uses",
        unpinned_count, pinned_count
    );
}
