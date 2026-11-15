# Phase 1a — dap/ workspace + headless audio slice — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development or superpowers:executing-plans. Steps use `- [ ]`.
> Spec: `docs/plans/2026-11-16-logic-and-headless-simulation-design.md` (this is spec-phase 1: the headless audio slice; power/controls/rip-out are later plans).

**Goal:** Stand up the `dap/` workspace and prove a **gapless/resampled/volume-controlled queue plays end-to-end on the host with zero hardware** — decode (soul-audio-core) → `Deck`/`Engine` → an analysis `SimSink`, sourced from a file-backed `SimStorage`.

**Architecture:** A `dap/` workspace (Embassy-0.6 / soul-audio-core world, separate from the Embassy-0.1 root). `dap-core` holds the board-agnostic ports + audio assembly over soul-audio-core (no embassy, host-runnable). `dap-sim` holds host adapters (`SimStorage`, `SimSink`) + a `cargo test` harness. `board-h7a3` (moved here) stays the hardware bin. Logic depends only on ports; `SimSink` is the first real `Sink` and the swap-test for later SAI/USB sinks.

**Tech Stack:** Rust, soul-audio-core (path dep), host `std` for `dap-sim`, `thumbv7em` for `board-h7a3`.

## Global Constraints

- **Crate names:** `dap-core`, `dap-sim`, `board-h7a3` (hardware bin keeps its name). Workspace dir: top-level `dap/`.
- `dap-core` and `dap-sim` are **embassy-free**; `dap-core` is `no_std`-capable + host-runnable; `dap-sim` is host `std`.
- Logic depends **only on ports** — `dap-core` must not name a concrete sink/source/battery. `Sink`/`Source`/`SeekSource` come from `soul-audio-core`; `Storage`/`Power`/`Controls` are new `dap-core` traits (this plan only needs `Storage`; `Power`/`Controls` are later plans).
- soul-audio-core consumed by **path** dep `../../soul-audio-core/crates/*` (from `dap/<crate>/`), `default-features = false`, with `[profile.dev.package.soul-audio-core] opt-level = 3` + the `[profile.test...]` twin in the `dap/` workspace.
- Reuse real fixtures from `../../soul-audio-core/testdata/` — do not invent audio.
- All commits `sebastianstupak <sebastian.stupak@pm.me>`, **no AI trailers**. Branch `migration/phase-1`.
- The implementer must read soul-audio-core's `README.md` + relevant `crates/soul-audio-core/src` + `crates/soul-audio-engine/src` for the **exact** `Sink`/`Source`/`Decoder`/`Deck`/`Engine` signatures — do not guess them.

---

### Task 1: Scaffold the `dap/` workspace (move board-h7a3; add dap-core + dap-sim skeletons)

**Files:**
- Create: `dap/Cargo.toml` (workspace), `dap/core/Cargo.toml`, `dap/core/src/lib.rs`, `dap/sim/Cargo.toml`, `dap/sim/src/lib.rs`
- Move: `crates/board-h7a3/` → `dap/board-h7a3/` (git mv; fix its `soul-audio-core` path dep depth and `.cargo/config` stays)
- Modify: root `Cargo.toml` (`exclude` → `dap/board-h7a3` becomes `dap`), `.github/workflows/ci.yml` (board-h7a3 job `working-directory: dap/board-h7a3`)

**Interfaces:**
- Produces: a `dap/` workspace that builds `dap-core` + `dap-sim` on host and `board-h7a3` for `thumbv7em`.

- [ ] **Step 1:** `git switch migration/phase-1`. `git mv crates/board-h7a3 dap/board-h7a3`. Fix the `soul-audio-core` path in `dap/board-h7a3/Cargo.toml` (now `../../soul-audio-core/crates/soul-audio-core` — one level shallower than before; verify the depth from `dap/board-h7a3/`).
- [ ] **Step 2:** Create `dap/Cargo.toml` as the workspace root:
```toml
[workspace]
resolver = "2"
members = ["core", "sim", "board-h7a3"]

[workspace.dependencies]
soul-audio-core   = { path = "../soul-audio-core/crates/soul-audio-core", default-features = false }
soul-audio-engine = { path = "../soul-audio-core/crates/soul-audio-engine" }

[profile.dev.package.soul-audio-core]
opt-level = 3
[profile.test.package.soul-audio-core]
opt-level = 3
```
  Remove the `[workspace]` table from `dap/board-h7a3/Cargo.toml` (it is now a member, not its own root) and switch its `soul-audio-core` dep to `{ workspace = true, optional = true }`.
- [ ] **Step 3:** Create `dap/core/Cargo.toml` (`name = "dap-core"`, `soul-audio-core = { workspace = true }`, `no_std`-capable — no `std`-only deps) and `dap/core/src/lib.rs` = `#![no_std] #![forbid(unsafe_code)]` + `pub mod ports;` + an empty `pub mod ports {}` for now.
- [ ] **Step 4:** Create `dap/sim/Cargo.toml` (`name = "dap-sim"`, deps: `dap-core = { path = "../core" }`, `soul-audio-core = { workspace = true }`, host `std` ok) and `dap/sim/src/lib.rs` = `//! host simulation adapters`.
- [ ] **Step 5:** Update the root `Cargo.toml`: change `exclude = ["crates/board-h7a3"]` → `exclude = ["dap"]`. Update `.github/workflows/ci.yml`'s board-h7a3 job `working-directory: crates/board-h7a3` → `dap/board-h7a3`.
- [ ] **Step 6 (verify):**
  - `cd dap && cargo build -p dap-core -p dap-sim` → host build OK.
  - `cd dap && cargo build -p board-h7a3 --target thumbv7em-none-eabihf --features hardware` → still links (its own `.cargo/config` moved with it).
  - `cd <repo root> && cargo build -p ui` → the old root workspace still builds (unaffected).
