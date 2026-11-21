# System Architecture (v2 — STM32H7A3)

The DAP firmware is **hexagonal** (ports & adapters) with **vertical feature
slices**, built on `soul-audio-core`'s decode/DSP engine. Every structural rule
below is a *fitness function* — a CI gate that fails on drift, not a doc that
rots. See the "Fitness functions" table and
`docs/plans/2026-11-17-architecture-enforcement-fitness-functions.md`.

> **Provenance.** This replaces a pre-migration `ARCHITECTURE.md` that described
> the abandoned v1 (`eink-emulator/` workspace; `platform`/`playback`/`library`/
> `bluetooth` crates; a `DisplayDriver`/`PlaybackEngine` trait layer; an
> STM32**H743ZI** map; a multi-task spawn model). All of that was deleted in the
> migration — this doc describes the code that actually ships today.

---

## 1. Workspaces (repo layout)

`dap/` is its own Cargo workspace (Embassy 0.6 can't share a lock with the old
line); `eink-ui/` and the repo root are separate workspaces by design.

```
dap/
  core/          dap-core — board-agnostic logic + ports (no_std, no unsafe, no embassy)
  sim/           dap-sim  — host adapters (SimSink/SimStorage/…) + the headless e2e tests
  board-h7a3/    the real STM32H7A3 firmware binary (Embassy 0.6; stub ports until Phase E)
  storage-fat32/ real FAT32 SD Storage adapter (embedded-sdmmc over a BlockDevice)
  eink-ssd1677/  real e-ink panel driver
  arch-tests/    host fitness functions (memory map / per-region budgets / footprint roster)
eink-ui/         no_std, no-alloc UI toolkit (flex layout + widgets)
crates/eink/…    panel hardware specs (eink-specs)
tools/           gate scripts (check-arch-boundaries, check-forbid-unsafe, mcu-stackprobe, pin)
```

`soul-audio-core` (+ `soul-audio-engine`, `soul-audio-index`) is a **sibling
repo**, consumed by `dap/` as a path dependency locally and **pinned** for CI
(`.github/soul-audio-core.pin`, currently `8d8e4e0…`) so DAP builds are
reproducible — picking up upstream changes is a deliberate, reviewed pin bump
(`tools/soul-audio-core-pin.sh`), never a drift.

---

## 2. dap-core: layers and vertical slices

`dap-core` is `#![cfg_attr(not(test), no_std)]`, `#![forbid(unsafe_code)]`,
`#![deny(unconditional_recursion)]`, and pulls in **no async runtime** — it
builds for the host and for `thumbv7em-none-eabihf` unchanged. Modules:
`ports`, `audio`, `power`, `display`, `screens`, `app`, `firmware`, `stub`.

```
                 ┌───────────── composition ─────────────┐
   firmware ───► │  app  (wires slices + ports together)  │ ◄─── stub (placeholder
   (run-loop)    └────────────────────────────────────────┘      port adapters)
                        │        │        │        │
            ┌───────────┘        │        │        └──────────┐
         audio ──┐            power ──┐  screens ──┐        display
         (slice) │            (slice) │  (UI views)│        (gfx primitives)
                 │                    │            └──► display
                 ▼                    ▼                        │
              ┌──────────────── ports ─────────────────────┐  │
              │  Sink · Storage · Power · Controls          │◄─┘
              │  + contract types (PowerProfile, …)         │
              │  THE INNERMOST LAYER — depends on nothing   │
              └─────────────────────────────────────────────┘
       (the board's panel adapter implements `display::Display`,
        the one capability trait that lives in a slice, not `ports` —
        it needs `display`'s own `Gray4`/`EinkRefresh` types)
```

- **`ports`** — the hexagonal boundary: the capability traits a board or
  simulator implements, plus the contract types they exchange. The audio ladder
  (`Sink`, `Source`, `BufSource`, `SeekSource`) is re-exported from
  `soul-audio-core` as-is (already board-agnostic); `Storage`, `Power`, and
  `Controls` are this crate's own. (The fifth capability a board implements,
  `Display`, lives in the `display` slice rather than here — it's bound on that
  slice's own `Gray4`/`EinkRefresh` types.) Contract
  types: `PowerProfile`, `BatteryReading`, `InputEvent`. **Depends on nothing
  intra-crate** — shared types a port signature uses (e.g. `PowerProfile`, which
  `Power::set_clock_profile` takes) live *here*, not up in a slice, so the
  boundary stays a true leaf.
