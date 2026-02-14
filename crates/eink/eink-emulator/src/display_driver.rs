//! Display driver abstraction for e-ink displays
//!
//! ## Real Hardware HAL Pattern
//!
//! This trait matches the architecture of real e-ink HAL drivers like `epd-waveshare`.
//! Real e-ink controllers (SSD1680, UC8151, IT8951) expose similar high-level APIs:
//!
//! ```rust,ignore
//! // epd-waveshare pattern (real hardware):
//! epd.update_frame(&mut spi, buffer, &mut delay)?;  // Transfer buffer to SRAM
//! epd.display_frame(&mut spi, &mut delay)?;         // Trigger physical refresh
//! epd.wait_until_idle(&mut delay)?;                 // Wait for BUSY pin
//! epd.sleep(&mut spi, &mut delay)?;                 // Power down
//! ```
//!
//! Our `DisplayDriver` trait provides the same functionality:
//!
//! ```rust,ignore
//! // Our emulator (matches hardware pattern):
//! emulator.refresh_full().await?;   // Update + display in one call
//! emulator.sleep().await?;          // Power management
//! ```
//!
//! ## Why This Architecture?
//!
//! - **Hardware drivers don't expose raw SPI commands** - applications call high-level
//!   methods like `display_frame()`, not `send_command(0x12)`
//! - **Waveform selection is application-controlled** - the HAL provides methods to
//!   load different waveform tables (LUTs), just like we select modes
//! - **Async is natural for e-ink** - refreshes take 200-2000ms, async allows other
//!   work during display updates
//! - **Same trait works for emulator and hardware** - applications can swap between
//!   emulator and real hardware without code changes
//!
//! ## Differences from Hardware
//!
//! Real hardware drivers separate "update buffer" from "display refresh":
//!
//! ```rust,ignore
//! // Hardware (explicit separation):
//! epd.update_frame(&mut spi, buffer, &mut delay)?;  // Step 1: Transfer
//! epd.display_frame(&mut spi, &mut delay)?;         // Step 2: Refresh
//!
//! // Our emulator (combined for simplicity):
//! emulator.refresh_full().await?;  // Does both steps
//! ```
//!
//! This is acceptable because the emulator has no real SPI bus or SRAM buffers.
//! Applications using this trait don't need the separation.
//!
//! ## Real-World Verification
//!
//! Confirmed against:
//! - [epd-waveshare](https://docs.rs/epd-waveshare) - Rust embedded HAL
//! - [Adafruit EPD](https://github.com/adafruit/Adafruit_EPD) - Arduino library
//! - Waveshare official C examples
//! - GoodDisplay driver documentation

use crate::refresh_mode::RefreshMode;
use eink_specs::DisplaySpec;
use embedded_graphics::prelude::*;
use std::future::Future;

/// Display driver trait for e-ink displays
///
/// Provides async refresh operations that work with both
/// hardware drivers and desktop emulator.
///
/// This trait combines embedded-graphics' `DrawTarget` with
/// e-ink specific operations like refresh modes and sleep.
pub trait DisplayDriver: DrawTarget {
    type DriverError: core::fmt::Debug;

    /// Get display specification
    ///
    /// Returns reference to the DisplaySpec that defines this display's
    /// characteristics (timing, ghosting rates, temperature ranges, etc.)
    fn spec(&self) -> &'static DisplaySpec;

    /// Update framebuffer to display controller (separate from display)
    ///
    /// Transfers framebuffer data to controller's internal SRAM.
    /// Real hardware: This is an SPI transfer taking 50-200ms for large displays.
    /// Emulator: This is instant but allows separation of concerns.
    ///
    /// After calling this, you can:
    /// - Call display() to trigger physical refresh
    /// - Update the framebuffer again for next frame
    /// - Do other work while display refreshes
    ///
    /// # Example
    /// ```rust,ignore
    /// // Pattern matching real hardware
    /// draw_content(&mut display)?;
    /// display.update_buffer().await?;    // Transfer to SRAM
    /// display.display().await?;          // Trigger refresh
    /// display.wait_ready().await?;       // Poll BUSY pin
    /// ```
    fn update_buffer(&mut self) -> impl Future<Output = Result<(), Self::DriverError>> {
        async { Ok(()) } // Default: no-op
    }

    /// Trigger physical display refresh
    ///
    /// Sends display command to controller, causing physical e-ink particle movement.
    /// Real hardware: This blocks until BUSY pin goes low (200-2000ms).
    /// Emulator: Simulates the timing and visual effects.
    ///
    /// Must call update_buffer() first, or use convenience methods:
    /// - refresh_full() = update_buffer() + display() with full waveform
    /// - refresh_partial() = update_buffer() + display() with partial waveform
    fn display(&mut self) -> impl Future<Output = Result<(), Self::DriverError>> {
        async { Ok(()) } // Default: no-op
    }

