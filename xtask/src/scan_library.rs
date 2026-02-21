//! xtask scan-library — scan a local music folder and write Soul binary files.
//!
//! Metadata is inferred from folder structure: `{Artist}/{Album}/{NN} - {Title}.{ext}`
//! No tag parsing — if you need accurate metadata, use Soul Player export instead.

use std::path::{Path, PathBuf};

use anyhow::Result;
use library::binary::{sort_key_for, TrackMeta};
use library::writer::LibraryWriter;
use walkdir::WalkDir;

const AUDIO_EXTENSIONS: &[&str] = &["flac", "mp3", "wav", "aiff", "ogg", "opus", "m4a"];

/// Entry point called from main.rs
pub fn run(music_dir: &Path, soul_root: &Path) -> Result<()> {
    println!("Scanning: {}", music_dir.display());
    run_scan(music_dir, soul_root)
}

/// Scan `music_dir` and write binary library to `soul_root`.
pub(crate) fn run_scan(music_dir: &Path, soul_root: &Path) -> Result<()> {
    let entries = scan_audio_files(music_dir)?;
    println!("Found {} audio files", entries.len());

    let mut metas: Vec<([u8; 16], TrackMeta)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, path)| {
            // SAFETY: a music library with > 4 billion tracks is not realistic on embedded storage.
            #[allow(clippy::cast_possible_truncation)]
            let soul_id = (i as u32).saturating_add(1);
            // TODO: derive album_id from artist+album hash once multi-album grouping is needed.
            // Currently all tracks share album_id=1, making album_count in the manifest always 1.
            let meta = infer_meta_from_path(path, soul_id, 1).ok()?;
            let key = sort_key_for(
                meta.artist.as_str(),
                meta.album.as_str(),
                meta.track_number,
                meta.disc_number,
            );
            Some((key, meta))
        })
        .collect();

    metas.sort_by_key(|(k, _)| *k);

    let root_str = soul_root
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("invalid soul_root path"))?;
    let mut writer = LibraryWriter::new(root_str)?;
    let mut album_ids = std::collections::HashSet::new();
    for (key, meta) in metas {
        album_ids.insert(meta.album_id);
        writer.add_track(key, meta)?;
    }
    // SAFETY: a library with > 4 billion distinct albums is not realistic.
    #[allow(clippy::cast_possible_truncation)]
    let album_count = album_ids.len() as u32;
    writer.finish(album_count, unix_now())?;

    println!("Written to: {}", soul_root.display());
    Ok(())
}

/// Recursively collect all audio file paths under `dir`.
pub(crate) fn scan_audio_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).sort_by_file_name() {
        let entry = entry?;
        if entry.file_type().is_file() {
            let ext = entry
                .path()
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if AUDIO_EXTENSIONS.contains(&ext.as_str()) {
                files.push(entry.into_path());
            }
        }
    }
    Ok(files)
}

/// Infer `TrackMeta` from file path components.
///
/// Expected structure: `{Artist}/{Album}/{NN} - {Title}.{ext}`
/// or fallback to filename as title with empty artist/album.
pub(crate) fn infer_meta_from_path(path: &Path, soul_id: u32, album_id: u32) -> Result<TrackMeta> {
    let components: Vec<&str> = path.iter().filter_map(|c| c.to_str()).collect();

    let n = components.len();
    // SAFETY: n >= 3 is checked before indexing; n.saturating_sub(3) < n <= components.len().
    #[allow(clippy::indexing_slicing)]
    let artist = if n >= 3 { components[n.saturating_sub(3)] } else { "" };
    // SAFETY: n >= 2 is checked before indexing; n.saturating_sub(2) < n <= components.len().
    #[allow(clippy::indexing_slicing)]
    let album = if n >= 2 { components[n.saturating_sub(2)] } else { "" };
    let filename = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");

    let (track_number, title) = parse_filename(filename);

    let format = match path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "flac" | "aiff" => 0,
        "mp3" => 1,
        "wav" => 2,
        _ => 0,
    };

    Ok(TrackMeta {
        soul_id,
        album_id,
        track_number,
        disc_number: 1,
        year: 0,
        format,
        channels: 2,
        duration_secs: 0,
        sample_rate: 44_100,
        title: heapless::String::try_from(title).unwrap_or_default(),
        artist: heapless::String::try_from(artist).unwrap_or_default(),
        album: heapless::String::try_from(album).unwrap_or_default(),
        file_path: heapless::String::try_from(path.to_str().unwrap_or("")).unwrap_or_default(),
    })
}

