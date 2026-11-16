//! `App` — the board-agnostic input/power/audio coordinator.
//!
//! Pure wiring, like [`crate::audio::AudioPlayer`]: every real decision
//! (what a decoded block sounds like, how low a battery has to get before
//! it's "critical") lives in `soul-audio-engine`/`soul-audio-core` and
//! [`crate::power::PowerService`]. `App` owns one [`AudioPlayer`], one
//! [`PowerService`], and the board's [`Power`]/[`Controls`] adapters, and
//! drives all three from a single [`App::tick`] call per scheduler period.
//!
//! ## Input mapping (PROVISIONAL)
//! [`App::map_input`] is a placeholder standing in until the real UI lands
//! (a volume HUD, menu navigation, etc. will all want a say in what a given
//! [`InputEvent`] means — a rotate during a menu is "move the selection",
//! not "change volume"). For now every event maps straight to transport,
//! unconditionally:
//! - [`InputEvent::Press`] → toggle play/pause.
//! - [`InputEvent::Rotate(n)`] → raise (`n > 0`) or lower (`n < 0`) volume
//!   by `n * AppConfig::volume_step`, clamped to `0.0..=1.0`.
//! - [`InputEvent::LongPress`] → skip to the next track.

use soul_audio_core::{DeckState, Progress, Quality, Sink};

use crate::audio::AudioPlayer;
use crate::ports::{Controls, InputEvent, Power, Storage};
use crate::power::{PowerConfig, PowerEvent, PowerService};

/// Tuning knobs for [`App::new`]: the deck's resample quality, the owned
/// [`PowerService`]'s thresholds, and the provisional volume-per-detent
/// step.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AppConfig {
    /// Resampler quality tier, forwarded to `AudioPlayer::new`.
    pub quality: Quality,
    /// Thresholds/tuning for the owned `PowerService`.
    pub power: PowerConfig,
    /// Volume change applied per `Rotate` detent (e.g. `0.05` == 5% per
    /// detent), clamped into `0.0..=1.0` — see the module's PROVISIONAL
    /// input mapping.
    pub volume_step: f32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            quality: Quality::Fast,
            power: PowerConfig::default(),
            volume_step: 0.05,
        }
    }
}

/// What one [`App::tick`] call did: the [`Controls`] events it drained and
/// how it mapped them, which [`PowerEvent`]s fired, and this tick's audio
/// pump result.
///
/// Returned for tests/telemetry — a driver doesn't need to inspect this to
/// function, since every effect is also reflected in `App`'s own accessors
/// (`state()`, `volume()`, `player().current()`, …); it's just convenient
/// for asserting "what happened this tick" without re-deriving it from
/// before/after state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TickOutcome {
    /// Number of `Controls::poll()` events drained and handled this tick.
    pub input_events: u8,
    /// A `Press` toggled play/pause this tick.
    pub play_pause_toggled: bool,
    /// Net volume delta applied this tick (sum of every `Rotate`'s
    /// `n * volume_step`, pre-clamp) — `0.0` if no `Rotate` arrived.
    pub volume_delta: f32,
    /// A `LongPress` advanced to the next track this tick.
    pub track_advanced: bool,
    /// [`PowerEvent::LowBattery`] fired this tick (playback was paused in
    /// response).
    pub low_battery: bool,
    /// [`PowerEvent::CriticalShutdown`] fired this tick (playback was
    /// stopped; the power service already requested power-off).
    pub critical_shutdown: bool,
    /// This tick's `AudioPlayer::pump` result: `Ok(progress)`, or `Err(())`
    /// if the pump itself failed (storage/open/decode/sink error) —
    /// swallowed here since `tick` has no `Result` to propagate through.
    /// Transport state still reflects every input/power effect from this
    /// same tick even when the pump errors.
    pub pump: Result<Progress, ()>,
}

/// The board-agnostic input/power/audio coordinator: owns an
/// [`AudioPlayer`], a [`PowerService`], and the board's [`Power`]/
/// [`Controls`] adapters, and drives all three from one
/// [`tick`](Self::tick) call per scheduler period.
pub struct App<S: Sink, St: Storage, P: Power, C: Controls, const PRELOAD: usize> {
    player: AudioPlayer<S, St, PRELOAD>,
    power_svc: PowerService,
    power: P,
    controls: C,
    volume: f32,
    volume_step: f32,
}

