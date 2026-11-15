# Phase 0 — Dependency Reconcile + Board Skeleton — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.
> Spec: `docs/plans/2026-11-15-v2-migration-design.md`. This is **Phase 0 of 6**; later phases get their own plans.

**Goal:** Stand up the workspace on current Embassy with `soul-audio-core` available as a dependency, and a fresh `board-h7a3` binary that cross-compiles for the STM32H7A3 — without disturbing the salvaged eink/UI crates.

**Architecture:** Quarantine the dead Embassy-0.1 crates (`firmware`, `platform`, `playback`, `library`, `bluetooth`) out of the workspace build; bump the workspace Embassy deps to 0.6/0.10/0.5; add `soul-audio-core` (git-pinned + local path override); create a minimal `crates/board-h7a3` binary on the new Embassy that boots and heartbeats. The salvage crates (`eink/*`, `ui`, `firmware-ui`) are Embassy-free (except `eink-emulator`, host-only) and keep building throughout.

**Tech Stack:** Rust (resolver 2), Embassy 0.6 (`embassy-stm32` `stm32h7a3zi`), `thumbv7em-none-eabihf`, `flip-link`, `probe-rs`, `defmt`, `soul-audio-core` (path/git dep).

## Global Constraints

- Target triple: **`thumbv7em-none-eabihf`** (H7A3 is Cortex-M7, same as H743).
- Embassy versions (workspace): **`embassy-executor 0.10`, `embassy-time 0.5`, `embassy-stm32 0.6`, `embassy-sync 0.6`, `embassy-futures 0.1`**.
- `soul-audio-core` consumed by **git-pinned `rev` + local `path` override** (`../soul-audio-core/crates/*`); **MUST** add `[profile.dev.package.soul-audio-core] opt-level = 3` and the matching `[profile.test...]` or debug decode runs slower than real time.
- `panic = "abort"` on all embedded profiles (already set).
- Do **not** modify `soul-audio-core` — consume only.
- All commits: author/committer `sebastianstupak <sebastian.stupak@pm.me>`, **no AI trailers**.
- Work on branch **`migration/phase-0`**; open a PR into `main` (protected: linear history, no force-push).

---

### Task 1: Migration branch + quarantine the dead Embassy-0.1 crates

**Files:**
- Modify: `Cargo.toml` (workspace `members`)

**Interfaces:**
- Produces: a workspace whose default members are only the salvage crates + `xtask` (no Embassy-0.1 code), so the 0.6 bump in Task 2 can't be broken by dead code.

- [ ] **Step 1: Branch**

```bash
cd D:/dev/soulaudio/soul-listener-gh
git switch -c migration/phase-0
```

- [ ] **Step 2: Comment the dead/superseded crates out of `members`**

In `Cargo.toml`, edit the `[workspace] members` list so these are commented (keep the directories on disk — deleted for real in Phase 1/3, kept as salvage reference until then):

```toml
members = [
    "crates/eink/eink-specs",
    "crates/eink/eink-emulator",
    "crates/eink/eink-system",
    "crates/eink/eink-components",
    "crates/eink/eink-testing",
    "crates/firmware-ui",
    "crates/ui",
    "xtask",
    # --- quarantined for the soul-audio-core migration (Embassy 0.1, retargeted/deleted in later phases) ---
    # "crates/firmware",     # dead: H743 board, rewritten as board-h7a3 (Phase 3)
    # "crates/platform",     # trait shapes lifted to a ports crate (Phase 2/3); Embassy-0.1 wrapper dropped
    # "crates/playback",     # superseded by soul-audio-core (Phase 1)
    # "crates/library",      # superseded by soul-audio-core catalog/.soulidx (Phase 1)
    # "crates/bluetooth",    # deferred (reference only)
]
```

- [ ] **Step 3: Verify the reduced workspace resolves**

Run: `cargo metadata --no-deps --format-version 1 | python -c "import sys,json; print('\n'.join(sorted(p['name'] for p in json.load(sys.stdin)['packages'])))"`
Expected: lists `eink-*`, `firmware-ui`, `ui`, `xtask` — and **none** of `firmware`, `platform`, `playback`, `library`, `bluetooth`.

