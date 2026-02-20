//! Metadata — magic-byte format detection and audio tag parsing.
//!
//! Format detection is based on the first few bytes of the file header.
//! No file-system I/O is performed here; the caller must supply the bytes.

use crate::track::AudioFormat;

/// Detect the audio format from the first bytes of a file.
///
/// Pass at least 4 bytes for reliable detection.
///
/// | Format | Magic bytes                          |
/// |--------|--------------------------------------|
/// | FLAC   | `fLaC` (0x66 0x4C 0x61 0x43)        |
/// | MP3    | `ID3`  (0x49 0x44 0x33)              |
/// | MP3    | MPEG sync word (0xFF, high 3 bits of next byte = 0xE0) |
/// | WAV    | `RIFF` (0x52 0x49 0x46 0x46)        |
///
/// Returns `None` when the header is empty or does not match a known format.
pub fn detect_format(header: &[u8]) -> Option<AudioFormat> {
    // FLAC: 'f','L','a','C'
    if header.len() >= 4 && &header[..4] == b"fLaC" {
        return Some(AudioFormat::Flac);
    }

    // MP3 with ID3 tag: 'I','D','3'
    if header.len() >= 3 && &header[..3] == b"ID3" {
        return Some(AudioFormat::Mp3);
    }

    // MP3 sync word: 0xFF followed by a byte where the top 3 bits are all 1
    // (covers MPEG-1/2/2.5, all layers)
    if header.len() >= 2 && header[0] == 0xFF && (header[1] & 0xE0) == 0xE0 {
        return Some(AudioFormat::Mp3);
    }

    // WAV / RIFF: 'R','I','F','F'
    if header.len() >= 4 && &header[..4] == b"RIFF" {
        return Some(AudioFormat::Wav);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::track::AudioFormat;

    #[test]
    fn test_flac_signature_detection() {
        // FLAC magic: 'f', 'L', 'a', 'C'  (0x66, 0x4C, 0x61, 0x43)
        assert_eq!(
            detect_format(&[0x66, 0x4C, 0x61, 0x43]),
            Some(AudioFormat::Flac)
        );
    }

    #[test]
    fn test_mp3_id3_signature() {
        // ID3 header: 'I', 'D', '3'  (0x49, 0x44, 0x33)
        assert_eq!(
            detect_format(&[0x49, 0x44, 0x33, 0x03]),
            Some(AudioFormat::Mp3)
        );
    }

    #[test]
    fn test_mp3_sync_signature() {
        // MPEG sync word: 0xFF 0xFB (MPEG1, Layer III, 128 kbps)
        assert_eq!(
            detect_format(&[0xFF, 0xFB, 0x00, 0x00]),
            Some(AudioFormat::Mp3)
        );
    }

    #[test]
    fn test_wav_riff_signature() {
        // RIFF/WAV header: 'R', 'I', 'F', 'F'  (0x52, 0x49, 0x46, 0x46)
        assert_eq!(
            detect_format(&[0x52, 0x49, 0x46, 0x46]),
            Some(AudioFormat::Wav)
        );
    }

    #[test]
    fn test_unknown_signature() {
        assert_eq!(detect_format(&[0x00, 0x00]), None);
    }

    #[test]
    fn test_empty_bytes_returns_none() {
        assert_eq!(detect_format(&[]), None);
    }
}
