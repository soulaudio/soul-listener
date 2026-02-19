//! SSD1677 Hardware Driver for STM32H7
//!
//! This driver implements the SSD1677 e-ink controller interface for the
//! GDEM0397T81P (800x480) display panel using Embassy async HAL.
//!
//! # Pin Configuration
//!
//! - PA5 (SPI1_SCK)  -> SCK
//! - PA7 (SPI1_MOSI) -> DIN
//! - PB0 (GPIO)      -> DC (Data/Command)
//! - PB1 (GPIO)      -> CS (Chip Select)
//! - PB2 (GPIO)      -> RST (Reset)
//! - PE3 (GPIO)      -> BUSY
//!
//! # Memory Layout
//!
//! Framebuffer is stored in AXI SRAM (96 KB):
//! ```text
//! #[link_section = ".axisram"]
//! static FRAMEBUFFER: [u8; 96000] = [0xFF; 96000];
//! ```

#[cfg(feature = "hardware")]
use embassy_stm32::gpio::{Input, Output};
#[cfg(feature = "hardware")]
use embassy_time::{Duration, Timer};
#[cfg(feature = "hardware")]
use embedded_graphics::pixelcolor::BinaryColor;
#[cfg(feature = "hardware")]
use embedded_graphics::prelude::*;
#[cfg(feature = "hardware")]
use embedded_hal_async::spi::SpiBus;
#[cfg(feature = "hardware")]
use platform::DisplayDriver;

#[cfg(feature = "hardware")]
use super::{DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE};
#[cfg(feature = "hardware")]
use crate::hal::{Color, DapDisplay};

#[cfg(feature = "hardware")]
/// SSD1677 Commands
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
enum Command {
    DriverOutputControl = 0x01,
    GateDrivingVoltage = 0x03,
    SourceDrivingVoltage = 0x04,
    DeepSleepMode = 0x10,
    DataEntryMode = 0x11,
    SoftwareReset = 0x12,
    TempSensorControl = 0x18,
    MasterActivation = 0x20,
    DisplayUpdateControl1 = 0x21,
    DisplayUpdateControl2 = 0x22,
    WriteRamBW = 0x24,
    WriteRamRed = 0x26,
    ReadRam = 0x27,
    VcomSense = 0x28,
    VcomSenseDuration = 0x29,
    ProgramVcomOTP = 0x2A,
    WriteVcomRegister = 0x2C,
    WriteLUT = 0x32,
    BorderWaveform = 0x3C,
    SetRamXAddress = 0x44,
    SetRamYAddress = 0x45,
    SetRamXCounter = 0x4E,
    SetRamYCounter = 0x4F,
    NOP = 0xFF,
}

#[cfg(feature = "hardware")]
/// SSD1677 Display Driver
pub struct Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    spi: SPI,
    dc: Output<'static, DC>,
    cs: Output<'static, CS>,
    rst: Output<'static, RST>,
    busy: Input<'static, BUSY>,
    refresh_mode: platform::RefreshMode,
    partial_refresh_count: u8,
    /// Internal 2bpp packed framebuffer (800x480 / 4 pixels per byte = 96 000 bytes).
    ///
    /// `draw_iter` writes pixels into this buffer so that embedded-graphics
    /// primitives are captured and can be pushed to the controller's SRAM via
    /// `update_buffer` / `refresh_*`.
    framebuffer: [u8; FRAMEBUFFER_SIZE],
}

