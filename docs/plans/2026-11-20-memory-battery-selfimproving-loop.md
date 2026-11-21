# Memory & battery optimization — self-improving loop

**Status:** active. Running loop. Each entry: measure → change → verify (build/test/
gate) → commit. Gates accumulate so a regression fails CI, not review.

Board: STM32H7A3 (1 MB AXI-SRAM linked as the single RAM region; 280 MHz max;
VOS0–3). Battery: Akyga LP623255 (see [[soul-listener-sourcing-licensing]]).
Ports are still `dap_core::stub` — so *battery* numbers are analytical/design at
this stage, while *memory* footprint is concretely measurable today.

## Research thesis (web-researched 2026-11-20 — see sources at end)

- **Race-to-sleep is the master battery lever.** Faster decode (better real-time
  factor) → CPU idles longer → less energy. Embassy already parks the core in WFI
  between `Timer::after` ticks, so decode efficiency (RTF) *is* battery life.
- **VOS voltage scaling**: run the lowest frequency + VOS that sustains real-time
  decode. STM32H7A3 ceilings are **VOS0 280 / VOS1 225 / VOS2 160 / VOS3 88 MHz**
  (per ST's own `DEV_ID 0x480` tables — *not* the H743 figures the v1 design doc
  §4.2 carried; see L11). Stop mode = 32 µA, RAM retained — the pause/idle target.
- **DMA audio (SAI/I²S) + WFI** lets the core sleep ~70 % during playback;
  interrupt only per DMA block, not per sample. (Phase E, when the real Sink lands.)
- **Memory**: fixed/static buffers, zero heap (already enforced), right-size
  scratch, compile-time footprint gates (`heapless`, `static`, no `Vec`/`String`).

## Measured baseline (2026-11-20)

- Decoder scratch (`decoder_scratch_len` = `wavpack::MAX_BLOCK_PCM`) = **65536 i32
  = 256 KiB**, `StaticCell`-backed on the board. **It is sized entirely by
  WavPack.** Per-format scratch carves (embedded-profile, interleaved i32):

  | format | scratch i32 | note |
  |---|---|---|
  | WavPack | 65536 | `MAX_BLOCK_PCM`; sets the shared size |
  | ALAC | 28 672 | `MAX_FRAME*3 + MAX_FRAME*2ch*2` |
  | Opus | 11 520 | `MAX_PACKET_SAMPLES` = 5760*2 |
  | FLAC | 9 216 | `2ch * MAX_BLOCK(4608)` |
  | AAC | 2 048 | `1024 * 2ch` |

- Static footprint: App 174.7 KiB + scratch 256 KiB ≈ 432 KiB; ~592 KiB stack
  headroom. Worst-case decode-open stack transient 315 KiB (fits — see the
  stack-measurement correction doc).

## Backlog (prioritized; value = static-RAM or battery, risk = correctness/scope)

Memory (concrete, measurable now):
1. **Right-size embedded decoder scratch.** WavPack sets 256 KiB; a standard
   WavPack stereo block (ffmpeg: 22050*2 = 44100 i32) needs < 48 K. Cap
   `MAX_BLOCK_PCM` under `embedded-profile` to ~49152 → scratch ~192 KiB (**−64
   KiB**); large-block WavPack declines gracefully (same tradeoff class as the
   capped `MAX_FLAC_FRAME` / dropped surround). *Format-support tradeoff — needs a
   call. Risk: med (shared scratch; full decode matrix must stay green).*
2. **Footprint gate for the scratch** (`dap-arch-tests`): assert
   `decoder_scratch_len()` ≤ budget + print the per-format breakdown, so #1's win
   locks in and any format enlarging the shared scratch fails CI. *Risk: none.*
3. **Second-scratch audit.** Board uses one scratch (PRELOAD=0); confirm no
   `SCRATCH2` equivalent is linked on the board. *Risk: none.*
4. **Alloc-count gate** — assert zero heap allocations across open→decode→pump
   (dap is `forbid(unsafe)` + no allocator, so this is near-free to assert). *none.*
5. **ALAC/Opus/AAC buffer audit** — are any inline buffers oversized for the
   stereo embedded target? *Risk: low.*

Battery (analytical / config / design):
6. **Power-budget model** — doc + a host test computing expected playback hours
   from battery capacity, H7A3 RUN/Stop current (datasheet), and an RTF-derived
   sleep fraction; makes the race-to-sleep target quantitative. *Risk: none.*
