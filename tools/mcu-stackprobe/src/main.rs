//! Bare-metal stack-measurement harness for soul-audio-core's open→decode→pump
//! path on real thumbv7em (Cortex-M7) codegen.
//!
//! This exists to answer one question cleanly: the DAP's open-path stack
//! transient was measured on HOST x86_64 at ~430 KB (release) — but that
//! number over-counts, because the host build copies the 80 KB `AnyDecoder`
//! enum by value through several call frames, while a thumbv7em release
//! (opt-3) build uses sret / in-place construction instead. This crate forces
//! the real `open_seekable` → `Deck::load` (`build_track`) → `Deck::pump`
//! (`produce_block` → `decode_into`) call chain into a linked thumbv7em ELF so
//! `-Z emit-stack-sizes` + `llvm-readobj --stack-sizes` can report each
//! function's REAL on-target frame size (see the repo's measurement notes for
//! how the top frames are summed into a worst-case transient).
//!
//! The `qemu` feature turns the SAME binary into a DYNAMIC measurement: built
//! for QEMU's `mps2-an500` (Cortex-M7), it paints the free stack with a
//! sentinel, runs the workload for real, and scans for the high-water (see
//! `check-stack-qemu.sh` and the `stackmeasure` module below). The number that
//! matters is the TRANSIENT = stack growth BELOW the call site
//! (`painted_hi - deepest`), i.e. the extra stack a task incurs calling into the
//! decode path. Worst case (Matroska-FLAC) is ~297 KiB and FITS the board's
//! 592 KiB headroom; the per-format sweep (Opus-in-WebM 221, WAV 77, WavPack 62,
//! ALAC-CAF 59 KiB) confirms nothing exceeds it. (An earlier revision reported
//! ~1081 KiB and called it a blocker — that was a measurement bug: it measured
//! depth from the top of RAM, counting this harness's own bloated `main` frame,
//! not growth below the call site. Fixed 2026-11-19 — see `measure`'s doc and
//! `docs/plans/2026-11-19-flac-stack-externalization.md`.)
//!
//! Two episodes run back to back in the same `main`, so BOTH are genuinely
//! *exercised* (open()'d and decoded, not just type-reachable by a `match`
//! arm the fixture never actually takes):
//!
//!   1. A 44.1 kHz WAV (the sink is fixed at 48 kHz), forcing `Deck::build_track`
//!      to also construct a real `Resampler` — the bit-perfect-pass-through
//!      path alone wouldn't exercise `make_resampler`.
//!   2. A Matroska/WebM-wrapped FLAC track — the worst-case open-path frame
//!      (`FlacDecoder<MatroskaFlacReader<S>, P>::new_in`, `check-stack.sh`'s
//!      "largest other frame"). Before this harness exercised it, that
//!      function's frame only showed up in `.stack_sizes` because every
//!      `MatroskaDecoder::from_demuxer_in` codec arm gets monomorphized and
//!      linked in regardless of which arm the fixture's bytes actually take
//!      (the compiler can't prove a `match` on runtime-read bytes dead) — so
//!      the *function's own* frame was always measured, but `main`'s own
//!      frame (the number actually gated) reflected only the WAV episode's
//!      locals/inlining. Actually opening+decoding this fixture closes that
//!      gap.
//!
//! It is deliberately a throwaway measurement rig, not product firmware: the
//! fixtures need not even decode cleanly (though they do) — the point is to
//! get every function on the path compiled into the binary so its frame size
//! lands in the `.stack_sizes` ELF section. After both episodes load, it runs
//! a plain `loop { pump() }` on the second (worst-case) track; nothing reads
//! the "output" (a `DiscardSink`).
#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;
use static_cell::StaticCell;

use soul_audio_core::{
    decoder_scratch_len, open_seekable, AnyDecoder, Deck, Quality, SampleFormat, Sink, SinkError,
    SliceSource, StreamInfo,
};

