//! Proves the board pattern — holding the ~332 KB [`AudioPlayer`] (post
//! decoder-memory-externalization; was ~664 KB — see `tests/stack_highwater.rs`'s
//! module doc for the full before/after) in a `static` via [`StaticCell`]
//! (the same pattern `crates/firmware/src/main.rs` already uses for its
//! framebuffer/audio-DMA buffers) — actually WORKS end to end: the player
//! built this way opens real fixtures, decodes, and produces output, not
//! just "the types line up".
//!
//! # The honest caveat — read before assuming `StaticCell` is free
//! [`StaticCell::init`] takes its argument **by value**:
//! `fn init(&'static self, val: T) -> &'static mut T`. That means the
//! ~332 KB `AudioPlayer` this test builds is **constructed on the calling
//! thread's stack first** (`AudioPlayer::new(..)` below), and only THEN
//! moved into the cell's backing storage.
//!
//! The optimizer *usually* elides that move entirely via RVO/NRVO —
//! especially likely here since `init`'s parameter and the local `player`
//! binding are both moved-from exactly once with no intervening use — but
//! **Rust's language semantics give no guarantee of this**. There is no
//! stable placement-new in Rust: you cannot ask the compiler to construct a
//! value directly inside the static's storage, only to construct it
//! somewhere and then move it there.
//!
//! So, precisely:
//! - **At-rest placement** (everything from `init`'s return onward, for the
//!   rest of the program's life) IS genuinely static — `PLAYER_CELL`'s
//!   backing storage lives in `.bss`/a static region, not on any thread's
//!   stack, and every `pump()`/`play()` call below operates on that static
//!   memory through the `&'static mut` reference `init` returns.
//! - **Construction-time stack cost is real** and `StaticCell` does
//!   nothing to shrink it — it only relocates where the value lives
//!   *after* construction succeeds. This ties directly to
//!   `tests/stack_highwater.rs`'s `STACK_BUDGET`: that test's measured
//!   ceiling (~2.75 MB, unoptimized — down from ~6.05 MB pre-externalization)
//!   is exactly the cost of constructing this same `AudioPlayer` (plus the
//!   open/decode/pump call chain on top) — building it "into a static"
//!   doesn't change that number, it only determines where the value ends up
//!   once built.
//!
//! Do not cite this test as proof that `StaticCell` avoids a large stack
//! temporary. It doesn't. What it proves is narrower and still useful: the
//! board pattern (build once, hand out a `&'static mut` from then on, no
//! second copy ever needed again) is wired correctly and the resulting
//! reference is fully functional.

use std::path::{Path, PathBuf};

use dap_core::audio::AudioPlayer;
use dap_sim::{sac_scratch, SimSink, SimStorage};
use soul_audio_core::{Progress, Quality};
use static_cell::StaticCell;

// 0 = single resident decoder; gapless via the output ring at track reopen
// (Rockbox model) -- the board's SAI DMA ring must be deep enough to cover
// the reopen's wall-clock latency. Matches every other fixture-backed test
// in this crate.
const PRELOAD: usize = 0;

type Player = AudioPlayer<SimSink, SimStorage, Box<[i32]>, PRELOAD>;

fn tone_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/tone.wav")
}

fn antiphase_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/antiphase.wav")
}

/// The board pattern: one static cell, sized for the whole `AudioPlayer`,
/// initialized exactly once at startup. Matches `crates/firmware/src/main.rs`'s
/// `FRAMEBUFFER`/`AUDIO_BUFFER` `StaticCell` statics.
static PLAYER_CELL: StaticCell<Player> = StaticCell::new();

#[test]
fn audio_player_built_into_a_static_cell_plays_real_fixtures() {
    // Same big-stack-worker convention as `audio_headless.rs`/
    // `stack_highwater.rs`: constructing `AudioPlayer` (see module doc) and
    // moving it through `init` needs real stack room in this unoptimized
    // build, well beyond a default thread's.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .expect("spawn worker thread")
        .join()
        .expect("worker thread panicked");
}

fn run() {
    let sink = SimSink::new();
    let storage = SimStorage::new([tone_path(), antiphase_path()]);

    // Construction happens HERE, on this thread's stack (see module doc) --
    // `StaticCell::init` below only relocates the already-built value.
    let mut player = AudioPlayer::new(sink, storage, Quality::Fast, sac_scratch);
    player.set_queue(2, 0);
    player.play();

    // The board pattern: move the fully-built player into its static home.
    // `init` panics if called twice -- fine, this test calls it exactly
    // once, like a real `main()` would.
    let player: &'static mut Player = PLAYER_CELL.init(player);

    // Prove the static-celled reference actually works, not just that the
    // types line up: pump real decode/output through it and observe real
    // progress and the expected gapless track swap.
    let mut saw_played = false;
    let mut reached_track1 = false;
    for _ in 0..200_000 {
        match player.pump().expect("pump") {
            Progress::Played | Progress::TrackChanged => saw_played = true,
            Progress::Idle | Progress::Ended => {}
        }
        // At PRELOAD = 0 (single resident decoder) the move into track 1 is a
        // reopen, not a preload swap: `current()` advances to the sequencer
        // cursor first, then the next `poll_open`/`provide` actually loads the
        // decoder. Wait for it to be loaded (not merely cursor-advanced).
        if player.current() == Some(1) && player.decode_info().is_some() {
            reached_track1 = true;
            break;
        }
    }
    assert!(
        saw_played,
        "static-celled AudioPlayer never produced output (Progress::Played/TrackChanged)"
    );
    assert!(
        reached_track1,
        "expected the reopen into track 1 (single-decoder, output-ring gapless) to have loaded"
    );

    // Same reference, read again later -- proving it's a stable `&'static
    // mut`, not something that only worked once right after `init`.
    assert!(player.decode_info().is_some());
}
