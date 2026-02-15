use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;
use std::time::Instant;

pub fn run(open: bool) -> Result<()> {
    println!();
    println!("{}", "📚 Building documentation...".cyan().bold());
    println!();

    let start = Instant::now();

    let mut cmd = Command::new("cargo");
    cmd.arg("doc")
        .arg("--workspace")
        .arg("--no-deps")
        .arg("--document-private-items");

    if open {
        cmd.arg("--open");
    }

    let output = cmd.output().context("Failed to build documentation")?;

    if !output.status.success() {
        eprintln!("{}", "✗ Documentation build failed".red().bold());
        eprintln!();
        eprintln!("{}", String::from_utf8_lossy(&output.stderr));
        anyhow::bail!("Documentation build failed");
    }

    println!(
        "{}",
        format!(
            "✓ Documentation built in {:.2}s",
            start.elapsed().as_secs_f64()
        )
        .green()
    );

    if !open {
        println!();
        println!(
            "   {}",
            "Open target/doc/firmware/index.html in your browser".dimmed()
        );
        println!(
            "   {}",
            "Or run 'cargo run -p xtask -- doc --open'".dimmed()
        );
    }

    println!();

    Ok(())
}
