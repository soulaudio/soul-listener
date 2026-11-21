# Memory headroom + battery optimization — design

> Spec. Captures the web research, the coupled SRAM re-budget (visual + library
> listing + audio), the battery-optimization design, and the test plan. Target:
> STM32H7A3ZI (1.18 MB DMA-capable user SRAM, 280 MHz Cortex-M7, 2 MB flash).
> Implemented across `dap-core` (power model), `dap-arch-tests` (budget guards),
> and `dap-sim` (energy-model + benchmark tests). No hardware required — the
> battery tests are a *model* grounded in the datasheet currents + our measured
> decode real-time-factors, not a measurement.

## 0. Why these three asks are one problem

The user asked for three things together: (a) memory headroom for the **visual/UI**
layer, (b) memory for **library listing**, (c) **battery optimization** with tests.
They are coupled through one scarce resource — the **1.18 MB of DMA-capable user
SRAM** — because the single biggest battery lever is the **decode-ahead buffer**,
and that buffer competes with the framebuffer(s) and the browse working set for
the same SRAM:

- A **bigger decode-ahead ring** ⇒ the CPU wakes less often to refill it and the
  SD card is read in bigger bursts then powered down longer ⇒ **less energy**
  (Rockbox's measured result for decades of DAPs).
- But that ring lives in the same 1.18 MB as the **framebuffer + partial-refresh
  shadow** (visual) and the **browse page/string working set** (listing).

So the honest design is **one explicit SRAM budget that balances all three**, with
the ring size called out as the audio↔battery↔RAM tunable. That is what §3 does.

## 1. Battery research findings (STM32H7 + DAP practice)

### 1.1 The three levers

1. **DMA plays while the CPU sleeps.** The SAI/I²S DMA streams the output ring to
   the DAC autonomously while the Cortex-M7 is in **Sleep (WFI)**. The CPU only
   wakes to decode-ahead and refill, then sleeps until the DMA half/complete IRQ.
   For audio playback the core is idle the large majority of wall-clock time.

2. **Dynamic voltage + frequency scaling (DVFS) — the biggest lever.** Run-mode
   current scales ~V²·f, and the H7 has on-the-fly voltage scaling steps
   (VOS0 1.35 V/480 MHz … **VOS3 1.0 V/200 MHz**). Representative run current:

   > **Correction (2026-11-21):** those ceilings are the **STM32H743's**, not
   > this board's. On the H7A3 (DEV_ID `0x480`) the scales cap at
   > **280 / 225 / 160 / 88 MHz** — so VOS3 is 88 MHz here, not 200. The figure
   > above was propagated into `power::model`'s clock-headroom gates and
   > invalidated their headline claim; see
   > `2026-11-20-memory-battery-selfimproving-loop.md` L11. The authoritative
   > tables now live in `dap-core::power::clocks`.


   | Clock   | ~Current |
   |---------|----------|
   | 480 MHz | ~125 mA  |
   | 400 MHz | ~90 mA   |
   | 280 MHz | ~58 mA   |
   | 60 MHz  | **~13.5 mA** |

   Running at the **lowest clock that still sustains real-time decode** is a ~4×
   swing. Our decode-RTF benchmarks tell us how low we can go per format.

3. **Decode-ahead-then-sleep + SD burst.** "CPU and disk are the two biggest
   drains." Decoding a big chunk fast then sleeping — and reading the SD card in
   infrequent bursts then powering it down — beats trickle-decoding. Bigger
   buffer ⇒ fewer SD spin-ups + longer CPU sleep ⇒ less power.

### 1.2 DAP-practice (Rockbox) playbook

- **Low base clock + boost-on-demand:** idle/UI at a very low clock, boost only
  while a hard codec decodes. FLAC and easy codecs never boost.
- **e-ink is a battery superpower:** **zero power to hold** a bistable image, **no
  backlight** (the #1 LCD-DAP drain), and **partial refresh ≪ full refresh**. Our
  UI is essentially free while static — a major advantage over LCD DAPs.
- **Peripheral gating:** power down BT/USB/unused oscillators when idle.

Sources:
- [ST AN5014 — STM32H7 smart power management](https://www.st.com/resource/en/application_note/an5014-stm32h7x3-smart-power-management-expansion-package-for-stm32cube-stmicroelectronics.pdf)
- [ST AN5215 — advanced power management on STM32H7](https://www.st.com/resource/en/application_note/an5215-how-to-implement-advanced-power-management-on-stm32h747757-mcus-stmicroelectronics.pdf)
- [ST AN5307 — STM32H7A3/7B3 hardware development](https://www.st.com/resource/en/application_note/an5307-getting-started-with-stm32h7a37b3-line-and-stm32h7b0-value-line-microcontroller-hardware-development-stmicroelectronics.pdf)
- [Rockbox BatteryFAQ](https://www.rockbox.org/wiki/BatteryFAQ)
- [Rockbox GUI-boost / frequency-scaling tracker](https://www.rockbox.org/tracker/8668)

## 2. Current budget (committed, PRELOAD=1)

`dap/arch-tests/tests/budget.rs` today, all guards green:

| Region                          | Budget    | Measured |
|---------------------------------|-----------|----------|
| Audio path (Deck PRELOAD=1)     | 704 KB    | 664 KB   |
| e-ink framebuffer (Gray4 2bpp)  | 93.75 KB  | —        |
| Catalog index (`IndexReader`)   | 32 KB     | 96 B     |
| DMA staging                     | 64 KB     | —        |
| **Total**                       | **894 KB**| of 1208  |

Margin: ~314 KB — but **visual is only the single framebuffer**, there is **no
partial-refresh shadow, no UI scratch, and no browse working set** budgeted. The
re-budget below spends that margin deliberately.

## 3. Proposed re-budget — explicit visual + listing headroom

| Region | Budget | Rationale |
|---|---|---|
| **Audio path** — Deck PRELOAD=1 + decode-ahead ring | **704 KB** | Measured 664 KB; the 40 KB headroom is the **battery ring tunable** — grow the ring into it (or into global margin) for more CPU/SD sleep. |
| **Visual — framebuffer** (800×480 Gray4, 2 bpp, packed) | **93.75 KB** | The live frame drawn to the panel. |
| **Visual — partial-refresh shadow** (last-flushed frame) | **93.75 KB** | Diff new-vs-shadow → dirty regions → **partial refresh** (the battery + speed win on e-ink). Optional-but-recommended; can be dropped if eink-ui tracks dirty rects at draw time instead. |
| **Visual — eink-ui scratch** (glyph/layout/compose) | **32 KB** | Bounded immediate-mode scratch; no-alloc. |
| **Library listing working set** — browse page records + order table + string buffer | **64 KB** | The visible list (~8–15 rows) + its strings, held for the browse screen. Distinct from the catalog *reader* below. |
| **Catalog index** — `.soulidx` `IndexReader` + scan scratch | **32 KB** | Paged reader; never scales with library size (proven by the index-no-growth guard). |
| **DMA staging** | **64 KB** | DMA descriptors + staging. |
| **TOTAL** | **~1,088 KB** | of 1,208 KB → **~120 KB global margin** |

Notes:
- **120 KB global margin** stays unspent as true headroom (stack excursions, future
  features) and as the overflow pool for a bigger battery ring.
- The **shadow buffer** is the one genuinely-optional line: if eink-ui ui-3 tracks
  dirty rectangles from the layout pass rather than diffing pixels, drop it and the
  margin rises to ~214 KB. Decision deferred to eink-ui ui-3; budgeted for now so
  the plan is safe either way.

### 3.1 The critical stack question (DTCM vs AXI)

`AnyDecoder` is **262 KB** and is returned **by value** from `open()`, so decoder
construction puts a ~262 KB temporary **on the stack** transiently. The H7A3's
fast DTCM is only **128 KB** — a 262 KB stack temporary **does not fit DTCM**.
Implication to verify + document (see §5 stack high-water test):
- Either the main stack lives in **AXI SRAM** (not DTCM), **or**
- the decoder is constructed **directly into a `StaticCell`** (no large stack
  temporary). Rust has no stable placement-new, so the robust answer is likely
  "**stack in AXI SRAM + decoder in a static**." The test measures the real peak
  so we size the stack region correctly before the board exists.

## 4. Battery-optimization design

### 4.1 Playback power loop
```
decode-ahead to fill ring → WFI sleep → DMA half/complete IRQ wakes CPU → refill → WFI …
```
CPU duty cycle ≈ decode_time / wall_time; the rest is Sleep. Bigger ring ⇒ lower
wakeup rate ⇒ longer uninterrupted sleep ⇒ less energy.

### 4.2 DVFS policy via PowerService profiles
The existing `PowerService` profiles become the clock/VOS policy. Clock is chosen
from the **current codec's measured RTF** (min clock that sustains ≥1× real-time
with a safety margin):

| Profile | Clock/VOS policy |
|---|---|
| **Performance** | High VOS, boost clock — snappy UI, hardest codecs, least sleep. |
| **Balanced** (default) | Moderate clock that sustains most codecs; sleep between refills. |
| **Battery-saver** | Lowest clock+VOS that sustains the *current* codec; max sleep; aggressive SD power-down. |

Plus **boost-on-demand**: low base clock for idle/UI, transient boost only while a
hard codec decodes (driven by RTF headroom).

### 4.3 SD + peripherals + e-ink
- **SD burst-then-idle:** read ring-fills in big bursts, power the SD down between.
- **Peripheral gating:** disable unused peripherals/oscillators when idle.
- **e-ink:** hold = zero power; prefer **partial refresh** (needs the shadow
  buffer, §3); never full-refresh on a scroll tick.

## 5. Test plan

### 5.1 Memory best-practices (dap-arch-tests + dap-sim, host)
- **Re-budget guards** — the §3 table as `const` budgets + the running-total fit
  test (extend `memory_budget_fits_user_sram`).
- **No-alloc enforcement** — a counting global allocator in a host test; run a full
  open→decode→pump→play cycle; **assert 0 heap allocations** from the audio path.
- **Stack high-water-mark per format** — measure the real peak stack of
  open/decode/pump per format (binary-search the minimum non-overflowing stack, or
  stack-painting). Report each number; assert ≤ the chosen stack-region size.
  Document the DTCM-vs-AXI implication (§3.1).
- **Static-placement demo** — construct `AudioPlayer`/`Deck` into a `StaticCell`
  (board pattern) and prove it works without a large stack temporary.
- **Component footprint roster** — `size_of` guards for every main-stack type
  (Deck, Engine, AudioPlayer, Resampler, each decoder variant, Catalog browse
  state, PowerService, App) in one table.
- **Index-no-growth** — prove `IndexReader` size is constant regardless of library
  size (already guarded; keep + strengthen with a comment-backed rationale).

### 5.2 Battery energy model (dap-core `power::model` pure fns + dap-sim tests)
A small, pure, `#![no_std]`-friendly model in `dap-core` with datasheet constants,
exercised by host tests (capacity parameterized — **default 2000 mAh single-cell**,
one `const` to change):
- `min_clock_mhz_for_rtf(rtf_at_ref, ref_mhz, safety) -> u32` — **clock-headroom
  per format** test: assert every format can sustain ≥1× at ≤ a target clock.
- `duty_cycle(decode_cost, ring_frames, sample_rate) -> f32` — **sleep-ratio**
  test: assert the ring is sized for a target wakeup rate / sleep fraction.
- `avg_current_ma(duty, active_ma, sleep_ma) -> f32` and
  `runtime_hours(capacity_mah, avg_ma) -> f32` — **runtime estimate per format**
  test: assert ≥ a target (e.g. hours) and flag regressions.
- `wakeups_per_sec(sample_rate, ring_frames) -> f32` — **wakeup-rate** test.

These are a *relative* energy model (datasheet currents × measured decode cost),
not a mAh measurement — they optimize the design and catch regressions before the
board exists; real battery life needs silicon.

### 5.3 Benchmarks (dap-sim, host, already seeded)
- Full-format decode RTF (feeds §5.2's clock-headroom).
- Resampler / pipeline throughput.
- Allocation-count under decode (ties to §5.1 no-alloc).

## 6. Implementation phases

- **Phase M1 — re-budget + memory best-practices tests** (§3, §5.1): update
  `budget.rs` to the §3 table; add no-alloc, stack high-water, static-placement,
  footprint-roster tests. No behavior change, pure guards + measurement.
- **Phase M2 — battery energy model + tests** (§4.2 model hooks, §5.2): add
  `dap-core::power::model` + the energy-model tests; wire the DVFS policy into the
  PowerService profiles (model-only; board applies VOS later).
- **Phase M3 — benchmarks** (§5.3): extend the decode benches to all formats and
  add the resampler/pipeline + alloc-count benches.

All three are host-only + no-hardware, commit as `sebastianstupak`, Conventional
Commits, hooks enforced, on a design branch off `feat/eink-ui`.
