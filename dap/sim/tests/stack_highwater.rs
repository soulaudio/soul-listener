//! Measures (indirectly, via the only portable technique available — see
//! "Method" below) the real **peak stack** of the on-target call chain —
//! `AudioPlayer::pump` → `Engine::poll_open`/`provide` →
//! `soul_audio_core::open_seekable` → `Deck::load`/`Deck::pump` → the
//! decoder itself — and proves it fits a committed [`STACK_BUDGET`].
//!
//! # Why this matters (spec §3.1)
//! `AnyDecoder` is ~262 KB and is returned **by value** from `open`/
//! `open_seekable`, then moved again into `Deck::load`'s `Track`. In an
//! *unoptimized* build (`dev`/`test` profile — what every `cargo test` run
//! on this workspace actually is; see `dap/Cargo.toml`'s
//! `profile.dev.package.soul-audio-core` / `profile.test.package.soul-audio-core`
//! `opt-level = 3` override, which pins the DECODER's own crate to -O3 even
//! here, but does **not** touch `dap-core`'s/`dap-sim`'s own call frames),
//! each of those moves is a real, non-elided stack copy, not just a
//! bookkeeping abstraction cost. The STM32H7A3's fast DTCM is only 128 KB —
//! nowhere near enough for even one 262 KB temporary, let alone several
//! stacked across call frames.
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
//! ## Measured ceiling (step 1, done for this commit)
//! Bisected on this change's host (Windows x86_64, `dev`/`test` unoptimized
//! profile — the same profile `cargo test` uses) with the exact
//! `AudioPlayer`-driven open→decode→pump loop below, one candidate stack
//! size per child process, 3 repeats per boundary value to rule out
//! run-to-run noise:
//!
//! | Stack size   | Result (WAV / FLAC / MP3) |
//! |--------------|----------------------------|
//! | 6,000,000 B  | overflow, all 3 formats, 3/3 runs |
//! | 6,050,000 B  | OK, all 3 formats, 3/3 runs |
//!
//! All three formats hit essentially the **same** ceiling (~6.0–6.05 MB) —
//! confirming the dominant cost is the repeated `AnyDecoder`/`Track`
//! stack-to-stack moves through `open_seekable`/`Deck::load`'s unoptimized
//! call frames, not any format-specific decode scratch (which lives inside
//! the already-budgeted 262 KB `AnyDecoder` itself either way).
//!
//! [`STACK_BUDGET`] below is set to **8 MiB** (8,388,608 B) — about 33 %
//! above the measured ~6.05 MB ceiling, independently reconfirmed passing
//! multiple times at this exact value during bisection.
//!
//! ## The on-target implication (this is the actual point of this test)
//! A *host* ceiling (x86_64, unoptimized, a totally different calling
//! convention/register set than Cortex-M7) is not the Cortex-M7's real
//! number — but it is decisive on one question regardless of ISA: **a
//! multi-megabyte transient, or even a `DECODER_BUDGET`-sized (288 KB) one,
//! categorically does not fit the H7A3's 128 KB DTCM.** Implication for
//! `board-h7a3` (spec §3.1):
//! - The **main stack must live in AXI SRAM**, not DTCM — DTCM is fast but
//!   simply too small for this call chain's transients, in ANY build
//!   profile.
//! - And/or the decoder should be constructed **directly into a
//!   `StaticCell`** rather than returned by value and moved — see
//!   `tests/static_placement.rs`, which also documents why `StaticCell`
//!   alone does not eliminate the construction-time stack cost this test
//!   measures (it only relocates the steady-state, at-rest storage).
//! - A *release* build does NOT rescue this — measured, not assumed. Bisecting
//!   `examples/stackprobe.rs` (`--release`) gives a ~2.1 MB ceiling with the
//!   player on-stack and **~1.44 MB with the player off-stack** (boxed ≈ the
//!   board's `StaticCell` — the board-representative figure). Both exceed the
//!   H7A3's entire 1.18 MB AXI SRAM. The earlier assumption that release would
//!   "RVO these copies away" is empirically false: the open chain moves the
//!   262 KB `AnyDecoder` by value ~5× through `open_seekable` → `provide` →
//!   `load` → `build_track`, and the optimizer does not elide them. The exact
//!   Cortex-M7 number needs QEMU/on-target confirmation (different ABI/`-Os`),
//!   but even a handful of un-elided 262 KB copies blows DTCM and stresses AXI
//!   SRAM. This is a LATENT BOARD-BLOCKER, fixed by shrinking `AnyDecoder` so
//!   moving it is cheap — see soul-audio-core's
//!   `docs/superpowers/specs/2026-11-17-decoder-memory-externalization.md`.
//!   The `size_of` budget guards measure only the at-rest footprint (664 KB,
//!   which fits) and by construction cannot see this transient — this test is
//!   what surfaces it.

use dap_core::audio::AudioPlayer;
use dap_core::ports::Storage;
use soul_audio_core::{Progress, Quality, SampleFormat, Sink, SinkError, SliceSource, StreamInfo};

/// Committed stack-region size this test proves the real open→decode→pump
/// high-water mark fits inside — see the module doc's "Measured ceiling"
/// section for how this number was chosen (ceiling ~6.05 MB + ~33% margin).
///
/// `println!`ed below so a test run's own output restates it, per the task
/// brief: this number exceeds the H7A3's 128 KB DTCM by ~64x, so on-target
/// the main stack must live in AXI SRAM (not DTCM), and/or the decoder must
/// be built directly into a static rather than returned by value — see the
/// module doc.
const STACK_BUDGET: usize = 8 * 1024 * 1024;

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

const PRELOAD: usize = 1;

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
    let mut player: AudioPlayer<DiscardSink, FixtureStorage, PRELOAD> =
        AudioPlayer::new(sink, storage, Quality::Fast);
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
