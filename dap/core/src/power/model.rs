//! Battery **energy model** — pure, `#![no_std]`-safe math over datasheet
//! run-mode currents and measured decode real-time-factors (RTFs).
//!
//! # What this is — and is not
//! This is a **relative, datasheet-grounded estimate**, built from two
//! inputs: (a) the STM32H7 run-mode current table below (§1.1 of
//! `docs/plans/2026-11-16-memory-and-battery-optimization-design.md`,
//! citing ST AN5014/AN5215/AN5307), and (b) this DAP's own measured decode
//! RTFs (`dap-sim/benches/decode_rtf.rs`, HOST-measured — see that file's
//! module doc for the host/target gap). It exists to **drive the DVFS
//! design** (which clock sustains which codec, with how much headroom —
//! see `power::dvfs`, built on top of this module) and to **catch
//! regressions** (a decoder/resampler slowdown moves these numbers) — it is
//! explicitly **not** a measurement of real battery life. Real runtime
//! needs silicon (`board-h7a3`) and a coulomb-counter on an actual pack;
//! nothing here substitutes for that. Every function is pure (no I/O, no
//! hardware, no global state) so it runs identically on host tests and (if
//! ever needed) on-target.
//!
//! # Tweaking it — [`PowerParams`]
//! The module `const`s are the datasheet-grounded baseline, but every tunable is
//! also gathered into [`PowerParams`] (with a `Default` that reproduces the
//! baseline) so a caller can re-run the whole estimate for a different pack,
//! **operating temperature**, regulator, or a bench-measured current without
//! editing consts. `PowerParams` also folds in two physical effects the bare
//! `const`s omit, which make the estimate realistic (and conservative): Li-Po
//! capacity **derates with temperature** ([`capacity_temp_derate`]), and the
//! battery→chip **regulator is lossy**, and lossy in a *load-dependent* way: a
//! buck's efficiency traces a curve, not a constant — it craters at light load
//! (fixed switching/control loss dominates a small output), peaks in the
//! tens-of-mA range, then droops again at heavy load (I²R conduction) — so the
//! battery supplies `chip/η∞ + fixed_loss + cond·chip²`, with efficiency falling
//! off on BOTH sides of the peak. The DAP's ~15–25 mA playback draw sits on the
//! rising (light-load) flank, so this costs real runtime the old flat factor
//! hid (see [`PowerParams::battery_current_ma`] /
//! [`PowerParams::regulator_efficiency_at`]).
//!
//! # The run-mode current table (§1.1)
//! Four measured/representative STM32H7-family operating points, spanning
//! this board's (`STM32H7A3ZI`) full clock range plus two higher points from
//! the broader H7 family (H743/H753 go up to 400/480 MHz where the H7A3 caps
//! at 280 MHz — see [`H7A3_MAX_CLOCK_MHZ`]) used only to characterize the
//! *shape* of the current-vs-clock curve [`active_current_ma_for_clock`]
//! interpolates against, never as a clock this board can actually select.
//!
//! # The sleep (WFI) current — an estimate, not a pinned datasheet row
//! [`SLEEP_CURRENT_MA`] is the one number in this module that is **not**
//! read off a specific datasheet table row. ST's own STM32H7 Sleep-mode
//! tables (AN5014 §2 covers the Sleep/Stop/Standby state machine; the
//! per-part datasheet carries the actual current table) show Sleep mode
//! gates only the CPU core clock — AHB/APB buses, the SAI peripheral, its
//! DMA controller, and flash stay clocked, which is exactly what lets the
//! SAI DMA keep streaming the output ring while the CPU is parked in WFI
//! (design doc §4.1's power loop) — so Sleep current is well below Run
//! current at the same clock, but not negligible. In the absence of a
//! pinned H7A3-specific Sleep-mode figure (the datasheet's electrical
//! characteristics tables were not extractable from the web research this
//! module was built from — see the design doc's sourcing), this constant is
//! a conservative **engineering estimate**: comfortably below every Run
//! entry in the table (including the lowest, 60 MHz), consistent with
//! Sleep-mode-vs-Run ratios documented elsewhere in the H7 family. Treat it
//! as a placeholder to replace with a real datasheet row or a bench-measured
//! figure once `board-h7a3` exists; every test in this module that depends
//! on it says so.
#![allow(clippy::doc_markdown)]

/// Run current @ 60 MHz, mA — the lowest measured/representative point
/// (design doc §1.1 table, citing ST AN5014/AN5215/AN5307). This is the
/// H7A3's own practical floor: the lowest clock any DVFS policy built on
/// this model should ever select (see [`IDLE_CLOCK_MHZ`]).
pub const RUN_CURRENT_MA_AT_60MHZ: f32 = 13.5;
/// Run current @ 280 MHz, mA — the H7A3's own rated maximum clock (see
/// [`H7A3_MAX_CLOCK_MHZ`]; mirrors `dap-arch-tests`' `memory_map::CPU_MHZ`).
pub const RUN_CURRENT_MA_AT_280MHZ: f32 = 58.0;
/// Run current @ 400 MHz, mA — above this board's rated ceiling; a broader-
/// H7-family curve point (see module doc).
pub const RUN_CURRENT_MA_AT_400MHZ: f32 = 90.0;
/// Run current @ 480 MHz, mA — above this board's rated ceiling; a broader-
/// H7-family curve point (see module doc).
pub const RUN_CURRENT_MA_AT_480MHZ: f32 = 125.0;

/// The run-mode current table as (clock MHz, current mA) pairs, ascending by
/// clock — what [`active_current_ma_for_clock`] linearly interpolates
/// across. Four points only; see each `RUN_CURRENT_MA_AT_*MHZ` const's doc
/// for provenance.
pub const RUN_CURRENT_TABLE_MHZ: [(u32, f32); 4] = [
    (60, RUN_CURRENT_MA_AT_60MHZ),
    (280, RUN_CURRENT_MA_AT_280MHZ),
    (400, RUN_CURRENT_MA_AT_400MHZ),
    (480, RUN_CURRENT_MA_AT_480MHZ),
];

/// This board's (`STM32H7A3ZI`) actual rated maximum CPU clock, MHz.
/// Duplicated from `dap-arch-tests/tests/budget.rs`'s `memory_map::CPU_MHZ`
/// rather than imported: `dap-arch-tests` is a dev-only test crate one layer
/// *above* `dap-core` in the dependency graph (it depends on `dap-core`, not
/// the reverse), so `dap-core` cannot reach into it. Keep the two in sync —
/// both cite the same ST `STM32H7A3ZI` datasheet (DS13677).
pub const H7A3_MAX_CLOCK_MHZ: u32 = 280;

/// Lowest clock (MHz) a DVFS policy built on this model should ever select
/// — the bottom of the measured current table ([`RUN_CURRENT_MA_AT_60MHZ`])
/// and the design doc §1.2's "low base clock for idle/UI" floor.
/// Extrapolating the current table below this point isn't attempted (no
/// measured/representative data point exists there).
pub const IDLE_CLOCK_MHZ: u32 = 60;

/// Estimated Sleep-mode (WFI, SAI DMA still streaming) current, mA — see the
/// module doc's dedicated section: this is a conservative placeholder, not a
/// pinned datasheet row. (The STM32H7A3 datasheet gives Run and Stop figures
/// but no Sleep-with-SAI-DMA row — see [`DATASHEET_RUN_UA_PER_MHZ_BEST`] /
/// [`STOP_MODE_CURRENT_MA`] for the two it does give, used as cross-checks.)
pub const SLEEP_CURRENT_MA: f32 = 5.0;

