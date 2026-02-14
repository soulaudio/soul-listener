//! Per-Pixel State Tracking
//!
//! Implements robust e-ink physics simulation with content-dependent ghosting,
//! DC balance tracking, and particle state modeling.

use crate::lut::WaveformLut;
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::GrayColor;

/// Physical state of a single e-ink pixel
///
/// Tracks all the stateful properties needed for realistic simulation:
/// - Current and previous grayscale levels
/// - Ghosting accumulation
/// - DC balance (net voltage applied)
/// - Particle charge state
/// - Color-specific state (optional for tri-color displays)
#[derive(Debug, Clone, Copy)]
pub struct PixelState {
    /// Current grayscale level (0-15, 4-bit)
    pub current: u8,

    /// Previous grayscale level (0-15, 4-bit)
    ///
    /// Used for content-dependent ghosting calculation.
    /// Larger transitions accumulate more ghosting.
    pub previous: u8,

    /// Ghosting level for this pixel (0.0 - 1.0)
    ///
    /// Represents residual charge from previous images.
    /// Accumulated through partial refreshes, cleared by full refresh.
    pub ghosting: f32,

    /// DC balance tracker (-100.0 to 100.0)
    ///
    /// Tracks net voltage applied to this pixel over time.
    /// Must be kept near zero to prevent permanent damage.
    /// Positive = net positive voltage, Negative = net negative voltage.
    pub dc_balance: f32,

    /// Number of refreshes since last full refresh
    ///
    /// Used to determine when mandatory full refresh is needed.
    pub refresh_count: u16,

    /// Last transition direction (-1 = darkening, 0 = none, +1 = lightening)
    ///
    /// Used for ghosting asymmetry and momentum effect.
    /// Lightening transitions (0→15) accumulate more ghosting than darkening (15→0).
    pub last_transition_direction: i8,

    /// Color-specific state (optional, for tri-color displays)
    pub color_state: Option<ColorPixelState>,
}

/// Color-specific pixel state for tri-color displays
#[derive(Debug, Clone, Copy)]
pub struct ColorPixelState {
    /// Red pigment level (0.0-1.0) for Spectra6
    pub red_pigment: f32,
    /// Yellow pigment level (0.0-1.0) for Spectra6
    pub yellow_pigment: f32,
    /// Blue pigment level (0.0-1.0) for Spectra6
    pub blue_pigment: f32,
    /// Color-specific ghosting (color particles ghost at 2x rate)
    pub color_ghosting: f32,
}

impl ColorPixelState {
    /// Create new color pixel state
    pub fn new() -> Self {
        Self {
            red_pigment: 0.0,
            yellow_pigment: 0.0,
            blue_pigment: 0.0,
            color_ghosting: 0.0,
        }
    }

    /// Update color pigments for refresh
    pub fn update_pigments(&mut self, target_red: f32, target_yellow: f32, target_blue: f32) {
        self.red_pigment = target_red.clamp(0.0, 1.0);
        self.yellow_pigment = target_yellow.clamp(0.0, 1.0);
        self.blue_pigment = target_blue.clamp(0.0, 1.0);
    }

    /// Full refresh clears color ghosting
    pub fn full_refresh(&mut self) {
        self.color_ghosting = 0.0;
    }

    /// Partial refresh accumulates color ghosting at 2x rate
    pub fn partial_refresh(&mut self, ghosting_rate: f32) {
        // Color particles are larger and ghost more
        self.color_ghosting = (self.color_ghosting + ghosting_rate * 2.0).min(1.0);
    }
}

impl Default for ColorPixelState {
    fn default() -> Self {
        Self::new()
    }
}

impl PixelState {
    /// Create new pixel in initial state (white, no ghosting)
    pub fn new() -> Self {
        Self {
            current: 0, // White (no ink particles up)
            previous: 0,
            ghosting: 0.0,
            dc_balance: 0.0,
            refresh_count: 0,
            last_transition_direction: 0,
            color_state: None,
        }
    }

    /// Create new pixel with color support
    pub fn new_with_color() -> Self {
        Self {
            current: 0,
            previous: 0,
            ghosting: 0.0,
            dc_balance: 0.0,
            refresh_count: 0,
            last_transition_direction: 0,
            color_state: Some(ColorPixelState::new()),
        }
    }

