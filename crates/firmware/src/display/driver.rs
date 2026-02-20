//! SSD1677 / SSD2677 Hardware Driver
//!
//! Complete async driver for the SSD1677 / SSD2677 e-ink controller used in the
//! Good Display GDEM0397T81P (800×480, Carta 1200) panel.
//!
//! # Wiring (STM32H743ZI example)
//!
//! | Signal | STM32 pin | Direction |
//! |--------|-----------|-----------|
//! | SCK    | PA5 (SPI1_SCK)  | Host → Display |
//! | MOSI   | PA7 (SPI1_MOSI) | Host → Display |
//! | DC     | PB0 (GPIO)      | Host → Display |
//! | CS     | Managed by `SpiDevice` | Host → Display |
//! | RST    | PB2 (GPIO)      | Host → Display |
//! | BUSY   | PE3 (GPIO)      | Display → Host |
//!
//! # Memory layout
//!
//! The internal framebuffer (1bpp, 800×480 / 8 = 48 000 bytes) lives in AXI
//! SRAM when on hardware:
//! ```text
//! #[link_section = ".axisram"]
//! static FRAMEBUFFER: [u8; 48_000] = [0xFF; 48_000];
//! ```
//!
//! # Y-reversal
//!
//! The GDEM0397T81P gates are physically wired in reverse order.  When
//! `DATA_ENTRY_MODE = 0x01` (X+, Y−) is selected the controller scans from
//! the *bottom* gate, so logical row `y = 0` maps to RAM row `HEIGHT-1 = 479`
//! and logical row `y = 479` maps to RAM row `0`.  Every method that sets RAM
//! Y counters / ranges must apply `y_ram = HEIGHT - 1 - y`.

// Display geometry constants are u32 (matching embedded-graphics) but SSD1677
// registers are u16/u8.  These narrowing casts are safe because DISPLAY_WIDTH=800
// and DISPLAY_HEIGHT=480 both fit in u16, and BYTES_PER_ROW=100 fits in u8.
// Pixel coordinates from embedded-graphics are i32; after bounds checks they
// are always non-negative so casting to usize is safe.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::arithmetic_side_effects,
)]

use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal_async::{delay::DelayNs, spi::SpiDevice};

use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;

use platform::{DisplayDriver, EinkDisplay, RefreshMode};

use super::{DISPLAY_HEIGHT, DISPLAY_WIDTH};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Bytes per row: 800 pixels / 8 bits = 100 bytes.
pub const BYTES_PER_ROW: usize = DISPLAY_WIDTH as usize / 8;

/// Total framebuffer size for 1bpp (black/white) at 800×480.
pub const FRAMEBUFFER_SIZE_1BPP: usize = BYTES_PER_ROW * DISPLAY_HEIGHT as usize;

// ---------------------------------------------------------------------------
// Command enum
// ---------------------------------------------------------------------------

/// SSD1677 / SSD2677 command codes.
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Command {
    /// Driver output control — 3 data bytes.
    DriverOutputControl = 0x01,
    /// Gate driving voltage — 1 data byte.
    GateVoltage = 0x03,
    /// Source driving voltage — 3 data bytes.
    SourceVoltage = 0x04,
    /// Booster soft-start — 5 data bytes.
    BoosterSoftStart = 0x0C,
    /// Deep sleep — 1 data byte (0x01 = preserve RAM).
    DeepSleep = 0x10,
    /// Data entry mode — 1 data byte.
    DataEntryMode = 0x11,
    /// Software reset — 0 data bytes; wait ≥10 ms then poll BUSY.
    SoftReset = 0x12,
    /// Temperature sensor control — 1 data byte.
    TempSensorControl = 0x18,
    /// Write temperature value — 2 data bytes.
    WriteTempValue = 0x1A,
    /// Master activation — 0 data bytes; triggers panel update.
    MasterActivation = 0x20,
    /// Display update control 1 — 2 data bytes.
    DisplayUpdateCtrl1 = 0x21,
    /// Display update control 2 — 1 data byte (sequence flags).
    DisplayUpdateCtrl2 = 0x22,
    /// Write RAM (B/W) — pixel data; 0=black, 1=white, MSB-first.
    WriteRamBW = 0x24,
    /// Write RAM (Red / previous frame buffer).
    WriteRamRed = 0x26,
    /// Write VCOM register — 1 data byte.
    WriteVcom = 0x2C,
    /// Write LUT register — 112 data bytes.
    WriteLut = 0x32,
    /// Border waveform control — 1 data byte.
    BorderWaveform = 0x3C,
    /// Set RAM X start/end address — 4 data bytes.
    SetRamXRange = 0x44,
    /// Set RAM Y start/end address — 4 data bytes.
    SetRamYRange = 0x45,
    /// Auto-write B/W RAM with fill byte — 1 data byte; wait BUSY after.
    AutoWriteBwRam = 0x46,
    /// Auto-write Red RAM with fill byte — 1 data byte; wait BUSY after.
    AutoWriteRedRam = 0x47,
    /// Set RAM X address counter — 2 data bytes.
    SetRamXCounter = 0x4E,
    /// Set RAM Y address counter — 2 data bytes.
    SetRamYCounter = 0x4F,
    /// NOP.
    Nop = 0xFF,
}

// ---------------------------------------------------------------------------
// DisplayUpdateCtrl2 flag constants
// ---------------------------------------------------------------------------

/// Full refresh using OTP waveform (clock + analog + temp + LUT + display).
pub const UPDATE_FULL: u8 = 0xF7;
/// Full refresh fast path.
pub const UPDATE_FULL_FAST: u8 = 0xD7;
/// Partial refresh.
pub const UPDATE_PARTIAL: u8 = 0xFC;
/// Power-off sequence.
pub const UPDATE_POWER_OFF: u8 = 0x83;
/// Power on only.
pub const UPDATE_POWER_ON: u8 = 0xE0;

// ---------------------------------------------------------------------------
// Driver struct
// ---------------------------------------------------------------------------

/// SSD1677 / SSD2677 display driver.
///
/// Generic over:
/// - `SPI` — an async [`embedded_hal_async::spi::SpiDevice`] (manages CS).
/// - `DC`  — Data/Command [`embedded_hal::digital::OutputPin`].
/// - `RST` — Reset [`embedded_hal::digital::OutputPin`].
/// - `BUSY`— Busy [`embedded_hal::digital::InputPin`] (HIGH when busy).
/// - `DELAY` — [`embedded_hal_async::delay::DelayNs`] for timing.
///
/// On the STM32H743 target supply `embassy_time::Delay` for the `DELAY`
/// parameter.  In host tests supply `embedded_hal_mock::eh1::delay::NoopDelay`
/// or a custom implementation.
pub struct Ssd1677<SPI, DC, RST, BUSY, DELAY> {
    spi: SPI,
    dc: DC,
    rst: RST,
    busy: BUSY,
    delay: DELAY,
    refresh_mode: RefreshMode,
    partial_refresh_count: u8,
    /// 1bpp packed framebuffer (800×480 / 8 bytes = 48 000 bytes).
    ///
    /// `draw_iter` accumulates pixel writes here; a subsequent call to
    /// `refresh_full` / `refresh_partial` pushes the buffer to the
    /// controller's RAM and triggers a panel update.
    framebuffer: [u8; FRAMEBUFFER_SIZE_1BPP],
}

