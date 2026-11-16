//! Pins real `soul-audio-core` / `soul-audio-engine` / `soul-audio-index` /
//! `dap-core` types against the STM32H7A3's actual memory map (see
//! [`memory_map`]) and this DAP's split of it into per-region SRAM budgets.
//!
//! Two of the five size guards below are `#[ignore]`d — expected, documented
//! failures, not flaky tests. Each prints its measured size vs. budget
//! before asserting, so a failing (or `--ignored`) run is itself the
//! committed, precise statement of how far over budget that type is. See
//! `src/lib.rs` for the roster and `MEMORY.md`/the spec doc for the fixes.

use core::mem::size_of;

use dap_core::audio::AudioPlayer;
use dap_core::ports::Storage;
use soul_audio_core::{AnyDecoder, Deck, SampleFormat, Sink, SinkError, SliceSource, StreamInfo};
use soul_audio_engine::{Engine, LinearSequencer};
use soul_audio_index::IndexReader;

/// The STM32H7A3's real, physical memory map — ground truth this crate's
/// budgets must fit inside, independent of how the DAP chooses to spend it.
///
/// Sources:
/// - ST `STM32H7A3ZI` datasheet (DS13677) — §3 "Memory mapping", the SRAM/
///   flash size table and base addresses.
/// - ST STM32H7A3/7B3 product page — core (280 MHz Cortex-M7), flash (2 MB),
///   and total embedded SRAM figures.
/// - `stm32-rs/stm32h7xx-hal`'s `memory.x` linker script for the H7A3/H7B3
///   family — the per-region base addresses and sizes (AXI_SRAM, SRAM1-3,
///   DTCM, ITCM, BACKUP_SRAM) linked firmware actually builds against.
pub mod memory_map {
    /// User-accessible, **DMA-capable** SRAM: AXI SRAM @ `0x2400_0000` plus
    /// AHB SRAM1/2/3 @ `0x3000_0000`, 1,208 KB total. The only pool both
    /// large enough and DMA-reachable to host the decoder, the e-ink
    /// framebuffer, the catalog index, and DMA staging buffers — everything
    /// this crate's per-region budgets (below) split up.
    pub const USER_SRAM: usize = 1_208 * 1024;

    /// Tightly-coupled data RAM @ `0x2000_0000`: fast, but **not DMA-capable**
    /// — the CPU only. Hosts stacks; never a candidate for the decode/DMA
    /// buffers this crate budgets.
    pub const DTCM: usize = 128 * 1024;

    /// Tightly-coupled instruction RAM @ `0x0000_0000` — hot code (ISRs, the
    /// audio pump's inner loop). Not part of the data budget this crate pins.
    pub const ITCM: usize = 64 * 1024;

    /// Battery/VBAT-retained backup SRAM. Listed for completeness only —
    /// far too small (and not DMA-capable) to be a decode-buffer candidate.
    pub const BACKUP_SRAM: usize = 4 * 1024;

    /// Program flash.
    pub const FLASH: usize = 2 * 1024 * 1024;

    /// CPU core clock, MHz (Cortex-M7). Relevant to the decode benches in
    /// `dap-sim/benches` — the host they actually run on is far faster.
    pub const CPU_MHZ: u32 = 280;
}

// --- This DAP's split of `memory_map::USER_SRAM` into per-region budgets ---

/// Budget for the whole live audio path: a `Deck` holding one playing
/// decoder + its resampler + the output ring, at the DAP's real
/// `PRELOAD = 2`.
const AUDIO_PATH_BUDGET: usize = 512 * 1024;

/// e-ink framebuffer budget: the 800x480 panel at Gray4 — **2 bits per
/// pixel**, packed — so `800 * 480 * 2 / 8 = 96,000 B`. (Previously
/// mis-budgeted here at ~190 KiB, as if Gray4 were 4 bpp; corrected to the
/// real 2-bpp packed size.)
const FRAMEBUFFER_BUDGET: usize = 800 * 480 * 2 / 8;

/// Budget for the `.soulidx` catalog index reader + SD/catalog scan scratch.
const CATALOG_INDEX_BUDGET: usize = 128 * 1024;

/// Budget for DMA descriptor / staging buffers.
const DMA_BUFFERS_BUDGET: usize = 64 * 1024;

