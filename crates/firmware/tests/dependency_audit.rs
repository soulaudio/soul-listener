//! Dependency audit tests.
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