/// Parse `"02 - Track Title"` → `(2, "Track Title")`.
/// Returns `(0, filename)` if no leading number found.
fn parse_filename(filename: &str) -> (u16, &str) {
    let mut chars = filename.char_indices().peekable();
    let mut num_end = 0;
    let mut has_num = false;
    while let Some((i, c)) = chars.peek() {
        if c.is_ascii_digit() {
            num_end = i.saturating_add(1);
            has_num = true;
            chars.next();
        } else {
            break;
        }
    }
    if !has_num {
        return (0, filename);
    }
    // SAFETY: All matched chars satisfy `is_ascii_digit()`, so each char is
    // exactly 1 byte wide in UTF-8. `num_end` is set to `i.saturating_add(1)`
    // where `i` is the byte offset of the last digit — a valid UTF-8 char
    // boundary and a value <= filename.len(). The `has_num` guard guarantees
    // at least one digit was seen before we reach these slice operations.
    #[allow(clippy::indexing_slicing)]
    let num: u16 = filename[..num_end].parse().unwrap_or(0);
    #[allow(clippy::indexing_slicing)]
    let rest = filename[num_end..].trim_start_matches([' ', '-', '.']);
    (num, rest)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_fake_library(dir: &TempDir) {
        let artist_dir = dir.path().join("Amon Tobin").join("Foley Room");
        fs::create_dir_all(&artist_dir).unwrap();
        fs::write(artist_dir.join("01 - Foley Room Remix.flac"), b"FAKE").unwrap();
        fs::write(artist_dir.join("02 - Kitchen Sink.flac"), b"FAKE").unwrap();

        let b = dir.path().join("Portishead").join("Dummy");
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join("01 - Mysterons.flac"), b"FAKE").unwrap();
    }

    #[test]
    fn scan_finds_all_audio_files() {
        let tmp = TempDir::new().unwrap();
        create_fake_library(&tmp);
        let entries = scan_audio_files(tmp.path()).unwrap();
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn infer_meta_parses_track_num_from_filename() {
        let path = std::path::Path::new("/soul/Amon Tobin/Foley Room/02 - Kitchen Sink.flac");
        let meta = infer_meta_from_path(path, 1, 1).unwrap();
        assert_eq!(meta.track_number, 2);
        assert_eq!(meta.title.as_str(), "Kitchen Sink");
        assert_eq!(meta.artist.as_str(), "Amon Tobin");
        assert_eq!(meta.album.as_str(), "Foley Room");
    }

    #[test]
    fn infer_meta_handles_no_track_number() {
        let path = std::path::Path::new("/soul/Artist/Album/song.mp3");
        let meta = infer_meta_from_path(path, 1, 1).unwrap();
        assert_eq!(meta.track_number, 0);
        assert_eq!(meta.title.as_str(), "song");
    }

    #[test]
    fn scan_and_write_creates_binary_files() {
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        create_fake_library(&src);
        run_scan(src.path(), dst.path()).unwrap();
        assert!(dst.path().join("manifest.bin").exists());
        assert!(dst.path().join("library.idx").exists());
        assert!(dst.path().join("library.meta").exists());
    }

    #[test]
    fn scan_and_write_track_count_matches() {
        use library::binary::ManifestBin;
        let src = TempDir::new().unwrap();
        let dst = TempDir::new().unwrap();
        create_fake_library(&src);
        run_scan(src.path(), dst.path()).unwrap();

        let bytes = std::fs::read(dst.path().join("manifest.bin")).unwrap();
        let manifest = ManifestBin::decode(bytes.as_slice().try_into().unwrap()).unwrap();
        assert_eq!(manifest.track_count, 3);
    }
}