/// Dynamic stack-painting measurement — compiled only for the `qemu` feature.
/// Paint the free stack with a sentinel, run the real decode workload, then scan
/// for the deepest word that was overwritten = the stack high-water. Validated
/// against a known-depth recursion before being wired to the decode path. This
/// `unsafe` is confined to the throwaway measurement rig (the shipped `dap/`
/// crates are `forbid(unsafe_code)`; this tool crate is not on that list).
#[cfg(feature = "qemu")]
mod stackmeasure {
    // cortex-m-rt's link.x provides these: `_stack_start` = top of RAM (the stack
    // grows down from it); `__sheap` = top of all static storage = the lowest the
    // stack can reach. Free stack is [__sheap .. _stack_start).
    extern "C" {
        static _stack_start: u8;
        static __sheap: u8;
    }

    const SENTINEL: u32 = 0xAAAA_AAAA;

    fn sheap() -> usize {
        (core::ptr::addr_of!(__sheap) as usize + 3) & !3
    }
    fn stack_start() -> usize {
        core::ptr::addr_of!(_stack_start) as usize
    }

    #[inline(always)]
    fn sp_now() -> usize {
        let sp: usize;
        unsafe { core::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack)) };
        sp
    }

    /// Fill [__sheap .. sp) with SENTINEL; returns the sp it painted up to.
    pub fn paint() -> usize {
        let lo = sheap();
        let hi = sp_now() - 64; // small margin below the current frame
        let mut p = lo;
        while p + 4 <= hi {
            unsafe { core::ptr::write_volatile(p as *mut u32, SENTINEL) };
            p += 4;
        }
        hi
    }

    /// Scan [__sheap .. painted_hi) from the bottom for the first overwritten
    /// word (the deepest the stack reached). Returns
    /// `(transient_below_paint, total_from_ram_top)` bytes.
    ///
    /// `transient_below_paint` = `painted_hi - deepest` — how far the workload
    /// grew the stack BELOW the paint point. THIS is the board-relevant number:
    /// the extra stack a task incurs by calling into the decode path, to compare
    /// against the board's free-stack headroom. It is 0 if nothing was
    /// overwritten (e.g. measuring immediately after `paint`).
    ///
    /// `total_from_ram_top` = `_stack_start - deepest` — the whole stack depth
    /// from the top of RAM. This is PROBE-SPECIFIC: it also counts everything
    /// above the paint point (this harness's own bloated `main` frame — it holds
    /// two episodes' decoders, both fixtures, and every measurement local at
    /// once), which a real board task never carries. Reported only for context;
    /// do NOT compare it against the board headroom (that was the original
    /// apples-to-oranges error that over-reported the open transient ~3x).
    pub fn measure(painted_hi: usize) -> (usize, usize) {
        let lo = sheap();
        let top = stack_start();
        let mut deepest = painted_hi;
        let mut p = lo;
        while p + 4 <= painted_hi {
            if unsafe { core::ptr::read_volatile(p as *const u32) } != SENTINEL {
                deepest = p;
                break;
            }
            p += 4;
        }
        (painted_hi - deepest, top - deepest)
    }
}