impl<S: Sink, St: Storage, P: Power, C: Controls, const PRELOAD: usize> App<S, St, P, C, PRELOAD> {
    /// Build a coordinator over `sink`/`storage` (→ an [`AudioPlayer`]),
    /// `power` (→ a [`PowerService`]), and `controls`. The queue starts
    /// empty — call [`set_queue`](Self::set_queue) before the first
    /// [`tick`](Self::tick) that should produce sound. Starting volume is
    /// full scale (`1.0`).
    pub fn new(sink: S, storage: St, power: P, controls: C, config: AppConfig) -> Self {
        Self {
            player: AudioPlayer::new(sink, storage, config.quality),
            power_svc: PowerService::new(config.power),
            power,
            controls,
            volume: 1.0,
            volume_step: config.volume_step,
        }
    }

    /// (Re)define the playback queue — forwarded to
    /// `AudioPlayer::set_queue`.
    pub fn set_queue(&mut self, len: usize, start: usize) {
        self.player.set_queue(len, start);
    }

    /// Drive one scheduler period:
    /// 1. Drain every pending [`Controls`] event and map it to
    ///    transport/volume (see the module's PROVISIONAL mapping).
    /// 2. Poll the [`PowerService`] and act on its events: pause on
    ///    [`PowerEvent::LowBattery`], stop on
    ///    [`PowerEvent::CriticalShutdown`] (the service itself already
    ///    requested power-off for the latter).
    /// 3. Pump the deck one block.
    ///
    /// `now_ms` drives the power service's idle/sleep timing exactly like
    /// [`PowerService::poll`] does. There is no separate `user_active`
    /// input here (PROVISIONAL): this tick treats "at least one `Controls`
    /// event arrived this tick" as the activity signal — a future UI layer
    /// that wants a finer-grained idle definition (e.g. a screen touch with
    /// no wheel motion) can still reach the same `PowerService` directly.
    pub fn tick(&mut self, now_ms: u64) -> TickOutcome {
        let mut outcome = TickOutcome {
            input_events: 0,
            play_pause_toggled: false,
            volume_delta: 0.0,
            track_advanced: false,
            low_battery: false,
            critical_shutdown: false,
            pump: Ok(Progress::Idle),
        };

        while let Some(event) = self.controls.poll() {
            outcome.input_events = outcome.input_events.saturating_add(1);
            self.map_input(event, &mut outcome);
        }

        let user_active = outcome.input_events > 0;
        let events = self.power_svc.poll(&mut self.power, now_ms, user_active);
        for event in events.iter().copied() {
            match event {
                PowerEvent::LowBattery => {
                    self.player.pause();
                    outcome.low_battery = true;
                }
                PowerEvent::CriticalShutdown => {
                    self.player.stop();
                    outcome.critical_shutdown = true;
                }
                _ => {}
            }
        }

        outcome.pump = self.player.pump().map_err(|_| ());
        outcome
    }

    /// PROVISIONAL input mapping — see the module docs. Kept as one small
    /// function so the real UI (a later task) has a single, obvious seam to
    /// replace.
    fn map_input(&mut self, event: InputEvent, outcome: &mut TickOutcome) {
        match event {
            InputEvent::Press => {
                if self.player.state() == DeckState::Playing {
                    self.player.pause();
                } else {
                    self.player.play();
                }
                outcome.play_pause_toggled = true;
            }
            InputEvent::Rotate(n) => {
                let delta = f32::from(n) * self.volume_step;
                self.volume = (self.volume + delta).clamp(0.0, 1.0);
                self.player.set_volume(self.volume);
                outcome.volume_delta += delta;
            }
            InputEvent::LongPress => {
                let _ = self.player.next();
                outcome.track_advanced = true;
            }
        }
    }

    /// Borrow the owned [`AudioPlayer`] (transport/queue queries, sink
    /// access).
    #[must_use]
    pub fn player(&self) -> &AudioPlayer<S, St, PRELOAD> {
        &self.player
    }

