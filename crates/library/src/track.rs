//! Track — core data type representing a single audio file entry.

use heapless::String;

/// Audio container/codec format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioFormat {
    /// Free Lossless Audio Codec
    Flac,
    /// MPEG Audio Layer III
    Mp3,
    /// Waveform Audio File Format
    Wav,
}

/// A single scanned audio track stored in the library index.
///
/// Sized to fit comfortably in a `heapless::Vec`; large collections must live
/// in external SDRAM (mapped at 0xC000_0000) rather than on the stack.
#[derive(Debug, Clone)]
pub struct Track {
    /// Display title (up to 128 UTF-8 bytes)
    pub title: String<128>,
    /// Artist name (up to 64 UTF-8 bytes)
    pub artist: String<64>,
    /// Album title (up to 64 UTF-8 bytes)
    pub album: String<64>,
    /// Full path on the FAT32 volume (up to 256 bytes)
    pub file_path: String<256>,
    /// Duration in whole seconds
    pub duration_secs: u32,
    /// Sample rate in Hz (e.g. 44100, 48000, 96000)
    pub sample_rate: u32,
    /// Container/codec format
    pub format: AudioFormat,
}

impl Track {
    /// Create a minimal `Track` with only the file path and format set.
    ///
    /// All text fields are empty strings; `duration_secs` is 0;
    /// `sample_rate` is 44 100 Hz (Red Book CD Audio default).
    #[allow(clippy::indexing_slicing)] // Safety: file_path.len() <= 256 checked above
    #[allow(clippy::expect_used)] // Safety: push_str only fails if len > capacity, guarded above
    pub fn new(file_path: &str, format: AudioFormat) -> Self {
        let mut path_buf = String::<256>::new();
        // Truncate silently if the path exceeds the buffer capacity.
        let trimmed = if file_path.len() <= 256 {
            file_path
        } else {
            &file_path[..256]
        };
        // SAFETY: push_str only fails if capacity is exceeded; we already
        // guaranteed trimmed.len() <= 256 == capacity.
        path_buf
            .push_str(trimmed)
            .expect("file_path fits within String<256>");

        Track {
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            file_path: path_buf,
            duration_secs: 0,
            sample_rate: 44_100,
            format,
        }
    }
}

impl Default for Track {
    fn default() -> Self {
        Track::new("", AudioFormat::Flac)
    }
}

#[cfg(test)]
#[allow(clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn test_track_title_max_len() {
        let t = Track::default();
        // title must be a heapless::String<128>; capacity is 128 bytes
        assert_eq!(t.title.capacity(), 128);
    }

    #[test]
    fn test_track_artist_max_len() {
        let t = Track::default();
        assert_eq!(t.artist.capacity(), 64);
    }

    #[test]
    fn test_track_album_max_len() {
        let t = Track::default();
        assert_eq!(t.album.capacity(), 64);
    }

    #[test]
    fn test_track_duration_seconds() {
        let mut t = Track::default();
        t.duration_secs = 300;
        assert_eq!(t.duration_secs, 300);
    }

    #[test]
    fn test_track_sample_rate() {
        let t = Track::default();
        // default sample rate must be 44100
        assert_eq!(t.sample_rate, 44100);
    }

    #[test]
    fn test_track_format() {
        let t = Track::new("/music/test.mp3", AudioFormat::Mp3);
        assert_eq!(t.format, AudioFormat::Mp3);
    }

    #[test]
    fn test_track_is_copy() {
        let t = Track::default();
        // Track implements Clone; verify that clone produces an equal value
        let cloned = t.clone();
        assert_eq!(cloned.duration_secs, t.duration_secs);
        assert_eq!(cloned.sample_rate, t.sample_rate);
        assert_eq!(cloned.format, t.format);
    }

    #[test]
    fn test_track_file_path_stored() {
        let t = Track::new("/music/album/track01.flac", AudioFormat::Flac);
        assert_eq!(t.file_path.as_str(), "/music/album/track01.flac");
        // capacity must be 256 bytes
        assert_eq!(t.file_path.capacity(), 256);
    }
}
