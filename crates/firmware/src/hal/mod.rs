//! Hardware Abstraction Layer for DAP Display
//!
//! This module provides a unified interface for both hardware and emulator displays.
//! The trait-based design allows seamless switching between real hardware and desktop
//! development without changing application code.

/// DAP Display trait - unified interface for hardware and emulator
///
/// This trait extends the basic `DisplayDriver` with additional capabilities
/// specific to the SoulAudio DAP, including buffer management and initialization.
pub trait DapDisplay: platform::DisplayDriver {
    /// Initialize the display hardware
    ///
    /// Performs all necessary setup including:
    /// - Hardware reset sequence
    /// - Controller configuration
    /// - LUT (Look-Up Table) loading
    /// - Initial clearing
    async fn init(&mut self) -> Result<(), Self::DriverError>;

    /// Update the internal framebuffer
    ///
    /// This copies pixel data to the display controller's RAM without triggering
    /// a refresh. Call `refresh_full()` or `refresh_partial()` to make changes visible.
    ///
    /// # Arguments
    /// * `framebuffer` - Raw pixel data (format depends on implementation)
    async fn update_buffer(&mut self, framebuffer: &[u8]) -> Result<(), Self::DriverError>;

    /// Get the size of the framebuffer in bytes
    fn framebuffer_size(&self) -> usize;

    /// Clear the display to a specific color
    ///
    /// Implementations should provide their own buffer management strategy
    /// (stack allocation, static buffers, or heap allocation depending on target).
    async fn clear(&mut self, color: Color) -> Result<(), Self::DriverError>;
}

/// Color enumeration for simple clear operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// White (background)
    White,
    /// Black (foreground)
    Black,
}

/// Display configuration for initialization
#[derive(Debug, Clone)]
pub struct DisplayConfig {
    /// SPI clock frequency (Hz)
    pub spi_frequency: u32,
    /// Use internal temperature sensor
    pub internal_temp_sensor: bool,
    /// Enable partial refresh support
    pub partial_refresh_enabled: bool,
    /// Initial refresh mode
    pub initial_mode: platform::RefreshMode,
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            spi_frequency: 4_000_000, // 4 MHz - safe default for SSD1677
            internal_temp_sensor: true,
            partial_refresh_enabled: true,
            initial_mode: platform::RefreshMode::Full,
        }
    }
}
