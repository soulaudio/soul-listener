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
//! # What it measures, and why the split matters
//!
//! Two quantities, deliberately separated, because conflating them produced a
//! wrong answer once already (see the placement doc S16):
//!
//! * **decode only** — each format at its OWN native rate, so the resampler is
//!   bypassed. This is the cost that bounds the clock floor for native-rate
//!   playback, the low-power path the board is built for.
//! * **full pipeline** — 44.1 kHz source into a 48 kHz sink, so the resampler
//!   runs. The difference between the two IS the resampler, and it is the
//!   heaviest stage in the audio path: ~19.1M instructions per second of audio,
//!   22x FLAC decode and 2x the heaviest decoder.
//!
//! Running every format into one fixed sink rate measures neither cleanly. It
//! ranks the formats by how far their native rate sits from the sink, not by
//! codec cost — which is how an earlier revision concluded that Opus was the
//! CHEAPEST format. Opus is 48 kHz by spec (RFC 6716), so against a 48 kHz sink
//! it was the only format NOT being resampled; at its native rate it is in fact
//! the most expensive decoder by 11x.
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

/// How many formats the sweep covers.
const FORMATS: usize = 6;

/// The per-format sweep: every format `power::model` hardcodes a
/// `HOST_RTF_*` for, so each of those projected inputs gets a measured
/// counterpart. All are the same synthetic `tone-*` source material, so the
/// numbers differ by codec and container cost rather than by content.
///
/// **AAC is deliberately absent.** It is Via LA patent-encumbered, gated behind
/// an opt-in `aac` feature, and `dap-arch-tests`' `aac_license.rs` asserts
/// `!soul_audio_core::AAC_ENABLED` — the DAP must not link it. Measuring it here
/// would import that decoder into exactly the binary that is legally required
/// not to ship it, which is the same mistake the variant roster made (§12).
/// `HOST_RTF_AAC` therefore stays a host-only figure on purpose.
const SWEEP: [(&str, &[u8], u32); FORMATS] = [
    // WAV: no codec at all, so this is the floor — container parse only.
    (
        "wav",
        include_bytes!("../../../../../soul-audio-core/testdata/tone.wav"),
        44_100,
    ),
    // FLAC in Matroska: the DAP's realistic worst case for the stack probes, and
    // the fixture the three cache passes below also use (at 48 kHz, resampled).
    ("mka-flac", MKA_FLAC_FIXTURE, 44_100),
    (
        "mp3",
        include_bytes!("../../../../../soul-audio-core/testdata/tone.mp3"),
        44_100,
    ),
    (
        "alac-caf",
        include_bytes!("../../../../../soul-audio-core/testdata/tone-alac.caf"),
        44_100,
    ),
    // Opus: the heaviest decoder (CELT + SILK), and the format L11's "needs VOS2,
    // VOS3 unreachable" conclusion is specifically about. Opus is 48 kHz BY SPEC
    // (RFC 6716), which is why its native rate differs from the rest — and why an
    // earlier version of this sweep, which ran everything into a fixed 48 kHz
    // sink, made Opus look like the CHEAPEST format: it was the only one not
    // being resampled. See the module doc.
    (
        "opus-webm",
        include_bytes!("../../../../../soul-audio-core/testdata/tone-opus.webm"),
        48_000,
    ),
    (
        "wavpack",
        include_bytes!("../../../../../soul-audio-core/testdata/tone-24.wv"),
        44_100,
    ),
];

/// Sink rate used for the RESAMPLED measurement: 48 kHz against 44.1 kHz source
/// material, so the pipeline includes the resampler. RTF is defined against the
/// OUTPUT timeline — the sink must be fed this many frames per wall-clock second
/// or audio glitches, whatever the file's native rate is.
const SINK_RATE_RESAMPLED: u32 = 48_000;
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

// TWO decks, one per sink rate — and no more than that.
//
// `Deck::load` replaces the current track and drops the previous decoder, so one
// deck can host many formats in turn —
// the same episode pattern `mcu-stackprobe`'s main.rs uses. That matters here
// because this bin runs nine passes (six formats + three cache configurations)
// and a deck is ~165 KiB; nine of them would not fit the harness's RAM. But the
// SINK is moved into the deck at construction and its rate is fixed, so measuring
// at two output rates needs exactly two decks.
//
// Worth spelling out, because getting it wrong is SILENT: an earlier revision
// threaded a `sink_rate` argument through `run_pass` while still constructing a
// single 48 kHz deck, so every "native rate" pass was in fact still resampling
// and only the divisor changed. The numbers stayed plausible (~18M rather than
// ~0.85M) and the only visible tell was `audio_ms` drifting to 2182 for a 2 s
// fixture. `run_pass` now asserts the deck's real sink rate against the rate the
// pass claims, so the two cannot disagree again.
static DECK_44K: StaticCell<ProbeDeck> = StaticCell::new();
static DECK_48K: StaticCell<ProbeDeck> = StaticCell::new();

