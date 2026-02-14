//! E-Ink Display Specifications
//!
//! Comprehensive specifications for e-ink displays including physical dimensions,
//! refresh timing, grayscale capabilities, ghosting behavior, and temperature characteristics.
//!
//! # Features
//!
//! - **no_std compatible** - Works on embedded systems
//! - **Display templates** - Pre-configured specs for Waveshare and Good Display panels
//! - **Serde support** - Optional serialization/deserialization for TOML/JSON configs
//! - **Temperature compensation** - Adjust refresh timing based on ambient temperature
//! - **Grayscale levels** - Track capabilities of different panel types
//!
//! # Example
//!
//! ```
//! use eink_specs::displays::WAVESHARE_2_13_V4;
//!
//! let spec = WAVESHARE_2_13_V4;
//! println!("Display: {}", spec.name);
//! println!("Resolution: {}×{}", spec.width, spec.height);
//! println!("Diagonal: {:.2}\"", spec.diagonal_inches());
//! println!("Full refresh: {}ms", spec.full_refresh_ms);
//!
//! // Temperature-adjusted timing
//! let cold_refresh = spec.adjusted_refresh_ms(spec.full_refresh_ms, -5);
//! println!("Full refresh at -5°C: {}ms", cold_refresh);  // ~3000ms (50% slower)
//! ```
//!
//! # Custom Display Specs
//!
//! ```
//! use eink_specs::{DisplaySpec, Controller, PanelType};
//!
//! const MY_DISPLAY: DisplaySpec = DisplaySpec {
//!     name: "Custom Display",
//!     width: 320,
//!     height: 240,
//!     controller: Controller::SSD1680,
//!     panel_type: PanelType::Carta1300,
//!     color_mode: None,
//!     grayscale_levels: 4,
//!     full_refresh_ms: 2000,
//!     partial_refresh_ms: 300,
//!     fast_refresh_ms: 260,
//!     ghosting_rate_partial: 0.15,
//!     ghosting_rate_fast: 0.25,
//!     flash_count_full: 3,
//!     temp_optimal_min: 15,
//!     temp_optimal_max: 35,
//!     temp_operating_min: 0,
//!     temp_operating_max: 50,
//!     quirks: None,
//! };
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

pub mod controller_quirks;
mod display_spec;
pub mod displays;

pub use controller_quirks::{quirks_for_controller, ControllerQuirks, Quirk};
pub use display_spec::{ColorMode, Controller, DisplaySpec, PanelType};
