//! Power consumption graph
//!
//! Provides time-series tracking and visualization of e-ink display power consumption.
//! Uses a ring buffer to maintain the last 300 samples (approximately 5 minutes at 1 sample/second).

use super::state::{PowerSample, RefreshType};
use std::collections::VecDeque;
use std::time::Instant;

/// Maximum number of power samples to retain in the ring buffer
const MAX_SAMPLES: usize = 300; // 5 minutes at 1 sample/sec

/// Width of the rendered power graph in pixels
const GRAPH_WIDTH: u32 = 190;

/// Height of the rendered power graph in pixels
const GRAPH_HEIGHT: u32 = 100;

/// Power consumption graph with ring buffer storage
///
/// Tracks power consumption over time and provides rendering capabilities.
/// Power values are stored in milliwatts (mW).
///
/// # Example
///
/// ```ignore
/// let mut graph = PowerGraph::new();
///
/// // Add power samples
/// graph.add_sample(10.0, None);  // Idle power
/// graph.add_sample(210.0, Some(RefreshType::Full));  // Full refresh spike
///
/// // Query statistics
/// let current = graph.current_power();
/// let average = graph.average_power();
/// ```
pub struct PowerGraph {
    /// Ring buffer of power samples
    samples: VecDeque<PowerSample>,

    /// Baseline power consumption when idle (mW)
    baseline_power: f32,
}

impl PowerGraph {
    /// Creates a new PowerGraph with default baseline power
    ///
    /// The baseline power is set to 10mW (typical e-ink idle consumption).
    pub fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(MAX_SAMPLES),
            baseline_power: 10.0, // 10mW idle
        }
    }

    /// Adds a power sample to the ring buffer
    ///
    /// If the buffer exceeds MAX_SAMPLES, the oldest sample is removed.
    ///
    /// # Arguments
    ///
    /// * `power_mw` - Power consumption in milliwatts
    /// * `refresh_type` - Optional refresh type that caused this power draw
    ///
    /// # Example
    ///
    /// ```ignore
    /// graph.add_sample(50.0, Some(RefreshType::Partial));
    /// ```
    pub fn add_sample(&mut self, power_mw: f32, refresh_type: Option<RefreshType>) {
        let sample = PowerSample {
            timestamp: Instant::now(),
            power_mw,
            refresh_type,
        };

        self.samples.push_back(sample);

        // Remove old samples to maintain ring buffer size
        if self.samples.len() > MAX_SAMPLES {
            self.samples.pop_front();
        }
    }

    /// Estimates power consumption for a given refresh type
    ///
    /// Returns the baseline power plus the estimated power draw for the refresh type:
    /// - Full refresh: +200mW
    /// - Partial refresh: +50mW
    /// - Fast refresh: +50mW
    /// - No refresh: baseline only
    ///
    /// # Arguments
    ///
    /// * `refresh_type` - The type of refresh operation
    ///
    /// # Returns
    ///
    /// Estimated power consumption in milliwatts
    pub fn estimate_power(&self, refresh_type: Option<RefreshType>) -> f32 {
        match refresh_type {
            Some(RefreshType::Full) => self.baseline_power + 200.0,   // +200mW spike
            Some(RefreshType::Partial) => self.baseline_power + 50.0, // +50mW spike
            Some(RefreshType::Fast) => self.baseline_power + 50.0,    // +50mW spike
            None => self.baseline_power,
        }
    }

    /// Returns the most recent power consumption value
    ///
    /// If no samples exist, returns the baseline power.
    ///
    /// # Returns
    ///
    /// Current power consumption in milliwatts
    pub fn current_power(&self) -> f32 {
        self.samples
            .back()
            .map(|s| s.power_mw)
            .unwrap_or(self.baseline_power)
    }

    /// Calculates the average power consumption across all samples
    ///
    /// If no samples exist, returns the baseline power.
    ///
    /// # Returns
    ///
    /// Average power consumption in milliwatts
    pub fn average_power(&self) -> f32 {
        if self.samples.is_empty() {
            return self.baseline_power;
        }

        let sum: f32 = self.samples.iter().map(|s| s.power_mw).sum();
        sum / self.samples.len() as f32
    }

    /// Renders the power graph to a pixel buffer
    ///
    /// Draws a green line graph scaled to fit the available samples.
    /// The Y-axis is auto-scaled based on min/max power in the sample set.
    ///
    /// # Arguments
    ///
    /// * `buffer` - ARGB pixel buffer to draw into
    /// * `screen_width` - Width of the screen in pixels (for indexing)
    /// * `x_offset` - X coordinate to start rendering
    /// * `y_offset` - Y coordinate to start rendering
    ///
    /// # Safety
    ///
    /// Performs bounds checking to prevent buffer overruns.
    pub fn render(&self, buffer: &mut [u32], screen_width: u32, x_offset: u32, y_offset: u32) {
        if self.samples.is_empty() {
            return;
        }

        // Find min/max for Y-axis scaling
        let min_power = self
            .samples
            .iter()
            .map(|s| s.power_mw)
            .fold(f32::INFINITY, f32::min);
        let max_power = self
            .samples
            .iter()
            .map(|s| s.power_mw)
            .fold(f32::NEG_INFINITY, f32::max);
        let range = (max_power - min_power).max(1.0); // Avoid division by zero

        // Draw graph line
        for (i, sample) in self.samples.iter().enumerate() {
            let x = (i as f32 / MAX_SAMPLES as f32 * GRAPH_WIDTH as f32) as u32;
            let y_norm = (sample.power_mw - min_power) / range;
            let y = GRAPH_HEIGHT - (y_norm * GRAPH_HEIGHT as f32) as u32;

            let px = x_offset + x;
            let py = y_offset + y;

            // Bounds check to prevent buffer overrun
            let idx = (py * screen_width + px) as usize;
            if idx < buffer.len() {
                buffer[idx] = 0xFF00FF00; // Green line (ARGB)
            }
        }
    }
}

