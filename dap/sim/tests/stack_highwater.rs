//! Measures (indirectly, via the only portable technique available — see
//! "Method" below) the real **peak stack** of the on-target call chain —
//! `AudioPlayer::pump` → `Engine::poll_open`/`provide` →
//! `soul_audio_core::open_seekable` → `Deck::load`/`Deck::pump` → the
//! decoder itself — and proves it fits a committed [`STACK_BUDGET`].
//!
//! # Why this matters (spec §3.1) — UPDATED post-externalization
//! `AnyDecoder` **was** ~262 KB (the inline WavPack `pcm: [i32; 65536]`
//! ceiling) and was returned **by value** from `open`/`open_seekable`, then
//! moved again into `Deck::load`'s `Track` — that was the original board
//! blocker this test exists to surface. soul-audio-core's
//! `docs/superpowers/specs/2026-11-17-decoder-memory-externalization.md`
//! fixed it: the big block-staging buffers (WavPack/FLAC/ALAC/Caf/AAC/the
//! top-level Opus variant) now live in a caller-owned scratch `P` (a 16 B
//! fat pointer the decoder owns and drops with it — `Box<[i32]>` on the
//! host, a `&'static mut [i32]` from a board `StaticCell` on-target), so
//! `AnyDecoder<S, P>` dropped from 262,224 B to **96,184 B** (now pinned by
//! `MatroskaDecoder`'s still-inline Opus-in-WebM path — see
//! `dap-arch-tests/tests/budget.rs`'s `DECODER_BUDGET`/`any_decoder_fits_the_h7a3_decoder_budget`
//! for the live measurement). This test re-measures what that ~2.7×
//! shrink actually bought the open-path TRANSIENT (as opposed to the
//! at-rest `size_of` the arch-tests budgets cover) — in an *unoptimized*
//! build (`dev`/`test` profile — what every `cargo test` run on this
//! workspace actually is; see `dap/Cargo.toml`'s
//! `profile.dev.package.soul-audio-core` / `profile.test.package.soul-audio-core`
//! `opt-level = 3` override, which pins the DECODER's own crate to -O3 even
//! here, but does **not** touch `dap-core`'s/`dap-sim`'s own call frames),
//! each move through the open chain is still a real, non-elided stack copy,
//! not just a bookkeeping abstraction cost.
//!
//! # Method — and its caveat
//! A thread stack overflow **aborts the whole process**; it is not a
//! catchable `panic!`. So this file does NOT bisect at test time (that would
//! mean deliberately crashing the test binary to find the edge, destroying
//! every other test's result in the same run). Instead:
//! 1. **Dev-time** (not part of this committed test): bisect by running the
//!    exact same call chain in a disposable example, in a fresh child
//!    process per candidate stack size (a crash only kills that child), at
//!    shrinking stack sizes until it overflows, to find the real ceiling.
//! 2. **Commit** a [`STACK_BUDGET`] safely ABOVE that measured ceiling, so
//!    the committed test always completes cleanly — proving "the real
//!    high-water mark is ≤ `STACK_BUDGET`" without ever running at the edge.
//!
//! ## Measured ceiling, RE-BISECTED post-externalization
//! Bisected on this change's host (Windows x86_64, `dev`/`test` unoptimized
//! profile — the same profile `cargo test` uses, via `examples/stackprobe.rs`'s
//! `local` mode, which is the exact `open_decode_pump_to_end` body below) with
//! the exact `AudioPlayer`-driven open→decode→pump loop below, one candidate
//! stack size per child process, 3 repeats per boundary value to rule out
//! run-to-run noise:
//!
//! | Stack size    | Result (WAV / FLAC / MP3) |
//! |---------------|----------------------------|
//! | 2,752,000 B   | overflow, all 3 formats, 3/3 runs |
//! | 2,754,000 B   | OK, all 3 formats, 3/3 runs |
//!
//! (Previously: 6,000,000 B overflow / 6,050,000 B OK, pre-externalization —
//! ~2.75 MB is a **~2.2× drop** from the old ~6.05 MB ceiling.) All three
//! formats again hit essentially the **same** ceiling — confirming the
//! dominant cost is still the repeated `AnyDecoder`/`Track` stack-to-stack
//! moves through `open_seekable`/`Deck::load`'s unoptimized call frames, now
//! moving a 96,184 B enum instead of a 262,224 B one (Rust enums are sized to
//! their largest variant regardless of which one is actually live, so the
//! WAV/FLAC/MP3 fixtures already move the full Matroska-sized ceiling even
//! though none of them decode Matroska — see the note on the Matroska
//! worst-case below).
//!
//! [`STACK_BUDGET`] below is set to **3.5 MiB** (3,670,016 B) — about 33 %
//! above the measured ~2.754 MB ceiling (the same ~33 % margin convention as
//! the old 8 MiB/~6.05 MB budget), independently reconfirmed passing
//! multiple times at this exact value during bisection. Down from the old
//! **8 MiB**.
//!
//! ## The on-target implication (this is the actual point of this test)
//! A *host* ceiling (x86_64, unoptimized, a totally different calling
//! convention/register set than Cortex-M7) is not the Cortex-M7's real
//! number — but it is decisive on one question regardless of ISA, even after
//! the externalization: **a multi-megabyte transient categorically does not
//! fit the H7A3's 128 KB DTCM.** Implication for `board-h7a3` (spec §3.1):
//! - The **main stack must live in AXI SRAM**, not DTCM — DTCM is fast but
//!   simply too small for this call chain's transients, in ANY build
//!   profile.
//! - And/or the decoder should be constructed **directly into a
//!   `StaticCell`** rather than returned by value and moved — see
//!   `tests/static_placement.rs`, which also documents why `StaticCell`
//!   alone does not eliminate the construction-time stack cost this test
//!   measures (it only relocates the steady-state, at-rest storage).
//! - A *release* build is dramatically better post-externalization, but still
//!   does NOT reach the ≤256 KB target on its own — measured, not assumed.
//!   RE-BISECTING `examples/stackprobe.rs` (`--release`) post-externalization
//!   gives a **~920 KB** ceiling with the player on-stack (down from ~2.1 MB)
//!   and **~590 KB with the player off-stack** (boxed ≈ the board's
//!   `StaticCell` — the board-representative figure; down from **~1.44 MB**).
//!   That boxed figure is the headline result: a **~60 % drop** (1.44 MB →
//!   ~0.59 MB), but still ~2.3× OVER the spec's ≤256 KB open-transient target
//!   — the win is real, just not sufficient on its own. Root cause
//!   (unchanged in kind, smaller in magnitude): the open chain still moves
//!   `AnyDecoder` by value ~5× through `open_seekable` → `provide` → `load`
//!   → `build_track` (5 × 96,184 B ≈ 481 KB, plus `Deck`'s own fixed
//!   resampler/ring overhead on top ≈ accounts for the ~590 KB measured) —
//!   the optimizer still does not elide these moves. Lever C from the
//!   externalization spec (collapsing the move count itself, not shrinking
//!   what's moved) is the next lever if ≤256 KB is still required; out of
//!   scope for this change. The H7A3's AXI SRAM is 1.18 MB, so ~590 KB now
//!   fits comfortably inside it (unlike the old ~1.44 MB, which didn't) —
//!   this is no longer a hard board-blocker, just a budget that still wants
//!   tightening.
//! - **The Matroska-Opus caveat.** This probe only decodes WAV/FLAC/MP3, and
//!   soul-audio-core's `embedded-profile` `AnyDecoder` worst-case variant is
//!   actually `MatroskaDecoder`'s inline Opus-in-WebM path (~96 KB — see
//!   soul-audio-core's externalization spec §8's "Finding" section) — NOT one
//!   of the three formats measured here. This sounds like a gap, but isn't a
//!   hidden worse number: a Rust enum's `size_of` is fixed at its largest
//!   variant's size regardless of which variant is constructed, so every
//!   measurement above (WAV/FLAC/MP3 alike) already moves the FULL
//!   96,184 B-sized `AnyDecoder` enum, Matroska-ceiling included — opening a
//!   real `.webm`/Opus file would not move anything bigger. The real
//!   watch-item is the other direction: `MatroskaOpus`'s packet/PCM buffers
//!   are flagged in the spec as Phase 4 (not done here) — externalizing them
//!   would shrink `AnyDecoder` (and so this transient) further, below
//!   today's 96,184 B ceiling toward Mp3's 44,240 B.
//!   The `size_of` budget guards (`dap-arch-tests`) measure only the at-rest
//!   footprint (now ~332 KB, which fits comfortably — see `budget.rs`'s
//!   `AUDIO_PATH_BUDGET`) and by construction cannot see this transient —
//!   this test is what surfaces it.

