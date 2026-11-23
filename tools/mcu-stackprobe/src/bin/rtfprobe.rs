//! Decode **real-time-factor (RTF)** harness on real Cortex-M7 codegen, with
//! the I-cache and D-cache switched on and off (feature `rtf`).
//!
//! # Why this exists
//!
//! The power model's whole chain is `RTF -> minimum clock -> current -> hours`,
//! and today its RTF input is **not measured on the target at all**: it is a
//! HOST (x86) benchmark divided by `power::model::HOST_TO_TARGET_DERATE` (50x),
//! which that module's own doc calls "a projection, not a measurement". This
//! harness runs the real `open_seekable -> Deck::load -> Deck::pump` path on real
//! thumbv7em codegen and reports work per second of decoded audio, which is what
//! the model actually wants.
//!
//! It is also the rig for the cache question.
//! `docs/plans/2026-11-21-memory-architecture-and-placement.md` §4 records that
//! the M7 resets with both caches OFF, that `cortex-m-rt` does not enable them,
//! and that `embassy-stm32` 0.6 contains no cache handling at all — so the
//! shipped firmware runs uncached, and ST's own figure is that a disabled
//! D-cache roughly **halves** CPU performance. If that holds, L11's conclusion
//! ("opus needs VOS2, VOS3 is unreachable") may be an artifact of measuring
//! uncached. Three passes — caches off, I-cache only, I+D — make that ratio a
//! measurement instead of a citation.
//!
//! # What each instrument can and cannot tell you
//!
//! Two counters, picked at runtime and reported in the VALIDATE line, because
//! they measure different things and only one of them exists in emulation:
//!
//! * **DWT CYCCNT** — true core cycles. The number that matters, available on
//!   silicon. QEMU's `mps2-an500` does **not implement it** (it reads 0 and
//!   `cycle_counter_enabled()` returns false), which is why the fallback exists.
//! * **SysTick** — always present. On silicon it measures real time at a known
//!   core clock. Under QEMU with `-icount shift=0` the virtual clock advances
//!   per guest instruction, so SysTick counts become **instructions, not
//!   cycles**. Without `-icount` the number is meaningless, so
//!   `check-rtf-qemu.sh` always passes it.
//!
//! **QEMU cannot answer the cache question, at all.** Its TCG is a functional
//! emulator: no caches, no flash/AXI wait states, no store buffer, no
//! dual-issue. All three passes execute the same instructions, so the expected
//! emulated speedup is exactly 1.00x. That is not a finding — it is the absence
//! of one, and the script says so next to the figure.
//!
//! What QEMU *does* give soundly is instructions per second of audio, which is
//! cache-independent and brackets the target RTF far better than a 50x host
//! derate:
//!
//!   target_RTF ~= f_cpu / (instructions_per_audio_second * CPI)
//!
//! CPI is the part that needs silicon — it is precisely what the caches change,
//! and the reason this file hardcodes no CPI constant. Run the same bin through
//! `probe-rs`/`cargo embed` on board-h7a3 and CYCCNT is selected automatically,
//! at which point the speedup columns become real.

#![no_std]
#![no_main]

use core::sync::atomic::{AtomicU32, Ordering};

use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::peripheral::{Peripherals, DWT, SYST};
use cortex_m_rt::entry;
use panic_halt as _;
use static_cell::StaticCell;

use soul_audio_core::{
    decoder_scratch_len, open_seekable, AnyDecoder, Deck, Progress, Quality, SampleFormat, Sink,
    SinkError, SliceSource, StreamInfo,
};

/// Matroska-wrapped FLAC — the same fixture the stack probes treat as the
/// worst-case path, and the realistic one for this product (container + lossless
/// codec + resample). Path is relative to this source file, per `include_bytes!`.
const MKA_FLAC_FIXTURE: &[u8] =
    include_bytes!("../../../../../soul-audio-core/testdata/tone-flac.mka");

/// Output rate the sink presents, so the measured path includes the resampler
/// the board really runs. RTF is defined against the OUTPUT timeline: the sink
/// must be fed `SINK_RATE` frames per wall-clock second or audio glitches,
/// whatever the file's native rate is.
const SINK_RATE: u32 = 48_000;
const SINK_CHANNELS: u16 = 2;

