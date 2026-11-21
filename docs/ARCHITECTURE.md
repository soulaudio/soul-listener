# Architecture

The DAP firmware is **hexagonal** (ports & adapters) with **vertical feature
slices**. Every structural rule here is a *fitness function* — a CI gate that
fails on drift, not a doc that rots (see "Enforcement" at the end, and
`docs/plans/2026-11-17-architecture-enforcement-fitness-functions.md`).

## Workspaces (repo layout)

`dap/` is its own Cargo workspace (Embassy 0.6 can't share a lock with the old
line); `eink-ui/` and the root are separate workspaces by design.

```
dap/
  core/          dap-core — the board-agnostic logic (see the slice map below)
  sim/           dap-sim  — host simulator adapters (SimSink/SimStorage/…) + the
                            headless e2e tests (audio/power/device/ui/firmware)
  board-h7a3/    the real STM32H7A3 firmware binary (stub ports until Phase E)
  storage-fat32/ real FAT32 SD Storage adapter (embedded-sdmmc over a BlockDevice)
  eink-ssd1677/  real e-ink panel driver
  arch-tests/    host fitness functions (memory/footprint budgets, …)
eink-ui/         the no_std/no-alloc UI toolkit (flex layout + widgets)
crates/eink/…    panel hardware specs
tools/           the gate scripts (forbid-unsafe, arch-boundaries, stack, pin)
```

`soul-audio-core` is a **sibling repo**, path-dep'd by `dap/` and pinned for CI
(`.github/soul-audio-core.pin`).

## dap-core: layers and vertical slices

```
                 ┌───────────── composition ─────────────┐
   firmware ───► │  app  (wires slices + ports together)  │ ◄─── stub (test
   (run-loop)    └────────────────────────────────────────┘      adapters)
                        │        │        │        │
            ┌───────────┘        │        │        └──────────┐
         audio ──┐            power ──┐  screens ──┐        display
         (slice) │            (slice) │  (UI views)│        (gfx primitives)
                 │                    │            └──► display
                 ▼                    ▼                        │
              ┌──────────────── ports ─────────────────────┐  │
              │  Sink · Storage · Power · Controls · …      │◄─┘
              │  + contract types (PowerProfile, …)         │
              │  THE INNERMOST LAYER — depends on nothing   │
              └─────────────────────────────────────────────┘
```

- **`ports`** — the hexagonal boundary: the capability traits a board or
  simulator implements (`Sink`, `Storage`, `Power`, `Controls`, `Display`) plus
  the contract types they exchange (e.g. `PowerProfile`). **Depends on nothing
  intra-crate.** Shared types the port signatures use live *here*, not up in a
  slice, so the boundary stays dependency-free.
- **Feature slices** — each a vertical cut of one capability, depending *only* on
  `ports`: **`audio`** (the `AudioPlayer`/Engine/Deck decode→resample→sink path),
  **`power`** (the `PowerService` state machine + the energy model/DVFS policy),
  **`display`** (gfx primitives). **`screens`** (now-playing view) is a UI slice
  over `display`, so it may use `ports` + `display`. Slices never import each
  other.
- **Composition** — **`app`** wires the slices + ports into the device;
  **`firmware::step`** is the one run-loop function the board ships *and* the host
  tests drive ("test what you ship"); **`stub`** are the no-op port adapters the
  board uses until Phase E. These may depend on anything below.

Adapters live outside dap-core: `dap-sim` (host), `board-h7a3`/`storage-fat32`/
`eink-ssd1677` (real). They implement `ports`; dap-core never names them.

## Enforcement (fitness functions — each fails CI on drift)

| Rule | Gate |
|---|---|
| **Module layering** (the diagram above) | `tools/check-arch-boundaries.sh` — no slice imports across a forbidden boundary |
| **No `unsafe`** in embedded crates | `tools/check-forbid-unsafe.sh` + `#![forbid(unsafe_code)]` |
| **No heap** on the audio path | `dap-sim/tests/no_alloc.rs` (counting allocator over the real cycle) |
| **Banned deps** (epd-waveshare, minimp3, soul-audio-core `playback`) | CI "Architecture boundaries" job |
| **Memory/stack budgets** | `dap-arch-tests/tests/budget.rs` + a compile-time AXI-SRAM assert in `board-h7a3` + `tools/mcu-stackprobe` (QEMU) |
| **No recursion / correct lints** | `#![deny(unconditional_recursion)]`, workspace lints |

To change the layering itself, change `check-arch-boundaries.sh`'s rules **and**
this diagram together, deliberately, in review — that's the point of the gate.