- [ ] **Step 4: Verify salvage crates still build on host**

Run: `cargo build`
Expected: builds. **If `firmware-ui` fails** with an unresolved `playback`/`library` import, comment `crates/firmware-ui` out of `members` too (it is rewired to `soul-audio-core` catalog types in Phase 1) and re-run.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml
git commit -m "chore(ws): quarantine Embassy-0.1 crates from the workspace build"
```

---

### Task 2: Bump the workspace Embassy to 0.6/0.10/0.5

**Files:**
- Modify: `Cargo.toml` (`[workspace.dependencies]`)

**Interfaces:**
- Produces: workspace Embassy deps that `board-h7a3` (Task 4) consumes via `*.workspace = true`.

- [ ] **Step 1: Replace the pinned Embassy versions**

In `[workspace.dependencies]`, set:

```toml
embassy-executor = { version = "0.10", features = ["arch-cortex-m", "executor-thread"] }
embassy-time     = { version = "0.5" }
embassy-sync     = "0.6"
embassy-futures  = "0.1"
embassy-stm32    = { version = "0.6", features = ["stm32h7a3zi", "memory-x", "time-driver-any", "exti", "unstable-pac"] }
```

(The old `=0.6.3`/`=0.3.2`/`=0.1.0` pins and the `embassy-embedded-hal =0.1.0` pin are removed; re-add `embassy-embedded-hal` at its 0.6-era version only when a board adapter needs it.)

- [ ] **Step 2: Verify nothing in the (reduced) workspace references the old Embassy**

Run: `cargo tree -i embassy-stm32 2>&1 | head` and `cargo build`
Expected: build succeeds (the salvage crates don't pull `embassy-stm32`; `eink-emulator` uses `embassy-time` with `std` on host, which resolves at 0.5).
If `eink-emulator` fails on an `embassy-time` API change, fix the call site (host-only, small surface).

- [ ] **Step 3: Commit**

```bash
git add Cargo.toml crates/eink/eink-emulator
git commit -m "chore(deps): bump workspace Embassy to 0.6/0.10/0.5"
```

---

### Task 3: Add `soul-audio-core` as a dependency

**Files:**
- Modify: `Cargo.toml` (`[workspace.dependencies]`, `[profile.*]`)

**Interfaces:**
- Produces: `soul-audio-core`, `soul-audio-engine`, `soul-audio-control`, `soul-audio-queue`, `soul-audio-catalog`, `soul-audio-index` available to `board-h7a3`/`dap` in later phases.

- [ ] **Step 1: Add the deps (git-pinned + local path override)**

Resolve the pinned rev first: `git -C ../soul-audio-core rev-parse HEAD`. Then in `[workspace.dependencies]`:

```toml
soul-audio-core    = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<REV>", default-features = false }
soul-audio-engine  = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<REV>" }
soul-audio-catalog = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<REV>", default-features = false }
soul-audio-index   = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<REV>" }
```

And a local override for co-development at the bottom of `Cargo.toml`:

```toml
[patch."https://github.com/soulaudio/soul-audio-core"]
soul-audio-core    = { path = "../soul-audio-core/crates/soul-audio-core" }
soul-audio-engine  = { path = "../soul-audio-core/crates/soul-audio-engine" }
soul-audio-catalog = { path = "../soul-audio-core/crates/soul-audio-catalog" }
soul-audio-index   = { path = "../soul-audio-core/crates/soul-audio-index" }
```

- [ ] **Step 2: Add the mandatory profile override**

```toml
[profile.dev.package.soul-audio-core]
opt-level = 3
[profile.test.package.soul-audio-core]
opt-level = 3
```

- [ ] **Step 3: Verify resolution**

Run: `cargo tree -p soul-audio-core 2>&1 | head -5`
Expected: resolves to the local path (the `[patch]` wins), no error.

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml
git commit -m "feat(deps): add soul-audio-core (git-pinned + local path override) + profile override"
```

---

### Task 4: `crates/board-h7a3` skeleton that cross-compiles for H7A3

**Files:**
- Create: `crates/board-h7a3/Cargo.toml`, `crates/board-h7a3/src/main.rs`
- Modify: `Cargo.toml` (`members`), `.cargo/config.toml` (runner chip)

