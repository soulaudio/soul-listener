//! `AudioPlayer` — the board-agnostic playback assembly.
//!
//! Pure wiring: every decode/resample/gapless/queue decision lives in
//! `soul-audio-core`'s `Deck` and `soul-audio-engine`'s `Engine`. This module
//! composes them over a caller-supplied `Sink` and `Storage`, filling the
//! engine's `poll_open()`/`provide()` seam through [`Storage::open`], and
//! exposes one `pump()` plus transport methods as the single surface the
//! rest of the DAP (power management, wheel/button controls — later plans)
//! drives.

use soul_audio_core::{
    open_seekable, AnyDecoder, Deck, DeckError, DeckState, DecodeError, DecodeInfo, Progress,
    Quality, Sink,
};
use soul_audio_engine::{Engine, LinearSequencer, RepeatMode};

use crate::ports::Storage;

/// Everything that can go wrong driving [`AudioPlayer::pump`] or transport:
/// either the [`Storage`] port failed to open a track, the container/codec
/// couldn't be opened, or the deck/engine pipeline (resample, channel
/// mismatch, sink write) failed.
pub enum AudioPlayerError<St: Storage> {
    /// [`Storage::open`] failed for the requested track.
    Storage(St::Error),
    /// `soul_audio_core::open_seekable` could not identify or open the
    /// container/codec.
    Open(DecodeError),
    /// The deck/engine pipeline failed.
    Deck(DeckError),
}

impl<St: Storage> core::fmt::Debug for AudioPlayerError<St>
where
    St::Error: core::fmt::Debug,
{
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Storage(e) => f.debug_tuple("Storage").field(e).finish(),
            Self::Open(e) => f.debug_tuple("Open").field(e).finish(),
            Self::Deck(e) => f.debug_tuple("Deck").field(e).finish(),
        }
    }
}

/// The decoder type the deck plays: whatever format `open_seekable` dispatches
/// to, over the byte source `St::Source` the storage port hands back.
type TrackDecoder<St> = AnyDecoder<<St as Storage>::Source>;

/// A generic, board-agnostic audio player: `soul-audio-engine`'s `Engine`
/// (queue cursor + repeat modes + gapless preload policy) driving a
/// `soul-audio-core` `Deck` (decode → resample → normalize → volume →
/// channel-map → write) into a caller-supplied [`Sink`], opening tracks
/// through a caller-supplied [`Storage`].
///
/// `PRELOAD` is the deck's gapless preload depth (tracks kept pre-opened and
/// pre-built for a zero-gap swap at a track boundary) — see
/// `soul_audio_core::Deck`.
pub struct AudioPlayer<S: Sink, St: Storage, const PRELOAD: usize> {
    engine: Engine<S, TrackDecoder<St>, LinearSequencer, PRELOAD>,
    storage: St,
}

impl<S: Sink, St: Storage, const PRELOAD: usize> AudioPlayer<S, St, PRELOAD> {
    /// Build a player over `sink`, opening tracks through `storage`.
    /// `quality` selects the deck's resampler tier. The queue starts empty —
    /// call [`set_queue`](Self::set_queue) before [`play`](Self::play).
    pub fn new(sink: S, storage: St, quality: Quality) -> Self {
        Self {
            engine: Engine::new(Deck::new(sink, quality), LinearSequencer::new()),
            storage,
        }
    }

    /// (Re)define the queue as `len` tracks (indices `0..len`, each resolved
    /// through [`Storage`]), starting at `start` (clamped to the queue).
    /// Stops and re-arms the engine from scratch.
    pub fn set_queue(&mut self, len: usize, start: usize) {
        self.engine.sequencer_mut().set_queue(len, start);
        self.engine.rearm();
    }

    /// Set the repeat mode (`Normal` / `RepeatOne` / `RepeatAll`).
    pub fn set_repeat_mode(&mut self, mode: RepeatMode) {
        self.engine.sequencer_mut().set_mode(mode);
    }

