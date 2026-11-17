//! Proves `soul-audio-core`'s audio path (decode → resample-or-passthrough →
//! sink write) does **ZERO heap allocation** in steady state — the crate's
//! own contract ("Cross-platform no_std, no-alloc audio decode + resample +
//! playback core", `soul-audio-core/Cargo.toml`'s description) checked by an
//! actual running measurement, not just a `#![no_std]` type-check.
//!
//! # Method
//! A global allocator wraps `std::alloc::System`, counting every
//! `alloc`/`realloc` call while a process-wide `ARMED` flag is set. For each
//! of the WAV/FLAC/MP3 bench fixtures (`dap-sim/benches/decode_rtf.rs`'s
//! fixture set): open the file through `soul_audio_core::open_seekable` into
//! a `Deck` (passthrough-rate, so no resampler is engaged — isolating the
//! decoder's own allocation behaviour, which is this test's actual target),
//! run an un-armed WARMUP batch of pumps (letting any one-time lazy
//! construction inside `open`/`load` happen off the clock), then arm the
//! counter, run a tight STEADY-STATE batch of pumps, disarm, and assert
//! exactly zero allocations happened during the armed window.
//!
//! # Critical isolation — read before adding a second `#[test]` here
//! The global allocator is **process-wide**. Each `tests/*.rs` integration
//! test file compiles to its own binary (its own process, its own address
//! space), so this file's `ALLOC_COUNT`/`ARMED` statics never interact with
//! any other test file's — cross-FILE parallelism is fine. But `cargo test`
//! runs every `#[test]` fn **within one file** on parallel threads of the
//! SAME process by default, sharing these statics: a second test here could
//! arm/disarm concurrently with this one and corrupt the count (or get
//! blamed for allocations this test's own setup made). So this file carries
//! **exactly one** `#[test]` fn, looping over formats itself rather than
//! splitting into one `#[test]` per format. Do not add a second `#[test]` to
//! this file — add a new file instead.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use soul_audio_core::{
    open_seekable, Deck, Decoder, Quality, SampleFormat, Sink, SinkError, SliceSource, StreamInfo,
};

/// Count of `alloc`/`realloc` calls observed while [`ARMED`] was `true`,
/// since the last reset. `SeqCst` throughout: this isn't a hot path, and the
/// whole point is a count a human can trust without reasoning about
/// reordering.
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Whether [`CountingAllocator`] is currently counting. `false` during
/// setup/warmup (test-harness and one-time lazy-init allocation is
/// expected and irrelevant here), `true` only for the tight steady-state
/// window this test actually asserts on.
static ARMED: AtomicBool = AtomicBool::new(false);