**Interfaces:**
- Produces: binary `board-h7a3` — the composition root that later phases fill with adapters.

- [ ] **Step 1: Create `crates/board-h7a3/Cargo.toml`**

```toml
[package]
name = "board-h7a3"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "board-h7a3"
test = false
bench = false

[dependencies]
embassy-executor = { workspace = true }
embassy-time     = { workspace = true }
embassy-stm32    = { workspace = true }
cortex-m-rt = "0.7"
defmt = "1"
defmt-rtt = "1"
panic-probe = { version = "1", features = ["print-defmt"] }
```

- [ ] **Step 2: Create `crates/board-h7a3/src/main.rs` (minimal boot + heartbeat)**

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

In `Cargo.toml`, add `"crates/board-h7a3",` to `members`.

- [ ] **Step 4: Point the probe-rs runner at the H7A3**

In `.cargo/config.toml`, change the runner line under `[target.thumbv7em-none-eabihf]`:

```toml
runner = "probe-rs run --chip STM32H7A3ZITxQ"
```

- [ ] **Step 5: Cross-compile for the target**

Run: `cargo build -p board-h7a3 --target thumbv7em-none-eabihf`
Expected: builds (links with `flip-link` + `memory-x`-generated layout for the H7A3). If `embassy-stm32` rejects a feature name, run `cargo add -p board-h7a3 embassy-stm32@0.6 --dry-run` to confirm the exact H7A3 feature/time-driver names and adjust `[workspace.dependencies]`.

- [ ] **Step 6: Commit**

```bash
git add crates/board-h7a3 Cargo.toml .cargo/config.toml
git commit -m "feat(board-h7a3): Embassy 0.6 boot+heartbeat skeleton (thumbv7em, STM32H7A3)"
```

---

### Task 5: Green gates — host tests + target cross-build

**Files:**
- Modify: `.github/workflows/*.yml` (add/adjust the build matrix) — inspect first; the repo already has CI.

**Interfaces:**
- Produces: CI that proves (a) salvage crates test on host, (b) `board-h7a3` cross-builds for `thumbv7em`.

- [ ] **Step 1: Host test the salvage crates**

Run: `cargo test -p ui -p eink-system -p eink-components -p eink-specs`
Expected: PASS (these carried their own tests from v1).

- [ ] **Step 2: Confirm the two gate commands both pass locally**

Run: `cargo test` then `cargo build -p board-h7a3 --target thumbv7em-none-eabihf`
Expected: both succeed.

- [ ] **Step 3: Wire the target cross-build into CI**

Inspect `.github/workflows/` and add a job step `cargo build -p board-h7a3 --target thumbv7em-none-eabihf` (with `rustup target add thumbv7em-none-eabihf` + `cargo install flip-link`) alongside the existing host-test job. Keep the existing jobs.

- [ ] **Step 4: Commit + open PR**

```bash
git add .github
git commit -m "ci: gate host tests + board-h7a3 thumbv7em cross-build"
git push -u origin migration/phase-0
gh pr create --fill --base main --title "Phase 0: dependency reconcile + board-h7a3 skeleton"
```

---

## Deliverable of Phase 0

A workspace that: builds the salvage crates + a fresh `board-h7a3` on current Embassy; has `soul-audio-core` wired and resolvable; cross-compiles `board-h7a3` for the STM32H7A3; and keeps host tests green — with the dead Embassy-0.1 crates quarantined (not yet deleted). **Phase 1** then rips out `playback`/`library` and routes decode → `Deck` → a SAI/I²S `Sink` through `board-h7a3`.

## Out of scope (later phases, own plans)

- Phase 1: audio rip-and-replace (soul-audio-core `Deck`/`Engine`, SAI/I²S `Sink`, SD `Source`, rewire `firmware-ui`).
- Phase 2: eink 4-gray + H7A3 SPI pins + dirty-rect; lift `platform` trait shapes → ports.
- Phase 3: board adapters (CS43131, MT6701, BQ21080/MAX17048).
- Phase 4: `dap` app + wheel browse UI.
- Phase 5: BOM/SBOM + FreeCAD mechanical consolidation.
