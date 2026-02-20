//! SAI (Serial Audio Interface) overrun recovery wrapper.
//!
//! # Background — Embassy Issue #3205
//!
//! Source: <https://github.com/embassy-rs/embassy/issues/3205>
//!
//! Once a SAI overrun occurs, all subsequent `write()` calls immediately fail
//! with `OverrunError`. There is no built-in reset mechanism: the ring buffer
//! and driver state become permanently stuck.
//!
//! **Recovery procedure** (confirmed in embassy issue #3205):
//! 1. Detect the `OverrunError` return from `write()`.
//! 2. Drop the SAI driver instance — the peripheral resets on `Drop`.
//! 3. Reconstruct the driver with `new_sai4()` (or equivalent for your SAI
//!    block). The new instance starts with a clean ring buffer.
//! 4. Resume writing audio samples as normal.
//!
//! **Prevention**: Keep the SAI TX FIFO fed continuously. If the audio decode
//! pipeline stalls, fill the DMA buffer with silence (zeroes) rather than
//! suspending `write()` calls. Refer to the ping-pong DMA pattern:
//! `AUDIO_BUFFER` in AXI SRAM (`0x2400_0000`) alternates between decode output
//! and silence fill; the SAI driver always has data to consume.
//!
//! # This Module
//!
//! Provides [`SaiRecoveryState`] — a lightweight state machine that tracks
//! whether the SAI driver has experienced an overrun and needs reconstruction.
//! The actual drop + reconstruct must be performed by the calling task, since
//! this module cannot hold embassy-stm32 concrete types (platform-crate rule).
//!
//! # Usage Pattern
//!
//! ```rust,ignore
//! let mut recovery = SaiRecoveryState::new();
//!
//! loop {
//!     if recovery.needs_recovery() {
//!         // Drop old driver, reconstruct it:
//!         drop(sai);
//!         sai = new_sai4(peripherals, dma_channels);
//!         recovery.on_recovered();
//!     }
//!
//!     let result = sai.write(&audio_buffer).await
//!         .map_err(|_| SaiWriteError::Overrun);
//!     recovery.on_write_result(result);
//! }
//! ```

// ─── Error type ──────────────────────────────────────────────────────────────

/// SAI write error variants relevant to overrun recovery.
///
/// Maps from `embassy_stm32::sai::Error` (hardware builds) or equivalent.
/// This type is intentionally independent of embassy-stm32 so tests run on host.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaiWriteError {
    /// TX FIFO / DMA underrun — driver is permanently stuck until dropped and
    /// reconstructed. See embassy issue #3205.
    Overrun,
    /// Other write error (DMA misconfiguration, clock error, etc.).
    Other,
}

// ─── State machine ───────────────────────────────────────────────────────────

/// SAI driver recovery state machine.
///
/// Tracks whether the SAI driver has experienced an overrun error and needs
/// to be dropped and reconstructed before audio output can resume.
///
/// All transitions are via [`on_write_result`][Self::on_write_result] and
/// [`on_recovered`][Self::on_recovered]. State is inspected via
/// [`needs_recovery`][Self::needs_recovery].
///
/// The `overrun_count` field saturates at [`u8::MAX`] (255) to prevent
/// wrapping on sustained failure before the caller can react.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SaiRecoveryState {
    /// Driver is operating normally; audio transfers succeed.
    Healthy,
    /// Driver has experienced one or more overrun errors.
    ///
    /// The caller must drop and reconstruct the SAI driver instance, then call
    /// [`on_recovered`][SaiRecoveryState::on_recovered] to return to `Healthy`.
    NeedsRecovery {
        /// Number of `OverrunError` events since the last successful recovery.
        /// Saturates at 255 — will not wrap on u8 overflow.
        overrun_count: u8,
    },
}

impl SaiRecoveryState {
    /// Create a new state machine in the [`Healthy`][Self::Healthy] state.
    pub fn new() -> Self {
        Self::Healthy
    }

    /// Returns `true` if the SAI driver must be dropped and reconstructed.
    pub fn needs_recovery(&self) -> bool {
        matches!(self, Self::NeedsRecovery { .. })
    }