/// SysTick is a 24-bit down-counter; this is its maximum reload.
const SYST_RELOAD: u32 = 0x00FF_FFFF;

/// Hard cap on pumps per pass so a fixture that never reports `Ended` cannot
/// hang the probe. Whether it was reached is reported, since a truncated pass
/// would make the three passes incomparable.
const MAX_PUMPS: u32 = 4_096;

/// Frames the sink has accepted in the pass currently running. A static counter
/// rather than a field, because `Deck` takes the sink **by value** and never
/// hands it back, so there is no way to read a count off it afterwards.
static FRAMES_OUT: AtomicU32 = AtomicU32::new(0);

/// Counts output frames and discards the audio.
struct CountingSink {
    info: StreamInfo,
}

impl Sink for CountingSink {
    fn stream_info(&self) -> StreamInfo {
        self.info
    }

    fn write(&mut self, frames: &[i32]) -> Result<(), SinkError> {
        // `frames` is interleaved, so one frame is `channels` samples.
        let ch = u32::from(self.info.channels).max(1);
        FRAMES_OUT.fetch_add(frames.len() as u32 / ch, Ordering::Relaxed);
        Ok(())
    }

    fn flush(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
}

type ProbeDecoder = AnyDecoder<SliceSource<'static>, &'static mut [i32]>;
type ProbeDeck = Deck<CountingSink, ProbeDecoder, 0>;

// One scratch and one deck per pass. The scratch is moved into the decoder by
// value, so a pass cannot reuse the previous pass's `&'static mut [i32]` — there
// is no safe way to reclaim that borrow, only an independent `StaticCell`. Same
// reasoning as `mcu-stackprobe`'s `SCRATCH2`.
static SCRATCH_A: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_B: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_C: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static DECK_A: StaticCell<ProbeDeck> = StaticCell::new();
static DECK_B: StaticCell<ProbeDeck> = StaticCell::new();
static DECK_C: StaticCell<ProbeDeck> = StaticCell::new();

/// Which counter the run is using. Reported, never assumed — the two measure
/// different quantities (see the module doc).
#[derive(Copy, Clone, PartialEq, Eq)]
enum Instrument {
    /// DWT CYCCNT: true core cycles (silicon).
    Cycles,
    /// SysTick: real time on silicon, guest instructions under `-icount`.
    SysTick,
}

impl Instrument {
    fn as_str(self) -> &'static str {
        match self {
            Instrument::Cycles => "dwt-cyccnt",
            Instrument::SysTick => "systick",
        }
    }
}

/// Accumulating counter that hides which instrument is in use.
///
/// SysTick needs accumulation because it is only 24 bits: it wraps every
/// 16,777,216 counts, so a multi-second decode wraps many times. `sample` is
/// called once per pump — roughly every ~1e6 counts for this workload, two
/// orders of magnitude inside the wrap period — and folds in at most one wrap
/// per call via `has_wrapped` (which reads and clears COUNTFLAG).
struct Counter {
    instrument: Instrument,
    /// Last raw reading. SysTick counts DOWN; DWT counts UP.
    last: u32,
    acc: u64,
    /// Set if two wraps could have happened between samples, which would make
    /// the total an undercount. Reported rather than silently tolerated.
    suspect_overflow: bool,
}

impl Counter {
    fn start(instrument: Instrument, syst: &mut SYST) -> Self {
        let last = match instrument {
            Instrument::Cycles => DWT::cycle_count(),
            Instrument::SysTick => {
                // Clear a stale COUNTFLAG so the first sample does not fold in a
                // wrap that happened before the window opened.
                let _ = syst.has_wrapped();
                SYST::get_current()
            }
        };
        Self {
            instrument,
            last,
            acc: 0,
            suspect_overflow: false,
        }
    }

