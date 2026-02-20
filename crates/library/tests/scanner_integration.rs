//! Integration tests for library scanner with mock file system entries.
//!
//! These tests verify the extension filter, magic-byte format detection, and
//! the `ScanEntry` construction pipeline without requiring any real file-system
//! I/O.  No `embedded-sdmmc` dependency is needed here.

use library::metadata::detect_format;
use library::scanner::Scanner;
use library::track::AudioFormat;

#[test]
fn test_scanner_rejects_non_audio() {
    assert!(!Scanner::is_supported_extension("jpg"));
    assert!(!Scanner::is_supported_extension("mp4"));
    assert!(!Scanner::is_supported_extension("pdf"));
    assert!(!Scanner::is_supported_extension(""));
    assert!(!Scanner::is_supported_extension("exe"));
}

#[test]
fn test_scanner_accepts_all_audio_formats() {
    assert!(Scanner::is_supported_extension("flac"));
    assert!(Scanner::is_supported_extension("FLAC"));
    assert!(Scanner::is_supported_extension("mp3"));
    assert!(Scanner::is_supported_extension("MP3"));
    assert!(Scanner::is_supported_extension("wav"));
    assert!(Scanner::is_supported_extension("WAV"));
}

#[test]
fn test_metadata_flac_magic() {
    // fLaC = [0x66, 0x4C, 0x61, 0x43]
    let header = [0x66u8, 0x4C, 0x61, 0x43, 0x00, 0x00];
    assert_eq!(detect_format(&header), Some(AudioFormat::Flac));
}

#[test]
fn test_metadata_mp3_id3() {
    let header = [0x49u8, 0x44, 0x33, 0x04, 0x00, 0x00]; // ID3v2.4
    assert_eq!(detect_format(&header), Some(AudioFormat::Mp3));
}

#[test]
fn test_metadata_mp3_sync_word() {
    // 0xFF 0xFB = MPEG1 Layer3 with sync word
    let header = [0xFFu8, 0xFB, 0x90, 0x00];
    assert_eq!(detect_format(&header), Some(AudioFormat::Mp3));
}

#[test]
fn test_metadata_wav_riff() {
    let header = [0x52u8, 0x49, 0x46, 0x46, 0x00, 0x00]; // RIFF
    assert_eq!(detect_format(&header), Some(AudioFormat::Wav));
}

#[test]
fn test_mock_scan_pipeline() {
    // Simulates scanning a directory listing with mock file entries.
    let file_names = [
        "01 - Track.flac",
        "02 - Other.mp3",
        "cover.jpg",
        "playlist.m3u",
    ];
    let audio_files: Vec<_> = file_names
        .iter()
        .filter_map(|name| {
            let ext = name.rsplit('.').next()?;
            if Scanner::is_supported_extension(ext) {
                Some(*name)
            } else {
                None
            }
        })
        .collect();

    assert_eq!(audio_files.len(), 2);
    assert!(audio_files.contains(&"01 - Track.flac"));
    assert!(audio_files.contains(&"02 - Other.mp3"));
}

#[test]
fn test_scan_entry_creation() {
    use heapless::String;
    use library::scanner::ScanEntry;

    let mut path: String<256> = String::new();
    path.push_str("/music/01 - Track.flac").ok();

    let entry = ScanEntry {
        path,
        format: AudioFormat::Flac,
    };
    assert_eq!(entry.format, AudioFormat::Flac);
    assert!(entry.path.as_str().ends_with(".flac"));
}
