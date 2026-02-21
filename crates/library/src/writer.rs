//! LibraryWriter — build and write the Soul binary library files.
//!
//! Only compiled with the `std` feature (used by the `scan-library` xtask).
//! Tracks must be added in sorted order (ascending `sort_key`).

#[cfg(not(feature = "std"))]
compile_error!("library::writer requires the `std` feature");

use std::fs;
use std::path::PathBuf;

use crc32fast::Hasher;

use crate::binary::{IndexEntry, ManifestBin, TrackMeta};

/// Error type for `LibraryWriter` operations.
#[derive(Debug)]
pub enum WriterError {
    /// An I/O error from std::io.
    Io(std::io::Error),
    /// postcard serialisation failed.
    Postcard(postcard::Error),
}

impl core::fmt::Display for WriterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "I/O error: {}", e),
            Self::Postcard(e) => write!(f, "postcard error: {}", e),
        }
    }
}

impl From<std::io::Error> for WriterError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<postcard::Error> for WriterError {
    fn from(e: postcard::Error) -> Self {
        Self::Postcard(e)
    }
}

/// Builds `manifest.bin`, `library.idx`, and `library.meta` under `soul_root`.
///
/// Tracks **must** be added in ascending `sort_key` order.  The caller is
/// responsible for sorting before calling [`LibraryWriter::add_track`].
///
/// `manifest.bin` is written last so that a partial write leaves the existing
/// manifest intact — the reader treats a missing/stale manifest as "rescan needed".
pub struct LibraryWriter {
    root: PathBuf,
    idx_buf: Vec<u8>,
    meta_buf: Vec<u8>,
}

impl LibraryWriter {
    /// Create a new writer targeting `soul_root`.
    ///
    /// Creates the root directory if it does not exist.
    ///
    /// # Errors
    ///
    /// Returns `WriterError::Io` if the directory cannot be created.
    pub fn new(soul_root: &str) -> Result<Self, WriterError> {
        let root = PathBuf::from(soul_root);
        fs::create_dir_all(&root)?;
        Ok(Self {
            root,
            idx_buf: Vec::new(),
            meta_buf: Vec::new(),
        })
    }

    /// Append one track to the in-memory buffers.
    ///
    /// `sort_key` must be computed via [`crate::binary::sort_key_for`].
    /// Tracks must be added in ascending sort-key order.
    ///
    /// # Errors
    ///
    /// Returns `WriterError::Postcard` if `meta` cannot be serialised.
    pub fn add_track(&mut self, sort_key: [u8; 16], meta: TrackMeta) -> Result<(), WriterError> {
        // SAFETY: a library with > 4 billion tracks (128 GB in idx alone) is not realistic.
        #[allow(clippy::cast_possible_truncation)]
        let meta_offset = self.meta_buf.len() as u32;
        let mut postcard_buf = [0u8; 512];
        let encoded = postcard::to_slice(&meta, &mut postcard_buf)?;
        // SAFETY: a library with > 4 billion tracks (128 GB in idx alone) is not realistic.
        #[allow(clippy::cast_possible_truncation)]
        let meta_size = encoded.len() as u32;

        let entry = IndexEntry {
            sort_key,
            meta_offset,
            meta_size,
        };
        self.idx_buf.extend_from_slice(&entry.encode());
        self.meta_buf.extend_from_slice(encoded);

        Ok(())
    }