- **Feature slices** — each a vertical cut of one capability, depending *only*
  on `ports`: **`audio`** (the `AudioPlayer` → `soul-audio-engine` `Engine`/`Deck`
  decode→resample→sink path), **`power`** (the `PowerService` state machine + the
  tweakable energy model), **`display`** (gfx primitives over `embedded-graphics`
  `Gray4`, plus the `Display` capability trait — `DrawTarget<Gray4> + EinkRefresh`
  — the board's panel adapter implements). **`screens`** (the now-playing view) is a UI slice over `display`, so
  it may use `ports` + `display`. Slices never import each other.
- **Composition** — **`app`** owns one `AudioPlayer` + one `PowerService` + the
  board's `Power`/`Controls`/`Display` adapters and drives all of them from a
  single `App::tick`; **`firmware::step`** is the one run-loop function the board
  ships *and* the host tests drive; **`stub`** (feature `stub`) are the no-op port
  adapters the board uses until Phase E. These may depend on anything below.

Adapters live **outside** dap-core: `dap-sim` (host), and
`board-h7a3`/`storage-fat32`/`eink-ssd1677` (real). They implement `ports`;
dap-core never names a concrete adapter.

---

## 3. Dependency injection via generics

Ports are injected as **type parameters**, not trait objects — zero dynamic
dispatch, zero vtables, everything monomorphized and alloc-free. The whole
device is one generic struct:

```rust
// board-h7a3 today: every port is a Phase-E stub placeholder.
type Dap =
    App<StubSink, StubStorage, &'static mut [i32], StubPower, StubControls, StubDisplay, PRELOAD>;
```

A real driver replaces a stub by swapping one type argument — the composition
and all the slices below are untouched. `Pcm` (the decoder scratch handle) is a
generic too (`&'static mut [i32]` on-target, `Box<[i32]>` in a host test), so
dap-core never assumes an allocator.

---

## 4. Concurrency: the `firmware::step` run-loop

v2 is **not** a multi-task spawn model. `board-h7a3` runs a single
`#[embassy_executor::main]` loop that calls **one** run-loop function each
scheduler period and then `Timer::after`s:

```rust
let mut last_render_ms: Option<u64> = None;
let mut now_ms: u64 = 0;
const PERIOD_MS: u64 = 20;
loop {
    let outcome = firmware::step(app, now_ms, &mut last_render_ms);
    // …log outcome…
    now_ms = now_ms.saturating_add(PERIOD_MS);
    Timer::after(Duration::from_millis(PERIOD_MS)).await;
}
```

- `firmware::step` → `App::tick(now_ms)` (drain `Controls`, poll `PowerService`,
  pump the deck one block) **then** renders only if the tick did something
  user-visible *or* `RENDER_PERIOD_MS` (1 s) has elapsed — a full e-ink refresh
  takes hundreds of ms to seconds (`eink_specs::DisplaySpec::full_refresh_duration`),
  so rendering every 20 ms tick would be wrong. `step` carries **no state**:
  `last_render_ms` is the caller's own `Option<u64>`, threaded through.
- This is the single run-loop both the board and `dap-sim/tests/firmware_step.rs`
  drive — *test what you ship*. The host test exercises byte-identical scheduling
  logic over swapped adapters.

Phase E will add Embassy tasks for genuinely concurrent, DMA-driven work (the
SAI/I²S audio ring, the e-ink SPI transfer) feeding this loop; the loop itself
stays the coordinator.

---

## 5. State management

- **`App::tick` → `TickOutcome`** — a plain `Copy` struct reporting what the
  tick did (`play_pause_toggled`, `volume_delta`, `track_advanced`,
  `low_battery`, `critical_shutdown`, `pump: Result<Progress, ()>`). Returned for
  tests/telemetry; a driver needn't inspect it (every effect is also in `App`'s
  accessors).
- **`PowerService`** — an explicit state machine over `BatteryReading` samples
  that emits a bounded (`heapless`, no-alloc) set of `PowerEvent`s
  (`LowBattery`, `CriticalShutdown`, `ThermalThrottle`, …) and auto-engages
  `PowerProfile::BatterySaver`/`Performance`.
