use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;
use std::time::Instant;

/// The atopile build stages that run BEFORE part picking.
///
/// This list is the whole basis of `hw check`, so it is worth being explicit
/// about why it exists. On atopile 0.15.8+ the `picker` stage resolves generic
/// components (a `Capacitor` with a capacitance and a tolerance) against a
/// remote parts service, and that service now REQUIRES SIGN-IN. There is no
/// offline mode and no flag that skips the stage: `-x picker`,
/// `exclude_targets: [picker]` and every `-t <earlier-stage>` were all measured
/// and all still run it.
///
/// A lot still happens before that point, and it was worth measuring exactly
/// what, rather than assuming. Each of these was checked by breaking a real
/// module and confirming the result:
///
/// | mutation                               | caught offline? |
/// |----------------------------------------|-----------------|
/// | syntax error                           | **yes**         |
/// | reference to a non-existent field      | **yes**         |
/// | shorted power rail (an ERC fault)      | **yes**         |
/// | an `assert` that is plainly false       | **NO**          |
///
/// That last row is the important one and it corrects what this comment first
/// claimed. `assert` statements are not predicates evaluated during
/// instantiation — they are constraints fed to a solver, and the solve plus
/// `post-solve-checks` run AFTER the picker. Changing
/// `assert xtal.frequency within 24MHz to 26MHz` to `10MHz to 12MHz`, against a
/// crystal declared at 25 MHz, passes this check.
///
/// So the gate is honestly two-tier, and neither tier should be described as the
/// other:
///
/// * **Offline (this check, the git hook, CI without a token):** structure and
///   connectivity. Syntax, imports, field resolution, the instance graph, ERC.
/// * **Authenticated (CI with `ATO_TOKEN`):** the above, plus part picking, the
///   constraint solve, the `assert` design rules and DRC.
///
/// `hw check` treats "every stage green, blocked at `Picking parts` with a
/// sign-in error" as a PASS for the offline tier, and anything failing earlier
/// as a FAIL. That rule is deliberate and slightly uncomfortable, which is why
/// it lives in one place with this comment attached rather than being spread
/// across a git hook and a CI step.
const PRE_PICKER_STAGES: usize = 9;

/// The per-module `check-*` builds declared in `hardware/ato.yaml`, paired with
/// the source file whose change should trigger them.
///
/// This is the vertical slice: editing `power.ato` runs `check-power` and
/// nothing else. Measured cold, through `cargo run`: one module is 20-35 s,
/// where every module plus the whole board is about 5 minutes. A 10x saving,
/// not the 14x an earlier version of this comment claimed from a warm run.
pub const MODULE_BUILDS: &[(&str, &str)] = &[
    ("power.ato", "check-power"),
    ("mcu.ato", "check-mcu"),
    ("audio.ato", "check-audio"),
    ("display.ato", "check-display"),
    ("bluetooth.ato", "check-bluetooth"),
    ("memory.ato", "check-memory"),
    ("input.ato", "check-input"),
    ("clock.ato", "check-clock"),
];

#[derive(Debug, clap::Subcommand)]
pub enum HwCommand {
    /// Validate the Atopile design (ERC assert checks, no KiCad required)
    Check,
    /// Render the board: 2D SVG and 3D GLB/PNG via kicad-cli
    Render,
    /// Validate only the modules whose .ato files changed (the lefthook hook)
    CheckChanged {
        /// Changed file paths. Non-.ato paths and unmapped modules are ignored.
        files: Vec<String>,
    },
    /// Full build: validate + generate KiCad project (requires KiCad 8)
    Build,
    /// Install / update locked Atopile dependencies (ato sync)
    Sync,
    /// Generate bill of materials (ato build --build bom-only)
    Bom,
}