    /// Full refresh (clears ghosting) - CONVENIENCE METHOD
    ///
    /// Combines update_buffer() + display() with GC16 waveform.
    /// - Duration: ~2000ms (varies by display and temperature)
    /// - Clears all accumulated ghosting
    /// - Shows 3-4 flashes (black → white → final image)
    /// - Use for: Page transitions, periodic cleanup
    ///
    /// Equivalent to:
    /// ```rust,ignore
    /// display.update_buffer().await?;
    /// display.display_with_mode(WaveformMode::GC16).await?;
    /// ```
    fn refresh_full(&mut self) -> impl Future<Output = Result<(), Self::DriverError>>;

    /// Partial refresh (fast, accumulates ghosting)
    ///
    /// Performs a fast partial update without flashing.
    /// - Duration: ~300ms
    /// - Accumulates ghosting (~15% per refresh)
    /// - Use for: Progress bars, live updates, animations
    /// - Trigger full refresh every 5-10 partial refreshes
    fn refresh_partial(&mut self) -> impl Future<Output = Result<(), Self::DriverError>>;

    /// Fast refresh (fastest, high ghosting)
    ///
    /// Performs ultra-fast refresh with single flash.
    /// - Duration: ~260ms
    /// - High ghosting accumulation (~25% per refresh)
    /// - Only 2 grayscale levels (black/white)
    /// - Use for: Page turns, scrolling
    fn refresh_fast(&mut self) -> impl Future<Output = Result<(), Self::DriverError>> {
        // Default to partial refresh if fast mode not supported
        self.refresh_partial()
    }

    /// Generic refresh with mode selection
    ///
    /// Convenience method that calls the appropriate refresh function
    /// based on the requested mode.
    fn refresh(
        &mut self,
        mode: RefreshMode,
    ) -> impl Future<Output = Result<(), Self::DriverError>> {
        async move {
            match mode {
                RefreshMode::Full => self.refresh_full().await,
                RefreshMode::Partial => self.refresh_partial().await,
                RefreshMode::Fast => self.refresh_fast().await,
            }
        }
    }

    /// Enter low-power sleep mode
    ///
    /// Puts display into deep sleep to save power.
    /// Call `wake()` before next refresh.
    fn sleep(&mut self) -> impl Future<Output = Result<(), Self::DriverError>>;

    /// Wake from sleep mode
    ///
    /// Wakes display from deep sleep.
    /// May require re-initialization.
    fn wake(&mut self) -> impl Future<Output = Result<(), Self::DriverError>>;

    /// Wait until display is ready
    ///
    /// Blocks until BUSY pin goes low (hardware) or
    /// refresh animation completes (emulator).
    ///
    /// Real hardware: Polls GPIO BUSY pin
    /// Emulator: Timing already handled in display(), so this is usually no-op
    fn wait_ready(&mut self) -> impl Future<Output = Result<(), Self::DriverError>> {
        async { Ok(()) }
    }

    /// Display with specific waveform mode
    ///
    /// Advanced method to control exact waveform used for refresh.
    /// Use this when you need precise control over:
    /// - Grayscale levels (16 for GC16, 4 for DU4, 2 for DU/A2)
    /// - Flash count and timing
    /// - Ghosting accumulation rate
    ///
    /// Most applications should use convenience methods instead:
    /// - refresh_full() → GC16
    /// - refresh_partial() → DU4
    /// - refresh_fast() → DU
    fn display_with_mode(
        &mut self,
        _mode: crate::WaveformMode,
    ) -> impl Future<Output = Result<(), Self::DriverError>> {
        async move { Ok(()) } // Default: no-op
    }
}

/// E-ink display extensions
///
/// Additional capabilities beyond basic DisplayDriver.
pub trait EinkDisplay: DisplayDriver {
    /// Get current refresh mode
    fn refresh_mode(&self) -> RefreshMode;

    /// Set refresh mode for future operations
    fn set_refresh_mode(&mut self, mode: RefreshMode);

    /// Read temperature from display sensor
    ///
    /// Returns None if temperature sensor not available.
    /// Temperature affects refresh timing and quality.
    fn temperature(&self) -> Option<i8>;

    /// Get current ghosting level (0.0 - 1.0)
    ///
    /// Only available on emulator, returns None on hardware.
    fn ghosting_level(&self) -> Option<f32> {
        None
    }
}