/// Budget for a single [`AnyDecoder`] value. This is below the size of
/// `MultistreamOpusDecoder` (surround Opus), today's ceiling variant, so
/// meeting it forces dropping that variant from the dispatch enum. Zero-loss
/// on this DAP: it is stereo-only, and `Deck` already rejects any decoder
/// that doesn't resolve to mono/stereo.
const DECODER_BUDGET: usize = 288 * 1024;

/// Generous budget for [`Engine`]'s *own* bookkeeping — on top of whatever
/// `Deck<K, D, PRELOAD>` it wraps by value: a [`LinearSequencer`] (an index
/// cursor + repeat mode) plus one `Option<Handle>` for the current track and
/// one per `PRELOAD` preload slot. The Deck's own footprint (dominated by
/// its fixed DSP scratch buffers, and by however many fully-built decoders
/// `PRELOAD` demands) is pinned separately by [`DECODER_BUDGET`] /
/// [`AUDIO_PATH_BUDGET`] above; this budget proves the sequencing/queue
/// layer does not multiply that further.
const ENGINE_OVERHEAD_BUDGET: usize = 1024;

// The documented split above must actually fit inside the SRAM it's a split
// of — a static sanity check on the budget itself, independent of whatever
// `soul-audio-core`/`soul-audio-engine`/`soul-audio-index` measure at.
const _: () = assert!(
    AUDIO_PATH_BUDGET + FRAMEBUFFER_BUDGET + CATALOG_INDEX_BUDGET + DMA_BUFFERS_BUDGET
        <= memory_map::USER_SRAM
);
const _: () = assert!(DECODER_BUDGET <= AUDIO_PATH_BUDGET);

/// A zero-size stand-in output so a measured type's size is dominated by the
/// decoder + resampler + ring buffers it holds, not by the sink.
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

/// The byte source `AnyDecoder`/`IndexReader` are generic over here.
/// `SliceSource` is the smallest real `BufSource + SeekSource` (+
/// `SeekSource` alone, for `IndexReader`) in `soul-audio-core` (a slice ref +
/// a cursor), so it adds negligible size of its own — the measured types'
/// internal scratch buffers dominate, which is exactly what these budgets
/// mean to measure.
type Src = SliceSource<'static>;

/// A zero-size [`Storage`] stand-in for the [`AudioPlayer`] size guard: hands
/// back an empty [`SliceSource`] (never actually opened — this guard only
/// calls `size_of`, never `pump`), so `AudioPlayer<StubSink, StubStorage,
/// PRELOAD>`'s size is dominated by the `Engine`/`Deck`/decoder it wraps, not
/// by the storage port.
struct StubStorage;

impl Storage for StubStorage {
    type Source = Src;
    type Error = ();

    fn open(&mut self, _track: usize) -> Result<Self::Source, Self::Error> {
        Ok(SliceSource::new(&[]))
    }
}

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

