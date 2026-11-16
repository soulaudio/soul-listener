# soul-listener — UI Phase Design Spec (e-ink, headless-first)

**Date:** 2026-11-16
**Status:** Draft for review
**Builds on:** `docs/plans/2026-11-16-logic-and-headless-simulation-design.md`. Phase 1 (logic + headless sim) is complete; this adds the display/UI layer, still headless-first.

## 1. Direction

Salvage v1's e-ink UI toolkit, add a `Display` port, and render the DAP's screens — **testable headless through a Gray4 framebuffer** (no window), exactly like audio/power/controls are. Build the plumbing first; **design the actual screens by rendering them for real through the emulator** (not mockups). Concrete hardware (SSD1677 on SPI) is a later, board-only adapter.

## 2. What's salvageable (verified)

- `crates/eink/eink-system` (flexbox layout), `eink-components` (Button/Label/ProgressBar/Icon), `eink-specs` (panel data), `crates/ui` (Navigator + screen state) — **all embassy-free** (embedded-graphics + heapless). They move into `dap/` as-is.
- The **SSD1677 driver** (`crates/firmware/src/display/driver.rs`) — generic over `embedded-hal-async` SpiDevice; becomes a `board-h7a3` Display adapter (hardware phase).
- `eink-emulator` is a winit desktop GUI coupled to `platform`; we **don't** take the window — we lift only its **Gray4 framebuffer + golden-image test** path into a headless `SimDisplay`.

## 3. Architecture (extends the hexagon)

```
dap-core
  ports::Display   = embedded-graphics DrawTarget<Color = Gray4> + an e-ink refresh trait
                     (full refresh · partial/region refresh · BUSY-wait is the adapter's concern)
  ui (salvaged)    = Navigator + screen state (board-agnostic)
  app::App<Sink, Storage, Power, Controls, Display, PRELOAD>
                     → each tick, renders the current screen to the Display port
dap/ui-kit          = eink-system + eink-components + eink-specs (moved in; the drawing toolkit)
dap-sim
  SimDisplay        = a headless Gray4 framebuffer DrawTarget; dumps PNG + supports golden-image
                      assertions (lifted from eink-emulator's headless path + eink-testing)
board-h7a3
  Ssd1677Display    = the real panel adapter (hardware phase)
```

`Display` is 4-gray (`Gray4`) — v1's spec'd it but only shipped 1bpp; we implement 4-gray here (the display gap the salvage audit flagged). The UI renders via `embedded-graphics`, so the exact same screen code draws to `SimDisplay` (host) and `Ssd1677Display` (board).

## 4. Screens

- **Now-Playing** — track/artist, progress, volume, battery (from the `App`'s player + power state). Straightforward; built first to prove the plumbing.
- **Browse / menu** — the wheel-driven list (twist = scroll, press = select). This is the **net-new UX** (v1 never shipped a real menu) and the **design-sensitive** part.

**Screen design process:** once `SimDisplay` renders real Gray4 PNGs, screen layouts are **rendered and compared as actual images** (via the headless renderer) and iterated — not decided on abstract mockups. The now-playing layout and the browse interaction get a render-and-pick pass before they're finalized.

## 5. Headless testing

The UI joins the virtual-DAP harness: `App` gains a `SimDisplay`; `cargo test` renders screens to Gray4 buffers and asserts via **golden images** (reuse `eink-testing`'s approach) + structural checks (e.g. the now-playing screen shows the current track after a `Press`; the browse list scrolls on `Rotate`). Still zero hardware.

## 6. Input mapping (now real)

Phase 1c's provisional `App` mapping (`Rotate`→volume) is replaced by **context-aware** mapping via the `Navigator`: in Browse, `Rotate` scrolls the list + `Press` selects; in Now-Playing, `Rotate` = volume + `Press` = play/pause, long-press → back to Browse. This is the `map_input` seam Phase 1c left for exactly this.

## 7. Phasing

1. **2a — plumbing:** `Display` port in dap-core; move the eink toolkit into `dap/`; `SimDisplay` (headless Gray4 + PNG); `App` gains `Display`; a minimal **Now-Playing** screen rendered + asserted headless. (No UX subjectivity — pure infrastructure + a first screen.)
2. **2b — screens (design-sensitive):** finalize Now-Playing + build the **wheel-driven Browse** UX, **rendered as real PNGs and iterated / picked**. Context-aware input mapping.
3. **2c — board adapter:** the SSD1677 `Display` adapter in `board-h7a3` (4-gray, partial refresh) — compile-gated now, bench-verified when silicon exists.

## 8. Out of scope / deferred

- The interactive winit desktop emulator app (nice-to-have; the headless `SimDisplay` covers development + tests).
- On-silicon display bring-up (needs the PCB).
- The `AnyDecoder` embedded-stack budgeting (tracked; a playback-on-hardware concern, not UI).