/// Stop-mode current (CPU + most clocks stopped, SRAM retained), mA — the DAP's
/// PAUSE target: STM32H7A3 datasheet DS13677 gives **32 µA typical**. At
/// [`DEFAULT_BATTERY_MAH`] (1100 mAh) that's ~4 years, so paused drain is
/// effectively battery shelf-life, not a playback concern (pause → Stop, don't
/// keep decoding). Resuming repaints the ring and restarts the SAI DMA.
pub const STOP_MODE_CURRENT_MA: f32 = 0.032;
// Compile-time invariant: paused (Stop) drain is negligible vs even the Sleep
// estimate — a floor the two constants must keep relative to each other.
const _: () = assert!(STOP_MODE_CURRENT_MA < SLEEP_CURRENT_MA / 100.0);

/// STM32H7A3 datasheet **best-case** Run slope, µA/MHz (VDD 3.3 V, 25 °C,
/// peripherals OFF, SMPS) — 120 µA/MHz per DS13677. Used only as a *physical
/// floor* cross-check (see `run_currents_stay_above_datasheet_best_case_floor`):
/// [`RUN_CURRENT_TABLE_MHZ`]'s figures are deliberately higher (peripherals on,
/// conservative) and must stay at/above this line so the model never
/// *under*-estimates current / *over*-estimates runtime. Not the model's own curve.
pub const DATASHEET_RUN_UA_PER_MHZ_BEST: f32 = 120.0;

/// Single-cell battery capacity, mAh — the actual pack this DAP sources: the
/// **Akyga LP623255** (AKY0646), a 3.7 V Li-Po pouch, **1100 mAh / 4.07 Wh**,
/// 6.2 × 32 × 55 mm (akyga.com; see soul-listener `sourcing/`). Earlier revisions
/// of this model used a generic 2000 mAh placeholder, which over-estimated runtime
/// by ~82 %; this is the real pack. Change this one `const` for a different cell.
pub const DEFAULT_BATTERY_MAH: f32 = 1100.0;

// --- Output-path (non-CPU) playback current ---------------------------------
// The functions above model the *CPU* (run/sleep) current, which is what drives
// the DVFS clock choice. But a DAP's battery also feeds the DAC/amp, SD reads,
// and the display the whole time it plays — omitting them makes `runtime_hours`
// above optimistic (most so for the cheap formats, where the CPU current is
// lowest and the near-constant output path dominates). These three constants let
// `system_current_ma` fold that always-on draw in. Like `SLEEP_CURRENT_MA`, they
// are datasheet-grounded *estimates* at a ~3.7 V single-cell nominal, to replace
// with bench-measured figures once `board-h7a3` + the analog section exist.

/// Cirrus **CS43131** (the selected integrated DAC + ground-centered Class-H amp —
/// see soul-listener `docs/research/2026-11-18-dac-amp-selection.md` §1–2) playback
/// current at the battery rail. Cirrus DS1155F2: operational power **6.25–40.2 mW**
/// (Class-H, no separate HV rail), **576 µA** standby, and a 32 Ω-load playback
/// condition summing ~26 mA across its internal rails. For the integrated
/// single-supply board design the battery-input draw is the chip's total power /
/// ~3.7 V ≈ **6 mA at moderate listening, up to ~11 mA near full 32 Ω output**.
/// Modeled at **7 mA** (moderate — the realistic average); it does NOT dwarf the
/// CPU but is a real always-on draw. Replace with a bench measurement at the
/// target volume/impedance once the analog section exists.
pub const DAC_AMP_PLAYBACK_MA: f32 = 7.0;

/// microSD current averaged over playback. Reads are bursty (decode-ahead ring
/// refills, then idle): active read is ~20–25 mA (SPI low-power) up to ~100 mA
/// (25 MHz SD mode), sleep ~250 µA (measured ranges, multiple cards). At a low
/// read duty (seconds of audio buffered per refill) the time-average is small —
/// modeled at a conservative **~2 mA**.
pub const SD_AVG_MA: f32 = 2.0;

/// E-ink display current averaged over playback. The panel is bistable — ~0 mA
/// holding a static image; only the brief refresh on a now-playing/progress
/// change draws, averaging to a negligible **~0.5 mA**.
pub const DISPLAY_AVG_MA: f32 = 0.5;

/// Total non-CPU output-path current during playback (DAC/amp + SD + display) at
/// the battery rail — the always-on draw `system_current_ma` adds to the CPU blend.
pub const PERIPHERAL_PLAYBACK_MA: f32 = DAC_AMP_PLAYBACK_MA + SD_AVG_MA + DISPLAY_AVG_MA;

/// How much faster the HOST this crate's tests/benches run on is than the
/// 280 MHz Cortex-M7 target, per `dap-sim/benches/decode_rtf.rs`'s own
/// module doc ("roughly 10-50x faster"). This module takes the
/// **pessimistic** (high) end of that range when projecting a host-measured
/// RTF onto the target — i.e. it assumes the *worst* plausible host/target
/// gap, so [`estimate_target_rtf`]'s output is a conservative (lower-bound)
/// target RTF, never an optimistic one. This is still a projection, not a
/// measurement: the real number needs `board-h7a3` silicon.
pub const HOST_TO_TARGET_DERATE: f32 = 50.0;

/// Project a HOST-measured RTF (`dap-sim/benches/decode_rtf.rs`) onto the
/// target clock's RTF, using the pessimistic [`HOST_TO_TARGET_DERATE`].
/// E.g. a HOST RTF of 250x becomes an estimated target RTF of 5x. Still a
/// model, not a measurement — see the module doc.
#[must_use]
pub fn estimate_target_rtf(host_rtf: f32) -> f32 {
    host_rtf / HOST_TO_TARGET_DERATE
}

/// `ceil`, without `f32::ceil` (`std`-only in `no_std` — see
/// `soul-audio-core/src/resample.rs`'s `libm_ceil`/`libm_floor` for the same
/// house-style bit-trick, used here instead of pulling in `libm`). Exact for
/// the finite, non-negative, `u32`-range values this module calls it with
/// (an `as u32` cast on a negative `x` saturates to `0`, which still
/// satisfies "the smallest integer `>= x`" degenerately).
fn ceil_to_u32(x: f32) -> u32 {
    let truncated = x as u32;
    if x > truncated as f32 {
        truncated + 1
    } else {
        truncated
    }
}

/// The lowest clock (MHz) that still sustains **>=1x real-time decode**,
/// given a measured/estimated RTF at a known reference clock, with a safety
/// margin: `ceil(ref_mhz / rtf_at_ref_clock * safety)`.
///
/// Model: decode cost is treated as a fixed number of CPU cycles per second
/// of audio (clock-independent, to first order — ignores flash wait-state/
/// cache effects that would bite harder at very low clocks; a known
/// simplification of this model, not reality). Wall time to decode 1 s of
/// audio then scales as `1 / rtf`, and `rtf` itself scales linearly with
/// clock, so the minimum clock sustaining `rtf == safety` (a safety margin
/// `> 1.0` above the bare 1x real-time point) is `ref_mhz / rtf_at_ref_clock
/// * safety`, rounded up to a whole MHz.
///
/// `rtf_at_ref_clock <= 0.0` is treated as "no usable decode-cost
/// information" and degrades to `ref_mhz` (assume the whole reference clock
/// is needed — the conservative, not the optimistic, fallback).
#[must_use]
pub fn min_clock_mhz_for_rtf(rtf_at_ref_clock: f32, ref_mhz: u32, safety: f32) -> u32 {
    if rtf_at_ref_clock <= 0.0 {
        return ref_mhz.max(1);
    }
    let needed = ref_mhz as f32 / rtf_at_ref_clock * safety;
    ceil_to_u32(needed).max(1)
}

