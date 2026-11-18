//! Decode **real-time-factor** (RTF) benchmarks: RTF = (decoded audio
//! seconds) / (wall-clock seconds), measured decoding real fixtures from
//! `../../../soul-audio-core/testdata/` end-to-end through
//! `soul_audio_core::open`'s format dispatch, across **all seven** formats
//! the DAP ships (WAV, FLAC, MP3, AAC, ALAC, Opus, WavPack) — plus one
//! resampler-only throughput bench and one full decode+resample **pipeline**
//! throughput bench (the shape `dap_core::audio::AudioPlayer` actually runs:
//! decode → resample → volume → sink-write). These RTFs are exactly the
//! `rtf_at_ref_clock` inputs `dap-core`'s `power::model` (battery energy
//! model) hardcodes for its clock-headroom/runtime tests — see that module's
//! doc for the measured numbers and how they're derated host→target.
//! Criterion drives the statistics/regression tracking (HTML report under
//! `target/criterion/`); a hand-rolled, criterion-independent timer alongside
//! it prints the actual RTF number straight to stdout, so reading one
//! doesn't require opening the HTML report.
//!
//! # Caveat — read this before trusting a number printed here
//!
//! **These benchmarks run on the HOST** (whatever desktop/CI x86_64 or
//! Apple-Silicon core built this binary) — roughly **10-50x faster** than
//! the DAP's actual target, a 280 MHz Cortex-M7 (STM32H7A3; see
//! `dap-arch-tests`' `memory_map::CPU_MHZ`). A host core has an out-of-order
//! pipeline, a deep cache hierarchy, and routinely a wider/faster FPU than
//! the M7's single-issue in-order pipeline ever will.
//!
//! So a number here is:
//! - A **relative regression baseline**: run this before/after touching a
//!   decoder or the resampler and diff the RTFs (or let criterion's own
//!   regression detection do it) — a meaningful drop is a real regression,
//!   independent of the host/target gap, which stays constant.
//! - An **"is the algorithm sane" sanity check**: if a format's host RTF
//!   isn't comfortably >> 1x here, it has no realistic chance of hitting 1x
//!   on a Cortex-M7 at a tenth to a fiftieth the speed.
//!
//! It is **NOT** proof of on-target real-time playback. The H7A3's actual
//! decode RTF needs silicon (`board-h7a3`) or a cycle-accurate M7 model —
//! this bench cannot, and does not claim to, predict that number.
//!
//! Run: `cargo bench -p dap-sim --bench decode_rtf`.
//!
//! Decoding runs on a 64 MiB worker-thread stack (`std::thread::Builder`),
//! the same pattern `dap/sim/tests/audio_headless.rs` uses: `AnyDecoder` is
//! large (built by value, no-alloc — see `dap-arch-tests`' `DECODER_BUDGET`
//! guard) and moves through several call frames on its way through `open`,
//! `decode_fully`, and criterion's own `Bencher::iter` plumbing, which a
//! default thread stack isn't generous enough to hold reliably. This file
//! writes its own `fn main` (rather than using the `criterion_main!` macro)
//! specifically so that thread spawn can wrap the whole criterion run.

use std::hint::black_box;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use criterion::Criterion;
use dap_core::audio::AudioPlayer;
use dap_sim::{sac_scratch, SimSink, SimStorage};
use soul_audio_core::{open, Decoder, Progress, Quality, Resampler, SliceSource};

/// Matches `audio_headless.rs`'s worker-thread stack size (see module doc).
const STACK: usize = 64 * 1024 * 1024;

/// Calibrate each hand-rolled timing to at least this much wall time, for a
/// stable number — same convention as `soul-audio-core`'s own
/// `benches/throughput.rs`.
const MIN_WALL: Duration = Duration::from_millis(300);

/// Decode `bytes` fully through `soul_audio_core::open`'s format dispatch.
/// Returns (per-channel frames decoded, sample rate, channels).
fn decode_fully(bytes: &'static [u8]) -> (u64, u32, u16) {
    let mut d = open(
        SliceSource::new(bytes),
        vec![0i32; soul_audio_core::decoder_scratch_len()],
    )
    .expect("open");
    let si = d.stream_info();
    let mut total = 0u64;
    let mut buf = [0i32; 8192];
    loop {
        let n = d.decode_into(&mut buf).expect("decode");
        if n == 0 {
            break;
        }
        total += n as u64;
    }
    let channels = u64::from(si.channels.max(1));
    (total / channels, si.sample_rate, si.channels)
}

/// Path to `realmusic.wav` (44.1 kHz/16-bit/stereo — see
/// `soul-audio-core/tests/decode.rs`'s "REAL MUSIC" fixture note), for the
/// pipeline bench below. A real path, not `include_bytes!`, because
/// [`SimStorage`] (the real `dap_core::ports::Storage` impl `AudioPlayer`
/// drives in production) opens a file by path, matching
/// `sim/tests/audio_headless.rs`'s own fixture-path convention.
fn realmusic_wav_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/realmusic.wav")
}

