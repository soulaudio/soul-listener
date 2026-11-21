# Memory & power: best practices, how to enforce them, and where this DAP stands

Web-researched 2026-11-20 against the embedded-Rust and low-power-firmware canon
(sources at the end), then mapped to this repo. The theme matches the project's
own philosophy: **every rule should be a gate, not a doc** — a lint, a test, a
compile-time assertion, or an on-target measurement that fails CI on regression.

Status key: **✅ enforced** · **🟡 partial / reporting-only** · **🔜 Phase E
(needs the board)**.

## Memory management

| Practice (how to code it) | How to enforce it | This DAP |
|---|---|---|
| **`#![no_std]`, no `alloc`** — no heap: no fragmentation, worst-case is analyzable, no runtime OOM | CI builds the `no_std`/thumbv7em target; grep/deny any `alloc` import | ✅ board cross-build + `forbid(unsafe_code)`; no allocator linked |
| **Zero heap allocation on the hot path** | Counting global allocator in a host test; assert 0 allocations across the real open→decode→pump cycle | ✅ `dap-sim/tests/no_alloc.rs` + `alloc_policy.rs` |
| **Fixed-capacity collections** (`heapless`), const-generic buffers; no `Vec`/`String` | `heapless` only; size is a compile-time fact | ✅ `heapless` (kept patched — bumped 0.9.3→0.9.4 for RUSTSEC-2026-0574/0575) |
| **`forbid(unsafe_code)`** (ANSSI rule: avoid `unsafe` in secure code) | `#![forbid(unsafe_code)]` at each crate root + a CI coverage gate | ✅ every embedded crate + `tools/check-forbid-unsafe.sh` |
| **Stack-overflow protection** (Rust can't prove no stack overflow at compile time) | `flip-link` (puts the stack below statics → overflow faults instead of corrupting) | ✅ board links with flip-link |
| **Bounded worst-case stack** | Static `-Z emit-stack-sizes` gate **and** a dynamic on-target high-water measurement | ✅ `check-stack.sh` (static) + `check-stack-qemu.sh` (QEMU stack-painting; worst-case open ~297 KiB, fits 592 KiB) |
| **Externalize big decoder buffers** to one shared `&'static` scratch (not inline) | `StaticCell` pool, constructed in place (`init_with`, not `init`) | ✅ §7 scratch design; board uses `init_with` |
| **Compile-time budget assertions** (size/layout invariants fail the *build*) | `const _: () = assert!(size_of::<T>() + … <= REGION)`; the `static_assertions` crate (`const_assert!`, `assert_eq_size!`) | ✅ **added this round** — a compile-time AXI-SRAM budget guard in `board-h7a3` (the twin of the runtime footprint test) + a const assert in `power::model` |
| **Static footprint roster** — `size_of` guard per main-stack type | Host test tabulating every component vs a per-region budget | ✅ `dap-arch-tests/tests/budget.rs` |
| **Cap the dominant tunable buffer** — the shared decoder scratch is a quarter of the linked AXI SRAM | An absolute ceiling, not a budget derived from the measured value | ✅ **as of this round** — `SHARED_DECODER_SCRATCH_CAP_I32` (65,536 i32) + `shared_decoder_scratch_stays_within_its_cap`, which also prints which format binds. The previous "budget" was defined *as* `decoder_scratch_len()`, so it capped nothing |

**Verdict:** memory discipline is textbook and comprehensively gated. Two
upgrades across this round: the dominant-statics budget moved from a *runtime
test* to also a *compile-time assertion* (a footprint regression now can't even
link), and the shared decoder scratch gained a real ceiling — its old "budget"
was defined as the measured value, so it capped nothing. Worth noting what that
pattern cost to find, since it recurred four times in this loop (battery
capacity, VOS tables, display current, scratch budget): **a number is only
enforced if something would fail when it changes.** When reading a budget,
ask what would have to break for the assertion to fire.

## Battery / power management

Master lever (confirmed across sources): **race-to-sleep** — minimize *energy per
task* `E = P_active × t_active`, not instantaneous current; CPU asleep by default,
waking only on real events, never polling.

| Practice (how to code it) | How to enforce / validate it | This DAP |
|---|---|---|
| **Tickless idle / WFI by default** — core sleeps at `await`, wakes on IRQ | Framework-level (don't hand-roll WFI loops — they miss wake conditions) | ✅ Embassy executor parks in WFI between ticks |
| **DVFS** — lowest freq+VOS that sustains the workload; `P = α·C·V²·f` | A model mapping measured RTF → min clock per codec; a regression gate | ✅ `power::model` + `power::dvfs`; race-to-sleep analyzed (min-clock is optimal here) |
| **Clock sequencing** — freq ↓ → flash wait-states → VOS ↓ (never VOS before freq) | A tested clock-transition routine; the ordering is a hard rule | ✅ **the routine, as of this round** — `power::clocks` emits the ordered plan and gates both invariants over *every* transition pair (`no_plan_ever_underwaits_flash_or_overclocks_the_voltage`); 🔜 the register writes that execute it |
| **Peripheral clock gating** — gate unused APB/AHB clocks explicitly (not NOPs) | Enable-register discipline; audit unused peripherals | ✅ **the policy + the audit, as of this round** — `power::peripherals`: every enabled clock must name an `(RCC_*ENR, bit)` coordinate (so "NOP gating" is unrepresentable), and the gated set is *derived* (`gateable & !enabled`), so nothing can be left unclassified. 12 of the part's 119 peripheral clocks on in Playback, 8 Idle, 4 Paused; 🔜 the RCC writes |
| **Avoid resampling when possible** — it's the heaviest stage | Native-rate clocking (dual MCLK oscillator); Deck builds no resampler at matching rates | ✅ modeled + gated (`native_rate_clocking_needs_lower_clock_than_resampling`); Deck verified |
| **DMA for audio** — core sleeps between blocks, IRQ per block not per sample | Double-buffered SAI DMA ring sized for a low wakeup rate | 🟡 modeled (`wakeups_per_sec`, ring budget); 🔜 real SAI DMA |
| **Stop mode on pause** (32 µA, RAM retained ≈ shelf-life) | Enter Stop on pause; wake on EXTI; restore clock tree *before* peripherals | 🟡 modeled (`STOP_MODE_CURRENT_MA`) + the pre-Stop clock configuration is now gated (`power::peripherals`' `Paused` state: 4 clocks, wake sources only, gated against any output-path peripheral); 🔜 the real suspend/resume |
| **Define every GPIO in low-power** — unused pins input-no-pull or driven; condition wake pins (pulls/RC/LSE RTC) | Pin-state audit; measure false-wake rate | 🔜 Phase E — **genuinely blocked on the schematic**, not on effort: there is no pin assignment in the repo to audit. The bank-level policy is gated (`power::peripherals`: unused banks gated, only a wake-carrying bank survives into Stop); the per-pin audit needs the pinout |
| **E-ink refresh discipline** — a bistable panel costs nothing to hold an image and ~34 mA while a waveform runs; minimize full refreshes (power + ghosting) | A policy keyed on what changed, with the ghosting bound derived from the panel spec; gate "no full refresh on a 1-field change" | ✅ **as of this round** — `display::refresh` + the wiring in `App::render`. Measured: **3 full refreshes over ten simulated minutes, where the old loop would have run ~600**. Found a live bug doing it (see the correction below) |
| **Battery model grounded in datasheets** | Datasheet currents × measured decode cost; runtime floors as gates | ✅ calibrated to real figures (Akyga LP623255 **1100 mAh**, CS43131 envelope, microSD, and — as of this round — the display's actual refresh budget rather than an assumed one); **~38–67 h** total-system modeled, ~33–57 h once the regulator and thermal derate are folded in |
| **Measure, don't claim** — "unverified power claims are product risks" | Power analyzer (Otii/Monsoon) or shunt+ADC; GPIO markers in traces; soak at temperature | 🔜 needs silicon — the one thing no model substitutes for |

**Verdict:** the *model and policy* are strong and gated, and this round moved
two more rows off Phase E by noticing they needed the board only to *execute*,
not to *decide*:

- **Clock sequencing** → `power::clocks`. The ordering rule is pure logic over
  the datasheet's VOS/wait-state tables, so it is proven on the host and merely
  executed on silicon.
- **Peripheral clock gating** → `power::peripherals`. Which clocks are on in
  which power state is a *policy*, and the "audit unused peripherals" half is a
  *completeness property* — both decidable without hardware.

The pattern is worth naming, since it emptied most of what looked like a
hardware-blocked backlog: **"needs the board" usually means "needs the board to
execute"**. Separate the decision from the register write and the decision can
be gated today. What is left after applying it:

- Genuinely needs silicon: the **measurement** (no model substitutes), the real
  `SLEEP_CURRENT_MA`, SAI DMA, and the suspend/resume code path.
- Genuinely needs the **schematic**: the per-pin GPIO audit — there is no pin
  assignment in the repo to audit yet.

**Correction this round (display):** `App::render` ran a **full 3 s panel
refresh on every render**, and `firmware::step` rendered once per second — a
flash the panel cannot even complete before the next is demanded, at ~34 mA
continuous against the battery model's 0.5 mA display budget (~68x). Every test
asserted `render().is_ok()`; none asserted what a render *cost*. Fixed by the
policy above; `DISPLAY_AVG_MA` is now **2.0 mA**, derived from the policy's own
energy model rather than assumed, and the two are tied together by a seam gate
in `dap-arch-tests`. Total-system runtime drops to ~38–67 h as a result — less
flattering and actually true.

**Correction (earlier this round):** the voltage-scale ceilings this repo had been using
were the **STM32H743's**, inherited from the v1-era design doc §4.2 ("VOS3
1.0 V/200 MHz"). On the actual part (H7A3, DEV_ID `0x480`) they are
**280 / 225 / 160 / 88 MHz**. Consequence: the claim that the DAP can decode
every format in the *lowest* voltage state was wrong — the heaviest format
(opus, ~102 MHz) needs **VOS2**; only the cheap ones (wav, mp3) reach VOS3. The
gates now assert the true bands per format rather than a mislabeled 200 MHz
bound, and the resample fallback path turns out to clear VOS2's ceiling by only
~2 MHz (158 vs 160), which is now itself gated.

## Phase-E low-power bring-up checklist (lock this in when the board exists)

From the researched 8-step protocol, specialized to this DAP:
1. **HW sanity** — battery chemistry, regulator (SMPS vs LDO), VBAT routing, LED draw; baseline quiescent current.
2. **Datasheet drill** — pin the H7A3 Sleep-with-SAI-DMA current (the model's one remaining placeholder), CS43131 draw at target volume/impedance, oscillator startup times.
3. **Minimal baseline** — boot all-peripherals-off, enter deepest viable sleep, measure.
4. **Active-path** — wake → SD read → decode burst → DMA → sleep; apply DVFS + native-rate clocking; gate unused clocks. Both the clock transition and the gating policy are already decided and gated: execute `power::clocks::plan`'s steps in order (each `ClockStep` documents the blocking it implies: `VOSRDY` poll, `FLASH_ACR` read-back, PLL re-lock), and write `power::peripherals::enabled_mask(state, bus)` to each `RCC_*ENR` — rather than hand-rolling either at the register level.
5. **Wake-path hardening** — scroll-wheel/button EXTI, LSE RTC; measure false-wake rate; condition inputs.
6. **Retention** — validate the deck/ring state across pause→Stop→resume (restore clocks before peripherals).
7. **Soak** — days of real duty cycles at temperature; capture `I_avg`, spike distribution, wake-failure stats.
8. **Document & lock** — real energy-per-task, sleep current, `I_avg`, measured battery hours → feed the real numbers back into `power::model` (replacing the estimates) and add an on-target power-regression check.

**Anti-patterns to gate against:** single-instant current ≠ battery life (average over duty cycles); software-NOP clock gating (use the controller); VOS-before-freq transitions (hard faults); floating wake lines; relying on peripheral-register retention through deep sleep.

## Sources
ST `stm32h7xx_ll_utils.c` (`STM32H7_DEV_ID == 0x480` branch — the CubeH7 LL
implementation of RM0455's per-scale frequency ceilings and "FLASH recommended
number of wait states" table; the authority for `power::clocks`' two tables);
ST RM0455 (H7A3/7B3 reference manual) / DS13677;
Embedded Rust Book (`no-std`); Effective Rust Item 33; ANSSI *Programming Rules
to Develop Secure Applications with Rust* (avoid `unsafe`); `flip-link`;
`static_assertions` / `const_assert!`; *Rust for Embedded Systems: Current State
and Open Problems* (arXiv 2311.05063); Embassy docs (async WFI idle); *Low-Power
Firmware Techniques for Battery-Powered MCUs* (dev.to) — sleep modes, `E = P·t`,
measurement protocol, anti-patterns; ST STM32H7 PWR / AN5014/AN5215/AN5307.
