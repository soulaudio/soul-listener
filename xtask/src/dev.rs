//! xtask dev - Hot-Reload Development Mode
//!
//! # Hot-Reload Architecture
//!
//! ## Current: Kill-and-Restart (default)
//!
//! When xtask dev runs without --hot-reload, it uses kill-and-restart:
//! 1. Build display_emulator binary with cargo run
//! 2. Spawn the emulator process
//! 3. Watch .rs/.toml files for changes
//! 4. On change: kill the process, rebuild, restart
//!
//! Pro: Works for all code changes
//! Con: Window closes and reopens on every change (~3-7s cycle)
//!
//! ## True In-Process Hot-Reload (--hot-reload flag)
//!
//! When xtask dev runs with --hot-reload:
//! 1. Build firmware-ui as a cdylib (firmware_ui.dll on Windows)
//! 2. Build display_emulator with emulator,hot-reload features
//! 3. Spawn the emulator - it uses hot-lib-reloader to dlopen the dylib
//! 4. Watch ONLY crates/firmware-ui/src/ for changes
//! 5. On change: rebuild ONLY the dylib (fast, no window close)
//! 6. The running emulator picks up the new dylib automatically
//!
//! Pro: Window stays open; only render logic is swapped
//! Con: Only changes to crates/firmware-ui/src/ trigger hot-reload
//!
//! ## Full Implementation Plan
//!
//! The following steps would be needed to productize this fully:
//!
//! 1. hot-lib-reloader watches source_files in define_lib_reloader! and
//!    invokes cargo to rebuild the dylib. We still need a file watcher here
//!    for the xtask level to watch for non-UI changes (firmware src, etc).
//!
//! 2. The render_ui() unsafe function in firmware-ui/src/lib.rs takes a
//!    raw pointer. Adding versioned shared state via shared memory or
//!    message passing would make the hot-reload boundary type-safe.
//!
//! 3. On Windows, the DLL is locked while loaded. hot-lib-reloader works
//!    around this by loading a copy with a unique filename. This is handled
//!    automatically by hot-lib-reloader 0.8.

use anyhow::{Context, Result};
use colored::Colorize;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use platform::config;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Child, Command};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

pub fn run(headless: bool, hot_reload: bool) -> Result<()> {
    clear_screen();
    print_banner();

    if hot_reload {
        println!("{}", "Starting TRUE in-process hot-reload mode".bold());
        println!(
            "{}",
            "Watching: crates/firmware-ui/src/ (UI code only)".dimmed()
        );
        println!(
            "{}",
            "Dylib: target/debug/firmware_ui.dll reloads without restart".dimmed()
        );
    } else {
        println!("{}", "Starting hot-reload development mode".bold());
        println!(
            "{}",
            "Watching: firmware, platform, eink-components, eink-system".dimmed()
        );
    }
    println!();

    // Initial build and run
    let mut emulator_process = match start_emulator(headless, hot_reload) {
        Ok(process) => process,
        Err(e) => {
            eprintln!("{}", format!("Build failed: {}", e).red().bold());
            eprintln!("{}", "Fix errors and save to trigger rebuild".dimmed());
            println!();
            None
        }
    };

    // Set up file watcher
    let (tx, rx) = channel();
    let mut watcher: RecommendedWatcher = Watcher::new(
        move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                // Only trigger on modify and create events for Rust files
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_))
                    && event.paths.iter().any(|p| {
                        p.extension()
                            .map(|ext| ext == "rs" || ext == "toml")
                            .unwrap_or(false)
                    })
                {
                    let _ = tx.send(());
                }
            }
        },
        notify::Config::default(),
    )?;

    // Watch firmware source directory, examples, and Cargo.toml
    let watch_paths = vec![
        Path::new("crates/firmware/src"),
        Path::new("crates/firmware/examples"), // Watch examples for hot-reload
        Path::new("crates/firmware/Cargo.toml"),
        Path::new("crates/platform"),
        Path::new("crates/eink/eink-components/src"), // Watch UI components
        Path::new("crates/eink/eink-system/src"),     // Watch layout system
        Path::new("crates/eink/eink-emulator/src"),   // Watch emulator rendering
    ];

    for path in &watch_paths {
        if path.exists() {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .with_context(|| format!("Failed to watch path: {}", path.display()))?;
        }
    }

    println!("{}", "Hot-reload active".green().bold());
    println!(
        "{}",
        "Save any .rs or .toml file to trigger rebuild".dimmed()
    );
    println!("{}", "Press Ctrl+C to stop".dimmed());
    println!();

    if emulator_process.is_some() {
        println!("{}", "Emulator running".cyan());
        println!();
    }

    // Main loop - wait for file changes
    let mut last_rebuild = Instant::now();

    loop {
        match rx.recv_timeout(Duration::from_millis(100)) {
            Ok(_) => {
                // Debounce - wait a bit for multiple file events
                let elapsed = last_rebuild.elapsed();
                if elapsed < Duration::from_millis(500) {
                    continue;
                }

                std::thread::sleep(Duration::from_millis(200));

                // Drain any pending events
                while rx.try_recv().is_ok() {}

                last_rebuild = Instant::now();

                println!();
                println!("{}", "Changes detected - rebuilding...".yellow().bold());
                println!();

                // Kill old emulator process
                if let Some(mut process) = emulator_process.take() {
                    let _ = process.kill();
                    let _ = process.wait();
                }

                // Clear screen and show fresh output
                clear_screen();
                print_banner();

                // Rebuild and restart
                match start_emulator(headless, hot_reload) {
                    Ok(new_process) => {
                        emulator_process = new_process;
                        println!();
                        println!("{}", "Reload complete".green().bold());
                        println!();

                        #[cfg(feature = "notifications")]
                        {
                            let _ = notify_rust::Notification::new()
                                .summary("SoulAudio DAP")
                                .body("Hot-reload complete!")
                                .timeout(2000)
                                .show();
                        }
                    }
                    Err(e) => {
                        println!();
                        eprintln!("{}", format!("Build failed: {}", e).red().bold());
                        println!("{}", "Fix errors and save to trigger rebuild".dimmed());
                        println!();
                        // Don't crash, just wait for next change
                        continue;
                    }
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                // Check if emulator is still running
                if let Some(ref mut process) = emulator_process {
                    if let Ok(Some(status)) = process.try_wait() {
                        println!();
                        if status.success() {
                            println!("{}", "Emulator exited normally".yellow());
                        } else {
                            println!(
                                "{}",
                                format!("Emulator exited with code: {:?}", status.code()).yellow()
                            );
                        }
                        println!("{}", "Waiting for changes...".dimmed());
                        println!();
                        emulator_process = None;
                    }
                }
            }
            Err(e) => {
                eprintln!("Watcher error: {}", e);
                break;
            }
        }
    }

    // Cleanup
    if let Some(mut process) = emulator_process {
        let _ = process.kill();
        let _ = process.wait();
    }

    Ok(())
}

