use anyhow::{Context, Result};
use colored::Colorize;
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use platform::config;
use std::io::{self, Write};
use std::path::Path;
use std::process::{Child, Command};
use std::sync::mpsc::channel;
use std::time::{Duration, Instant};

pub fn run(headless: bool) -> Result<()> {
    clear_screen();
    print_banner();

    println!("{}", "Starting hot-reload development mode".bold());
    println!("{}", "Watching: firmware, platform, eink-components, eink-system".dimmed());
    println!();

    // Initial build and run
    let mut emulator_process = match start_emulator(headless) {
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
                if matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
                    if event.paths.iter().any(|p| {
                        p.extension()
                            .map(|ext| ext == "rs" || ext == "toml")
                            .unwrap_or(false)
                    }) {
                        let _ = tx.send(());
                    }
                }
            }
        },
        notify::Config::default(),
    )?;

    // Watch firmware source directory, examples, and Cargo.toml
    let watch_paths = vec![
        Path::new("crates/firmware/src"),
        Path::new("crates/firmware/examples"),  // Watch examples for hot-reload
        Path::new("crates/firmware/Cargo.toml"),
        Path::new("crates/platform"),
        Path::new("crates/eink/eink-components/src"),  // Watch UI components
        Path::new("crates/eink/eink-system/src"),      // Watch layout system
    ];

    for path in &watch_paths {
        if path.exists() {
            watcher
                .watch(path, RecursiveMode::Recursive)
                .with_context(|| format!("Failed to watch path: {}", path.display()))?;
        }
    }

    println!("{}", "Hot-reload active".green().bold());
    println!("{}", "Save any .rs or .toml file to trigger rebuild".dimmed());
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
                match start_emulator(headless) {
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

fn start_emulator(headless: bool) -> Result<Option<Child>> {
    let start = Instant::now();

    println!();
    println!("{}", "Building emulator...".bold());
    println!();

    // Build and run in one command (don't build twice!)
    let mut cmd = Command::new("cargo");
    cmd.arg("run")
        .arg("--example")
        .arg("display_emulator")
        .arg("-p")
        .arg("firmware")
        .arg("--features")
        .arg("emulator")
        .env("HOT_RELOAD_MODE", "1")  // Signal to example that we're in hot-reload mode
        .stdout(std::process::Stdio::inherit())  // Show live output
        .stderr(std::process::Stdio::inherit()); // Show errors in real-time

    if headless {
        println!("{}", "Running in headless mode (no window)".dimmed());
        let status = cmd.status().context("Failed to run cargo")?;

        if !status.success() {
            anyhow::bail!("Build/run failed");
        }

        let build_time = start.elapsed();
        println!();
        println!("{}", format!("Completed in {:.1}s", build_time.as_secs_f64()).green());
        return Ok(None);
    }

    // Spawn for windowed mode
    let child = cmd.spawn().context("Failed to spawn emulator")?;

    let build_time = start.elapsed();
    println!();
    println!("{}", format!("Started in {:.1}s", build_time.as_secs_f64()).green());
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
