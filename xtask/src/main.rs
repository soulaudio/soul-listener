// Desktop/tooling crate — unwrap/expect/panic acceptable in non-embedded code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
// TODO: Add rustdoc to all public items (tracked as tech debt)
#![allow(missing_docs)]

// TODO(scan-library replacement): Phase 1d removed xtask's old `scan-library`
// subcommand (it baked v1's own library format). The replacement lives in the
// sibling repo `../soul-audio-core/crates/soul-audio-import`, which exposes
// `pub fn export(root: &Path, out_dir: &Path) -> std::io::Result<()>` — scan
// `root` and write `out_dir/.soulidx/{index.sidx,art.blob}`. That part is a
// trivial one-call wire-up for a `bake-index <dir> <out>` subcommand here.
// NOT wired in yet because `soul-audio-import` transitively depends on
// `soul-audio-tags`, which declares `edition = "2024"` (needs rustc >= 1.85).
// This workspace's declared MSRV is 1.75 (`rust-version` in the root
// Cargo.toml), enforced by the `msrv` CI job pinned to that exact toolchain
// (.github/workflows/ci.yml) — adding the dependency today would silently
// break that job. Wire this in once the MSRV policy is revisited (bump
// `rust-version`, or drop the `msrv` job if the dap/ pivot has made it moot —
// dap/ already needs stable/1.88+ for Embassy 0.6's let-chains).

mod check;
mod commit_msg;
mod dev;
mod doc;
mod flash;
mod gate;
mod hardware;
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
    /// Fast pre-commit gate (lefthook `pre-commit`): fmt-check both
    /// workspaces, clippy only the active dap/ workspace.
    Precommit,
    /// Full pre-push / CI gate (lefthook `pre-push`): precommit, plus the
    /// dap/ board-h7a3 thumbv7em clippy + build, and `cargo test` in dap/.
    Verify,
    /// Validate a commit-message file against the Conventional Commits +
    /// no-AI-attribution policy (lefthook `commit-msg`).
    CheckCommitMsg {
        /// Path to the commit-message file (lefthook passes this as `{1}`).
        path: String,
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
        Commands::Precommit => gate::precommit(),
        Commands::Verify => gate::verify(),
        Commands::CheckCommitMsg { path } => commit_msg::check(&path),
    }
}
