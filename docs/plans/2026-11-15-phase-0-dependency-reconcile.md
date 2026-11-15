# Phase 0 — Dependency Reconcile + Board Skeleton — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development or superpowers:executing-plans. Steps use checkbox (`- [ ]`) syntax.
> Spec: `docs/plans/2026-11-15-v2-migration-design.md`. **Phase 0 of 6.**

**Goal:** Make `soul-audio-core` available to the workspace and add a fresh `board-h7a3` binary that boots on Embassy 0.6 and cross-compiles for the STM32H7A3 — **without touching the Embassy-0.1 crates** (`firmware`, `platform`, `playback`, `library`, `bluetooth`, `eink-emulator`).

**Architecture:** Do **not** bump the workspace Embassy — the old crates (and the salvaged `eink-emulator`, which depends on `platform`) stay on Embassy 0.1 until they are deleted/rewritten in later phases. `board-h7a3` declares its **own** Embassy 0.6 deps, gated behind a `hardware` feature (binary `required-features = ["hardware"]`), exactly like the existing `firmware` crate — so the two Embassy major versions coexist as separate binaries. `soul-audio-core` is added as a resolvable dependency (nothing links it yet; Phase 1 does).

**Tech Stack:** Rust (resolver 2), Embassy 0.6 (`embassy-stm32` `stm32h7a3zi`) for `board-h7a3` only, `thumbv7em-none-eabihf`, `flip-link`, `probe-rs`, `defmt`, `soul-audio-core` (path/git dep).

## Global Constraints

