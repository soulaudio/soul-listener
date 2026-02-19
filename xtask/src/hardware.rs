use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;
use std::time::Instant;

#[derive(Debug, clap::Subcommand)]
pub enum HwCommand {
    /// Validate the Atopile design (ERC assert checks, no KiCad required)
    Check,
    /// Full build: validate + generate KiCad project (requires KiCad 8)
    Build,
    /// Install / update locked Atopile dependencies (ato sync)
    Sync,
    /// Generate bill of materials (ato build --build bom-only)
    Bom,
}

pub fn run(cmd: HwCommand) -> Result<()> {
    // Verify ato is installed
    let ato_check = Command::new("ato")
        .arg("--version")
        .output();

    if ato_check.is_err() || !ato_check.unwrap().status.success() {
        eprintln!("{}", "ato not found in PATH.".red().bold());
        eprintln!(
            "{}",
            "Install with: pip install atopile==0.12.5".dimmed()
        );
        anyhow::bail!("ato CLI not installed");
    }

    match cmd {
        HwCommand::Check => hw_check(),
        HwCommand::Build => hw_build(),
        HwCommand::Sync => hw_sync(),
        HwCommand::Bom => hw_bom(),
    }
}

fn hw_check() -> Result<()> {
    println!();
    println!("{}", "Atopile ERC check".cyan().bold());
    println!(
        "{}",
        "Validating assert statements (voltage ranges, interface compliance)...".dimmed()
    );
    println!();

    let start = Instant::now();

    let status = Command::new("ato")
        .args(["build", "--build", "check-only"])
        .current_dir("hardware")
        .status()
        .context("Failed to run 'ato build --build check-only'")?;

    if !status.success() {
        eprintln!();
        eprintln!("{}", "ERC check failed.".red().bold());
        eprintln!(
            "{}",
            "Review the assert statements in hardware/elec/src/*.ato".dimmed()
        );
        anyhow::bail!("Atopile ERC check failed");
    }

    println!();
    println!(
        "{}",
        format!(
            "ERC check passed in {:.2}s",
            start.elapsed().as_secs_f64()
        )
        .green()
        .bold()
    );
    println!();
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
        format!(
            "BOM generated in {:.2}s",
            start.elapsed().as_secs_f64()
        )
        .green()
        .bold()
    );
    println!();
    println!("{}", "Sourcing BOM files:".cyan());
    println!("  {}", "hardware/bom/sourcing/tme.csv       (TME — CZ/EU primary)".dimmed());
    println!("  {}", "hardware/bom/sourcing/digikey-eu.csv (Digi-Key EU — NL)".dimmed());
    println!("  {}", "hardware/bom/sourcing/mouser.csv     (Mouser EU — DE)".dimmed());
    println!();
    println!(
        "{}",
        "Atopile BOM output: hardware/build/".dimmed()
    );
    println!();
    Ok(())
}
