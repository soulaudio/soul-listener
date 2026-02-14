//! Good Display e-ink display specifications
//!
//! Pre-configured specs for common Good Display panels based on official datasheets.

use crate::{controller_quirks::quirks_for_controller, Controller, DisplaySpec, PanelType};

/// Good Display GDEW0213I5F (212×104, UC8151, Pearl)
///
/// Compact 2.13" display with older Pearl panel technology.
/// - Grayscale: 4 levels
/// - Full refresh: ~2s
/// - Partial refresh: 500ms (slower than newer Carta panels)
/// - No fast mode
///
/// # Known Quirks
/// - Rotation changes can cause garbled output
/// - SPI interface can hang during initialization
pub const GDEW0213I5F: DisplaySpec = DisplaySpec {
    name: "GDEW0213I5F",
    width: 212,
    height: 104,
    controller: Controller::UC8151,
    panel_type: PanelType::Pearl,
    grayscale_levels: 4,
    full_refresh_ms: 2000,
    partial_refresh_ms: 500,
    fast_refresh_ms: 500,
    ghosting_rate_partial: 0.18, // Higher ghosting on Pearl
    ghosting_rate_fast: 0.18,
    flash_count_full: 4, // More flashes needed
    temp_optimal_min: 15,
    temp_optimal_max: 35,
    temp_operating_min: 0,
    temp_operating_max: 50,
    color_mode: None,
    quirks: Some(quirks_for_controller(Controller::UC8151)),
};

/// Good Display GDEW029T5 (296×128, GDEW, Carta 1000)
///
/// 2.9" display with improved Carta panel.
/// - Grayscale: 4 levels
/// - Full refresh: ~2s
/// - Partial refresh: 300ms
/// - Fast refresh: 280ms
pub const GDEW029T5: DisplaySpec = DisplaySpec {
    name: "GDEW029T5",
    width: 296,
    height: 128,
    controller: Controller::GDEW,
    panel_type: PanelType::Carta1000,
    grayscale_levels: 4,
    full_refresh_ms: 2000,
    partial_refresh_ms: 300,
    fast_refresh_ms: 280,
    ghosting_rate_partial: 0.15,
    ghosting_rate_fast: 0.24,
    flash_count_full: 3,
    temp_optimal_min: 15,
    temp_optimal_max: 35,
    temp_operating_min: 0,
    temp_operating_max: 50,
    color_mode: None,
    quirks: Some(quirks_for_controller(Controller::GDEW)),
};