/// The CPU cycles needed per second of audio, at the given reference clock's
/// measured/estimated RTF — the clock-invariant "decode cost" [`duty_cycle`]
/// takes, derived from the same model [`min_clock_mhz_for_rtf`] uses (cost
/// in cycles/s = `ref_hz / rtf_at_ref_clock`, independent of which clock the
/// CPU actually runs the decode at).
#[must_use]
pub fn decode_cost_cycles_per_sec(rtf_at_ref_clock: f32, ref_mhz: u32) -> f64 {
    let ref_hz = f64::from(ref_mhz) * 1.0e6;
    if rtf_at_ref_clock <= 0.0 {
        return ref_hz;
    }
    ref_hz / f64::from(rtf_at_ref_clock)
}

/// Fraction of wall-time the CPU is actively decoding — the rest is WFI
/// sleep while the SAI DMA plays the already-decoded ring (design doc §4.1's
/// power loop). `decode_cost_cycles_per_sec` is the clock-invariant cost
/// (see [`decode_cost_cycles_per_sec`]); `cpu_hz` is the clock the CPU
/// actually runs the decode at. Clamped to `[0.0, 1.0]`: a `duty_cycle` of
/// `1.0` means the CPU never sleeps (the chosen clock barely/doesn't sustain
/// real-time).
#[must_use]
pub fn duty_cycle(decode_cost_cycles_per_sec: f64, cpu_hz: f64) -> f32 {
    if cpu_hz <= 0.0 {
        return 1.0;
    }
    (decode_cost_cycles_per_sec / cpu_hz).clamp(0.0, 1.0) as f32
}

/// Linearly interpolate the measured run-mode current
/// ([`RUN_CURRENT_TABLE_MHZ`]) at an arbitrary clock. Clamped at both ends
/// (below [`IDLE_CLOCK_MHZ`]'s 60 MHz point: flat at the 60 MHz current;
/// above 480 MHz: flat at the 480 MHz current). Linear interpolation
/// between adjacent measured points is a simplification (current-vs-
/// frequency is not perfectly linear across a VOS boundary in reality) but
/// a reasonable one for a relative model with only four measured points.
#[must_use]
pub fn active_current_ma_for_clock(clock_mhz: u32) -> f32 {
    let table = RUN_CURRENT_TABLE_MHZ;
    if clock_mhz <= table[0].0 {
        return table[0].1;
    }
    let last = table.len() - 1;
    if clock_mhz >= table[last].0 {
        return table[last].1;
    }
    let mut i = 0;
    while i < last && clock_mhz > table[i + 1].0 {
        i += 1;
    }
    let (f0, c0) = table[i];
    let (f1, c1) = table[i + 1];
    let t = (clock_mhz - f0) as f32 / (f1 - f0) as f32;
    c0 + t * (c1 - c0)
}

/// Blended average current, mA: `duty * active_ma + (1 - duty) * sleep_ma`.
/// `duty` is clamped to `[0.0, 1.0]` defensively (callers should already be
/// passing a clamped [`duty_cycle`] output).
#[must_use]
pub fn avg_current_ma(duty: f32, active_ma: f32, sleep_ma: f32) -> f32 {
    let duty = duty.clamp(0.0, 1.0);
    duty * active_ma + (1.0 - duty) * sleep_ma
}

/// Estimated runtime, hours: `capacity_mah / avg_ma`. `avg_ma <= 0.0`
/// degrades to `f32::INFINITY` (no discharge current modeled — avoids a
/// div-by-zero without pretending to know a real answer).
#[must_use]
pub fn runtime_hours(capacity_mah: f32, avg_ma: f32) -> f32 {
    if avg_ma <= 0.0 {
        return f32::INFINITY;
    }
    capacity_mah / avg_ma
}

/// Full-system average playback current, mA: the CPU blend ([`avg_current_ma`])
/// plus the always-on output path ([`PERIPHERAL_PLAYBACK_MA`]). This is the
/// realistic total a DAP draws while playing — `runtime_hours(capacity, this)`
/// gives a total-system runtime, where `runtime_hours(capacity,
/// avg_current_ma(..))` alone gives only the (optimistic) CPU-only figure.
#[must_use]
pub fn system_current_ma(cpu_avg_ma: f32) -> f32 {
    cpu_avg_ma + PERIPHERAL_PLAYBACK_MA
}

/// Full-system average playback current, mA, **if the decode runs at
/// `clock_mhz`**: derive the duty cycle from the codec's `rtf_at_ref_clock`,
/// blend the CPU run/sleep current, and add the always-on output path. Average
/// current over a fixed playback time is the charge drawn (energy proxy), so
/// comparing this across clocks for one codec *is* the
/// race-to-sleep-vs-minimum-clock question (`power::dvfs`'s flagged, previously
/// out-of-scope refinement). Finding (see
/// `min_clock_not_beaten_by_race_to_sleep`): for this DAP's ~linear
/// current-vs-clock curve, running at max clock then sleeping (race-to-sleep)
/// does NOT draw less than the minimum sustaining clock — so `BatterySaver`'s
/// minimize-clock policy is already near energy-optimal, no high-clock burst
/// strategy beats it here.
#[must_use]
pub fn system_avg_current_at_clock(rtf_at_ref_clock: f32, ref_mhz: u32, clock_mhz: u32) -> f32 {
    let cost = decode_cost_cycles_per_sec(rtf_at_ref_clock, ref_mhz);
    let duty = duty_cycle(cost, f64::from(clock_mhz) * 1.0e6);
    let cpu = avg_current_ma(
        duty,
        active_current_ma_for_clock(clock_mhz),
        SLEEP_CURRENT_MA,
    );
    system_current_ma(cpu)
}

/// DMA half-transfer + transfer-complete interrupt rate, Hz: a circular
/// double-buffered DMA ring fires **two** IRQs (half + complete) per full
/// ring drain, and the ring drains at `sample_rate` frames/s, so
/// `wakeups_per_sec = 2 * sample_rate / ring_frames`. Bigger `ring_frames`
/// ⇒ fewer wakeups ⇒ longer uninterrupted CPU sleep between refills (design
/// doc §0/§1.1's "bigger decode-ahead ring ⇒ less energy" thesis). Degrades
/// to `f32::INFINITY` at `ring_frames == 0` (an ill-defined ring, not a
/// real zero-wakeup state).
#[must_use]
pub fn wakeups_per_sec(sample_rate: u32, ring_frames: u32) -> f32 {
    if ring_frames == 0 {
        return f32::INFINITY;
    }
    2.0 * sample_rate as f32 / ring_frames as f32
}

// === Tweakable, thermal- and regulator-aware parameter bundle ===============
//
// The functions above work off module `const`s (the datasheet-grounded baseline).
// `PowerParams` gathers every tunable into ONE struct so a caller can re-run the
// whole estimate for a different pack, operating temperature, regulator, or a
// bench-measured current — without editing consts. `Default` reproduces the
// baseline; override a field and the estimate follows. It also folds in two
// physical effects the bare `const`s omit, which make the estimate *realistic*
// (and conservative): Li-Po capacity derates with temperature, and the
// battery→chip regulator is lossy — most so at the DAP's light playback load.