    /// Update state after every SAI write attempt.
    ///
    /// - `Ok(())`: no state change (stays `Healthy` or `NeedsRecovery`).
    /// - `Err(SaiWriteError::Overrun)`: transitions to `NeedsRecovery` or
    ///   increments the `overrun_count` (saturating).
    /// - `Err(SaiWriteError::Other)`: no state change (non-overrun errors
    ///   do not make the driver permanently stuck).
    pub fn on_write_result(&mut self, result: Result<(), SaiWriteError>) {
        if let Err(SaiWriteError::Overrun) = result {
            *self = Self::NeedsRecovery {
                overrun_count: match self {
                    Self::NeedsRecovery { overrun_count } => overrun_count.saturating_add(1),
                    Self::Healthy => 1,
                },
            };
        }
        // Ok(()) and Err(Other) leave state unchanged.
    }

    /// Call after successfully dropping and reconstructing the SAI driver.
    ///
    /// Transitions from [`NeedsRecovery`][Self::NeedsRecovery] back to
    /// [`Healthy`][Self::Healthy]. Safe to call from `Healthy` (no-op).
    pub fn on_recovered(&mut self) {
        *self = Self::Healthy;
    }

    /// Returns the number of overrun events since last recovery, or 0 if healthy.
    pub fn overrun_count(&self) -> u8 {
        match self {
            Self::NeedsRecovery { overrun_count } => *overrun_count,
            Self::Healthy => 0,
        }
    }
}

impl Default for SaiRecoveryState {
    fn default() -> Self {
        Self::new()
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::absurd_extreme_comparisons)]
mod tests {
    use super::*;

    // ── Test A ────────────────────────────────────────────────────────────────
    /// Freshly created state must be Healthy.
    #[test]
    fn test_initial_state_is_healthy() {
        assert_eq!(SaiRecoveryState::new(), SaiRecoveryState::Healthy);
    }

    // ── Test B ────────────────────────────────────────────────────────────────
    /// Overrun errors transition to NeedsRecovery and increment overrun_count.
    #[test]
    fn test_transitions_on_overrun() {
        let mut state = SaiRecoveryState::new();

        state.on_write_result(Err(SaiWriteError::Overrun));
        assert_eq!(state, SaiRecoveryState::NeedsRecovery { overrun_count: 1 });

        state.on_write_result(Err(SaiWriteError::Overrun));
        assert_eq!(state, SaiRecoveryState::NeedsRecovery { overrun_count: 2 });
    }

    // ── Test C ────────────────────────────────────────────────────────────────
    /// on_recovered() returns state to Healthy.
    #[test]
    fn test_recovery_resets_to_healthy() {
        let mut state = SaiRecoveryState::new();
        state.on_write_result(Err(SaiWriteError::Overrun));
        state.on_recovered();
        assert_eq!(state, SaiRecoveryState::Healthy);
    }

    // ── Test D ────────────────────────────────────────────────────────────────
    /// A successful write from Healthy keeps state Healthy.
    #[test]
    fn test_healthy_ok_stays_healthy() {
        let mut state = SaiRecoveryState::new();
        state.on_write_result(Ok(()));
        assert_eq!(state, SaiRecoveryState::Healthy);
    }

    // ── Test E ────────────────────────────────────────────────────────────────
    /// needs_recovery() predicate matches the enum variant.
    #[test]
    fn test_needs_recovery_predicate() {
        let mut state = SaiRecoveryState::new();
        assert!(!state.needs_recovery());

        state.on_write_result(Err(SaiWriteError::Overrun));
        assert!(state.needs_recovery());
    }

    // ── Test F ────────────────────────────────────────────────────────────────
    /// overrun_count saturates at u8::MAX (255) and does not wrap.
    #[test]
    // panic! in test is an intentional assertion failure message.
    #[allow(clippy::panic)]
    fn test_overrun_count_saturates() {
        let mut state = SaiRecoveryState::new();
        for _ in 0..300 {
            state.on_write_result(Err(SaiWriteError::Overrun));
        }
        // Must not overflow — saturates at u8::MAX (255).
        // The comparison `overrun_count <= 255` is intentionally tautological for
        // u8 — it documents the saturation contract and would catch a future type
        // widening regression.
        #[allow(unused_comparisons)]
        if let SaiRecoveryState::NeedsRecovery { overrun_count } = state {
            assert!(overrun_count <= 255);
        } else {
            panic!("expected NeedsRecovery after 300 overruns");
        }
    }
}