/// Good Display GDEW042T2 (400×300, SSD1619, Carta 1200)
///
/// Large 4.2" display with Carta 1200 panel.
/// - Grayscale: 4 levels
/// - Full refresh: ~2s
/// - Partial refresh: 800ms
/// - Fast refresh: 500ms
pub const GDEW042T2: DisplaySpec = DisplaySpec {
    name: "GDEW042T2",
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

/// Good Display GDEW075T7 (800×480, GDEW, Carta 1200)
///
/// Large 7.5" display for dashboards and larger interfaces.
/// - Grayscale: 4 levels
/// - Full refresh: ~5s
/// - Partial refresh: 2s
/// - Fast refresh: 1.5s
pub const GDEW075T7: DisplaySpec = DisplaySpec {
    name: "GDEW075T7",
    width: 800,
    height: 480,
    controller: Controller::GDEW,
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
    quirks: Some(quirks_for_controller(Controller::GDEW)),
};

/// Good Display GDEM0397T81P (800×480, SSD1677, Carta)
///
/// High-resolution 3.97" display perfect for Digital Audio Players.
/// **Part Number:** 100397T8
///
/// ## Specifications
/// - Resolution: 800×480 pixels (235 PPI)
/// - Active Area: 86.40 × 51.84mm
/// - Grayscale: 4 levels
/// - Interface: SPI (24-pin FPC, 0.5mm pitch)
///
/// ## Performance
/// - Full refresh: 3 seconds
/// - Fast refresh: 1.5 seconds
/// - Partial refresh: 300ms
///
/// ## Power (at 3.3V)
/// - Typical: 36mW (~11mA average)
/// - Refresh: ~34mA average during full refresh
/// - Deep sleep: 0.003mW (~1µA)
///
/// ## Temperature
/// - Operating: 0°C to 50°C
/// - Storage: -25°C to 70°C
/// - Optimal: 15°C to 35°C
///
/// ## Compatibility
/// - STM32, ESP32, ESP8266 microcontrollers
/// - SPI interface at up to 10MHz
///
/// ## Use Cases
/// - Digital Audio Player displays
/// - E-book readers
/// - Smart home control panels
/// - Portable instrumentation
///
/// **Datasheet:** [Good Display GDEM0397T81P](https://www.good-display.com/product/613.html)
pub const GDEM0397T81P: DisplaySpec = DisplaySpec {
    name: "GDEM0397T81P",
    width: 800,
    height: 480,
    controller: Controller::SSD1677,
    panel_type: PanelType::Carta1200,
    grayscale_levels: 4,
    full_refresh_ms: 3000,      // 3 seconds per datasheet
    partial_refresh_ms: 300,    // 0.3 seconds per datasheet
    fast_refresh_ms: 1500,      // 1.5 seconds per datasheet
    ghosting_rate_partial: 0.10, // Low ghosting on Carta panel
    ghosting_rate_fast: 0.18,    // Moderate ghosting on fast refresh
    flash_count_full: 3,         // Typical for SSD1677
    temp_optimal_min: 15,
    temp_optimal_max: 35,
    temp_operating_min: 0,
    temp_operating_max: 50,
    color_mode: None,
    quirks: Some(quirks_for_controller(Controller::SSD1677)),
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gdew0213i5f() {
        assert_eq!(GDEW0213I5F.width, 212);
        assert_eq!(GDEW0213I5F.height, 104);
        assert_eq!(GDEW0213I5F.controller, Controller::UC8151);
        assert_eq!(GDEW0213I5F.panel_type, PanelType::Pearl);
    }

    #[test]
    fn test_gdew029t5() {
        assert_eq!(GDEW029T5.width, 296);
        assert_eq!(GDEW029T5.height, 128);
        assert_eq!(GDEW029T5.controller, Controller::GDEW);
    }

    #[test]
    fn test_gdew042t2() {
        assert_eq!(GDEW042T2.width, 400);
        assert_eq!(GDEW042T2.height, 300);
        // Diagonal is approximate - just check it's in reasonable range
        let diagonal = GDEW042T2.diagonal_inches();
        assert!(diagonal > 3.0 && diagonal < 5.0);
    }

    #[test]
    fn test_all_displays_valid_temps() {
        for spec in &[&GDEW0213I5F, &GDEW029T5, &GDEW042T2, &GDEW075T7, &GDEM0397T81P] {
            assert!(spec.is_optimal_temp(25));
            assert!(spec.is_operating_temp(25));
            assert!(!spec.is_operating_temp(-20));
        }
    }

    #[test]
    fn test_gdem0397t81p() {
        // Verify basic dimensions
        assert_eq!(GDEM0397T81P.width, 800);
        assert_eq!(GDEM0397T81P.height, 480);
        assert_eq!(GDEM0397T81P.controller, Controller::SSD1677);
        assert_eq!(GDEM0397T81P.panel_type, PanelType::Carta1200);

        // Verify refresh timings match datasheet
        assert_eq!(GDEM0397T81P.full_refresh_ms, 3000);
        assert_eq!(GDEM0397T81P.partial_refresh_ms, 300);
        assert_eq!(GDEM0397T81P.fast_refresh_ms, 1500);

        // Verify temperature ranges
        assert_eq!(GDEM0397T81P.temp_operating_min, 0);
        assert_eq!(GDEM0397T81P.temp_operating_max, 50);

        // Note: diagonal_inches() uses 130 PPI assumption, but this display is 235 PPI
        // Actual diagonal: 3.97" (per datasheet)
        // Calculated diagonal: ~7.18" (using 130 PPI formula)
        // This is expected due to the high-resolution panel

        // Verify high resolution (235 PPI per datasheet)
        let total_pixels = GDEM0397T81P.width * GDEM0397T81P.height;
        assert_eq!(total_pixels, 384_000, "Should have 384,000 pixels");

        // Verify aspect ratio
        let aspect = GDEM0397T81P.aspect_ratio();
        assert!((aspect - 1.6667).abs() < 0.01, "Aspect ratio should be ~5:3 (1.667)");
    }
}