/// Usable-capacity fraction of a single-cell Li-Po at `temp_c`. Piecewise-linear
/// over the researched shape: ~**0.80 at 0 °C** rising to **1.00 at 25 °C**, flat
/// 25–45 °C (the DAP's operating band), with a mild further droop below 0 °C
/// (ion mobility falls off). Clamped to `[0.55, 1.0]` — this is a usable-energy
/// derate, not a cycle-life model (aging above 45 °C is a separate concern).
#[must_use]
pub fn capacity_temp_derate(temp_c: f32) -> f32 {
    let f = if temp_c >= 25.0 {
        1.0
    } else if temp_c >= 0.0 {
        0.80 + (temp_c / 25.0) * 0.20 // 0 °C→0.80 … 25 °C→1.00
    } else {
        // below freezing: extrapolate the 0 °C point down toward ~0.55 at -20 °C
        0.80 + (temp_c / 20.0) * 0.25
    };
    f.clamp(0.55, 1.0)
}

/// Every tunable input to the battery estimate, in one place. Each field defaults
/// to the module's datasheet-grounded baseline; construct with `..Default::default()`
/// and override only what you're changing (a bench figure, a colder temperature,
/// a different regulator, a bigger pack).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PowerParams {
    /// Nominal pack capacity, mAh (Akyga LP623255 = 1100).
    pub battery_mah: f32,
    /// Operating temperature, °C — derates usable capacity via
    /// [`capacity_temp_derate`]. 25 °C nominal.
    pub temp_c: f32,
    /// CPU Sleep (WFI, SAI DMA running) current, mA — the dominant, least-known
    /// term (see [`SLEEP_CURRENT_MA`]); bracket it by varying this field.
    pub sleep_ma: f32,
    /// DAC/amp playback current, mA ([`DAC_AMP_PLAYBACK_MA`]).
    pub dac_ma: f32,
    /// microSD average current, mA ([`SD_AVG_MA`]).
    pub sd_ma: f32,
    /// Display average current, mA ([`DISPLAY_AVG_MA`]).
    pub display_ma: f32,
    /// Regulator **conversion-efficiency ceiling** η∞ ∈ `(0, 1]`: the loss-free
    /// conversion limit (gate-drive/duty), which the curve *approaches* but never
    /// reaches — the actual peak efficiency is strictly below it (see
    /// [`regulator_efficiency_at`](Self::regulator_efficiency_at)). Default
    /// **0.92**. `η∞ <= 0` degrades the whole regulator to infinite battery draw.
    pub regulator_conv_efficiency: f32,
    /// Regulator **fixed loss** referred to the battery, mA: the roughly
    /// load-independent switching + control draw the converter burns whenever
    /// it's running (≈ its no-load input current). This is the knob that sets the
    /// **light-load droop** — it dominates the loss when the output is small, so a
    /// bigger value craters efficiency harder at the DAP's ~20 mA load. Default
    /// **1.5 mA**. (Deep-standby draw is modeled separately by
    /// [`STOP_MODE_CURRENT_MA`], not here — this term is the *active* fixed loss.)
    pub regulator_fixed_loss_ma: f32,
    /// Regulator **conduction-loss coefficient**, per mA (so `cond · I²` is in
    /// mA): the I²R term (inductor DCR, switch R_DS(on)) that pulls efficiency
    /// back down at **heavy load**. Together with `fixed_loss` it fixes the peak:
    /// efficiency peaks at `I* = sqrt(fixed_loss / cond)` (see
    /// [`regulator_peak_load_ma`](Self::regulator_peak_load_ma)). Default
    /// **4.0e-4 /mA** (peak ≈ 61 mA). `cond <= 0` removes the heavy-load droop.
    pub regulator_cond_loss_coeff: f32,
}

impl Default for PowerParams {
    fn default() -> Self {
        Self {
            battery_mah: DEFAULT_BATTERY_MAH,
            temp_c: 25.0,
            sleep_ma: SLEEP_CURRENT_MA,
            dac_ma: DAC_AMP_PLAYBACK_MA,
            sd_ma: SD_AVG_MA,
            display_ma: DISPLAY_AVG_MA,
            regulator_conv_efficiency: 0.92,
            regulator_fixed_loss_ma: 1.5,
            regulator_cond_loss_coeff: 4.0e-4,
        }
    }
}

impl PowerParams {
    /// Non-CPU output-path current during playback (DAC + SD + display), mA.
    #[must_use]
    pub fn peripheral_ma(&self) -> f32 {
        self.dac_ma + self.sd_ma + self.display_ma
    }

    /// Temperature-derated usable capacity, mAh.
    #[must_use]
    pub fn usable_capacity_mah(&self) -> f32 {
        self.battery_mah * capacity_temp_derate(self.temp_c)
    }

    /// The current the BATTERY supplies for a given chip-side draw, mA, through
    /// the **load-dependent** lossy regulator:
    ///
    /// ```text
    /// I_batt(I) = I / η∞           (ideal conversion)
    ///           + fixed_loss_ma     (switching/control: the light-load penalty)
    ///           + cond · I²         (I²R conduction: the heavy-load penalty)
    /// ```
    ///
    /// This is the term the bare `system_current_ma` omits. Because `fixed_loss`
    /// is a constant adder, the *efficiency* `I / I_batt(I)` is poor at tiny `I`,
    /// best near `I* = sqrt(fixed_loss / cond)`, and tails off again as the `I²`
    /// term grows — the real buck shape (see
    /// [`regulator_efficiency_at`](Self::regulator_efficiency_at)). `η∞ <= 0`
    /// degrades to `INFINITY`; a negative `cond` or `fixed_loss` is clamped to 0
    /// (no negative loss).
    #[must_use]
    pub fn battery_current_ma(&self, chip_ma: f32) -> f32 {
        if self.regulator_conv_efficiency <= 0.0 {
            return f32::INFINITY;
        }
        let fixed = self.regulator_fixed_loss_ma.max(0.0);
        let cond = self.regulator_cond_loss_coeff.max(0.0);
        chip_ma / self.regulator_conv_efficiency + fixed + cond * chip_ma * chip_ma
    }

    /// The regulator's **efficiency at a given chip-side load**, `I / I_batt(I)` ∈
    /// `[0, 1)` — the load-dependent buck curve (see
    /// [`battery_current_ma`](Self::battery_current_ma)). Rises from ~0 at tiny
    /// load (fixed loss dominates), peaks at
    /// [`regulator_peak_load_ma`](Self::regulator_peak_load_ma), then declines as
    /// conduction dominates. `0.0` for a non-positive load or a degenerate
    /// (infinite-draw) regulator.
    #[must_use]
    pub fn regulator_efficiency_at(&self, chip_ma: f32) -> f32 {
        if chip_ma <= 0.0 {
            return 0.0;
        }
        let batt = self.battery_current_ma(chip_ma);
        if !batt.is_finite() || batt <= 0.0 {
            return 0.0;
        }
        chip_ma / batt
    }