fn start_emulator(headless: bool, hot_reload: bool) -> Result<Option<Child>> {
    let start = Instant::now();

    println!();
    if hot_reload {
        // Build the firmware-ui dylib first (needed by the emulator for dlopen)
        println!("{}", "Building firmware-ui dylib...".bold());
        let status = Command::new("cargo")
            .args([
                "build",
                "--package",
                "firmware-ui",
                "--features",
                "hot-reload",
            ])
            .status()
            .context("Failed to build firmware-ui dylib")?;
        if !status.success() {
            anyhow::bail!("firmware-ui dylib build failed");
        }
        println!("{}", "Dylib built. Building emulator binary...".bold());
    } else {
        println!("{}", "Building emulator...".bold());
    }
    println!();

    // Build and run in one command (don't build twice!)
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--example")
        .arg("display_emulator")
        .arg("-p")
        .arg("firmware")
        .arg("--features")
        .arg(if hot_reload {
            "emulator,hot-reload"
        } else {
            "emulator,debug,keyboard-input"
        })
        .env("HOT_RELOAD_MODE", "1") // Signal to example that we're in hot-reload mode
        .stdout(std::process::Stdio::inherit()) // Show live output
        .stderr(std::process::Stdio::inherit()); // Show errors in real-time

    // Forward RUST_LOG to the child process, defaulting to "info" so that
    // log::info!() calls inside the emulator binary are always visible.
    if std::env::var("RUST_LOG").is_err() {
        cmd.env("RUST_LOG", "info");
    }

    if headless {
        println!("{}", "Running in headless mode (no window)".dimmed());
        let status = cmd.status().context("Failed to run cargo")?;

        if !status.success() {
            anyhow::bail!("Build/run failed");
        }

        let build_time = start.elapsed();
        println!();
        println!(
            "{}",
            format!("Completed in {:.1}s", build_time.as_secs_f64()).green()
        );
        return Ok(None);
    }

    // Spawn for windowed mode
    let child = cmd.spawn().context("Failed to spawn emulator")?;

    let build_time = start.elapsed();
    println!();
    println!(
        "{}",
        format!("Started in {:.1}s", build_time.as_secs_f64()).green()
    );
    println!();

    Ok(Some(child))
}

fn clear_screen() {
    // ANSI escape code to clear screen and move cursor to top-left
    print!("\x1B[2J\x1B[1;1H");
    io::stdout().flush().ok();
}

fn print_banner() {
    let banner_text = format!("     {} - Development Mode       ", config::APP_NAME);
    println!("{}", "═════════════════════════════════════════════".cyan());
    println!("{}", banner_text.cyan().bold());
    println!("{}", "═════════════════════════════════════════════".cyan());
    println!();
}
