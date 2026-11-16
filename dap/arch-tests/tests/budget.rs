//! Pins the exact AXI-SRAM overage of today's audio decode path against the
//! STM32H7A3 budget (see `src/lib.rs` for the full story and the fix).
//!
//! RED BY DESIGN: these assertions are expected to fail until
//! `soul-audio-core` gains an `embedded-profile` (drops the dead surround
//! Opus variant) and the DAP's preload policy goes JIT (stops holding a
//! fully-built decoder per preload slot). Each test prints the measured
//! size, the budget, and the overage before asserting, so a failing run is
//! itself the committed, precise statement of how far over budget we are.

use core::mem::size_of;

use soul_audio_core::{AnyDecoder, Deck, SampleFormat, Sink, SinkError, SliceSource, StreamInfo};

/// The H7A3's main AXI SRAM: ~1 MB, the only pool both large enough and
/// DMA-capable to host decode buffers (as opposed to the separate 128 KB
/// DTCM, where stacks live and which this budget does not cover).
const AXI_SRAM_BYTES: usize = 1_048_576;

/// Budget for the whole live audio path: a `Deck` holding one playing
/// decoder + its resampler + the output ring, at the DAP's real
/// `PRELOAD = 2`.
///
/// Documented split of [`AXI_SRAM_BYTES`] (only this slice is enforced by
/// this crate; the rest is recorded here so the budget is auditable as a
/// whole):
///   - audio path (this budget):        512 KiB
///   - e-ink framebuffer (Gray4):        ~190 KiB
///   - catalog / SD scan buffers:        ~128 KiB
///   - DMA descriptor / staging:          ~64 KiB
///   - margin:                       the remainder (~174 KiB)
const AUDIO_PATH_BUDGET: usize = 512 * 1024;

/// Budget for a single [`AnyDecoder`] value. This is below the size of
/// `MultistreamOpusDecoder` (surround Opus), today's ceiling variant, so
/// meeting it forces dropping that variant from the dispatch enum. Zero-loss
/// on this DAP: it is stereo-only, and `Deck` already rejects any decoder
/// that doesn't resolve to mono/stereo.
const DECODER_BUDGET: usize = 288 * 1024;

// The documented split above must actually fit inside the SRAM it's a split
// of — a static sanity check on the budget itself, independent of whatever
// `soul-audio-core` measures at.
const _: () = assert!(AUDIO_PATH_BUDGET <= AXI_SRAM_BYTES);

/// A zero-size stand-in output so a `Deck`'s measured size is dominated by
/// the decoder + resampler + ring buffers it holds, not by the sink.
struct StubSink;

impl Sink for StubSink {
    fn stream_info(&self) -> StreamInfo {
        StreamInfo {
            sample_rate: 48_000,
            channels: 2,
            format: SampleFormat::Int { bits: 24 },
        }
    }

    fn write(&mut self, _frames: &[i32]) -> Result<(), SinkError> {
        Ok(())
    }

    fn flush(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
}

/// The byte source `AnyDecoder` is generic over here. `SliceSource` is the
/// smallest real `BufSource + SeekSource` in `soul-audio-core` (a slice ref +
/// a cursor), so it adds negligible size of its own — the decoder variants'
/// internal scratch buffers dominate, which is exactly what this budget
/// means to measure.
type Src = SliceSource<'static>;

/// The DAP's real preload depth (`dap_core::audio::AudioPlayer`'s const —
/// see `dap/sim/tests/audio_headless.rs`).
const PRELOAD: usize = 2;

/// Print `name`'s measured size against `budget` before the caller asserts,
/// so a failing test's output is a precise, self-contained report.
fn report(name: &str, actual: usize, budget: usize) {
    if actual > budget {
        let over = actual - budget;
        println!("{name}: {actual} B vs budget {budget} B — OVER by {over} B");
    } else {
        let spare = budget - actual;
        println!("{name}: {actual} B vs budget {budget} B — within budget ({spare} B to spare)");
    }
}

#[test]
fn any_decoder_fits_the_h7a3_decoder_budget() {
    let actual = size_of::<AnyDecoder<Src>>();
    report("AnyDecoder<SliceSource>", actual, DECODER_BUDGET);
    assert!(
        actual <= DECODER_BUDGET,
        "AnyDecoder<SliceSource<'_>> is {actual} B, over the {DECODER_BUDGET} B H7A3 decoder \
         budget by {over} B. The ceiling variant is MultistreamOpusDecoder (surround Opus); drop \
         it via soul-audio-core's embedded-profile — this DAP is stereo-only and Deck already \
         rejects non-stereo decoders, so this is zero-loss. Every other format is retained.",
        over = actual.saturating_sub(DECODER_BUDGET),
    );
}

#[test]
fn deck_fits_the_h7a3_audio_path_budget() {
    let actual = size_of::<Deck<StubSink, AnyDecoder<Src>, PRELOAD>>();
    report(
        "Deck<StubSink, AnyDecoder<SliceSource>, 2>",
        actual,
        AUDIO_PATH_BUDGET,
    );
    assert!(
        actual <= AUDIO_PATH_BUDGET,
        "Deck<_, AnyDecoder<SliceSource<'_>>, {PRELOAD}> is {actual} B, over the \
         {AUDIO_PATH_BUDGET} B H7A3 audio-path budget by {over} B. Each preload slot holds a \
         fully-built AnyDecoder + resampler (~343 KB apiece) on top of the live decoder — fix via \
         JIT preload (stop holding built decoders per slot), on top of the DECODER_BUDGET fix \
         above.",
        over = actual.saturating_sub(AUDIO_PATH_BUDGET),
    );
}