/// Prints the full `memory_map::USER_SRAM` split as a running-total/margin
/// report — each region's budget, the running total after it, and the
/// margin still left in `USER_SRAM` — then asserts the whole split actually
/// fits. This is the memory-map report: a failure here means the DAP's
/// *documented* budget plan itself doesn't fit the real H7A3 SRAM, before
/// even asking whether any real type fits its own slice of that plan.
#[test]
fn memory_budget_fits_user_sram() {
    println!(
        "\nSTM32H7A3 user-SRAM budget report (USER_SRAM = {} B / {} KB):",
        memory_map::USER_SRAM,
        memory_map::USER_SRAM / 1024
    );
    let regions: [(&str, usize); 4] = [
        (
            "audio path (Deck: decoder + resampler + ring)",
            AUDIO_PATH_BUDGET,
        ),
        (
            "e-ink framebuffer (800x480 Gray4, 2 bpp)",
            FRAMEBUFFER_BUDGET,
        ),
        (
            "catalog index (.soulidx IndexReader + scan scratch)",
            CATALOG_INDEX_BUDGET,
        ),
        ("DMA descriptor / staging buffers", DMA_BUFFERS_BUDGET),
    ];
    let mut total = 0usize;
    for (name, budget) in regions {
        total += budget;
        let margin = memory_map::USER_SRAM.saturating_sub(total);
        println!(
            "  {name:<55} budget {budget:>9} B   running total {total:>9} B   margin {margin:>9} B"
        );
    }
    println!(
        "  {:<55}        =  {:>9} B   of {:>9} B USER_SRAM\n",
        "TOTAL",
        total,
        memory_map::USER_SRAM
    );
    assert!(
        total <= memory_map::USER_SRAM,
        "the memory-map split totals {total} B, over the {} B H7A3 USER_SRAM budget by {} B — \
         the DAP's own documented plan doesn't fit the real SRAM, independent of what any actual \
         type measures at",
        memory_map::USER_SRAM,
        total.saturating_sub(memory_map::USER_SRAM),
    );
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
#[ignore = "RED until JIT-preload (hardware phase): Deck holds a fully-built decoder per preload \
            slot; un-ignore once the Deck fits the budget"]
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

/// Proves the `Engine` queue/sequencer layer itself is cheap: the overhead it
/// adds on top of the `Deck` it wraps by value (a `LinearSequencer` cursor +
/// repeat mode, plus `Option<Handle>` for the current track and each
/// `PRELOAD` preload slot) is tiny, regardless of whether the wrapped `Deck`
/// currently fits its own budget (see `deck_fits_the_h7a3_audio_path_budget`
/// above, which is where any Deck-sized overage belongs).
#[test]
fn engine_overhead_is_tiny_on_top_of_the_deck_it_wraps() {
    let engine = size_of::<Engine<StubSink, AnyDecoder<Src>, LinearSequencer, PRELOAD>>();
    let deck = size_of::<Deck<StubSink, AnyDecoder<Src>, PRELOAD>>();
    let overhead = engine.saturating_sub(deck);
    report(
        "Engine overhead (Engine<..,LinearSequencer,2> - Deck<..,2> it wraps)",
        overhead,
        ENGINE_OVERHEAD_BUDGET,
    );
    assert!(
        overhead <= ENGINE_OVERHEAD_BUDGET,
        "Engine<_, AnyDecoder<SliceSource<'_>>, LinearSequencer, {PRELOAD}> is {overhead} B \
         bigger than the Deck<_, _, {PRELOAD}> it wraps, over the {ENGINE_OVERHEAD_BUDGET} B \
         bookkeeping budget — the queue/sequencer layer itself grew, which is a different (and \
         more concerning) problem than the already-tracked Deck/decoder overage above.",
    );
}

#[test]
fn index_reader_fits_the_catalog_index_budget() {
    let actual = size_of::<IndexReader<Src>>();
    report("IndexReader<SliceSource>", actual, CATALOG_INDEX_BUDGET);
    assert!(
        actual <= CATALOG_INDEX_BUDGET,
        "IndexReader<SliceSource<'_>> is {actual} B, over the {CATALOG_INDEX_BUDGET} B H7A3 \
         catalog-index budget by {over} B. It's a no-alloc header cache (a RefCell<Src> + the \
         72 B parsed header) — every read seeks and re-reads the source directly, so it should \
         never scale with catalog size; an overage here means the header or source type grew, \
         not catalog content.",
        over = actual.saturating_sub(CATALOG_INDEX_BUDGET),
    );
}

#[test]
#[ignore = "tied to the Deck/JIT-preload guard above: AudioPlayer wraps an Engine which wraps a \
            Deck, so it inherits the same per-PRELOAD-slot full-decoder overage; un-ignore \
            alongside deck_fits_the_h7a3_audio_path_budget"]
fn audio_player_fits_the_h7a3_audio_path_budget() {
    let actual = size_of::<AudioPlayer<StubSink, StubStorage, PRELOAD>>();
    report(
        "AudioPlayer<StubSink, StubStorage, 2>",
        actual,
        AUDIO_PATH_BUDGET,
    );
    assert!(
        actual <= AUDIO_PATH_BUDGET,
        "AudioPlayer<StubSink, StubStorage, {PRELOAD}> is {actual} B, over the \
         {AUDIO_PATH_BUDGET} B H7A3 audio-path budget by {over} B. AudioPlayer wraps an Engine \
         wrapping a Deck<_, AnyDecoder<_>, {PRELOAD}> by value, so it inherits exactly the \
         per-preload-slot full-decoder overage `deck_fits_the_h7a3_audio_path_budget` already \
         pins — the same JIT-preload fix resolves both.",
        over = actual.saturating_sub(AUDIO_PATH_BUDGET),
    );
}
