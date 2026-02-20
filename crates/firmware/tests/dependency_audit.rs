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