// The scratch, however, MUST be one per pass. It is moved into the decoder by
// value, so a pass cannot reuse the previous pass's `&'static mut [i32]` — there
// is no safe way to reclaim that borrow, only an independent `StaticCell`. Same
// reasoning as `mcu-stackprobe`'s `SCRATCH2`. Nine passes, nine scratches, at
// `decoder_scratch_len()` i32 each (~256 KiB) = ~2.25 MB of .bss, which fits the
// harness's deliberately oversized 4 MB `memory.x` region (that file is a
// generic test map, NOT the real H7A3 one — see its own doc comment).
static SCRATCH_FMT: [&StaticCell<[i32; decoder_scratch_len()]>; FORMATS] = [
    &SCRATCH_F0,
    &SCRATCH_F1,
    &SCRATCH_F2,
    &SCRATCH_F3,
    &SCRATCH_F4,
    &SCRATCH_F5,
];
static SCRATCH_F0: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_F1: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_F2: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_F3: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_F4: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_F5: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_A: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_B: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH_C: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();

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

/// One pass's result. Carries its own `sink_rate`, because the sweep runs each
/// format at its NATIVE rate (no resampler) while the cache passes run at 48 kHz
/// against 44.1 kHz source (resampler engaged) — so normalising work to a second
/// of audio cannot use one global rate.
struct Pass {
    ticks: u64,
    frames: u32,
    sink_rate: u32,
    pumps: u32,
    hit_cap: bool,
    suspect_overflow: bool,
}