/// Full **pipeline** throughput: the same `dap_core::audio::AudioPlayer`
/// (`Engine` + `Deck`) the board runs in production, decoding
/// `realmusic.wav` (44.1 kHz) into a [`SimSink`] declaring [`SimSink`]'s
/// fixed 48 kHz device rate — so this exercises decode *and* resample *and*
/// the volume/sink-write stage together, unlike [`decode_fully`] (decode
/// only, passthrough rate) or [`resample_run`] (resampler only, synthetic
/// input). `PRELOAD = 0` matches the DAP's real gapless depth (single
/// resident decoder; see `sim/tests/no_alloc.rs`'s module doc). Returns
/// (per-channel frames written, device sample rate, device channels) so
/// [`print_decode_rtf`] can report this as an RTF exactly like a decode-only
/// run — "pipeline RTF", the number that actually matters for
/// `power::model`'s duty-cycle math (the CPU-active window is decode +
/// resample, not decode alone).
fn pipeline_run(path: &Path) -> (u64, u32, u16) {
    let sink = SimSink::new();
    let storage = SimStorage::new([path]);
    let mut player: AudioPlayer<SimSink, SimStorage, Box<[i32]>, 0> =
        AudioPlayer::new(sink, storage, Quality::Fast, sac_scratch);
    player.set_queue(1, 0);
    player.play();
    loop {
        if player.pump().expect("pump") == Progress::Ended {
            break;
        }
    }
    let sink = player.sink();
    let channels = sink.channels();
    let frames = (sink.recorded().len() / channels.max(1)) as u64;
    (frames, sink.sample_rate(), channels as u16)
}

/// Resample a fixed stereo block 44.1 kHz -> 48 kHz for `blocks` output
/// blocks; returns the output frame count (mirrors `soul-audio-core`'s own
/// `benches/throughput.rs::resample_run`, so the two throughput baselines
/// read the same way side by side).
fn resample_run(quality: Quality, blocks: usize) -> u64 {
    const OUT: usize = 1024;
    let mut rs = Resampler::new(2, 44_100, 48_000, 1.0, OUT, quality).expect("resampler");
    let mut out = [0i32; OUT * 2];
    let mut total_out = 0u64;
    for b in 0..blocks {
        let need = rs.input_frames_next();
        let input: Vec<i32> = (0..need * 2)
            .map(|i| ((b + i) as i32 % 4096) - 2048)
            .collect();
        rs.process(&input, &mut out).expect("process");
        total_out += out.len() as u64;
    }
    total_out
}

/// Hand-rolled, criterion-independent RTF measurement: runs `f` enough times
/// to exceed [`MIN_WALL`], takes the fastest per-call wall time (best-of-5,
/// so shared-machine jitter can only ever inflate — never deflate — a
/// sample), and prints `audio_seconds / wall_seconds` as an explicit RTF
/// line. `f` returns (per-channel frames, sample_rate) so the audio length
/// is computed from the real decode, not assumed.
fn print_decode_rtf(label: &str, f: impl Fn() -> (u64, u32, u16)) {
    let (frames, rate, channels) = f(); // warm up + capture shape
    let audio_s = frames as f64 / f64::from(rate.max(1));

    let mut iters = 1u64;
    loop {
        let t = Instant::now();
        for _ in 0..iters {
            black_box(f());
        }
        if t.elapsed() >= MIN_WALL {
            break;
        }
        iters = iters.saturating_mul(2);
    }
    let mut best = f64::INFINITY;
    for _ in 0..5 {
        let t = Instant::now();
        for _ in 0..iters {
            black_box(f());
        }
        best = best.min(t.elapsed().as_secs_f64() / iters as f64);
    }
    let rtf = audio_s / best;
    println!(
        "  {label:<6} RTF {rtf:>7.1}x   ({audio_s:.2}s audio @ {rate} Hz/{channels}ch, \
         {wall_ms:.3} ms wall)  [HOST baseline, NOT on-target — see module doc]",
        wall_ms = best * 1000.0,
    );
}

/// Same hand-rolled timing, for the resampler's Msamples/s (not an RTF —
/// there's no fixed "audio length" a resampler call corresponds to, so this
/// reports raw output throughput, matching `soul-audio-core`'s own
/// `throughput.rs` convention for its resampler benches).
fn print_resample_throughput(label: &str, f: impl Fn() -> u64) {
    let mut iters = 1u64;
    loop {
        let t = Instant::now();
        for _ in 0..iters {
            black_box(f());
        }
        if t.elapsed() >= MIN_WALL {
            break;
        }
        iters = iters.saturating_mul(2);
    }
    let mut best_ns = f64::INFINITY;
    let mut out_samples = 0u64;
    for _ in 0..5 {
        let t = Instant::now();
        for _ in 0..iters {
            out_samples = black_box(f());
        }
        best_ns = best_ns.min(t.elapsed().as_nanos() as f64 / iters as f64);
    }
    let msps = out_samples as f64 / (best_ns / 1e9) / 1e6;
    println!("  {label:<15} {msps:>7.1} Msmp/s  [HOST baseline, NOT on-target — see module doc]");
}

