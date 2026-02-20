//! Emulator Display Driver
//!
//! Wraps the eink-emulator for desktop development. Provides the same
//! interface as the hardware driver, enabling seamless testing without
//! physical hardware.

use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::{
    Dimensions, DrawTarget, Drawable, GrayColor, OriginDimensions, Pixel, Point, Primitive, Size,
};
use tokio::time::Duration;

use super::{DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE, GDEM0397T81P_SPEC};
use crate::hal::DapDisplay;

/// Emulator display wrapper
#[allow(clippy::module_name_repetitions)] // Type is named for its module; used as firmware::EmulatorDisplay
pub struct EmulatorDisplay {
    emulator: eink_emulator::Emulator,
    refresh_mode: platform::RefreshMode,
}

impl EmulatorDisplay {
    /// Create a new emulator display with default configuration
    #[must_use]
    pub fn new() -> Self {
        // Use with_spec to create with GDEM0397T81P specification
        let emulator = eink_emulator::Emulator::with_spec(&GDEM0397T81P_SPEC);

        Self {
            emulator,
            refresh_mode: platform::RefreshMode::Full,
        }
    }

    /// Create an emulator display with custom configuration
    ///
    /// # Example
    /// ```no_run
    /// use firmware::EmulatorDisplay;
    /// use eink_emulator::{EmulatorConfig, Rotation};
    ///
    /// let config = EmulatorConfig {
    ///     rotation: Rotation::Degrees90,  // Portrait mode
    ///     scale: 1,                        // No upscaling
    /// };
    /// let display = EmulatorDisplay::with_config(config);
    /// ```
    #[must_use]
    pub fn with_config(config: eink_emulator::EmulatorConfig) -> Self {
        let emulator = eink_emulator::Emulator::with_spec_and_config(&GDEM0397T81P_SPEC, config);

        Self {
            emulator,
            refresh_mode: platform::RefreshMode::Full,
        }
    }

    /// Create emulator with custom display specification and configuration
    pub fn with_spec_and_config(
        spec: &'static eink_specs::DisplaySpec,
        config: eink_emulator::EmulatorConfig,
    ) -> Self {
        let emulator = eink_emulator::Emulator::with_spec_and_config(spec, config);

        Self {
            emulator,
            refresh_mode: platform::RefreshMode::Full,
        }
    }

    /// Create a headless emulator (no window, for testing)
    pub fn headless() -> Self {
        let emulator = eink_emulator::Emulator::headless_with_spec(&GDEM0397T81P_SPEC);

        Self {
            emulator,
            refresh_mode: platform::RefreshMode::Full,
        }
    }

    /// Get a reference to the underlying emulator
    #[must_use]
    pub fn emulator(&self) -> &eink_emulator::Emulator {
        &self.emulator
    }

    /// Get a mutable reference to the underlying emulator
    pub fn emulator_mut(&mut self) -> &mut eink_emulator::Emulator {
        &mut self.emulator
    }

    /// Consume this display and return the underlying emulator
    ///
    /// Useful for running the event loop: `display.into_inner().run()`
    pub fn into_inner(self) -> eink_emulator::Emulator {
        self.emulator
    }
}

impl Default for EmulatorDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl DapDisplay for EmulatorDisplay {
    async fn init(&mut self) -> Result<(), Self::DriverError> {
        tracing::debug!(
            width = DISPLAY_WIDTH,
            height = DISPLAY_HEIGHT,
            "Initializing emulator display"
        );

        // Emulator doesn't need initialization, but we simulate the delay
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Clear to white using the DapDisplay trait method
        DapDisplay::clear(self, crate::hal::Color::White).await?;

        tracing::debug!("Emulator display initialized");
        Ok(())
    }

    fn framebuffer_size(&self) -> usize {
        FRAMEBUFFER_SIZE
    }

    async fn clear(&mut self, color: crate::hal::Color) -> Result<(), Self::DriverError> {
        // Clear the entire display
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let fill_color = match color {
            crate::hal::Color::White => Gray4::WHITE,
            crate::hal::Color::Black => Gray4::BLACK,
        };

        let size = self.emulator.bounding_box().size;
        Rectangle::new(Point::zero(), size)
            .into_styled(PrimitiveStyle::with_fill(fill_color))
            .draw(&mut self.emulator)
            .map_err(|_| EmulatorError::DrawFailed)?;

        // Trigger a refresh to show the clear
        platform::DisplayDriver::refresh_full(self).await
    }
}

