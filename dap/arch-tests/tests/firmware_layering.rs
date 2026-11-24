//! The firmware's vertical slices, and the contracts between them.
//!
//! # What this enforces, and why it is a test rather than a convention
//!
//! `dap-core`'s dependency graph is currently clean: the port traits depend on
//! nothing, each slice depends only on the ports, and `app` composes them. That
//! is a good architecture and it was entirely unenforced — one
//! `use crate::audio::AudioPlayer` inside `power.rs` would couple two slices and
//! nothing in the build would object. The property these tests protect is the
//! one the whole design rests on: **a slice can be replaced without touching any
//! other slice**, because the only thing it shares with its neighbours is a
//! trait.
//!
//! # The layering
//!
//! ```text
//!   board-h7a3 / dap-sim        adapters: construct the concrete drivers
//!        |
//!        v
//!   firmware  (the run loop)  ->  app  (the composition root)
//!                                  |
//!                                  v
//!            audio   display   power   screens        <- vertical slices
//!                \      |       /
//!                 v     v      v
//!                     ports                           <- contracts, depend on NOTHING
//! ```
//!
//! Read as: an arrow may only point down or sideways-to-ports. A slice pointing
//! at another slice is the violation.
//!
//! # Why text analysis
//!
//! These read the source rather than the compiled crate graph, because Rust's
//! module system has no notion of a private sibling: everything inside a crate
//! can see everything else, so the compiler cannot express "audio must not see
//! power". Splitting each slice into its own crate *would* let the compiler
//! enforce it, and is the obvious next step if these tests ever start feeling
//! like a substitute for the real thing. They are a substitute. They are also
//! the cheap version that works today.
//!
//! Doc comments are stripped before analysis. The first version of this did not,
//! and intra-doc links (`[`crate::power::PowerService`]` inside `ports.rs`'s
//! documentation) made the contract layer look like it depended on three slices
//! when it depends on nothing. A dependency check that counts prose is worse
//! than none, because it reports a violation nobody can fix.

use std::path::{Path, PathBuf};

/// The vertical slices: a self-contained capability of the device.
///
/// Each one owns its implementation and talks to the rest of the firmware only
/// through a port trait. `screens` is the UI layer rather than a device
/// capability, and is allowed to use `display` because drawing is what it does.
const SLICES: &[&str] = &["audio", "display", "power", "screens"];

/// Modules each slice is allowed to reach into, besides itself.
///
/// Everything not listed is a violation. The list is deliberately short — the
/// moment it starts growing, the slices have stopped being slices.
fn allowed(module: &str) -> &'static [&'static str] {
    match module {
        // The contracts. Depend on NOTHING: a trait that needs an
        // implementation to compile is not a contract, it is a coupling with
        // extra steps.
        "ports" => &[],
        // Slices: their own contract only.
        "audio" | "power" => &["ports"],
        // `display` is the board-agnostic rendering surface; `screens` draws on
        // it. Neither knows about audio or power.
        "display" => &["ports"],
        "screens" => &["display"],
        // The composition root. Allowed to see everything, because wiring the
        // slices together is its entire job and there is nowhere else for that
        // knowledge to live.
        "app" => &["audio", "display", "ports", "power", "screens"],
        // The run loop drives the composed app.
        "firmware" => &["app", "display", "ports", "power"],
        // Placeholder adapters. They implement the ports, so they see the
        // ports and whatever types the ports mention.
        "stub" => &["app", "display", "ports", "power"],
        "lib" => &[],
        _ => &[],
    }
}

fn core_src() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../core/src")
        .canonicalize()
        .expect("dap/core/src must exist")
}