impl Default for PowerGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_graph_creation() {
        let graph = PowerGraph::new();
        assert_eq!(graph.current_power(), 10.0);
    }

    #[test]
    fn test_add_sample() {
        let mut graph = PowerGraph::new();
        graph.add_sample(50.0, Some(RefreshType::Partial));
        assert_eq!(graph.current_power(), 50.0);
    }

    #[test]
    fn test_average_power() {
        let mut graph = PowerGraph::new();
        graph.add_sample(10.0, None);
        graph.add_sample(20.0, None);
        graph.add_sample(30.0, None);
        assert_eq!(graph.average_power(), 20.0);
    }

    #[test]
    fn test_ring_buffer() {
        let mut graph = PowerGraph::new();

        // Add more than MAX_SAMPLES
        for i in 0..350 {
            graph.add_sample(i as f32, None);
        }

        // Should only keep last 300
        assert_eq!(graph.samples.len(), 300);
        assert_eq!(graph.current_power(), 349.0);
    }

    #[test]
    fn test_estimate_power() {
        let graph = PowerGraph::new();

        assert_eq!(graph.estimate_power(None), 10.0);
        assert_eq!(graph.estimate_power(Some(RefreshType::Full)), 210.0);
        assert_eq!(graph.estimate_power(Some(RefreshType::Partial)), 60.0);
        assert_eq!(graph.estimate_power(Some(RefreshType::Fast)), 60.0);
    }

    #[test]
    fn test_average_power_empty() {
        let graph = PowerGraph::new();
        assert_eq!(graph.average_power(), 10.0); // Should return baseline
    }

    #[test]
    fn test_render_empty_samples() {
        let graph = PowerGraph::new();
        let mut buffer = vec![0u32; 1000];

        // Should not panic with empty samples
        graph.render(&mut buffer, 100, 0, 0);

        // Buffer should remain unchanged
        assert!(buffer.iter().all(|&px| px == 0));
    }

    #[test]
    fn test_render_with_samples() {
        let mut graph = PowerGraph::new();
        graph.add_sample(10.0, None);
        graph.add_sample(50.0, Some(RefreshType::Partial));
        graph.add_sample(210.0, Some(RefreshType::Full));

        let mut buffer = vec![0u32; 50000]; // Large enough buffer

        graph.render(&mut buffer, 200, 0, 0);

        // Should have drawn some green pixels
        let green_pixels = buffer.iter().filter(|&&px| px == 0xFF00FF00).count();
        assert!(green_pixels > 0);
    }

    #[test]
    fn test_render_bounds_checking() {
        let mut graph = PowerGraph::new();
        for i in 0..100 {
            graph.add_sample(i as f32, None);
        }

        // Small buffer - should not panic due to bounds checking
        let mut buffer = vec![0u32; 100];
        graph.render(&mut buffer, 10, 0, 0);
    }

    #[test]
    fn test_default_trait() {
        let graph = PowerGraph::default();
        assert_eq!(graph.current_power(), 10.0);
    }
}
