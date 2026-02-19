use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;
use std::time::Instant;

pub fn run() -> Result<()> {
    println!();
    println!("{}", "🔍 Checking all crates...".cyan().bold());
    println!();

    let total_start = Instant::now();

    // Check 1: Hardware target
    println!("{}", "  Checking hardware target (STM32H7)...".cyan());
    let hw_start = Instant::now();

    let hw_output = Command::new("cargo")
        .args([
            "check",
            "-p",
            "firmware",
            "--target",
            "thumbv7em-none-eabihf",
            "--features",
            "hardware",
        ])
        .output()
        .context("Failed to check hardware build")?;

    if !hw_output.status.success() {
        eprintln!("{}", "  ✗ Hardware check failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&hw_output.stderr));
        anyhow::bail!("Hardware check failed");
    }

    println!(
        "{}",
        format!(
            "  ✓ Hardware check passed in {:.2}s",
            hw_start.elapsed().as_secs_f64()
        )
        .green()
    );
    println!();

    // Check 2: Emulator target
    println!("{}", "  Checking emulator target (host)...".cyan());
    let emu_start = Instant::now();

    let emu_output = Command::new("cargo")
        .args(["check", "-p", "firmware", "--features", "emulator"])
        .output()
        .context("Failed to check emulator build")?;

    if !emu_output.status.success() {
        eprintln!("{}", "  ✗ Emulator check failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&emu_output.stderr));
        anyhow::bail!("Emulator check failed");
    }

    println!(
        "{}",
        format!(
            "  ✓ Emulator check passed in {:.2}s",
            emu_start.elapsed().as_secs_f64()
        )
        .green()
    );
    println!();

    // Check 3: Platform crate (no_std compatibility)
    println!("{}", "  Checking platform crate (no_std)...".cyan());
    let platform_start = Instant::now();

    let platform_output = Command::new("cargo")
        .args([
            "check",
            "-p",
            "platform",
            "--target",
            "thumbv7em-none-eabihf",
            "--no-default-features",
        ])
        .output()
        .context("Failed to check platform crate")?;

    if !platform_output.status.success() {
        eprintln!("{}", "  ✗ Platform check failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&platform_output.stderr));
        anyhow::bail!("Platform check failed");
    }

    println!(
        "{}",
        format!(
            "  ✓ Platform check passed in {:.2}s",
            platform_start.elapsed().as_secs_f64()
        )
        .green()
    );
    println!();

    // Check 4: eink-specs (no_std)
    println!("{}", "  Checking eink-specs (no_std)...".cyan());
    let specs_start = Instant::now();

    let specs_output = Command::new("cargo")
        .args([
            "check",
            "-p",
            "eink-specs",
            "--target",
            "thumbv7em-none-eabihf",
            "--no-default-features",
        ])
        .output()
        .context("Failed to check eink-specs")?;

    if !specs_output.status.success() {
        eprintln!("{}", "  ✗ eink-specs check failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&specs_output.stderr));
        anyhow::bail!("eink-specs check failed");
    }

    println!(
        "{}",
        format!(
            "  ✓ eink-specs check passed in {:.2}s",
            specs_start.elapsed().as_secs_f64()
        )
        .green()
    );
    println!();

    // Check 5: eink-system (no_std)
    println!("{}", "  Checking eink-system (no_std)...".cyan());
    let system_start = Instant::now();

    let system_output = Command::new("cargo")
        .args([
            "check",
            "-p",
            "eink-system",
            "--target",
            "thumbv7em-none-eabihf",
            "--no-default-features",
        ])
        .output()
        .context("Failed to check eink-system")?;

    if !system_output.status.success() {
        eprintln!("{}", "  ✗ eink-system check failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&system_output.stderr));
        anyhow::bail!("eink-system check failed");
    }

    println!(
        "{}",
        format!(
            "  ✓ eink-system check passed in {:.2}s",
            system_start.elapsed().as_secs_f64()
        )
        .green()
    );
    println!();

    // Check 6: eink-components (no_std)
    println!("{}", "  Checking eink-components (no_std)...".cyan());
    let components_start = Instant::now();

    let components_output = Command::new("cargo")
        .args([
            "check",
            "-p",
            "eink-components",
            "--target",
            "thumbv7em-none-eabihf",
            "--no-default-features",
        ])
        .output()
        .context("Failed to check eink-components")?;

    if !components_output.status.success() {
        eprintln!("{}", "  ✗ eink-components check failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&components_output.stderr));
        anyhow::bail!("eink-components check failed");
    }

    println!(
        "{}",
        format!(
            "  ✓ eink-components check passed in {:.2}s",
            components_start.elapsed().as_secs_f64()
        )
        .green()
    );
    println!();

    // Check 7: firmware-ui (no_std)
    println!("{}", "  Checking firmware-ui (no_std)...".cyan());
    let firmware_ui_start = Instant::now();

    let firmware_ui_output = Command::new("cargo")
        .args([
            "check",
            "-p",
            "firmware-ui",
            "--lib",
            "--target",
            "thumbv7em-none-eabihf",
            "--features",
            "no-std",
        ])
        .output()
        .context("Failed to check firmware-ui")?;

    if !firmware_ui_output.status.success() {
        eprintln!("{}", "  ✗ firmware-ui check failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&firmware_ui_output.stderr));
        anyhow::bail!("firmware-ui check failed");
    }

    println!(
        "{}",
        format!(
            "  ✓ firmware-ui check passed in {:.2}s",
            firmware_ui_start.elapsed().as_secs_f64()
        )
        .green()
    );
    println!();

    // Check 8: Clippy lints
    println!("{}", "  Running clippy lints...".cyan());
    let clippy_start = Instant::now();

    let clippy_output = Command::new("cargo")
        .args([
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ])
        .output()
        .context("Failed to run clippy")?;

    if !clippy_output.status.success() {
        eprintln!("{}", "  ✗ Clippy warnings found".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&clippy_output.stderr));
        anyhow::bail!("Clippy check failed");
    } else {
        println!(
            "{}",
            format!(
                "  ✓ Clippy passed in {:.2}s",
                clippy_start.elapsed().as_secs_f64()
            )
            .green()
        );
    }
    println!();

    // Check 9: Format check
    println!("{}", "  Checking code formatting...".cyan());

    let fmt_output = Command::new("cargo")
        .args(["fmt", "--all", "--check"])
        .output()
        .context("Failed to run cargo fmt")?;

    if !fmt_output.status.success() {
        eprintln!("{}", "  ✗ Formatting issues found".red().bold());
        eprintln!("     Run 'cargo fmt --all' to fix");
        anyhow::bail!("Format check failed");
    } else {
        println!("{}", "  ✓ Formatting check passed".green());
    }
    println!();

    println!(
        "{}",
        format!(
            "✓ All checks completed in {:.2}s",
            total_start.elapsed().as_secs_f64()
        )
        .green()
        .bold()
    );
    println!();

    Ok(())
}