#[cfg(feature = "hardware")]
impl<SPI, DC, CS, RST, BUSY> Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: SpiBus,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    /// Create a new SSD1677 display driver.
    ///
    /// # Arguments
    ///
    /// * `spi` - Configured SPI peripheral
    /// * `dc` - Data/Command GPIO pin
    /// * `cs` - Chip Select GPIO pin
    /// * `rst` - Reset GPIO pin
    /// * `busy` - Busy status GPIO pin
    pub fn new(
        spi: SPI,
        dc: Output<'static, DC>,
        cs: Output<'static, CS>,
        rst: Output<'static, RST>,
        busy: Input<'static, BUSY>,
    ) -> Self {
        // Initialize framebuffer to white (0xFF = all pixels at level 3 = white)
        Self {
            spi,
            dc,
            cs,
            rst,
            busy,
            refresh_mode: platform::RefreshMode::Full,
            partial_refresh_count: 0,
            framebuffer: [0xFF; FRAMEBUFFER_SIZE],
        }
    }

    /// Send a command to the display controller
    async fn send_command(&mut self, command: Command) -> Result<(), DisplayError> {
        self.cs.set_low();
        self.dc.set_low(); // Command mode

        let result = self.spi.write(&[command as u8]).await;

        self.cs.set_high();

        result.map_err(|_| DisplayError::Communication)
    }

    /// Send data to the display controller
    async fn send_data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        if data.is_empty() {
            return Ok(());
        }

        self.cs.set_low();
        self.dc.set_high(); // Data mode

        let result = self.spi.write(data).await;

        self.cs.set_high();

        result.map_err(|_| DisplayError::Communication)
    }

    /// Wait for the display to become ready (BUSY pin goes low)
    async fn wait_busy(&self) -> Result<(), DisplayError> {
        let timeout = Duration::from_millis(5000);
        let start = embassy_time::Instant::now();

        while self.busy.is_high() {
            if start.elapsed() > timeout {
                return Err(DisplayError::Timeout);
            }
            Timer::after(Duration::from_millis(10)).await;
        }

        Ok(())
    }

    /// Hardware reset sequence
    async fn hardware_reset(&mut self) -> Result<(), DisplayError> {
        self.rst.set_high();
        Timer::after(Duration::from_millis(10)).await;

        self.rst.set_low();
        Timer::after(Duration::from_millis(10)).await;

        self.rst.set_high();
        Timer::after(Duration::from_millis(10)).await;

        self.wait_busy().await
    }

    /// Software reset
    async fn software_reset(&mut self) -> Result<(), DisplayError> {
        self.send_command(Command::SoftwareReset).await?;
        self.wait_busy().await
    }

    /// Load partial refresh LUT
    async fn load_lut_partial(&mut self) -> Result<(), DisplayError> {
        // Faster LUT for partial updates (less flashing)
        const LUT_PARTIAL: [u8; 159] = [
            // Simplified LUT with reduced voltage cycling
            0x00, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x80,
            0x00, 0x00, 0x00, 0x00, 0x40, 0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x80, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x0A, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x22, 0x22, 0x22, 0x22, 0x22, 0x22, 0x00, 0x00, 0x00, 0x22,
            0x17, 0x41, 0xB0, 0x32, 0x36, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        self.send_command(Command::WriteLUT).await?;
        self.send_data(&LUT_PARTIAL).await
    }

    /// Set the RAM window (region to update)
    async fn set_ram_window(
        &mut self,
        x_start: u16,
        y_start: u16,
        x_end: u16,
        y_end: u16,
    ) -> Result<(), DisplayError> {
        // X address: in 8-pixel units (800 pixels / 8 = 100)
        let x_start_byte = (x_start / 8) as u8;
        let x_end_byte = (x_end / 8) as u8;

        self.send_command(Command::SetRamXAddress).await?;
        self.send_data(&[x_start_byte, x_end_byte]).await?;

        // Y address: in pixels
        self.send_command(Command::SetRamYAddress).await?;
        self.send_data(&[
            (y_start & 0xFF) as u8,
            ((y_start >> 8) & 0xFF) as u8,
            (y_end & 0xFF) as u8,
            ((y_end >> 8) & 0xFF) as u8,
        ])
        .await
    }

    /// Set the RAM counter (starting position)
    async fn set_ram_counter(&mut self, x: u16, y: u16) -> Result<(), DisplayError> {
        let x_byte = (x / 8) as u8;

        self.send_command(Command::SetRamXCounter).await?;
        self.send_data(&[x_byte]).await?;

        self.send_command(Command::SetRamYCounter).await?;
        self.send_data(&[(y & 0xFF) as u8, ((y >> 8) & 0xFF) as u8])
            .await
    }

    /// Push the internal framebuffer to the display controller's RAM via SPI.
    ///
    /// Sends in fixed-size chunks so the borrow checker is satisfied: each
    /// chunk slice lives only for the duration of its `send_data` call, after
    /// which `self` is no longer borrowed immutably.
    async fn flush_framebuffer(&mut self) -> Result<(), DisplayError> {
        self.set_ram_counter(0, 0).await?;
        self.send_command(Command::WriteRamBW).await?;

        const CHUNK: usize = 256;
        let mut offset = 0;
        while offset < FRAMEBUFFER_SIZE {
            let end = (offset + CHUNK).min(FRAMEBUFFER_SIZE);
            // SAFETY: offset..end is always within bounds.
            // Each iteration takes a fresh slice that doesn't outlive the call.
            let slice: [u8; CHUNK] = {
                let mut buf = [0u8; CHUNK];
                let len = end - offset;
                buf[..len].copy_from_slice(&self.framebuffer[offset..end]);
                buf
            };
            self.send_data(&slice[..end - offset]).await?;
            offset = end;
        }
        Ok(())
    }
}