    fn sample(&mut self, syst: &mut SYST) {
        match self.instrument {
            Instrument::Cycles => {
                let now = DWT::cycle_count();
                self.acc += u64::from(now.wrapping_sub(self.last));
                self.last = now;
            }
            Instrument::SysTick => {
                let now = SYST::get_current();
                let wrapped = syst.has_wrapped();
                // Down-counter: elapsed = last - now, plus a full period per wrap.
                let delta = if wrapped {
                    // COUNTFLAG only says "at least one wrap"; it cannot
                    // distinguish one from several.
                    if now > self.last {
                        u64::from(SYST_RELOAD + 1) - u64::from(now - self.last)
                    } else {
                        u64::from(SYST_RELOAD + 1) + u64::from(self.last - now)
                    }
                } else if self.last >= now {
                    u64::from(self.last - now)
                } else {
                    // Counter went up without COUNTFLAG: a wrap was missed.
                    self.suspect_overflow = true;
                    u64::from(SYST_RELOAD + 1) - u64::from(now - self.last)
                };
                self.acc += delta;
                self.last = now;
            }
        }
    }

    fn total(&mut self, syst: &mut SYST) -> u64 {
        self.sample(syst);
        self.acc
    }
}

/// One pass's result.
struct Pass {
    ticks: u64,
    frames: u32,
    pumps: u32,
    hit_cap: bool,
    suspect_overflow: bool,
}

fn sink() -> CountingSink {
    CountingSink {
        info: StreamInfo {
            sample_rate: SINK_RATE,
            channels: SINK_CHANNELS,
            format: SampleFormat::Int { bits: 24 },
        },
    }
}

/// Run the decode workload once and return work + frames.
///
/// The measured window deliberately covers `open_seekable` **and** the pump
/// loop: the board pays decoder construction on every track change, and it is
/// exactly the path whose instruction fetch an I-cache would serve. A
/// steady-state-only number would flatter the cached case.
fn run_pass(
    instrument: Instrument,
    scratch: &'static mut [i32],
    deck_cell: &'static StaticCell<ProbeDeck>,
    syst: &mut SYST,
) -> Pass {
    FRAMES_OUT.store(0, Ordering::Relaxed);
    let mut counter = Counter::start(instrument, syst);

    let deck: &'static mut ProbeDeck = deck_cell.init(Deck::new(sink(), Quality::Fast));
    let src = SliceSource::new(MKA_FLAC_FIXTURE);
    let decoder = open_seekable(src, scratch).unwrap();
    deck.load(decoder).unwrap();

    let mut pumps = 0;
    let mut hit_cap = true;
    while pumps < MAX_PUMPS {
        pumps += 1;
        let progress = deck.pump();
        // Sample every pump: keeps SysTick's 24-bit window from overflowing and
        // costs a handful of instructions against ~1e6 of decode work.
        counter.sample(syst);
        match progress {
            Ok(Progress::Played) => {}
            // Ended/Idle: fixture fully decoded — the comparable stopping point.
            Ok(_) | Err(_) => {
                hit_cap = false;
                break;
            }
        }
    }

    let ticks = counter.total(syst);
    Pass {
        ticks,
        frames: FRAMES_OUT.load(Ordering::Relaxed),
        pumps,
        hit_cap,
        suspect_overflow: counter.suspect_overflow,
    }
}

/// `ticks * SINK_RATE / frames`, in u64 so a long fixture cannot overflow.
fn ticks_per_audio_second(p: &Pass) -> u64 {
    if p.frames == 0 {
        return 0;
    }
    p.ticks * u64::from(SINK_RATE) / u64::from(p.frames)
}

/// RTF at `f_hz`, x100 so it prints without float formatting: `250` == 2.50x.
/// Only meaningful when the instrument counts CYCLES; with SysTick-under-icount
/// the denominator is instructions, so this is `RTF * CPI` and the script labels
/// it accordingly.
fn rtf_centi(p: &Pass, f_hz: u64) -> u64 {
    let t = ticks_per_audio_second(p);
    if t == 0 {
        return 0;
    }
    f_hz * 100 / t
}

fn speedup_centi(base: &Pass, other: &Pass) -> u64 {
    if base.ticks == 0 || other.ticks == 0 {
        return 0;
    }
    base.ticks * 100 / other.ticks
}