pub fn run(cmd: HwCommand) -> Result<()> {
    // `render` drives kicad-cli against the committed PCB and never invokes
    // atopile, so requiring the atopile toolchain for it would block the one
    // command that works on a machine with neither atopile nor a sign-in.
    if matches!(cmd, HwCommand::Render) {
        return hw_render();
    }

    // Verify ato is installed
    let ato_check = Command::new("ato").arg("--version").output();

    if ato_check.is_err() || !ato_check.unwrap().status.success() {
        // A missing toolchain is reported, not failed, for the hook path: most
        // contributors to this repo touch firmware and have no reason to carry a
        // Python 3.14 + atopile install. CI does have it, and CI is where the
        // design is actually gated.
        if matches!(cmd, HwCommand::CheckChanged { .. }) {
            eprintln!(
                "{}",
                "ato not found in PATH — skipping the hardware check.".yellow()
            );
            eprintln!(
                "{}",
                "  Install: uv tool install --python 3.14 atopile==0.15.9".dimmed()
            );
            eprintln!(
                "{}",
                "  The Hardware CI workflow checks the design regardless.".dimmed()
            );
            return Ok(());
        }
        eprintln!("{}", "ato not found in PATH.".red().bold());
        eprintln!(
            "{}",
            "Install with: uv tool install --python 3.14 atopile==0.15.9".dimmed()
        );
        eprintln!(
            "{}",
            "0.15.9 specifically — see docs/hardware/ATOPILE-TOOLCHAIN.md.".dimmed()
        );
        anyhow::bail!("ato CLI not installed");
    }

    match cmd {
        HwCommand::Check => hw_check(),
        HwCommand::Render => hw_render(),
        HwCommand::CheckChanged { files } => hw_check_changed(&files),
        HwCommand::Build => hw_build(),
        HwCommand::Sync => hw_sync(),
        HwCommand::Bom => hw_bom(),
    }
}

/// Run one `check-*` build and classify the outcome.
///
/// Returns `Ok(true)` when the design validated, `Ok(false)` when it did not.
/// The distinction that matters is in [`PRE_PICKER_STAGES`]: a run that clears
/// every validation stage and then stops at `Picking parts` because nobody is
/// signed in has told us everything an unauthenticated machine can know, and
/// reporting that as a failure would train people to ignore this check.
fn run_check_build(build: &str) -> Result<bool> {
    let start = Instant::now();
    let out = Command::new("ato")
        .args(["build", "--build", build])
        .current_dir("hardware")
        .output()
        .with_context(|| format!("failed to run 'ato build --build {build}'"))?;

    // atopile writes the stage list to stderr via its live display.
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let green = text.matches('\u{2713}').count();
    let sign_in_blocked = text.contains("Sign-in required") || text.contains("requires sign-in");
    let elapsed = start.elapsed().as_secs_f64();

    if out.status.success() {
        println!(
            "  {} {build}  ({green} stages, {elapsed:.1}s)",
            "PASS".green().bold()
        );
        return Ok(true);
    }
    if sign_in_blocked && green >= PRE_PICKER_STAGES {
        println!(
            "  {} {build}  ({green} stages green, part picking needs sign-in, {elapsed:.1}s)",
            "PASS".green().bold()
        );
        return Ok(true);
    }
    if sign_in_blocked {
        // Blocked at the picker but something earlier also failed: the sign-in
        // message must not be allowed to excuse a real error.
        println!(
            "  {} {build}  (only {green} of {PRE_PICKER_STAGES} stages green before \
             part picking, {elapsed:.1}s)",
            "FAIL".red().bold()
        );
    } else {
        println!("  {} {build}  ({elapsed:.1}s)", "FAIL".red().bold());
    }
    // Only now print atopile's own output, so a passing run stays quiet and a
    // failing one shows the compiler's message rather than a summary of it.
    eprintln!("{text}");
    Ok(false)
}

