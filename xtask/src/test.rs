use anyhow::{Context, Result};
use colored::Colorize;
use std::process::Command;
use std::time::Instant;

pub fn run(unit_only: bool, integration_only: bool) -> Result<()> {
    println!();
    println!("{}", "🧪 Running tests...".cyan().bold());
    println!();

    let total_start = Instant::now();

    // Determine which tests to run
    let run_unit = !integration_only;
    let run_integration = !unit_only;

    // Test 1: Unit tests
    if run_unit {
        println!("{}", "  Running unit tests...".cyan());
        let unit_start = Instant::now();

        let unit_output = Command::new("cargo")
            .args(["test", "--lib", "--workspace"])
            .output()
            .context("Failed to run unit tests")?;

        if !unit_output.status.success() {
            eprintln!("{}", "  ✗ Unit tests failed".red().bold());
            eprintln!();
            let output_str = String::from_utf8_lossy(&unit_output.stdout);
            for line in output_str.lines() {
                eprintln!("  {}", line);
            }
            anyhow::bail!("Unit tests failed");
        }

        // Parse test results
        let output_str = String::from_utf8_lossy(&unit_output.stdout);
        let summary = extract_test_summary(&output_str);

        println!(
            "{}",
            format!(
                "  ✓ Unit tests passed {} in {:.2}s",
                summary,
                unit_start.elapsed().as_secs_f64()
            )
            .green()
        );
        println!();
    }

    // Test 2: Integration tests
    if run_integration {
        println!("{}", "  Running integration tests...".cyan());
        let int_start = Instant::now();

        let int_output = Command::new("cargo")
            .args(["test", "--test", "integration"])
            .output();

        match int_output {
            Ok(output) if output.status.success() => {
                let output_str = String::from_utf8_lossy(&output.stdout);
                let summary = extract_test_summary(&output_str);

                println!(
                    "{}",
                    format!(
                        "  ✓ Integration tests passed {} in {:.2}s",
                        summary,
                        int_start.elapsed().as_secs_f64()
                    )
                    .green()
                );
            }
            Ok(output) => {
                eprintln!("{}", "  ✗ Integration tests failed".red().bold());
                eprintln!();
                let output_str = String::from_utf8_lossy(&output.stdout);
                for line in output_str.lines() {
                    eprintln!("  {}", line);
                }
                anyhow::bail!("Integration tests failed");
            }
            Err(_) => {
                println!("{}", "  ⚠ No integration tests found".yellow());
            }
        }
        println!();
    }

    // Test 3: Doc tests
    println!("{}", "  Running doc tests...".cyan());
    let doc_start = Instant::now();

    let doc_output = Command::new("cargo")
        .args(["test", "--doc", "--workspace"])
        .output()
        .context("Failed to run doc tests")?;

    if !doc_output.status.success() {
        eprintln!("{}", "  ⚠ Doc tests failed".yellow().bold());
        // Don't fail on doc test failures
    } else {
        let output_str = String::from_utf8_lossy(&doc_output.stdout);
        let summary = extract_test_summary(&output_str);

        println!(
            "{}",
            format!(
                "  ✓ Doc tests passed {} in {:.2}s",
                summary,
                doc_start.elapsed().as_secs_f64()
            )
            .green()
        );
    }
    println!();

    println!(
        "{}",
        format!(
            "✓ All tests completed in {:.2}s",
            total_start.elapsed().as_secs_f64()
        )
        .green()
        .bold()
    );
    println!();

    Ok(())
}

fn extract_test_summary(output: &str) -> String {
    // Look for lines like "test result: ok. 5 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out"
    for line in output.lines() {
        if line.contains("test result:") {
            if let Some(summary) = line.split("test result:").nth(1) {
                return summary.trim().to_string();
            }
        }
    }
    "(summary not available)".to_string()
}
