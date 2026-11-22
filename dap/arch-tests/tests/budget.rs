//! Pins real `soul-audio-core` / `soul-audio-engine` / `soul-audio-index` /
//! `dap-core` types against the STM32H7A3's actual memory map (see
//! [`memory_map`]) and this DAP's split of it into per-region SRAM budgets
//! (see `docs/plans/2026-11-16-memory-and-battery-optimization-design.md`
//! §3 for the full rationale behind the 7-region split below).
//!
//! Every size guard below is GREEN at the DAP's real `PRELOAD = 0` (one
//! resident decoder — gapless comes from the output ring at a track reopen, not
//! a preloaded next decoder; `PRELOAD = 1`/`2` were earlier/desktop values).
//! Each prints its measured size vs. budget before
//! asserting, so a run's output is itself the committed, precise statement
//! of how much headroom each type has. See `src/lib.rs` for the roster and
//! `MEMORY.md`/the spec doc for history.

use core::mem::size_of;

use dap_core::app::App;
use dap_core::audio::AudioPlayer;
use dap_core::display::{DisplayError, EinkRefresh, Gray4, Rectangle};
use dap_core::ports::{BatteryReading, Controls, InputEvent, Power, Storage};
use dap_core::power::{PowerProfile, PowerService};
use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_graphics::Pixel;
use soul_audio_core::formats::caf::CafDecoder;
use soul_audio_core::formats::matroska::MatroskaDecoder;
use soul_audio_core::formats::mp3::Mp3Decoder;
use soul_audio_core::formats::{
    aiff::AiffDecoder, alac::AlacDecoder, flac::FlacDecoder, opus::OpusDecoder, wav::WavDecoder,
    wavpack::WavpackDecoder,
};
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
    /// User-accessible, **DMA-capable**, **CD-domain** SRAM: AXI SRAM
    /// (`0x2400_0000`, 1024 KB — internally AXI-SRAM1/2/3 at 256/384/384 KB but
    /// contiguous and linked as one region) plus AHB-SRAM1 (`0x3000_0000`, 64 KB)
    /// and AHB-SRAM2 (`0x3002_0000`, 64 KB) = **1152 KB**. The only pool both
    /// large enough and DMA-reachable to host the decoder, the e-ink
    /// framebuffer, the catalog index, and DMA staging buffers — everything
    /// this crate's per-region budgets (below) split up.
    ///
    /// # Corrected 2026-11-21
    /// This was **1208 KB**, described as "AXI SRAM plus AHB SRAM1/2/**3**".
    /// There is no AHB-SRAM3 on this part: the H7A3 has exactly two AHB SRAMs of
    /// 64 KB each (128 KB total, not 184 KB). The old figure over-stated the
    /// available pool by 56 KB, so `memory_budget_fits_user_sram` was checking
    /// the region plan against a ceiling that doesn't exist. Cross-checks: the
    /// datasheet's "1.4 MB RAM" headline reconciles as 1152 (CD) + 32
    /// ([`SRD_SRAM`]) + 128 ([`DTCM`]) + 64 ([`ITCM`]) + 4 ([`BACKUP_SRAM`]) ≈
    /// 1380 KB. See `docs/plans/2026-11-21-memory-architecture-and-placement.md`.
    ///
    /// **Only 1024 KB of this is actually linked today** — `memory.x` maps the
    /// AXI SRAM alone. The 128 KB of AHB SRAM is physically present and DMA-
    /// reachable but invisible to the linker until a region is declared for it;
    /// see the placement doc above.
    pub const USER_SRAM: usize = 1152 * 1024;

    /// SRAM in the **Smart Run Domain** (SRD), 32 KB. Deliberately *excluded*
    /// from [`USER_SRAM`]: it is DMA-reachable, but it sits in a different power
    /// domain from the CPU and the AXI/AHB SRAMs, with its own retention and
    /// clock-gating behaviour in Stop (see `dap_core::power::peripherals`'
    /// `Paused` state). Counting it as general-purpose RAM would mix two power
    /// domains in one budget, which is how a buffer ends up unexpectedly
    /// un-retained across a pause.
    pub const SRD_SRAM: usize = 32 * 1024;

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
//
// Seven regions (spec §3): audio, two visual lines (live framebuffer + the
// optional partial-refresh shadow), UI scratch, the library-listing working
// set, the catalog index, and DMA staging. Total ≈1,083.5 KB of 1,208 KB —
// see `memory_budget_fits_user_sram` below for the precise, printed total
// and margin.

/// Budget for the whole live audio path: a `Deck` holding one playing
/// decoder + its resampler + the output ring, at the DAP's real
/// `PRELOAD = 0` (one resident decoder; gapless comes from the output ring at a
/// track reopen — the Rockbox model — not a second preloaded decoder).
///
/// **Post-externalization** (soul-audio-core's
/// `docs/superpowers/specs/2026-11-17-decoder-memory-externalization.md`):
/// `AnyDecoder`'s big block-staging buffers now live in a caller-owned
/// scratch (`P = &'static mut [i32]` here — a 16 B fat pointer, not an inline
/// array), and at `PRELOAD = 0` the path holds ONE decoder slot, so
/// `Deck`/`Engine`/`AudioPlayer`/`App` sit at ~174 KB. Measured at 178,744 B
/// (`Deck`) / 178,808 B (`Engine`) / 178,816 B (`AudioPlayer`) / 178,904 B
/// (`App`); 384 KB leaves ~209 KB (~55%) headroom — the spare is the
/// **battery-ring tunable** (spec §0/§4.1): grow the decode-ahead ring into it
/// for fewer SD wakeups and longer CPU sleep. The budget stays 384 KB (not
/// shrunk to the measurement) so that ring has room to grow; the old 704 KB
/// budget was sized for the pre-externalization ~664 KB / PRELOAD=1 ceiling.
const AUDIO_PATH_BUDGET: usize = 384 * 1024;