use dap_core::audio::AudioPlayer;
use dap_core::ports::Storage;
use dap_sim::sac_scratch;
use soul_audio_core::{Progress, Quality, SampleFormat, Sink, SinkError, SliceSource, StreamInfo};

/// Committed stack-region size this test proves the real open→decode→pump
/// high-water mark fits inside — see the module doc's "Measured ceiling,
/// RE-BISECTED post-externalization" section for how this number was chosen
/// (ceiling ~2.754 MB + ~33% margin). Down from the pre-externalization
/// 8 MiB (ceiling was then ~6.05 MB) — see the module doc for the full
/// before/after.
///
/// `println!`ed below so a test run's own output restates it, per the task
/// brief: this number still exceeds the H7A3's 128 KB DTCM by ~28x, so
/// on-target the main stack must live in AXI SRAM (not DTCM), and/or the
/// decoder must be built directly into a static rather than returned by
/// value — see the module doc.
const STACK_BUDGET: usize = 3 * 1024 * 1024 + 512 * 1024; // 3.5 MiB

/// A sink that discards every frame — this test cares about stack depth,
/// not output, so no recording (keeps the call chain as close to the real
/// on-target `write` cost as a host std allocation-backed sink reasonably
/// can — no `Vec` growth to muddy a stack measurement with heap activity).
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