    /// The chip-side load, mA, at which [`regulator_efficiency_at`] peaks:
    /// `I* = sqrt(fixed_loss / cond)` — where the light-load (fixed) and
    /// heavy-load (conduction) losses cross over. Returns `INFINITY` if there is
    /// no conduction term (`cond <= 0`), i.e. efficiency rises monotonically.
    #[must_use]
    pub fn regulator_peak_load_ma(&self) -> f32 {
        let fixed = self.regulator_fixed_loss_ma.max(0.0);
        let cond = self.regulator_cond_loss_coeff.max(0.0);
        if cond <= 0.0 {
            return f32::INFINITY;
        }
        // `f32::sqrt` is std-only; this crate is `no_std`, so use the libm intrinsic.
        libm::sqrtf(fixed / cond)
    }

    /// Full-system battery-rail current during playback at CPU-blend `cpu_avg_ma`:
    /// `(cpu + peripheral)` drawn through the regulator.
    #[must_use]
    pub fn system_battery_ma(&self, cpu_avg_ma: f32) -> f32 {
        self.battery_current_ma(cpu_avg_ma + self.peripheral_ma())
    }

    /// Realistic estimated runtime, hours, from an already-computed CPU blend —
    /// folds in BOTH temperature (usable capacity) and the regulator (efficiency +
    /// I_Q), the two effects the bare `runtime_hours(DEFAULT_BATTERY_MAH,
    /// system_current_ma(..))` omits.
    #[must_use]
    pub fn runtime_hours(&self, cpu_avg_ma: f32) -> f32 {
        runtime_hours(
            self.usable_capacity_mah(),
            self.system_battery_ma(cpu_avg_ma),
        )
    }

    /// CPU blend current at `clock_mhz` for a codec's `rtf_at_ref_clock`, using
    /// THIS params' `sleep_ma` (so `sleep_ma` is a live tunable, not just the
    /// module const). Same math as [`system_avg_current_at_clock`]'s CPU part.
    #[must_use]
    pub fn cpu_avg_at_clock(&self, rtf_at_ref_clock: f32, ref_mhz: u32, clock_mhz: u32) -> f32 {
        let cost = decode_cost_cycles_per_sec(rtf_at_ref_clock, ref_mhz);
        let duty = duty_cycle(cost, f64::from(clock_mhz) * 1.0e6);
        avg_current_ma(duty, active_current_ma_for_clock(clock_mhz), self.sleep_ma)
    }