- **Do NOT modify** `Cargo.toml`'s `[workspace.dependencies]` Embassy pins, and do NOT edit any of `crates/{firmware,platform,playback,library,bluetooth}` or `crates/eink/eink-emulator`. They stay on Embassy 0.1.
- `board-h7a3` Embassy deps live in **its own** `Cargo.toml** (not `workspace = true`): `embassy-executor 0.10`, `embassy-time 0.5`, `embassy-stm32 0.6` (`stm32h7a3zi`), all `optional`, enabled by a `hardware` feature; the `[[bin]]` sets `required-features = ["hardware"]` (so host builds skip it). Mirror `crates/firmware/Cargo.toml`'s feature/target structure.
- Build the firmware binary only with `--target thumbv7em-none-eabihf --features hardware`.
- `soul-audio-core`: **git-pinned `rev` + local `path` override** to `../soul-audio-core/crates/*`; add `[profile.dev.package.soul-audio-core] opt-level = 3` and the `[profile.test...]` twin (required, or debug decode runs slower than real time).
- Target triple `thumbv7em-none-eabihf` (H7A3 is Cortex-M7).
- All commits: `sebastianstupak <sebastian.stupak@pm.me>`, **no AI trailers**.
- Branch **`migration/phase-0`** (already created by the controller — `git switch` to it, do not `-c`); PR into protected `main`.

---

### Task 1: Add `soul-audio-core` as a resolvable dependency

**Files:**
- Modify: `Cargo.toml` (`[workspace.dependencies]`, a `[patch]` block, `[profile.*]`)

**Interfaces:**
- Produces: `soul-audio-core`, `soul-audio-engine`, `soul-audio-catalog`, `soul-audio-index` resolvable for Phase 1 (nothing depends on them yet).

- [ ] **Step 1: Resolve the pin rev**

Run: `git -C ../soul-audio-core rev-parse HEAD` → record as `<REV>`.

- [ ] **Step 2: Add the deps to `[workspace.dependencies]`**

```toml
soul-audio-core    = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<REV>", default-features = false }
soul-audio-engine  = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<REV>" }
soul-audio-catalog = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<REV>", default-features = false }
soul-audio-index   = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<REV>" }
```

- [ ] **Step 3: Add the local path override + profile override (bottom of `Cargo.toml`)**

```toml
[patch."https://github.com/soulaudio/soul-audio-core"]
soul-audio-core    = { path = "../soul-audio-core/crates/soul-audio-core" }
soul-audio-engine  = { path = "../soul-audio-core/crates/soul-audio-engine" }
soul-audio-catalog = { path = "../soul-audio-core/crates/soul-audio-catalog" }
soul-audio-index   = { path = "../soul-audio-core/crates/soul-audio-index" }

[profile.dev.package.soul-audio-core]
opt-level = 3
[profile.test.package.soul-audio-core]
opt-level = 3
```

- [ ] **Step 4: Verify resolution (no build, path override wins)**

Run: `cargo tree -p soul-audio-core 2>&1 | head -3`
Expected: resolves to `soul-audio-core v… (D:\dev\soulaudio\soul-audio-core\crates\soul-audio-core)` — no error.

- [ ] **Step 5: Verify the salvage crates still check (nothing regressed)**

Run: `cargo check -p ui -p eink-specs -p eink-system -p eink-components`
Expected: all succeed.

- [ ] **Step 6: Commit**

```bash
git add Cargo.toml Cargo.lock
git commit -m "feat(deps): add soul-audio-core (git-pinned + local path override) + profile override"
```

---

### Task 2: `crates/board-h7a3` — Embassy 0.6 boot skeleton for the STM32H7A3

**Files:**
- Create: `crates/board-h7a3/Cargo.toml`, `crates/board-h7a3/src/main.rs`
- Modify: `Cargo.toml` (`members`), `.cargo/config.toml` (runner chip)
- Reference (read, do not edit): `crates/firmware/Cargo.toml` (mirror its feature/target gating)

**Interfaces:**
- Produces: binary `board-h7a3` (the composition root later phases fill with adapters).

- [ ] **Step 1: Create `crates/board-h7a3/Cargo.toml`** (mirrors `firmware`'s optional-embassy + `hardware` gate)

```toml
[package]
name = "board-h7a3"
version = "0.1.0"
edition = "2021"

[features]
hardware = ["dep:embassy-executor", "dep:embassy-time", "dep:embassy-stm32",
            "dep:cortex-m-rt", "dep:defmt", "dep:defmt-rtt", "dep:panic-probe"]

[[bin]]
name = "board-h7a3"
required-features = ["hardware"]
test = false
bench = false

[dependencies]
embassy-executor = { version = "0.10", features = ["arch-cortex-m", "executor-thread"], optional = true }
embassy-time     = { version = "0.5", optional = true }
embassy-stm32    = { version = "0.6", features = ["stm32h7a3zi", "memory-x", "time-driver-any"], optional = true }
cortex-m-rt = { version = "0.7", optional = true }
defmt       = { version = "1", optional = true }
defmt-rtt   = { version = "1", optional = true }
panic-probe = { version = "1", features = ["print-defmt"], optional = true }
```

- [ ] **Step 2: Create `crates/board-h7a3/src/main.rs`**

```rust
#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let _p = embassy_stm32::init(Default::default());
    info!("board-h7a3: boot");
    loop {
        info!("heartbeat");
        Timer::after(Duration::from_secs(1)).await;
    }
}
```

- [ ] **Step 3: Add to workspace members**

In `Cargo.toml`, add `"crates/board-h7a3",` to `[workspace] members`.

- [ ] **Step 4: Point the probe-rs runner at the H7A3**

In `.cargo/config.toml`, under `[target.thumbv7em-none-eabihf]`, change the runner line to:

```toml
runner = "probe-rs run --chip STM32H7A3ZITxQ"
```

- [ ] **Step 5: Host build skips it (sanity)**

Run: `cargo build -p board-h7a3`
Expected: succeeds and builds nothing (bin skipped — `hardware` not set). No `cortex-m-rt`/Embassy compiled for host.

- [ ] **Step 6: Cross-compile for the target**

Run: `cargo build -p board-h7a3 --target thumbv7em-none-eabihf --features hardware`
Expected: builds (links via `flip-link` + the `memory-x`-generated H7A3 layout). If `embassy-stm32` rejects `stm32h7a3zi` or `time-driver-any`, run `cargo add -p board-h7a3 embassy-stm32@0.6 --dry-run` to confirm exact feature names and adjust.

- [ ] **Step 7: Commit**

```bash
git add crates/board-h7a3 Cargo.toml Cargo.lock .cargo/config.toml
git commit -m "feat(board-h7a3): Embassy 0.6 boot+heartbeat skeleton (thumbv7em, STM32H7A3)"
```

---

### Task 3: Green gates — host tests + target cross-build in CI

**Files:**
- Modify: `.github/workflows/*.yml` (inspect first; the repo already has CI)

**Interfaces:**
- Produces: CI proving salvage crates test on host and `board-h7a3` cross-builds.

- [ ] **Step 1: Host-test the salvage crates**

Run: `cargo test -p ui -p eink-system -p eink-components -p eink-specs`
Expected: PASS (these carry their own tests from v1).

- [ ] **Step 2: Confirm both gate commands pass locally**

Run: `cargo test -p ui -p eink-system -p eink-components -p eink-specs` then `cargo build -p board-h7a3 --target thumbv7em-none-eabihf --features hardware`
Expected: both succeed.

- [ ] **Step 3: Add the target cross-build to CI**

Inspect `.github/workflows/`. Add — next to the existing host job — a step:
`rustup target add thumbv7em-none-eabihf && cargo install flip-link && cargo build -p board-h7a3 --target thumbv7em-none-eabihf --features hardware`.
Keep every existing job/step intact.

- [ ] **Step 4: Commit + open PR**

```bash
git add .github
git commit -m "ci: gate board-h7a3 thumbv7em cross-build alongside host tests"
git push -u origin migration/phase-0
gh pr create --fill --base main --title "Phase 0: dependency reconcile + board-h7a3 skeleton"
```

---

## Deliverable

`soul-audio-core` resolvable; a fresh `board-h7a3` that cross-compiles for the STM32H7A3 on Embassy 0.6; salvage crates green; **old Embassy-0.1 crates untouched**. Phase 1 then rips out `playback`/`library` and routes decode → `Deck` → a SAI/I²S `Sink` through `board-h7a3`.

## Out of scope (later phases)

Phase 1 audio rip-and-replace · Phase 2 eink 4-gray + ports lift · Phase 3 board adapters (CS43131/MT6701/BQ21080) · Phase 4 `dap` app + wheel UI · Phase 5 BOM/SBOM + mechanical consolidation.
