// Desktop/tooling crate — unwrap/expect/panic acceptable in non-embedded code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// TODO: Add rustdoc to all public items (tracked as tech debt)
#![allow(missing_docs)]

mod check;
mod dev;
mod doc;
mod flash;
mod hardware;
mod scan_library;
mod test;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "xtask")]
#[command(about = "SoulAudio DAP development tasks", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Flash firmware to STM32H7 hardware via probe-rs
    Flash {
        /// Build and flash release version
        #[arg(short, long)]
        release: bool,
    },
    /// Run emulator with hot-reload development mode
    Dev {
        /// Run in headless mode (no window)
        #[arg(long)]
        headless: bool,
        /// Enable true in-process hot-reload via firmware-ui dylib.
        /// Requires: cargo build --package firmware-ui --features hot-reload
        #[arg(long)]
        hot_reload: bool,
        /// Local music directory — passed as MUSIC_PATH env var to the emulator.
        /// The emulator's LocalFileStorage reads this to locate Soul library files.
        #[arg(long)]
        music_path: Option<std::path::PathBuf>,
    },
    /// Check firmware builds for both hardware and emulator targets
    Check,
    /// Run all tests (unit, integration, and hardware)
    Test {
        /// Run only unit tests
        #[arg(long)]
        unit: bool,
        /// Run only integration tests
        #[arg(long)]
        integration: bool,
    },
    /// Build and optionally open documentation
    Doc {
        /// Open documentation in browser
        #[arg(long)]
        open: bool,
    },
    /// Hardware design tools (Atopile): check, build, sync, bom
    /// Alias: `cargo hw <subcommand>`
    Hardware {
        #[command(subcommand)]
        command: hardware::HwCommand,
    },
    /// Scan a local music folder and write Soul binary library files
    ScanLibrary {
        /// Directory containing music files (Artist/Album/track structure)
        #[arg(long)]
        music_dir: std::path::PathBuf,
        /// Output directory for binary library files
        #[arg(long)]
        soul_root: std::path::PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Flash { release } => flash::run(release),
        Commands::Dev {
            headless,
            hot_reload,
            music_path,
        } => dev::run(headless, hot_reload, music_path.as_deref()),
        Commands::Check => check::run(),
        Commands::Test { unit, integration } => test::run(unit, integration),
        Commands::Doc { open } => doc::run(open),
        Commands::Hardware { command } => hardware::run(command),
        Commands::ScanLibrary { music_dir, soul_root } => {
            scan_library::run(&music_dir, &soul_root)
        }
    }
}
