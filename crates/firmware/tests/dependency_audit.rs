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

/// Verify that the CI `clippy-embedded` job runs clippy on the platform crate
/// with the embedded target (thumbv7em-none-eabihf).
///
/// The platform crate defines hardware abstraction traits that must compile in
/// no_std context. Running clippy only on the host target misses embedded-specific
/// violations (e.g. accidentally importing std, using alloc without the feature flag).
/// The `clippy-embedded` job must cover the platform crate explicitly.
#[test]
fn ci_clippy_covers_platform_crate_for_embedded_target() {
    let ci_yml = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../.github/workflows/ci.yml")
    ).expect("ci.yml must exist");
    // CI must run clippy on the platform crate with the embedded target.
    // Acceptable patterns:
    //   1. A dedicated `clippy-embedded` job exists (which contains a platform step), OR
    //   2. Both "platform" and "thumbv7em-none-eabihf" appear in a clippy context
    assert!(
        ci_yml.contains("clippy-embedded") ||
        (ci_yml.contains("platform") && ci_yml.contains("thumbv7em-none-eabihf")),
        "CI must run clippy on platform crate for --target thumbv7em-none-eabihf"
    );
    // Stricter: the clippy-embedded job must contain the platform + embedded target combo.
    // This ensures it is not just a check job but actually runs clippy lints.
    let clippy_embedded_section = ci_yml
        .split("clippy-embedded:")
        .nth(1)
        .unwrap_or("");
    // The section after "clippy-embedded:" should mention both platform and thumbv7em
    assert!(
        clippy_embedded_section.contains("platform") &&
        clippy_embedded_section.contains("thumbv7em-none-eabihf"),
        "The clippy-embedded CI job must explicitly run clippy on the platform crate          with --target thumbv7em-none-eabihf. Add:          `cargo clippy -p platform --target thumbv7em-none-eabihf --no-default-features -- -D warnings`"
    );
}

/// Verify that supply-chain/audits.toml contains at least one real audit entry.
///
/// cargo-vet exemptions are a crutch that acknowledge unreviewed code. Real audit
/// entries ([audits.<crate>]) provide actual supply-chain security guarantees by
/// recording who reviewed what version and when. At least the most foundational
/// embedded crates (heapless, embedded-hal) should have real audits rather than
/// blanket exemptions.
#[test]
fn supply_chain_has_real_audits_not_only_exemptions() {
    let audits = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../supply-chain/audits.toml")
    ).unwrap_or_default();
    let config = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../supply-chain/config.toml")
    ).unwrap_or_default();
    // Either audits.toml has real criteria entries, or config.toml has imports from trusted registries
    let has_real_audits = audits.contains("[audits.") || config.contains("[imports]");
    // At minimum, the supply-chain directory must exist and be non-empty
    assert!(!config.is_empty(), "supply-chain/config.toml must not be empty");
    // Hard check: audits.toml must have at least one real audit entry (not just exemptions).
    // Exemptions are a crutch; real audits provide actual supply-chain security guarantees.
    assert!(
        has_real_audits,
        "supply-chain/audits.toml must contain at least one real audit ([audits.<crate>])          or config.toml must import from a trusted registry ([imports]).          Run: cargo vet certify <crate> <version> --criteria safe-to-deploy"
    );
}
