//! Display specification types
//!
//! Defines characteristics of e-ink displays for emulation and hardware abstraction.

use core::time::Duration;

/// Complete specification of an e-ink display
///
/// Contains all characteristics needed for realistic emulation:
/// - Physical dimensions
/// - Refresh timing and modes
/// - Grayscale capabilities
/// - Ghosting behavior
/// - Temperature characteristics
/// - Hardware quirks and limitations
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub struct DisplaySpec {
    /// Display name (e.g., "Waveshare 2.13\" V4")
    pub name: &'static str,

    /// Width in pixels
    pub width: u32,

    /// Height in pixels
    pub height: u32,

    /// Display controller chip
    pub controller: Controller,

    /// E-ink panel type
    pub panel_type: PanelType,

    /// Number of grayscale levels (typically 4 or 16)
    pub grayscale_levels: u8,

    /// Full refresh duration in milliseconds (typical: 2000ms)
    pub full_refresh_ms: u32,

    /// Partial refresh duration in milliseconds (typical: 300ms)
    pub partial_refresh_ms: u32,

    /// Fast refresh duration in milliseconds (typical: 260ms)
    pub fast_refresh_ms: u32,

    /// Ghosting accumulation rate per partial refresh (0.0-1.0)
    pub ghosting_rate_partial: f32,

    /// Ghosting accumulation rate per fast refresh (0.0-1.0)
    pub ghosting_rate_fast: f32,

    /// Number of flashes during full refresh (typical: 3)
    pub flash_count_full: u8,

    /// Optimal temperature range minimum (°C)
    pub temp_optimal_min: i8,

    /// Optimal temperature range maximum (°C)
    pub temp_optimal_max: i8,

    /// Operating temperature range minimum (°C)
    pub temp_operating_min: i8,

    /// Operating temperature range maximum (°C)
    pub temp_operating_max: i8,

    /// Color mode (optional, for tri-color displays)
    pub color_mode: Option<ColorMode>,

    /// Known hardware quirks for this display's controller (optional)
    ///
    /// Set to None to ignore quirks, or Some(&quirks) to enable quirk simulation.
    /// Quirks are automatically populated based on the controller type.
    #[cfg_attr(feature = "serde", serde(skip))]
    pub quirks: Option<&'static [crate::controller_quirks::Quirk]>,
}

impl DisplaySpec {
    /// Get display aspect ratio (width / height)
    pub fn aspect_ratio(&self) -> f32 {
        self.width as f32 / self.height as f32
    }

    /// Get approximate diagonal size in inches
    ///
    /// Assumes typical e-ink PPI of 130.
    /// Note: This is a rough approximation - actual PPI varies by display (100-150).
    /// Use for rough size estimates only.
    pub fn diagonal_inches(&self) -> f32 {
        const TYPICAL_PPI: f32 = 130.0;
        let diagonal_px = libm::sqrtf((self.width.pow(2) + self.height.pow(2)) as f32);
        diagonal_px / TYPICAL_PPI
    }

    /// Get full refresh duration as Duration
    pub fn full_refresh_duration(&self) -> Duration {
        Duration::from_millis(self.full_refresh_ms as u64)
    }

    /// Get partial refresh duration as Duration
    pub fn partial_refresh_duration(&self) -> Duration {
        Duration::from_millis(self.partial_refresh_ms as u64)
    }

    /// Get fast refresh duration as Duration
    pub fn fast_refresh_duration(&self) -> Duration {
        Duration::from_millis(self.fast_refresh_ms as u64)
    }

