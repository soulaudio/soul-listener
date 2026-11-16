//! Headless, hardware-free proof that the whole audio path — real files ->
//! soul-audio-core decode/resample/gapless -> a `Sink` -- runs correctly
//! end to end when driven through `dap_core::audio::AudioPlayer`.
//!
//! Fixtures (`soul-audio-core/testdata`, the shared-tone playback set):
//! - `tone.wav`      -- 44.1 kHz, 2 ch, 16-bit, 88_200 frames (2.0 s)
//! - `antiphase.wav` -- 48.0 kHz, 2 ch, 16-bit, 96_000 frames (2.0 s)
//!
//! queued in that order into a sink declaring a 48 kHz device rate, so track
//! 0 is resampled and track 1 is rate-matched passthrough -- exercising both
//! paths and the boundary between them.

use std::path::{Path, PathBuf};

use dap_core::audio::AudioPlayer;
use dap_sim::{SimSink, SimStorage};
use soul_audio_core::{Progress, Quality};

// 1 = gapless (one preloaded next track, all gapless playback needs);
// PRELOAD = 2 was a leftover desktop value.
const PRELOAD: usize = 1;

fn tone_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/tone.wav")
}

fn antiphase_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/antiphase.wav")
}

type Player = AudioPlayer<SimSink, SimStorage, PRELOAD>;

fn build_player(volume: Option<f32>) -> Player {
    let sink = SimSink::new();
    let storage = SimStorage::new([tone_path(), antiphase_path()]);
    let mut player = AudioPlayer::new(sink, storage, Quality::Fast);
    player.set_queue(2, 0);
    if let Some(v) = volume {
        player.set_volume(v);
    }
    player.play();
    player
}

/// Pump until the sequencer's cursor reaches `target` (i.e. the gapless swap
/// into that track has happened), bounded so a stuck pipeline fails loudly
/// instead of hanging.
fn pump_until_current(player: &mut Player, target: usize) {
    for _ in 0..1_000_000 {
        if player.current() == Some(target) {
            return;
        }
        match player.pump().expect("pump") {
            Progress::Ended => panic!("queue ended before reaching track {target}"),
            Progress::Idle | Progress::Played | Progress::TrackChanged => {}
        }
    }
    panic!("track {target} not reached within the pump budget");
}

/// Pump to the natural end of the queue.
fn run_to_end(player: &mut Player) {
    for _ in 0..1_000_000 {
        if player.pump().expect("pump") == Progress::Ended {
            return;
        }
    }
    panic!("queue did not end within the pump budget");
}

/// Longest run of frames (across the whole recorded stream) whose samples on
/// every channel are exactly zero. A real gapless-boundary bug (silence
/// inserted between tracks) shows up as a long run; the fixtures' 50 ms
/// in/out fades only ever touch exact zero at a handful of edge frames.
fn longest_silent_run(samples: &[i32], channels: usize) -> (usize, usize) {
    let mut longest = 0usize;
    let mut longest_start = 0usize;
    let mut current = 0usize;
    let mut current_start = 0usize;
    for (i, frame) in samples.chunks_exact(channels).enumerate() {
        if frame.iter().all(|&s| s == 0) {
            if current == 0 {
                current_start = i;
            }
            current += 1;
            if current > longest {
                longest = current;
                longest_start = current_start;
            }
        } else {
            current = 0;
        }
    }
    (longest, longest_start)
}

fn mean_abs(samples: &[i32]) -> f64 {
    samples.iter().map(|&s| (s as f64).abs()).sum::<f64>() / samples.len() as f64
}

#[test]
fn gapless_resample_and_volume() {
    // Generous worker stack: `Deck`/`Engine` embed `AnyDecoder` by value
    // (no-alloc) in both the current slot and each of the `PRELOAD` preload
    // slots, and move it through several unoptimized (debug) call frames —
    // the same reason `soul-audio-cli`'s `main.rs` spawns its worker with a
    // 64 MiB stack instead of the default thread stack.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .expect("spawn worker thread")
        .join()
        .expect("worker thread panicked");
}