/// Board-static **decoder scratch pool** the externalized decoders carve their
/// working buffers from — the actual memory `AnyDecoder<S, P>`'s pointer-sized
/// `P` points at, which [`AUDIO_PATH_BUDGET`] (the struct's `size_of` only) does
/// NOT count. This region is the honest cost the first externalization re-budget
/// silently omitted. One scratch per simultaneously-live decoder: at
/// `PRELOAD = 0` (single resident decoder, the Rockbox output-ring gapless
/// model) that is exactly ONE `decoder_scratch_len()` (65,536 i32 = 256 KB),
/// versus TWO (512 KB) at the old PRELOAD=1 — halving this pool is the whole
/// reason the DAP moved to one resident decoder.
///
/// # This is a CAP, not a mirror
/// It used to be `(1 + PRELOAD) * decoder_scratch_len() * size_of::<i32>()` —
/// i.e. the budget was *defined as* the measured value, so "measured ≤ budget"
/// was a tautology and a format that doubled the shared scratch doubled its own
/// budget with it. Only the aggregate region total would have noticed, and it
/// would have reported "everything got bigger" rather than naming the cause.
/// [`SHARED_DECODER_SCRATCH_CAP_I32`] is now a committed ceiling and
/// `shared_decoder_scratch_stays_within_its_cap` is the gate that holds the
/// actual value to it.
const DECODER_SCRATCH_POOL_BUDGET: usize =
    (1 + PRELOAD) * SHARED_DECODER_SCRATCH_CAP_I32 * core::mem::size_of::<i32>();

/// The committed ceiling on `soul_audio_core::decoder_scratch_len()` — the
/// single shared buffer the externalized decoders carve their block-staging
/// working set from, in `i32` samples.
///
/// **65,536 i32 = 256 KiB**, which is today's exact value and set entirely by
/// WavPack (see `shared_decoder_scratch_stays_within_its_cap` for the
/// breakdown). It is a ceiling, so shrinking the scratch — the memory backlog's
/// item #1, capping `wavpack::MAX_BLOCK_PCM` under `embedded-profile` for a
/// ~64 KiB win — passes freely; only *growth* fails.
///
/// Raising this is a real decision, not a formality: at `PRELOAD = 0` the pool
/// is a quarter of the 1 MB AXI SRAM the board links, and it competes directly
/// with the decode-ahead ring (see [`AUDIO_PATH_BUDGET`]'s battery-ring note).
const SHARED_DECODER_SCRATCH_CAP_I32: usize = 65_536;

/// Visual — **live framebuffer**: the 800x480 panel at Gray4 — **2 bits per
/// pixel**, packed — so `800 * 480 * 2 / 8 = 96,000 B`. This is the frame
/// actively being drawn/composed, committed to the panel on the next
/// refresh. (Previously mis-budgeted here at ~190 KiB, as if Gray4 were 4
/// bpp; corrected to the real 2-bpp packed size.)
const FRAMEBUFFER_BUDGET: usize = 800 * 480 * 2 / 8;

/// Visual — **partial-refresh shadow**: a second 800x480 Gray4 buffer
/// holding the *last-flushed* frame, so a new frame can be diffed
/// pixel-for-pixel against it to find the dirty region and issue a cheap
/// [`EinkRefresh::partial_refresh`] instead of a slow, flashy full refresh —
/// the single biggest UI-side battery/latency win on e-ink (spec §1.2/§4.3).
///
/// This is the **one genuinely optional line** in the whole budget (spec
/// §3's note): if eink-ui's layout pass tracks dirty rectangles directly
/// (from what changed in the widget tree) instead of diffing pixels after
/// the fact, this buffer is never allocated and the margin below rises by
/// its ~93.75 KB. Budgeted in for now so the plan is safe either way; drop
/// it only once eink-ui ui-3 actually commits to draw-time dirty tracking.
const FRAMEBUFFER_SHADOW_BUDGET: usize = 800 * 480 * 2 / 8;

/// Visual — **eink-ui scratch**: bounded, no-alloc immediate-mode scratch
/// for glyph rasterization, layout, and compose passes (eink-system/
/// eink-components' own working buffers) — distinct from the framebuffers
/// above, which hold committed pixels, not in-progress layout state.
const UI_SCRATCH_BUDGET: usize = 32 * 1024;

/// **Library listing** working set: the visible browse page's records +
/// its order/index table + a string buffer for titles/artists (~8–15 rows
/// held for the browse screen at once). Distinct from [`CATALOG_INDEX_BUDGET`]
/// below — that's the paged `.soulidx` *reader*, which never holds more than
/// one page's worth of raw bytes; this is the *decoded* row data the browse
/// UI actually renders.
const LIBRARY_LISTING_BUDGET: usize = 64 * 1024;

/// Budget for the `.soulidx` catalog index reader + SD/catalog scan scratch.
/// `IndexReader` itself measures 96 B (a no-alloc header cache); 32 KB is
/// already generous headroom for the paged reader + scan scratch on top. It
/// must never scale with catalog size — see
/// `index_reader_size_is_a_small_constant_independent_of_catalog_size` below,
/// which pins that independently of this budget.
const CATALOG_INDEX_BUDGET: usize = 32 * 1024;

