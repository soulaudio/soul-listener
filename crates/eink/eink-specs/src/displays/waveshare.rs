//! Waveshare e-ink display specifications
//!
//! Pre-configured specs for common Waveshare displays based on official datasheets.

use crate::{
    controller_quirks::quirks_for_controller, ColorMode, Controller, DisplaySpec, PanelType,
};

/// Waveshare 2.13" V4 (250×122, SSD1680, Carta 1000)
///
/// Popular small display with fast refresh support.
/// - Grayscale: 4 levels
/// - Full refresh: ~2s with 3 flashes
/// - Partial refresh: 300ms
/// - Fast refresh: 260ms
///
/// # Known Quirks
/// - Uncontrollable refresh rate with certain driver implementations
pub const WAVESHARE_2_13_V4: DisplaySpec = DisplaySpec {
    name: "Waveshare 2.13\" V4",
    width: 250,
    height: 122,
    controller: Controller::SSD1680,
    panel_type: PanelType::Carta1000,
    grayscale_levels: 4,
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
    color_mode: None,
    quirks: Some(quirks_for_controller(Controller::SSD1680)),
};

/// Waveshare 2.9" V2 (296×128, IL0373, Carta 1000)
///
/// Medium-sized display with good contrast.
/// - Grayscale: 4 levels
/// - Full refresh: ~2s
/// - Partial refresh: 300ms
/// - No fast mode support
pub const WAVESHARE_2_9_V2: DisplaySpec = DisplaySpec {
    name: "Waveshare 2.9\" V2",
    width: 296,
    height: 128,
    controller: Controller::IL0373,
    panel_type: PanelType::Carta1000,
    grayscale_levels: 4,
    full_refresh_ms: 2000,
    partial_refresh_ms: 300,
    fast_refresh_ms: 300, // No dedicated fast mode
    ghosting_rate_partial: 0.15,
    ghosting_rate_fast: 0.15,
    flash_count_full: 3,
    temp_optimal_min: 15,
    temp_optimal_max: 35,
    temp_operating_min: 0,
    temp_operating_max: 50,
    color_mode: None,
    quirks: Some(quirks_for_controller(Controller::IL0373)),
};

/// Waveshare 4.2" V2 (400×300, SSD1619, Carta 1200)
///
/// Large display with improved Carta 1200 panel.
/// - Grayscale: 4 levels
/// - Full refresh: ~2s
/// - Partial refresh: 800ms (larger area)
/// - Fast refresh: 500ms
pub const WAVESHARE_4_2_V2: DisplaySpec = DisplaySpec {
    name: "Waveshare 4.2\" V2",
    width: 400,
    height: 300,
    controller: Controller::SSD1619,
    panel_type: PanelType::Carta1200,
    grayscale_levels: 4,
    full_refresh_ms: 2000,
    partial_refresh_ms: 800,
    fast_refresh_ms: 500,
    ghosting_rate_partial: 0.12,
    ghosting_rate_fast: 0.22,
    flash_count_full: 3,
    temp_optimal_min: 15,
    temp_optimal_max: 35,
    temp_operating_min: 0,
    temp_operating_max: 50,
    color_mode: None,
    quirks: Some(quirks_for_controller(Controller::SSD1619)),
};

/// Waveshare 7.5" V2 (800×480, ED075TC1, Carta 1200)
///
/// Large display suitable for dashboards and signage.
/// - Grayscale: 4 levels
/// - Full refresh: ~5s (large area)
/// - Partial refresh: 2s
/// - Fast refresh: 1.5s
pub const WAVESHARE_7_5_V2: DisplaySpec = DisplaySpec {
    name: "Waveshare 7.5\" V2",
    width: 800,
    height: 480,
    controller: Controller::ED075TC1,
    panel_type: PanelType::Carta1200,
    grayscale_levels: 4,
    full_refresh_ms: 5000,
    partial_refresh_ms: 2000,
    fast_refresh_ms: 1500,
    ghosting_rate_partial: 0.12,
    ghosting_rate_fast: 0.20,
    flash_count_full: 4,
    temp_optimal_min: 15,
    temp_optimal_max: 35,
    temp_operating_min: 0,
    temp_operating_max: 50,
    color_mode: None,
    quirks: Some(quirks_for_controller(Controller::ED075TC1)),
};

/// Waveshare 5.65" Spectra 6 (600×448, ACeP, Spectra 6)
///
/// Advanced Color ePaper display with 6 colors.
/// - Colors: Black, White, Red, Yellow, Blue, Green
/// - Full refresh: ~15s (color particle movement)
/// - Partial refresh: Not recommended (color ghosting)
/// - Fast refresh: Not supported
///
/// # Characteristics
/// - 4 ink particles: red, blue, yellow, white
/// - Produces 6 distinct colors through particle combinations
/// - Temperature range: 0-50°C (wider than B&W)
/// - Resolution: 200ppi
pub const WAVESHARE_5_65_SPECTRA6: DisplaySpec = DisplaySpec {
    name: "Waveshare 5.65\" Spectra 6",
    width: 600,
    height: 448,
    controller: Controller::ACeP,
    panel_type: PanelType::Spectra6,
    grayscale_levels: 6,         // 6 distinct colors
    full_refresh_ms: 15000,      // 15 seconds for color
    partial_refresh_ms: 15000,   // Same as full (not recommended)
    fast_refresh_ms: 15000,      // No fast mode for color
    ghosting_rate_partial: 0.12, // Higher ghosting for color
    ghosting_rate_fast: 0.12,
    flash_count_full: 30, // Many flashes for color particles
    temp_optimal_min: 0,  // Wider range for color
    temp_optimal_max: 50,
    temp_operating_min: 0,
    temp_operating_max: 50,
    color_mode: Some(ColorMode::Spectra6),
    quirks: Some(quirks_for_controller(Controller::ACeP)),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_waveshare_2_13_v4() {
        assert_eq!(WAVESHARE_2_13_V4.width, 250);
        assert_eq!(WAVESHARE_2_13_V4.height, 122);
        assert_eq!(WAVESHARE_2_13_V4.controller, Controller::SSD1680);
        // Diagonal is approximate - just check it's in reasonable range
        let diagonal = WAVESHARE_2_13_V4.diagonal_inches();
        assert!(diagonal > 1.5 && diagonal < 3.0);
    }

    #[test]
    fn test_waveshare_2_9_v2() {
        assert_eq!(WAVESHARE_2_9_V2.width, 296);
        assert_eq!(WAVESHARE_2_9_V2.height, 128);
        assert_eq!(WAVESHARE_2_9_V2.controller, Controller::IL0373);
    }

    #[test]
    fn test_waveshare_4_2_v2() {
        assert_eq!(WAVESHARE_4_2_V2.width, 400);
        assert_eq!(WAVESHARE_4_2_V2.height, 300);
        // Diagonal is approximate - just check it's in reasonable range
        let diagonal = WAVESHARE_4_2_V2.diagonal_inches();
        assert!(diagonal > 3.0 && diagonal < 5.0);
    }

    #[test]
    fn test_all_displays_valid_temps() {
        for spec in &[
            &WAVESHARE_2_13_V4,
            &WAVESHARE_2_9_V2,
            &WAVESHARE_4_2_V2,
            &WAVESHARE_7_5_V2,
        ] {
            assert!(spec.is_optimal_temp(25));
            assert!(spec.is_operating_temp(25));
            assert!(!spec.is_operating_temp(-20));
        }
    }
}
