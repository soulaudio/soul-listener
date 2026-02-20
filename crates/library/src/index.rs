//! TrackIndex — fixed-capacity, in-memory catalogue of scanned tracks.
//!
//! On hardware the `FullIndex` (capacity 8 192) must live in external SDRAM
//! (0xC000_0000) because each `Track` is ~600 bytes, totalling ~4.8 MB.
//! Tests use `SmallIndex` (capacity 64) which fits on the host stack.

use crate::track::Track;
use heapless::Vec;

/// Maximum number of tracks the hardware index holds.
///
/// At ~600 bytes per `Track`, `FullIndex` occupies ≈ 4.8 MB and must be
/// placed in external SDRAM — never allocated on the stack.
pub const MAX_TRACKS: usize = 8192;

/// Error type for index operations.
#[derive(Debug, PartialEq, Eq)]
pub enum IndexError {
    /// The index has reached its compile-time capacity.
    Full,
    /// The requested position does not exist.
    OutOfBounds,
}

/// A fixed-capacity, ordered catalogue of [`Track`] entries.
///
/// `N` is the maximum number of tracks; use [`SmallIndex`] in tests and
/// [`FullIndex`] only when placing the value in SDRAM.
pub struct TrackIndex<const N: usize> {
    tracks: Vec<Track, N>,
}

/// Alias for hardware full catalogue — **must live in SDRAM, never on stack**.
pub type FullIndex = TrackIndex<MAX_TRACKS>;

/// Alias used in tests (stack-safe, capacity 64).
pub type SmallIndex = TrackIndex<64>;

impl<const N: usize> TrackIndex<N> {
    /// Create an empty index.
    pub const fn new() -> Self {
        TrackIndex { tracks: Vec::new() }
    }

    /// Append `track` to the index.
    ///
    /// Returns `Err(IndexError::Full)` when capacity `N` is exhausted.
    pub fn insert(&mut self, track: Track) -> Result<(), IndexError> {
        self.tracks.push(track).map_err(|_| IndexError::Full)
    }

    /// Return a reference to the track at zero-based `pos`, or `None`.
    pub fn get(&self, pos: usize) -> Option<&Track> {
        self.tracks.get(pos)
    }

    /// Number of tracks currently stored.
    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    /// Returns `true` when no tracks have been inserted.
    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    /// Remove all tracks, resetting length to zero.
    pub fn clear(&mut self) {
        self.tracks.clear();
    }
}

impl<const N: usize> Default for TrackIndex<N> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::{AudioFormat, Track};

    fn make_track(path: &str) -> Track {
        Track::new(path, AudioFormat::Flac)
    }

    #[test]
    fn test_index_starts_empty() {
        let idx = SmallIndex::new();
        assert_eq!(idx.len(), 0);
    }

    #[test]
    fn test_index_add_track() {
        let mut idx = SmallIndex::new();
        idx.insert(make_track("/a.flac")).expect("insert failed");
        assert_eq!(idx.len(), 1);
    }

    #[test]
    fn test_index_capacity() {
        // MAX_TRACKS must be 8192 (the hardware constant).
        assert_eq!(MAX_TRACKS, 8192);
    }

    #[test]
    fn test_index_full_returns_err() {
        let mut idx = TrackIndex::<4>::new();
        for i in 0..4usize {
            // Build a short path using only core — no std format! macro.
            let mut path_buf = heapless::String::<256>::new();
            path_buf.push_str("/t").expect("push /t");
            // Append decimal digits of `i` without alloc.
            push_usize(&mut path_buf, i);
            path_buf.push_str(".flac").expect("push .flac");
            idx.insert(make_track(path_buf.as_str()))
                .expect("should not be full yet");
        }
        let err = idx.insert(make_track("/overflow.flac")).unwrap_err();
        assert!(matches!(err, IndexError::Full));
    }

    #[test]
    fn test_index_get_by_position() {
        let mut idx = SmallIndex::new();
        idx.insert(make_track("/first.flac")).expect("insert");
        let t = idx.get(0).expect("should have entry at 0");
        assert_eq!(t.file_path.as_str(), "/first.flac");
    }

    #[test]
    fn test_index_get_out_of_bounds() {
        let idx = SmallIndex::new();
        assert!(idx.get(1000).is_none());
    }

    #[test]
    fn test_index_clear() {
        let mut idx = SmallIndex::new();
        idx.insert(make_track("/a.flac")).expect("insert");
        idx.clear();
        assert_eq!(idx.len(), 0);
    }

    /// Append the decimal representation of `n` to `s`.
    fn push_usize(s: &mut heapless::String<256>, mut n: usize) {
        if n == 0 {
            s.push('0').expect("push digit");
            return;
        }
        // Collect digits in reverse.
        let mut digits = [0u8; 20];
        let mut count = 0;
        while n > 0 {
            digits[count] = (n % 10) as u8;
            n /= 10;
            count += 1;
        }
        for i in (0..count).rev() {
            s.push((b'0' + digits[i]) as char).expect("push digit");
        }
    }
}