/// Wraps [`System`], counting `alloc`/`realloc` while [`ARMED`]. `dealloc`
/// is never counted — a steady-state loop that allocated-then-immediately-
/// freed every pump would otherwise dodge detection on the alloc side only
/// to show up here; not counting dealloc is deliberate because a genuinely
/// zero-alloc loop also frees nothing, so this asymmetry never hides a real
/// violation — any armed-window alloc is caught the moment it happens.
struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if ARMED.load(Ordering::SeqCst) {
            ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        System.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        System.dealloc(ptr, layout);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        if ARMED.load(Ordering::SeqCst) {
            ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        }
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// A zero-alloc [`Sink`] test double: declares whatever stream shape it's
/// built with (set to the decoder's own native rate/channels below, so the
/// deck never builds a resampler — this test's target is the decoder's own
/// allocation behaviour, not the resampler's, which gets its own no-alloc
/// coverage implicitly via `audio_headless.rs`'s resampled-path run), and
/// just counts frames. No `Vec` (unlike `dap_sim::SimSink`, which records
/// the full PCM stream for playback assertions this test doesn't need).
struct CountingSink {
    info: StreamInfo,
    frames_written: u64,
}

impl CountingSink {
    fn new(sample_rate: u32, channels: u16) -> Self {
        Self {
            info: StreamInfo {
                sample_rate,
                channels,
                format: SampleFormat::Int { bits: 24 },
            },
            frames_written: 0,
        }
    }
}

impl Sink for CountingSink {
    fn stream_info(&self) -> StreamInfo {
        self.info
    }

    fn write(&mut self, frames: &[i32]) -> Result<(), SinkError> {
        self.frames_written += (frames.len() / self.info.channels.max(1) as usize) as u64;
        Ok(())
    }

    fn flush(&mut self) -> Result<(), SinkError> {
        Ok(())
    }
}

/// Gapless preload depth — matches the DAP's real `PRELOAD = 1`, though this
/// test never preloads a second track (one loaded track, pumped repeatedly,
/// is enough to exercise steady-state decode).
const PRELOAD: usize = 1;

/// Un-armed pumps before measurement starts, letting any one-time lazy
/// construction inside the decoder (format-specific table init, etc.)
/// happen off the clock. Comfortably more than any format's decoder needs
/// to reach steady state.
const WARMUP_PUMPS: usize = 16;

/// Armed, steady-state pumps actually measured. At `BLOCK = 256` output
/// frames/pump (`soul_audio_core::deck`'s internal constant) this is far
/// short of the ~4 s fixtures' full length at any of their native rates, so
/// every armed pump decodes real audio — none of this window runs past EOF
/// into the cheap `Progress::Idle` no-op path.
const STEADY_STATE_PUMPS: usize = 200;

/// Open `bytes` (a full container+codec file), build a passthrough-rate
/// `Deck` over it (sink rate/channels == decoder's own, so no resampler is
/// built — isolating the decoder), warm it up un-armed, then pump
/// [`STEADY_STATE_PUMPS`] armed blocks and assert zero allocations occurred
/// during that window.
fn assert_steady_state_decode_allocates_nothing(label: &str, bytes: &'static [u8]) {
    // The scratch allocation happens here, before WARMUP/ARMED -- see the
    // module doc's method: only the steady-state pump window is measured.
    let scratch = vec![0i32; soul_audio_core::decoder_scratch_len()];
    let decoder = open_seekable(SliceSource::new(bytes), scratch).expect("open fixture");
    let si = decoder.stream_info();
    let sink = CountingSink::new(si.sample_rate, si.channels);
    let mut deck: Deck<CountingSink, _, PRELOAD> = Deck::new(sink, Quality::Fast);
    deck.load(decoder)
        .expect("load decoder (passthrough rate, must not fail)");

    // WARMUP (unarmed): let anything one-time happen.
    for _ in 0..WARMUP_PUMPS {
        deck.pump().expect("warmup pump");
    }

    // ARM: the tight, measured window.
    ALLOC_COUNT.store(0, Ordering::SeqCst);
    ARMED.store(true, Ordering::SeqCst);
    for _ in 0..STEADY_STATE_PUMPS {
        deck.pump().expect("steady-state pump");
    }
    ARMED.store(false, Ordering::SeqCst);

    let allocs = ALLOC_COUNT.load(Ordering::SeqCst);
    println!("  {label:<6} {STEADY_STATE_PUMPS} armed pumps, {allocs} heap allocation(s) observed");
    assert_eq!(
        allocs, 0,
        "{label}: soul-audio-core's decode/pump path allocated {allocs} time(s) during a \
         {STEADY_STATE_PUMPS}-pump steady-state window — this breaks the no_std/no-alloc audio \
         path contract (soul-audio-core/Cargo.toml's own description). This is a real finding if \
         it fires, not a flaky test: find what allocated (a new Vec/Box/String in the hot decode \
         path) and remove it rather than raising the budget."
    );
}

/// Single `#[test]` (see module doc for why): runs the zero-alloc steady-
/// state check for each of the WAV/FLAC/MP3 bench fixtures
/// (`dap-sim/benches/decode_rtf.rs`'s "same ~4 s CC0 real-music clip,
/// encoded three ways" set), inside one big-stack worker thread. The large
/// stack mirrors `audio_headless.rs`/`decode_rtf.rs`: `AnyDecoder` is ~96 KB
/// (post decoder-memory-externalization; was ~262 KB) and moves through
/// several unoptimized (debug) call frames on its way
/// through `open_seekable`/`Deck::load` — a default thread stack isn't
/// generous enough to hold that reliably (see `tests/stack_highwater.rs` for
/// exactly how large that really needs to be).
#[test]
fn audio_path_steady_state_pump_allocates_zero_heap_bytes() {
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let wav: &'static [u8] =
                include_bytes!("../../../../soul-audio-core/testdata/realmusic.wav");
            let flac: &'static [u8] =
                include_bytes!("../../../../soul-audio-core/testdata/realmusic.flac");
            let mp3: &'static [u8] =
                include_bytes!("../../../../soul-audio-core/testdata/realmusic.mp3");

            println!("\nno-alloc steady-state decode/pump check:");
            assert_steady_state_decode_allocates_nothing("wav", wav);
            assert_steady_state_decode_allocates_nothing("flac", flac);
            assert_steady_state_decode_allocates_nothing("mp3", mp3);
        })
        .expect("spawn worker thread")
        .join()
        .expect("worker thread panicked");
}