    /// Update pixel with full refresh (GC16/GL16)
    ///
    /// Clears ghosting completely and applies DC balancing waveform.
    pub fn full_refresh(&mut self, target: u8) {
        self.previous = self.current;
        self.current = target.min(15);
        self.ghosting = 0.0;
        self.refresh_count = 0;

        // Full refresh includes DC balancing phases
        // Black → White → Target sequence neutralizes accumulated charge
        self.dc_balance *= 0.1; // 90% reduction

        // Clear color ghosting if present
        if let Some(ref mut color) = self.color_state {
            color.full_refresh();
        }
    }

    /// Update pixel with partial refresh using custom LUT
    ///
    /// Uses LUT-derived ghosting and DC balance characteristics.
    pub fn partial_refresh_with_lut(
        &mut self,
        target: u8,
        lut: &WaveformLut,
        temperature: i8,
    ) {
        let target = target.min(15);

        // Use LUT data to calculate ghosting and DC balance
        let ghosting_from_lut = lut.ghosting_contribution();
        let dc_from_lut = lut.dc_balance();

        // Calculate transition
        let transition = (target as i16 - self.current as i16) as f32 / 15.0;

        // Temperature factor based on LUT's temperature range
        let temp_factor = if temperature < lut.temperature_range.0
            || temperature > lut.temperature_range.1
        {
            0.85 // Reduced effectiveness outside temp range
        } else {
            1.0
        };

        // Content-dependent ghosting from transition magnitude
        let content_ghosting = ghosting_from_lut * transition.abs() * temp_factor;

        self.previous = self.current;
        self.current = (self.current as f32 + transition * 15.0 * temp_factor).round() as u8;
        self.current = self.current.min(15);
        self.ghosting = (self.ghosting + content_ghosting).min(1.0);
        self.refresh_count += 1;

        // DC balance from LUT
        self.dc_balance += dc_from_lut * transition.abs();
    }

    /// Update pixel with partial refresh (DU4)
    ///
    /// Fast update with content-dependent ghosting accumulation.
    /// Uses asymmetric ghosting: lightening (0→15) accumulates more than darkening (15→0).
    pub fn partial_refresh(&mut self, target: u8, ghosting_rate: f32, temperature: i8) {
        let target = target.min(15);

        // Calculate transition (positive = lightening, negative = darkening)
        let transition = (target as i16 - self.current as i16) as f32 / 15.0;

        // Asymmetric ghosting based on direction
        let direction_factor = if transition > 0.0 {
            1.2 // Lightening (0→15): 20% MORE ghosting
        } else if transition < 0.0 {
            0.9 // Darkening (15→0): 10% LESS ghosting
        } else {
            1.0 // No change
        };

        // Temperature-dependent ghosting factor
        let temp_factor = Self::temperature_ghosting_factor(temperature);

        // Content-dependent ghosting: larger transitions accumulate more
        let mut content_ghosting = ghosting_rate
            * transition.abs()
            * (1.0 + self.ghosting * 0.5)
            * direction_factor
            * temp_factor;

        // Momentum effect: consecutive same-direction transitions accumulate less ghosting
        if self.last_transition_direction != 0
            && self.last_transition_direction == transition.signum() as i8
        {
            content_ghosting *= 0.85; // 15% reduction for momentum
        }

        self.previous = self.current;
        self.current = target;
        self.ghosting = (self.ghosting + content_ghosting).min(1.0);
        self.refresh_count += 1;
        self.last_transition_direction = if transition == 0.0 {
            0
        } else {
            transition.signum() as i8
        };

        // Partial refresh adds DC imbalance due to simplified waveform
        // Even symmetric transitions (0→15→0) accumulate some imbalance
        // because partial refreshes don't use the full balancing sequence
        let voltage_delta = (target as f32 - self.previous as f32) / 15.0;
        let transition_magnitude = voltage_delta.abs();

        // Accumulate both signed voltage and magnitude-based aging
        // Higher magnitude weight to simulate cumulative stress even with balanced voltages
        self.dc_balance += voltage_delta * 1.0 + transition_magnitude * 1.5;

        // Update color ghosting if this is a color pixel (2× accumulation rate)
        if let Some(ref mut color) = self.color_state {
            color.partial_refresh(content_ghosting);
        }
    }

