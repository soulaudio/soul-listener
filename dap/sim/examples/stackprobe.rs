//! Disposable stack-ceiling probe — NOT a committed test. Used to measure the
//! real open->decode->pump peak stack in an OPTIMIZED (release) build, which
//! approximates the board far better than the unoptimized `cargo test` profile.
//!
//! Usage: `cargo run --release --example stackprobe -- <stack_bytes> <wav|flac|mp3>`
//! Spawns a worker thread of exactly `<stack_bytes>`, runs a full
//! open->decode->pump to end on it, prints `OK <stack_bytes> <fmt>` and exits 0
//! on success. On stack overflow the whole process aborts (nonzero) — so a
//! bisection driver runs one child process per candidate size.

use dap_core::audio::AudioPlayer;
use dap_core::ports::Storage;
use dap_sim::sac_scratch;
use soul_audio_core::{Progress, Quality, SampleFormat, Sink, SinkError, SliceSource, StreamInfo};

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

/// Board-representative variant: the `AudioPlayer` lives on the HEAP (a stand-in
/// for the board's `StaticCell`, i.e. off the pump caller's stack), so the
/// worker thread's stack bears only the steady-state open->decode->pump
/// transient (the `AnyDecoder`-by-value moves through `open_seekable`/
/// `Deck::load`), NOT the player object itself.
fn pump_only_boxed(player: &mut AudioPlayer<DiscardSink, FixtureStorage, Box<[i32]>, PRELOAD>) {
    player.set_queue(1, 0);
    player.play();
    loop {
        match player.pump().expect("open+decode+pump") {
            Progress::Ended => break,
            Progress::Idle | Progress::Played | Progress::TrackChanged => {}
        }
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let stack_bytes: usize = args
        .next()
        .expect("arg1 stack_bytes")
        .parse()
        .expect("stack_bytes usize");
    let fmt = args.next().unwrap_or_else(|| "wav".to_string());
    // "boxed" => player on heap (board StaticCell stand-in); else player is a
    // stack local (counts its own ~332 KB against the measured stack --
    // post decoder-memory-externalization; was ~664 KB).
    let mode = args.next().unwrap_or_else(|| "local".to_string());
    let bytes: &'static [u8] = match fmt.as_str() {
        "wav" => include_bytes!("../../../../soul-audio-core/testdata/realmusic.wav"),
        "flac" => include_bytes!("../../../../soul-audio-core/testdata/realmusic.flac"),
        "mp3" => include_bytes!("../../../../soul-audio-core/testdata/realmusic.mp3"),
        other => panic!("unknown fmt {other}"),
    };
    if mode == "boxed" {
        // Construct the player on the heap on the MAIN thread, then move the Box
        // into the worker: the worker stack never holds the ~332 KB player.
        let sink = DiscardSink {
            info: StreamInfo {
                sample_rate: 48_000,
                channels: 2,
                format: SampleFormat::Int { bits: 24 },
            },
        };
        let mut player: Box<AudioPlayer<DiscardSink, FixtureStorage, Box<[i32]>, PRELOAD>> =
            Box::new(AudioPlayer::new(
                sink,
                FixtureStorage { bytes },
                Quality::Fast,
                sac_scratch,
            ));
        std::thread::Builder::new()
            .stack_size(stack_bytes)
            .spawn(move || pump_only_boxed(&mut player))
            .expect("spawn")
            .join()
            .expect("join");
    } else {
        std::thread::Builder::new()
            .stack_size(stack_bytes)
            .spawn(move || open_decode_pump_to_end(bytes))
            .expect("spawn")
            .join()
            .expect("join");
    }
    println!("OK {stack_bytes} {fmt} {mode}");
}
