//! E-Ink Display Initialization Sequence
//!
//! Simulates a realistic multi-step initialization process similar to real e-ink
//! display controllers like SSD1680, UC8151, etc.
//!
//! # Real Hardware Initialization
//!
//! Real e-ink displays require initialization before use:
//! 1. Power settling (wait for voltage stabilization)
//! 2. Panel detection (read display ID and configuration)
//! 3. Temperature sensor calibration
//! 4. Waveform LUT loading (look-up tables for refresh patterns)
//! 5. VCOM calibration (common electrode voltage)
//! 6. Initial pattern display (often checkerboard)
//! 7. Clear to default state (usually white)
//!
//! # This Implementation
//!
//! Provides realistic timing and visual feedback during initialization:
//! - Configurable timeout detection
//! - Visual checkerboard pattern at step 6
//! - Clear to white at step 7
//! - State tracking (Uninitialized → Initializing → Initialized/Failed)

use std::time::Instant;

/// Initialization state of the display
#[derive(Debug, Clone, PartialEq)]
pub enum InitializationState {
    /// Display has not been initialized
    Uninitialized,

    /// Display is currently initializing
    Initializing {
        /// Current step (1-7)
        step: u8,
        /// Total number of steps
        total_steps: u8,
    },

    /// Display successfully initialized and ready to use
    Initialized,

    /// Initialization failed
    Failed {
        /// Error message describing the failure
        error: String,
    },
}

impl Default for InitializationState {
    fn default() -> Self {
        Self::Uninitialized
    }
}

impl InitializationState {
    /// Check if display is ready to use (initialized)
    pub fn is_ready(&self) -> bool {
        matches!(self, Self::Initialized)
    }

    /// Check if display is currently initializing
    pub fn is_initializing(&self) -> bool {
        matches!(self, Self::Initializing { .. })
    }

    /// Check if initialization failed
    pub fn is_failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }
}

/// Initialization sequence manager
///
/// Manages the multi-step initialization process with timeout detection.
pub struct InitSequence {
    /// Current initialization state
    state: InitializationState,

    /// Timeout in milliseconds (default: 5000ms)
    timeout_ms: u32,

    /// Total number of initialization steps
    total_steps: u8,

    /// Time when initialization started
    start_time: Option<Instant>,
}

impl InitSequence {
    /// Create new initialization sequence with default timeout (5000ms)
    pub fn new() -> Self {
        Self {
            state: InitializationState::Uninitialized,
            timeout_ms: 5000,
            total_steps: 7,
            start_time: None,
        }
    }

    /// Set timeout in milliseconds
    pub fn set_timeout(&mut self, timeout_ms: u32) {
        self.timeout_ms = timeout_ms;
    }

    /// Get current initialization state
    pub fn state(&self) -> &InitializationState {
        &self.state
    }

    /// Start initialization sequence
    ///
    /// Transitions from Uninitialized to Initializing at step 1.
    pub fn start(&mut self) -> Result<(), String> {
        match &self.state {
            InitializationState::Uninitialized | InitializationState::Failed { .. } => {
                self.state = InitializationState::Initializing {
                    step: 1,
                    total_steps: self.total_steps,
                };
                self.start_time = Some(Instant::now());
                Ok(())
            }
            InitializationState::Initializing { .. } => Err("Already initializing".to_string()),
            InitializationState::Initialized => Err("Already initialized".to_string()),
        }
    }

    /// Advance to next initialization step
    ///
    /// Returns the current step number, or None if initialization complete.
    pub fn next_step(&mut self) -> Result<Option<u8>, String> {
        // Check timeout
        if let Some(start) = self.start_time {
            let elapsed = start.elapsed().as_millis() as u32;
            if elapsed > self.timeout_ms {
                self.state = InitializationState::Failed {
                    error: format!("Initialization timeout after {}ms", elapsed),
                };
                return Err("Initialization timeout".to_string());
            }
        }

        match &self.state {
            InitializationState::Initializing { step, total_steps } => {
                let current_step = *step;

                if current_step < *total_steps {
                    // Advance to next step
                    self.state = InitializationState::Initializing {
                        step: current_step + 1,
                        total_steps: *total_steps,
                    };
                    Ok(Some(current_step + 1))
                } else {
                    // Complete initialization
                    self.state = InitializationState::Initialized;
                    self.start_time = None;
                    Ok(None)
                }
            }
            _ => Err("Not initializing".to_string()),
        }
    }

    /// Mark initialization as failed
    pub fn fail(&mut self, error: String) {
        self.state = InitializationState::Failed { error };
        self.start_time = None;
    }

    /// Reset to uninitialized state
    pub fn reset(&mut self) {
        self.state = InitializationState::Uninitialized;
        self.start_time = None;
    }

    /// Get elapsed time since initialization started (in milliseconds)
    pub fn elapsed_ms(&self) -> u32 {
        self.start_time
            .map(|start| start.elapsed().as_millis() as u32)
            .unwrap_or(0)
    }
}