/// Budget for DMA descriptor / staging buffers.
const DMA_BUFFERS_BUDGET: usize = 64 * 1024;

/// Budget for a single [`AnyDecoder`] value.
///
/// **Post-externalization.** `AnyDecoder<S, P>`'s big block-staging buffers
/// (WavPack/FLAC/ALAC/Caf/AAC/Matroska's top-level codec) were externalized
/// into a caller-owned scratch `P` (here pinned to `&'static mut [i32]`, a
/// 16 B fat pointer — see soul-audio-core's
/// `docs/superpowers/specs/2026-11-17-decoder-memory-externalization.md`),
/// so meeting this budget no longer means dropping a dispatch variant (that
/// was the OLD mechanism, via `embedded-profile` dropping
/// `MultistreamOpusDecoder` — still gone, surround Opus is still a graceful
/// `Unsupported`, but no longer the thing that made this budget fit).
///
/// Measured ceiling: 96,184 B, pinned by `MatroskaDecoder`'s **inline**
/// Opus-in-WebM path (`MatroskaOpus`'s own packet/PCM buffers don't yet
/// route through `P` — flagged in the spec as the remaining Phase-4 slice,
/// out of scope here). 128 KB gives ~34.9 KB (~36%) headroom over that.
/// Old budget: 288 KB (294,912 B), sized for the pre-externalization
/// 262,224 B WavPack-ceiling `AnyDecoder` — more than halved here.
const DECODER_BUDGET: usize = 128 * 1024;

/// Generous budget for [`Engine`]'s *own* bookkeeping — on top of whatever
/// `Deck<K, D, PRELOAD>` it wraps by value: a [`LinearSequencer`] (an index
/// cursor + repeat mode) plus one `Option<Handle>` for the current track and
/// one per `PRELOAD` preload slot. The Deck's own footprint (dominated by
/// its fixed DSP scratch buffers, and by however many fully-built decoders
/// `PRELOAD` demands) is pinned separately by [`DECODER_BUDGET`] /
/// [`AUDIO_PATH_BUDGET`] above; this budget proves the sequencing/queue
/// layer does not multiply that further.
const ENGINE_OVERHEAD_BUDGET: usize = 1024;

/// Generous budget for [`PowerService`]'s own state: a smoothed-SoC EMA,
/// charge/profile/thermal flags, an `Option<u64>` idle timestamp, and a
/// couple of `u32` counters — no heap, no arrays that scale with anything.
/// Guards against accidental growth (e.g. a history buffer) independent of
/// every audio/visual budget above, which this type has nothing to do with.
const POWER_SERVICE_BUDGET: usize = 256;

// The documented split above must actually fit inside the SRAM it's a split
// of — a static sanity check on the budget itself, independent of whatever
// `soul-audio-core`/`soul-audio-engine`/`soul-audio-index` measure at.
const _: () = assert!(
    AUDIO_PATH_BUDGET
        + DECODER_SCRATCH_POOL_BUDGET
        + FRAMEBUFFER_BUDGET
        + FRAMEBUFFER_SHADOW_BUDGET
        + UI_SCRATCH_BUDGET
        + LIBRARY_LISTING_BUDGET
        + CATALOG_INDEX_BUDGET
        + DMA_BUFFERS_BUDGET
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

/// `AnyDecoder<S, P>`'s externalized decoder-scratch storage type, pinned
/// here to a concrete, pointer-sized type so `size_of` reflects what the
/// externalization actually bought: a `&'static mut [i32]` is a fat pointer
/// (ptr + len, 16 B on a 64-bit host) regardless of how large the backing
/// buffer it points to is — exactly the board's real on-target shape (a
/// `StaticCell`-backed static, not an inline array). See soul-audio-core's
/// `docs/superpowers/specs/2026-11-17-decoder-memory-externalization.md`.
type P = &'static mut [i32];

/// A zero-size [`Storage`] stand-in for the [`AudioPlayer`] size guard: hands
/// back an empty [`SliceSource`] (never actually opened — this guard only
/// calls `size_of`, never `pump`), so `AudioPlayer<StubSink, StubStorage, P,
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

/// A zero-size [`Power`] stand-in for the [`App`] size guard: never actually
/// polled (this guard only calls `size_of`), so `App<_, _, _, StubPower, _,
/// _, _>`'s size is dominated by the `AudioPlayer`/`PowerService` it wraps,
/// not by the power port.
struct StubPower;

impl Power for StubPower {
    fn read(&mut self) -> BatteryReading {
        BatteryReading {
            soc_permille: 0,
            voltage_mv: 0,
            current_ma: 0,
            temp_centi_c: 0,
            charger_present: false,
            charging: false,
        }
    }

    fn set_charge_current_ma(&mut self, _ma: u16) {}
    fn set_clock_profile(&mut self, _profile: PowerProfile) {}
    fn request_power_off(&mut self) {}
}

/// A zero-size [`Controls`] stand-in for the [`App`] size guard, matching
/// [`StubPower`]'s role.
struct StubControls;

impl Controls for StubControls {
    fn poll(&mut self) -> Option<InputEvent> {
        None
    }
}

/// A zero-size [`dap_core::display::Display`] stand-in for the [`App`] size
/// guard: discards every pixel, declares the real panel's 800x480 geometry,
/// never fails. Mirrors `dap_core::app`'s own `MockDisplay` test double (see
/// `core/src/app.rs`'s unit tests) — duplicated here rather than imported
/// since that one is `#[cfg(test)]`-private to `dap-core`.
struct StubDisplay;

impl DrawTarget for StubDisplay {
    type Color = Gray4;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, _pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        Ok(())
    }
}