#[cfg(feature = "hardware")]
impl<SPI, DC, CS, RST, BUSY> DapDisplay for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: SpiBus,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    async fn init(&mut self) -> Result<(), Self::DriverError> {
        defmt::info!("Initializing SSD1677 display controller");

        // Hardware reset
        self.hardware_reset().await?;

        // Software reset
        self.software_reset().await?;

        // Driver output control (800x480)
        // Gate = 480 - 1 = 479 = 0x01DF
        self.send_command(Command::DriverOutputControl).await?;
        self.send_data(&[0xDF, 0x01, 0x00]).await?;

        // Data entry mode: Y increment, X increment, address counter updated in X direction
        self.send_command(Command::DataEntryMode).await?;
        self.send_data(&[0x03]).await?;

        // Set RAM window to full screen
        self.set_ram_window(0, 0, DISPLAY_WIDTH as u16 - 1, DISPLAY_HEIGHT as u16 - 1)
            .await?;

        // Set RAM counter to origin
        self.set_ram_counter(0, 0).await?;

        // Border waveform - follow LUT
        self.send_command(Command::BorderWaveform).await?;
        self.send_data(&[0x05]).await?;

        // Temperature sensor - use internal
        self.send_command(Command::TempSensorControl).await?;
        self.send_data(&[0x80]).await?;

        // Display update control 1
        self.send_command(Command::DisplayUpdateControl1).await?;
        self.send_data(&[0x00, 0x80]).await?;

        // Using OTP LUT -- custom LUT implementation is future work.
        // Skipping WriteLUT (0x32) so the controller uses the panel's built-in
        // OTP waveform tables, which are guaranteed correct for this panel.

        // Initial clear
        DapDisplay::clear(self, Color::White).await?;

        defmt::info!("SSD1677 initialization complete");
        Ok(())
    }

    fn framebuffer_size(&self) -> usize {
        FRAMEBUFFER_SIZE
    }

    async fn clear(&mut self, color: Color) -> Result<(), Self::DriverError> {
        let byte = match color {
            Color::White => 0xFF,
            Color::Black => 0x00,
        };

        self.framebuffer.fill(byte);
        self.flush_framebuffer().await?;
        self.refresh_full().await
    }
}