/// A minimal valid RIFF/WAVE fixture: the standard 44-byte canonical header
/// (no extra `fmt` chunk padding/extensions) followed by 8 interleaved 16-bit
/// stereo samples (4 frames) — just enough for `probe()` to recognize it as
/// WAV and `WavDecoder` to decode real PCM out of it. 44.1 kHz on purpose
/// (not 48 kHz): the sink below is fixed at 48 kHz, so loading this track
/// forces `Deck::build_track` to also construct a real `Resampler`
/// (`make_resampler`), exercising that part of the open→decode→pump chain
/// too, not just the bit-perfect pass-through path.
///
/// Byte layout (all multi-byte fields little-endian, per the RIFF spec):
///   "RIFF" | chunkSize=52 | "WAVE"
///   "fmt " | fmtSize=16 | audioFormat=1 (PCM) | channels=2 | sampleRate=44100
///         | byteRate=176400 | blockAlign=4 | bitsPerSample=16
///   "data" | dataSize=16 | 8 × i16 samples: 1000,-1000, 2000,-2000, 3000,-3000, 4000,-4000
#[rustfmt::skip]
const WAV_FIXTURE: &[u8] = &[
    // "RIFF"
    0x52, 0x49, 0x46, 0x46,
    // chunkSize = 52
    0x34, 0x00, 0x00, 0x00,
    // "WAVE"
    0x57, 0x41, 0x56, 0x45,
    // "fmt "
    0x66, 0x6D, 0x74, 0x20,
    // fmtSize = 16
    0x10, 0x00, 0x00, 0x00,
    // audioFormat = 1 (PCM)
    0x01, 0x00,
    // numChannels = 2
    0x02, 0x00,
    // sampleRate = 44100
    0x44, 0xAC, 0x00, 0x00,
    // byteRate = 44100 * 2 * 2 = 176400
    0x10, 0xB1, 0x02, 0x00,
    // blockAlign = 2 * 16/8 = 4
    0x04, 0x00,
    // bitsPerSample = 16
    0x10, 0x00,
    // "data"
    0x64, 0x61, 0x74, 0x61,
    // dataSize = 16 (8 samples * 2 bytes)
    0x10, 0x00, 0x00, 0x00,
    // PCM data: 1000, -1000, 2000, -2000, 3000, -3000, 4000, -4000 (i16 LE)
    0xE8, 0x03, 0x18, 0xFC, 0xD0, 0x07, 0x30, 0xF8,
    0xB8, 0x0B, 0x48, 0xF4, 0xA0, 0x0F, 0x60, 0xF0,
];

/// A tiny Matroska/WebM-wrapped FLAC (`A_FLAC`) track — the worst-case
/// open-path format this harness exists to measure (see the module doc
/// comment's episode 2, and `check-stack.sh`'s "largest other frame":
/// `FlacDecoder<MatroskaFlacReader<S>, P>::new_in`). Reused byte-for-byte from
/// soul-audio-core's own test corpus (a short stereo tone) rather than
/// hand-built: a real FLAC-in-Matroska bitstream needs correct EBML framing
/// *and* real FLAC frame headers/CRCs, which isn't worth hand-rolling here
/// when a tiny, already-exercised fixture exists next door. Path is relative
/// to this source file (`include_bytes!`'s own rule), not the crate root —
/// four `../` from `src/` reaches the `soulaudio/` directory both repos are
/// checked out under, matching this crate's `soul-audio-core` path
/// dependency in `Cargo.toml` (three `../` from *that* file's directory, the
/// crate root one level up from `src/`).
const MKA_FLAC_FIXTURE: &[u8] =
    include_bytes!("../../../../soul-audio-core/testdata/tone-flac.mka");

/// Additional real fixtures for the per-format open-transient sweep (design
/// `2026-11-16-memory-and-battery-optimization-design.md` §5.1 "stack
/// high-water-mark per format"). Each exercises a different scratch-carving
/// decoder's construction nesting: WavPack (the largest shared scratch),
/// Opus-in-WebM (`MatroskaOpus`), ALAC-in-CAF (`CafAlac`). All from
/// soul-audio-core's test corpus, same path rule as `MKA_FLAC_FIXTURE`.
const WAVPACK_FIXTURE: &[u8] = include_bytes!("../../../../soul-audio-core/testdata/tone-24.wv");
const OPUS_FIXTURE: &[u8] = include_bytes!("../../../../soul-audio-core/testdata/tone-opus.webm");
const ALAC_FIXTURE: &[u8] = include_bytes!("../../../../soul-audio-core/testdata/tone-alac.caf");

/// A no-op output: write/flush succeed and discard. Fixed at 48 kHz/stereo/
/// 24-bit so the deck always has a concrete target to pipe decoded PCM into.
struct DiscardSink {
    info: StreamInfo,
}

