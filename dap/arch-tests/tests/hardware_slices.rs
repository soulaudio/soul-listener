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

const PLACEMENT: &str = include_str!("../../../hardware/elec/placement.yaml");

/// The floorplan must declare a zone for every atopile module, with a reason.
///
/// A module with no zone is a module whose parts can land anywhere on the board
/// without `tools/ato-placement.py` noticing — which makes the floorplan look
/// enforced while leaving a hole in it. The reasons are required because a
/// floorplan without them can only be obeyed, not reviewed: the whole point is
/// that `memory` sits beside `mcu` *because* the FMC bus has to stay under
/// 10 mm, and that fact should not live only in a layout engineer's memory.
#[test]
fn every_module_has_a_placement_zone() {
    let dir = concat!(env!("CARGO_MANIFEST_DIR"), "/../../hardware/elec/src");
    let mut missing = Vec::new();
    for entry in std::fs::read_dir(dir).expect("hardware/elec/src must exist") {
        let path = entry.expect("readable dir entry").path();
        if path.extension().and_then(|e| e.to_str()) != Some("ato") {
            continue;
        }
        let name = path.file_stem().and_then(|n| n.to_str()).unwrap_or("");
        if name == "main" || name == "interfaces" {
            continue;
        }
        if !PLACEMENT.contains(&format!("\n  {name}:")) {
            missing.push(name.to_string());
        }
    }
    assert!(
        missing.is_empty(),
        "hardware/elec/placement.yaml declares no zone for: {missing:?}\n\
         Add one with an `x`, `y`, `width`, `height` and a `why`. Without it the \
         module's parts are unconstrained and the floorplan check cannot tell."
    );
}

/// Placement and part identity must be locked against silent rebuild drift.
///
/// atopile has no netlist hand-off — it MODIFIES the `.kicad_pcb` in place on
/// every build. So designators, net names and the picked parts themselves are
/// all things a rebuild can change, and a changed part is invisible in a diff of
/// the `.ato` sources. `keep_picked_parts` is the one that costs money: without
/// it a rebuild can swap one maker's 100 nF 0402 for another's, after the reel
/// is bought.
#[test]
fn the_pcb_is_locked_against_rebuild_drift() {
    for (key, why) in [
        (
            "keep_designators: true",
            "a renumbered board invalidates every assembly note",
        ),
        (
            "keep_net_names: true",
            "renamed nets break any netlist-based review",
        ),
        (
            "keep_picked_parts: true",
            "a rebuild can otherwise re-query the parts service and silently \
             substitute a different manufacturer's equivalent",
        ),
    ] {
        assert!(
            ATO_YAML.contains(key),
            "hardware/ato.yaml's `default` build must set `{key}` — {why}."
        );
    }
}

const MAIN_ATO: &str = include_str!("../../../hardware/elec/src/main.ato");

/// The declared board outline and the floorplan must agree.
///
/// Two files state the board's size: `main.ato` declares a
/// `RectangularBoardShape`, which is what atopile actually draws on `Edge.Cuts`,
/// and `placement.yaml` declares the envelope every zone is checked against. If
/// they drift, the floorplan is validated against a board that is not the board.
///
/// They cannot be merged into one. atopile will only take the outline from the
/// design, and the zone geometry has nowhere to live in `.ato` — the language has
/// no placement concept at all. So there are two numbers and this is the check
/// that keeps them equal.
#[test]
fn board_outline_matches_the_floorplan() {
    fn mm(src: &str, pat: &str) -> Option<f64> {
        let i = src.find(pat)? + pat.len();
        let rest = &src[i..];
        let end = rest.find("mm")?;
        rest[..end].trim().parse().ok()
    }
    let ato_x = mm(MAIN_ATO, "board.x =").expect("main.ato must declare board.x");
    let ato_y = mm(MAIN_ATO, "board.y =").expect("main.ato must declare board.y");

    fn yaml_num(src: &str, key: &str) -> Option<f64> {
        let i = src.find(key)? + key.len();
        let line = src[i..].lines().next()?;
        line.trim().parse().ok()
    }
    let spec_x = yaml_num(PLACEMENT, "width_mm:").expect("placement.yaml board.width_mm");
    let spec_y = yaml_num(PLACEMENT, "height_mm:").expect("placement.yaml board.height_mm");

    assert_eq!(
        (ato_x, ato_y),
        (spec_x, spec_y),
        "main.ato declares a {ato_x} x {ato_y} mm board but placement.yaml says \
         {spec_x} x {spec_y} mm. atopile draws Edge.Cuts from the .ato declaration, so the \
         .ato number is the real board — update placement.yaml, or change both together. \
         Every zone is checked against the placement.yaml envelope, so a mismatch validates \
         the floorplan against a board that does not exist."
    );
}

/// The outline must be declared in the design, not drawn into the PCB.
///
/// Both are possible and they are mutually destructive: when a
/// `RectangularBoardShape` is present, atopile's `_clear_existing_outline`
/// deletes every `Edge.Cuts` primitive on every build. A hand-drawn outline
/// therefore survives exactly until the next build.
///
/// An earlier version of `tools/ato-outline.py` wrote the outline itself, which
/// would have been silently erased. This pins the resolution: the design owns
/// `Edge.Cuts`, the tool annotates the user layers only.
#[test]
fn the_outline_is_owned_by_the_design_not_the_tool() {
    assert!(
        MAIN_ATO.contains("new RectangularBoardShape"),
        "main.ato must declare the board outline with RectangularBoardShape. Without it \
         atopile leaves Edge.Cuts alone and the board has no outline at all, which kicad-cli \
         reports as 'Board outline is malformed'."
    );
    let outline_tool = include_str!("../../../tools/ato-outline.py");
    assert!(
        !outline_tool.contains("\"Edge.Cuts\""),
        "tools/ato-outline.py writes to Edge.Cuts, which atopile deletes on every build when \
         a RectangularBoardShape is declared. The tool must annotate Dwgs.User/Cmts.User only."
    );
}
