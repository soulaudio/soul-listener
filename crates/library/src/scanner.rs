//! Scanner — walks a FAT32 directory tree and emits supported audio file entries.

use crate::track::AudioFormat;
use heapless::String;

/// A single audio file discovered during a directory scan.
pub struct ScanEntry {
    /// Full path on the FAT32 volume (up to 256 bytes).
    pub path: String<256>,
    /// Detected audio format (from file extension).
    pub format: AudioFormat,
}

/// Stateless helper for file-system traversal and extension filtering.
pub struct Scanner;

impl Scanner {
    /// Returns `true` when `ext` is a supported audio file extension.
    ///
    /// The comparison is **case-insensitive** and does not allocate; it
    /// operates entirely in `core` so it is `no_std` compatible.
    ///
    /// Supported extensions: `flac`, `mp3`, `wav`.
    pub fn is_supported_extension(ext: &str) -> bool {
        // Compare byte-by-byte with ASCII lowercasing to avoid std::string.
        eq_ignore_ascii_case(ext, "flac")
            || eq_ignore_ascii_case(ext, "mp3")
            || eq_ignore_ascii_case(ext, "wav")
    }

    /// Derive an [`AudioFormat`] from a file extension, or return `None`.
    pub fn format_for_extension(ext: &str) -> Option<AudioFormat> {
        if eq_ignore_ascii_case(ext, "flac") {
            Some(AudioFormat::Flac)
        } else if eq_ignore_ascii_case(ext, "mp3") {
            Some(AudioFormat::Mp3)
        } else if eq_ignore_ascii_case(ext, "wav") {
            Some(AudioFormat::Wav)
        } else {
            None
        }
    }
}

/// Compare two byte strings case-insensitively (ASCII only).
///
/// This avoids any `std` dependency; it is equivalent to
/// `a.eq_ignore_ascii_case(b)` which is available in `core`.
fn eq_ignore_ascii_case(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scanner_recognises_flac_extension() {
        assert!(Scanner::is_supported_extension("flac"));
    }

    #[test]
    fn test_scanner_recognises_mp3() {
        assert!(Scanner::is_supported_extension("mp3"));
    }

    #[test]
    fn test_scanner_rejects_jpg() {
        assert!(!Scanner::is_supported_extension("jpg"));
    }

    #[test]
    fn test_scanner_rejects_empty() {
        assert!(!Scanner::is_supported_extension(""));
    }

    #[test]
    fn test_scanner_extension_case_insensitive() {
        assert!(Scanner::is_supported_extension("FLAC"));
        assert!(Scanner::is_supported_extension("MP3"));
        assert!(Scanner::is_supported_extension("WAV"));
    }

    #[test]
    fn test_scan_result_has_path_and_format() {
        let entry = ScanEntry {
            path: {
                let mut s = String::<256>::new();
                s.push_str("/music/test.flac").expect("push_str");
                s
            },
            format: AudioFormat::Flac,
        };
        assert_eq!(entry.path.as_str(), "/music/test.flac");
        assert_eq!(entry.format, AudioFormat::Flac);
    }
}