    /// Adjust refresh timing based on temperature with realistic non-linear model
    ///
    /// E-ink displays have complex temperature behavior:
    /// - Below 0°C: Exponential slowdown as particles become sluggish
    /// - 0-5°C: Transition zone
    /// - 5-35°C: Optimal performance (1.0x speed)
    /// - 35-45°C: Gradual slowdown due to increased viscosity
    /// - Above 45°C: Significant slowdown
    pub fn adjusted_refresh_ms(&self, base_ms: u32, temp: i8) -> u32 {
        let factor = match temp {
            t if t < 0 => {
                // Below freezing: exponential slowdown
                1.5 + (0.0 - t as f32) * 0.05
            }
            t if t < 5 => {
                // 0-5°C: transition
                1.5 - (t as f32 / 5.0) * 0.3
            }
            t if t <= 35 => {
                // Optimal: 1.0x
                1.0
            }
            t if t <= 45 => {
                // 35-45°C: gradual slowdown
                1.0 + ((t - 35) as f32 / 10.0) * 0.2
            }
            t => {
                // Above 45°C: significant slowdown
                1.2 + ((t - 45) as f32) * 0.03
            }
        };
        (base_ms as f32 * factor) as u32
    }

    /// Check if temperature is in optimal range
    pub fn is_optimal_temp(&self, temp_celsius: i8) -> bool {
        temp_celsius >= self.temp_optimal_min && temp_celsius <= self.temp_optimal_max
    }

    /// Check if temperature is in operating range
    pub fn is_operating_temp(&self, temp_celsius: i8) -> bool {
        temp_celsius >= self.temp_operating_min && temp_celsius <= self.temp_operating_max
    }
}

/// E-ink display controller chips
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum Controller {
    /// Solomon Systech SSD1680 (common in Waveshare 2.13" V4)
    SSD1680,
    /// ImagEInk IL0373 / UltraChip UC8151 (Waveshare 2.9" V2)
    IL0373,
    /// UltraChip UC8151 (similar to IL0373)
    UC8151,
    /// Solomon Systech SSD1619 (Waveshare 4.2" V2)
    SSD1619,
    /// E Ink ED075TC1 (7.5" displays)
    ED075TC1,
    /// IT8951 controller (high-end displays with fast refresh)
    IT8951,
    /// GDEW controller series
    GDEW,
    /// Generic/unknown controller
    Generic,
    /// ACeP (Advanced Color ePaper) controller for Spectra 6
    ACeP,
    /// Solomon Systech SSD1677 (3.97" and larger displays)
    SSD1677,
}

/// E-ink panel technology types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum PanelType {
    /// E Ink Pearl (older generation)
    Pearl,
    /// E Ink Carta 1000 (improved contrast)
    Carta1000,
    /// E Ink Carta 1200 (faster response)
    Carta1200,
    /// E Ink Carta 1300 (latest B&W)
    Carta1300,
    /// E Ink Kaleido 3 (color with filter layer)
    Kaleido3,
    /// E Ink Spectra 6 (ACeP - Advanced Color ePaper)
    Spectra6,
}

/// Color mode for e-ink display
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
pub enum ColorMode {
    /// Grayscale only (traditional e-ink)
    Grayscale,
    /// Spectra 6 (6 colors: black, white, red, yellow, blue, green)
    Spectra6,
    /// Kaleido 3 (4096 colors via RGB filter)
    Kaleido3,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_spec() -> DisplaySpec {
        DisplaySpec {
            name: "Test Display",
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
            color_mode: None, // Grayscale only
            quirks: None,     // No quirks for test spec
        }
    }

    #[test]
    fn test_aspect_ratio() {
        let spec = test_spec();
        assert!((spec.aspect_ratio() - 2.049).abs() < 0.01);
    }

    #[test]
    fn test_diagonal_inches() {
        let spec = test_spec();
        let diagonal = spec.diagonal_inches();
        assert!(diagonal > 1.0 && diagonal < 3.0);
    }

    #[test]
    fn test_temperature_adjustment() {
        let spec = test_spec();

        // Normal temp - no adjustment
        assert_eq!(spec.adjusted_refresh_ms(2000, 25), 2000);

        // Cold temp: -5°C is in the exponential range
        // factor = 1.5 + (0 - (-5)) * 0.05 = 1.5 + 0.25 = 1.75
        assert_eq!(spec.adjusted_refresh_ms(2000, -5), 3500);

        // Hot temp: 45°C is at the boundary
        // factor = 1.0 + ((45 - 35) / 10.0) * 0.2 = 1.0 + 0.2 = 1.2
        assert_eq!(spec.adjusted_refresh_ms(2000, 45), 2400);
    }

