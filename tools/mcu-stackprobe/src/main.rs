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
//! It is deliberately a throwaway measurement rig, not product firmware: the
//! fixture WAV need not even decode successfully (though it does) — the point
//! is to get every function on the path compiled into the binary so its frame
//! size lands in the `.stack_sizes` ELF section. Runs in a plain `loop {
//! pump() }`; nothing reads the "output" (a `DiscardSink`).
#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;
use static_cell::StaticCell;

use soul_audio_core::{
    AnyDecoder, Deck, Quality, SampleFormat, Sink, SinkError, SliceSource, StreamInfo,
    decoder_scratch_len, open_seekable,
};

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
const FIXTURE: &[u8] = &[
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
/// `unsafe` (the same pattern `dap/board-h7a3/src/main.rs` uses).
static SCRATCH: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();

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

    let scratch: &'static mut [i32] = SCRATCH.init([0; decoder_scratch_len()]);
    let src = SliceSource::new(FIXTURE);

    // The real open->decode->pump chain this harness exists to force into
    // the ELF. `unwrap()` is fine here even on failure: a panic just halts
    // (`panic_halt`) rather than corrupting anything, and either way every
    // function on this path has already been monomorphized into the binary
    // by the time the call executes, which is all `-Z emit-stack-sizes` cares
    // about.
    let decoder = open_seekable(src, scratch).unwrap();
    deck.load(decoder).unwrap();

    loop {
        let _ = deck.pump();
    }
}