7. **Board clock/VOS recommendation** — pick the lowest freq+VOS that sustains
   real-time decode once RTF is known. *Answered in L11:* VOS2 (160 MHz) covers
   every shipped format and the resample fallback; VOS3 (88 MHz) covers wav/mp3
   only. The transition *sequence* is now gated too (`power::clocks`), so the
   board's remaining job is executing the plan's steps, not deciding them.
8. **Stop-mode-on-pause design** — enter Stop (32 µA, RAM retained) when paused;
   wake on the scroll-wheel/button EXTI. *Phase E.*
9. **DMA SAI sink design** — double-buffered DMA so the core sleeps between
   blocks. *Phase E.*
10. **E-ink partial-refresh policy** — minimize full refreshes (power + ghosting);
    the UI already tracks dirty regions — gate "no full refresh on a 1-field
    change". *Risk: low.*

Infra / gates:
11. RTF-regression gate (desktop proxy, scaled) — guard decode efficiency.
12. Wire footprint/alloc/stack gates into CI (QEMU stack gate needs a runner).
13–20. Per-format RTF profiling, resampler throughput, EQ cost, index-read
    footprint, buffer-reuse audits, const-assert budgets, etc. (expanded as the
    loop surfaces them — self-improving.)

## Loop log

- **L0 (this doc)** — research + measured baseline + backlog. Committed.
- **L1 — realistic battery model (output path).** Found the mature
  `dap-core::power::model` was **CPU-only** — its `runtime_hours` omitted the
  always-on DAC/amp + SD + display draw, making the estimates optimistic (WAV
  ~371h). Added `DAC_AMP_PLAYBACK_MA` (7 mA — CS43131 ~23 mW playback / 3.7 V,
  from the project's DAC decision), `SD_AVG_MA` (2), `DISPLAY_AVG_MA` (0.5),
  `PERIPHERAL_PLAYBACK_MA` (9.5), and `system_current_ma(cpu_avg)`. New gate
  `system_runtime_is_realistic_and_below_cpu_only` asserts the total-system
  runtime clears 30h AND is strictly below the CPU-only figure for all 7 formats
  (so a dropped peripheral term regresses). WAV ~371h → realistic ~130h. 11/11
  model tests + 36/36 dap-core green. *Finding: the CS43131 is efficient enough
  (~7 mA) that CPU and DAC are comparable — neither dominates — so both CPU DVFS
  AND decode efficiency matter for battery, validating the race-to-sleep focus.*
- **L2 — resolve the DVFS "race-to-sleep" open question.** `power::dvfs` had
  explicitly flagged (in `BATTERY_SAVER_SAFETY`'s doc) that a race-to-sleep policy
  (burst at max clock, WFI longer) "is a different, not-strictly-dominated
  strategy the model can represent but this doesn't implement." Answered it with
  `system_avg_current_at_clock(rtf, ref, clock)` + a gate
  `min_clock_not_beaten_by_race_to_sleep`: for **every** format, total-system
  current at the minimum sustaining clock is ≤ at max clock — **race-to-sleep does
  NOT win for this DAP**. Why: the H7A3 current-vs-clock curve is ~linear (no
  super-linear high-clock penalty to exploit), low-clock active current is cheap,
  and the ~9.5 mA output path is on either way. So `BatterySaver`'s minimize-clock
  is already near energy-optimal — a high-clock-burst DVFS policy would not help.
  Updated the dvfs doc to mark the refinement analyzed + answered. 25/25 power
  tests green. *The L1 DAC model (constant output path) is exactly what made this
  analyzable — it turned a hand-wavy "maybe race-to-sleep" into a gated answer.*
- **L3 — datasheet cross-checks (SLEEP placeholder + Run floor + Stop pause).**
  Web-researched the STM32H7A3 datasheet (DS13677): it gives **120 µA/MHz**
  best-case Run (periph-off, SMPS) and **Stop = 32 µA**, but NO Sleep-with-SAI-DMA
  row — so `SLEEP_CURRENT_MA`'s placeholder is confirmed as the honest state (not
  faked). Turned the data into a gate `run_currents_stay_above_datasheet_best_case_floor`:
  the model's (conservative, periph-on) Run figures must stay ABOVE the 120 µA/MHz
  physical floor, so nobody "optimizes" them into under-estimating current /
  over-estimating runtime. Added `STOP_MODE_CURRENT_MA` (32 µA) — paused drain is
  ~7-year shelf-life, confirming pause→Stop is the right pause policy. 13/13 model
  green.

- **L4 — per-format stack transients** (design §5.1 "stack high-water per
  format", previously only 2 formats measured). Extended `tools/mcu-stackprobe`
  with a re-painted per-format open sweep over the scratch-carving decoders.
  Result (growth below the call site, embedded-profile, fits the 592 KiB
  headroom): **Matroska-FLAC 297 · Opus-in-WebM 221 · WAV 77 · WavPack 62 ·
  ALAC-CAF 59 KiB**. Confirms Matroska-FLAC is the worst case and **nothing
  exceeds headroom**; Opus is a notable 2nd (CELT/SILK construction). The gate
  still keys on the worst case (which = the max), now with the full breakdown
  printed. QEMU green.

- **L5 — native-rate clocking avoids the resampler** (ties the DAC research to
  the power model). The CS43131 takes both rate families via a dual MCLK
  oscillator (22.5792 MHz→44.1k, 24.576 MHz→48k), so the firmware clocks the SAI
  at the source's native rate and skips the resampler for all standard content —
  and resampling is the heaviest pipeline stage. Gate
  `native_rate_clocking_needs_lower_clock_than_resampling`: worst-case raw decode
  needs ~102 MHz vs ~158 MHz for resample-to-fixed-rate → native-rate is a real
  power win, so the firmware should prefer it over a fixed-DAC-rate path.
  Corrected the stale "DAC's fixed output rate" comment. 14/14 model green.
  **Verified end-to-end:** the Deck already skips the resampler at matching rates
  (`make_resampler` → `None` when `src_rate == sink_rate`; `resampler:
  Option<Resampler>`), so native-rate clocking builds NO resampler — saving its
  CPU *and* its memory. The only remaining piece is the board (Phase E) setting
  the SAI/sink rate to the source's native family + switching the MCLK oscillator
  — the actionable Phase-E clocking recommendation this loop produces.
- **L6 — datasheet calibration (real figures, not placeholders).** Web-researched
  the authoritative sources and replaced estimates: **battery capacity 2000 →
  1100 mAh** (the actual sourced pack is the Akyga LP623255 / AKY0646, 3.7 V,
  1100 mAh / 4.07 Wh — the 2000 mAh default had over-estimated runtime by ~82 %).
  Grounded the CS43131 draw against its datasheet (DS1155F2: 6.25–40.2 mW
  operational, 576 µA standby → ~6–11 mA battery depending on volume/32 Ω load;
  7 mA moderate stands) and the microSD draw (20–100 mA read / 250 µA sleep →
  ~2 mA average). Realistic runtime at the real pack: **~41 h (opus) to ~74 h
  (wav)** total-system; paused ≈ 4 years (Stop mode). 14/14 model tests still
  green (floors hold at the lower capacity). *The single highest-impact accuracy
  fix in the battery model — it was silently ~2× optimistic on the real pack.*

- **L7 — tweakable, thermal- and regulator-aware `PowerParams`** (#30). Gathered
  every tunable (pack, temperature, sleep current, output-path currents,
  regulator) into one struct with a `Default` reproducing the const baseline, and
  folded in two physical effects the bare consts omitted: Li-Po capacity derates
  with temperature (`capacity_temp_derate`), and the battery→chip regulator is
  lossy. Both always *cost* runtime, never add it — gated
  (`params_runtime_is_lower_than_the_bare_model`).
- **L8 — load-dependent regulator efficiency curve** (#34). The flat efficiency
  factor from L7 was wrong in a way that flattered the DAP: a buck's efficiency
  is a *curve*, and the DAP's ~15–25 mA playback draw sits on its **light-load
  flank**, where fixed switching/control loss dominates. Replaced with
  `I_batt(I) = I/η∞ + fixed_loss + cond·I²` (three datasheet-fittable params),
  so efficiency craters at tiny load, peaks at `I* = sqrt(fixed/cond)` (~61 mA
  default), then droops again — η ≈ 0.85 at the DAP's 20 mA. New
  `regulator_efficiency_at` / `regulator_peak_load_ma`, plus a curve-shape gate.
- **L9 — the dynamic stack measurement became a real gate** (#33). The QEMU
  open→decode→pump high-water was *measured but only reported*; the static
  `-Z emit-stack-sizes` gate was the sole blocking check, and it undercounts a
  deep construction chain. Wired the dynamic measurement in as CI-blocking —
  closing the measure→enforce half of the stack loop.
- **L10 — whole-firmware-tick stack gate** (#35). The decode gate measures
  soul-audio-core's open path *in isolation* (~297 KiB), but the board reaches it
  through `App::tick → AudioPlayer::pump → Engine::poll_open` and then runs
  `App::render` in the same tick. New `mcu-tickprobe` wires a real `App` exactly
  as the board does (with an in-memory Matroska-FLAC fixture, since `StubStorage`
  never decodes): whole-tick worst is **413 KiB**, ~116 KiB above the isolated
  number, fitting the 592 KiB headroom with ~179 KiB spare. *This* is the number
  that answers "does the firmware fit its stack?".

- **L11 — clock sequencing: the one Phase-E item that didn't need the board.**
  The enforcement roster listed "Clock sequencing — freq ↓ → flash wait-states →
  VOS ↓" as 🔜 Phase E, needing "a tested clock-transition routine". But the
  ordering rule is *pure logic* over two datasheet tables — only the register
  writes need silicon. New `dap-core::power::clocks` transcribes ST's own H7A3
  tables (`stm32h7xx_ll_utils.c`, `STM32H7_DEV_ID == 0x480`): per-scale clock
  ceilings and the flash wait-state ladder. It exposes `ClockPoint::for_clock`
  (the canonical minimum-voltage operating point — only constructible from a
  clock, so an inconsistent point is *unrepresentable* and the clock→voltage
  mapping stays monotone) and `plan(from, to) → ClockPlan`, an ordered
  `heapless::Vec` of at most three `ClockStep`s.
  **What's gated:** rather than asserting the ordering rule on faith, the two
  underlying invariants are checked at *every* step boundary of *every*
  transition pair over a clock set covering both sides of all 22 wait-state
  thresholds: (1) programmed wait-states ≥ what the live clock+voltage requires
  (under-waiting corrupts instruction fetch), (2) live clock ≤ the live VOS's
  ceiling (the "VOS-before-freq" hard-fault anti-pattern). The familiar
  down/up asymmetry then *falls out* of the invariants. Plus a seam gate —
  every clock `dvfs::clock_mhz_for_profile` can return, for every profile ×
  format, must be a datasheet-legal point reachable by a safe plan.
  **Finding (a real bug, not a gap):** the VOS ceilings this repo had been
  using were the **H743's**, inherited from the v1-era design doc §4.2 ("VOS3
  1.0 V/200 MHz"). The H7A3's are **280/225/160/88 MHz**. So
  `clock_headroom_per_format_fits_vos3_target` was asserting `≤ 200 MHz` under a
  label that was wrong by a factor of >2, and its headline claim — "the DAP can
  run the lowest voltage state for every shipped format" — is **false**: the
  heaviest format (opus, ~102 MHz) needs **VOS2**; only wav/mp3 reach VOS3.
  Replaced with gates that pin the true band per format
  (`heaviest_format_needs_vos2_not_vos3`, which fails in *both* directions — a
  decoder speedup that newly unlocks VOS3 is a power win worth noticing).
  Second finding: the resample **fallback** path needs ~158 MHz against VOS2's
  160 MHz ceiling — a ~2 MHz margin, now gated, so a couple of percent of
  resampler slowdown can't silently push the off-family-rate path up a voltage
  scale. 57/57 dap-core, 11/11 new; clippy/fmt/arch-boundary/forbid-unsafe green.

- **L12 — peripheral clock gating: the policy *and* the audit, host-gated.**
  Applying L11's lesson again (see Status): the roster's "gate unused APB/AHB
  clocks explicitly (not NOPs) — enable-register discipline; audit unused
  peripherals" also needed the board only to *execute*. New
  `dap-core::power::peripherals`:
  - **Discipline as a type property.** Every enabled clock must name a concrete
    `(RCC_*ENR, bit)` coordinate, so the anti-pattern the roster names
    ("software-NOP clock gating — use the controller") is *unrepresentable*
    rather than discouraged.
  - **The audit as a completeness property.** The module works in masks, not
    ~120 enum variants, because the gated set is **derived**:
    `gated = gateable & !enabled`. Not being on the roster *is* a peripheral's
    classification — off — so nothing can be left unclassified, and turning a
    clock on is only possible by adding a roster entry, which forces a per-state
    decision and a written justification at the same time. The gateable masks per
    bus come from `stm32-metapac` v21's `rcc_h7ab` block (the same register
    definitions `embassy-stm32` drives this part through).
  - **Three power states** (`Playback` / `Idle` / `Paused`, the last being the
    pre-Stop configuration), gated to nest **strictly**: `Paused ⊂ Idle ⊂
    Playback`, proper subsets — so "lower power state" is a fact, not a label.
  **Result:** of the part's **119** gateable peripheral clocks, **12** are on in
  Playback, **8** in Idle, **4** in Paused — ~90 % of the chip unclocked even at
  full tilt. Gates beyond the partition: the audio path (SAI1 + DMA1 + SDMMC1 +
  its AF bank) is enabled *only* in Playback; `Paused` keeps nothing but the four
  named wake/core clocks and no output-path peripheral (Stop's 32 µA budget
  cannot absorb one); only a wake-carrying GPIO bank survives into Stop, and its
  justification string must say which wake line; and a `MUST_STAY_GATED` tripwire
  pins USB OTG HS/FS, FMC, OCTOSPI1/2, SRAM1-3 and DTCM/ITCM off in every state —
  USB/BT are deferred past v2 and `memory.x` links AXI SRAM only, so enabling one
  has to be a deliberate edit, not a quiet roster addition. Also a seam gate to
  the battery model: every device `PERIPHERAL_PLAYBACK_MA` bills for (CS43131, SD,
  display) must have its bus clock enabled in Playback — otherwise the two modules
  describe different machines. 68/68 dap-core (11 new); full `xtask verify` green.
  *Two inputs are flagged placeholders, not findings: the GPIO bank assignment and
  the SPI/I²C instance choice, both pending the schematic. The gates are written
  over the policy, so they survive the schematic renaming either.*

## Status

**L0–L12 landed**, all gated. Four lessons, in rough order of how much backlog
each one cleared:

1. **"Needs the board" usually means "needs the board to *execute*."** L11 and
   L12 were both filed as Phase E and both turned out to be pure decisions —
   a transition *ordering* and a gating *policy* — with only the register writes
   needing silicon. Separating the decision from the write is what moved them.
   Apply this before accepting any remaining "blocked on hardware" label.
2. **Check inherited numbers against the actual part.** L6 caught a 2×
   optimistic battery capacity; L11 caught H743 voltage tables masquerading as
   H7A3's. Neither was a code bug, and both erred in the *flattering* direction.
   Anything still carried over from the v1 design docs deserves the same pass.
3. **Promote measurements from reported to CI-blocking** (L9/L10) — a measured
   number that nothing enforces is a number that drifts.
4. **The subsystem was already mature**, so the value was never in reinventing it
   — it was in filling gaps the code had *explicitly flagged* (output-path
   current, the race-to-sleep answer, datasheet floors, the regulator curve).

Remaining work, by what actually blocks it:
- **Host-gateable now** — *E-ink partial-refresh policy* (#10; the UI already
  tracks dirty regions, so "no full refresh on a 1-field change" is a gate
  waiting to be written). Best candidate for L13.
- **Needs a decision** — *scratch right-size* (#1, ~64 KiB): the WavPack
  large-block tradeoff, plus a soul-audio-core change.
- **Needs care, not hardware** — *RTF hot-loop / decode-ahead ring*:
  soul-audio-core, correctness-sensitive.
- **Needs the schematic** — the per-pin *GPIO low-power audit*. The bank-level
  policy landed in L12; the pin states need a pinout that does not exist yet.
- **Needs silicon** — the *measurement* itself, the real `SLEEP_CURRENT_MA`, SAI
  DMA, suspend/resume, and executing the clock plan. Note `SLEEP_CURRENT_MA`
  remains the model's dominant sensitivity (gated as such), so every headline
  runtime figure is only as good as that one bench measurement.

## Sources

`stm32-metapac` v21's `rcc_h7ab` RCC block (generated from ST's SVD via
`stm32-data`; the authority for L12's per-bus gateable masks and enable-bit
coordinates); ST `stm32h7xx_ll_utils.c`, `STM32H7_DEV_ID == 0x480` branch (CubeH7's LL
implementation of RM0455's per-scale clock ceilings + flash wait-state table —
the authority for L11's two transcribed tables, and what exposed the inherited
H743 figures); Memfault *Measuring Stack Usage*; ST *STM32H7 System Power
Control (PWR)* &
*AN5307 (H7A3/7B3 HW dev)* & VOS/SVOS scaling; TI *SPRAAW6* (DMA audio race-to-
sleep); IEEE *perception-aware low-power audio decoder*; `static_cell`,
`heapless`, rust-embedded book *Memory management in no_std*.