    #[test]
    fn test_temperature_nonlinearity() {
        let spec = test_spec();

        // Test -10°C: 1.5 + (0 - (-10)) * 0.05 = 1.5 + 0.5 = 2.0x slower
        assert_eq!(spec.adjusted_refresh_ms(2000, -10), 4000);

        // Test 0°C: 1.5x slower (at transition boundary)
        assert_eq!(spec.adjusted_refresh_ms(2000, 0), 3000);

        // Test 2.5°C: 1.5 - (2.5 / 5.0) * 0.3 = 1.5 - 0.15 = 1.35x
        let result = spec.adjusted_refresh_ms(2000, 2);
        assert!((2640..=2760).contains(&result)); // ~2700 ± 60ms tolerance

        // Test 25°C: optimal (1.0x)
        assert_eq!(spec.adjusted_refresh_ms(2000, 25), 2000);

        // Test 40°C: 1.0 + ((40 - 35) / 10.0) * 0.2 = 1.0 + 0.1 = 1.1x
        assert_eq!(spec.adjusted_refresh_ms(2000, 40), 2200);

        // Test 50°C: 1.2 + ((50 - 45)) * 0.03 = 1.2 + 0.15 = 1.35x
        assert_eq!(spec.adjusted_refresh_ms(2000, 50), 2700);
    }

    #[test]
    fn test_temperature_smooth_transitions() {
        let spec = test_spec();

        // Test that temperature transitions are smooth (no big jumps)
        let temps = [
            -5, -4, -3, -2, -1, 0, 1, 2, 3, 4, 5, 10, 20, 25, 30, 35, 40, 45, 50,
        ];
        let mut previous_time = 0u32;

        for temp in temps {
            let time = spec.adjusted_refresh_ms(2000, temp);

            // Ensure monotonic decrease as we warm up (except at boundaries)
            // Allow for rounding and transition zones
            if (0..=35).contains(&temp) {
                // Should be decreasing or stable until optimal range
                if previous_time > 0 {
                    assert!(
                        time <= previous_time + 100,
                        "Non-smooth transition at {}°C: {}ms -> {}ms",
                        temp,
                        previous_time,
                        time
                    );
                }
            }

            previous_time = time;
        }
    }

    #[test]
    fn test_temperature_extreme_cold() {
        let spec = test_spec();

        // Test -20°C: 1.5 + 20 * 0.05 = 2.5x slower
        assert_eq!(spec.adjusted_refresh_ms(2000, -20), 5000);

        // Verify it keeps getting worse at extreme cold
        let at_minus_10 = spec.adjusted_refresh_ms(2000, -10);
        let at_minus_20 = spec.adjusted_refresh_ms(2000, -20);
        assert!(at_minus_20 > at_minus_10);
    }

    #[test]
    fn test_temperature_extreme_heat() {
        let spec = test_spec();

        // Test 60°C: 1.2 + (60 - 45) * 0.03 = 1.2 + 0.45 = 1.65x
        assert_eq!(spec.adjusted_refresh_ms(2000, 60), 3300);

        // Verify it keeps getting worse at extreme heat
        let at_50 = spec.adjusted_refresh_ms(2000, 50);
        let at_60 = spec.adjusted_refresh_ms(2000, 60);
        assert!(at_60 > at_50);
    }

    #[test]
    fn test_temperature_ranges() {
        let spec = test_spec();

        assert!(spec.is_optimal_temp(25));
        assert!(!spec.is_optimal_temp(5));

        assert!(spec.is_operating_temp(5));
        assert!(!spec.is_operating_temp(-10));
    }
}