    /// Write all files and return.
    ///
    /// `album_count` is the number of unique album IDs seen during scanning.
    /// `export_timestamp` is Unix seconds; pass `0` for dev builds.
    ///
    /// # Errors
    ///
    /// Returns `WriterError::Io` if any file write fails.
    pub fn finish(self, album_count: u32, export_timestamp: u64) -> Result<(), WriterError> {
        // SAFETY: a library with > 4 billion tracks (128 GB in idx alone) is not realistic.
        #[allow(clippy::cast_possible_truncation)]
        let track_count = (self.idx_buf.len() / IndexEntry::SIZE) as u32;

        let mut idx_hasher = Hasher::new();
        idx_hasher.update(&self.idx_buf);
        let idx_checksum = idx_hasher.finalize();

        let mut meta_hasher = Hasher::new();
        meta_hasher.update(&self.meta_buf);
        let meta_checksum = meta_hasher.finalize();

        // Write idx and meta first; manifest last (atomic-ish)
        fs::write(self.root.join("library.idx"), &self.idx_buf)?;
        fs::write(self.root.join("library.meta"), &self.meta_buf)?;

        let manifest = ManifestBin {
            track_count,
            album_count,
            export_timestamp,
            idx_checksum,
            meta_checksum,
        };
        fs::write(self.root.join("manifest.bin"), manifest.encode())?;

        Ok(())
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use crate::binary::sort_key_for;
    use tempfile::TempDir;

    fn sample_meta(n: u32) -> TrackMeta {
        TrackMeta {
            soul_id: n,
            album_id: 1,
            track_number: n as u16,
            disc_number: 1,
            year: 2024,
            format: 0,
            channels: 2,
            duration_secs: 240,
            sample_rate: 44_100,
            title: heapless::String::try_from(format!("Track {}", n).as_str()).unwrap(),
            artist: heapless::String::try_from("Test Artist").unwrap(),
            album: heapless::String::try_from("Test Album").unwrap(),
            file_path: heapless::String::try_from(
                format!("/soul/music/ta/ta/{:02}.flac", n).as_str(),
            )
            .unwrap(),
        }
    }

    #[test]
    fn writer_creates_three_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        let mut w = LibraryWriter::new(root).unwrap();
        let key = sort_key_for("Test Artist", "Test Album", 1, 1);
        w.add_track(key, sample_meta(1)).unwrap();
        w.finish(1, 1_700_000_000).unwrap();

        assert!(tmp.path().join("manifest.bin").exists());
        assert!(tmp.path().join("library.idx").exists());
        assert!(tmp.path().join("library.meta").exists());
    }

    #[test]
    fn writer_manifest_has_correct_track_count() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        let mut w = LibraryWriter::new(root).unwrap();
        for i in 1..=5u32 {
            let key = sort_key_for("Test Artist", "Test Album", i as u16, 1);
            w.add_track(key, sample_meta(i)).unwrap();
        }
        w.finish(1, 0).unwrap();

        let bytes = std::fs::read(tmp.path().join("manifest.bin")).unwrap();
        let arr: [u8; 64] = bytes.try_into().unwrap();
        let manifest = ManifestBin::decode(&arr).unwrap();
        assert_eq!(manifest.track_count, 5);
    }

    #[test]
    fn writer_idx_size_matches_track_count() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        let mut w = LibraryWriter::new(root).unwrap();
        for i in 1..=7u32 {
            let key = sort_key_for("Artist", "Album", i as u16, 1);
            w.add_track(key, sample_meta(i)).unwrap();
        }
        w.finish(1, 0).unwrap();

        let idx = std::fs::read(tmp.path().join("library.idx")).unwrap();
        assert_eq!(idx.len(), 7 * IndexEntry::SIZE);
    }

    #[test]
    fn writer_produces_crc_matching_manifest() {
        use crc32fast::Hasher;
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        let mut w = LibraryWriter::new(root).unwrap();
        let key = sort_key_for("A", "B", 1, 1);
        w.add_track(key, sample_meta(1)).unwrap();
        w.finish(1, 0).unwrap();

        let manifest_bytes = std::fs::read(tmp.path().join("manifest.bin")).unwrap();
        let arr: [u8; 64] = manifest_bytes.try_into().unwrap();
        let manifest = ManifestBin::decode(&arr).unwrap();

        let idx_bytes = std::fs::read(tmp.path().join("library.idx")).unwrap();
        let mut h = Hasher::new();
        h.update(&idx_bytes);
        assert_eq!(manifest.idx_checksum, h.finalize());

        let meta_bytes = std::fs::read(tmp.path().join("library.meta")).unwrap();
        let mut h2 = Hasher::new();
        h2.update(&meta_bytes);
        assert_eq!(manifest.meta_checksum, h2.finalize());
    }
}
