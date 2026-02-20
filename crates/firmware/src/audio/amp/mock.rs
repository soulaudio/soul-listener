//! Mock headphone amplifier for host-side testing
//!
//! Implements [`AmpDriver`] without any hardware dependency. Records all calls
//! for assertion in tests.

use super::AmpDriver;

/// Mock amplifier — records all calls for test assertions.
pub struct MockAmp {
    /// Whether the amplifier is currently enabled.
    pub enabled: bool,
    /// Total number of times [`AmpDriver::enable`] has been called.
    pub enable_count: usize,
    /// Total number of times [`AmpDriver::disable`] has been called.
    pub disable_count: usize,
}

impl MockAmp {
    /// Create a new mock amplifier. Starts in the disabled state.
    pub fn new() -> Self {
        Self {
            enabled: false,
            enable_count: 0,
            disable_count: 0,
        }
    }
}

impl Default for MockAmp {
    fn default() -> Self {
        Self::new()
    }
}

impl AmpDriver for MockAmp {
    type Error = core::convert::Infallible;

    #[allow(clippy::arithmetic_side_effects)] // Mock counter; overflow not a concern in tests
    async fn enable(&mut self) -> Result<(), Self::Error> {
        self.enabled = true;
        self.enable_count += 1;
        Ok(())
    }

    #[allow(clippy::arithmetic_side_effects)] // Mock counter; overflow not a concern in tests
    async fn disable(&mut self) -> Result<(), Self::Error> {
        self.enabled = false;
        self.disable_count += 1;
        Ok(())
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_initial_state() {
        let amp = MockAmp::new();
        assert!(!amp.is_enabled(), "amp should start disabled");
        assert_eq!(amp.enable_count, 0);
        assert_eq!(amp.disable_count, 0);
    }

    #[tokio::test]
    async fn test_enable() {
        let mut amp = MockAmp::new();
        amp.enable().await.unwrap();
        assert!(amp.is_enabled());
        assert_eq!(amp.enable_count, 1);
        assert_eq!(amp.disable_count, 0);
    }

    #[tokio::test]
    async fn test_disable() {
        let mut amp = MockAmp::new();
        amp.enable().await.unwrap();
        amp.disable().await.unwrap();
        assert!(!amp.is_enabled());
        assert_eq!(amp.enable_count, 1);
        assert_eq!(amp.disable_count, 1);
    }

    #[tokio::test]
    async fn test_toggle() {
        let mut amp = MockAmp::new();
        amp.enable().await.unwrap();
        assert!(amp.is_enabled());
        amp.disable().await.unwrap();
        assert!(!amp.is_enabled());
        amp.enable().await.unwrap();
        assert!(amp.is_enabled());
    }

    #[tokio::test]
    async fn test_enable_count() {
        let mut amp = MockAmp::new();
        for _ in 0..5 {
            amp.enable().await.unwrap();
            amp.disable().await.unwrap();
        }
        assert_eq!(amp.enable_count, 5);
        assert_eq!(amp.disable_count, 5);
    }

    #[tokio::test]
    async fn test_default_matches_new() {
        let a = MockAmp::new();
        let b = MockAmp::default();
        assert_eq!(a.enabled, b.enabled);
        assert_eq!(a.enable_count, b.enable_count);
        assert_eq!(a.disable_count, b.disable_count);
    }
}