fn bench_decode(c: &mut Criterion) {
    let wav: &'static [u8] = include_bytes!("../../../../soul-audio-core/testdata/realmusic.wav");
    let flac: &'static [u8] = include_bytes!("../../../../soul-audio-core/testdata/realmusic.flac");
    let mp3: &'static [u8] = include_bytes!("../../../../soul-audio-core/testdata/realmusic.mp3");
    let aac: &'static [u8] =
        include_bytes!("../../../../soul-audio-core/testdata/realmusic-aac.m4a");
    let alac: &'static [u8] =
        include_bytes!("../../../../soul-audio-core/testdata/realmusic-alac.m4a");
    let opus: &'static [u8] = include_bytes!("../../../../soul-audio-core/testdata/realmusic.opus");
    let wavpack: &'static [u8] =
        include_bytes!("../../../../soul-audio-core/testdata/realmusic.wv");

    // The same ~4 s CC0 real-music clip (see soul-audio-core's
    // `tests/decode.rs` "REAL MUSIC" fixture note), encoded seven ways, so
    // the seven RTFs are directly comparable to each other -- this is the
    // DAP's full shipped format set (`soul-audio-core`'s `embedded-profile`
    // dispatch; see `dap/Cargo.toml`'s workspace-dependency comment). AAC/
    // ALAC/Opus/WavPack all route through the same `open()` dispatch as
    // WAV/FLAC/MP3 (confirmed against `soul-audio-core/benches/throughput.rs`
    // and `tests/matrix.rs`'s own realmusic-* fixture cases) -- no decoder
    // variant is dropped by `embedded-profile` for any of these (only the
    // surround-Opus `MultistreamOpusDecoder` and AAC's HE-AAC/SBR extension
    // are gated off, neither of which this stereo AAC-LC/Opus-mono-family-0
    // fixture pair touches).
    println!(
        "\ndecode RTF — HOST baseline (~10-50x faster than the 280 MHz H7A3; see module doc):"
    );
    print_decode_rtf("wav", || decode_fully(wav));
    print_decode_rtf("flac", || decode_fully(flac));
    print_decode_rtf("mp3", || decode_fully(mp3));
    print_decode_rtf("aac", || decode_fully(aac));
    print_decode_rtf("alac", || decode_fully(alac));
    print_decode_rtf("opus", || decode_fully(opus));
    print_decode_rtf("wavpack", || decode_fully(wavpack));
    println!();

    let mut group = c.benchmark_group("decode_rtf");
    group.bench_function("wav", |b| b.iter(|| decode_fully(black_box(wav))));
    group.bench_function("flac", |b| b.iter(|| decode_fully(black_box(flac))));
    group.bench_function("mp3", |b| b.iter(|| decode_fully(black_box(mp3))));
    group.bench_function("aac", |b| b.iter(|| decode_fully(black_box(aac))));
    group.bench_function("alac", |b| b.iter(|| decode_fully(black_box(alac))));
    group.bench_function("opus", |b| b.iter(|| decode_fully(black_box(opus))));
    group.bench_function("wavpack", |b| b.iter(|| decode_fully(black_box(wavpack))));
    group.finish();
}

fn bench_pipeline(c: &mut Criterion) {
    let path = realmusic_wav_path();

    println!(
        "pipeline RTF (decode+resample+sink, 44.1kHz->48kHz) — HOST baseline (see module doc):"
    );
    print_decode_rtf("wav->48k", || pipeline_run(&path));
    println!();

    let mut group = c.benchmark_group("pipeline_throughput");
    group.bench_function("wav_44k1_to_48k", |b| {
        b.iter(|| pipeline_run(black_box(&path)))
    });
    group.finish();
}

fn bench_resample(c: &mut Criterion) {
    println!("resampler 44.1->48 kHz stereo — HOST baseline (output Msamples/s):");
    print_resample_throughput("resample fast", || resample_run(Quality::Fast, 256));
    print_resample_throughput("resample hq", || resample_run(Quality::Hq, 256));
    println!();

    let mut group = c.benchmark_group("resample_throughput");
    group.bench_function("fast", |b| b.iter(|| resample_run(Quality::Fast, 32)));
    group.bench_function("hq", |b| b.iter(|| resample_run(Quality::Hq, 32)));
    group.finish();
}

/// Hand-written in place of the `criterion_group!`/`criterion_main!` macros
/// (see module doc) so the whole criterion run — bench registration,
/// sampling, and the final summary — happens on the big-stack worker thread
/// instead of the default-size main thread.
fn main() {
    std::thread::Builder::new()
        .stack_size(STACK)
        .spawn(|| {
            let mut criterion: Criterion<_> = Criterion::default().configure_from_args();
            bench_decode(&mut criterion);
            bench_resample(&mut criterion);
            bench_pipeline(&mut criterion);
            Criterion::default().configure_from_args().final_summary();
        })
        .expect("spawn bench worker thread")
        .join()
        .expect("bench worker thread panicked");
}
