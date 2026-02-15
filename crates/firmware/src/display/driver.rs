//! SSD1677 Hardware Driver for STM32H7
//!
//! This driver implements the SSD1677 e-ink controller interface for the
//! GDEM0397T81P (800×480) display panel using Embassy async HAL.
//!
//! # Pin Configuration
//!
//! - PA5 (SPI1_SCK)  → SCK
//! - PA7 (SPI1_MOSI) → DIN
//! - PB0 (GPIO)      → DC (Data/Command)
//! - PB1 (GPIO)      → CS (Chip Select)
//! - PB2 (GPIO)      → RST (Reset)
//! - PB3 (GPIO)      → BUSY
//!
//! # Memory Layout
//!
//! Framebuffer is stored in AXI SRAM (96KB):
//! ```text
//! #[link_section = ".axisram"]
//! static FRAMEBUFFER: [u8; 96000] = [0xFF; 96000];
//! ```

use embassy_stm32::gpio::{Input, Level, Output, Pull, Speed};
use embassy_stm32::spi::{BitOrder, Config as SpiConfig, Spi, MODE_0};
use embassy_stm32::time::Hertz;
use embassy_time::{Duration, Timer};
use embedded_graphics::geometry::Point;
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use heapless::Vec;
use static_cell::StaticCell;

use super::{DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE};
use crate::hal::{DapDisplay, DisplayConfig};

/// SSD1677 Commands
#[repr(u8)]
#[derive(Debug, Clone, Copy)]
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

/// SSD1677 Display Driver
pub struct Ssd1677Display<SPI, DC, CS, RST, BUSY> {
    spi: SPI,
    dc: Output<'static, DC>,
    cs: Output<'static, CS>,
    rst: Output<'static, RST>,
    busy: Input<'static, BUSY>,
    refresh_mode: platform::RefreshMode,
    partial_refresh_count: u8,
}

impl<SPI, DC, CS, RST, BUSY> Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    /// Create a new SSD1677 display driver
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
        Self {
            spi,
            dc,
            cs,
            rst,
            busy,
            refresh_mode: platform::RefreshMode::Full,
            partial_refresh_count: 0,
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

    /// Load full refresh LUT (Look-Up Table)
    async fn load_lut_full(&mut self) -> Result<(), DisplayError> {
        // SSD1677 full refresh LUT (waveform data)
        // This is a standard 4-gray LUT optimized for the GDEM0397T81P panel
        const LUT_FULL: [u8; 159] = [
            // VCOM LUT (70 bytes)
            0x00, 0x19, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, // White-to-White LUT (42 bytes)
            0x00, 0x19, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // Black-to-White LUT (42 bytes)
            0x80, 0x19, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            // White-to-Black LUT (5 bytes remaining = 159 total)
            0x40, 0x19, 0x01, 0x00, 0x00,
        ];

        self.send_command(Command::WriteLUT).await?;
        self.send_data(&LUT_FULL).await
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
}

impl<SPI, DC, CS, RST, BUSY> DapDisplay for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: embedded_hal_async::spi::SpiDevice,
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

        // Driver output control (800×480)
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

        // Load full refresh LUT
        self.load_lut_full().await?;

        // Initial clear
        self.clear(crate::hal::Color::White).await?;

        defmt::info!("SSD1677 initialization complete");
        Ok(())
    }

    async fn update_buffer(&mut self, framebuffer: &[u8]) -> Result<(), Self::DriverError> {
        if framebuffer.len() != FRAMEBUFFER_SIZE {
            return Err(DisplayError::InvalidBuffer);
        }

        // Set RAM counter to origin
        self.set_ram_counter(0, 0).await?;

        // Write to black/white RAM
        self.send_command(Command::WriteRamBW).await?;
        self.send_data(framebuffer).await?;

        Ok(())
    }

    fn framebuffer_size(&self) -> usize {
        FRAMEBUFFER_SIZE
    }
}

impl<SPI, DC, CS, RST, BUSY> platform::DisplayDriver for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    type DriverError = DisplayError;

    async fn refresh_full(&mut self) -> Result<(), Self::DriverError> {
        defmt::debug!("Starting full refresh");

        // Load full refresh LUT
        self.load_lut_full().await?;

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

impl<SPI, DC, CS, RST, BUSY> DrawTarget for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    type Color = BinaryColor;
    type Error = DisplayError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        // Drawing directly to hardware is not efficient for e-ink
        // This implementation is provided for compatibility but should
        // not be used in production. Instead, draw to a framebuffer
        // and call update_buffer().

        // For now, return an error to indicate this is not supported
        Err(DisplayError::Unsupported)
    }
}

impl<SPI, DC, CS, RST, BUSY> OriginDimensions for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    fn size(&self) -> Size {
        Size::new(DISPLAY_WIDTH, DISPLAY_HEIGHT)
    }
}

impl<SPI, DC, CS, RST, BUSY> platform::EinkDisplay for Ssd1677Display<SPI, DC, CS, RST, BUSY>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embassy_stm32::gpio::Pin,
    CS: embassy_stm32::gpio::Pin,
    RST: embassy_stm32::gpio::Pin,
    BUSY: embassy_stm32::gpio::Pin,
{
    fn set_refresh_mode(&mut self, mode: platform::RefreshMode) {
        self.refresh_mode = mode;
    }

    fn temperature(&self) -> Option<i8> {
        // TODO: Implement temperature reading from internal sensor
        // For now, return None
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
