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
  decode. STM32H7A3: VOS0 (280 MHz) … VOS3 (lower); Stop mode = 32 µA, RAM
  retained — the pause/idle target.
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
   real-time decode once RTF is known; documented now, applied when measurable.
   *(Blind RCC changes risk no-boot with no hardware to verify — design-only now.)*
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

## Status (end of session)

Loop infrastructure + backlog established; **L0–L3 landed** (4 commits, all gated).
Finding: the memory/battery subsystem is already **mature** — no-alloc gates,
footprint roster, stack measurement, battery model + DVFS all pre-existed; the
genuine value was filling flagged gaps (realistic output-path current, the
race-to-sleep answer, datasheet floors), not reinventing. Remaining high-value
loops are **gated on decisions/pushes/hardware**, not effort:
- **Scratch right-size** (#1, ~64 KiB): needs the WavPack large-block tradeoff
  decision + the soul-audio-core push (blocked on creds).
- **Per-format stack transients** (extend the probe): doable; needs per-format
  fixtures — next safe loop.
- **RTF hot-loop / decode-ahead ring**: soul-audio-core, correctness-sensitive.
- **Real battery numbers / Sleep current / clock+VOS apply**: need silicon.

## Sources

Memfault *Measuring Stack Usage*; ST *STM32H7 System Power Control (PWR)* &
*AN5307 (H7A3/7B3 HW dev)* & VOS/SVOS scaling; TI *SPRAAW6* (DMA audio race-to-
sleep); IEEE *perception-aware low-power audio decoder*; `static_cell`,
`heapless`, rust-embedded book *Memory management in no_std*.