impl platform::DisplayDriver for EmulatorDisplay {
    type DriverError = EmulatorError;

    fn spec(&self) -> platform::display::DisplayInfo {
        platform::display::DisplayInfo {
            width: DISPLAY_WIDTH,
            height: DISPLAY_HEIGHT,
        }
    }

    /// Parse a 2bpp packed framebuffer and draw each pixel into the emulator.
    ///
    /// 2bpp format: each byte holds 4 pixels, MSB-first. Pixel values 0-3 map
    /// to Gray4 levels 0-3 (0 = black, 3 = white).
    async fn update_buffer(&mut self, framebuffer: &[u8]) -> Result<(), Self::DriverError> {
        let expected = (DISPLAY_WIDTH as usize * DISPLAY_HEIGHT as usize) / 4;
        if framebuffer.len() != expected {
            return Err(EmulatorError::InvalidBuffer);
        }

        // Unpack 2bpp bytes: 4 pixels per byte, MSB-first.
        // Gray4 values: 0 = black, 3 = white (fully lit).
        let mut pixel_index: u32 = 0;
        for &byte in framebuffer {
            for shift in (0..4).rev() {
                let val = (byte >> (shift * 2)) & 0x03;
                let color = Gray4::new(val);
                let x = pixel_index % DISPLAY_WIDTH;
                let y = pixel_index / DISPLAY_WIDTH;
                self.emulator
                    .draw_iter(core::iter::once(Pixel(
                        Point::new(x as i32, y as i32),
                        color,
                    )))
                    .map_err(|_| EmulatorError::DrawFailed)?;
                pixel_index += 1;
            }
        }

        Ok(())
    }

    async fn refresh_full(&mut self) -> Result<(), Self::DriverError> {
        tracing::debug!("Emulator: full refresh");

        // Use the eink-emulator's DisplayDriver trait
        use eink_emulator::DisplayDriver;

        self.emulator
            .refresh_full()
            .await
            .map_err(|_| EmulatorError::RefreshFailed)?;

        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::DriverError> {
        tracing::debug!("Emulator: partial refresh");

        // Use the eink-emulator's DisplayDriver trait
        use eink_emulator::DisplayDriver;

        self.emulator
            .refresh_partial()
            .await
            .map_err(|_| EmulatorError::RefreshFailed)?;

        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::DriverError> {
        tracing::debug!("Emulator: entering sleep mode");

        // Use the eink-emulator's DisplayDriver trait
        use eink_emulator::DisplayDriver;

        self.emulator
            .sleep()
            .await
            .map_err(|_| EmulatorError::RefreshFailed)?;

        Ok(())
    }

    async fn wake(&mut self) -> Result<(), Self::DriverError> {
        tracing::debug!("Emulator: waking from sleep");

        // Use the eink-emulator's DisplayDriver trait
        use eink_emulator::DisplayDriver;

        self.emulator
            .wake()
            .await
            .map_err(|_| EmulatorError::RefreshFailed)?;

        Ok(())
    }
}

impl DrawTarget for EmulatorDisplay {
    type Color = Gray4;
    type Error = EmulatorError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        // Forward to emulator's DrawTarget implementation
        self.emulator
            .draw_iter(pixels)
            .map_err(|_| EmulatorError::DrawFailed)
    }
}

impl OriginDimensions for EmulatorDisplay {
    fn size(&self) -> Size {
        self.emulator.bounding_box().size
    }
}

impl platform::EinkDisplay for EmulatorDisplay {
    fn refresh_mode(&self) -> platform::RefreshMode {
        self.refresh_mode
    }

    fn set_refresh_mode(&mut self, mode: platform::RefreshMode) {
        self.refresh_mode = mode;
    }

    fn temperature(&self) -> Option<i8> {
        // Emulator can simulate different temperatures
        Some(25) // 25 degrees C default
    }

    fn ghosting_level(&self) -> Option<f32> {
        Some(self.emulator.ghosting_level())
    }
}

/// Emulator display errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::module_name_repetitions)] // Error type named after its module; follows Rust convention for module-scoped errors
pub enum EmulatorError {
    /// Invalid buffer size
    InvalidBuffer,
    /// Refresh operation failed
    RefreshFailed,
    /// Draw operation failed
    DrawFailed,
}

impl core::fmt::Display for EmulatorError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidBuffer => write!(f, "Invalid buffer size"),
            Self::RefreshFailed => write!(f, "Refresh operation failed"),
            Self::DrawFailed => write!(f, "Draw operation failed"),
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for EmulatorError {}