#[cfg(feature = "hardware")]
impl<SPI, DC, CS, RST, BUSY> platform::DisplayDriver for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: SpiBus,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    type DriverError = DisplayError;

    fn spec(&self) -> platform::DisplayInfo {
        platform::DisplayInfo {
            width: DISPLAY_WIDTH,
            height: DISPLAY_HEIGHT,
        }
    }

    /// Copy `framebuffer` bytes into the internal buffer and push them to the
    /// display controller's SRAM via SPI.
    ///
    /// The caller must supply exactly `FRAMEBUFFER_SIZE` bytes packed as 2bpp
    /// (4 pixels per byte, MSB-first).
    async fn update_buffer(&mut self, framebuffer: &[u8]) -> Result<(), Self::DriverError> {
        if framebuffer.len() != FRAMEBUFFER_SIZE {
            return Err(DisplayError::InvalidBuffer);
        }

        self.framebuffer.copy_from_slice(framebuffer);
        self.flush_framebuffer().await
    }

    async fn refresh_full(&mut self) -> Result<(), Self::DriverError> {
        defmt::debug!("Starting full refresh");

        // Using OTP LUT -- custom LUT implementation is future work.
        // The panel's internal OTP waveform tables are used for the full refresh.

        // Display update control 2
        self.send_command(Command::DisplayUpdateControl2).await?;
        self.send_data(&[0xF7]).await?;

        // Master activation
        self.send_command(Command::MasterActivation).await?;

        // Wait for completion
        self.wait_busy().await?;

        // Reset partial refresh counter
        self.partial_refresh_count = 0;

        defmt::debug!("Full refresh complete");
        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::DriverError> {
        defmt::debug!("Starting partial refresh");

        // Load partial refresh LUT
        self.load_lut_partial().await?;

        // Display update control 2
        self.send_command(Command::DisplayUpdateControl2).await?;
        self.send_data(&[0xFF]).await?;

        // Master activation
        self.send_command(Command::MasterActivation).await?;

        // Wait for completion
        self.wait_busy().await?;

        // Track partial refresh count
        self.partial_refresh_count += 1;

        // Trigger full refresh after 5 partial refreshes to clear ghosting
        if self.partial_refresh_count >= 5 {
            defmt::info!(
                "Triggering full refresh to clear ghosting after {} partial refreshes",
                self.partial_refresh_count
            );
            self.refresh_full().await?;
        }

        defmt::debug!("Partial refresh complete");
        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::DriverError> {
        defmt::info!("Entering deep sleep mode");

        self.send_command(Command::DeepSleepMode).await?;
        self.send_data(&[0x01]).await?;

        Timer::after(Duration::from_millis(100)).await;

        Ok(())
    }

    async fn wake(&mut self) -> Result<(), Self::DriverError> {
        defmt::info!("Waking from deep sleep");

        // Hardware reset to wake
        self.hardware_reset().await?;

        // Re-initialize
        self.init().await
    }
}

#[cfg(feature = "hardware")]
impl<SPI, DC, CS, RST, BUSY> DrawTarget for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: SpiBus,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    type Color = BinaryColor;
    type Error = DisplayError;

    /// Write pixels directly into the internal 2bpp framebuffer.
    ///
    /// Pixels are packed MSB-first: 4 pixels per byte, 2 bits each.
    /// `BinaryColor::Off` (white) maps to value `3`; `BinaryColor::On`
    /// (black) maps to value `0`.
    ///
    /// Call `update_buffer` or one of the `refresh_*` methods to push the
    /// buffer to the display controller and trigger a physical refresh.
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            // Reject out-of-bounds coordinates
            if point.x < 0
                || point.y < 0
                || point.x >= DISPLAY_WIDTH as i32
                || point.y >= DISPLAY_HEIGHT as i32
            {
                continue;
            }

            let x = point.x as u32;
            let y = point.y as u32;
            let pixel_index = y * DISPLAY_WIDTH + x;
            let byte_index = (pixel_index / 4) as usize;
            // Bit position within the byte (MSB-first: pixel 0 uses bits 7:6)
            let bit_shift = 6 - ((pixel_index % 4) * 2);

            // 2bpp value: BinaryColor::Off (white) = 3, BinaryColor::On (black) = 0
            let val: u8 = match color {
                BinaryColor::Off => 3,
                BinaryColor::On => 0,
            };

            // Clear the 2-bit field then write the new value
            self.framebuffer[byte_index] &= !(0x03 << bit_shift);
            self.framebuffer[byte_index] |= val << bit_shift;
        }
        Ok(())
    }
}