    /// Update pixel with fast refresh (DU/A2)
    ///
    /// Ultra-fast update with high ghosting accumulation.
    /// Uses asymmetric ghosting: lightening (0→15) accumulates more than darkening (15→0).
    pub fn fast_refresh(&mut self, target: u8, ghosting_rate: f32, temperature: i8) {
        let target = target.min(15);

        // Calculate transition (positive = lightening, negative = darkening)
        let transition = (target as i16 - self.current as i16) as f32 / 15.0;

        // Asymmetric ghosting based on direction
        let direction_factor = if transition > 0.0 {
            1.2 // Lightening (0→15): 20% MORE ghosting
        } else if transition < 0.0 {
            0.9 // Darkening (15→0): 10% LESS ghosting
        } else {
            1.0 // No change
        };

        // Temperature-dependent ghosting factor
        let temp_factor = Self::temperature_ghosting_factor(temperature);

        // Fast modes have even higher content-dependent ghosting
        let mut content_ghosting = ghosting_rate
            * transition.abs()
            * (1.0 + self.ghosting)
            * direction_factor
            * temp_factor;

        // Momentum effect: consecutive same-direction transitions accumulate less ghosting
        if self.last_transition_direction != 0
            && self.last_transition_direction == transition.signum() as i8
        {
            content_ghosting *= 0.85; // 15% reduction for momentum
        }

        self.previous = self.current;
        self.current = target;
        self.ghosting = (self.ghosting + content_ghosting).min(1.0);
        self.refresh_count += 1;
        self.last_transition_direction = if transition == 0.0 {
            0
        } else {
            transition.signum() as i8
        };

        // Fast refresh: highly unbalanced DC due to minimal waveform
        let voltage_delta = (target as f32 - self.previous as f32) / 15.0;
        let transition_magnitude = voltage_delta.abs();

        // Even higher DC imbalance for fast modes
        self.dc_balance += voltage_delta * 2.0 + transition_magnitude * 1.0;

        // Update color ghosting if this is a color pixel (2× accumulation rate)
        if let Some(ref mut color) = self.color_state {
            color.partial_refresh(content_ghosting);
        }
    }

    /// Get effective gray level with ghosting applied
    ///
    /// Blends current with previous based on ghosting level.
    /// This simulates incomplete particle transition.
    pub fn effective_gray(&self) -> u8 {
        let current_f = self.current as f32;
        let previous_f = self.previous as f32;

        // Blend current with previous weighted by ghosting
        let blended = current_f * (1.0 - self.ghosting) + previous_f * self.ghosting;

        blended.round().min(15.0) as u8
    }

    /// Get effective gray level as Gray4 color
    pub fn effective_color(&self) -> Gray4 {
        let gray_15 = self.effective_gray(); // 0-15 range
                                             // Convert to 0-3 range for Gray4
        Gray4::new(gray_15 / 5)
    }

    /// Check if DC balance is critically high
    ///
    /// Returns true if pixel needs full refresh to prevent damage.
    pub fn dc_critical(&self) -> bool {
        self.dc_balance.abs() > 50.0
    }

    /// Check if mandatory full refresh is needed
    ///
    /// Based on refresh count and DC balance.
    pub fn needs_full_refresh(&self, threshold: u16) -> bool {
        self.refresh_count >= threshold || self.dc_critical()
    }

    /// Get ghosting level as percentage (0-100)
    pub fn ghosting_percent(&self) -> f32 {
        self.ghosting * 100.0
    }
    /// Calculate temperature-dependent ghosting factor
    ///
    /// Returns multiplier for ghosting accumulation based on temperature:
    /// - Cold (<5°C): 1.3x more ghosting (particles sluggish)
    /// - Hot (>40°C): 1.2x more ghosting (increased particle diffusion)
    /// - Optimal (5-40°C): 1.0x (normal ghosting)
    fn temperature_ghosting_factor(temperature: i8) -> f32 {
        match temperature {
            t if t < 5 => 1.3,  // Cold: 30% more ghosting
            t if t > 40 => 1.2, // Hot: 20% more ghosting
            _ => 1.0,           // Optimal range
        }
    }
}

