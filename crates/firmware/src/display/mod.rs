//! Display drivers for SoulAudio DAP
//!
//! This module provides both hardware and emulator implementations of the
//! display interface for the GDEM0397T81P (Good Display 3.97" 800×480) panel
//! with SSD1677 controller.

pub mod driver;

#[cfg(feature = "emulator")]
pub mod emulator;

// The driver module is always compiled (no hardware gate) so that
// `cargo test` can exercise the SSD1677 driver tests on the host.
pub use driver::{DisplayError, Ssd1677, BYTES_PER_ROW, FRAMEBUFFER_SIZE_1BPP};

// Re-export hardware type alias when building for the embedded target.
#[cfg(feature = "hardware")]
pub use driver::Ssd1677Display;

#[cfg(feature = "emulator")]
pub use emulator::EmulatorDisplay;

/// Display specification for GDEM0397T81P
pub const GDEM0397T81P_SPEC: eink_specs::DisplaySpec = eink_specs::DisplaySpec {
    name: "Good Display GDEM0397T81P",
    width: 800,
    height: 480,
    controller: eink_specs::Controller::SSD1677,
    panel_type: eink_specs::PanelType::Carta1200,
    color_mode: None,    // Monochrome
    grayscale_levels: 4, // 2-bit per pixel
    full_refresh_ms: 2000,
    partial_refresh_ms: 300,
    fast_refresh_ms: 260,
    ghosting_rate_partial: 0.15,
    ghosting_rate_fast: 0.25,
    flash_count_full: 3,
    temp_optimal_min: 15,
    temp_optimal_max: 35,
    temp_operating_min: 0,
    temp_operating_max: 50,
    quirks: None,
};

/// Display width in pixels (GDEM0397T81P)
pub const DISPLAY_WIDTH: u32 = 800;

/// Display height in pixels (GDEM0397T81P)
pub const DISPLAY_HEIGHT: u32 = 480;

/// Framebuffer size in bytes (800×480 with 1 bit per pixel = 48,000 bytes)
///
/// The SSD1677 B/W RAM uses 1bpp: 1=white, 0=black, MSB-first.
pub const FRAMEBUFFER_SIZE: usize = FRAMEBUFFER_SIZE_1BPP;