#[cfg(feature = "hardware")]
impl<SPI, DC, CS, RST, BUSY> OriginDimensions for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: SpiBus,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    fn size(&self) -> Size {
        Size::new(DISPLAY_WIDTH, DISPLAY_HEIGHT)
    }
}

#[cfg(feature = "hardware")]
impl<SPI, DC, CS, RST, BUSY> platform::EinkDisplay for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: SpiBus,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    fn refresh_mode(&self) -> platform::RefreshMode {
        self.refresh_mode
    }

    fn set_refresh_mode(&mut self, mode: platform::RefreshMode) {
        self.refresh_mode = mode;
    }

    fn temperature(&self) -> Option<i8> {
        // Not yet implemented: requires reading the controller's internal temperature ADC register.
        None
    }
}

/// Display driver errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayError {
    /// SPI communication error
    Communication,
    /// Display is busy
    Busy,
    /// Operation timed out
    Timeout,
    /// Invalid state
    InvalidState,
    /// Invalid buffer size
    InvalidBuffer,
    /// Unsupported operation
    Unsupported,
}

impl core::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Communication => write!(f, "SPI communication error"),
            Self::Busy => write!(f, "Display is busy"),
            Self::Timeout => write!(f, "Operation timed out"),
            Self::InvalidState => write!(f, "Invalid display state"),
            Self::InvalidBuffer => write!(f, "Invalid buffer size"),
            Self::Unsupported => write!(f, "Unsupported operation"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for DisplayError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::string::ToString;

    #[test]
    fn test_display_error_display_format() {
        assert_eq!(
            DisplayError::Communication.to_string(),
            "SPI communication error"
        );
        assert_eq!(DisplayError::Busy.to_string(), "Display is busy");
        assert_eq!(DisplayError::Timeout.to_string(), "Operation timed out");
        assert_eq!(
            DisplayError::InvalidState.to_string(),
            "Invalid display state"
        );
        assert_eq!(
            DisplayError::InvalidBuffer.to_string(),
            "Invalid buffer size"
        );
        assert_eq!(
            DisplayError::Unsupported.to_string(),
            "Unsupported operation"
        );
    }

    #[test]
    fn test_display_error_equality() {
        assert_eq!(DisplayError::Communication, DisplayError::Communication);
        assert_ne!(DisplayError::Communication, DisplayError::Busy);
        assert_ne!(DisplayError::Timeout, DisplayError::InvalidBuffer);
    }

    #[test]
    fn test_display_error_copy() {
        let e = DisplayError::Timeout;
        let e2 = e; // Copy
        assert_eq!(e, e2);
    }

    #[test]
    fn test_x_address_byte_calculation() {
        // X addresses are stored as 8-pixel units (800 / 8 = 100 bytes per row)
        // Verify the calculation: x_start_byte = (x_start / 8) as u8
        let x_start: u16 = 0;
        let x_end: u16 = 799;
        assert_eq!((x_start / 8) as u8, 0);
        assert_eq!((x_end / 8) as u8, 99);

        // Partial window
        let x_start: u16 = 200;
        let x_end: u16 = 400;
        assert_eq!((x_start / 8) as u8, 25);
        assert_eq!((x_end / 8) as u8, 50);
    }

    #[test]
    fn test_y_address_byte_split() {
        // Y address splits into low and high bytes
        let y_start: u16 = 0x01DF; // 479
        assert_eq!((y_start & 0xFF) as u8, 0xDF);
        assert_eq!(((y_start >> 8) & 0xFF) as u8, 0x01);
    }

    #[test]
    fn test_framebuffer_size_constant() {
        // 2 bits per pixel (4 grayscale levels), packed into bytes
        // 800 * 480 / 4 = 96,000 bytes
        let expected =
            (super::super::DISPLAY_WIDTH as usize * super::super::DISPLAY_HEIGHT as usize) / 4;
        assert_eq!(super::super::FRAMEBUFFER_SIZE, expected);
        assert_eq!(super::super::FRAMEBUFFER_SIZE, 96_000);
    }
}