/// Render the board with `kicad-cli`.
///
/// Deliberately NOT `ato build -t glb`. atopile's 3D targets are tagged
/// `REQUIRES_KICAD` and sit after the part picker, so they need a sign-in even
/// to render an outline. `kicad-cli` reads the committed `.kicad_pcb` directly,
/// which works today and shows whatever the board actually contains.
///
/// Measured: exporting does NOT modify the input. The hash of
/// `default.kicad_pcb` is unchanged across `export glb` and `export svg`, which
/// matters because atopile writes the PCB in KiCad 9's format (version
/// 20241229) while nixpkgs ships KiCad 10 — a read-write round trip through the
/// newer tool could migrate the file and break the `--frozen` lock.
///
/// Components appear once parts are vendored under `hardware/elec/src/parts/`;
/// until then this renders the outline and the floorplan annotation, which is
/// still the fastest way to see whether the board shape is right.
fn hw_render() -> Result<()> {
    let kicad = Command::new("kicad-cli").arg("version").output();
    if kicad.is_err() || !kicad.unwrap().status.success() {
        eprintln!("{}", "kicad-cli not found in PATH.".red().bold());
        eprintln!(
            "{}",
            "  nix shell nixpkgs#kicad --command cargo xtask hardware render".dimmed()
        );
        anyhow::bail!("kicad-cli not installed");
    }

    let pcb = "hardware/elec/layouts/default/default.kicad_pcb";
    let out_dir = std::path::Path::new("hardware/build/render");
    std::fs::create_dir_all(out_dir).context("create hardware/build/render")?;

    println!();
    println!("{}", "Rendering the board".cyan().bold());
    println!();

    // (target, kicad-cli args). SVG first: it is the one that works with no 3D
    // models at all, so a board with no vendored parts still produces something
    // a person can look at.
    let jobs: &[(&str, &[&str])] = &[
        (
            "default.svg",
            &[
                "pcb",
                "export",
                "svg",
                "--layers",
                "F.Cu,B.Cu,F.SilkS,Edge.Cuts,Dwgs.User,Cmts.User",
                "--exclude-drawing-sheet",
            ],
        ),
        (
            "default.glb",
            &[
                "pcb",
                "export",
                "glb",
                "--include-tracks",
                "--include-zones",
            ],
        ),
        (
            "default.png",
            &[
                "pcb",
                "render",
                "--width",
                "1400",
                "--height",
                "900",
                "--side",
                "top",
                "--quality",
                "high",
            ],
        ),
    ];

    let mut made = Vec::new();
    for (name, args) in jobs {
        let out = out_dir.join(name);
        let status = Command::new("kicad-cli")
            .args(*args)
            .arg("--output")
            .arg(&out)
            .arg(pcb)
            .output()
            .with_context(|| format!("kicad-cli {}", args.join(" ")))?;
        if status.status.success() && out.exists() {
            let bytes = std::fs::metadata(&out).map(|m| m.len()).unwrap_or(0);
            println!("  {} {name}  ({bytes} bytes)", "OK".green().bold());
            made.push(name.to_string());
        } else {
            println!("  {} {name}", "FAIL".red().bold());
            eprintln!("{}", String::from_utf8_lossy(&status.stderr));
        }
    }
    println!();
    if made.is_empty() {
        anyhow::bail!("no render produced");
    }
    println!(
        "{}",
        format!("wrote {} to hardware/build/render/", made.join(", ")).green()
    );
    println!(
        "{}",
        "  Components appear once parts are vendored — see tools/ato-vendor-part.py".dimmed()
    );
    println!();
    Ok(())
}

/// Check the declared floorplan (`hardware/elec/placement.yaml`).
///
/// Needs neither atopile nor the network: the spec-against-itself half is pure
/// geometry, and the spec-against-PCB half is skipped while the board has no
/// placed footprints. Run through Python because the spec is data and the check
/// is arithmetic over it, not something that wants a Rust type.
fn run_placement_check() -> Result<bool> {
    run_tool("tools/ato-placement.py", &[], "floorplan")
}

