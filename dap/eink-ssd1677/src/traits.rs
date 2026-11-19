//! Display-driver abstraction traits.
//!
//! Salvaged from the retired v1 `platform` crate (`crates/platform/src/
//! display.rs`) so this driver crate is self-contained. [`Ssd1677`](crate::Ssd1677)
//! implements [`DisplayDriver`] + [`EinkDisplay`]; a Phase-E board adapter can be
//! generic over these, or use the driver's `embedded_graphics::DrawTarget` impl
//! directly. (The v1 `platform::DisplayError` is intentionally NOT carried over —
//! the driver has its own [`DisplayError`](crate::DisplayError).)

use embedded_graphics::prelude::*;

/// Compact description of a display panel.
///
/// Returned by [`DisplayDriver::spec`] so application code can query display
/// dimensions without depending on the `eink-specs` crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct DisplayInfo {
    /// Panel width in pixels.
    pub width: u32,
    /// Panel height in pixels.
    pub height: u32,
}

/// Unified display driver trait for e-ink panels.
///
/// The hardware SSD1677 driver implements this so application code can be generic
/// over display backends.
#[allow(async_fn_in_trait)]
pub trait DisplayDriver: DrawTarget {
    /// Driver-specific error type.
    type DriverError: core::fmt::Debug;

    /// Return basic display information (dimensions).
    fn spec(&self) -> DisplayInfo;

    /// Transfer a packed 1bpp framebuffer to the display controller's SRAM.
    ///
    /// On hardware this is an SPI transfer. The buffer must be exactly
    /// `(width * height) / 8` bytes long (1 bit per pixel, 8 pixels per byte,
    /// MSB-first; 1 = white, 0 = black).
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
    /// Only meaningful on an emulator; hardware implementations return `None`.
    fn ghosting_level(&self) -> Option<f32> {
        None
    }
}

/// E-ink refresh modes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum RefreshMode {
    /// Full refresh — best quality, clears ghosting, slowest (~2000 ms).
    Full,
    /// Partial refresh — fast, accumulates ghosting (~300 ms).
    Partial,
    /// Fast refresh — fastest, highest ghosting (~260 ms).
    Fast,
}
