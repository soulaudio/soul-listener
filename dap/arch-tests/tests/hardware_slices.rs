//! The atopile design's build targets and the tooling that runs them must agree.
//!
//! These are cheap text assertions over committed files, not atopile builds —
//! the toolchain needs Python 3.14 and `cargo test` must run without it. What
//! they protect is the wiring: a module with no check build is a module the
//! pre-commit hook and CI silently skip, which is the exact failure the
//! per-module targets exist to prevent.

const ATO_YAML: &str = include_str!("../../../hardware/ato.yaml");
const LEFTHOOK: &str = include_str!("../../../lefthook.yml");
const HW_XTASK: &str = include_str!("../../../xtask/src/hardware.rs");
const HW_CI: &str = include_str!("../../../.github/workflows/hardware.yml");

/// Every `.ato` module with a top-level module must have a `check-*` build.
///
/// A slice nobody checks is worse than one that fails: the hook reports nothing,
/// CI reports nothing, and the design rots silently. `main.ato` and
/// `interfaces.ato` are exempt — the first IS the `default` build's entry point,
/// the second declares interfaces and has no module to build.
#[test]
fn every_ato_module_has_a_check_build() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../hardware/elec/src");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(dir).expect("hardware/elec/src must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("ato") {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .expect("utf-8 filename")
            .to_string();
        if name == "main.ato" || name == "interfaces.ato" {
            continue;
        }
        let stem = name.trim_end_matches(".ato");
        if !ATO_YAML.contains(&format!("check-{stem}:")) {
            missing.push(name.clone());
        }
        // And the xtask must map the file to that build, or the hook cannot
        // route a change to it.
        if !HW_XTASK.contains(&format!("(\"{name}\", \"check-{stem}\")")) {
            missing.push(format!("{name} (not in xtask MODULE_BUILDS)"));
        }
    }
    assert!(
        missing.is_empty(),
        "these atopile modules have no check build and/or no xtask mapping: {missing:?}\n\
         Add `check-<module>:` to hardware/ato.yaml with the module's entry point, and the \
         (file, build) pair to MODULE_BUILDS in xtask/src/hardware.rs. Without both, the \
         pre-commit hook and CI skip the slice without saying so."
    );
}

/// No two builds may share an entry point.
///
/// atopile 0.15.x rejects this outright — `Multiple build targets share the same
/// entry point` — and it is a config error that only surfaces when someone with
/// the toolchain installed tries to build, which may be days after the commit.
#[test]
fn no_two_builds_share_an_entry_point() {
    let mut seen: Vec<(&str, &str)> = Vec::new();
    let mut current = "";
    for line in ATO_YAML.lines() {
        let t = line.trim();
        if let Some(name) = t.strip_suffix(':') {
            if !line.starts_with("    ") && line.starts_with("  ") && !name.contains(' ') {
                current = name;
            }
        }
        if let Some(entry) = t.strip_prefix("entry: ") {
            seen.push((entry, current));
        }
    }
    for (i, (entry, build)) in seen.iter().enumerate() {
        for (other_entry, other_build) in &seen[i + 1..] {
            assert_ne!(
                entry, other_entry,
                "builds `{build}` and `{other_build}` share the entry point `{entry}`. \
                 atopile 0.15.x refuses to load a project like this. If the two differ only \
                 in which artefacts they emit, that is a `-t <target>` flag at invocation, \
                 not a second build."
            );
        }
    }
}

/// The deprecated target alias must not come back.
///
/// `__default__` was accepted by 0.12.x and is rejected by 0.15.x with
/// `Target `__default__` not recognized.` — a one-word diff that costs a build.
#[test]
fn no_deprecated_atopile_config_keys() {
    for (bad, instead) in [
        ("__default__", "default"),
        ("ato-version", "requires-atopile"),
        ("component_lib", "paths.parts"),
    ] {
        assert!(
            !ATO_YAML.contains(bad),
            "hardware/ato.yaml still uses `{bad}`, which atopile 0.15.x rejects. Use \
             `{instead}`."
        );
    }
    // The pin itself: 0.12.x and 0.15.7 resolve parts against
    // `components.atopileapi.com`, which no longer exists in DNS, so they can
    // never complete a build again regardless of how stable they look.
    assert!(
        ATO_YAML.contains("requires-atopile: \"^0.15.9\"")
            || ATO_YAML.contains("requires-atopile: \"^0.15.8\""),
        "hardware/ato.yaml must pin atopile 0.15.8 or newer. Earlier versions point at \
         `components.atopileapi.com`, which is gone — see \
         docs/hardware/ATOPILE-TOOLCHAIN.md."
    );
}

/// The pre-commit hook and CI must actually invoke the hardware check.
#[test]
fn the_hardware_check_is_wired_into_the_hook_and_ci() {
    assert!(
        LEFTHOOK.contains("hardware check-changed"),
        "lefthook.yml has no pre-commit command running `cargo xtask hardware check-changed`, \
         so editing a .ato file is not checked before it is committed."
    );
    assert!(
        LEFTHOOK.contains("hardware/**/*.{ato,yaml}"),
        "the lefthook hardware hook must glob the atopile sources, or it never fires."
    );
    assert!(
        HW_CI.contains("xtask hardware check") || HW_CI.contains("ato build"),
        "the Hardware workflow must run the design check."
    );
}