impl Sink for DiscardSink {
    fn stream_info(&self) -> StreamInfo {
        self.info
    }
    fn write(&mut self, _frames: &[i32]) -> Result<(), SinkError> {
        Ok(())
    }
    fn flush(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
}

/// The WavPack block-staging scratch `open_seekable` needs (see
/// `decoder_scratch_len`'s doc comment) — a static, `StaticCell`-backed
/// array, so the ~256 KB buffer lives in `.bss`, not on this function's
/// stack, and `open_seekable` gets a sound `&'static mut [i32]` with no
/// `unsafe` (the same pattern `dap/board-h7a3/src/main.rs` uses). One per
/// episode: `scratch: P` is taken *by value* and moved into the constructed
/// `AnyDecoder`, so episode 1's `&'static mut [i32]` can't be recovered (not
/// even once episode 1's decoder is dropped by episode 2's `deck.load`) —
/// there is no safe "give me that borrow back" operation, only a second,
/// independent `StaticCell`. This harness isn't board-RAM-constrained (see
/// `memory.x`'s own doc comment — it's a generic, oversized region, not the
/// real H7A3 map), so two 256 KB statics cost nothing here.
static SCRATCH: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH2: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
// One more per format in the per-format open sweep (same by-value-scratch reason
// as SCRATCH2's doc comment: each open consumes its `&'static mut [i32]`).
static SCRATCH3: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH4: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();
static SCRATCH5: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();

type ProbeDecoder = AnyDecoder<SliceSource<'static>, &'static mut [i32]>;
type ProbeDeck = Deck<DiscardSink, ProbeDecoder, 0>;

/// The `Deck` itself (~199 KB — it embeds a full `AnyDecoder` by value inside
/// its `Track`), board-representative: the real board-h7a3 firmware holds its
/// player in a `StaticCell`, off the calling task's stack (see
/// `dap/board-h7a3/src/main.rs`'s own `SCRATCH` doc comment for the same
/// pattern). Measuring `main`'s frame with `Deck` as a `main`-local (as the
/// first iteration of this harness did) over-counts the real transient by
/// `Deck`'s own static size, since a real caller never carries that weight on
/// its own stack.
static DECK: StaticCell<ProbeDeck> = StaticCell::new();

#[entry]
fn main() -> ! {
    // Paint the free stack BEFORE any deep work. `main`'s own frame is already
    // reserved by its prologue, so painting from here down captures everything
    // the open->decode->pump calls below will touch.
    #[cfg(feature = "qemu")]
    let painted_hi = stackmeasure::paint();
    #[cfg(feature = "qemu")]
    let after_paint = stackmeasure::measure(painted_hi).0;

    let sink = DiscardSink {
        info: StreamInfo {
            sample_rate: 48_000,
            channels: 2,
            format: SampleFormat::Int { bits: 24 },
        },
    };

    // PRELOAD = 0: only the single-resident-decoder path matters for this
    // measurement (gapless preload is a separate, non-transient-critical
    // slot — see dap/sim/examples/stackprobe.rs's own PRELOAD=0 comment).
    let deck: &'static mut ProbeDeck = DECK.init(Deck::new(sink, Quality::Fast));

    // Episode 1: WAV (forces `make_resampler` too — see the module doc
    // comment). `unwrap()` is fine here even on failure: a panic just halts
    // (`panic_halt`) rather than corrupting anything, and either way every
    // function on this path has already been monomorphized into the binary
    // by the time the call executes, which is all `-Z emit-stack-sizes` cares
    // about.
    let scratch: &'static mut [i32] = SCRATCH.init([0; decoder_scratch_len()]);
    let src = SliceSource::new(WAV_FIXTURE);
    let decoder = open_seekable(src, scratch).unwrap();
    // qemu: WAV open transient (growth below the paint point), for per-format context.
    #[cfg(feature = "qemu")]
    let ep1_open = stackmeasure::measure(painted_hi).0;
    deck.load(decoder).unwrap();
    for _ in 0..4 {
        let _ = deck.pump();
    }

    // Episode 2: Matroska/WebM-wrapped FLAC — the worst-case open-path format
    // (see the module doc comment). `deck.load` discards episode 1's track
    // (and, with it, episode 1's scratch borrow — see `SCRATCH2`'s doc
    // comment on why this needs its own static rather than reusing `scratch`).
    let scratch2: &'static mut [i32] = SCRATCH2.init([0; decoder_scratch_len()]);
    let src2 = SliceSource::new(MKA_FLAC_FIXTURE);
    let decoder2 = open_seekable(src2, scratch2).unwrap();
    // DIAGNOSTIC: peak after episode-2 open_seekable (before load).
    #[cfg(feature = "qemu")]
    let ep2_open_peak = stackmeasure::measure(painted_hi).0;
    deck.load(decoder2).unwrap();

    // qemu: snapshot the OPEN-path peak (both episodes' decoder construction) now,
    // BEFORE any pump, to separate it from steady-state decode.
    #[cfg(feature = "qemu")]
    let open_peak = stackmeasure::measure(painted_hi).0;

    // A few pumps so the steady-state produce_block/decode_into frames are
    // exercised too.
    for _ in 0..8 {
        let _ = deck.pump();
    }

    // qemu: capture the open+pump peak (relative to the ORIGINAL paint) now,
    // BEFORE the per-format sweep below re-paints the stack.
    #[cfg(feature = "qemu")]
    let (openpump_transient, openpump_total) = stackmeasure::measure(painted_hi);

    // --- Per-format open-transient sweep (design §5.1 "stack high-water-mark
    // per format"). RE-PAINT before each open so each number is that format's
    // OWN transient (growth below the call site), not the cumulative peak; drop
    // each before the next. Compiled unconditionally so the static
    // `-Z emit-stack-sizes` build monomorphizes every decoder too; measured only
    // under qemu. These are the remaining scratch-carving decoders (WAV + the
    // Matroska-FLAC worst case are the two episodes above).
    let s3 = SCRATCH3.init([0; decoder_scratch_len()]);
    #[cfg(feature = "qemu")]
    let ph_wv = stackmeasure::paint();
    let wv = open_seekable(SliceSource::new(WAVPACK_FIXTURE), s3).unwrap();
    #[cfg(feature = "qemu")]
    let wavpack_open = stackmeasure::measure(ph_wv).0;
    drop(wv);

    let s4 = SCRATCH4.init([0; decoder_scratch_len()]);
    #[cfg(feature = "qemu")]
    let ph_op = stackmeasure::paint();
    let op = open_seekable(SliceSource::new(OPUS_FIXTURE), s4).unwrap();
    #[cfg(feature = "qemu")]
    let opus_open = stackmeasure::measure(ph_op).0;
    drop(op);

    let s5 = SCRATCH5.init([0; decoder_scratch_len()]);
    #[cfg(feature = "qemu")]
    let ph_al = stackmeasure::paint();
    let al = open_seekable(SliceSource::new(ALAC_FIXTURE), s5).unwrap();
    #[cfg(feature = "qemu")]
    let alac_open = stackmeasure::measure(ph_al).0;
    drop(al);

    // qemu: report the headline worst case (ep2 Matroska-FLAC) + the per-format
    // sweep, then exit QEMU.
    #[cfg(feature = "qemu")]
    {
        let _ = cortex_m_semihosting::hprintln!(
            "mcu-stackprobe-qemu: VALIDATE after_paint={} B (expect 0); open-path transient = {} KiB ({} B); open+pump transient = {} KiB; probe-total(ctx) = {} KiB",
            after_paint,
            open_peak / 1024,
            open_peak,
            openpump_transient / 1024,
            openpump_total / 1024,
        );
        let _ = cortex_m_semihosting::hprintln!(
            "mcu-stackprobe-qemu: per-format open transient (KiB): wav={} mka-flac={} wavpack={} opus-webm={} alac-caf={}",
            ep1_open / 1024,
            ep2_open_peak / 1024,
            wavpack_open / 1024,
            opus_open / 1024,
            alac_open / 1024,
        );
        cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);
    }

    loop {
        let _ = deck.pump();
    }
}
