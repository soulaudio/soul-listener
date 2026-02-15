use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;
use std::time::Instant;

pub fn run(release: bool) -> Result<()> {
    let mode = if release { "release" } else { "debug" };

    println!();
    println!(
        "{}",
        format!("🔨 Building firmware ({} mode)...", mode)
            .cyan()
            .bold()
    );
    println!();

    // Build firmware for STM32H7 target
    let build_start = Instant::now();
    let mut build_cmd = Command::new("cargo");
    build_cmd
        .arg("build")
        .arg("-p")
        .arg("firmware")
        .arg("--target")
        .arg("thumbv7em-none-eabihf")
        .arg("--features")
        .arg("hardware");

    if release {
        build_cmd.arg("--release");
    }

    let build_output = build_cmd.output().context("Failed to run cargo build")?;

    if !build_output.status.success() {
        eprintln!("{}", "✗ Build failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&build_output.stderr));
        anyhow::bail!("Build failed");
    }

    let build_time = build_start.elapsed();
    println!(
        "{}",
        format!("✓ Build successful in {:.2}s", build_time.as_secs_f64()).green()
    );
    println!();

    // Show binary size
    show_binary_size(release)?;
    println!();

    // Flash with probe-rs
    println!("{}", "📡 Flashing to STM32H7...".cyan().bold());
    println!("   {}", "Connecting to probe...".dimmed());

    let flash_start = Instant::now();
    let mut flash_cmd = Command::new("probe-rs");
    flash_cmd.arg("run");

    if release {
        flash_cmd.arg("target/thumbv7em-none-eabihf/release/firmware");
    } else {
        flash_cmd.arg("target/thumbv7em-none-eabihf/debug/firmware");
    }

    flash_cmd
        .arg("--chip")
        .arg("STM32H743ZITx")
        .arg("--probe-index")
        .arg("0");

    let flash_output = flash_cmd
        .output()
        .context("Failed to run probe-rs. Is probe-rs installed? (cargo install probe-rs-tools)")?;

    if !flash_output.status.success() {
        eprintln!("{}", "✗ Flash failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&flash_output.stderr));
        anyhow::bail!("Flash failed - check that the probe is connected and the device is powered");
    }

    let flash_time = flash_start.elapsed();
    println!(
        "{}",
        format!("✓ Flash successful in {:.2}s", flash_time.as_secs_f64()).green()
    );
    println!();
    println!("{}", "🎵 SoulAudio DAP is running on hardware!".bold());
    println!(
        "   {}",
        "Use 'probe-rs attach --chip STM32H743ZITx' to view RTT logs".dimmed()
    );
    println!();

    Ok(())
}

fn show_binary_size(release: bool) -> Result<()> {
    let binary_path = if release {
        "target/thumbv7em-none-eabihf/release/firmware"
    } else {
        "target/thumbv7em-none-eabihf/debug/firmware"
    };

    let output = Command::new("rust-size")
        .arg(binary_path)
        .arg("-A")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            println!("{}", "📊 Binary size:".cyan());
            let size_output = String::from_utf8_lossy(&out.stdout);
            for line in size_output.lines() {
                println!("   {}", line.dimmed());
            }
        }
        _ => {
            // If rust-size is not available, try cargo-size
            let output = Command::new("cargo")
                .arg("size")
                .arg("--")
                .arg("-A")
                .arg("--bin")
                .arg("firmware")
                .arg("--target")
                .arg("thumbv7em-none-eabihf")
                .output();

            if let Ok(out) = output {
                if out.status.success() {
                    println!("{}", "📊 Binary size:".cyan());
                    let size_output = String::from_utf8_lossy(&out.stdout);
                    for line in size_output.lines().skip(1) {
                        // Skip cargo output
                        println!("   {}", line.dimmed());
                    }
                }
            }
        }
    }

    Ok(())
}