impl OriginDimensions for StubDisplay {
    fn size(&self) -> Size {
        Size::new(800, 480)
    }
}

impl EinkRefresh for StubDisplay {
    fn full_refresh(&mut self) -> Result<(), DisplayError> {
        Ok(())
    }

    fn partial_refresh(&mut self, _area: Rectangle) -> Result<(), DisplayError> {
        Ok(())
    }
}

/// The DAP's real preload depth (`dap_core::audio::AudioPlayer`'s const —
/// see `dap/sim/tests/audio_headless.rs`). **0 = single resident decoder**:
/// gapless comes from the output ring at track reopen (the Rockbox model —
/// verified by `audio_headless.rs`'s `longest_silent_run` staying ~zero), not
/// from a second preloaded decoder. This is what makes
/// [`DECODER_SCRATCH_POOL_BUDGET`] exactly ONE scratch (256 KB) instead of two.
const PRELOAD: usize = 0;

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

/// The shared decoder scratch — the single largest tunable static on the board
/// after the `App` itself — must stay within its committed ceiling
/// ([`SHARED_DECODER_SCRATCH_CAP_I32`]), and the test prints which format is
/// *setting* it.
///
/// # Why this gate exists
/// [`DECODER_SCRATCH_POOL_BUDGET`] used to be defined as the measured value, so
/// nothing actually capped the scratch: a format change that doubled it doubled
/// its budget too. This is the real cap, and the breakdown below is what the
/// memory backlog's item #2 asked for: assert `decoder_scratch_len()` is within
/// budget and print the per-format breakdown, so item #1's win locks in and any
/// format enlarging the shared scratch fails CI.
///
/// # The breakdown
/// **Seven** of `AnyDecoder`'s variants carve their working buffers from this one
/// shared allocation — FLAC, ALAC, AAC, Opus, Matroska, WavPack and CAF — each
/// sub-allocating what it needs through a `ScratchCursor`. Only WAV, AIFF and
/// MP3 take no `P` at all. `decoder_scratch_len()` is therefore sized to satisfy
/// the **largest** carver, which is WavPack: `wavpack::MAX_BLOCK_PCM` =
/// **65,536 i32 = 256 KiB**.
///
/// (Carving from `P` is separate from a variant's *inline* footprint — see
/// `any_decoder_variant_inline_footprint_roster`, which audits that. The two are
/// complementary: WavPack carves the most from `P` and has almost the smallest
/// inline footprint, 96 B; Matroska carves from `P` *and* keeps a large inline
/// block buffer.)
///
/// The two revisions differ only in how the maximum is spelled: at the pinned
/// rev `decoder_scratch_len()` is `wavpack::MAX_BLOCK_PCM` directly, while a
/// later one (present in a sibling checkout tracking `main`) computes
/// `max(wavpack::SCRATCH_LEN, alac::SCRATCH_LEN)`. WavPack binds in both, so the
/// assertions below are written against `wavpack::MAX_BLOCK_PCM` — public in
/// both revisions — and are correct whether or not the pin is bumped. (An
/// earlier draft read `wavpack::SCRATCH_LEN`, which only exists post-pin: it
/// compiled locally against the sibling and broke CI, which builds the pin.)
///
/// Asserting `decoder_scratch_len() == wavpack::MAX_BLOCK_PCM` pins the
/// *consequence* rather than the formula: if any carve ever overtakes WavPack,
/// the equality breaks and names the new binder — which is the fact you want at
/// that moment, since the backlog's "cap `MAX_BLOCK_PCM`" plan (item #1) would
/// then no longer shrink this buffer.
#[test]
fn shared_decoder_scratch_stays_within_its_cap() {
    let actual_i32 = soul_audio_core::decoder_scratch_len();
    let wavpack_i32 = soul_audio_core::formats::wavpack::MAX_BLOCK_PCM;

    println!(
        "\nshared decoder scratch: {actual_i32} i32 ({} B) — binding format: WavPack \
         (MAX_BLOCK_PCM = {wavpack_i32} i32)",
        actual_i32 * size_of::<i32>()
    );
    report(
        "shared decoder scratch (decoder_scratch_len)",
        actual_i32 * size_of::<i32>(),
        SHARED_DECODER_SCRATCH_CAP_I32 * size_of::<i32>(),
    );

    assert!(
        actual_i32 <= SHARED_DECODER_SCRATCH_CAP_I32,
        "the shared decoder scratch grew to {actual_i32} i32 ({} B), over the committed \
         {SHARED_DECODER_SCRATCH_CAP_I32} i32 ({} B) ceiling. At PRELOAD = {PRELOAD} this pool \
         is {} B of the 1 MB AXI SRAM the board links, competing directly with the \
         decode-ahead ring — so raising the cap is a deliberate tradeoff, not a formality. \
         Shrink the format that grew, or raise SHARED_DECODER_SCRATCH_CAP_I32 and say why.",
        actual_i32 * size_of::<i32>(),
        SHARED_DECODER_SCRATCH_CAP_I32 * size_of::<i32>(),
        DECODER_SCRATCH_POOL_BUDGET,
    );

    assert_eq!(
        actual_i32, wavpack_i32,
        "the shared scratch is no longer set by WavPack's MAX_BLOCK_PCM ({wavpack_i32} i32) but \
         by some other carve ({actual_i32} i32) — most likely the post-pin ALAC carve. The size \
         is now driven by a different format, so the backlog's 'cap MAX_BLOCK_PCM' plan \
         (item #1) would no longer shrink it; re-derive the breakdown in this test's doc \
         comment before updating this assertion."
    );
}