impl<SPI, DC, RST, BUSY, DELAY> Ssd1677<SPI, DC, RST, BUSY, DELAY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
    DELAY: DelayNs,
{
    /// Create a new driver instance.
    ///
    /// The framebuffer is initialised to all-white (0xFF).
    pub fn new(spi: SPI, dc: DC, rst: RST, busy: BUSY, delay: DELAY) -> Self {
        // SAFETY / note: On embedded (no_std) targets this struct is typically
        // placed in a `static` or on a task stack that lives in AXI SRAM, so
        // the large array size is expected and acceptable.
        #[allow(clippy::large_stack_arrays)]
        Self {
            spi,
            dc,
            rst,
            busy,
            delay,
            refresh_mode: RefreshMode::Full,
            partial_refresh_count: 0,
            framebuffer: [0xFF; FRAMEBUFFER_SIZE_1BPP],
        }
    }

    // -----------------------------------------------------------------------
    // Low-level SPI helpers
    // -----------------------------------------------------------------------

    /// Assert DC low (command mode) and send one command byte over SPI.
    ///
    /// `SpiDevice` handles CS assertion/de-assertion per transaction.
    async fn send_command(&mut self, cmd: Command) -> Result<(), DisplayError> {
        self.dc.set_low().map_err(|_| DisplayError::Gpio)?;
        self.spi
            .write(&[cmd as u8])
            .await
            .map_err(|_| DisplayError::Communication)
    }

    /// Assert DC high (data mode) and send bytes over SPI.
    async fn send_data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        if data.is_empty() {
            return Ok(());
        }
        self.dc.set_high().map_err(|_| DisplayError::Gpio)?;
        self.spi
            .write(data)
            .await
            .map_err(|_| DisplayError::Communication)
    }

    /// Send one command followed immediately by its data bytes.
    async fn cmd_data(&mut self, cmd: Command, data: &[u8]) -> Result<(), DisplayError> {
        self.send_command(cmd).await?;
        self.send_data(data).await
    }

    // -----------------------------------------------------------------------
    // BUSY polling
    // -----------------------------------------------------------------------

    /// Block until BUSY goes LOW (controller idle) or timeout expires.
    ///
    /// BUSY is active HIGH on SSD1677 / SSD2677.  Poll every 10 ms.
    /// After `MAX_BUSY_POLLS` attempts without BUSY going LOW the function
    /// returns `Err(DisplayError::Timeout)`.
    async fn wait_busy(&mut self) -> Result<(), DisplayError> {
        const POLL_MS: u32 = 10; // poll interval in milliseconds
        const MAX_BUSY_POLLS: u32 = 200; // 200 × 10 ms = 2 000 ms total

        for _ in 0..MAX_BUSY_POLLS {
            let is_busy = self.busy.is_high().map_err(|_| DisplayError::Gpio)?;
            if !is_busy {
                return Ok(());
            }
            self.delay.delay_ms(POLL_MS).await;
        }
        Err(DisplayError::Timeout)
    }

    // -----------------------------------------------------------------------
    // Reset
    // -----------------------------------------------------------------------

    /// Hardware reset sequence.
    ///
    /// RST HIGH 20 ms → LOW 2 ms → HIGH 20 ms.
    async fn hardware_reset(&mut self) -> Result<(), DisplayError> {
        self.rst.set_high().map_err(|_| DisplayError::Gpio)?;
        self.delay.delay_ms(20).await;
        self.rst.set_low().map_err(|_| DisplayError::Gpio)?;
        self.delay.delay_ms(2).await;
        self.rst.set_high().map_err(|_| DisplayError::Gpio)?;
        self.delay.delay_ms(20).await;
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Coordinate helpers (Y-reversal)
    // -----------------------------------------------------------------------

    /// Convert a logical Y coordinate to the controller RAM Y address.
    ///
    /// The GDEM0397T81P gates are physically reversed when `DATA_ENTRY_MODE`
    /// is `0x01` (X+, Y−).  RAM row 0 corresponds to the bottom physical
    /// gate, so `y_ram = HEIGHT - 1 - y_logical`.
    ///
    /// Returns `None` if `y_logical >= DISPLAY_HEIGHT` (out-of-bounds).
    #[inline]
    pub(crate) fn y_to_ram(y_logical: u16) -> Option<u16> {
        // DISPLAY_HEIGHT = 480 fits in u16 (max 65535). The cast is lossless.
        #[allow(clippy::arithmetic_side_effects)]
        if y_logical >= DISPLAY_HEIGHT as u16 {
            return None;
        }
        // The guard above ensures y_logical <= DISPLAY_HEIGHT - 1, so:
        //   (DISPLAY_HEIGHT as u16 - 1): 480 - 1 = 479, no underflow (DISPLAY_HEIGHT >= 1).
        //   (479) - y_logical: y_logical <= 479, so result >= 0, no underflow.
        #[allow(clippy::arithmetic_side_effects)]
        Some((DISPLAY_HEIGHT as u16 - 1) - y_logical)
    }

    // -----------------------------------------------------------------------
    // RAM window / counter helpers
    // -----------------------------------------------------------------------

    /// Set the RAM X address range.
    ///
    /// `x_byte_start` and `x_byte_end` are byte offsets (pixel_x / 8).
    async fn set_ram_x_range(
        &mut self,
        x_byte_start: u8,
        x_byte_end: u8,
    ) -> Result<(), DisplayError> {
        self.cmd_data(
            Command::SetRamXRange,
            &[x_byte_start, 0x00, x_byte_end, 0x00],
        )
        .await
    }

    /// Set the RAM Y address range using already-converted (reversed) Y values.
    async fn set_ram_y_range_raw(
        &mut self,
        y_start_ram: u16,
        y_end_ram: u16,
    ) -> Result<(), DisplayError> {
        self.cmd_data(
            Command::SetRamYRange,
            &[
                (y_start_ram & 0xFF) as u8,
                (y_start_ram >> 8) as u8,
                (y_end_ram & 0xFF) as u8,
                (y_end_ram >> 8) as u8,
            ],
        )
        .await
    }

    /// Set the RAM X address counter.
    async fn set_ram_x_counter(&mut self, x_byte: u8) -> Result<(), DisplayError> {
        self.cmd_data(Command::SetRamXCounter, &[x_byte, 0x00])
            .await
    }

    /// Set the RAM Y address counter (already reversed).
    async fn set_ram_y_counter_raw(&mut self, y_ram: u16) -> Result<(), DisplayError> {
        self.cmd_data(
            Command::SetRamYCounter,
            &[(y_ram & 0xFF) as u8, (y_ram >> 8) as u8],
        )
        .await
    }

    /// Configure full-screen RAM window and reset counters to top-left.
    ///
    /// With DATA_ENTRY_MODE 0x01 (X+, Y−):
    /// - X range: byte 0 to byte 99 (pixels 0–799)
    /// - Y range: RAM row 479 (logical y=0) down to RAM row 0 (logical y=479)
    /// - Counters: start at X=0, Y=479 (logical top-left)
    async fn set_full_window(&mut self) -> Result<(), DisplayError> {
        let x_end_byte = (BYTES_PER_ROW as u8).saturating_sub(1); // 99
                                                                  // Both inputs are compile-time constants within valid range; the
                                                                  // `ok_or` guard satisfies the bounds-checked API without runtime cost.
        let y_start_ram = Self::y_to_ram(0).ok_or(DisplayError::InvalidCoordinate)?; // 479 = 0x01DF
        // DISPLAY_HEIGHT as u16 - 1 = 479; DISPLAY_HEIGHT = 480 >= 1, no underflow.
        #[allow(clippy::arithmetic_side_effects)]
        let y_end_ram =
            Self::y_to_ram(DISPLAY_HEIGHT as u16 - 1).ok_or(DisplayError::InvalidCoordinate)?; // 0

        self.set_ram_x_range(0x00, x_end_byte).await?;
        self.set_ram_y_range_raw(y_start_ram, y_end_ram).await?;
        self.set_ram_x_counter(0x00).await?;
        self.set_ram_y_counter_raw(y_start_ram).await
    }

    // -----------------------------------------------------------------------
    // Framebuffer flush
    // -----------------------------------------------------------------------

    /// Push the internal framebuffer to the controller's B/W RAM via SPI.
    ///
    /// Sends in fixed 256-byte chunks to avoid borrow-checker conflicts.
    async fn flush_framebuffer(&mut self) -> Result<(), DisplayError> {
        self.send_command(Command::WriteRamBW).await?;

        const CHUNK: usize = 256;
        let mut offset = 0;
        while offset < FRAMEBUFFER_SIZE_1BPP {
            // offset < FRAMEBUFFER_SIZE_1BPP (loop guard), offset + CHUNK then capped
            // by .min(FRAMEBUFFER_SIZE_1BPP), so end <= FRAMEBUFFER_SIZE_1BPP.
            // offset + CHUNK: max is FRAMEBUFFER_SIZE_1BPP - 1 + 256 < usize::MAX.
            #[allow(clippy::arithmetic_side_effects)]
            let end = (offset + CHUNK).min(FRAMEBUFFER_SIZE_1BPP);
            let mut buf = [0u8; CHUNK];
            // end = (offset + CHUNK).min(FRAMEBUFFER_SIZE_1BPP) >= offset (since CHUNK > 0
            // and FRAMEBUFFER_SIZE_1BPP > offset), so end - offset >= 0.
            // end <= offset + CHUNK = offset + 256, so len <= 256 = CHUNK.
            #[allow(clippy::arithmetic_side_effects)]
            let len = end - offset;
            // offset <= end <= FRAMEBUFFER_SIZE_1BPP = len(self.framebuffer): valid slice.
            // len <= CHUNK = len(buf): valid slice.
            #[allow(clippy::indexing_slicing)]
            buf[..len].copy_from_slice(&self.framebuffer[offset..end]);
            #[allow(clippy::indexing_slicing)]
            self.send_data(&buf[..len]).await?;
            offset = end;
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Initialisation (public for testing / firmware)
    // -----------------------------------------------------------------------

    /// Full initialisation sequence for GDEM0397T81P with SSD1677.
    ///
    /// Must be called once after power-on or wake-from-sleep before any
    /// display operations.
    pub async fn init(&mut self) -> Result<(), DisplayError> {
        // 1. Hardware reset
        self.hardware_reset().await?;

        // 2. Software reset — wait 10 ms then poll BUSY
        self.send_command(Command::SoftReset).await?;
        self.delay.delay_ms(10).await;
        self.wait_busy().await?;

        // 3. Internal temperature sensor
        self.cmd_data(Command::TempSensorControl, &[0x80]).await?;

        // 4. Booster soft-start
        self.cmd_data(Command::BoosterSoftStart, &[0xAE, 0xC7, 0xC3, 0xC0, 0x80])
            .await?;

        // 5. Driver output: 480 gates (0x01DF), TB=1
        //    Gate = 479 = 0x01DF → low byte 0xDF, high byte 0x01; GD=0,SM=0,TB=1 → 0x02
        self.cmd_data(Command::DriverOutputControl, &[0xDF, 0x01, 0x02])
            .await?;

        // 6. Border waveform
        self.cmd_data(Command::BorderWaveform, &[0x01]).await?;

        // 7. Data entry mode: X+, Y- (matches Y-reversal)
        self.cmd_data(Command::DataEntryMode, &[0x01]).await?;

        // 8 & 9. RAM window + counters for full screen
        self.set_full_window().await?;

        // 10. Clear B/W RAM to white (0xF7 fill byte)
        self.cmd_data(Command::AutoWriteBwRam, &[0xF7]).await?;
        self.wait_busy().await?;

        // 11. Clear Red RAM to white
        self.cmd_data(Command::AutoWriteRedRam, &[0xF7]).await?;
        self.wait_busy().await?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// platform::DisplayDriver implementation
// ---------------------------------------------------------------------------

impl<SPI, DC, RST, BUSY, DELAY> DisplayDriver for Ssd1677<SPI, DC, RST, BUSY, DELAY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
    DELAY: DelayNs,
{
    type DriverError = DisplayError;

    fn spec(&self) -> platform::DisplayInfo {
        platform::DisplayInfo {
            width: DISPLAY_WIDTH,
            height: DISPLAY_HEIGHT,
        }
    }

    /// Copy a framebuffer into the internal buffer and push it to the
    /// controller's SRAM via SPI.
    ///
    /// `framebuffer` must be exactly [`FRAMEBUFFER_SIZE_1BPP`] bytes packed
    /// as 1bpp (8 pixels per byte, MSB-first, 1=white, 0=black).
    async fn update_buffer(&mut self, framebuffer: &[u8]) -> Result<(), Self::DriverError> {
        if framebuffer.len() != FRAMEBUFFER_SIZE_1BPP {
            return Err(DisplayError::InvalidBuffer);
        }
        self.framebuffer.copy_from_slice(framebuffer);
        self.set_full_window().await?;
        self.flush_framebuffer().await
    }

    /// Trigger a full panel refresh using the OTP waveform.
    ///
    /// Clears ghosting.  Takes ~3 000 ms.
    async fn refresh_full(&mut self) -> Result<(), Self::DriverError> {
        self.set_full_window().await?;
        self.flush_framebuffer().await?;

        // Bypass Red RAM (B/W only mode)
        self.cmd_data(Command::DisplayUpdateCtrl1, &[0x40, 0x00])
            .await?;

        // Full refresh via OTP LUT
        self.cmd_data(Command::DisplayUpdateCtrl2, &[UPDATE_FULL])
            .await?;
        self.send_command(Command::MasterActivation).await?;
        self.wait_busy().await?;

        self.partial_refresh_count = 0;
        Ok(())
    }

    /// Trigger a partial panel refresh (~300 ms).
    ///
    /// Uses `UPDATE_PARTIAL` (0xFC) sequence flag.
    async fn refresh_partial(&mut self) -> Result<(), Self::DriverError> {
        self.set_full_window().await?;
        self.flush_framebuffer().await?;

        self.cmd_data(Command::DisplayUpdateCtrl2, &[UPDATE_PARTIAL])
            .await?;
        self.send_command(Command::MasterActivation).await?;
        self.wait_busy().await?;

        self.partial_refresh_count = self.partial_refresh_count.saturating_add(1);
        Ok(())
    }

    /// Enter deep sleep (preserves RAM, ~1 µA).
    async fn sleep(&mut self) -> Result<(), Self::DriverError> {
        self.cmd_data(Command::DeepSleep, &[0x01]).await?;
        self.delay.delay_ms(100).await;
        Ok(())
    }

    /// Wake from deep sleep by running a hardware reset and re-init.
    async fn wake(&mut self) -> Result<(), Self::DriverError> {
        self.hardware_reset().await?;
        self.init().await
    }

    /// Poll BUSY until the controller signals idle.
    async fn wait_ready(&mut self) -> Result<(), Self::DriverError> {
        self.wait_busy().await
    }
}

// ---------------------------------------------------------------------------
// DrawTarget — pixel accumulation into the framebuffer
// ---------------------------------------------------------------------------

impl<SPI, DC, RST, BUSY, DELAY> DrawTarget for Ssd1677<SPI, DC, RST, BUSY, DELAY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
    DELAY: DelayNs,
{
    type Color = BinaryColor;
    type Error = DisplayError;

    /// Write pixels into the internal 1bpp framebuffer.
    ///
    /// Format: each byte holds 8 pixels, MSB-first.
    /// `BinaryColor::Off` (white) → bit = 1; `BinaryColor::On` (black) → bit = 0.
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x < 0
                || point.y < 0
                || point.x >= DISPLAY_WIDTH as i32
                || point.y >= DISPLAY_HEIGHT as i32
            {
                continue;
            }
            let x = point.x as usize;
            let y = point.y as usize;
            // Bounds guard above: x < DISPLAY_WIDTH (800), y < DISPLAY_HEIGHT (480).
            //   y * DISPLAY_WIDTH: max = 479 * 800 = 383200 < usize::MAX.
            //   + x: max total = 479 * 800 + 799 = 384399 < usize::MAX.
            #[allow(clippy::arithmetic_side_effects)]
            let pixel_index = y * DISPLAY_WIDTH as usize + x;
            let byte_index = pixel_index / 8;
            // pixel_index % 8 is in [0, 7]; 7 - (0..=7) is in [0, 7]. No underflow.
            #[allow(clippy::arithmetic_side_effects)]
            // MSB-first: pixel 0 of a row is in bit 7 of byte 0.
            let bit_shift = 7 - (pixel_index % 8);
            match color {
                BinaryColor::Off => {
                    // white → set bit
                    // byte_index = pixel_index / 8 <= (DISPLAY_WIDTH*DISPLAY_HEIGHT - 1) / 8
                    //   = (800*480 - 1) / 8 = 47999, which is < FRAMEBUFFER_SIZE_1BPP (48000).
                    #[allow(clippy::indexing_slicing)]
                    { self.framebuffer[byte_index] |= 1 << bit_shift; }
                }
                BinaryColor::On => {
                    // black → clear bit (same bound as above).
                    #[allow(clippy::indexing_slicing)]
                    { self.framebuffer[byte_index] &= !(1 << bit_shift); }
                }
            }
        }
        Ok(())
    }
}

impl<SPI, DC, RST, BUSY, DELAY> OriginDimensions for Ssd1677<SPI, DC, RST, BUSY, DELAY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
    DELAY: DelayNs,
{
    fn size(&self) -> Size {
        Size::new(DISPLAY_WIDTH, DISPLAY_HEIGHT)
    }
}

// ---------------------------------------------------------------------------
// EinkDisplay
// ---------------------------------------------------------------------------

impl<SPI, DC, RST, BUSY, DELAY> EinkDisplay for Ssd1677<SPI, DC, RST, BUSY, DELAY>
where
    SPI: SpiDevice,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
    DELAY: DelayNs,
{
    fn refresh_mode(&self) -> RefreshMode {
        self.refresh_mode
    }

    fn set_refresh_mode(&mut self, mode: RefreshMode) {
        self.refresh_mode = mode;
    }

    fn temperature(&self) -> Option<i8> {
        // Reading the on-chip temperature ADC register is not yet implemented.
        None
    }
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors returned by the SSD1677 driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayError {
    /// SPI communication error.
    Communication,
    /// GPIO operation error.
    Gpio,
    /// Display busy — BUSY pin did not go LOW within the timeout.
    Busy,
    /// BUSY polling timed out.
    Timeout,
    /// Invalid state (e.g. calling refresh without init).
    InvalidState,
    /// Caller supplied a framebuffer with the wrong number of bytes.
    InvalidBuffer,
    /// A coordinate was outside the valid display area.
    InvalidCoordinate,
    /// Operation not supported.
    Unsupported,
}

impl core::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Communication => write!(f, "SPI communication error"),
            Self::Gpio => write!(f, "GPIO error"),
            Self::Busy => write!(f, "Display is busy"),
            Self::Timeout => write!(f, "Operation timed out"),
            Self::InvalidState => write!(f, "Invalid display state"),
            Self::InvalidBuffer => write!(f, "Invalid buffer size"),
            Self::InvalidCoordinate => write!(f, "Coordinate out of bounds"),
            Self::Unsupported => write!(f, "Unsupported operation"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DisplayError {}

// ---------------------------------------------------------------------------
// Convenience type alias for the hardware binary
// ---------------------------------------------------------------------------

/// Type alias used by `firmware/src/main.rs` on the STM32H7 target.
///
/// On hardware, the SPI device is typically wrapped by
/// `embassy_embedded_hal::shared_bus::asynch::spi::SpiDevice`.
/// The delay source is `embassy_time::Delay`.
#[cfg(feature = "hardware")]
pub type Ssd1677Display<SPI, DC, RST, BUSY, DELAY> = Ssd1677<SPI, DC, RST, BUSY, DELAY>;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    use embedded_hal_mock::eh1::delay::NoopDelay;
    use embedded_hal_mock::eh1::digital::{
        Mock as PinMock, State as PinState, Transaction as PinTransaction,
    };
    use embedded_hal_mock::eh1::spi::{Mock as SpiMock, Transaction as SpiTransaction};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Alias type for test driver instances.
    type TestDriver = Ssd1677<SpiMock<u8>, PinMock, PinMock, PinMock, NoopDelay>;

    /// Encoded `cmd` byte for SPI write expectation.
    fn cmd_byte(c: Command) -> u8 {
        c as u8
    }

    /// Build the three SPI expectations that correspond to one `spi.write(&data)` call
    /// via the `SpiDevice` trait:
    ///   TransactionStart + Write(data) + TransactionEnd
    ///
    /// The `embedded-hal-async` `SpiDevice::write()` helper calls
    /// `transaction(&mut [Operation::Write(buf)])` which the mock checks as:
    ///   1. `TransactionStart`
    ///   2. `Write(data)`
    ///   3. `TransactionEnd`
    fn spi_device_write(data: &[u8]) -> [SpiTransaction<u8>; 3] {
        [
            SpiTransaction::transaction_start(),
            SpiTransaction::write_vec(data.to_vec()),
            SpiTransaction::transaction_end(),
        ]
    }

    /// A BUSY pin that returns HIGH (busy) for `busy_count` polls then LOW once.
    fn busy_pin_sequence(busy_count: usize) -> PinMock {
        let mut txns = vec![];
        for _ in 0..busy_count {
            txns.push(PinTransaction::get(PinState::High));
        }
        txns.push(PinTransaction::get(PinState::Low));
        PinMock::new(&txns)
    }

    /// Create a DC pin mock from a slice of transactions.
    fn dc_pin(transactions: &[PinTransaction]) -> PinMock {
        PinMock::new(transactions)
    }

    /// RST pin mock that expects the hardware_reset sequence:
    /// set_high, set_low, set_high.
    fn rst_reset_sequence() -> PinMock {
        PinMock::new(&[
            PinTransaction::set(PinState::High),
            PinTransaction::set(PinState::Low),
            PinTransaction::set(PinState::High),
        ])
    }

    /// Create a minimal (no-expectation) pin mock for unused pins.
    fn idle_pin() -> PinMock {
        PinMock::new(&[])
    }

    // -----------------------------------------------------------------------
    // Test: Y-reversal mapping
    // -----------------------------------------------------------------------

    /// `test_y_reversal` — logical (0,0) must map to RAM y=479,
    /// logical (0,479) must map to RAM y=0.
    #[test]
    fn test_y_reversal() {
        // Logical y=0 → RAM y = HEIGHT-1 = 479
        assert_eq!(
            TestDriver::y_to_ram(0).unwrap(),
            479,
            "logical y=0 must map to RAM row 479 (bottom gate)"
        );
        // Logical y=479 → RAM y = 0
        assert_eq!(
            TestDriver::y_to_ram(479).unwrap(),
            0,
            "logical y=479 must map to RAM row 0 (top gate)"
        );
        // Logical y=240 (mid) → RAM y = 239
        assert_eq!(TestDriver::y_to_ram(240).unwrap(), 239);
        // Logical y=1 → RAM y = 478
        assert_eq!(TestDriver::y_to_ram(1).unwrap(), 478);
        // Out-of-bounds must return None
        assert_eq!(TestDriver::y_to_ram(480), None);
        assert_eq!(TestDriver::y_to_ram(u16::MAX), None);
    }

    // -----------------------------------------------------------------------
    // Test: RAM window coordinate encoding
    // -----------------------------------------------------------------------

    /// `test_set_window_coordinates` — verify the exact bytes for X and Y
    /// range commands for both full-screen and a sub-region.
    #[test]
    fn test_set_window_coordinates() {
        // Full-screen X byte range: 0..=99 (800/8-1=99)
        let x_end_byte = (BYTES_PER_ROW as u8).saturating_sub(1);
        assert_eq!(x_end_byte, 99, "800/8 - 1 = 99");

        // X range data: [x_start_L, x_start_H, x_end_L, x_end_H]
        let x_range_full = [0x00u8, 0x00, x_end_byte, 0x00];
        assert_eq!(x_range_full, [0x00, 0x00, 0x63, 0x00]);

        // Full-screen Y range (reversed):
        //   y_start_ram = 479 = 0x01DF  → [0xDF, 0x01]
        //   y_end_ram   = 0             → [0x00, 0x00]
        let y_start_ram = TestDriver::y_to_ram(0).unwrap(); // 479
        let y_end_ram = TestDriver::y_to_ram(DISPLAY_HEIGHT as u16 - 1).unwrap(); // 0
        assert_eq!(y_start_ram, 479);
        assert_eq!(y_end_ram, 0);

        let y_range_full = [
            (y_start_ram & 0xFF) as u8, // 0xDF
            (y_start_ram >> 8) as u8,   // 0x01
            (y_end_ram & 0xFF) as u8,   // 0x00
            (y_end_ram >> 8) as u8,     // 0x00
        ];
        assert_eq!(y_range_full, [0xDF, 0x01, 0x00, 0x00]);

        // Partial region: logical y=10..=19 → RAM y=469..=460
        let py_start = 10u16;
        let py_end = 19u16;
        let y_start_ram_partial = TestDriver::y_to_ram(py_start).unwrap();
        let y_end_ram_partial = TestDriver::y_to_ram(py_end).unwrap();
        assert_eq!(y_start_ram_partial, 469);
        assert_eq!(y_end_ram_partial, 460);

        // Partial region X: logical pixel x=8..=15 → byte 1..=1
        let px_start = 8u16;
        let px_end = 15u16;
        assert_eq!((px_start / 8) as u8, 1);
        assert_eq!((px_end / 8) as u8, 1);
    }

    // -----------------------------------------------------------------------
    // Test: deep sleep command
    // -----------------------------------------------------------------------

    /// `test_deep_sleep_command` — verify that `sleep()` emits exactly:
    /// - DC low + SPI write [0x10]  (DeepSleep command)
    /// - DC high + SPI write [0x01] (data: preserve RAM)
    ///
    /// Each `spi.write()` via `SpiDevice` wraps in TransactionStart + Write + TransactionEnd.
    #[tokio::test]
    async fn test_deep_sleep_command() {
        let spi_expectations: Vec<SpiTransaction<u8>> = [
            // DeepSleep cmd
            &spi_device_write(&[cmd_byte(Command::DeepSleep)]) as &[_],
            // data: 0x01
            &spi_device_write(&[0x01]),
        ]
        .iter()
        .flat_map(|s| s.iter().cloned())
        .collect();

        let dc_expectations = [
            PinTransaction::set(PinState::Low),  // DC low for command
            PinTransaction::set(PinState::High), // DC high for data
        ];

        let mut spi = SpiMock::new(&spi_expectations);
        let mut dc = dc_pin(&dc_expectations);
        let mut rst = idle_pin();
        let mut busy = idle_pin();

        let mut drv = Ssd1677::new(
            spi.clone(),
            dc.clone(),
            rst.clone(),
            busy.clone(),
            NoopDelay,
        );
        drv.sleep().await.unwrap();

        spi.done();
        dc.done();
        rst.done();
        busy.done();
    }

    // -----------------------------------------------------------------------
    // Test: full vs partial refresh flag values
    // -----------------------------------------------------------------------

    /// `test_partial_refresh_sequence` — verify correct DISPLAY_UPDATE_CTRL2
    /// flag bytes: 0xFC for partial, 0xF7 for full.
    #[test]
    fn test_refresh_flag_values() {
        assert_eq!(UPDATE_FULL, 0xF7, "full refresh flag must be 0xF7");
        assert_eq!(UPDATE_PARTIAL, 0xFC, "partial refresh flag must be 0xFC");
        assert_eq!(UPDATE_FULL_FAST, 0xD7, "fast refresh flag must be 0xD7");
        assert_eq!(UPDATE_POWER_OFF, 0x83, "power off flag must be 0x83");
    }

    // -----------------------------------------------------------------------
    // Test: busy polling waits for LOW
    // -----------------------------------------------------------------------

    /// `test_busy_polling` — driver must keep polling while BUSY is HIGH and
    /// succeed once BUSY goes LOW.
    #[tokio::test]
    async fn test_busy_polling() {
        // BUSY: HIGH × 3, then LOW
        let busy = busy_pin_sequence(3);

        // Keep clones to call .done() after the driver is consumed.
        let mut spi_handle = SpiMock::new(&[]);
        let mut dc_handle = idle_pin();
        let mut rst_handle = idle_pin();
        let mut busy_handle = busy.clone();

        let mut drv = Ssd1677::new(
            spi_handle.clone(),
            dc_handle.clone(),
            rst_handle.clone(),
            busy,
            NoopDelay,
        );

        // Should resolve without error
        drv.wait_busy().await.expect("busy polling should succeed");

        // Verify all expectations consumed
        spi_handle.done();
        dc_handle.done();
        rst_handle.done();
        busy_handle.done();
    }

    // -----------------------------------------------------------------------
    // Test: clear screen uses AutoWrite commands
    // -----------------------------------------------------------------------

    /// `test_clear_screen` — verify `AutoWriteBwRam` (0x46) and
    /// `AutoWriteRedRam` (0x47) with fill byte 0xF7 are used for clearing.
    #[test]
    fn test_clear_screen_command_codes() {
        assert_eq!(
            Command::AutoWriteBwRam as u8,
            0x46,
            "AUTO_WRITE_BW_RAM must be 0x46"
        );
        assert_eq!(
            Command::AutoWriteRedRam as u8,
            0x47,
            "AUTO_WRITE_RED_RAM must be 0x47"
        );
    }

    // -----------------------------------------------------------------------
    // Test: full refresh pixel data
    // -----------------------------------------------------------------------

    /// `test_full_refresh_pixel_data` — after drawing a single black pixel at
    /// (0, 0) with `draw_iter`, the first byte of the framebuffer must have
    /// bit 7 clear (black = 0 in 1bpp MSB-first).
    #[test]
    fn test_full_refresh_pixel_data() {
        let mut spi_h = SpiMock::new(&[]);
        let mut dc_h = idle_pin();
        let mut rst_h = idle_pin();
        let mut busy_h = idle_pin();

        let mut drv = Ssd1677::new(
            spi_h.clone(),
            dc_h.clone(),
            rst_h.clone(),
            busy_h.clone(),
            NoopDelay,
        );

        // Framebuffer starts at all-white (0xFF)
        assert_eq!(
            drv.framebuffer[0], 0xFF,
            "framebuffer should be initialised to 0xFF (all white)"
        );

        // Draw a single black pixel at (0, 0)
        drv.draw_iter(core::iter::once(Pixel(Point::new(0, 0), BinaryColor::On)))
            .unwrap();

        // Bit 7 of byte 0 should now be 0 (black)
        assert_eq!(
            drv.framebuffer[0] & 0x80,
            0,
            "pixel (0,0) black → bit 7 of byte 0 must be 0"
        );
        // Remaining 7 bits of byte 0 should still be white
        assert_eq!(
            drv.framebuffer[0] & 0x7F,
            0x7F,
            "other 7 bits of byte 0 must remain white"
        );

        // Restore to white
        drv.draw_iter(core::iter::once(Pixel(Point::new(0, 0), BinaryColor::Off)))
            .unwrap();
        assert_eq!(
            drv.framebuffer[0], 0xFF,
            "after drawing white at (0,0), byte 0 must return to 0xFF"
        );

        // Draw black at (7, 0) — last pixel in the first byte (bit 0)
        drv.draw_iter(core::iter::once(Pixel(Point::new(7, 0), BinaryColor::On)))
            .unwrap();
        assert_eq!(
            drv.framebuffer[0] & 0x01,
            0,
            "pixel (7,0) black → bit 0 of byte 0 must be 0"
        );

        // Draw black at (8, 0) — first pixel of second byte (bit 7 of byte 1)
        drv.draw_iter(core::iter::once(Pixel(Point::new(8, 0), BinaryColor::On)))
            .unwrap();
        assert_eq!(
            drv.framebuffer[1] & 0x80,
            0,
            "pixel (8,0) black → bit 7 of byte 1 must be 0"
        );

        spi_h.done();
        dc_h.done();
        rst_h.done();
        busy_h.done();
    }

    // -----------------------------------------------------------------------
    // Test: init sequence (byte-level SPI verification)
    // -----------------------------------------------------------------------

    /// `test_init_sequence` — drive `init()` and verify every SPI byte matches
    /// the documented GDEM0397T81P initialisation sequence.
    ///
    /// Each driver `spi.write(&data)` call becomes three mock expectations:
    ///   TransactionStart + Write(data) + TransactionEnd
    ///
    /// Expected command sequence:
    ///  1. RST HIGH → LOW → HIGH  (hardware reset — no SPI bytes)
    ///  2. SoftReset (0x12)        then 10 ms delay, then poll BUSY once
    ///  3. TempSensorControl (0x18), data=[0x80]
    ///  4. BoosterSoftStart  (0x0C), data=[0xAE,0xC7,0xC3,0xC0,0x80]
    ///  5. DriverOutputControl (0x01), data=[0xDF,0x01,0x02]
    ///  6. BorderWaveform (0x3C), data=[0x01]
    ///  7. DataEntryMode (0x11), data=[0x01]
    ///  8. SetRamXRange (0x44), data=[0x00,0x00,0x63,0x00]
    ///  9. SetRamYRange (0x45), data=[0xDF,0x01,0x00,0x00]
    /// 10. SetRamXCounter (0x4E), data=[0x00,0x00]
    /// 11. SetRamYCounter (0x4F), data=[0xDF,0x01]
    /// 12. AutoWriteBwRam (0x46), data=[0xF7], wait BUSY
    /// 13. AutoWriteRedRam (0x47), data=[0xF7], wait BUSY
    #[tokio::test]
    async fn test_init_sequence() {
        // Build SPI expectations directly — no closures to avoid borrow conflicts.
        // Each spi.write() via SpiDevice wraps as: TransactionStart + Write + TransactionEnd.
        let spi_expectations: Vec<SpiTransaction<u8>> = [
            // 2. SoftReset cmd only
            &spi_device_write(&[cmd_byte(Command::SoftReset)]) as &[_],
            // 3. TempSensorControl cmd + data
            &spi_device_write(&[cmd_byte(Command::TempSensorControl)]),
            &spi_device_write(&[0x80]),
            // 4. BoosterSoftStart cmd + data
            &spi_device_write(&[cmd_byte(Command::BoosterSoftStart)]),
            &spi_device_write(&[0xAE, 0xC7, 0xC3, 0xC0, 0x80]),
            // 5. DriverOutputControl cmd + data
            &spi_device_write(&[cmd_byte(Command::DriverOutputControl)]),
            &spi_device_write(&[0xDF, 0x01, 0x02]),
            // 6. BorderWaveform cmd + data
            &spi_device_write(&[cmd_byte(Command::BorderWaveform)]),
            &spi_device_write(&[0x01]),
            // 7. DataEntryMode cmd + data
            &spi_device_write(&[cmd_byte(Command::DataEntryMode)]),
            &spi_device_write(&[0x01]),
            // 8. SetRamXRange cmd + data  (set_full_window)
            &spi_device_write(&[cmd_byte(Command::SetRamXRange)]),
            &spi_device_write(&[0x00, 0x00, 0x63, 0x00]),
            // 9. SetRamYRange cmd + data
            &spi_device_write(&[cmd_byte(Command::SetRamYRange)]),
            &spi_device_write(&[0xDF, 0x01, 0x00, 0x00]),
            // 10. SetRamXCounter cmd + data
            &spi_device_write(&[cmd_byte(Command::SetRamXCounter)]),
            &spi_device_write(&[0x00, 0x00]),
            // 11. SetRamYCounter cmd + data
            &spi_device_write(&[cmd_byte(Command::SetRamYCounter)]),
            &spi_device_write(&[0xDF, 0x01]),
            // 12. AutoWriteBwRam cmd + data
            &spi_device_write(&[cmd_byte(Command::AutoWriteBwRam)]),
            &spi_device_write(&[0xF7]),
            // 13. AutoWriteRedRam cmd + data
            &spi_device_write(&[cmd_byte(Command::AutoWriteRedRam)]),
            &spi_device_write(&[0xF7]),
        ]
        .iter()
        .flat_map(|slice| slice.iter().cloned())
        .collect();

        // DC pin transitions:
        //   SoftReset:           1 × set_low  (command only, no data)
        //   Then for each cmd+data pair: set_low (cmd) + set_high (data)
        //   11 cmd+data pairs: TempSensor, Booster, DriverOutput, Border,
        //                      DataEntry, XRange, YRange, XCounter, YCounter,
        //                      AutoWriteBW, AutoWriteRed
        //   Total: 1 + 11*2 = 23 DC transitions
        let dc_expectations: Vec<PinTransaction> = {
            let mut v = vec![PinTransaction::set(PinState::Low)]; // SoftReset cmd
            for _ in 0..11 {
                v.push(PinTransaction::set(PinState::Low)); // cmd
                v.push(PinTransaction::set(PinState::High)); // data
            }
            v
        };

        // BUSY: polled 3 times total:
        //   - after SoftReset wait_busy()
        //   - after AutoWriteBwRam wait_busy()
        //   - after AutoWriteRedRam wait_busy()
        // Each poll: 1×HIGH then 1×LOW
        let busy_expectations: Vec<PinTransaction> = (0..3)
            .flat_map(|_| {
                [
                    PinTransaction::get(PinState::High),
                    PinTransaction::get(PinState::Low),
                ]
            })
            .collect();

        let mut spi = SpiMock::new(&spi_expectations);
        let mut dc = dc_pin(&dc_expectations);
        // RST: hardware_reset sequence (HIGH → LOW → HIGH)
        let mut rst = rst_reset_sequence();
        let mut busy = PinMock::new(&busy_expectations);

        let mut drv = Ssd1677::new(
            spi.clone(),
            dc.clone(),
            rst.clone(),
            busy.clone(),
            NoopDelay,
        );
        drv.init().await.expect("init() must succeed");

        spi.done();
        dc.done();
        rst.done();
        busy.done();
    }

    // -----------------------------------------------------------------------
    // Test: DisplayUpdateCtrl1 bypass-red-RAM encoding
    // -----------------------------------------------------------------------

    /// Verify that `DisplayUpdateCtrl1` data bytes [0x40, 0x00] correctly
    /// encode "bypass red RAM" (B/W-only mode).
    #[test]
    fn test_display_update_ctrl1_bw_only() {
        let data = [0x40u8, 0x00];
        assert_eq!(data[0], 0x40, "byte 0 must be 0x40 (bypass red RAM)");
        assert_eq!(data[1], 0x00, "byte 1 must be 0x00");
        assert_eq!(
            Command::DisplayUpdateCtrl1 as u8,
            0x21,
            "DisplayUpdateCtrl1 must be command 0x21"
        );
    }

    // -----------------------------------------------------------------------
    // Test: framebuffer size constants
    // -----------------------------------------------------------------------

    #[test]
    fn test_framebuffer_size() {
        // 1bpp: 800×480 / 8 = 48 000 bytes
        assert_eq!(
            FRAMEBUFFER_SIZE_1BPP, 48_000,
            "1bpp framebuffer must be 48 000 bytes"
        );
        assert_eq!(BYTES_PER_ROW, 100, "bytes per row must be 100 (800/8)");
    }

    // -----------------------------------------------------------------------
    // Test: display error formatting
    // -----------------------------------------------------------------------

    #[test]
    fn test_display_error_format() {
        use std::string::ToString;
        assert_eq!(
            DisplayError::Communication.to_string(),
            "SPI communication error"
        );
        assert_eq!(DisplayError::Gpio.to_string(), "GPIO error");
        assert_eq!(DisplayError::Busy.to_string(), "Display is busy");
        assert_eq!(DisplayError::Timeout.to_string(), "Operation timed out");
        assert_eq!(
            DisplayError::InvalidBuffer.to_string(),
            "Invalid buffer size"
        );
        assert_eq!(
            DisplayError::InvalidState.to_string(),
            "Invalid display state"
        );
        assert_eq!(
            DisplayError::Unsupported.to_string(),
            "Unsupported operation"
        );
    }

    // -----------------------------------------------------------------------
    // Test: driver spec / dimensions
    // -----------------------------------------------------------------------

    #[test]
    fn test_driver_spec() {
        let mut spi_h = SpiMock::new(&[]);
        let mut dc_h = idle_pin();
        let mut rst_h = idle_pin();
        let mut busy_h = idle_pin();

        let drv = Ssd1677::new(
            spi_h.clone(),
            dc_h.clone(),
            rst_h.clone(),
            busy_h.clone(),
            NoopDelay,
        );

        let spec = drv.spec();
        assert_eq!(spec.width, 800);
        assert_eq!(spec.height, 480);

        let dims = drv.dimensions();
        assert_eq!(dims, embedded_graphics::prelude::Size::new(800, 480));

        spi_h.done();
        dc_h.done();
        rst_h.done();
        busy_h.done();
    }

    // -----------------------------------------------------------------------
    // Test: out-of-bounds pixels are silently ignored
    // -----------------------------------------------------------------------

    #[test]
    fn test_out_of_bounds_pixels_ignored() {
        let mut spi_h = SpiMock::new(&[]);
        let mut dc_h = idle_pin();
        let mut rst_h = idle_pin();
        let mut busy_h = idle_pin();

        let mut drv = Ssd1677::new(
            spi_h.clone(),
            dc_h.clone(),
            rst_h.clone(),
            busy_h.clone(),
            NoopDelay,
        );

        // Draw at negative coords and out-of-range — should not panic or error
        drv.draw_iter(
            [
                Pixel(Point::new(-1, 0), BinaryColor::On),
                Pixel(Point::new(0, -1), BinaryColor::On),
                Pixel(Point::new(800, 0), BinaryColor::On),
                Pixel(Point::new(0, 480), BinaryColor::On),
            ]
            .iter()
            .copied(),
        )
        .unwrap();

        // Framebuffer must still be all-white (no valid pixel written)
        assert!(
            drv.framebuffer.iter().all(|&b| b == 0xFF),
            "out-of-bounds pixels must not modify framebuffer"
        );

        spi_h.done();
        dc_h.done();
        rst_h.done();
        busy_h.done();
    }

    // -----------------------------------------------------------------------
    // Test: update_buffer rejects wrong-size slice
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_update_buffer_wrong_size() {
        let mut spi_h = SpiMock::new(&[]);
        let mut dc_h = idle_pin();
        let mut rst_h = idle_pin();
        let mut busy_h = idle_pin();

        let mut drv = Ssd1677::new(
            spi_h.clone(),
            dc_h.clone(),
            rst_h.clone(),
            busy_h.clone(),
            NoopDelay,
        );

        let short_buf = [0u8; 100];
        let result = drv.update_buffer(&short_buf).await;
        assert_eq!(
            result,
            Err(DisplayError::InvalidBuffer),
            "wrong-size buffer must return InvalidBuffer"
        );

        spi_h.done();
        dc_h.done();
        rst_h.done();
        busy_h.done();
    }

    // -----------------------------------------------------------------------
    // Test: wait_busy timeout (FAILING — see Step 3 comment below)
    // -----------------------------------------------------------------------

    /// `test_wait_busy_timeout` — `wait_busy` must return `Err(DisplayError::Timeout)`
    /// after MAX_BUSY_POLLS polls without BUSY going LOW.
    ///
    /// **Expected failure mode (before fix):** The current code uses MAX_POLLS = 500.
    /// With only 200 HIGH transactions in the mock, the 201st call to `is_high()`
    /// panics with "no expectation for pin::is_high call" instead of returning
    /// `Err(Timeout)`.  After fix (MAX_BUSY_POLLS = 200), the loop exhausts all 200
    /// HIGH reads and returns `Err(DisplayError::Timeout)` cleanly.
    #[tokio::test]
    async fn test_wait_busy_timeout() {
        // 200 × HIGH, no trailing LOW — pin never deasserts.
        let busy_txns: Vec<PinTransaction> = (0..200)
            .map(|_| PinTransaction::get(PinState::High))
            .collect();

        let mut spi_handle = SpiMock::new(&[]);
        let mut dc_handle = idle_pin();
        let mut rst_handle = idle_pin();
        let mut busy_handle = PinMock::new(&busy_txns);

        let mut drv = Ssd1677::new(
            spi_handle.clone(),
            dc_handle.clone(),
            rst_handle.clone(),
            busy_handle.clone(),
            NoopDelay,
        );

        let result = drv.wait_busy().await;
        assert_eq!(
            result,
            Err(DisplayError::Timeout),
            "wait_busy must return Timeout when BUSY never deasserts"
        );

        spi_handle.done();
        dc_handle.done();
        rst_handle.done();
        busy_handle.done();
    }

    // -----------------------------------------------------------------------
    // Test: y_to_ram out-of-bounds returns None (FAILING — see Step 3 comment)
    // -----------------------------------------------------------------------

    /// `test_y_to_ram_out_of_bounds` — `y_to_ram` must return `None` for y >= DISPLAY_HEIGHT.
    ///
    /// **Expected failure mode (before fix):** `y_to_ram` currently returns `u16`,
    /// not `Option<u16>`, so this test will not compile until the signature is changed.
    #[test]
    fn test_y_to_ram_out_of_bounds() {
        // One past the last valid row — must return None.
        assert_eq!(
            TestDriver::y_to_ram(DISPLAY_HEIGHT as u16),
            None,
            "y = DISPLAY_HEIGHT (480) is out-of-bounds and must return None"
        );
        // Last valid row — must return Some(0).
        assert_eq!(
            TestDriver::y_to_ram(479),
            Some(0),
            "logical y=479 must map to RAM row 0"
        );
        // First valid row — must return Some(479).
        assert_eq!(
            TestDriver::y_to_ram(0),
            Some(479),
            "logical y=0 must map to RAM row 479"
        );
    }

    // -----------------------------------------------------------------------
    // Test: wait_busy propagates GPIO errors (test C)
    // -----------------------------------------------------------------------

    /// `test_wait_busy_error_propagated` — a GPIO read error must cause `wait_busy`
    /// to return `Err(DisplayError::Gpio)` immediately.
    ///
    /// **Status:** This test PASSES with current code because `map_err(|_| DisplayError::Gpio)`
    /// is already in place.  It is included to lock in that behaviour.
    #[tokio::test]
    async fn test_wait_busy_error_propagated() {
        use embedded_hal_mock::eh1::MockError;
        use std::io::ErrorKind;

        // One failing read — simulates a GPIO bus error.
        let busy_txns =
            [PinTransaction::get(PinState::High)
                .with_error(MockError::Io(ErrorKind::NotConnected))];

        let mut spi_handle = SpiMock::new(&[]);
        let mut dc_handle = idle_pin();
        let mut rst_handle = idle_pin();
        let mut busy_handle = PinMock::new(&busy_txns);

        let mut drv = Ssd1677::new(
            spi_handle.clone(),
            dc_handle.clone(),
            rst_handle.clone(),
            busy_handle.clone(),
            NoopDelay,
        );

        let result = drv.wait_busy().await;
        assert_eq!(
            result,
            Err(DisplayError::Gpio),
            "a GPIO error during busy polling must propagate as DisplayError::Gpio"
        );

        spi_handle.done();
        dc_handle.done();
        rst_handle.done();
        busy_handle.done();
    }

    // -----------------------------------------------------------------------
    // Test: EinkDisplay trait implementation
    // -----------------------------------------------------------------------

    #[test]
    fn test_eink_display_trait() {
        let mut spi_h = SpiMock::new(&[]);
        let mut dc_h = idle_pin();
        let mut rst_h = idle_pin();
        let mut busy_h = idle_pin();

        let mut drv = Ssd1677::new(
            spi_h.clone(),
            dc_h.clone(),
            rst_h.clone(),
            busy_h.clone(),
            NoopDelay,
        );

        assert_eq!(drv.refresh_mode(), RefreshMode::Full);
        drv.set_refresh_mode(RefreshMode::Partial);
        assert_eq!(drv.refresh_mode(), RefreshMode::Partial);
        drv.set_refresh_mode(RefreshMode::Fast);
        assert_eq!(drv.refresh_mode(), RefreshMode::Fast);
        // Temperature not implemented on hardware
        assert_eq!(drv.temperature(), None);

        spi_h.done();
        dc_h.done();
        rst_h.done();
        busy_h.done();
    }
}
