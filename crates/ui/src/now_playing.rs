//! Now-playing screen state — track metadata, playback status, volume, progress.

#[cfg(test)]
mod tests {
    use super::NowPlayingState;

    #[test]
    fn test_now_playing_state_default() {
        let state = NowPlayingState::default();
        assert!(!state.playing);
        assert_eq!(state.volume, 50);
        assert_eq!(state.position_ms, 0);
    }

    #[test]
    fn test_now_playing_set_playing() {
        let mut state = NowPlayingState::default();
        state.set_playing(true);
        assert!(state.playing);
    }

    #[test]
    fn test_now_playing_volume_clamped() {
        let mut state = NowPlayingState::default();
        state.set_volume(200);
        assert_eq!(state.volume, 100);
    }

    #[test]
    fn test_now_playing_volume_zero_valid() {
        let mut state = NowPlayingState::default();
        state.set_volume(0);
        assert_eq!(state.volume, 0);
    }

    #[test]
    fn test_now_playing_progress_ratio() {
        let mut state = NowPlayingState::default();
        state.set_duration_ms(10_000);
        state.set_position_ms(5_000);
        let ratio = state.progress();
        // Allow small floating-point tolerance.
        assert!((ratio - 0.5_f32).abs() < 1e-6, "expected ~0.5, got {ratio}");
    }

    #[test]
    fn test_now_playing_progress_zero_duration() {
        let state = NowPlayingState::default();
        assert_eq!(state.progress(), 0.0_f32);
    }
}

/// State for the now-playing screen.
pub struct NowPlayingState {
    /// Whether audio is currently playing (vs. paused).
    pub playing: bool,
    /// Volume level in the range `0..=100`.
    pub volume: u8,
    /// Current playback position in milliseconds.
    pub position_ms: u64,
    /// Total track duration in milliseconds (0 when unknown).
    pub duration_ms: u64,
    /// Track title (up to 128 UTF-8 bytes).
    pub title: heapless::String<128>,
    /// Artist name (up to 64 UTF-8 bytes).
    pub artist: heapless::String<64>,
}

impl NowPlayingState {
    /// Set whether audio is playing.
    pub fn set_playing(&mut self, playing: bool) {
        self.playing = playing;
    }

    /// Set the volume, clamping to `0..=100`.
    pub fn set_volume(&mut self, v: u8) {
        self.volume = v.min(100);
    }

    /// Set the current playback position.
    pub fn set_position_ms(&mut self, ms: u64) {
        self.position_ms = ms;
    }

    /// Set the total track duration.
    pub fn set_duration_ms(&mut self, ms: u64) {
        self.duration_ms = ms;
    }

    /// Return a `0.0..=1.0` progress ratio.
    ///
    /// Returns `0.0` when `duration_ms` is zero.
    #[allow(clippy::cast_precision_loss)]
    #[must_use]
    pub fn progress(&self) -> f32 {
        if self.duration_ms == 0 {
            return 0.0;
        }
        self.position_ms as f32 / self.duration_ms as f32
    }
}

impl Default for NowPlayingState {
    fn default() -> Self {
        NowPlayingState {
            playing: false,
            volume: 50,
            position_ms: 0,
            duration_ms: 0,
            title: heapless::String::new(),
            artist: heapless::String::new(),
        }
    }
}