/// Hands back the same in-memory fixture bytes for the one-track queue —
/// the real `soul_audio_core::open_seekable` dispatch + `AnyDecoder`
/// construction is what this test measures the stack cost of, same as every
/// other fixture-backed test in this crate (`audio_headless.rs`).
struct FixtureStorage {
    bytes: &'static [u8],
}

impl Storage for FixtureStorage {
    type Source = SliceSource<'static>;
    type Error = ();

    fn open(&mut self, _track: usize) -> Result<Self::Source, Self::Error> {
        Ok(SliceSource::new(self.bytes))
    }
}

// 0 = single resident decoder; gapless via the output ring at track reopen
// (Rockbox model) — the board's SAI DMA ring must be deep enough to cover
// the reopen's wall-clock latency.
const PRELOAD: usize = 0;

/// Run a FULL open→decode→pump of `bytes` to completion (`Progress::Ended`),
/// on the CURRENT thread — the caller is responsible for running this inside
/// a [`STACK_BUDGET`]-sized worker thread.
fn open_decode_pump_to_end(bytes: &'static [u8]) {
    let sink = DiscardSink {
        info: StreamInfo {
            sample_rate: 48_000,
            channels: 2,
            format: SampleFormat::Int { bits: 24 },
        },
    };
    let storage = FixtureStorage { bytes };
    let mut player: AudioPlayer<DiscardSink, FixtureStorage, Box<[i32]>, PRELOAD> =
        AudioPlayer::new(sink, storage, Quality::Fast, sac_scratch);
    player.set_queue(1, 0);
    player.play();
    loop {
        match player.pump().expect("open+decode+pump") {
            Progress::Ended => break,
            Progress::Idle | Progress::Played | Progress::TrackChanged => {}
        }
    }
}

/// Run `open_decode_pump_to_end` on a dedicated [`STACK_BUDGET`]-sized
/// worker thread. If the real high-water mark exceeded `STACK_BUDGET`, this
/// would abort the whole test process (see module doc) rather than
/// returning an `Err` — so a clean `join()` return is itself the proof that
/// this run's peak stack usage was `<= STACK_BUDGET`.
fn run_within_budget(label: &str, bytes: &'static [u8]) {
    std::thread::Builder::new()
        .stack_size(STACK_BUDGET)
        .spawn(move || open_decode_pump_to_end(bytes))
        .expect("spawn worker thread")
        .join()
        .unwrap_or_else(|e| panic!("{label}: worker thread panicked: {e:?}"));
    println!("  {label:<6} completed a full open+decode+pump within {STACK_BUDGET} B of stack");
}

#[test]
fn open_decode_pump_fits_the_committed_stack_budget_wav() {
    println!(
        "\nSTACK_BUDGET = {STACK_BUDGET} B ({:.2} MiB) — see module doc for the measured ceiling \
         this covers, and why it means DTCM (128 KB) cannot host this call chain's stack on-target.",
        STACK_BUDGET as f64 / (1024.0 * 1024.0)
    );
    let wav: &'static [u8] = include_bytes!("../../../../soul-audio-core/testdata/realmusic.wav");
    run_within_budget("wav", wav);
}

#[test]
fn open_decode_pump_fits_the_committed_stack_budget_flac() {
    let flac: &'static [u8] = include_bytes!("../../../../soul-audio-core/testdata/realmusic.flac");
    run_within_budget("flac", flac);
}

#[test]
fn open_decode_pump_fits_the_committed_stack_budget_mp3() {
    let mp3: &'static [u8] = include_bytes!("../../../../soul-audio-core/testdata/realmusic.mp3");
    run_within_budget("mp3", mp3);
}