#[entry]
fn main() -> ! {
    let mut p = Peripherals::take().unwrap();

    // Prefer true cycles; fall back to SysTick where DWT is absent (QEMU).
    p.DCB.enable_trace();
    p.DWT.enable_cycle_counter();
    let instrument = if DWT::cycle_counter_enabled() {
        Instrument::Cycles
    } else {
        Instrument::SysTick
    };

    // Free-running SysTick at the core clock, max reload. Set up regardless of
    // the chosen instrument so the fallback is always armed.
    p.SYST.set_clock_source(SystClkSource::Core);
    p.SYST.set_reload(SYST_RELOAD);
    p.SYST.clear_current();
    p.SYST.enable_counter();

    // Pass A — caches OFF: what the firmware ships as today. The M7 resets with
    // both disabled and nothing in the dependency chain turns them on. Disabled
    // explicitly anyway, so the pass is self-describing rather than relying on
    // reset state (and stays correct after a bootloader that enabled them).
    p.SCB.disable_icache();
    p.SCB.disable_dcache(&mut p.CPUID);
    let a = run_pass(
        instrument,
        SCRATCH_A.init([0; decoder_scratch_len()]),
        &DECK_A,
        &mut p.SYST,
    );

    // Pass B — I-cache only. Isolated because it is the half that is nearly free
    // to adopt: instruction fetch is read-only, so it carries none of the DMA
    // coherency hazards that make the D-cache a real decision (AN4839, and §4's
    // "caches on WITH MPU non-cacheable DMA regions, or off and documented.
    // Never partially").
    p.SCB.enable_icache();
    let b = run_pass(
        instrument,
        SCRATCH_B.init([0; decoder_scratch_len()]),
        &DECK_B,
        &mut p.SYST,
    );

    // Pass C — I-cache + D-cache: the configuration ST's "roughly 2x" refers to,
    // and the one needing MPU work before it is safe alongside DMA.
    p.SCB.enable_dcache(&mut p.CPUID);
    let c = run_pass(
        instrument,
        SCRATCH_C.init([0; decoder_scratch_len()]),
        &DECK_C,
        &mut p.SYST,
    );

    // VALIDATE first: every fact that decides whether the numbers below mean
    // anything — which instrument ran, whether all three passes decoded the SAME
    // audio (otherwise incomparable), whether any pass was truncated, and whether
    // the 24-bit SysTick window was ever at risk of a missed wrap.
    let comparable = a.frames == b.frames && b.frames == c.frames && a.frames > 0;
    cortex_m_semihosting::hprintln!(
        "mcu-rtfprobe: VALIDATE instrument={} comparable={} frames={}/{}/{} pumps={}/{}/{} hit_cap={}/{}/{} suspect_overflow={}/{}/{} audio_ms={}",
        instrument.as_str(),
        comparable,
        a.frames, b.frames, c.frames,
        a.pumps, b.pumps, c.pumps,
        a.hit_cap, b.hit_cap, c.hit_cap,
        a.suspect_overflow, b.suspect_overflow, c.suspect_overflow,
        u64::from(a.frames) * 1000 / u64::from(SINK_RATE)
    );
    cortex_m_semihosting::hprintln!(
        "mcu-rtfprobe: caches=off ticks={} ticks_per_audio_s={} rtf_centi_at_280mhz={}",
        a.ticks,
        ticks_per_audio_second(&a),
        rtf_centi(&a, 280_000_000)
    );
    cortex_m_semihosting::hprintln!(
        "mcu-rtfprobe: caches=i   ticks={} ticks_per_audio_s={} rtf_centi_at_280mhz={}",
        b.ticks,
        ticks_per_audio_second(&b),
        rtf_centi(&b, 280_000_000)
    );
    cortex_m_semihosting::hprintln!(
        "mcu-rtfprobe: caches=id  ticks={} ticks_per_audio_s={} rtf_centi_at_280mhz={}",
        c.ticks,
        ticks_per_audio_second(&c),
        rtf_centi(&c, 280_000_000)
    );
    cortex_m_semihosting::hprintln!(
        "mcu-rtfprobe: speedup_centi i_vs_off={} id_vs_off={}",
        speedup_centi(&a, &b),
        speedup_centi(&a, &c)
    );

    cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
    // `debug::exit` ends the QEMU run, so this is only reached on silicon (where
    // semihosting exit is a no-op without a debugger attached). WFI rather than a
    // spin so a real board idles instead of burning current in a hot loop.
    loop {
        cortex_m::asm::wfi();
    }
}
