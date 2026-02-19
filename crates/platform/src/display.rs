//! Display abstraction layer

use embedded_graphics::prelude::*;

/// Compact description of a display panel.
///
/// Returned by [`DisplayDriver::spec`] so application code can query display
/// dimensions without depending on the `eink-specs` crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayInfo {
    /// Panel width in pixels.
    pub width: u32,
    /// Panel height in pixels.
    pub height: u32,
}

/// Unified display driver trait for e-ink panels.
///
/// Both the hardware SSD1677 driver and the desktop `EmulatorDisplay` implement
/// this trait so application code can be generic over display backends.
#[allow(async_fn_in_trait)]
pub trait DisplayDriver: DrawTarget {
    /// Driver-specific error type.
    type DriverError: core::fmt::Debug;

    /// Return basic display information (dimensions).
    fn spec(&self) -> DisplayInfo;

    /// Transfer a 2bpp packed framebuffer to the display controller's SRAM.
    ///
    /// On hardware this is an SPI transfer; on the emulator it parses the
    /// packed bytes and draws each pixel into the emulator framebuffer.
    ///
    /// The buffer must be exactly `(width * height) / 4` bytes long (2 bits
    /// per pixel, 4 pixels per byte, MSB-first).
    async fn update_buffer(&mut self, framebuffer: &[u8]) -> Result<(), Self::DriverError>;

    /// Perform a full refresh (clears ghosting, slow).
    async fn refresh_full(&mut self) -> Result<(), Self::DriverError>;

    /// Perform a partial refresh (fast, may accumulate ghosting).
    async fn refresh_partial(&mut self) -> Result<(), Self::DriverError>;

    /// Perform a fast refresh (fastest, highest ghosting).
    ///
    /// Defaults to `refresh_partial` if not separately implemented.
    async fn refresh_fast(&mut self) -> Result<(), Self::DriverError> {
        self.refresh_partial().await
    }

    /// Enter deep sleep to save power.
    async fn sleep(&mut self) -> Result<(), Self::DriverError>;

    /// Wake from deep sleep.
    async fn wake(&mut self) -> Result<(), Self::DriverError>;

    /// Block until the display signals it is ready (BUSY pin low).
    ///
    /// On the emulator this is a no-op because timing is simulated inside the
    /// refresh methods.
    async fn wait_ready(&mut self) -> Result<(), Self::DriverError> {
        Ok(())
    }

    /// Get display dimensions (convenience wrapper around [`spec`][Self::spec]).
    fn dimensions(&self) -> Size {
        let info = self.spec();
        Size::new(info.width, info.height)
    }
}

/// E-ink specific extensions beyond the base [`DisplayDriver`].
pub trait EinkDisplay: DisplayDriver {
    /// Get the currently active refresh mode.
    fn refresh_mode(&self) -> RefreshMode;

    /// Set the refresh mode for future operations.
    fn set_refresh_mode(&mut self, mode: RefreshMode);

    /// Read the display's internal temperature sensor.
    ///
    /// Returns `None` if the sensor is unavailable.
    fn temperature(&self) -> Option<i8>;

    /// Get the current ghosting level as a value in `[0.0, 1.0]`.
    ///
    /// Only meaningful on the emulator; hardware implementations return `None`.
    fn ghosting_level(&self) -> Option<f32> {
        None
    }
}

/// E-ink refresh modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshMode {
    /// Full refresh — best quality, clears ghosting, slowest (~2000 ms).
    Full,
    /// Partial refresh — fast, accumulates ghosting (~300 ms).
    Partial,
    /// Fast refresh — fastest, highest ghosting (~260 ms).
    Fast,
}

/// Platform-level display errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayError {
    /// Communication error (e.g. SPI failure).
    Communication,
    /// Display busy.
    Busy,
    /// Invalid state.
    InvalidState,
    /// Operation timed out.
    Timeout,
}

#[cfg(feature = "std")]
impl std::error::Error for DisplayError {}

impl core::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Communication => write!(f, "Display communication error"),
            Self::Busy => write!(f, "Display is busy"),
            Self::InvalidState => write!(f, "Display in invalid state"),
            Self::Timeout => write!(f, "Display operation timeout"),
        }
    }
}