    /// Fully self-contained realistic runtime for a codec decoded at `clock_mhz`:
    /// folds in sleep (via the CPU blend), the output path, the regulator, and
    /// temperature — i.e. every tunable in `self`. Give it a measured RTF + the
    /// chosen clock and it yields the realistic hours for *this* configuration.
    #[must_use]
    pub fn runtime_hours_at_clock(
        &self,
        rtf_at_ref_clock: f32,
        ref_mhz: u32,
        clock_mhz: u32,
    ) -> f32 {
        self.runtime_hours(self.cpu_avg_at_clock(rtf_at_ref_clock, ref_mhz, clock_mhz))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---------------------------------------------------------------------
    // Host RTFs measured via `cargo bench -p dap-sim --bench decode_rtf`
    // (decode-only, passthrough rate; see that file for the full method/
    // caveat). These are the DAP's full shipped format set. Re-measure and
    // update these if a decoder/resampler change moves the numbers --
    // that's this test module doing its job (a regression gate), not a bug.
    // All seven decode the same ~4 s CC0 "Open Goldberg Variations" clip
    // (soul-audio-core's `tests/decode.rs` REAL MUSIC fixture note).
    // ---------------------------------------------------------------------
    const HOST_RTF_WAV: f32 = 5127.3;
    const HOST_RTF_FLAC: f32 = 266.6;
    const HOST_RTF_MP3: f32 = 465.1;
    const HOST_RTF_AAC: f32 = 247.2;
    const HOST_RTF_ALAC: f32 = 265.3;
    const HOST_RTF_OPUS: f32 = 179.2;
    const HOST_RTF_WAVPACK: f32 = 264.6;
    /// Full pipeline (decode + resample 44.1kHz->48kHz + sink-write) for the
    /// cheapest format (WAV) -- the worst-case RTF across the whole
    /// benchmark set, because resampling costs far more than any of the
    /// seven decoders' own entropy/predict stage on this host. On THIS board the
    /// resampler is the FALLBACK path, not the default: the CS43131's dual MCLK
    /// oscillator plays 44.1k and 48k families natively, so standard content
    /// decodes without resampling (see `native_rate_clocking_needs_lower_clock_than_resampling`).
    /// This number drives the clock choice only for the uncommon off-family rate.
    const HOST_RTF_PIPELINE_RESAMPLED: f32 = 115.5;

    const ALL_DECODE_FORMATS: [(&str, f32); 7] = [
        ("wav", HOST_RTF_WAV),
        ("flac", HOST_RTF_FLAC),
        ("mp3", HOST_RTF_MP3),
        ("aac", HOST_RTF_AAC),
        ("alac", HOST_RTF_ALAC),
        ("opus", HOST_RTF_OPUS),
        ("wavpack", HOST_RTF_WAVPACK),
    ];

    /// A representative "headroom" safety margin for this module's own
    /// tests -- independent of `power::dvfs`'s `BALANCED_SAFETY`/
    /// `BATTERY_SAVER_SAFETY` (which bias real profile selection); this
    /// module tests the underlying math on its own terms, not any
    /// particular policy's choice of margin.
    const TEST_SAFETY_MARGIN: f32 = 1.3;
    /// A tighter margin, for the runtime-estimate test below -- mirrors the
    /// shape of `power::dvfs::BATTERY_SAVER_SAFETY` (bare-minimum clock)
    /// without depending on that module.
    const TEST_TIGHT_SAFETY_MARGIN: f32 = 1.15;

    // --- clock-headroom per format -----------------------------------

    /// Every format must sustain >=1x real-time at or below 200 MHz (VOS3's
    /// ceiling per design doc §4.2 -- "VOS3 1.0V/200MHz") even under the
    /// pessimistic host->target derate and a 30% safety margin. This is the
    /// model's central claim: the DAP can run the *lowest* voltage/power
    /// state for every shipped format, not just the easy ones.
    #[test]
    fn clock_headroom_per_format_fits_vos3_target() {
        const VOS3_TARGET_MHZ: u32 = 200;
        for (label, host_rtf) in ALL_DECODE_FORMATS {
            let target_rtf = estimate_target_rtf(host_rtf);
            let min_clock =
                min_clock_mhz_for_rtf(target_rtf, H7A3_MAX_CLOCK_MHZ, TEST_SAFETY_MARGIN);
            assert!(
                min_clock <= VOS3_TARGET_MHZ,
                "{label}: derated target RTF {target_rtf:.2}x needs {min_clock} MHz, \
                 expected <= {VOS3_TARGET_MHZ} MHz (VOS3 ceiling)"
            );
        }
    }

    /// Even the worst-case pipeline (decode + resample, not decode alone --
    /// see [`HOST_RTF_PIPELINE_RESAMPLED`]'s doc) still fits under the VOS3
    /// target, with margin to spare. This is the conservative check: if a
    /// track needs resampling (source rate != DAC rate), this is the real
    /// number to budget against, not the decode-only RTF.
    #[test]
    fn clock_headroom_pipeline_resampled_fits_vos3_target() {
        const VOS3_TARGET_MHZ: u32 = 200;
        let target_rtf = estimate_target_rtf(HOST_RTF_PIPELINE_RESAMPLED);
        let min_clock = min_clock_mhz_for_rtf(target_rtf, H7A3_MAX_CLOCK_MHZ, TEST_SAFETY_MARGIN);
        assert!(
            min_clock <= VOS3_TARGET_MHZ,
            "resampled pipeline: derated target RTF {target_rtf:.2}x needs {min_clock} MHz, \
             expected <= {VOS3_TARGET_MHZ} MHz (VOS3 ceiling)"
        );
    }

    /// Native-rate clocking beats resample-to-fixed-rate, on clock and so on
    /// power. The board's CS43131 DAC takes BOTH sample-rate families via a dual
    /// MCLK oscillator (22.5792 MHz→44.1k, 24.576 MHz→48k — see the DAC-selection
    /// research), so the firmware can clock the SAI at the source's native rate
    /// and SKIP the resampler for all standard content (44.1/48k + multiples) —
    /// resampling being the single heaviest pipeline stage (see
    /// [`HOST_RTF_PIPELINE_RESAMPLED`]). This gate pins that the worst-case *raw
    /// decode* (no resample) needs a strictly LOWER CPU clock than resampling, so
    /// native-rate clocking is a real power win and the firmware should prefer it
    /// over a fixed-DAC-rate + always-resample path. (If a future resampler
    /// speedup ever inverts this, revisit the clocking strategy — that's the gate
    /// doing its job.)
    #[test]
    fn native_rate_clocking_needs_lower_clock_than_resampling() {
        // Heaviest raw decoder = the lowest RTF of the seven.
        let worst_decode_rtf = ALL_DECODE_FORMATS
            .iter()
            .map(|&(_, r)| r)
            .fold(f32::INFINITY, f32::min);
        let native_clock = min_clock_mhz_for_rtf(
            estimate_target_rtf(worst_decode_rtf),
            H7A3_MAX_CLOCK_MHZ,
            TEST_SAFETY_MARGIN,
        );
        let resampled_clock = min_clock_mhz_for_rtf(
            estimate_target_rtf(HOST_RTF_PIPELINE_RESAMPLED),
            H7A3_MAX_CLOCK_MHZ,
            TEST_SAFETY_MARGIN,
        );
        assert!(
            native_clock < resampled_clock,
            "native-rate worst-case decode needs {native_clock} MHz but resampling needs \
             {resampled_clock} MHz — native-rate clocking should need the lower clock (it avoids \
             the resampler, the heaviest stage). If this inverts, revisit the clocking strategy."
        );
    }

    // --- runtime estimate per format ----------------------------------

    /// Runtime estimate per format at a bare-minimum-clock policy (tight
    /// 15% margin, clamped to the selectable [`IDLE_CLOCK_MHZ`]..=
    /// [`H7A3_MAX_CLOCK_MHZ`] range -- the shape of `power::dvfs`'s
    /// `BatterySaver` policy, reproduced locally so this module's tests
    /// don't depend on that one), @ [`DEFAULT_BATTERY_MAH`]. This is a
    /// model/guard (datasheet currents x measured decode cost), not a
    /// measurement -- see the module doc. Measured outputs at the real 1100 mAh
    /// pack range ~65h (opus, the worst case -- highest clock/duty of the seven)
    /// to ~200h (wav, clamped to the idle clock floor); the 50h bound leaves
    /// ~1.3x margin under the worst case while still catching a real regression
    /// (e.g. `SLEEP_CURRENT_MA` or a duty-cycle bug blowing up), not asserting a
    /// specific battery-life number this model has no authority to promise.
    /// (CPU-only; the realistic figure folds the output path in -- see
    /// `system_runtime_is_realistic_and_below_cpu_only`.)
    #[test]
    fn runtime_hours_per_format_meets_sane_target() {
        const MIN_SANE_RUNTIME_HOURS: f32 = 50.0;
        for (label, host_rtf) in ALL_DECODE_FORMATS {
            let target_rtf = estimate_target_rtf(host_rtf);
            let clock =
                min_clock_mhz_for_rtf(target_rtf, H7A3_MAX_CLOCK_MHZ, TEST_TIGHT_SAFETY_MARGIN)
                    .clamp(IDLE_CLOCK_MHZ, H7A3_MAX_CLOCK_MHZ);
            let cost = decode_cost_cycles_per_sec(target_rtf, H7A3_MAX_CLOCK_MHZ);
            let duty = duty_cycle(cost, f64::from(clock) * 1.0e6);
            let active_ma = active_current_ma_for_clock(clock);
            let avg_ma = avg_current_ma(duty, active_ma, SLEEP_CURRENT_MA);
            let hours = runtime_hours(DEFAULT_BATTERY_MAH, avg_ma);
            assert!(
                hours >= MIN_SANE_RUNTIME_HOURS,
                "{label}: modeled runtime {hours:.1}h at {clock} MHz (duty {duty:.2}, \
                 avg {avg_ma:.1} mA) below the {MIN_SANE_RUNTIME_HOURS}h sanity floor"
            );
            // Sanity: duty must leave real sleep room (the whole point of
            // the power loop) -- never fully busy.
            assert!(
                duty < 1.0,
                "{label}: duty cycle {duty:.3} leaves no sleep time"
            );
        }
    }

    /// The REALISTIC total-system runtime — the CPU blend plus the always-on
    /// output path (DAC/amp + SD + display, [`system_current_ma`]). Two claims:
    /// (a) it still clears a sane floor at [`DEFAULT_BATTERY_MAH`], and (b) it is
    /// strictly LOWER than the CPU-only estimate for every format — i.e. the
    /// output path is really being counted (a dropped peripheral term regresses
    /// this). The gap is largest for the cheap formats (WAV), where the CPU
    /// current is lowest and the ~9.5 mA output path dominates: at the real
    /// 1100 mAh pack, WAV's optimistic CPU-only ~200h becomes a realistic ~74h
    /// once the DAC/amp is in; the worst case (opus) is ~41h.
    #[test]
    fn system_runtime_is_realistic_and_below_cpu_only() {
        const MIN_SYSTEM_RUNTIME_HOURS: f32 = 30.0;
        for (label, host_rtf) in ALL_DECODE_FORMATS {
            let target_rtf = estimate_target_rtf(host_rtf);
            let clock =
                min_clock_mhz_for_rtf(target_rtf, H7A3_MAX_CLOCK_MHZ, TEST_TIGHT_SAFETY_MARGIN)
                    .clamp(IDLE_CLOCK_MHZ, H7A3_MAX_CLOCK_MHZ);
            let cost = decode_cost_cycles_per_sec(target_rtf, H7A3_MAX_CLOCK_MHZ);
            let duty = duty_cycle(cost, f64::from(clock) * 1.0e6);
            let cpu_ma = avg_current_ma(duty, active_current_ma_for_clock(clock), SLEEP_CURRENT_MA);
            let sys_ma = system_current_ma(cpu_ma);
            let cpu_hours = runtime_hours(DEFAULT_BATTERY_MAH, cpu_ma);
            let sys_hours = runtime_hours(DEFAULT_BATTERY_MAH, sys_ma);
            assert!(
                sys_hours >= MIN_SYSTEM_RUNTIME_HOURS,
                "{label}: total-system runtime {sys_hours:.1}h (sys {sys_ma:.1} mA) below the \
                 {MIN_SYSTEM_RUNTIME_HOURS}h floor"
            );
            assert!(
                sys_hours < cpu_hours,
                "{label}: system runtime {sys_hours:.1}h should be below the CPU-only \
                 {cpu_hours:.1}h (the output path must be counted)"
            );
        }
    }

    #[test]
    fn system_current_adds_the_output_path() {
        // 7 (CS43131) + 2 (SD) + 0.5 (display) = 9.5 mA.
        assert!((PERIPHERAL_PLAYBACK_MA - 9.5).abs() < 1e-6);
        assert!((system_current_ma(10.0) - 19.5).abs() < 1e-6);
    }

    /// Resolves `power::dvfs`'s flagged, previously-out-of-scope refinement:
    /// is "race to sleep" (burst at max clock, then WFI) more energy-efficient
    /// than `BatterySaver`'s minimum sustaining clock? For this DAP: **no.**
    /// For every format, total-system current at the minimum sustaining clock is
    /// `<=` current at max clock — so racing to sleep never wins. Why: the H7A3
    /// current-vs-clock curve is ~linear (energy per cycle ~constant, no
    /// super-linear high-clock penalty to exploit by racing), low-clock active
    /// current is cheap, and the ~9.5 mA output path is on either way. So
    /// minimize-clock IS near energy-optimal; a high-clock-burst DVFS policy
    /// would not improve battery here. (Model-dependent on the 4-point current
    /// curve + `SLEEP_CURRENT_MA`; confirm on silicon — the direction is the
    /// finding.)
    #[test]
    fn min_clock_not_beaten_by_race_to_sleep() {
        for (label, host_rtf) in ALL_DECODE_FORMATS {
            let rtf = estimate_target_rtf(host_rtf);
            let min_clock =
                min_clock_mhz_for_rtf(rtf, H7A3_MAX_CLOCK_MHZ, TEST_TIGHT_SAFETY_MARGIN)
                    .clamp(IDLE_CLOCK_MHZ, H7A3_MAX_CLOCK_MHZ);
            let at_min = system_avg_current_at_clock(rtf, H7A3_MAX_CLOCK_MHZ, min_clock);
            let at_max = system_avg_current_at_clock(rtf, H7A3_MAX_CLOCK_MHZ, H7A3_MAX_CLOCK_MHZ);
            assert!(
                at_min <= at_max + 1e-3,
                "{label}: min-clock {min_clock} MHz draws {at_min:.2} mA but race-to-sleep \
                 (max clock) draws {at_max:.2} mA — race-to-sleep unexpectedly wins; \
                 the dvfs minimize-clock policy may no longer be energy-optimal"
            );
        }
    }

    // --- sleep-ratio / wakeup-rate (decode-ahead ring sizing) ---------

    /// A decode-ahead ring sized against `dap-arch-tests`' `DMA_BUFFERS_BUDGET`
    /// (64 KB; stereo, 24-bit-in-`i32` samples => 8 B/frame => 8192 frames)
    /// keeps the DMA half/complete wakeup rate low (infrequent bursts, long
    /// sleep between them -- design doc §1.1's third lever) at both common
    /// device rates.
    #[test]
    fn decode_ahead_ring_sized_for_infrequent_wakeups() {
        const RING_BUDGET_BYTES: u32 = 64 * 1024;
        const BYTES_PER_FRAME: u32 = 2 /* stereo */ * 4 /* i32 */;
        const RING_FRAMES: u32 = RING_BUDGET_BYTES / BYTES_PER_FRAME;
        const TARGET_MAX_WAKEUPS_PER_SEC: f32 = 20.0;

        for sample_rate in [44_100u32, 48_000u32] {
            let wakeups = wakeups_per_sec(sample_rate, RING_FRAMES);
            assert!(
                wakeups <= TARGET_MAX_WAKEUPS_PER_SEC,
                "{sample_rate} Hz: {wakeups:.2} wakeups/s over a {RING_FRAMES}-frame ring, \
                 expected <= {TARGET_MAX_WAKEUPS_PER_SEC}/s"
            );
        }
    }

    /// Contrast: sizing the ring down to `soul_audio_core::deck`'s own
    /// per-pump `BLOCK` granularity (256 frames -- no extra decode-ahead
    /// buffering at all) wakes the CPU an order of magnitude more often.
    /// This is *why* the decode-ahead ring is budgeted as its own region
    /// (design doc §0/§3) rather than left at the bare pump block size.
    #[test]
    fn tiny_ring_wakes_up_far_more_often_than_the_budgeted_ring() {
        const RING_BUDGET_BYTES: u32 = 64 * 1024;
        const BYTES_PER_FRAME: u32 = 2 * 4;
        const BUDGETED_RING_FRAMES: u32 = RING_BUDGET_BYTES / BYTES_PER_FRAME;
        const BARE_PUMP_BLOCK_FRAMES: u32 = 256;

        let budgeted = wakeups_per_sec(48_000, BUDGETED_RING_FRAMES);
        let bare = wakeups_per_sec(48_000, BARE_PUMP_BLOCK_FRAMES);
        assert!(
            bare > budgeted * 10.0,
            "expected the bare pump-block ring ({bare:.1}/s) to wake up far more often than \
             the budgeted decode-ahead ring ({budgeted:.1}/s)"
        );
    }

    // --- pure-function unit sanity -------------------------------------

    /// The model's run-current table is deliberately CONSERVATIVE (peripherals
    /// on): every point must sit at/above the STM32H7A3 datasheet best-case
    /// peripherals-off-SMPS line (120 µA/MHz, DS13677), so the model never
    /// under-estimates current / over-estimates runtime. Guards against anyone
    /// "optimizing" the table below the physical floor. Also pins that the
    /// Stop-mode pause current is negligible vs even the Sleep estimate.
    #[test]
    fn run_currents_stay_above_datasheet_best_case_floor() {
        for (mhz, ma) in RUN_CURRENT_TABLE_MHZ {
            let floor_ma = DATASHEET_RUN_UA_PER_MHZ_BEST * mhz as f32 / 1000.0;
            assert!(
                ma >= floor_ma,
                "{mhz} MHz: model {ma} mA is below the datasheet best-case floor {floor_ma:.1} mA \
                 — the model would under-estimate current"
            );
        }
    }

    #[test]
    fn min_clock_mhz_for_rtf_scales_as_expected() {
        // Exactly 2x RTF headroom at the reference clock needs exactly half
        // the reference clock with no extra safety margin.
        assert_eq!(min_clock_mhz_for_rtf(2.0, 280, 1.0), 140);
        // A safety margin scales the result proportionally.
        assert_eq!(min_clock_mhz_for_rtf(2.0, 280, 1.2), 168);
        // Degenerate (no RTF info) falls back to the whole reference clock.
        assert_eq!(min_clock_mhz_for_rtf(0.0, 280, 1.2), 280);
    }

    #[test]
    fn avg_current_ma_interpolates_between_active_and_sleep() {
        assert!((avg_current_ma(0.0, 100.0, 5.0) - 5.0).abs() < 1e-6);
        assert!((avg_current_ma(1.0, 100.0, 5.0) - 100.0).abs() < 1e-6);
        assert!((avg_current_ma(0.5, 100.0, 0.0) - 50.0).abs() < 1e-6);
    }

    #[test]
    fn runtime_hours_halves_when_current_doubles() {
        let h1 = runtime_hours(2000.0, 20.0);
        let h2 = runtime_hours(2000.0, 40.0);
        assert!((h1 / 2.0 - h2).abs() < 1e-3);
    }

    #[test]
    fn active_current_ma_for_clock_matches_table_points_and_clamps() {
        assert!((active_current_ma_for_clock(60) - RUN_CURRENT_MA_AT_60MHZ).abs() < 1e-6);
        assert!((active_current_ma_for_clock(280) - RUN_CURRENT_MA_AT_280MHZ).abs() < 1e-6);
        assert!((active_current_ma_for_clock(480) - RUN_CURRENT_MA_AT_480MHZ).abs() < 1e-6);
        // Below the table's floor and above its ceiling: clamp, don't
        // extrapolate.
        assert!((active_current_ma_for_clock(10) - RUN_CURRENT_MA_AT_60MHZ).abs() < 1e-6);
        assert!((active_current_ma_for_clock(1000) - RUN_CURRENT_MA_AT_480MHZ).abs() < 1e-6);
        // Midpoint between two measured points interpolates linearly.
        let mid = active_current_ma_for_clock(170); // halfway 60..280
        let expected = (RUN_CURRENT_MA_AT_60MHZ + RUN_CURRENT_MA_AT_280MHZ) / 2.0;
        assert!((mid - expected).abs() < 0.5);
    }

    // --- PowerParams: tweakable + thermal + regulator-aware ------------

    #[test]
    fn default_params_match_the_module_constants() {
        let p = PowerParams::default();
        assert_eq!(p.battery_mah, DEFAULT_BATTERY_MAH);
        assert_eq!(p.temp_c, 25.0);
        assert_eq!(p.sleep_ma, SLEEP_CURRENT_MA);
        assert!((p.peripheral_ma() - PERIPHERAL_PLAYBACK_MA).abs() < 1e-6);
    }

    #[test]
    fn capacity_derates_with_temperature() {
        assert!(
            (capacity_temp_derate(25.0) - 1.0).abs() < 1e-6,
            "25 C = full"
        );
        assert!(
            (capacity_temp_derate(45.0) - 1.0).abs() < 1e-6,
            "45 C flat full"
        );
        assert!((capacity_temp_derate(0.0) - 0.80).abs() < 1e-6, "0 C ~80%");
        assert!(capacity_temp_derate(-20.0) < 0.6, "deep cold well under");
        assert!(
            capacity_temp_derate(-100.0) >= 0.55,
            "clamped, not negative"
        );
        // Cold strictly reduces usable capacity.
        assert!(capacity_temp_derate(0.0) < capacity_temp_derate(25.0));
    }

    #[test]
    fn regulator_makes_battery_draw_exceed_chip_draw() {
        let p = PowerParams::default();
        // The regulator always costs: battery draw strictly exceeds the chip draw.
        assert!(
            p.battery_current_ma(20.0) > 20.0,
            "the regulator always costs — the bare model omitted this"
        );
        // Sanity: at the DAP's ~20 mA playback load the modeled efficiency is a
        // realistic-buck number on the light-load flank (well under the ceiling,
        // not implausibly low).
        let eta20 = p.regulator_efficiency_at(20.0);
        assert!(
            (0.75..p.regulator_conv_efficiency).contains(&eta20),
            "η(20 mA) = {eta20:.3} should be a realistic light-load point below the ceiling"
        );
        // A worse regulator — bigger fixed (switching) loss — costs more.
        let worse = PowerParams {
            regulator_fixed_loss_ma: p.regulator_fixed_loss_ma + 2.0,
            ..p
        };
        assert!(worse.battery_current_ma(20.0) > p.battery_current_ma(20.0));
        // A degenerate regulator (η∞ <= 0) degrades to infinite battery draw.
        let dead = PowerParams {
            regulator_conv_efficiency: 0.0,
            ..p
        };
        assert!(dead.battery_current_ma(20.0).is_infinite());
        assert_eq!(dead.regulator_efficiency_at(20.0), 0.0);
    }

    /// The regulator efficiency traces the real buck curve: it rises from a poor
    /// light-load point, peaks near `regulator_peak_load_ma`, then droops again as
    /// conduction loss takes over at heavy load.
    #[test]
    fn regulator_efficiency_follows_a_load_curve() {
        let p = PowerParams::default();
        let peak = p.regulator_peak_load_ma();
        assert!(
            (40.0..90.0).contains(&peak),
            "default peak load {peak:.0} mA should sit in the tens-of-mA range"
        );
        let eta_light = p.regulator_efficiency_at(3.0);
        let eta_peak = p.regulator_efficiency_at(peak);
        let eta_heavy = p.regulator_efficiency_at(4.0 * peak);
        // Light-load droop: far worse than peak.
        assert!(
            eta_light < eta_peak - 0.05,
            "light load η={eta_light:.3} should be well below peak η={eta_peak:.3}"
        );
        // Heavy-load droop: conduction pulls it back down past the peak.
        assert!(
            eta_heavy < eta_peak,
            "heavy load η={eta_heavy:.3} should fall below peak η={eta_peak:.3}"
        );
        // The peak really is a local max: a step either side is no higher.
        assert!(p.regulator_efficiency_at(peak * 0.5) <= eta_peak);
        assert!(p.regulator_efficiency_at(peak * 2.0) <= eta_peak);
        // The peak stays strictly below the conversion ceiling η∞.
        assert!(eta_peak < p.regulator_conv_efficiency);
        // Monotone rise on the light-load flank (0 -> peak).
        assert!(
            p.regulator_efficiency_at(5.0) < p.regulator_efficiency_at(20.0),
            "efficiency must climb with load below the peak"
        );
    }

    /// The realistic PowerParams runtime (temperature + regulator folded in) is
    /// strictly LOWER than the bare CPU+peripheral estimate — the two omitted
    /// physical effects always cost runtime, never add it.
    #[test]
    fn params_runtime_is_lower_than_the_bare_model() {
        let p = PowerParams::default();
        let cpu = 15.0_f32;
        let bare = runtime_hours(DEFAULT_BATTERY_MAH, system_current_ma(cpu));
        let real = p.runtime_hours(cpu);
        assert!(
            real < bare,
            "realistic {real:.1}h should be below the bare {bare:.1}h (regulator + any thermal derate)"
        );
        // Cold operation is strictly worse than nominal.
        let cold = PowerParams { temp_c: 0.0, ..p };
        assert!(cold.runtime_hours(cpu) < real, "0 C runtime < 25 C runtime");
    }

    /// Sensitivity bracket for the model's dominant unknown, `sleep_ma`. The CPU
    /// sleeps most of playback, so the estimate hinges on this one placeholder:
    /// tripling it (5 → 15 mA) must move the runtime a LOT (well over 25%). This
    /// makes the fragility explicit — the headline number is only as good as this
    /// bench-measurable figure. (Task: replace with a measured value on silicon.)
    #[test]
    fn sleep_current_is_the_dominant_sensitivity() {
        // WAV at the idle clock: tiny duty, so the CPU is asleep almost the whole
        // time and the sleep current dominates the average.
        let rtf = estimate_target_rtf(HOST_RTF_WAV);
        let at = |sleep: f32| {
            PowerParams {
                sleep_ma: sleep,
                ..PowerParams::default()
            }
            .runtime_hours_at_clock(rtf, H7A3_MAX_CLOCK_MHZ, IDLE_CLOCK_MHZ)
        };
        let at5 = at(5.0);
        let at15 = at(15.0);
        assert!(at15 < at5, "more sleep current = less runtime");
        let spread = (at5 - at15) / at5;
        assert!(
            spread > 0.25,
            "sleep_ma 5→15 moved runtime only {:.0}% — expected a large swing, since the \
             estimate hinges on this placeholder",
            spread * 100.0
        );
    }
}
