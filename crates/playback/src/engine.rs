//! Playback state machine.
//!
//! `PlaybackEngine` is a pure, `no_std`, allocation-free state machine that
//! tracks whether the player is stopped, playing, or paused, as well as the
//! current seek position within the active track.
//!
//! It deliberately has **no** I/O — it does not drive hardware, read files, or
//! call decoders.  Those concerns are handled by higher-level tasks that read
//! `engine.state()` and issue commands via Embassy channels.  This separation
//! makes the state machine trivially testable on the host.

/// Current playback state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState {
    /// No track is loaded or playback has been explicitly stopped.
    Stopped,
    /// A track is actively decoding and streaming to the DAC.
    Playing,
    /// Playback is temporarily suspended; position is preserved.
    Paused,
}

/// Errors returned by `PlaybackEngine` operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackError {
    /// The requested operation requires the player to be in a playing or
    /// paused state, but it is currently stopped.
    NotPlaying,
    /// The player is already playing; calling `play()` again is a no-op error.
    AlreadyPlaying,
    /// A seek target exceeded the track duration (only returned by callers that
    /// want strict range checking; the default `seek_ms` clamps silently).
    SeekOutOfRange,
}

/// Pure state machine for audio playback control.
///
/// All fields are private; state is mutated only through the method API.
/// No allocations, no I/O, no hardware dependencies.
pub struct PlaybackEngine {
    state: PlaybackState,
    position_ms: u64,
    duration_ms: u64,
}

impl PlaybackEngine {
    /// Create a new engine in the `Stopped` state with unknown duration.
    ///
    /// Use [`with_duration`] when the track length is known at construction
    /// time so that `seek_ms` can clamp correctly.
    ///
    /// [`with_duration`]: PlaybackEngine::with_duration
    pub fn new() -> Self {
        Self {
            state: PlaybackState::Stopped,
            position_ms: 0,
            duration_ms: u64::MAX,
        }
    }

    /// Create a new engine with a known track duration in milliseconds.
    ///
    /// Seek operations will be clamped to `[0, duration_ms]`.
    pub fn with_duration(duration_ms: u64) -> Self {
        Self {
            state: PlaybackState::Stopped,
            position_ms: 0,
            duration_ms,
        }
    }

    /// Start or resume playback.
    ///
    /// Transitions:
    /// - `Stopped  → Playing`  ✓
    /// - `Paused   → Playing`  ✓
    /// - `Playing  → Playing`  returns `Err(AlreadyPlaying)`
    ///
    /// # Errors
    ///
    /// Returns `Err(PlaybackError::AlreadyPlaying)` when already playing.
    pub fn play(&mut self) -> Result<(), PlaybackError> {
        match self.state {
            PlaybackState::Playing => Err(PlaybackError::AlreadyPlaying),
            PlaybackState::Stopped | PlaybackState::Paused => {
                self.state = PlaybackState::Playing;
                Ok(())
            }
        }
    }

    /// Pause playback, preserving the current position.
    ///
    /// Transitions:
    /// - `Playing → Paused`  ✓
    /// - `Stopped → —`       returns `Err(NotPlaying)`
    /// - `Paused  → Paused`  idempotent, returns `Ok(())`
    ///
    /// # Errors
    ///
    /// Returns `Err(PlaybackError::NotPlaying)` when in the `Stopped` state.
    pub fn pause(&mut self) -> Result<(), PlaybackError> {
        match self.state {
            PlaybackState::Stopped => Err(PlaybackError::NotPlaying),
            PlaybackState::Playing | PlaybackState::Paused => {
                self.state = PlaybackState::Paused;
                Ok(())
            }
        }
    }

    /// Stop playback and reset position to zero.
    ///
    /// This always succeeds: stopping an already-stopped engine is a no-op.
    ///
    /// # Errors
    ///
    /// Always returns `Ok(())`.
    pub fn stop(&mut self) -> Result<(), PlaybackError> {
        self.state = PlaybackState::Stopped;
        self.position_ms = 0;
        Ok(())
    }

    /// Seek to the given position (milliseconds from the start of the track).
    ///
    /// The position is clamped to `[0, duration_ms]`.  If the duration is not
    /// known (constructed with [`new`]), clamping is effectively disabled
    /// because `duration_ms` is initialised to `u64::MAX`.
    ///
    /// [`new`]: PlaybackEngine::new
    pub fn seek_ms(&mut self, ms: u64) {
        self.position_ms = ms.min(self.duration_ms);
    }

    /// Return the current playback position in milliseconds.
    pub fn position_ms(&self) -> u64 {
        self.position_ms
    }

    /// Return the current [`PlaybackState`].
    pub fn state(&self) -> PlaybackState {
        self.state
    }

    /// Return the track duration in milliseconds.
    ///
    /// Returns `u64::MAX` when no duration has been set.
    pub fn duration_ms(&self) -> u64 {
        self.duration_ms
    }
}

impl Default for PlaybackEngine {
    fn default() -> Self {
        Self::new()
    }
}