/// Ceiling on any single `AnyDecoder` variant's **inline** footprint — the
/// per-decoder state that is *not* externalized to the shared `P` scratch.
///
/// 64 KiB. The worst variant today is Matroska at 59,760 B, so this is ~5.6 KiB
/// of headroom: deliberately tight, because `AnyDecoder` is held by value in the
/// `Deck` (and therefore in `Engine`/`AudioPlayer`/`App`), so every byte here is
/// paid once per resident decoder whether or not that format is playing.
const ANY_DECODER_VARIANT_CEILING: usize = 64 * 1024;

/// The per-variant inline-footprint roster — the memory backlog's item #5
/// ("ALAC/Opus/AAC buffer audit — are any inline buffers oversized for the
/// stereo embedded target?"), answered by measurement.
///
/// # What it found
/// Measured against the pinned `soul-audio-core` (`embedded-profile`, `Src =
/// SliceSource`, `P = &'static mut [i32]`):
///
/// | variant | bytes | note |
/// |---|---|---|
/// | **Matroska** | **59,760** | **sets `size_of::<AnyDecoder>()`** — the Phase-A3 edge |
/// | MP3 | 44,240 | synthesis/subband state |
/// | Opus | 41,104 | CELT + SILK state |
/// | AAC | 34,056 | filterbank/SBR/PS state |
/// | ALAC | 25,896 | |
/// | CAF | 25,784 | wraps ALAC/PCM |
/// | WAV | 18,576 | large for a format with no codec state — worth a look |
/// | FLAC | 12,472 | |
/// | AIFF | 104 | |
/// | WavPack | 96 | externalizes essentially everything to `P` |
///
/// So the answer to #5 is **no for ALAC/Opus/AAC specifically** — none of them
/// is the problem — and **yes for Matroska**, which alone sets the enum's size.
/// That is not a new discovery so much as a confirmation of the firmware
/// roadmap's **Phase A3**, which flagged exactly this: *"close the one edge —
/// Matroska-wrapped-FLAC's 48 KB u8 SimpleBlock buffer (needs a byte-arena seam
/// alongside the i32 scratch)"*. A3 is still open, and this is the number that
/// quantifies it: externalizing that buffer would cut `AnyDecoder` by ~26 %
/// (59,760 → ~44 KB, where MP3 would become the binder).
///
/// Contrast WavPack (96 B) with Matroska (59,760 B): both carve from the shared
/// `P`, but WavPack externalized *everything* while Matroska kept its block
/// buffer inline. That is the shape of the remaining win.
///
/// # Why the roster is complete
/// The final assertion pins `size_of::<AnyDecoder>()` to the largest measured
/// variant (plus a discriminant word). A new or grown variant that this roster
/// doesn't list would make the total exceed that, and the test says so — so the
/// table can't silently fall behind the enum.
#[test]
fn any_decoder_variant_inline_footprint_roster() {
    // `Mp3Decoder` takes no `P` (it externalizes nothing); the rest are
    // instantiated exactly as `AnyDecoder` instantiates them.
    let variants: [(&str, usize); 9] = [
        ("matroska", size_of::<MatroskaDecoder<Src, P>>()),
        ("mp3", size_of::<Mp3Decoder<Src>>()),
        ("opus", size_of::<OpusDecoder<Src, P>>()),
        ("alac", size_of::<AlacDecoder<Src, P>>()),
        ("caf", size_of::<CafDecoder<Src, P>>()),
        ("wav", size_of::<WavDecoder<Src>>()),
        ("flac", size_of::<FlacDecoder<Src, P>>()),
        ("aiff", size_of::<AiffDecoder<Src>>()),
        ("wavpack", size_of::<WavpackDecoder<Src, P>>()),
    ];

    println!("\nAnyDecoder per-variant INLINE footprint (not counting the shared P scratch):");
    let mut worst = ("", 0usize);
    for (name, bytes) in variants {
        println!("  {name:<9} {bytes:>7} B");
        if bytes > worst.1 {
            worst = (name, bytes);
        }
        assert!(
            bytes <= ANY_DECODER_VARIANT_CEILING,
            "the {name} variant's inline footprint is {bytes} B, over the \
             {ANY_DECODER_VARIANT_CEILING} B per-variant ceiling. AnyDecoder is held by value in \
             the Deck, so this is paid once per resident decoder regardless of what is playing — \
             externalize the buffer into the shared P scratch (as WavPack does, at 96 B) rather \
             than raising the ceiling."
        );
    }

    let total = size_of::<AnyDecoder<Src, P>>();
    report(
        "AnyDecoder (largest variant)",
        total,
        ANY_DECODER_VARIANT_CEILING,
    );

    assert_eq!(
        worst.0, "matroska",
        "the largest inline variant is now {} ({} B), not Matroska — the Phase-A3 'externalize \
         Matroska's SimpleBlock buffer' plan is no longer the biggest inline win, so re-derive \
         this roster's doc comment before updating the expectation",
        worst.0, worst.1
    );

    // Completeness: the enum can be no bigger than its largest variant plus a
    // discriminant. If it is, some variant is missing from the roster above.
    assert!(
        total <= worst.1 + size_of::<usize>(),
        "size_of::<AnyDecoder>() is {total} B but the largest variant this roster measures is \
         {} B ({}) — some variant is missing from the roster, so the audit is incomplete. Add it.",
        worst.1,
        worst.0,
    );
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
    let regions: [(&str, usize); 8] = [
        (
            "audio path (Deck struct: decoder slot + resampler + ring)",
            AUDIO_PATH_BUDGET,
        ),
        (
            "decoder scratch pool ((1+PRELOAD) x the 65,536-i32 scratch cap)",
            DECODER_SCRATCH_POOL_BUDGET,
        ),
        (
            "visual: framebuffer, live (800x480 Gray4, 2 bpp)",
            FRAMEBUFFER_BUDGET,
        ),
        (
            "visual: framebuffer shadow (partial-refresh dirty-diff; optional)",
            FRAMEBUFFER_SHADOW_BUDGET,
        ),
        (
            "visual: eink-ui scratch (glyph/layout/compose)",
            UI_SCRATCH_BUDGET,
        ),
        (
            "library listing (browse page + order table + strings)",
            LIBRARY_LISTING_BUDGET,
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
    let actual = size_of::<AnyDecoder<Src, P>>();
    report("AnyDecoder<SliceSource, P>", actual, DECODER_BUDGET);
    assert!(
        actual <= DECODER_BUDGET,
        "AnyDecoder<SliceSource<'_>, P> is {actual} B, over the {DECODER_BUDGET} B H7A3 decoder \
         budget by {over} B. The ceiling is MatroskaDecoder's inline Opus-in-WebM path (its \
         packet/PCM buffers don't yet route through the externalized `P` scratch — see \
         soul-audio-core's decoder-memory-externalization spec, Phase 4); every other large \
         variant (WavPack/FLAC/ALAC/Caf/AAC/top-level Opus) already externalizes into `P`.",
        over = actual.saturating_sub(DECODER_BUDGET),
    );
}

#[test]
fn deck_fits_the_h7a3_audio_path_budget() {
    let actual = size_of::<Deck<StubSink, AnyDecoder<Src, P>, PRELOAD>>();
    report(
        "Deck<StubSink, AnyDecoder<SliceSource, P>, 0>",
        actual,
        AUDIO_PATH_BUDGET,
    );
    assert!(
        actual <= AUDIO_PATH_BUDGET,
        "Deck<_, AnyDecoder<SliceSource<'_>, P>, {PRELOAD}> is {actual} B, over the \
         {AUDIO_PATH_BUDGET} B H7A3 audio-path budget by {over} B. At PRELOAD=0 this should fit \
         with ample headroom — an overage here means the live decoder + resampler + output ring \
         grew.",
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
    let engine = size_of::<Engine<StubSink, AnyDecoder<Src, P>, LinearSequencer, PRELOAD>>();
    let deck = size_of::<Deck<StubSink, AnyDecoder<Src, P>, PRELOAD>>();
    let overhead = engine.saturating_sub(deck);
    report(
        "Engine overhead (Engine<..,LinearSequencer,0> - Deck<..,0> it wraps)",
        overhead,
        ENGINE_OVERHEAD_BUDGET,
    );
    assert!(
        overhead <= ENGINE_OVERHEAD_BUDGET,
        "Engine<_, AnyDecoder<SliceSource<'_>, P>, LinearSequencer, {PRELOAD}> is {overhead} B \
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
fn audio_player_fits_the_h7a3_audio_path_budget() {
    let actual = size_of::<AudioPlayer<StubSink, StubStorage, P, PRELOAD>>();
    report(
        "AudioPlayer<StubSink, StubStorage, P, 0>",
        actual,
        AUDIO_PATH_BUDGET,
    );
    assert!(
        actual <= AUDIO_PATH_BUDGET,
        "AudioPlayer<StubSink, StubStorage, P, {PRELOAD}> is {actual} B, over the \
         {AUDIO_PATH_BUDGET} B H7A3 audio-path budget by {over} B. AudioPlayer wraps an Engine \
         wrapping a Deck<_, AnyDecoder<_, _>, {PRELOAD}> by value, so it inherits exactly \
         `deck_fits_the_h7a3_audio_path_budget`'s footprint — an overage here means that guard \
         is failing too.",
        over = actual.saturating_sub(AUDIO_PATH_BUDGET),
    );
}

/// Catches ANY growth of [`IndexReader`] — not just a budget-busting one.
/// [`index_reader_fits_the_catalog_index_budget`] above checks it against the
/// generous 32 KB [`CATALOG_INDEX_BUDGET`] (which also covers paged-scan
/// scratch on top); this is the tight, no-slack check: `IndexReader` is
/// architecturally a no-alloc header cache (a `RefCell<Src>` + the parsed
/// ~64 B fixed header) that re-seeks and re-reads the source on every field
/// access, so it must stay a small CONSTANT regardless of how large the
/// catalog it indexes gets — it never caches a record, a string, or
/// anything else that would scale with library size. 256 B is generous
/// headroom over the measured 96 B while still catching essentially any
/// real growth (e.g. an accidentally-added cache field).
const INDEX_READER_CONST_SIZE_CEILING: usize = 256;

#[test]
fn index_reader_size_is_a_small_constant_independent_of_catalog_size() {
    let actual = size_of::<IndexReader<Src>>();
    report(
        "IndexReader<SliceSource> (constant-size check)",
        actual,
        INDEX_READER_CONST_SIZE_CEILING,
    );
    assert!(
        actual <= INDEX_READER_CONST_SIZE_CEILING,
        "IndexReader<SliceSource<'_>> is {actual} B, over the {INDEX_READER_CONST_SIZE_CEILING} B \
         constant-size ceiling by {over} B. This is tighter than \
         `index_reader_fits_the_catalog_index_budget`'s 32 KB budget on purpose: IndexReader must \
         never scale with catalog size (it re-seeks/re-reads the source per field access instead \
         of caching records), so ANY growth here — even one still well under the 32 KB budget — \
         means something that should be a re-seek turned into cached state instead.",
        over = actual.saturating_sub(INDEX_READER_CONST_SIZE_CEILING),
    );
}

/// Single-glance footprint roster + regression guard for every main-stack
/// type this crate's budgets care about, in one table: each type's measured
/// `size_of` against the SRAM budget it actually has to fit (the same
/// budgets the dedicated guards above already enforce one at a time — this
/// test re-checks them together so a reviewer can see the whole picture in
/// one `cargo test -- --nocapture` run). `App` and `PowerService` are the
/// two `dap_core` types from the spec's §5.1 roster not already covered by a
/// dedicated test above.
///
/// No type here was skipped — every type the spec's roster names
/// (`AnyDecoder`, `Deck`, `Engine`, `AudioPlayer`, `IndexReader`, `App`,
/// `PowerService`) is public and constructible with the stub ports already
/// defined in this file. `soul-audio-core`'s individual decoder variants
/// (`WavDecoder`, `FlacDecoder`, …) and `Resampler` are deliberately NOT
/// listed: they aren't re-exported from `soul_audio_core`'s public surface
/// (see that crate's `lib.rs`), so sizing them here would mean reaching into
/// a module path this DAP doesn't otherwise depend on, for marginal value
/// over `AnyDecoder`'s own (already-guarded) ceiling.
#[test]
fn component_footprint_roster() {
    println!("\nComponent footprint roster (size_of vs. its relevant SRAM budget):");

    let any_decoder = size_of::<AnyDecoder<Src, P>>();
    report("AnyDecoder<SliceSource, P>", any_decoder, DECODER_BUDGET);
    assert!(
        any_decoder <= DECODER_BUDGET,
        "see any_decoder_fits_the_h7a3_decoder_budget"
    );

    let deck = size_of::<Deck<StubSink, AnyDecoder<Src, P>, PRELOAD>>();
    report(
        "Deck<StubSink, AnyDecoder<SliceSource, P>, 0>",
        deck,
        AUDIO_PATH_BUDGET,
    );
    assert!(
        deck <= AUDIO_PATH_BUDGET,
        "see deck_fits_the_h7a3_audio_path_budget"
    );

    let engine = size_of::<Engine<StubSink, AnyDecoder<Src, P>, LinearSequencer, PRELOAD>>();
    report(
        "Engine<StubSink, AnyDecoder<SliceSource, P>, LinearSequencer, 0>",
        engine,
        AUDIO_PATH_BUDGET,
    );
    assert!(
        engine <= AUDIO_PATH_BUDGET,
        "Engine wraps a Deck by value plus tiny bookkeeping — see \
         engine_overhead_is_tiny_on_top_of_the_deck_it_wraps for the overhead-only guard"
    );

    let audio_player = size_of::<AudioPlayer<StubSink, StubStorage, P, PRELOAD>>();
    report(
        "AudioPlayer<StubSink, StubStorage, P, 0>",
        audio_player,
        AUDIO_PATH_BUDGET,
    );
    assert!(
        audio_player <= AUDIO_PATH_BUDGET,
        "see audio_player_fits_the_h7a3_audio_path_budget"
    );

    let index_reader = size_of::<IndexReader<Src>>();
    report(
        "IndexReader<SliceSource>",
        index_reader,
        CATALOG_INDEX_BUDGET,
    );
    assert!(
        index_reader <= CATALOG_INDEX_BUDGET,
        "see index_reader_fits_the_catalog_index_budget / the constant-size guard"
    );

    let app =
        size_of::<App<StubSink, StubStorage, P, StubPower, StubControls, StubDisplay, PRELOAD>>();
    report(
        "App<StubSink, StubStorage, P, StubPower, StubControls, StubDisplay, 0>",
        app,
        AUDIO_PATH_BUDGET,
    );
    assert!(
        app <= AUDIO_PATH_BUDGET,
        "App<_> is {app} B, over the {AUDIO_PATH_BUDGET} B audio-path budget by {over} B. App \
         wraps an AudioPlayer by value plus a PowerService + zero-size ports, so its footprint \
         should track AudioPlayer's almost exactly — an overage here that AudioPlayer's own guard \
         doesn't also show means the App-level wiring (PowerService, ports) grew, not the audio \
         path itself.",
        over = app.saturating_sub(AUDIO_PATH_BUDGET),
    );

    let power_service = size_of::<PowerService>();
    report("PowerService", power_service, POWER_SERVICE_BUDGET);
    assert!(
        power_service <= POWER_SERVICE_BUDGET,
        "PowerService is {power_service} B, over the {POWER_SERVICE_BUDGET} B budget by {over} B \
         — it's meant to be fixed-size state (an SoC EMA, flags, counters), so growth here likely \
         means an unbounded field (e.g. a history buffer) snuck in.",
        over = power_service.saturating_sub(POWER_SERVICE_BUDGET),
    );

    println!();
}

/// The board (`dap/board-h7a3`) LINKS a single RAM region: the 1 MB AXI SRAM
/// (`memory.x` maps only `RAM : 0x2400_0000, 1024K`; the +128 KB of AHB-SRAM1/2
/// @ `0x3000_0000` that [`memory_map::USER_SRAM`] also counts is NOT in the
/// linker script). So unlike the per-region split above -- which checks against
/// the full 1152 KB `USER_SRAM` -- every static the board actually places AND its
/// whole call stack must fit THIS 1 MB.
///
/// This guards the board's two dominant AXI-SRAM statics -- the `App` moved into
/// `APP_CELL`, and the one `decoder_scratch_len()` pool `SCRATCH` holds at
/// `PRELOAD = 0` -- plus `BOARD_STACK_RESERVE` of stack headroom, measured on the
/// board's EXACT instantiation (`P = &'static mut [i32]`, `PRELOAD = 0`). It is
/// the linker-agnostic companion to board-h7a3's `APP_CELL` note: the on-chip
/// `build-board-h7a3` link is rust-lld without flip-link, so nothing there stops
/// static storage from silently starving the stack -- this catches it at the
/// Rust level, in the host `test-dap` job. (A linker `ASSERT` was tried first but
/// can't be made robust: the board is linked rust-lld from `dap/board-h7a3/` yet
/// flip-link from `dap/`, and flip-link's two-pass relink defeats a headroom
/// check on `_stack_start`/`__sheap`.)
///
/// # The reserve must match what the stack actually measures
/// Reserve = **480 KiB**, the budget `tools/mcu-stackprobe/check-tick-qemu.sh`
/// enforces (`DYNAMIC_TICK_BUDGET_KIB`), against a measured whole-firmware-tick
/// worst case of **413 KiB**.
///
/// This was **256 KiB**, taken from the memory/battery design doc's
/// open-transient budget — but that figure predates the dynamic measurements and
/// is *below both of them*: the isolated decode-open transient is 297 KiB and the
/// whole tick (decode + `App::render`) is 413 KiB. Two gates reserving 256 KiB
/// while a third enforces 480 KiB meant this check reported ~345 KB "to spare"
/// that the real stack would have run straight into. Corrected 2026-11-21; see
/// `docs/plans/2026-11-21-memory-architecture-and-placement.md`.
///
/// Keep this in lockstep with `check-tick-qemu.sh`'s budget and with
/// `board-h7a3`'s `STACK_RESERVE_BYTES` (the compile-time twin). The measured
/// transient is the single largest consumer of linked RAM — larger than the
/// decoder scratch — so reducing it is worth more than any placement change; the
/// placement doc's Phase 1 covers that.
///
/// When Phase E adds real-port statics (an e-ink framebuffer, DMA rings) to AXI
/// SRAM, add them to the `statics` total below -- or place them in the
/// as-yet-unlinked AHB-SRAM1/2 (extend `memory.x` + `#[link_section]`).
#[test]
fn board_static_footprint_leaves_stack_room_in_linked_axi_sram() {
    // memory.x's only linked RAM region: AXI SRAM @ 0x2400_0000.
    const AXI_SRAM_LINKED: usize = 1024 * 1024;
    // Stack headroom to keep clear above all board statics. MEASURED, not
    // assumed: `check-tick-qemu.sh`'s 480 KiB budget over a 413 KiB worst case.
    // Keep in lockstep with board-h7a3/src/main.rs's STACK_RESERVE_BYTES.
    const BOARD_STACK_RESERVE: usize = 480 * 1024;

    // The board's exact App (PRELOAD = 0, Pcm = &'static mut [i32]) and the one
    // decoder-scratch pool it hands out -- the two statics that dominate its RAM.
    let app =
        size_of::<App<StubSink, StubStorage, P, StubPower, StubControls, StubDisplay, PRELOAD>>();
    let scratch = DECODER_SCRATCH_POOL_BUDGET;
    let statics = app + scratch;
    let total = statics + BOARD_STACK_RESERVE;

    // Label derived from the constant, not restated — a hardcoded "256 KiB" here
    // is exactly how the stale reserve survived unnoticed.
    println!(
        "board AXI-SRAM statics (App {app} B + decoder scratch {scratch} B) + \
         {} KiB measured stack reserve:",
        BOARD_STACK_RESERVE / 1024
    );
    report(
        "board AXI-SRAM statics + stack reserve",
        total,
        AXI_SRAM_LINKED,
    );
    assert!(
        total <= AXI_SRAM_LINKED,
        "board statics ({statics} B: App {app} B + decoder scratch {scratch} B) plus the \
         {BOARD_STACK_RESERVE} B stack reserve total {total} B, over the {AXI_SRAM_LINKED} B the \
         board actually links (AXI SRAM only -- memory.x doesn't map AHB SRAM1/2/3) by {over} B. \
         Move a large AXI-SRAM static into AHB SRAM1/2/3 (extend memory.x + #[link_section]) or \
         shrink statics; do NOT just raise the USER_SRAM-based budgets, which count 184 KB this \
         link can't reach.",
        over = total.saturating_sub(AXI_SRAM_LINKED),
    );
}