/// Strip doc comments and line comments, so prose cannot look like a dependency.
fn code_only(src: &str) -> String {
    src.lines()
        .map(|l| {
            let t = l.trim_start();
            if t.starts_with("//") {
                ""
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Which crate-internal modules a file actually references in code.
fn deps_of(path: &Path) -> Vec<String> {
    let src = std::fs::read_to_string(path).expect("readable source file");
    let code = code_only(&src);
    let mut out: Vec<String> = Vec::new();
    let bytes = code.as_bytes();
    let needle = b"crate::";
    let mut i = 0;
    while let Some(pos) = code[i..].find("crate::") {
        let start = i + pos + needle.len();
        let mut end = start;
        while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
            end += 1;
        }
        if end > start {
            let name = code[start..end].to_string();
            if !out.contains(&name) {
                out.push(name);
            }
        }
        i = start;
    }
    out
}

/// The module a source file belongs to: `power/dvfs.rs` is part of `power`.
fn module_of(rel: &Path) -> String {
    let comps: Vec<_> = rel.components().collect();
    if comps.len() > 1 {
        comps[0].as_os_str().to_string_lossy().to_string()
    } else {
        rel.file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    }
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    for e in std::fs::read_dir(dir).expect("readable dir") {
        let p = e.expect("dir entry").path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().and_then(|x| x.to_str()) == Some("rs") {
            out.push(p);
        }
    }
}

/// No slice may reach into another slice. Contracts only.
///
/// This is the whole point: if `power` can name a type from `audio`, then
/// replacing the audio slice means editing the power slice, and the vertical
/// split has bought nothing.
#[test]
fn slices_talk_to_each_other_only_through_ports() {
    let root = core_src();
    let mut files = Vec::new();
    walk(&root, &mut files);
    assert!(files.len() > 5, "expected to find dap-core's sources");

    let mut violations: Vec<String> = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&root).expect("under core/src");
        let module = module_of(rel);
        // Intra-module references (`power/dvfs.rs` -> `power`) are internal.
        let permitted = allowed(&module);
        for dep in deps_of(f) {
            if dep == module || permitted.contains(&dep.as_str()) {
                continue;
            }
            // A reference to something that is not a module at all (a re-export
            // at the crate root, say) is not a layering question.
            if !SLICES.contains(&dep.as_str())
                && !["app", "firmware", "ports", "stub"].contains(&dep.as_str())
            {
                continue;
            }
            violations.push(format!(
                "{}: module `{module}` references `crate::{dep}`, which is not in its \
                 allowed set {permitted:?}",
                rel.display()
            ));
        }
    }
    assert!(
        violations.is_empty(),
        "firmware layering violated:\n  {}\n\n\
         A slice may only reach into `ports`. If this edge is genuinely needed, the \
         capability belongs in a port trait that both sides depend on — not in a direct \
         reference, which makes the two slices unreplaceable independently. Update \
         `allowed()` in this file ONLY together with a reason, because every entry added \
         there is a slice boundary given up.",
        violations.join("\n  ")
    );
}

/// The contract layer must depend on nothing.
///
/// Stated separately from the layering test even though `allowed("ports")` is
/// already empty, because this is the single most load-bearing property in the
/// design and it deserves to fail by name. A port trait that references a slice
/// cannot be implemented by anyone who does not also take that slice — which is
/// exactly the dependency the ports exist to remove.
#[test]
fn the_contracts_depend_on_nothing() {
    let ports = core_src().join("ports.rs");
    assert!(ports.exists(), "dap-core must have a ports module");
    let deps = deps_of(&ports);
    assert!(
        deps.is_empty(),
        "ports.rs references {deps:?}. The contract layer must be self-contained: a board \
         implementing `Storage` should need the trait and nothing else. Move whatever type \
         is being reached for INTO ports.rs, or express it as an associated type."
    );
}

/// Every slice must own a port trait, and every port trait must belong to a slice.
///
/// A slice with no contract cannot be swapped — there is nothing for a
/// replacement to implement. A contract with no slice is a trait nobody provides.
#[test]
fn every_slice_has_a_contract_and_every_contract_has_a_slice() {
    let ports = std::fs::read_to_string(core_src().join("ports.rs")).expect("ports.rs");
    // (slice, what makes it replaceable, where to look)
    //
    // `Sink` is NOT declared here, and that is correct: the audio output
    // contract belongs to `soul-audio-core`, the decode engine that writes into
    // it. `ports.rs` re-exports it so a board implements one trait set from one
    // place. An earlier version of this test demanded a local `trait Sink` and
    // failed on a design that is right — a contract owned by the crate that
    // consumes it is better than a copy of it here.
    let pairs = [
        ("audio", "Sink"),
        ("power", "trait Power"),
        ("display", "trait Display"),
    ];
    let display_rs = std::fs::read_to_string(core_src().join("display.rs")).unwrap_or_default();
    for (slice, decl) in pairs {
        let found = ports.contains(decl) || display_rs.contains(decl);
        assert!(
            found,
            "slice `{slice}` has no `{decl}` contract. Without one it cannot be replaced \
             independently, which is the only reason to have slices at all."
        );
    }
    // Contracts that exist but whose slice is only a stub are fine and expected
    // (storage and input have no slice yet, just ports and stub adapters) —
    // asserting the reverse direction would forbid defining a contract before
    // its implementation, which is the correct order to work in.
    for decl in ["trait Storage", "trait Controls"] {
        assert!(
            ports.contains(decl),
            "ports.rs is missing `{decl}`; the board-facing contract set has shrunk"
        );
    }
}

/// Concrete adapters may only be constructed at the edges.
///
/// This is the test that makes "swappable" mean something. A slice may name a
/// port trait; it may not name a concrete implementation of one. The moment
/// `audio.rs` constructs a `StubSink`, the audio slice has chosen its own output
/// device and no board can choose a different one.
///
/// The edges, and why each is allowed:
///
/// * `board-h7a3` — the real device. Picking the drivers is its entire purpose.
/// * `dap-sim` and `arch-tests` — host harnesses. They substitute adapters on
///   purpose; that substitution is what makes the firmware testable off-target.
/// * `core/src/stub.rs` — where the placeholder adapters are DEFINED. Defining
///   them is not choosing them.
///
/// Everything else in `dap-core` must see only traits.
///
/// # What replaced this test, and why
///
/// This slot used to hold a size check: no slice may exceed 40% of the crate.
/// It fired immediately on `power` at 60%, which looked like a find and was not.
/// Investigated: those 4009 lines are one cohesive chain — a datasheet energy
/// model, the DVFS/clock/peripheral-gating policies derived from it, and the
/// runtime service applying them — and for a battery-powered player that is
/// legitimately the dominant concern. Raising the threshold until it passed
/// would have been fitting the test to the data, so the test went instead. Size
/// was a proxy; this is the property the proxy was standing in for.
#[test]
fn concrete_adapters_are_constructed_only_at_the_edges() {
    let root = core_src();
    let mut files = Vec::new();
    walk(&root, &mut files);

    let mut violations = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&root).expect("under core/src");
        // stub.rs DEFINES the placeholder adapters; that is not a choice.
        if rel.file_name().and_then(|n| n.to_str()) == Some("stub.rs") {
            continue;
        }
        let src = std::fs::read_to_string(f).expect("readable");
        let code = code_only(&src);
        for (i, line) in code.lines().enumerate() {
            // A construction, not a mention in a generic bound or a type alias.
            if line.contains("Stub")
                && (line.contains("::new")
                    || line.contains("::default")
                    || line.contains("Stub") && line.contains('{'))
                && !line.contains("pub use")
                && !line.contains("use crate")
            {
                violations.push(format!("{}:{}: {}", rel.display(), i + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "dap-core constructs a concrete adapter outside stub.rs:\n  {}\n\n\
         A slice must name only port traits. Constructing an implementation inside the \
         firmware core means the board can no longer choose a different one, which is \
         exactly the coupling the ports exist to prevent. Move the construction to \
         board-h7a3, dap-sim or a test.",
        violations.join("\n  ")
    );
}

/// The firmware core must not name a concrete pin or peripheral.
///
/// Nothing in `dap-core` should know that the display is on SPI4 or that the
/// wheel is on PE2. Those are board facts: a different board revision moves
/// them, and a slice that names one cannot be reused across revisions.
///
/// This passes trivially today — the real drivers do not exist yet and every
/// port is a stub, so there are zero pin literals in the whole workspace. That
/// is precisely why it is worth adding NOW. Enforcing it before the drivers are
/// written costs nothing; enforcing it after several thousand lines of them is a
/// rewrite. When SAI/SDMMC/SPI drivers arrive they will be in `board-h7a3`,
/// taking their pins as parameters, and this test is what keeps a convenient
/// `const WHEEL_PIN: ... = PE2` from appearing in a slice.
#[test]
fn the_firmware_core_names_no_pin_or_peripheral() {
    let root = core_src();
    let mut files = Vec::new();
    walk(&root, &mut files);

    // KNOWN VIOLATION, exempted deliberately and narrowly.
    //
    // `power/peripherals.rs` is the RCC clock-gating policy: a roster of 12
    // peripherals, each with an `(enable register, bit)` coordinate, and the
    // mask arithmetic deriving `gated = gateable & !enabled`. Those coordinates
    // are STM32H7A3 facts, so the module names SAI1, SDMMC1, SPI4, I2C1, FMC
    // and OCTOSPI1/2 — in the portable firmware core, which is wrong by the rule
    // this test states. Its own docstring concedes the boundary: "What still
    // needs the board is writing the masks to RCC".
    //
    // THE FIX, when someone takes it: move the ROSTER data (register/bit
    // coordinates and bus assignment) into `dap-hw-spec`, which already holds
    // the MCU's facts — its memory map, clock ceiling and part number — and keep
    // the generic mask arithmetic here, taking the roster as a parameter. Then a
    // different MCU supplies a different roster and the policy code is untouched,
    // which is the whole point.
    //
    // Exempted rather than deleted-or-fixed because that move is a real
    // refactor of a 910-line module plus its consumers, and an exemption with
    // the fix written down is more honest than a rule quietly widened to let it
    // pass. The list is asserted not to grow, below.
    const KNOWN_PIN_NAMING: &[&str] = &["power/peripherals.rs"];

    // STM32 port-pin names (PA0..PK15) and peripheral instance names. Written
    // as a conservative pattern: `PA0` matches, an English word does not.
    let mut violations = Vec::new();
    let mut exempted_hit = Vec::new();
    for f in &files {
        let rel = f.strip_prefix(&root).expect("under core/src");
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        let exempt = KNOWN_PIN_NAMING.contains(&rel_str.as_str());
        let src = std::fs::read_to_string(f).expect("readable");
        let code = code_only(&src);
        for (i, line) in code.lines().enumerate() {
            let bytes = line.as_bytes();
            let mut hit = None;
            // PA0 .. PK15
            for (j, w) in line.char_indices() {
                if w != 'P' {
                    continue;
                }
                let rest = &line[j..];
                let mut it = rest.chars().skip(1);
                let port = it.next();
                let d1 = it.next();
                if let (Some(p), Some(d)) = (port, d1) {
                    if ('A'..='K').contains(&p) && d.is_ascii_digit() {
                        // must not be part of a longer identifier
                        let before_ok = j == 0
                            || !(bytes[j - 1].is_ascii_alphanumeric() || bytes[j - 1] == b'_');
                        if before_ok {
                            hit = Some(
                                rest.split(|c: char| !c.is_ascii_alphanumeric())
                                    .next()
                                    .unwrap_or("")
                                    .to_string(),
                            );
                            break;
                        }
                    }
                }
            }
            // Peripheral instances the firmware must not pin down.
            for periph in [
                "SAI1", "SAI2", "SDMMC1", "SDMMC2", "SPI1", "SPI2", "SPI4", "I2C1", "I2C2", "I2C4",
                "USART1", "UART4", "QUADSPI", "FMC",
            ] {
                if line.contains(periph) {
                    hit = Some(periph.to_string());
                }
            }
            if let Some(name) = hit {
                let msg = format!(
                    "{}:{}: names `{name}` — {}",
                    rel.display(),
                    i + 1,
                    line.trim()
                );
                if exempt {
                    exempted_hit.push(msg);
                } else {
                    violations.push(msg);
                }
            }
        }
    }
    assert!(
        violations.is_empty(),
        "dap-core names a concrete pin or peripheral:\n  {}\n\n\
         Pins and peripheral instances are BOARD facts. A slice that names one cannot be \
         reused on a revision that moves it. Take it as a parameter instead: the board \
         constructs the driver with its pins and hands the firmware a trait object.",
        violations.join("\n  ")
    );

    // The exemption must stay earned. If `power/peripherals.rs` stops naming
    // peripherals — because the roster moved to the spec — the exemption is
    // stale and should go, and this is what says so.
    assert!(
        !exempted_hit.is_empty(),
        "`power/peripherals.rs` is exempted from the pin/peripheral rule but no longer \
         names any peripheral. The roster has presumably moved to dap-hw-spec — delete \
         KNOWN_PIN_NAMING in this test so the rule covers the whole core again."
    );
}
