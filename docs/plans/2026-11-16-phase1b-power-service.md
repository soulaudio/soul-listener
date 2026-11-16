# Phase 1b — Power service + SimBattery (headless) — Implementation Plan

> REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`.
> Spec: `docs/plans/2026-11-16-logic-and-headless-simulation-design.md` §4–5 (full power scope + SimBattery).

**Goal:** Build the board-agnostic battery/power service (full scope) behind a `Power` port, and prove it headless with a scriptable `SimBattery`: low-batt→auto-pause, critical→safe-shutdown, perf/battery-saver profiles, thermal-aware charging, sleep/idle, runtime estimation.

**Architecture:** `dap-core::ports::Power` (hardware abstraction: battery reads + actuator commands) + `dap-core::power::PowerService` (a pure state machine over readings → events + port commands). `dap-sim::SimBattery` drives a scriptable timeline; the harness wires `SimBattery` + `PowerService` + the Phase-1a `AudioPlayer` to assert cross-cutting behavior (low-batt pauses playback).

**Tech Stack:** Rust, `dap-core` (no_std, host+target), `dap-sim` (host std). No embassy.

## Global Constraints

- `dap-core` stays `#![no_std]` + `#![forbid(unsafe_code)]`, host + `thumbv7em` capable, no `alloc`, no `dyn`. `PowerService` is generic over `P: Power`, not boxed.
- Time is passed in as `now_ms: u64` (no `dap-core` clock dep — the caller/sim/board supplies time).
- No floating-point reliance that bloats the target unreasonably; integer math where practical (SoC in per-mille, mV, mA, centi-°C).
- Commits `sebastianstupak <sebastian.stupak@pm.me>`, no AI trailers. Branch `migration/phase-1b`.

---

### Task 1: `Power` port + `PowerService` state machine (full scope) in `dap-core`

**Files:** Create `dap/core/src/power.rs`; modify `dap/core/src/ports.rs`, `dap/core/src/lib.rs`.

**Interfaces — Produces:**
- `ports::Power`: reads + actuators, e.g.
  `trait Power { fn read(&mut self) -> BatteryReading; fn set_charge_current_ma(&mut self, ma: u16); fn set_clock_profile(&mut self, p: PowerProfile); fn request_power_off(&mut self); }`
  `struct BatteryReading { soc_permille: u16, voltage_mv: u16, current_ma: i16, temp_centi_c: i16, charger_present: bool, charging: bool }`
- `power::{PowerService, PowerConfig, PowerProfile, PowerEvent, ChargeState}`.
  `PowerEvent` covers: `LowBattery`, `CriticalShutdown`, `ChargeStarted`, `ChargeComplete`, `ProfileChanged(PowerProfile)`, `ThermalThrottle{charge_ma}`, `EnterSleep`, `Wake`.
  `PowerService::poll<P: Power>(&mut self, power: &mut P, now_ms: u64, user_active: bool) -> heapless::Vec<PowerEvent, N>` (or an iterator/Option — a bounded, no-alloc event set).
  Queries: `soc_permille()`, `charge_state()`, `time_to_empty_s()`/`time_to_full_s()` (runtime estimation), `profile()`, `cycle_count()`.

- [ ] **Step 1 (TDD):** write `dap/core/src/power.rs` unit tests (a `MockPower` feeding scripted `BatteryReading`s) asserting each behavior: smoothed SoC (EMA, no jitter); discharge→`LowBattery` once at the warn threshold (not repeatedly); →`CriticalShutdown` + `request_power_off` called at critical; charger plug→`ChargeStarted`, full→`ChargeComplete`; temp outside the safe band→`ThermalThrottle` with reduced `set_charge_current_ma`; `user_active=false` past idle-timeout→`EnterSleep`, activity→`Wake`; `BatterySaver` auto-engaged below a SoC threshold→`ProfileChanged` + `set_clock_profile`; `time_to_empty_s` reasonable from SoC + current draw; `cycle_count` increments per cumulative full-discharge-equivalent. Run them → FAIL (no impl).
- [ ] **Step 2:** implement `PowerService` until the unit tests pass. Keep it a deterministic state machine; thresholds come from `PowerConfig` (with sensible defaults). Run → PASS, pristine.
- [ ] **Step 3 (verify):** `cd dap && cargo build -p dap-core --target thumbv7em-none-eabihf` (stays target-capable) and `cargo test -p dap-core` (host) → both OK; `cargo clippy -p dap-core` clean.
- [ ] **Step 4:** Commit `feat(dap-core): Power port + PowerService state machine (SoC/charge/thermal/profiles/sleep/runtime)`.

---

### Task 2: `SimBattery` + the power↔playback headless harness in `dap-sim`

**Files:** Create `dap/sim/src/battery.rs`, `dap/sim/tests/power_headless.rs`; modify `dap/sim/src/lib.rs`.

**Interfaces — Consumes:** Task 1's `Power`/`PowerService`; Phase-1a `AudioPlayer` + `SimSink`/`SimStorage`.

- [ ] **Step 1:** `SimBattery` implements `ports::Power` driven by a scriptable timeline: a sequence of `(at_ms, soc_permille, temp_centi_c, charger_present)` keyframes it interpolates/steps through; it records actuator calls (charge-current changes, clock-profile changes, power-off requested) for assertions.
- [ ] **Step 2 (TDD):** `tests/power_headless.rs` runs scenarios on the host:
  - **discharge-to-critical while playing:** start an `AudioPlayer` queue playing; advance `SimBattery` from 100%→critical; a driver loop calls `PowerService::poll` and, on `LowBattery`, `player.pause()`, on `CriticalShutdown`, `player.stop()` + asserts `request_power_off` was recorded. Assert the player actually paused at the warn point and stopped at critical.
  - **overheat:** push temp out of band → assert `ThermalThrottle` + a reduced charge current recorded.
  - **low-SoC profile:** cross the saver threshold → assert `ProfileChanged(BatterySaver)` + `set_clock_profile` recorded.
  - **idle sleep:** `user_active=false` past timeout → `EnterSleep`.
  - **runtime estimate:** assert `time_to_empty_s` is finite and decreases as SoC drops.
  Run → FAIL first, then implement `SimBattery` + the test driver until PASS.
- [ ] **Step 3 (verify):** `cd dap && cargo test` (whole workspace, incl. Phase-1a `audio_headless` still green) → all pass.
- [ ] **Step 4:** Commit `feat(dap-sim): SimBattery + power↔playback headless harness`.

---

## Deliverable

The full power/battery logic runs and is asserted headless, including its cross-cutting effect on playback (low-batt pauses, critical stops+powers-off). Next: Phase 1c (`SimControls` + the `app` coordinator tying audio+power+input), then 1d (rip out v1 `playback`/`library`).