impl Default for InitSequence {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialization step information
pub struct InitStep {
    /// Step number (1-7)
    pub number: u8,

    /// Step description
    pub description: &'static str,

    /// Duration in milliseconds
    pub duration_ms: u32,

    /// Whether this step has visual output
    pub has_visual: bool,
}

impl InitStep {
    /// Get all initialization steps
    pub fn all_steps() -> [InitStep; 7] {
        [
            InitStep {
                number: 1,
                description: "Power settling",
                duration_ms: 100,
                has_visual: false,
            },
            InitStep {
                number: 2,
                description: "Panel detection",
                duration_ms: 50,
                has_visual: false,
            },
            InitStep {
                number: 3,
                description: "Temperature sensor",
                duration_ms: 20,
                has_visual: false,
            },
            InitStep {
                number: 4,
                description: "Waveform tables",
                duration_ms: 200,
                has_visual: false,
            },
            InitStep {
                number: 5,
                description: "VCOM calibration",
                duration_ms: 100,
                has_visual: false,
            },
            InitStep {
                number: 6,
                description: "Checkerboard pattern",
                duration_ms: 500,
                has_visual: true,
            },
            InitStep {
                number: 7,
                description: "Clear to white",
                duration_ms: 1000,
                has_visual: true,
            },
        ]
    }

    /// Get specific step by number (1-indexed)
    pub fn get(step: u8) -> Option<InitStep> {
        Self::all_steps().into_iter().find(|s| s.number == step)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_init_state_default() {
        let state = InitializationState::default();
        assert_eq!(state, InitializationState::Uninitialized);
    }

    #[test]
    fn test_init_state_is_ready() {
        assert!(!InitializationState::Uninitialized.is_ready());
        assert!(!InitializationState::Initializing {
            step: 1,
            total_steps: 7
        }
        .is_ready());
        assert!(InitializationState::Initialized.is_ready());
        assert!(!InitializationState::Failed {
            error: "test".to_string()
        }
        .is_ready());
    }

    #[test]
    fn test_init_sequence_creation() {
        let seq = InitSequence::new();
        assert_eq!(seq.state(), &InitializationState::Uninitialized);
        assert_eq!(seq.timeout_ms, 5000);
        assert_eq!(seq.total_steps, 7);
    }

    #[test]
    fn test_init_sequence_start() {
        let mut seq = InitSequence::new();

        seq.start().unwrap();
        assert_eq!(
            seq.state(),
            &InitializationState::Initializing {
                step: 1,
                total_steps: 7
            }
        );

        // Cannot start twice
        assert!(seq.start().is_err());
    }

    #[test]
    fn test_init_sequence_steps() {
        let mut seq = InitSequence::new();

        seq.start().unwrap();

        // Advance through all steps
        for expected_next in 2..=7 {
            let next = seq.next_step().unwrap();
            assert_eq!(next, Some(expected_next));
        }

        // Final step completes initialization
        let final_step = seq.next_step().unwrap();
        assert_eq!(final_step, None);
        assert_eq!(seq.state(), &InitializationState::Initialized);
    }

    #[test]
    fn test_init_sequence_reset() {
        let mut seq = InitSequence::new();

        seq.start().unwrap();
        seq.next_step().unwrap();

        seq.reset();
        assert_eq!(seq.state(), &InitializationState::Uninitialized);
    }

    #[test]
    fn test_init_sequence_fail() {
        let mut seq = InitSequence::new();

        seq.start().unwrap();
        seq.fail("Test error".to_string());

        assert!(seq.state().is_failed());
        if let InitializationState::Failed { error } = seq.state() {
            assert_eq!(error, "Test error");
        }
    }

    #[test]
    fn test_init_step_info() {
        let steps = InitStep::all_steps();
        assert_eq!(steps.len(), 7);

        assert_eq!(steps[0].number, 1);
        assert_eq!(steps[0].description, "Power settling");
        assert_eq!(steps[0].duration_ms, 100);
        assert!(!steps[0].has_visual);

        assert_eq!(steps[5].number, 6);
        assert_eq!(steps[5].description, "Checkerboard pattern");
        assert!(steps[5].has_visual);

        assert_eq!(steps[6].number, 7);
        assert_eq!(steps[6].description, "Clear to white");
        assert!(steps[6].has_visual);
    }

    #[test]
    fn test_init_step_get() {
        let step = InitStep::get(6).unwrap();
        assert_eq!(step.number, 6);
        assert_eq!(step.description, "Checkerboard pattern");

        assert!(InitStep::get(0).is_none());
        assert!(InitStep::get(8).is_none());
    }

    #[test]
    fn test_init_sequence_timeout_setting() {
        let mut seq = InitSequence::new();
        seq.set_timeout(10000);
        assert_eq!(seq.timeout_ms, 10000);
    }
}
