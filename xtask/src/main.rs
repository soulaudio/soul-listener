mod check;
mod dev;
mod doc;
mod flash;
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
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Flash { release } => flash::run(release),
        Commands::Dev {
            headless,
            hot_reload,
        } => dev::run(headless, hot_reload),
        Commands::Check => check::run(),
        Commands::Test { unit, integration } => test::run(unit, integration),
        Commands::Doc { open } => doc::run(open),
    }
}