fn run() {
    let mut player = build_player(None);

    // One pump opens + loads track 0 (and primes track 1) and decodes one
    // block. Prove the resample path directly off the real decode chain, not
    // by inference from frame counts alone.
    player.pump().expect("pump");
    let info0 = player.decode_info().expect("track 0 loaded");
    assert_eq!(info0.decoded.sample_rate, 44_100, "tone.wav is 44.1 kHz");
    assert_eq!(info0.device_rate, 48_000, "sink declares 48 kHz");
    assert!(
        info0.resampling,
        "44.1 kHz source into a 48 kHz sink must resample"
    );

    pump_until_current(&mut player, 1);
    let info1 = player.decode_info().expect("track 1 loaded");
    assert_eq!(info1.decoded.sample_rate, 48_000, "antiphase.wav is 48 kHz");
    assert!(
        !info1.resampling,
        "48 kHz source into a 48 kHz sink is passthrough"
    );

    run_to_end(&mut player);

    let sink = player.sink();
    assert_eq!(sink.sample_rate(), 48_000, "SimSink's declared device rate");
    let channels = sink.channels();
    let recorded = sink.recorded();
    let total_frames = recorded.len() / channels;
    assert!(total_frames > 0);

    // Expected: 88_200 src frames @44.1k resampled to 48k (exactly 96_000 for
    // this ratio: 88_200 * 48_000 / 44_100 == 96_000) + 96_000 passthrough
    // frames from track 1, with slack for the resampler's block-quantized
    // startup/drain.
    let expected = 96_000 + 96_000;
    assert!(
        total_frames.abs_diff(expected) < 2_000,
        "total frames {total_frames} vs expected ~{expected}"
    );

    // Gapless: no inserted silence anywhere, including the track boundary --
    // beyond the resampler's own known group delay/drain margin. A resampled
    // track's `Deck::pump` keeps calling the resampler with zero-padded input
    // for `latency / BLOCK + 2` extra blocks after decoder EOF, to flush the
    // filter's internal history (`soul_audio_core::deck`'s `block_resampled` /
    // `drain` field) -- the tail of *that* ringdown can legitimately land on
    // exact silence for up to a couple of its fixed 256-frame output blocks,
    // still counted as track 0's own output (it swaps to track 1 only once
    // the whole drain finishes). That is a bounded, documented artifact of
    // *this* track being resampled, not a dropped/duplicated buffer -- so the
    // bound here is "at most a couple of resampler output blocks", not zero.
    // (Measured on this fixture pair: exactly one 256-frame block, landing
    // right at the resampled track's natural end.)
    let (longest_gap, gap_start) = longest_silent_run(recorded, channels);
    assert!(
        longest_gap <= 512,
        "found a {longest_gap}-frame all-zero run starting at frame {gap_start} (total {total_frames}) -- looks like a real gap, bigger than the resampler's drain margin can explain"
    );

    // Volume: an independent run at 0.5 must be measurably quieter than the
    // default (1.0) run. `Deck` never scales for volume itself (only for
    // ReplayGain) -- the effect asserted on here is `SimSink` applying gain
    // at its output stage, exactly where a real cpal/SAI sink would.
    let mut full = build_player(Some(1.0));
    run_to_end(&mut full);
    let level_full = mean_abs(full.sink().recorded());

    let mut half = build_player(Some(0.5));
    run_to_end(&mut half);
    let level_half = mean_abs(half.sink().recorded());

    assert!(level_full > 0.0, "sanity: full-volume run isn't silent");
    let ratio = level_half / level_full;
    assert!(
        (0.4..0.6).contains(&ratio),
        "volume 0.5 level ratio {ratio} vs 1.0 (expected ~0.5)"
    );
}
