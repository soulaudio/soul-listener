//! Ports: the capabilities a board (`board-h7a3`) or a host simulator
//! (`dap-sim`) must supply for [`crate::audio::AudioPlayer`] to play audio.
//!
//! [`Sink`], [`Source`], [`BufSource`], and [`SeekSource`] are re-exported
//! from `soul-audio-core` as-is — they're already board-agnostic (a cpal
//! `Sink` and a SAI/I²S DMA `Sink` both satisfy the same trait; a slice, a
//! desktop `File`, and an SD-card file all satisfy the same source ladder).
//! [`Storage`] is this crate's one original port.

pub use soul_audio_core::{BufSource, SeekSource, Sink, Source};

/// The minimal capability [`AudioPlayer`](crate::audio::AudioPlayer) needs to
/// turn a queue position into bytes ready for decoding: given a track index,
/// open its encoded bytes as a seekable, bufferable source.
///
/// This mirrors (and generalizes) the two-step shape of
/// `soul-audio-firmware`'s SD reference: `playback::locate(id)` resolves a
/// track's on-card folder/file coordinates from the baked `.soulidx`, and
/// `sd_source::SdSource` wraps the opened `embedded_sdmmc::File` as a
/// `Source`/`SeekSource`. A `Storage` implementation owns both steps —
/// resolving `track` to its physical location *and* opening it — behind one
/// call, so this crate never needs to know what a "location" looks like on
/// disk, SD, or in an in-memory test fixture.
///
/// `Self::Source` is bound on [`BufSource`] + [`SeekSource`] (not just
/// [`Source`]) because [`AudioPlayer`](crate::audio::AudioPlayer) hands it
/// straight to `soul_audio_core::open_seekable`, which probes the container
/// via `BufSource::fill_buf` and needs `SeekSource` both for the resulting
/// decoder to seek and (for MP4) to find `moov` wherever it sits.
///
/// Deliberately `dyn`-free (an associated type, not a trait object) and
/// alloc-free: the concrete source — an SD file, a desktop `File`, a
/// `SliceSource` over an in-memory fixture — lives entirely in the
/// implementor. `open` is called off the real-time path: `AudioPlayer::pump`
/// only ever writes through an *already-opened* decoder (see
/// `soul_audio_engine::Engine`'s `poll_open`/`provide` split, which this
/// trait's `open` fills the "open" half of).
pub trait Storage {
    /// The seekable, bufferable byte source this storage hands back for a
    /// track — e.g. an SD file wrapper, a desktop `File` adapter, or an
    /// in-memory `SliceSource` fixture.
    type Source: BufSource + SeekSource;
    /// Storage-level error: I/O failure, track index out of range, a
    /// corrupt/truncated index, etc.
    type Error;

    /// Open the bytes for the track at queue position `track`, ready for
    /// `soul_audio_core::open_seekable` to probe and decode.
    ///
    /// # Errors
    /// Implementation-defined: typically an I/O failure or an out-of-range
    /// `track`.
    fn open(&mut self, track: usize) -> Result<Self::Source, Self::Error>;
}

/// One battery/charger sample: everything [`crate::power::PowerService`]
/// needs to drive its state machine for a single [`Power::read`] poll.
///
/// Units are fixed-point/integer throughout (no floats): `soc_permille` is
/// state of charge out of 1000 (so `150` = 15.0%), `current_ma` is signed
/// with the convention "positive = current flowing *into* the battery
/// (charging), negative = current flowing *out* (discharging)" — mirroring
/// how fuel-gauge ICs (e.g. MAX17048-style coulomb counters) typically report
/// it, so a board implementation can usually pass the raw register value
/// straight through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BatteryReading {
    /// State of charge, per-mille (0..=1000).
    pub soc_permille: u16,
    /// Terminal voltage, millivolts.
    pub voltage_mv: u16,
    /// Battery current, milliamps. Positive = charging, negative =
    /// discharging (see struct docs).
    pub current_ma: i16,
    /// Battery pack temperature, centi-°C (e.g. `2550` = 25.50°C).
    pub temp_centi_c: i16,
    /// Whether a charger is physically connected (independent of whether
    /// it's actively delivering current right now).
    pub charger_present: bool,
    /// Whether the charge IC currently reports actively charging.
    pub charging: bool,
}

/// The board's battery/charger capability: [`crate::power::PowerService`]
/// polls [`read`](Power::read) and drives the actuator methods in response —
/// it never touches hardware registers directly, so the same state machine
/// runs over real fuel-gauge/charger ICs on-target and over a scripted
/// `SimBattery` on the host.
///
/// Deliberately `dyn`-free and alloc-free, matching [`Storage`]: a concrete,
/// monomorphized implementor (board driver or host simulator), no trait
/// objects.
pub trait Power {
    /// Sample the current battery/charger state.
    fn read(&mut self) -> BatteryReading;

    /// Set the charge IC's target charge current, in milliamps (e.g. to
    /// taper or halt charging under [`crate::power::PowerEvent::ThermalThrottle`]).
    fn set_charge_current_ma(&mut self, ma: u16);

    /// Set the system clock/performance profile (e.g. to engage
    /// [`crate::power::PowerProfile::BatterySaver`] at low battery).
    fn set_clock_profile(&mut self, profile: crate::power::PowerProfile);

    /// Request an orderly power-off (called once on
    /// [`crate::power::PowerEvent::CriticalShutdown`]).
    fn request_power_off(&mut self);
}
