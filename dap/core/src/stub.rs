//! No_std, no-alloc stub port adapters — placeholders for the real hardware
//! drivers (SAI/I²S DMA, the SD-card catalog reader, the SSD1677 e-ink SPI
//! driver, the PEC11R/EC11 wheel+button GPIO driver, the BQ fuel-gauge I²C
//! driver) that Phase E ("Hardware bring-up") will swap in one at a time —
//! see the roadmap's Phase B, which these exist for: proving the *complete*
//! `App` wiring links and runs for `thumbv7em-none-eabihf` today, ahead of
//! any PCB existing.
//!
//! Every type here is a trivial, deterministic, zero-size placeholder: no
//! real I/O, nothing that can panic, `forbid(unsafe_code)` (inherited from
//! the crate root — see `lib.rs`). None of these are meant to ship; they are
//! exactly what a real driver replaces, one port at a time, as hardware
//! comes online.

use embedded_graphics::draw_target::DrawTarget;
use embedded_graphics::geometry::{OriginDimensions, Size};
use embedded_graphics::Pixel;
use soul_audio_core::{SampleFormat, Sink, SinkError, SliceSource, StreamInfo};

use crate::display::{DisplayError, EinkRefresh, Gray4, Rectangle};
use crate::ports::{BatteryReading, Controls, InputEvent, Power, Storage};
use crate::power::PowerProfile;

/// Declared device sample rate (Hz) [`StubSink`] reports — matches
/// `dap-sim`'s `SimSink` default so swapping `Stub*` for `Sim*` (or back)
/// sees the same declared stream shape.
pub const STREAM_SAMPLE_RATE: u32 = 48_000;
/// Declared device channel count [`StubSink`] reports — see
/// [`STREAM_SAMPLE_RATE`].
pub const STREAM_CHANNELS: u16 = 2;

/// Placeholder [`Sink`]: declares a fixed [`STREAM_SAMPLE_RATE`]/
/// [`STREAM_CHANNELS`]/24-bit stream and discards every write — the real
/// SAI/I²S DMA sink (Phase E) replaces this.
#[derive(Debug, Default)]
pub struct StubSink;

impl Sink for StubSink {
    fn stream_info(&self) -> StreamInfo {
        StreamInfo {
            sample_rate: STREAM_SAMPLE_RATE,
            channels: STREAM_CHANNELS,
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

/// Why [`StubStorage::open`] always fails: there is no real media backend
/// wired up yet — the real SD-card `.soulidx` catalog reader (Phase E)
/// replaces [`StubStorage`] outright. [`StubStorage`] exists purely so
/// [`crate::app::App`] can construct and run its tick loop before any media
/// exists, not to ever actually play audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoMedia;

/// Placeholder [`Storage`]: reports [`NoMedia`] for every track, every
/// time — the real SD-card storage backend (Phase E) replaces this.
///
/// Because `open` always fails *before* `AudioPlayer::pump` ever reaches its
/// `scratch_fn` call (see `AudioPlayer::pump`'s doc comment: `storage.open`
/// is called, and propagated via `?`, strictly before `(self.scratch_fn)()`),
/// a `StubStorage`-backed `App` never actually touches its decoder scratch
/// buffer — which is exactly what lets a board's `scratch_fn` safely be a
/// plain `StaticCell::init` call (see `board-h7a3/src/main.rs`'s `SCRATCH`):
/// it would panic if called a second time, but `StubStorage` guarantees it
/// is never called even once.
#[derive(Debug, Default)]
pub struct StubStorage;

impl Storage for StubStorage {
    type Source = SliceSource<'static>;
    type Error = NoMedia;

    fn open(&mut self, _track: usize) -> Result<Self::Source, Self::Error> {
        Err(NoMedia)
    }
}

/// Placeholder [`Controls`]: never reports an input event — the real
/// PEC11R/EC11 wheel+button GPIO driver (Phase E) replaces this.
#[derive(Debug, Default)]
pub struct StubControls;

impl Controls for StubControls {
    fn poll(&mut self) -> Option<InputEvent> {
        None
    }
}

/// The fixed, healthy-looking reading [`StubPower::read`] always returns:
/// 80% state of charge, a nominal Li-ion terminal voltage, a nominal
/// discharge current, room temperature, no charger connected — chosen so a
/// `StubPower`-backed [`crate::power::PowerService`] never spuriously fires
/// [`crate::power::PowerEvent::LowBattery`]/[`crate::power::PowerEvent::CriticalShutdown`]
/// against [`crate::power::PowerConfig::default`]'s thresholds, matching
/// `dap_core::app`'s own private test fixture's `healthy_reading` shape.
pub const STUB_BATTERY_READING: BatteryReading = BatteryReading {
    soc_permille: 800,
    voltage_mv: 3_800,
    current_ma: -200,
    temp_centi_c: 2_500,
    charger_present: false,
    charging: false,
};

/// Placeholder [`Power`]: reports the fixed [`STUB_BATTERY_READING`] and
/// discards every actuator call — the real BQ fuel-gauge/charger-IC I²C
/// driver (Phase E) replaces this.
#[derive(Debug, Default)]
pub struct StubPower;

impl Power for StubPower {
    fn read(&mut self) -> BatteryReading {
        STUB_BATTERY_READING
    }

    fn set_charge_current_ma(&mut self, _ma: u16) {}
    fn set_clock_profile(&mut self, _profile: PowerProfile) {}
    fn request_power_off(&mut self) {}
}

/// Panel width [`StubDisplay`] declares, pixels — the GDEM0397T81P/SSD1677
/// panel's native resolution, matching `dap-sim`'s `SimDisplay` (see
/// `dap-sim/src/display.rs`).
pub const PANEL_WIDTH: u32 = 800;
/// Panel height [`StubDisplay`] declares, pixels — see [`PANEL_WIDTH`].
pub const PANEL_HEIGHT: u32 = 480;

/// Placeholder [`crate::display::Display`]: declares the real panel's
/// [`PANEL_WIDTH`]x[`PANEL_HEIGHT`] geometry, discards every drawn pixel,
/// and never fails a refresh — the real SSD1677 SPI driver (Phase E)
/// replaces this.
#[derive(Debug, Default)]
pub struct StubDisplay;

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
        Size::new(PANEL_WIDTH, PANEL_HEIGHT)
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