- **`Deck`/`Engine` transport state** lives in `soul-audio-core` (`DeckState`,
  `Progress`); `App` is pure wiring over it, deliberately holding no decision
  logic of its own.

---

## 6. Memory architecture

The embedded path is **entirely static and alloc-free**. `board-h7a3` carries
**no allocator** at all — the one 4 KB `embedded_alloc` arena v1 needed (leaked
by the old display toolkit) was removed when the DAP moved onto the no-alloc
`eink-ui` toolkit (proven by `dap-sim`'s `alloc_policy` + `no_alloc` tests).

- **Static placement.** The large `App` and the decoder scratch are built once at
  boot into `StaticCell`s (`App` via `init_with`, in-place, so a 175 KB value is
  never constructed on `main`'s stack and moved). Scratch is externalized as a
  `&'static mut [i32]` of `decoder_scratch_len()` (65,536 × i32 = **256 KiB**) —
  no allocator needed for a real `&'static mut`.
- **Gapless, one resident decoder.** `PRELOAD = 0`: a single decoder is resident;
  gapless comes from the output ring at track reopen (the Rockbox model), not a
  second preloaded decoder — half the scratch of the old PRELOAD=1 design.

### Memory map (STM32H7A3ZI)

```
0x0800_0000  FLASH        2 MB   dual-bank      ← code, const data
0x2000_0000  DTCM       128 KB   fast, NOT DMA-capable
0x2400_0000  AXI SRAM  1024 KB   D1, DMA-capable ← the ONE region memory.x links;
                                                   cortex-m-rt puts the stack at its top
0x3000_0000  AHB SRAM1/2/3       with AXI = 1,208 KB "USER_SRAM" (DMA-capable, not yet linked)
0x0000_0000  ITCM        64 KB   hot code (ISRs)
             BACKUP_SRAM  4 KB   VBAT-retained
```

`memory.x` deliberately links a **single** RAM region — the 1 MB AXI SRAM — so
the stack sits at its top. With `App` ≈ **175 KiB** (measured 178,744 B) +
scratch **256 KiB** ≈ 432 KiB of statics, that leaves **≈ 592 KiB of stack
headroom**. `dap-arch-tests` splits the full 1,208 KB `USER_SRAM` into
documented per-region budgets (audio path 384 KB, scratch pool 256 KB,
framebuffer + partial-refresh shadow 2 × 93.75 KB at 800×480 Gray4 2bpp, catalog
index, DMA staging, …), summing to ≈ 1,083.5 KB with headroom to spare, and a
compile-time `assert!` in these region sums enforces the split actually fits.

### Stack (measured, not reasoned)

`tools/mcu-stackprobe` runs the real open→decode→pump path under **QEMU
`mps2-an500` (Cortex-M7)** with stack-painting. Worst-case track-open transient
(Matroska-FLAC) is **≈ 315 KiB** of growth below the call site — it fits the
592 KiB headroom with ≈ 277 KiB to spare. (An earlier "~1 MB" figure was a
measurement bug: it counted the probe's own `main` frame by measuring depth from
the top of RAM instead of growth below the call site; fixed 2026-11-19.)

---

## 7. Error handling

No `thiserror`/`anyhow` on the embedded path. Ports surface failure through
associated `Error` types and `Result`:

- `Storage::open` → `Result<Self::Source, Self::Error>`; `Display::flush` →
  `Result<(), DisplayError>`.
- `App::tick` has no `Result` to propagate, so a failed `AudioPlayer::pump`
  (storage/open/decode/sink error) is folded into `TickOutcome::pump:
  Result<Progress, ()>` — transport state still reflects every input/power effect
  from the same tick even when the pump errors.
- **Panics.** `board-h7a3` uses `panic-probe` + `defmt-rtt` (log via RTT, halt).
  There is no panic on the steady-state hot path; `#![deny(unconditional_recursion)]`
  and the no-alloc gates keep it that way.

---

## 8. Performance

- **No-alloc steady state.** `dap-sim/tests/no_alloc.rs` runs the real
  decode→pump cycle for every format under a counting allocator and asserts
  **zero** heap allocations in the measured window.
- **DMA-capable statics.** The linked AXI SRAM is D1-domain, DMA-capable;
  Phase-E audio/display rings live there (the budgets reserve DMA staging).
- **Zero-copy decode.** Decoders write into the externalized scratch and the
  output ring in place; there is no intermediate per-block allocation.
- **Sleep between ticks.** The run-loop `Timer::after`s each period; Phase E
  grows a decode-ahead ring to lower the wakeup rate and extend CPU sleep (the
  audio-path budget keeps 384 KB specifically to let that ring grow).

---

## 9. Build configurations

| Target | What it is | Ports |
|---|---|---|
| `dap-core` (default) | board-agnostic logic + ports | — (traits only) |
| `dap-core` `--features stub` | adds `StubSink/StubStorage/StubControls/StubPower/StubDisplay` | placeholder |
| `board-h7a3` | the shipped firmware binary (Embassy 0.6, `thumbv7em-none-eabihf`) | stubs → real drivers, one at a time (Phase E) |
| `dap-sim` | host simulator adapters + headless e2e tests | `Sim*` doubles |

`soul-audio-core` codecs are feature-gated **upstream** (in the sibling repo); a
format is enabled by turning on its decoder feature there, which flows through
`AnyDecoder`. The DAP consumes whatever the pinned revision enables.

---

## 10. Extensibility points

- **Add an audio codec** → implement it behind `soul-audio-core`'s `AnyDecoder`
  (feature-gated), in the sibling repo. dap-core and the board need no change —
  `Storage` hands bytes to `open_seekable`, which probes the container.
- **Add a port adapter** (new display panel, SD backend, fuel-gauge) → implement
  the `ports` trait in a board/sim crate and swap the type argument in the `App<…>`
  alias. No slice or composition change.
- **Add a screen** → a new view in `screens/` drawing with `eink-ui` widgets over
  the `display` primitives; wire it into `App::render`.

---

## 11. Testing architecture

Host-first: the board-agnostic logic is exercised end-to-end on the host over
swapped adapters, then the exact same `firmware::step` runs on-target.

- **Headless e2e** (`dap/sim/tests/`): `audio_headless`, `power_headless`,
  `device_headless`, `firmware_step` (the shipped run-loop), `ui_headless`,
  `fat32_pipeline_e2e` (real FAT32 SD adapter → decode → sink), plus the memory
  fitness tests `no_alloc`, `alloc_policy`, `static_placement`, `stack_highwater`.
- **On-target stack** (`tools/mcu-stackprobe`): QEMU Cortex-M7 stack-painting
  over the real decode path.
- **Footprint/budgets** (`dap/arch-tests/`): the memory map + per-region budget
  guards described in §6.

---

## 12. Fitness functions (each fails CI on drift)

| Rule | Gate |
|---|---|
| **Module layering** (§2) | `tools/check-arch-boundaries.sh` — no slice imports across a forbidden boundary (ports→nothing, slices→ports, screens→ports+display, composition→free) |
| **No `unsafe`** in embedded crates | `tools/check-forbid-unsafe.sh` + `#![forbid(unsafe_code)]` |
| **No heap** on the audio path | `dap-sim/tests/no_alloc.rs` + `alloc_policy.rs` (counting allocator over the real cycle) |
| **Banned deps** (epd-waveshare, minimp3) | CI *Architecture boundaries* job (`cargo tree` grep) |
| **Static memory budgets** | `dap-arch-tests/tests/budget.rs` + a compile-time AXI-SRAM `assert!` in `board-h7a3` |
| **Stack headroom** | `tools/mcu-stackprobe` (QEMU) + `stack_highwater.rs` |
| **No recursion / workspace lints** | `#![deny(unconditional_recursion)]`, enforced workspace lints |
| **Reproducible upstream** | `.github/soul-audio-core.pin` + pin-drift job |

To change the layering itself, change `check-arch-boundaries.sh`'s rules **and**
the §2 diagram together, deliberately, in review — that is the point of the gate.

---

## References

- [Hexagonal / Ports & Adapters (Cockburn)](https://alistair.cockburn.us/hexagonal-architecture/)
- [Vertical Slice Architecture](https://www.jimmybogard.com/vertical-slice-architecture/)
- [Architecture fitness functions (Building Evolutionary Architectures)](https://www.thoughtworks.com/insights/articles/fitness-function-driven-development)
- [Embassy](https://embassy.dev/) · [The Embedded Rust Book](https://docs.rust-embedded.org/book/)
- `docs/plans/2026-11-17-architecture-enforcement-fitness-functions.md` (the enforcement canon)
