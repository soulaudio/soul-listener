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
/// to, over the byte source `St::Source` the storage port hands back, carving
/// its block-staging working buffer from the caller-owned `Pcm` scratch (see
/// [`AudioPlayer`]'s doc comment).
type TrackDecoder<St, Pcm> = AnyDecoder<<St as Storage>::Source, Pcm>;

/// A generic, board-agnostic audio player: `soul-audio-engine`'s `Engine`
/// (queue cursor + repeat modes + gapless preload policy) driving a
/// `soul-audio-core` `Deck` (decode → resample → normalize → volume →
/// channel-map → write) into a caller-supplied [`Sink`], opening tracks
/// through a caller-supplied [`Storage`].
///
/// `Pcm` is the decoder's externalized scratch storage type — the big
/// block-staging buffer `soul_audio_core::open_seekable` used to embed inline
/// (the board-blocking ~1.44 MB open-path transient); it now lives in a
/// caller-owned `Pcm` (`AsRef<[i32]> + AsMut<[i32]>`) that the decoder owns
/// outright and drops with it. `AudioPlayer` never allocates one itself (this
/// crate is no-alloc) — it only calls the caller-supplied `scratch_fn`
/// factory each time a track is opened. On the host, `scratch_fn` typically
/// allocates a fresh `Box<[i32]>` per open (see `dap-sim`'s callers); the
/// board's (future, PCB-blocked) factory will hand out a `&'static mut [i32]`
/// slice from a static pool instead. A plain `fn() -> Pcm` factory (not a
/// closure/trait object) keeps this crate free of extra generics and
/// allocator assumptions.
///
/// `PRELOAD` is the deck's gapless preload depth (tracks kept pre-opened and
/// pre-built for a zero-gap swap at a track boundary) — see
/// `soul_audio_core::Deck`.
pub struct AudioPlayer<S: Sink, St: Storage, Pcm: AsRef<[i32]> + AsMut<[i32]>, const PRELOAD: usize>
{
    engine: Engine<S, TrackDecoder<St, Pcm>, LinearSequencer, PRELOAD>,
    storage: St,
    /// Factory for a fresh decoder scratch buffer, called once per track
    /// open (see the struct's doc comment).
    scratch_fn: fn() -> Pcm,
}

impl<S: Sink, St: Storage, Pcm: AsRef<[i32]> + AsMut<[i32]>, const PRELOAD: usize>
    AudioPlayer<S, St, Pcm, PRELOAD>
{
    /// Build a player over `sink`, opening tracks through `storage`.
    /// `quality` selects the deck's resampler tier. `scratch_fn` is called
    /// once per track open to produce that decoder's externalized scratch
    /// storage (see the struct's doc comment) — e.g.
    /// `|| vec![0i32; soul_audio_core::decoder_scratch_len()].into_boxed_slice()`
    /// on the host. The queue starts empty — call
    /// [`set_queue`](Self::set_queue) before [`play`](Self::play).
    pub fn new(sink: S, storage: St, quality: Quality, scratch_fn: fn() -> Pcm) -> Self {
        Self {
            engine: Engine::new(Deck::new(sink, quality), LinearSequencer::new()),
            storage,
            scratch_fn,
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
    /// # Stack cost: ~119 KiB, and it is NOT reducible from this crate
    /// Measured (`tools/mcu-stackprobe/check-tick-qemu.sh` + `-Z
    /// emit-stack-sizes`, 2026-11-21): this function's own frame is
    /// **119,416 B** — almost exactly *two* `size_of::<AnyDecoder>()`
    /// (59,760 B). One slot is `open_seekable`'s by-value return; the other is
    /// the decoder bound here before it moves into the engine (`Engine::provide`
    /// is small enough that the optimizer inlines it, so it has no frame of its
    /// own). Both are live across the whole nested open, which for
    /// Matroska-FLAC adds ~283 KiB of construction frames beneath this one — so
    /// this frame lands directly on the whole-tick peak of ~415 KiB.
    ///
    /// **Three source-level fixes were tried and measured; none moved it:**
    /// dropping the named local and passing the call result straight to
    /// `provide` (119,416 B), marking the `provide` hand-off `#[inline(never)]`
    /// (119,424 B), and outlining the entire loop body (119,416 B — the frame
    /// simply moves to the helper, which is still live during the decode, so the
    /// peak is unchanged). The two slots are inherent to receiving a ~58 KiB
    /// value by value and then moving it onward.
    ///
    /// **The fix is not a different call shape — it is a smaller type.** Rust has
    /// no safe placement-new (`MaybeUninit` needs `unsafe`, forbidden crate-wide;
    /// `Box::new_in` needs an allocator, there is none; `get_or_insert_with`
    /// still takes its closure's return by value), so these copies cannot be
    /// removed. They are, however, all *proportional*: every frame on this path
    /// measures as a small multiple of `size_of::<AnyDecoder>()` — this one at
    /// 2.00x, `MatroskaDecoder::new_in` at 2.08x, `open_seekable` at 1.03x,
    /// ~7.8x in total. Shrinking the decoder structs (moving their remaining
    /// inline arrays into the shared `P` scratch, as WavPack already does at
    /// 96 B inline versus Matroska's 59,760 B) shrinks every copy with them,
    /// including this frame — with no change to this crate.
    ///
    /// That work lives in `soul-audio-core`, behind
    /// `.github/soul-audio-core.pin`. See
    /// `docs/plans/2026-11-21-memory-architecture-and-placement.md` §8.
    ///
    /// Please don't re-attempt the local restructurings above without re-reading
    /// that section; they have been measured.
    pub fn pump(&mut self) -> Result<Progress, AudioPlayerError<St>> {
        while let Some(track) = self.engine.poll_open() {
            let source = self
                .storage
                .open(track)
                .map_err(AudioPlayerError::Storage)?;
            let decoder =
                open_seekable(source, (self.scratch_fn)()).map_err(AudioPlayerError::Open)?;
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
