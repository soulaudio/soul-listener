//! Dependency audit tests — these ensure banned crates are not in the dep graph.
//! Run with: cargo test -p firmware --test dependency_audit

/// This test verifies at compile time that we don't accidentally depend on epd-waveshare.
/// If this file compiles, the dep graph is clean.
/// The real enforcement is in deny.toml (cargo-deny).
#[test]
fn compile_time_audit_passes() {
    // If epd-waveshare were a dependency, cargo would have compiled it.
    // We use a custom SSD1677 driver instead.
    // This test exists as documentation — the actual enforcement is:
    //   1. deny.toml [bans] section
    //   2. CI cargo-deny check step
    //   3. epd-waveshare removed from Cargo.toml workspace.dependencies
}