impl Default for PixelState {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffer of pixel states for entire display
pub struct PixelStateBuffer {
    states: Vec<PixelState>,
    width: u32,
    height: u32,
}

impl PixelStateBuffer {
    /// Create new pixel state buffer
    pub fn new(width: u32, height: u32) -> Self {
        let size = (width * height) as usize;
        Self {
            states: vec![PixelState::new(); size],
            width,
            height,
        }
    }

    /// Get pixel state at position
    pub fn get(&self, x: u32, y: u32) -> Option<&PixelState> {
        if x < self.width && y < self.height {
            let index = (x + y * self.width) as usize;
            self.states.get(index)
        } else {
            None
        }
    }

    /// Get mutable pixel state at position
    pub fn get_mut(&mut self, x: u32, y: u32) -> Option<&mut PixelState> {
        if x < self.width && y < self.height {
            let index = (x + y * self.width) as usize;
            self.states.get_mut(index)
        } else {
            None
        }
    }

    /// Get average ghosting level across all pixels
    pub fn average_ghosting(&self) -> f32 {
        let sum: f32 = self.states.iter().map(|s| s.ghosting).sum();
        sum / self.states.len() as f32
    }

    /// Get maximum ghosting level
    pub fn max_ghosting(&self) -> f32 {
        self.states
            .iter()
            .map(|s| s.ghosting)
            .fold(0.0f32, |a, b| a.max(b))
    }

    /// Get average DC balance (absolute value)
    pub fn average_dc_balance(&self) -> f32 {
        let sum: f32 = self.states.iter().map(|s| s.dc_balance.abs()).sum();
        sum / self.states.len() as f32
    }

    /// Get maximum DC balance (absolute value)
    pub fn max_dc_balance(&self) -> f32 {
        self.states
            .iter()
            .map(|s| s.dc_balance.abs())
            .fold(0.0f32, |a, b| a.max(b))
    }

    /// Count pixels with critical DC balance
    pub fn dc_critical_count(&self) -> usize {
        self.states.iter().filter(|s| s.dc_critical()).count()
    }

    /// Check if any pixel needs full refresh
    pub fn needs_full_refresh(&self, threshold: u16) -> bool {
        self.states.iter().any(|s| s.needs_full_refresh(threshold))
    }

    /// Full refresh all pixels
    pub fn full_refresh_all(&mut self, framebuffer: &[Gray4]) {
        for (i, state) in self.states.iter_mut().enumerate() {
            // Convert Gray4 luma (0-3) to 0-15 range for pixel state
            let luma = framebuffer.get(i).map(|c| c.luma()).unwrap_or(0);
            let target = luma * 5; // 0,1,2,3 → 0,5,10,15
            state.full_refresh(target);
        }
    }

    /// Partial refresh all pixels
    pub fn partial_refresh_all(
        &mut self,
        framebuffer: &[Gray4],
        ghosting_rate: f32,
        temperature: i8,
    ) {
        for (i, state) in self.states.iter_mut().enumerate() {
            // Convert Gray4 luma (0-3) to 0-15 range for pixel state
            let luma = framebuffer.get(i).map(|c| c.luma()).unwrap_or(0);
            let target = luma * 5; // 0,1,2,3 → 0,5,10,15
            state.partial_refresh(target, ghosting_rate, temperature);
        }
    }

    /// Fast refresh all pixels
    pub fn fast_refresh_all(&mut self, framebuffer: &[Gray4], ghosting_rate: f32, temperature: i8) {
        for (i, state) in self.states.iter_mut().enumerate() {
            // Convert Gray4 luma (0-3) to 0-15 range for pixel state
            let luma = framebuffer.get(i).map(|c| c.luma()).unwrap_or(0);
            let target = luma * 5; // 0,1,2,3 → 0,5,10,15
            state.fast_refresh(target, ghosting_rate, temperature);
        }
    }

