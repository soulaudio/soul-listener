//! Format/lint/test gating for lefthook (`pre-commit` / `pre-push`) and CI.
//!
//! soul-listener has three workspaces (see the root `Cargo.toml`'s
//! `workspace.exclude` comment for why they can't share one `Cargo.lock`):
//!
//! - the ROOT workspace (`crates/*`): v1 Embassy-0.1 salvage code. It is
//!   currently bit-rotted — its `clippy`/`test` are red — so it is only ever
//!   `fmt --check`'d here. Gating it with clippy/test would block every
//!   commit on pre-existing breakage unrelated to whatever change is being
//!   made; that cleanup is tracked separately.
//! - the `dap/` workspace (`dap/core` = dap-core, `dap/sim` = dap-sim,
//!   `dap/board-h7a3`): the active code. It is fully gated here: fmt,
//!   clippy (host + the `board-h7a3` thumbv7em cross-build), and
//!   `cargo test`.
//! - the `eink-ui/` workspace: the standalone, publishable e-ink UI toolkit
//!   (see docs/plans/2026-11-16-eink-ui-toolkit-design.md). It is fully
//!   gated here too: fmt, clippy (host, `--all-features`), and
//!   `cargo test --features std` (the golden-test harness is std-gated).
//!
//! `precommit` is the fast pre-commit hook; `verify` is the fuller pre-push
//! gate (also what CI's authoritative jobs run). Pure Rust (no shell script)
//! so both behave identically on Windows, macOS, and Linux.

use anyhow::{bail, Context, Result};
use std::process::Command;

/// Run `cargo <args>` with the given working directory, streaming
/// stdout/stderr directly (not captured) so output appears live.
fn run_in(dir: &str, args: &[&str]) -> Result<()> {
    println!("+ (cd {dir} && cargo {})", args.join(" "));
    let status = Command::new("cargo")
        .args(args)
        .current_dir(dir)
        .status()
        .with_context(|| format!("spawning `cargo {}` in {dir}", args.join(" ")))?;
    if !status.success() {
        bail!("`cargo {}` failed in {dir} ({status})", args.join(" "));
    }
    Ok(())
}

/// `cargo xtask precommit` — the fast pre-commit hook.
///
/// fmt-checks ALL THREE workspaces (cheap and tree-wide), then clippies the
/// active `dap/` workspace and the standalone `eink-ui/` workspace. The
/// root workspace's clippy/tests stay red (tracked separately) and are
/// deliberately never run here — see the module doc comment.
pub fn precommit() -> Result<()> {
    run_in(".", &["fmt", "--all", "--", "--check"])?;
    run_in("dap", &["fmt", "--all", "--", "--check"])?;
    run_in(
        "dap",
        &[
            "clippy",
            "--workspace",
            "--all-targets",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run_in("eink-ui", &["fmt", "--all", "--", "--check"])?;
    run_in(
        "eink-ui",
        &[
            "clippy",
            "--all-targets",
            "--all-features",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    println!("precommit: OK");
    Ok(())
}

/// `cargo xtask verify` — the pre-push / CI gate.
///
/// Everything `precommit` does, plus the thumbv7em cross-checks for
/// `board-h7a3` (clippy + build) and the dap host test suite, and the
/// `eink-ui/` crate's own no_std/no-alloc host build, its thumbv7em
/// cross-build, and its (std-gated) golden-test suite. flip-link and the
/// thumbv7em target are assumed installed — true in CI (see the
/// `fmt-dap`/`clippy-dap`/`test-dap`/`build-board-h7a3` jobs in
/// `.github/workflows/ci.yml`) and for any contributor who has run the
/// hardware setup described in `dap/board-h7a3`'s README.
pub fn verify() -> Result<()> {
    precommit()?;
    run_in(
        "dap",
        &[
            "clippy",
            "-p",
            "board-h7a3",
            "--target",
            "thumbv7em-none-eabihf",
            "--features",
            "hardware",
            "--",
            "-D",
            "warnings",
        ],
    )?;
    run_in("dap", &["test"])?;
    run_in(
        "dap",
        &[
            "build",
            "-p",
            "board-h7a3",
            "--target",
            "thumbv7em-none-eabihf",
            "--features",
            "hardware",
        ],
    )?;
    run_in("eink-ui", &["build"])?;
    run_in("eink-ui", &["build", "--target", "thumbv7em-none-eabihf"])?;
    run_in("eink-ui", &["test", "--features", "std"])?;
    println!("verify: OK");
    Ok(())
}