/// Run one of the repo's Python hardware checks and report pass/fail.
///
/// Python because these are data checks — geometry over a YAML spec, file
/// inventory over a parts directory — not something that wants a Rust type. The
/// output is only shown on failure, so a passing run stays one line.
fn run_tool(script: &str, args: &[&str], label: &str) -> Result<bool> {
    let out = Command::new("python3")
        .arg(script)
        .args(args)
        .output()
        .with_context(|| format!("failed to run {script}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    if out.status.success() {
        println!("  {} {label}", "PASS".green().bold());
        return Ok(true);
    }
    println!("  {} {label}", "FAIL".red().bold());
    eprintln!("{text}");
    Ok(false)
}

fn hw_check() -> Result<()> {
    println!();
    println!("{}", "Atopile design check — every module".cyan().bold());
    println!(
        "{}",
        "Syntax, imports, field resolution, instance graph and ERC. No KiCad, no network.".dimmed()
    );
    println!(
        "{}",
        "NOT checked here: `assert` design rules and DRC — those need the solver, which runs \
         after part picking, which needs sign-in. CI with a token covers them."
            .dimmed()
    );
    println!();

    let start = Instant::now();
    let mut failed = Vec::new();
    for (_, build) in MODULE_BUILDS {
        if !run_check_build(build)? {
            failed.push(*build);
        }
    }
    // The whole board last: it catches the cross-module wiring that a
    // per-module check cannot see, and it is the slow one.
    if !run_check_build("default")? {
        failed.push("default");
    }

    // The floorplan. Separate from atopile entirely — it checks
    // hardware/elec/placement.yaml, which exists because atopile 0.15.9 has no
    // way to state placement in the language at all.
    if !run_placement_check()? {
        failed.push("floorplan");
    }
    if !run_tool("tools/ato-vendor-part.py", &["--check"], "vendored parts")? {
        failed.push("vendored parts");
    }

    println!();
    if !failed.is_empty() {
        anyhow::bail!("atopile design check failed: {}", failed.join(", "));
    }
    println!(
        "{}",
        format!(
            "design check passed in {:.1}s",
            start.elapsed().as_secs_f64()
        )
        .green()
        .bold()
    );
    println!();
    Ok(())
}

/// Check only the slices whose `.ato` files changed.
///
/// A pre-commit hook has to be fast enough that people do not reach for
/// `--no-verify`. Measured: one module 20-35 s, the full design about 5 minutes.
/// 25 seconds on a hardware commit is tolerable; five minutes is not, which is
/// the whole reason this routes by slice.
///
/// A change to anything shared — `main.ato`, `interfaces.ato`, a file under
/// `parts/`, or `ato.yaml` — cannot be attributed to one slice, so it escalates
/// to the full check. That is the conservative direction: the cost of being
/// wrong is a slow commit, where the other way round it is an unchecked design.
fn hw_check_changed(files: &[String]) -> Result<()> {
    let mut builds: Vec<&str> = Vec::new();
    let mut full = false;

    for f in files {
        let norm = f.replace('\\', "/");
        if !norm.ends_with(".ato")
            && !norm.ends_with("ato.yaml")
            && !norm.ends_with("placement.yaml")
        {
            continue;
        }
        if norm.ends_with("ato.yaml")
            || norm.ends_with("placement.yaml")
            || norm.ends_with("main.ato")
            || norm.ends_with("interfaces.ato")
            || norm.contains("/parts/")
        {
            full = true;
            continue;
        }
        if let Some((_, build)) = MODULE_BUILDS
            .iter()
            .find(|(src, _)| norm.ends_with(&format!("/{src}")) || norm == *src)
        {
            if !builds.contains(build) {
                builds.push(build);
            }
        } else {
            // An .ato file with no declared check build. Escalating rather than
            // skipping: a module nobody checks is the failure mode this hook
            // exists to prevent, and `every_ato_module_has_a_check_build` in the
            // arch tests is what stops it happening quietly.
            full = true;
        }
    }

    if !full && builds.is_empty() {
        return Ok(());
    }

    println!();
    if full {
        println!(
            "{}",
            "Atopile: a shared file changed — checking the whole design"
                .cyan()
                .bold()
        );
        return hw_check();
    }

    println!(
        "{} {}",
        "Atopile: checking changed slice(s):".cyan().bold(),
        builds.join(", ")
    );
    println!();
    let mut failed = Vec::new();
    for b in &builds {
        if !run_check_build(b)? {
            failed.push(*b);
        }
    }
    println!();
    if !failed.is_empty() {
        anyhow::bail!("atopile design check failed: {}", failed.join(", "));
    }
    Ok(())
}

fn hw_build() -> Result<()> {
    println!();
    println!("{}", "Atopile full build".cyan().bold());
    println!(
        "{}",
        "Compiling .ato → KiCad project (requires KiCad 8 installed)...".dimmed()
    );
    println!();

    let start = Instant::now();

    let status = Command::new("ato")
        .arg("build")
        .current_dir("hardware")
        .status()
        .context("Failed to run 'ato build'")?;

    if !status.success() {
        eprintln!();
        eprintln!("{}", "Build failed.".red().bold());
        anyhow::bail!("Atopile build failed");
    }

    println!();
    println!(
        "{}",
        format!(
            "Build complete in {:.2}s — KiCad project written to hardware/build/",
            start.elapsed().as_secs_f64()
        )
        .green()
        .bold()
    );
    println!();
    Ok(())
}

fn hw_sync() -> Result<()> {
    println!();
    println!("{}", "Atopile sync".cyan().bold());
    println!(
        "{}",
        "Installing locked dependencies from hardware/ato-lock.yaml...".dimmed()
    );
    println!();

    let start = Instant::now();

    let status = Command::new("ato")
        .arg("sync")
        .current_dir("hardware")
        .status()
        .context("Failed to run 'ato sync'")?;

    if !status.success() {
        eprintln!();
        eprintln!("{}", "Sync failed.".red().bold());
        anyhow::bail!("Atopile sync failed");
    }

    println!();
    println!(
        "{}",
        format!(
            "Dependencies synced in {:.2}s",
            start.elapsed().as_secs_f64()
        )
        .green()
        .bold()
    );
    println!();
    Ok(())
}

fn hw_bom() -> Result<()> {
    println!();
    println!("{}", "Atopile BOM generation".cyan().bold());
    println!(
        "{}",
        "Generating bill of materials from netlist...".dimmed()
    );
    println!();

    let start = Instant::now();

    let status = Command::new("ato")
        .args(["build", "--build", "bom-only"])
        .current_dir("hardware")
        .status()
        .context("Failed to run 'ato build --build bom-only'")?;

    if !status.success() {
        eprintln!();
        eprintln!("{}", "BOM generation failed.".red().bold());
        anyhow::bail!("BOM generation failed");
    }

    println!();
    println!(
        "{}",
        format!("BOM generated in {:.2}s", start.elapsed().as_secs_f64())
            .green()
            .bold()
    );
    println!();
    // Name the revision these files bill. They bill the schematic in
    // hardware/elec/src, which is V1, while the firmware targets V2 — printing
    // them as plain "sourcing BOM files" invites reading a correct V1 BOM as a
    // current shopping list, which is the confusion
    // dap/arch-tests/tests/sourcing_boms.rs exists to prevent.
    //
    // Read from the file's own `# Revision:` header rather than hardcoded, so
    // this line cannot drift from the files it is describing — the same failure
    // mode the header was added to fix.
    let billed = std::fs::read_to_string("hardware/bom/sourcing/tme.csv")
        .ok()
        .and_then(|t| {
            t.lines()
                .filter(|l| l.starts_with('#'))
                .filter_map(|l| l.trim_start_matches('#').trim().split_once(':'))
                .find(|(k, _)| k.trim().eq_ignore_ascii_case("Revision"))
                .map(|(_, v)| v.trim().to_string())
        });
    match billed {
        Some(rev) => println!(
            "{}",
            format!("Sourcing BOM files (bill the {rev} schematic):").cyan()
        ),
        None => println!("{}", "Sourcing BOM files:".cyan()),
    }
    println!(
        "  {}",
        "hardware/bom/sourcing/tme.csv       (TME — CZ/EU primary)".dimmed()
    );
    println!(
        "  {}",
        "hardware/bom/sourcing/digikey-eu.csv (Digi-Key EU — NL)".dimmed()
    );
    println!(
        "  {}",
        "hardware/bom/sourcing/mouser.csv     (Mouser EU — DE)".dimmed()
    );
    println!();
    println!("{}", "Atopile BOM output: hardware/build/".dimmed());
    println!();
    Ok(())
}