    /// Mutably borrow the owned [`AudioPlayer`] (e.g. for a driver's own
    /// sink-side controls).
    pub fn player_mut(&mut self) -> &mut AudioPlayer<S, St, PRELOAD> {
        &mut self.player
    }

    /// Smoothed battery state of charge, per-mille (0..=1000) — see
    /// [`PowerService::soc_permille`].
    #[must_use]
    pub fn power_soc(&self) -> u16 {
        self.power_svc.soc_permille()
    }

    /// Current output volume (0.0..=1.0), as tracked by the provisional
    /// `Rotate` mapping (there is no getter on [`Sink`]/the deck to read
    /// this back from — volume is applied one-way at the sink's output
    /// stage).
    #[must_use]
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Current deck transport state — see `AudioPlayer::state`.
    #[must_use]
    pub fn state(&self) -> DeckState {
        self.player.state()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::VecDeque;
    use std::sync::{Arc, Mutex};

    use soul_audio_core::{SampleFormat, SinkError, SliceSource, StreamInfo};

    use super::*;
    use crate::ports::BatteryReading;
    use crate::power::PowerProfile;

    // --- mocks --------------------------------------------------------

    /// A battery reading shared between a `MockPower` (moved into the
    /// `App`) and the test, so the test can change what the power service
    /// sees on a later tick without needing `App` to expose a `power_mut`.
    /// `Arc<Mutex<_>>` (not `Rc<RefCell<_>>`) so the whole fixture stays
    /// `Send` — every test below runs its body on a dedicated large-stack
    /// thread (see `run_on_big_stack`).
    #[derive(Clone)]
    struct SharedReading(Arc<Mutex<BatteryReading>>);

    impl SharedReading {
        fn new(reading: BatteryReading) -> Self {
            Self(Arc::new(Mutex::new(reading)))
        }

        fn set(&self, reading: BatteryReading) {
            *self.0.lock().expect("lock poisoned") = reading;
        }

        fn get(&self) -> BatteryReading {
            *self.0.lock().expect("lock poisoned")
        }
    }

    struct MockPower {
        reading: SharedReading,
    }

    impl Power for MockPower {
        fn read(&mut self) -> BatteryReading {
            self.reading.get()
        }
        fn set_charge_current_ma(&mut self, _ma: u16) {}
        fn set_clock_profile(&mut self, _profile: PowerProfile) {}
        fn request_power_off(&mut self) {}
    }

    /// An `InputEvent` queue shared between a `MockControls` (moved into
    /// the `App`) and the test, for the same reason as `SharedReading`:
    /// events get scripted *between* `tick` calls, not all up front.
    #[derive(Clone)]
    struct SharedQueue(Arc<Mutex<VecDeque<InputEvent>>>);

    impl SharedQueue {
        fn new() -> Self {
            Self(Arc::new(Mutex::new(VecDeque::new())))
        }

        fn push(&self, event: InputEvent) {
            self.0.lock().expect("lock poisoned").push_back(event);
        }
    }

    struct MockControls {
        queue: SharedQueue,
    }

    impl Controls for MockControls {
        fn poll(&mut self) -> Option<InputEvent> {
            self.queue.0.lock().expect("lock poisoned").pop_front()
        }
    }

    struct MockSink {
        info: StreamInfo,
    }

    impl Sink for MockSink {
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

    /// Hands back the same in-memory WAV bytes for every track index — the
    /// test only needs `AudioPlayer::pump` to succeed enough to exercise
    /// transport/volume/power wiring, not distinct per-track content.
    struct MockStorage {
        bytes: &'static [u8],
    }

    impl Storage for MockStorage {
        type Source = SliceSource<'static>;
        type Error = ();

        fn open(&mut self, _track: usize) -> Result<Self::Source, Self::Error> {
            Ok(SliceSource::new(self.bytes))
        }
    }

    /// Build a minimal, fully valid mono 16-bit PCM WAV in memory: `frames`
    /// silent samples at `sample_rate` Hz. Real enough for
    /// `soul_audio_core::open_seekable` to probe and decode (just not
    /// musical) — `dap-sim`'s `audio_headless` test already proves the full
    /// resample/gapless path against real music fixtures; this crate's unit
    /// tests only need the transport/volume/power wiring exercised, so a
    /// tiny silent fixture (leaked to `'static` so it satisfies
    /// `ports::Storage::Source`, which carries no lifetime) is enough.
    fn tiny_wav(sample_rate: u32, frames: u32) -> &'static [u8] {
        let channels: u16 = 1;
        let bits: u16 = 16;
        let block_align = channels * (bits / 8);
        let data_size = frames * u32::from(block_align);

        let mut v = Vec::with_capacity(44 + data_size as usize);
        v.extend_from_slice(b"RIFF");
        v.extend_from_slice(&(36 + data_size).to_le_bytes());
        v.extend_from_slice(b"WAVE");
        v.extend_from_slice(b"fmt ");
        v.extend_from_slice(&16u32.to_le_bytes());
        v.extend_from_slice(&1u16.to_le_bytes()); // PCM
        v.extend_from_slice(&channels.to_le_bytes());
        v.extend_from_slice(&sample_rate.to_le_bytes());
        let byte_rate = sample_rate * u32::from(block_align);
        v.extend_from_slice(&byte_rate.to_le_bytes());
        v.extend_from_slice(&block_align.to_le_bytes());
        v.extend_from_slice(&bits.to_le_bytes());
        v.extend_from_slice(b"data");
        v.extend_from_slice(&data_size.to_le_bytes());
        v.resize(v.len() + data_size as usize, 0);
        Box::leak(v.into_boxed_slice())
    }

    /// Fixture sample rate, matching the `MockSink`'s declared rate so no
    /// resampling occurs — irrelevant to what's under test here.
    const SAMPLE_RATE: u32 = 8_000;
    /// 200_000 frames (25 s @ 8 kHz) — comfortably more than any test loop
    /// below pumps through (`BLOCK` = 256 output frames/tick), so the
    /// fixture never runs out mid-test and masks a transport assertion
    /// behind an unrelated `Progress::Ended`.
    const FRAMES: u32 = 200_000;

    type TestApp = App<MockSink, MockStorage, MockPower, MockControls, 1>;

    fn healthy_reading(soc_permille: u16) -> BatteryReading {
        BatteryReading {
            soc_permille,
            voltage_mv: 3800,
            current_ma: -200,
            temp_centi_c: 2500,
            charger_present: false,
            charging: false,
        }
    }

    fn build_app(queue_len: usize) -> (TestApp, SharedQueue, SharedReading) {
        let queue = SharedQueue::new();
        let reading = SharedReading::new(healthy_reading(800));
        let sink = MockSink {
            info: StreamInfo {
                sample_rate: SAMPLE_RATE,
                channels: 1,
                format: SampleFormat::Int { bits: 24 },
            },
        };
        let storage = MockStorage {
            bytes: tiny_wav(SAMPLE_RATE, FRAMES),
        };
        let controls = MockControls {
            queue: queue.clone(),
        };
        let power = MockPower {
            reading: reading.clone(),
        };
        let mut app = App::new(sink, storage, power, controls, AppConfig::default());
        app.set_queue(queue_len, 0);
        (app, queue, reading)
    }

    /// Run `f` on a dedicated 64 MiB-stack thread, then propagate any panic
    /// (via `join().unwrap()`) as this test's own failure.
    ///
    /// `Deck`/`Engine` embed `AnyDecoder` by value (no-alloc) in both the
    /// current slot and the preload slot, and move it through several
    /// unoptimized (debug) call frames inside `AudioPlayer::pump` — the
    /// same reason `dap-sim`'s `audio_headless` test spawns its worker with
    /// a 64 MiB stack instead of the default thread stack. Every test below
    /// calls `App::tick` (which pumps the deck), so every test needs it.
    fn run_on_big_stack(f: impl FnOnce() + Send + 'static) {
        std::thread::Builder::new()
            .stack_size(64 * 1024 * 1024)
            .spawn(f)
            .expect("spawn worker thread")
            .join()
            .expect("worker thread panicked");
    }

    // --- tests ----------------------------------------------------------

    #[test]
    fn press_toggles_play_pause() {
        run_on_big_stack(|| {
            let (mut app, queue, _reading) = build_app(1);

            // First tick, no input: the engine auto-loads and starts playing
            // track 0 (`Deck::load` sets `Playing` unconditionally) — nothing
            // to toggle yet.
            let outcome = app.tick(0);
            assert!(!outcome.play_pause_toggled);
            assert_eq!(app.state(), DeckState::Playing);

            // Press while playing -> pause.
            queue.push(InputEvent::Press);
            let outcome = app.tick(1_000);
            assert!(outcome.play_pause_toggled);
            assert_eq!(app.state(), DeckState::Paused);

            // Press while paused -> resume.
            queue.push(InputEvent::Press);
            let outcome = app.tick(2_000);
            assert!(outcome.play_pause_toggled);
            assert_eq!(app.state(), DeckState::Playing);
        });
    }

    #[test]
    fn rotate_raises_and_lowers_volume() {
        run_on_big_stack(|| {
            let (mut app, queue, _reading) = build_app(1);
            app.tick(0);
            assert_eq!(app.volume(), 1.0, "starting volume is full scale");

            queue.push(InputEvent::Rotate(-4));
            let outcome = app.tick(1_000);
            assert!(
                outcome.volume_delta < 0.0,
                "negative rotate must lower volume"
            );
            let lowered = app.volume();
            assert!(lowered < 1.0);

            queue.push(InputEvent::Rotate(4));
            let outcome = app.tick(2_000);
            assert!(outcome.volume_delta > 0.0, "positive rotate must raise volume");
            assert!(
                app.volume() > lowered,
                "volume must have gone back up from the lowered level"
            );
        });
    }

    #[test]
    fn rotate_volume_clamps_to_unit_range() {
        run_on_big_stack(|| {
            let (mut app, queue, _reading) = build_app(1);
            app.tick(0);

            // Way more negative than one detent could ever need: must clamp at
            // 0.0, not go negative.
            queue.push(InputEvent::Rotate(-100));
            app.tick(1_000);
            assert_eq!(app.volume(), 0.0);

            // Symmetric check at the top end.
            queue.push(InputEvent::Rotate(100));
            app.tick(2_000);
            assert_eq!(app.volume(), 1.0);
        });
    }

    #[test]
    fn long_press_advances_track() {
        run_on_big_stack(|| {
            let (mut app, queue, _reading) = build_app(2);

            app.tick(0);
            assert_eq!(app.player().current(), Some(0));

            queue.push(InputEvent::LongPress);
            let outcome = app.tick(1_000);
            assert!(outcome.track_advanced);
            assert_eq!(app.player().current(), Some(1));
        });
    }

    #[test]
    fn low_battery_pauses_playback() {
        run_on_big_stack(|| {
            let (mut app, _queue, reading) = build_app(1);

            app.tick(0);
            assert_eq!(
                app.state(),
                DeckState::Playing,
                "must be playing before the battery drops"
            );

            let mut fired = false;
            for i in 1..60u64 {
                reading.set(healthy_reading(0));
                let outcome = app.tick(i * 1_000);
                if outcome.low_battery {
                    fired = true;
                    break;
                }
            }
            assert!(fired, "LowBattery must fire within the loop budget");
            assert_eq!(
                app.state(),
                DeckState::Paused,
                "LowBattery must pause an already-playing deck"
            );
        });
    }

    #[test]
    fn critical_battery_stops_playback() {
        run_on_big_stack(|| {
            let (mut app, _queue, reading) = build_app(1);

            app.tick(0);
            assert_eq!(app.state(), DeckState::Playing);

            let mut fired = false;
            for i in 1..100u64 {
                reading.set(healthy_reading(0));
                let outcome = app.tick(i * 1_000);
                if outcome.critical_shutdown {
                    fired = true;
                    break;
                }
            }
            assert!(fired, "CriticalShutdown must fire within the loop budget");
            assert_eq!(
                app.state(),
                DeckState::Stopped,
                "CriticalShutdown must stop an already-playing deck"
            );
        });
    }
}