    /// Get effective framebuffer with ghosting applied
    pub fn effective_framebuffer(&self) -> Vec<Gray4> {
        self.states.iter().map(|s| s.effective_color()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pixel_state_creation() {
        let pixel = PixelState::new();
        assert_eq!(pixel.current, 0);
        assert_eq!(pixel.previous, 0);
        assert_eq!(pixel.ghosting, 0.0);
        assert_eq!(pixel.dc_balance, 0.0);
    }

    #[test]
    fn test_full_refresh_clears_ghosting() {
        let mut pixel = PixelState::new();

        // Accumulate some ghosting
        pixel.partial_refresh(15, 0.15, 25);
        pixel.partial_refresh(0, 0.15, 25);
        assert!(pixel.ghosting > 0.0);

        // Full refresh should clear it
        pixel.full_refresh(8);
        assert_eq!(pixel.ghosting, 0.0);
        assert_eq!(pixel.current, 8);
        assert_eq!(pixel.refresh_count, 0);
    }

    #[test]
    fn test_content_dependent_ghosting() {
        let mut pixel1 = PixelState::new();
        let mut pixel2 = PixelState::new();

        // Small transition (0 → 2)
        pixel1.partial_refresh(2, 0.15, 25);
        let ghosting_small = pixel1.ghosting;

        // Large transition (0 → 15)
        pixel2.partial_refresh(15, 0.15, 25);
        let ghosting_large = pixel2.ghosting;

        // Larger transitions should accumulate more ghosting
        assert!(ghosting_large > ghosting_small);
    }

    #[test]
    fn test_dc_balance_accumulation() {
        let mut pixel = PixelState::new();

        // Multiple partial refreshes accumulate DC imbalance
        for _ in 0..10 {
            pixel.partial_refresh(15, 0.15, 25);
            pixel.partial_refresh(0, 0.15, 25);
        }

        assert!(pixel.dc_balance.abs() > 0.0);
        assert!(!pixel.dc_critical()); // Shouldn't be critical yet

        // Full refresh reduces DC imbalance
        let dc_before = pixel.dc_balance.abs();
        pixel.full_refresh(8);
        let dc_after = pixel.dc_balance.abs();

        assert!(dc_after < dc_before);
    }

    #[test]
    fn test_effective_gray_blending() {
        let mut pixel = PixelState::new();
        pixel.current = 0; // White
        pixel.previous = 15; // Black
        pixel.ghosting = 0.5; // 50% ghosting

        let effective = pixel.effective_gray();

        // Should be blend of 0 and 15 with 50% ghosting
        // effective = 0 * (1 - 0.5) + 15 * 0.5 = 7.5 ≈ 8
        assert!(effective >= 7 && effective <= 8);
    }

    #[test]
    fn test_pixel_state_buffer() {
        let mut buffer = PixelStateBuffer::new(10, 10);

        assert_eq!(buffer.average_ghosting(), 0.0);
        assert_eq!(buffer.max_ghosting(), 0.0);

        // Update some pixels
        if let Some(pixel) = buffer.get_mut(5, 5) {
            pixel.partial_refresh(15, 0.15, 25);
        }

        assert!(buffer.average_ghosting() > 0.0);
        assert!(buffer.max_ghosting() > 0.0);
    }

    #[test]
    fn test_mandatory_full_refresh() {
        let mut pixel = PixelState::new();

        // Not needed initially
        assert!(!pixel.needs_full_refresh(5));

        // After threshold refreshes, should be needed
        for _ in 0..5 {
            pixel.partial_refresh(15, 0.15, 25);
        }

        assert!(pixel.needs_full_refresh(5));

        // Or if DC is critical
        pixel.refresh_count = 0;
        pixel.dc_balance = 60.0; // Above threshold
        assert!(pixel.needs_full_refresh(100));
    }

    // Ghosting Asymmetry Tests

    #[test]
    fn test_lightening_has_more_ghosting_than_darkening() {
        let mut pixel_lighten = PixelState::new();
        let mut pixel_darken = PixelState::new();

        // Lightening: 0 → 15
        pixel_lighten.partial_refresh(15, 0.15, 25);
        let ghosting_lighten = pixel_lighten.ghosting;

        // Darkening: 15 → 0
        pixel_darken.current = 15;
        pixel_darken.previous = 15;
        pixel_darken.partial_refresh(0, 0.15, 25);
        let ghosting_darken = pixel_darken.ghosting;

        // Lightening should have MORE ghosting than darkening
        assert!(
            ghosting_lighten > ghosting_darken,
            "Lightening ghosting ({}) should be greater than darkening ghosting ({})",
            ghosting_lighten,
            ghosting_darken
        );
    }

    #[test]
    fn test_ghosting_asymmetry_ratio() {
        let mut pixel_lighten = PixelState::new();
        let mut pixel_darken = PixelState::new();

        // Lightening: 0 → 15
        pixel_lighten.partial_refresh(15, 0.15, 25);
        let ghosting_lighten = pixel_lighten.ghosting;

        // Darkening: 15 → 0
        pixel_darken.current = 15;
        pixel_darken.previous = 15;
        pixel_darken.partial_refresh(0, 0.15, 25);
        let ghosting_darken = pixel_darken.ghosting;

        // Expected ratio: 1.2 / 0.9 = 1.333...
        let ratio = ghosting_lighten / ghosting_darken;
        let expected_ratio = 1.2 / 0.9;

        // Allow 1% tolerance for floating point arithmetic
        assert!(
            (ratio - expected_ratio).abs() < 0.01 * expected_ratio,
            "Ghosting ratio ({:.3}) should be approximately {:.3}",
            ratio,
            expected_ratio
        );
    }

    #[test]
    fn test_momentum_reduces_ghosting() {
        let mut pixel_no_momentum = PixelState::new();
        let mut pixel_with_momentum = PixelState::new();

        // No momentum: do a different transition first (0→8) then test transition 8→15
        pixel_no_momentum.partial_refresh(0, 0.15, 25); // Reset
        pixel_no_momentum.partial_refresh(8, 0.15, 25); // First transition (different direction marker)
        let before_no_momentum = pixel_no_momentum.ghosting;
        pixel_no_momentum.last_transition_direction = -1; // Fake different direction
        pixel_no_momentum.partial_refresh(15, 0.15, 25); // 8→15 without momentum benefit
        let ghosting_no_momentum = pixel_no_momentum.ghosting - before_no_momentum;

        // With momentum: 0 → 8 → 15 (same direction)
        pixel_with_momentum.partial_refresh(8, 0.15, 25);
        let before_momentum = pixel_with_momentum.ghosting;
        pixel_with_momentum.partial_refresh(15, 0.15, 25); // 8→15 with momentum benefit
        let ghosting_with_momentum = pixel_with_momentum.ghosting - before_momentum;

        // Momentum transition should accumulate less ghosting
        assert!(
            ghosting_with_momentum < ghosting_no_momentum,
            "Momentum ghosting ({}) should be less than no-momentum ({})",
            ghosting_with_momentum,
            ghosting_no_momentum
        );

        // Verify ~15% reduction from momentum effect
        let ratio = ghosting_with_momentum / ghosting_no_momentum;
        assert!(
            (ratio - 0.85).abs() < 0.05,
            "Momentum ratio ({}) should be ~0.85 (15% reduction)",
            ratio
        );
    }

    #[test]
    fn test_direction_changes_have_more_ghosting() {
        let mut pixel_same_dir = PixelState::new();
        let mut pixel_alt_dir = PixelState::new();

        // Same direction: 0 → 8 → 15 (both lightening)
        pixel_same_dir.partial_refresh(8, 0.15, 25);
        pixel_same_dir.partial_refresh(15, 0.15, 25);
        let ghosting_same = pixel_same_dir.ghosting;

        // Alternating direction: 0 → 15 → 8 (lighten then darken)
        pixel_alt_dir.partial_refresh(15, 0.15, 25);
        pixel_alt_dir.partial_refresh(8, 0.15, 25);
        let ghosting_alt = pixel_alt_dir.ghosting;

        // Alternating should have more ghosting (no momentum bonus)
        assert!(
            ghosting_alt > ghosting_same,
            "Alternating direction ghosting ({}) should be greater than same direction ({})",
            ghosting_alt,
            ghosting_same
        );
    }

    #[test]
    fn test_symmetric_vs_asymmetric_transitions() {
        let mut pixel_symmetric = PixelState::new();
        let mut pixel_asymmetric = PixelState::new();

        // Symmetric: 0 → 8 → 0 (equal up and down)
        pixel_symmetric.partial_refresh(8, 0.15, 25);
        pixel_symmetric.partial_refresh(0, 0.15, 25);
        let ghosting_symmetric = pixel_symmetric.ghosting;

        // Asymmetric: 0 → 15 → 0 (full range)
        pixel_asymmetric.partial_refresh(15, 0.15, 25);
        pixel_asymmetric.partial_refresh(0, 0.15, 25);
        let ghosting_asymmetric = pixel_asymmetric.ghosting;

        // Larger transitions should accumulate more ghosting
        assert!(
            ghosting_asymmetric > ghosting_symmetric,
            "Full range transition ghosting ({}) should be greater than half range ({})",
            ghosting_asymmetric,
            ghosting_symmetric
        );
    }

    #[test]
    fn test_fast_refresh_asymmetry() {
        let mut pixel_lighten = PixelState::new();
        let mut pixel_darken = PixelState::new();

        // Lightening: 0 → 15 (fast refresh)
        pixel_lighten.fast_refresh(15, 0.25, 25); // 25°C - normal temp
        let ghosting_lighten = pixel_lighten.ghosting;

        // Darkening: 15 → 0 (fast refresh)
        pixel_darken.current = 15;
        pixel_darken.previous = 15;
        pixel_darken.fast_refresh(0, 0.25, 25); // 25°C - normal temp
        let ghosting_darken = pixel_darken.ghosting;

        // Same asymmetry should apply to fast refresh
        assert!(
            ghosting_lighten > ghosting_darken,
            "Fast refresh lightening ghosting ({}) should be greater than darkening ({})",
            ghosting_lighten,
            ghosting_darken
        );

        // Verify ratio is still approximately 1.333
        let ratio = ghosting_lighten / ghosting_darken;
        let expected_ratio = 1.2 / 0.9;
        assert!(
            (ratio - expected_ratio).abs() < 0.01 * expected_ratio,
            "Fast refresh ratio ({:.3}) should match partial refresh ratio ({:.3})",
            ratio,
            expected_ratio
        );
    }

    #[test]
    fn test_transition_direction_tracking() {
        let mut pixel = PixelState::new();

        // Initially no direction
        assert_eq!(pixel.last_transition_direction, 0);

        // Lightening transition
        pixel.partial_refresh(15, 0.15, 25);
        assert_eq!(pixel.last_transition_direction, 1);

        // Darkening transition
        pixel.partial_refresh(0, 0.15, 25);
        assert_eq!(pixel.last_transition_direction, -1);

        // No transition
        pixel.partial_refresh(0, 0.15, 25);
        assert_eq!(pixel.last_transition_direction, 0);
    }
    #[test]
    fn test_temperature_ghosting_cold() {
        let mut pixel_cold = PixelState::new();
        let mut pixel_normal = PixelState::new();

        // Same transition at different temperatures
        pixel_cold.partial_refresh(15, 0.15, 0); // 0°C - cold
        pixel_normal.partial_refresh(15, 0.15, 25); // 25°C - optimal

        // Cold should have 30% more ghosting
        assert!(
            pixel_cold.ghosting > pixel_normal.ghosting * 1.2,
            "Cold temp ({}) should have more ghosting than normal ({})",
            pixel_cold.ghosting,
            pixel_normal.ghosting
        );
    }

    #[test]
    fn test_temperature_ghosting_hot() {
        let mut pixel_hot = PixelState::new();
        let mut pixel_normal = PixelState::new();

        // Same transition at different temperatures
        pixel_hot.fast_refresh(15, 0.25, 45); // 45°C - hot
        pixel_normal.fast_refresh(15, 0.25, 25); // 25°C - optimal

        // Hot should have 20% more ghosting
        assert!(
            pixel_hot.ghosting > pixel_normal.ghosting * 1.1,
            "Hot temp ({}) should have more ghosting than normal ({})",
            pixel_hot.ghosting,
            pixel_normal.ghosting
        );
    }

    #[test]
    fn test_temperature_ghosting_optimal() {
        let mut pixel1 = PixelState::new();
        let mut pixel2 = PixelState::new();

        // Optimal range (5-40°C) should have same factor
        pixel1.partial_refresh(15, 0.15, 10); // 10°C
        pixel2.partial_refresh(15, 0.15, 35); // 35°C

        // Should be very similar (both 1.0x factor)
        let diff = (pixel1.ghosting - pixel2.ghosting).abs();
        assert!(
            diff < 0.01,
            "Optimal temps should have similar ghosting: {} vs {}",
            pixel1.ghosting,
            pixel2.ghosting
        );
    }
}
