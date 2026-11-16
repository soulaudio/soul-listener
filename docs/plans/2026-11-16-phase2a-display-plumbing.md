# Phase 2a — Display port + headless UI plumbing — Implementation Plan

> REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`.
> Spec: `docs/plans/2026-11-16-phase2-ui-design.md` (phase 2a — plumbing + first screen; the screen UX is designed-by-rendering in 2b).

**Goal:** Add a `Display` port, let `dap/` draw with the salvaged e-ink toolkit, and render a minimal **Now-Playing** screen to a headless Gray4 framebuffer (`SimDisplay`) — asserted in `cargo test`, zero hardware.

**Architecture:** `dap-core::ports::Display` (embedded-graphics `DrawTarget<Color=Gray4>` + an e-ink refresh trait). `dap/` **consumes the eink toolkit in place** via path deps (`crates/eink/eink-system`/`-components`/`-specs` + `crates/ui` are embassy-free, so no move/collision). `App` gains a `Display` and renders the current screen each tick. `dap-sim::SimDisplay` is a host Gray4 framebuffer that dumps PNG + supports pixel/golden assertions.

**Tech Stack:** Rust, `embedded-graphics` 0.8, the salvaged eink crates (consumed in place), `image` crate for PNG in `dap-sim` (host only).

## Global Constraints

- `dap-core` stays `#![no_std]` + `#![forbid(unsafe_code)]`, host + `thumbv7em`, no alloc/dyn. The `Display` port is a generic bound (`D: DrawTarget<Color=Gray4> + EinkRefresh`), not boxed.
- **Do not move or edit** `crates/eink/*` or `crates/ui` — consume them via path deps from `dap/`. (They're embassy-free; verified.) Do not touch `eink-emulator`/`eink-testing`.
- `SimDisplay` (host, `std`, `image` for PNG) lives in `dap-sim` only.
- Commits `sebastianstupak <sebastian.stupak@pm.me>`, no AI trailers (the commit-msg hook now enforces this). Branch `migration/phase-2-ui`.

---

### Task 1: `Display` port + wire the eink toolkit into `dap/`

**Files:** Create `dap/core/src/display.rs`; modify `dap/core/src/{ports.rs,lib.rs}`, `dap/core/Cargo.toml`, `dap/Cargo.toml`.

**Interfaces — Produces:** `ports::Display` + a re-export surface so screen code can use the eink toolkit from `dap-core`.

- [ ] **Step 1:** In `dap/Cargo.toml` `[workspace.dependencies]`, add path deps (relative to `dap/`): `eink-system = { path = "../crates/eink/eink-system" }`, `eink-components = { path = "../crates/eink/eink-components" }`, `eink-specs = { path = "../crates/eink/eink-specs" }`, `ui-kit = ...` — wait: the crate is `crates/ui`; add `v1-ui = { path = "../crates/ui", package = "ui" }` (alias to avoid a bare `ui` name clash). Confirm each builds for `thumbv7em` as a dep.
- [ ] **Step 2:** `dap/core/src/display.rs`: define `trait EinkRefresh { fn full_refresh(&mut self) -> Result<(), DisplayError>; fn partial_refresh(&mut self, area: Rectangle) -> Result<(), DisplayError>; }` and a `Display` alias/bound = `embedded_graphics::draw_target::DrawTarget<Color = embedded_graphics::pixelcolor::Gray4> + EinkRefresh`. Re-export `eink_system`/`eink_components` draw helpers that screen code will use.
- [ ] **Step 3:** `dap-core` depends on `eink-system`/`-components`/`-specs` (workspace) + `embedded-graphics`. `lib.rs`: `pub mod display;`.
- [ ] **Step 4 (verify):** `cd dap && cargo build -p dap-core` (host) + `cargo build -p dap-core --target thumbv7em-none-eabihf` (eink toolkit links for target) + `cargo test -p dap-core` (still green) + `cargo clippy -p dap-core`.
- [ ] **Step 5:** Commit `feat(dap-core): Display port (Gray4 + e-ink refresh) + eink toolkit wired in`.

---

### Task 2: `SimDisplay` + `App` renders a Now-Playing screen, asserted headless

**Files:** Create `dap/sim/src/display.rs` (`SimDisplay`), `dap/core/src/screens/now_playing.rs`, `dap/sim/tests/ui_headless.rs`; modify `dap/sim/src/lib.rs`, `dap/core/src/{app.rs,lib.rs}`, `dap/sim/Cargo.toml`.

**Interfaces — Consumes:** Task 1's `Display`/`EinkRefresh`; the `App`; `SimSink`/`SimStorage` from earlier phases.

- [ ] **Step 1:** `dap-sim::SimDisplay` — an 800×480 `Gray4` framebuffer implementing `DrawTarget<Color=Gray4>` + `EinkRefresh` (refresh = record a refresh happened / snapshot). `fn save_png(&self, path)` via the `image` crate; `fn pixel(x,y) -> Gray4` + a simple non-blank / coverage check for assertions.
- [ ] **Step 2:** `dap-core::screens::now_playing` — `fn render<D: Display>(d: &mut D, state: &NowPlayingView) -> Result<(),_>` drawing track title, a progress bar, volume, and battery % using `eink-components` widgets + `eink-system` layout. `NowPlayingView` is a small struct the `App` fills from its player + power state. **Keep layout minimal/functional — 2b polishes it by rendering.**
- [ ] **Step 3:** `App` gains a `D: Display` generic (`App<S, St, P, C, D, PRELOAD>`); add `fn render(&mut self)` (and call it in `tick` or expose it) that builds a `NowPlayingView` and calls `now_playing::render` + a `full_refresh`.
- [ ] **Step 4 (TDD):** `dap/sim/tests/ui_headless.rs` — build an `App` with `SimSink`+`SimStorage`+`SimBattery`+`SimControls`+`SimDisplay`, `Press` to play, `render()`, then assert the framebuffer is non-blank and (structurally) that the track title region changed vs a blank baseline; `save_png` to the target dir for eyeballing. Run → FAIL first, implement → PASS. (Use the 64 MiB test-thread helper, as the other sim tests do.)
- [ ] **Step 5 (verify):** `cd dap && cargo test` (audio + power + device + ui headless all green) + `cargo clippy -p dap-sim` clean + `cargo build -p dap-core --target thumbv7em-none-eabihf` (screens stay target-capable).
- [ ] **Step 6:** Commit `feat(dap): SimDisplay + Now-Playing screen rendered + asserted headless`.

---

## Deliverable

The UI renders headless: a Now-Playing screen drawn with the salvaged e-ink toolkit to a Gray4 framebuffer, driven by the real `App`, asserted in `cargo test` and dumped as PNG for eyeballing — zero hardware. Next: **2b** finalizes Now-Playing + builds the wheel-driven Browse UX, **designed by rendering real PNGs and picking** (per the design preference); then **2c** is the SSD1677 board adapter.
