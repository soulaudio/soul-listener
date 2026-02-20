//! E-Ink Refresh Modes
//!
//! Defines the different refresh strategies for e-ink displays.

/// Refresh mode for e-ink displays
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RefreshMode {
    /// Full refresh with flashing (~2000ms)
    /// - 16 grayscale levels
    /// - 3-4 flashes (black → white → final)
    /// - Clears all ghosting
    /// - Use for: Page transitions, periodic cleanup
    #[default]
    Full,

    /// Partial refresh (~300ms)
    /// - 4 grayscale levels
    /// - No flashing
    /// - Accumulates ghosting (~15% per refresh)
    /// - Use for: Progress bars, live updates, animations
    Partial,

    /// Fast refresh (~260ms)
    /// - 2 levels (black/white only)
    /// - Single flash
    /// - High ghosting accumulation (~25% per refresh)
    /// - Use for: Page turns, scrolling
    Fast,
}

impl RefreshMode {
    /// Get human-readable name
    pub fn name(&self) -> &'static str {
        match self {
            RefreshMode::Full => "Full",
            RefreshMode::Partial => "Partial",
            RefreshMode::Fast => "Fast",
        }
    }

    /// Check if this mode accumulates ghosting
    pub fn accumulates_ghosting(&self) -> bool {
        matches!(self, RefreshMode::Partial | RefreshMode::Fast)
    }

    /// Check if this mode clears ghosting
    pub fn clears_ghosting(&self) -> bool {
        *self == RefreshMode::Full
    }
}

/// Refresh strategy for managing ghosting accumulation
///
/// Automatically triggers full refresh after N partial refreshes
/// to prevent ghosting buildup.
#[derive(Debug, Clone)]
pub struct RefreshStrategy {
    /// Number of partial/fast refreshes since last full refresh
    partial_count: usize,

    /// Threshold for automatic full refresh (typically 5-10)
    full_refresh_threshold: usize,

    /// Current accumulated ghosting level (0.0 - 1.0)
    ghosting_level: f32,
}

impl RefreshStrategy {
    /// Create new refresh strategy with default threshold of 5
    pub fn new() -> Self {
        Self::with_threshold(5)
    }

    /// Create refresh strategy with custom threshold
    pub fn with_threshold(threshold: usize) -> Self {
        Self {
            partial_count: 0,
            full_refresh_threshold: threshold,
            ghosting_level: 0.0,
        }
    }

    /// Get current ghosting level (0.0 - 1.0)
    pub fn ghosting_level(&self) -> f32 {
        self.ghosting_level
    }

    /// Get number of partial refreshes since last full refresh
    pub fn partial_count(&self) -> usize {
        self.partial_count
    }

    /// Check if full refresh is recommended
    pub fn needs_full_refresh(&self) -> bool {
        self.partial_count >= self.full_refresh_threshold
    }

    /// Record a refresh and update ghosting level
    ///
    /// Returns true if a full refresh should be performed to clear ghosting
    // SAFETY: partial_count is a usize counter bounded by realistic refresh counts;
    // ghosting_level is clamped to [0.0, 1.0] so the f32 addition cannot overflow.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn record_refresh(&mut self, mode: RefreshMode, ghosting_rate: f32) -> bool {
        match mode {
            RefreshMode::Full => {
                self.partial_count = 0;
                self.ghosting_level = 0.0;
                false
            }
            RefreshMode::Partial | RefreshMode::Fast => {
                self.partial_count += 1;
                self.ghosting_level = (self.ghosting_level + ghosting_rate).min(1.0);
                self.needs_full_refresh()
            }
        }
    }

    /// Reset the strategy (call after full refresh)
    pub fn reset(&mut self) {
        self.partial_count = 0;
        self.ghosting_level = 0.0;
    }
}

impl Default for RefreshStrategy {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_refresh_mode_properties() {
        assert!(RefreshMode::Partial.accumulates_ghosting());
        assert!(RefreshMode::Fast.accumulates_ghosting());
        assert!(!RefreshMode::Full.accumulates_ghosting());

        assert!(RefreshMode::Full.clears_ghosting());
        assert!(!RefreshMode::Partial.clears_ghosting());
    }

    #[test]
    fn test_refresh_strategy() {
        let mut strategy = RefreshStrategy::with_threshold(3);

        // Record 3 partial refreshes
        assert!(!strategy.record_refresh(RefreshMode::Partial, 0.15));
        assert!(!strategy.record_refresh(RefreshMode::Partial, 0.15));
        assert!(strategy.record_refresh(RefreshMode::Partial, 0.15)); // Should trigger

        assert!(strategy.needs_full_refresh());
        assert_eq!(strategy.partial_count(), 3);
        assert!((strategy.ghosting_level() - 0.45).abs() < 0.01);

        // Full refresh resets
        strategy.record_refresh(RefreshMode::Full, 0.0);
        assert_eq!(strategy.partial_count(), 0);
        assert_eq!(strategy.ghosting_level(), 0.0);
    }

    #[test]
    fn test_ghosting_accumulation() {
        let mut strategy = RefreshStrategy::new();

        // Fast refresh has higher ghosting rate
        strategy.record_refresh(RefreshMode::Fast, 0.25);
        strategy.record_refresh(RefreshMode::Fast, 0.25);

        assert!((strategy.ghosting_level() - 0.50).abs() < 0.01);
    }

    #[test]
    fn test_ghosting_clamping() {
        let mut strategy = RefreshStrategy::new();

        // Ghosting should not exceed 1.0
        for _ in 0..10 {
            strategy.record_refresh(RefreshMode::Fast, 0.25);
        }

        assert_eq!(strategy.ghosting_level(), 1.0);
    }
}