- [ ] **Step 7:** Commit `chore(dap): scaffold dap/ workspace (move board-h7a3; add dap-core + dap-sim)`.

---

### Task 2: `dap-core` ports + audio assembly over soul-audio-core

**Files:**
- Create: `dap/core/src/ports.rs` (the `Storage` trait + re-exports), `dap/core/src/audio.rs` (the assembly)
- Modify: `dap/core/src/lib.rs`

**Interfaces:**
- Consumes: soul-audio-core's `Sink`, `Source`/`SeekSource`, `Decoder`, `open`, `Deck`, and `soul-audio-engine`'s `Engine` (read their source for exact signatures).
- Produces: `dap_core::ports::{Sink, Source, SeekSource, Storage}` and `dap_core::audio::AudioPlayer<…>` — a generic player driving `Engine`+`Deck` over a caller-supplied `Sink` and `Storage`, with `play/pause/next/prev/set_volume/pump` style transport. Later plans (power/controls) build on this.

- [ ] **Step 1:** In `ports.rs`, re-export soul-audio-core's `Sink`, `Source`, `SeekSource`, and define a **`Storage`** port — a trait that, given a track's located coordinates (folder/file from the `.soulidx`, as `board-h7a3`'s existing `playback::Located` does), yields a `SeekSource` for decoding. Keep it generic/object-free. (Read `soul-audio-core`'s source ladder + the existing `crates/firmware`/`board-h7a3` SD source for the shape.)
- [ ] **Step 2:** In `audio.rs`, build `AudioPlayer` generic over `<S: Sink, St: Storage>` that composes `soul-audio-engine::Engine` + `soul-audio-core::Deck` — opening decoders through `Storage`, writing to `S`, exposing transport + a `pump()` that advances one block (gapless handled by `Deck`/`Engine`). No decoder/resampler/queue logic is written here — it is pure assembly. Match the real `Deck`/`Engine` API (the engine emits `poll_open()`/`provide()`; opening happens via `Storage`).
- [ ] **Step 3 (verify):** `cd dap && cargo build -p dap-core` and `cargo build -p dap-core --target thumbv7em-none-eabihf` (prove it stays no_std/target-capable). Both succeed.
- [ ] **Step 4:** Commit `feat(dap-core): Storage port + AudioPlayer assembly over soul-audio-core`.

---

### Task 3: `dap-sim` SimStorage + SimSink + the headless audio harness

**Files:**
- Create: `dap/sim/src/sink.rs` (`SimSink`), `dap/sim/src/storage.rs` (`SimStorage`), `dap/sim/tests/audio_headless.rs`
- Modify: `dap/sim/src/lib.rs`

**Interfaces:**
- Consumes: `dap_core::{ports, audio::AudioPlayer}`, soul-audio-core fixtures under `../../soul-audio-core/testdata/`.
- Produces: `dap_sim::{SimSink, SimStorage}` host adapters + a `cargo test` proving headless gapless/resample/volume playback.

- [ ] **Step 1:** `SimSink` implements `soul_audio_core::Sink`: records every PCM block (total frames, channel count, rate, plus continuity bookkeeping so gaplessness is assertable) and optionally dumps a `.pcm` file. Never blocks indefinitely (host).
- [ ] **Step 2:** `SimStorage` implements `dap_core::ports::Storage` over real files from `soul-audio-core/testdata/` (and, if needed, a tiny baked `.soulidx` or a direct file list) so the actual decode path runs.
- [ ] **Step 3 (write the failing test):** `tests/audio_headless.rs` — construct `AudioPlayer` with `SimSink` + `SimStorage`, queue two real fixtures (e.g. a WAV then a FLAC at a different rate), `pump()` to completion, and assert: total frames ≈ expected, the resampled track lands at the sink's rate, **no gap/discontinuity at the track boundary**, and volume scaling changes the recorded level. Run: `cd dap && cargo test -p dap-sim --test audio_headless` → FAILS (not yet wired).
- [ ] **Step 4:** Implement until it passes. Run the same command → PASS, output pristine.
- [ ] **Step 5 (verify):** `cd dap && cargo test` (all dap crates) → green.
- [ ] **Step 6:** Commit `feat(dap-sim): SimSink + SimStorage + headless gapless/resample/volume test`.

---

## Deliverable

The whole audio path runs and is asserted **on the host with no hardware**: real files → soul-audio-core decode/resample/gapless → `SimSink`, driven through `dap-core::AudioPlayer`. The ports are in place; `SimSink` proves the swap-point for a future SAI/USB sink. Next plans: Power service + `SimBattery`; `SimControls` + transport; then rip out v1 `playback`/`library`.

## Out of scope (later)

Power/battery (spec §4) · Controls/input · ripping out v1 audio (spec §7) · UI · hardware adapters.
