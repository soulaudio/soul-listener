//! Ports: the capabilities a board (`board-h7a3`) or a host simulator
//! (`dap-sim`) must supply for [`crate::audio::AudioPlayer`] to play audio.
//!
//! [`Sink`], [`Source`], [`BufSource`], and [`SeekSource`] are re-exported
//! from `soul-audio-core` as-is — they're already board-agnostic (a cpal
//! `Sink` and a SAI/I²S DMA `Sink` both satisfy the same trait; a slice, a
//! desktop `File`, and an SD-card file all satisfy the same source ladder).
//! [`Storage`] is this crate's one original port.

pub use soul_audio_core::{BufSource, SeekSource, Sink, Source};

/// The minimal capability [`AudioPlayer`](crate::audio::AudioPlayer) needs to
/// turn a queue position into bytes ready for decoding: given a track index,
/// open its encoded bytes as a seekable, bufferable source.
///
/// This mirrors (and generalizes) the two-step shape of
/// `soul-audio-firmware`'s SD reference: `playback::locate(id)` resolves a
/// track's on-card folder/file coordinates from the baked `.soulidx`, and
/// `sd_source::SdSource` wraps the opened `embedded_sdmmc::File` as a
/// `Source`/`SeekSource`. A `Storage` implementation owns both steps —
/// resolving `track` to its physical location *and* opening it — behind one
/// call, so this crate never needs to know what a "location" looks like on
/// disk, SD, or in an in-memory test fixture.
///
/// `Self::Source` is bound on [`BufSource`] + [`SeekSource`] (not just
/// [`Source`]) because [`AudioPlayer`](crate::audio::AudioPlayer) hands it
/// straight to `soul_audio_core::open_seekable`, which probes the container
/// via `BufSource::fill_buf` and needs `SeekSource` both for the resulting
/// decoder to seek and (for MP4) to find `moov` wherever it sits.
///
/// Deliberately `dyn`-free (an associated type, not a trait object) and
/// alloc-free: the concrete source — an SD file, a desktop `File`, a
/// `SliceSource` over an in-memory fixture — lives entirely in the
/// implementor. `open` is called off the real-time path: `AudioPlayer::pump`
/// only ever writes through an *already-opened* decoder (see
/// `soul_audio_engine::Engine`'s `poll_open`/`provide` split, which this
/// trait's `open` fills the "open" half of).
pub trait Storage {
    /// The seekable, bufferable byte source this storage hands back for a
    /// track — e.g. an SD file wrapper, a desktop `File` adapter, or an
    /// in-memory `SliceSource` fixture.
    type Source: BufSource + SeekSource;
    /// Storage-level error: I/O failure, track index out of range, a
    /// corrupt/truncated index, etc.
    type Error;

    /// Open the bytes for the track at queue position `track`, ready for
    /// `soul_audio_core::open_seekable` to probe and decode.
    ///
    /// # Errors
    /// Implementation-defined: typically an I/O failure or an out-of-range
    /// `track`.
    fn open(&mut self, track: usize) -> Result<Self::Source, Self::Error>;
}