    /// Resume/begin playback of the current queue position.
    pub fn play(&mut self) {
        self.engine.play();
    }

    /// Pause (retains position).
    pub fn pause(&mut self) {
        self.engine.pause();
    }

    /// Stop and clear the deck (drops the current track and any preloads).
    pub fn stop(&mut self) {
        self.engine.stop();
    }

    /// Skip to the next track. No-op at the end of a `Normal` queue.
    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> Result<(), AudioPlayerError<St>> {
        self.engine.next().map_err(AudioPlayerError::Deck)
    }

    /// Skip to the previous track. No-op at the start of a `Normal` queue.
    pub fn prev(&mut self) -> Result<(), AudioPlayerError<St>> {
        self.engine.prev().map_err(AudioPlayerError::Deck)
    }

    /// Jump to an explicit queue position. No-op if out of range.
    pub fn goto(&mut self, pos: usize) -> Result<(), AudioPlayerError<St>> {
        self.engine.goto(pos).map_err(AudioPlayerError::Deck)
    }

    /// Seek the current track to per-channel `frame`.
    ///
    /// # Errors
    /// [`AudioPlayerError::Deck`] if nothing is loaded or the track can't seek.
    pub fn seek(&mut self, frame: u64) -> Result<u64, AudioPlayerError<St>> {
        self.engine.seek(frame).map_err(AudioPlayerError::Deck)
    }

    /// Set output volume (0.0..=1.0), applied at the sink's output stage.
    pub fn set_volume(&mut self, volume: f32) {
        self.engine.set_volume(volume);
    }

    /// Drive one step of playback: service every pending open the engine
    /// wants (via [`Storage::open`] + `soul_audio_core::open_seekable`,
    /// filling `poll_open()`/`provide()`), then advance the deck one block
    /// (decode → resample → normalize → volume → channel-map → write).
    ///
    /// # Errors
    /// [`AudioPlayerError::Storage`] if opening a track's bytes fails,
    /// [`AudioPlayerError::Open`] if the container/codec can't be identified
    /// or opened, [`AudioPlayerError::Deck`] if the decode/resample/sink
    /// pipeline fails.
    pub fn pump(&mut self) -> Result<Progress, AudioPlayerError<St>> {
        while let Some(track) = self.engine.poll_open() {
            let source = self
                .storage
                .open(track)
                .map_err(AudioPlayerError::Storage)?;
            let decoder = open_seekable(source).map_err(AudioPlayerError::Open)?;
            self.engine
                .provide(track, decoder)
                .map_err(AudioPlayerError::Deck)?;
        }
        self.engine.pump().map_err(AudioPlayerError::Deck)
    }

    /// Transport state of the deck.
    #[must_use]
    pub fn state(&self) -> DeckState {
        self.engine.state()
    }

    /// The queue position currently loaded (or being loaded), if any.
    #[must_use]
    pub fn current(&self) -> Option<usize> {
        self.engine.current()
    }

    /// Per-channel source-frame position of the current track.
    #[must_use]
    pub fn position_frames(&self) -> u64 {
        self.engine.deck().position_frames()
    }

    /// Total per-channel frames of the current track, if known.
    #[must_use]
    pub fn duration_frames(&self) -> Option<u64> {
        self.engine.deck().duration_frames()
    }

    /// The current track's decode→output chain, for a signal-path display.
    #[must_use]
    pub fn decode_info(&self) -> Option<DecodeInfo> {
        self.engine.deck().decode_info()
    }

    /// Borrow the output sink (e.g. to query its stream info).
    #[must_use]
    pub fn sink(&self) -> &S {
        self.engine.deck().sink()
    }

    /// Mutably borrow the output sink — the driver's hook for sink-side
    /// controls such as pausing/resuming the concrete output device.
    pub fn sink_mut(&mut self) -> &mut S {
        self.engine.deck_mut().sink_mut()
    }
}
