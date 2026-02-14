//! Emulator Display Driver
//!
//! Wraps the eink-emulator for desktop development. Provides the same
//! interface as the hardware driver, enabling seamless testing without
//! physical hardware.

use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::{DrawTarget, OriginDimensions, Point, Pixel, Size, GrayColor, Dimensions, Primitive, Drawable};
use tokio::time::Duration;

use crate::hal::DapDisplay;
use super::{GDEM0397T81P_SPEC, DISPLAY_WIDTH, DISPLAY_HEIGHT, FRAMEBUFFER_SIZE};

/// Emulator display wrapper
pub struct EmulatorDisplay {
    emulator: eink_emulator::Emulator,
}

impl EmulatorDisplay {
    /// Create a new emulator display with default configuration
    pub fn new() -> Self {
        // Use with_spec to create with GDEM0397T81P specification
        let emulator = eink_emulator::Emulator::with_spec(&GDEM0397T81P_SPEC);

        Self { emulator }
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
    pub fn with_config(config: eink_emulator::EmulatorConfig) -> Self {
        let emulator = eink_emulator::Emulator::with_spec_and_config(&GDEM0397T81P_SPEC, config);

        Self { emulator }
    }

    /// Create a headless emulator (no window, for testing)
    pub fn headless() -> Self {
        let emulator = eink_emulator::Emulator::headless_with_spec(&GDEM0397T81P_SPEC);

        Self { emulator }
    }

    /// Get a reference to the underlying emulator
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
        println!("Initializing emulator display ({}×{})", DISPLAY_WIDTH, DISPLAY_HEIGHT);

        // Emulator doesn't need initialization, but we simulate the delay
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Clear to white using the DapDisplay trait method
        DapDisplay::clear(self, crate::hal::Color::White).await?;

        println!("Emulator display initialized");
        Ok(())
    }

    async fn update_buffer(&mut self, _framebuffer: &[u8]) -> Result<(), Self::DriverError> {
        // The emulator works with DrawTarget directly, so we don't need
        // to manually transfer a framebuffer. This method is a no-op
        // for the emulator.

        // In practice, applications should draw using embedded-graphics
        // primitives directly to the display.
        Ok(())
    }

    fn framebuffer_size(&self) -> usize {
        FRAMEBUFFER_SIZE
    }

    async fn clear(&mut self, color: crate::hal::Color) -> Result<(), Self::DriverError> {
        // Clear the entire display
        use embedded_graphics::primitives::{Rectangle, PrimitiveStyle};

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

    async fn refresh_full(&mut self) -> Result<(), Self::DriverError> {
        println!("Emulator: Full refresh");

        // Use the eink-emulator's DisplayDriver trait
        use eink_emulator::DisplayDriver;

        self.emulator.refresh_full().await
            .map_err(|_| EmulatorError::RefreshFailed)?;

        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::DriverError> {
        println!("Emulator: Partial refresh");

        // Use the eink-emulator's DisplayDriver trait
        use eink_emulator::DisplayDriver;

        self.emulator.refresh_partial().await
            .map_err(|_| EmulatorError::RefreshFailed)?;

        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::DriverError> {
        println!("Emulator: Entering sleep mode");

        // Use the eink-emulator's DisplayDriver trait
        use eink_emulator::DisplayDriver;

        self.emulator.sleep().await
            .map_err(|_| EmulatorError::RefreshFailed)?;

        Ok(())
    }

    async fn wake(&mut self) -> Result<(), Self::DriverError> {
        println!("Emulator: Waking from sleep");

        // Use the eink-emulator's DisplayDriver trait
        use eink_emulator::DisplayDriver;

        self.emulator.wake().await
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
        self.emulator.draw_iter(pixels)
            .map_err(|_| EmulatorError::DrawFailed)
    }
}

impl OriginDimensions for EmulatorDisplay {
    fn size(&self) -> Size {
        self.emulator.bounding_box().size
    }
}

impl platform::EinkDisplay for EmulatorDisplay {
    fn set_refresh_mode(&mut self, _mode: platform::RefreshMode) {
        // The emulator handles refresh mode automatically
        // This is a no-op for compatibility
    }

    fn temperature(&self) -> Option<i8> {
        // Emulator can simulate different temperatures
        Some(25) // 25°C default
    }
}

/// Emulator display errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