fn sink(rate: u32) -> CountingSink {
    CountingSink {
        info: StreamInfo {
            sample_rate: rate,
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
    fixture: &'static [u8],
    sink_rate: u32,
    scratch: &'static mut [i32],
    deck: &mut ProbeDeck,
    syst: &mut SYST,
) -> Pass {
    FRAMES_OUT.store(0, Ordering::Relaxed);
    // The deck's sink rate IS the output rate; `sink_rate` only normalises work
    // per second of audio. If they disagree the pass measures a different
    // pipeline than it reports, so refuse rather than print a plausible wrong
    // number (see DECK_44K's note).
    assert!(
        deck.sink().stream_info().sample_rate == sink_rate,
        "run_pass: deck sink rate does not match the rate this pass claims"
    );
    let mut counter = Counter::start(instrument, syst);

    let src = SliceSource::new(fixture);
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
        sink_rate,
        pumps,
        hit_cap,
        suspect_overflow: counter.suspect_overflow,
    }
}

/// `ticks * sink_rate / frames`, in u64 so a long fixture cannot overflow.
fn ticks_per_audio_second(p: &Pass) -> u64 {
    if p.frames == 0 {
        return 0;
    }
    p.ticks * u64::from(p.sink_rate) / u64::from(p.frames)
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

    // `Quality::Fast` matches `dap_core::app`'s own `AppConfig` default, so the
    // resampler cost measured here is the one the board actually pays.
    let deck_44k: &'static mut ProbeDeck = DECK_44K.init(Deck::new(sink(44_100), Quality::Fast));
    let deck_48k: &'static mut ProbeDeck =
        DECK_48K.init(Deck::new(sink(SINK_RATE_RESAMPLED), Quality::Fast));

    // Caches OFF for the whole format sweep: that is what the firmware ships as
    // today (the M7 resets with both disabled and nothing in the dependency chain
    // turns them on). Disabled explicitly rather than relying on reset state, so
    // the sweep stays correct after a bootloader that enabled them.
    p.SCB.disable_icache();
    p.SCB.disable_dcache(&mut p.CPUID);

    // --- Per-format sweep -------------------------------------------------
    // One row per format the power model projects a HOST_RTF_* for, so every
    // projected input gains a measured counterpart. Printed as it goes, because a
    // format that fails to decode should still leave the earlier rows readable.
    // Each format at its OWN native rate, so the resampler is bypassed and the
    // figure is DECODE cost alone. Measuring every format into one fixed sink rate
    // instead measures "decode + whatever resampling that mismatch implies", which
    // is dominated by the resampler and ranks the formats wrongly (see the module
    // doc's note on Opus).
    let mut worst_name = "none";
    let mut worst_per_s = 0u64;
    let mut flac_native_per_s = 0u64;
    for (i, (name, fixture, native_rate)) in SWEEP.iter().enumerate() {
        let scratch = SCRATCH_FMT[i].init([0; decoder_scratch_len()]);
        // The deck whose sink ALREADY runs at this fixture's native rate, so the
        // resampler is genuinely bypassed rather than nominally so.
        let deck: &mut ProbeDeck = if *native_rate == 44_100 {
            &mut *deck_44k
        } else {
            &mut *deck_48k
        };
        let r = run_pass(
            instrument,
            fixture,
            *native_rate,
            scratch,
            deck,
            &mut p.SYST,
        );
        let per_s = ticks_per_audio_second(&r);
        if per_s > worst_per_s {
            worst_per_s = per_s;
            worst_name = name;
        }
        if *name == "mka-flac" {
            flac_native_per_s = per_s;
        }
        cortex_m_semihosting::hprintln!(
            "mcu-rtfprobe: decode_only format={} sink_hz={} ticks={} frames={} audio_ms={} ticks_per_audio_s={} hit_cap={} suspect_overflow={}",
            name,
            native_rate,
            r.ticks,
            r.frames,
            u64::from(r.frames) * 1000 / u64::from(*native_rate),
            per_s,
            r.hit_cap,
            r.suspect_overflow
        );
    }
    cortex_m_semihosting::hprintln!(
        "mcu-rtfprobe: decode_only worst_format={} worst_ticks_per_audio_s={}",
        worst_name,
        worst_per_s
    );

    // --- Cache sweep, on the Matroska-FLAC path ---------------------------
    // Pass A — caches off (the shipped configuration), the baseline for the two
    // ratios below.
    let a = run_pass(
        instrument,
        MKA_FLAC_FIXTURE,
        SINK_RATE_RESAMPLED,
        SCRATCH_A.init([0; decoder_scratch_len()]),
        deck_48k,
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
        MKA_FLAC_FIXTURE,
        SINK_RATE_RESAMPLED,
        SCRATCH_B.init([0; decoder_scratch_len()]),
        deck_48k,
        &mut p.SYST,
    );

    // Pass C — I-cache + D-cache: the configuration ST's "roughly 2x" refers to,
    // and the one needing MPU work before it is safe alongside DMA.
    p.SCB.enable_dcache(&mut p.CPUID);
    let c = run_pass(
        instrument,
        MKA_FLAC_FIXTURE,
        SINK_RATE_RESAMPLED,
        SCRATCH_C.init([0; decoder_scratch_len()]),
        deck_48k,
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
        u64::from(a.frames) * 1000 / u64::from(SINK_RATE_RESAMPLED)
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

    // The resampler, attributed. Same fixture, same codec, same cache state —
    // the only difference between `a` (48 kHz sink) and the sweep's mka-flac row
    // (44.1 kHz native) is whether the resampler runs. That difference is the
    // single largest term in the DAP's decode-path CPU cost, and the reason the
    // board carries a dual MCLK oscillator so the SAI can clock natively and skip
    // it entirely (see power::model's native_rate_clocking test).
    let flac_resampled_per_s = ticks_per_audio_second(&a);
    let resampler_per_s = flac_resampled_per_s.saturating_sub(flac_native_per_s);
    let resample_x100 = (flac_resampled_per_s * 100)
        .checked_div(flac_native_per_s)
        .unwrap_or(0);
    cortex_m_semihosting::hprintln!(
        "mcu-rtfprobe: resampler flac_native_per_s={} flac_resampled_per_s={} resampler_per_s={} resampled_vs_native_centi={}",
        flac_native_per_s,
        flac_resampled_per_s,
        resampler_per_s,
        resample_x100
    );

    cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
    // `debug::exit` ends the QEMU run, so this is only reached on silicon (where
    // semihosting exit is a no-op without a debugger attached). WFI rather than a
    // spin so a real board idles instead of burning current in a hot loop.
    loop {
        cortex_m::asm::wfi();
    }
}
