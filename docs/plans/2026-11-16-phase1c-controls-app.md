# Phase 1c — Controls port + App coordinator (headless) — Implementation Plan

> REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`.
> Spec: `docs/plans/2026-11-16-logic-and-headless-simulation-design.md` §2,§5 (the `app` coordinator + `SimControls`).

**Goal:** Add the `Controls` input port and the board-agnostic **`App` coordinator** that ties `AudioPlayer` + `PowerService` + `Controls` into one `tick()` loop, then prove the whole virtual DAP headless: scripted wheel/press input drives transport, while a scripted battery still pauses/stops playback — all on the host.

**Architecture:** `dap-core::ports::Controls` (input events) + `dap-core::app::App` (owns the player + power service, maps input → transport, applies power events, pumps audio). `dap-sim::SimControls` scripts input; the harness runs `App` with all sim adapters.

**Tech Stack:** Rust, `dap-core` (no_std, host+target), `dap-sim` (host). No embassy.

## Global Constraints

- `dap-core` stays `#![no_std]` + `#![forbid(unsafe_code)]`, host + `thumbv7em`, no `alloc`, no `dyn`. `App` is generic over `<S: Sink, St: Storage, P: Power, C: Controls, const PRELOAD: usize>`.
- The input→action mapping is **provisional** (no UI yet): `Rotate(delta)` → volume adjust, `Press` → play/pause toggle, `LongPress` → next. Mark it clearly as provisional (the real mapping arrives with the UI phase, where `Rotate` becomes list-scroll).
- Time passed in as `now_ms: u64`.
- Commits `sebastianstupak <sebastian.stupak@pm.me>`, no AI trailers. Branch `migration/phase-1c`.

---

### Task 1: `Controls` port + `App` coordinator in `dap-core`

**Files:** Create `dap/core/src/app.rs`; modify `dap/core/src/ports.rs`, `dap/core/src/lib.rs`.

**Interfaces — Produces:**
- `ports::Controls`: `trait Controls { fn poll(&mut self) -> Option<InputEvent>; }` with `enum InputEvent { Rotate(i8), Press, LongPress }`.
- `app::App<S: Sink, St: Storage, P: Power, C: Controls, const PRELOAD: usize>` — owns an `AudioPlayer<S, St, PRELOAD>`, a `PowerService`, the `P` and `C` adapters. Exposes `App::new(sink, storage, power, controls, config)`, `set_queue(len, start)`, and `tick(&mut self, now_ms: u64) -> TickOutcome` which: drains `Controls::poll` → maps to transport (provisional mapping above); calls `PowerService::poll` → on `LowBattery` pauses, on `CriticalShutdown` stops + (service already requested power-off); pumps the audio one block. `TickOutcome` reports what happened (events handled, playback progress) for tests/telemetry. Add accessors (`player()`, `power_soc()`, current volume/state).

- [ ] **Step 1 (TDD):** `#[cfg(test)]` tests in `app.rs` with a `MockControls` (scripted `InputEvent`s) + a `MockPower` (scripted readings) + a trivial in-memory `Sink`/`Storage` (or reuse minimal mocks — do NOT depend on `dap-sim`): assert `Press` toggles play/pause, `Rotate(+n)` raises volume (`Rotate(-n)` lowers), `LongPress` advances the track, and a critical battery reading drives a stop. Run → FAIL.
- [ ] **Step 2:** implement `App::tick` until tests pass. Keep mapping in one small `fn map_input`. Run → PASS, pristine.
- [ ] **Step 3 (verify):** `cd dap && cargo test -p dap-core`; `cargo build -p dap-core --target thumbv7em-none-eabihf`; `cargo clippy -p dap-core` → all clean.
- [ ] **Step 4:** Commit `feat(dap-core): Controls port + App coordinator (input->transport + power handling)`.

---

### Task 2: `SimControls` + the full virtual-DAP headless harness in `dap-sim`

**Files:** Create `dap/sim/src/controls.rs`, `dap/sim/tests/device_headless.rs`; modify `dap/sim/src/lib.rs`.

**Interfaces — Consumes:** `app::App`, `ports::Controls`, Phase-1a `SimSink`/`SimStorage`, Phase-1b `SimBattery`.

- [ ] **Step 1:** `SimControls` implements `ports::Controls` from a scripted timeline of `(at_ms, InputEvent)`; `poll()` returns events whose time has arrived.
- [ ] **Step 2 (TDD):** `tests/device_headless.rs` — the whole virtual DAP: build `App` with `SimSink` + `SimStorage` (a 2-track queue) + `SimBattery` + `SimControls`; run a `tick()` loop advancing `now_ms`. Script: `Press` to play → audio flows to `SimSink`; `Rotate(+)` → recorded level rises; `LongPress` → advances to track 2 (gaplessly); then discharge `SimBattery` to low → playback auto-pauses; to critical → stops + `request_power_off` recorded. Assert each. Run → FAIL first, implement `SimControls` + driver → PASS.
- [ ] **Step 3 (verify):** `cd dap && cargo test` (whole workspace — audio_headless + power_headless + device_headless all green). `cargo clippy -p dap-sim` clean.
- [ ] **Step 4:** Commit `feat(dap-sim): SimControls + full virtual-DAP headless harness`.

---

## Deliverable

The complete virtual DAP runs headless: scripted input drives transport while a scripted battery governs power behavior, all through the `App` coordinator with soul-audio-core doing the real decode/resample/gapless — zero hardware. Next and final Phase-1 step: 1d, rip out the v1 `playback`/`library` crates (now fully superseded).
