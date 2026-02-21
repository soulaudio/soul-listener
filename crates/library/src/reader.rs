//! `SoulLibraryReader` — generic reader for the Soul binary library format.
//!
//! Parameterised over any [`platform::Storage`] implementation.
//! Reads on-demand from `library.idx` and `library.meta` without pre-loading.
//!
//! # Access patterns
//!
//! | Method | Performance | Notes |
//! |--------|-------------|-------|
//! | `track(index)` | O(1) seek + O(meta_size) read | Single track by index |
//! | `page(offset, count)` | O(count) seeks + reads | Page for UI browsing |
//! | `search_by_artist(prefix)` | O(log N) binary search | Artist name prefix search |

use platform::soul_library::{library_idx_path, library_meta_path, manifest_path};
use platform::storage::{File, Storage};

use crate::binary::{IndexEntry, LibraryError, ManifestBin, TrackMeta};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error from `SoulLibraryReader` operations.
#[derive(Debug)]
pub enum ReaderError<E: core::fmt::Debug> {
    /// I/O error from the underlying `Storage` implementation.
    Storage(E),
    /// Binary format error (bad magic, unsupported version, corrupt data).
    Format(LibraryError),
    /// Track index is out of range (>= `track_count()`).
    OutOfRange,
}

impl<E: core::fmt::Debug> From<LibraryError> for ReaderError<E> {
    fn from(e: LibraryError) -> Self {
        Self::Format(e)
    }
}

// ---------------------------------------------------------------------------
// SoulLibraryReader
// ---------------------------------------------------------------------------

/// Generic reader for the Soul binary library.
///
/// Call [`SoulLibraryReader::open`] to read and validate `manifest.bin`,
/// then use [`track`](SoulLibraryReader::track) or [`page`](SoulLibraryReader::page)
/// to retrieve metadata.
///
/// The type constraint `S::File: File<Error = S::Error>` requires that both
/// the `Storage` and its associated `File` share the same error type. This is
/// satisfied by all concrete implementations (e.g. `LocalFileStorage` uses
/// `LocalStorageError` for both).
pub struct SoulLibraryReader<S>
where
    S: Storage,
    S::File: File<Error = S::Error>,
{
    storage: S,
    root: heapless::String<64>,
    manifest: ManifestBin,
}

impl<S> SoulLibraryReader<S>
where
    S: Storage,
    S::File: File<Error = S::Error>,
{
    /// Open the library at `soul_root`.
    ///
    /// Reads and validates `manifest.bin`. Does not pre-load the index.
    ///
    /// # Errors
    ///
    /// Returns `ReaderError::Storage` if `manifest.bin` cannot be opened or read.
    /// Returns `ReaderError::Format` if the manifest magic or version is invalid.
    pub async fn open(mut storage: S, soul_root: &str) -> Result<Self, ReaderError<S::Error>> {
        let mpath = manifest_path(soul_root);
        let mut file = storage
            .open_file(mpath.as_str())
            .await
            .map_err(ReaderError::Storage)?;

        let mut buf = [0u8; ManifestBin::SIZE];
        read_exact(&mut file, &mut buf)
            .await
            .map_err(ReaderError::Storage)?;
        let manifest = ManifestBin::decode(&buf)?;

        let mut root = heapless::String::<64>::new();
        let _ = root.push_str(soul_root);

        Ok(Self { storage, root, manifest })
    }

    /// Number of tracks in the library.
    #[must_use]
    pub fn track_count(&self) -> u32 {
        self.manifest.track_count
    }

    /// Load a single [`TrackMeta`] by 0-based track index.
    ///
    /// # Errors
    ///
    /// Returns `ReaderError::OutOfRange` if `index >= track_count()`.
    /// Returns `ReaderError::Storage` on I/O failure.
    /// Returns `ReaderError::Format` if the meta blob is corrupt.
    pub async fn track(&mut self, index: u32) -> Result<TrackMeta, ReaderError<S::Error>> {
        if index >= self.manifest.track_count {
            return Err(ReaderError::OutOfRange);
        }

        let idx_path = library_idx_path(self.root.as_str());
        let mut idx_file = self
            .storage
            .open_file(idx_path.as_str())
            .await
            .map_err(ReaderError::Storage)?;

        let meta_path = library_meta_path(self.root.as_str());
        let mut meta_file = self
            .storage
            .open_file(meta_path.as_str())
            .await
            .map_err(ReaderError::Storage)?;

        let entry = read_index_entry(&mut idx_file, index).await?;
        read_track_meta(&mut meta_file, &entry).await
    }

    // -- std/test-only methods below --

    /// Load a page of [`TrackMeta`] starting at 0-based `offset`.
    ///
    /// Returns up to `count` tracks. Returns an empty `Vec` if `offset >= track_count()`.
    ///
    /// # Errors
    ///
    /// Returns `ReaderError::Storage` on I/O failure.
    /// Returns `ReaderError::Format` if any meta blob is corrupt.
    #[cfg(any(feature = "std", test))]
    pub async fn page(
        &mut self,
        offset: u32,
        count: u32,
    ) -> Result<Vec<TrackMeta>, ReaderError<S::Error>> {
        let total = self.manifest.track_count;
        if offset >= total {
            return Ok(Vec::new());
        }
        let available = total.saturating_sub(offset).min(count);

        let idx_path = library_idx_path(self.root.as_str());
        let mut idx_file = self
            .storage
            .open_file(idx_path.as_str())
            .await
            .map_err(ReaderError::Storage)?;

        let meta_path = library_meta_path(self.root.as_str());
        let mut meta_file = self
            .storage
            .open_file(meta_path.as_str())
            .await
            .map_err(ReaderError::Storage)?;

        // available <= count <= u32::MAX; usize is at least 32 bits on all targets.
        #[allow(clippy::cast_possible_truncation)]
        let mut result = Vec::with_capacity(available as usize);
        for i in 0..available {
            let track_idx = offset.saturating_add(i);
            let entry = read_index_entry(&mut idx_file, track_idx).await?;
            let meta = read_track_meta(&mut meta_file, &entry).await?;
            result.push(meta);
        }
        Ok(result)
    }

    /// Search for tracks whose sort_key starts with the artist ASCII prefix.
    ///
    /// Uses binary search on the sorted index. Only the first 6 ASCII bytes of
    /// `artist_prefix` are used (the artist portion of the sort key).
    /// Returns up to 64 matching tracks.
    ///
    /// # Errors
    ///
    /// Returns `ReaderError::Storage` on I/O failure.
    /// Returns `ReaderError::Format` on corrupt data.
    #[cfg(any(feature = "std", test))]
    pub async fn search_by_artist(
        &mut self,
        artist_prefix: &str,
    ) -> Result<Vec<TrackMeta>, ReaderError<S::Error>> {
        // Build prefix: first 6 bytes of artist_prefix, lowercased and zero-padded.
        // prefix_len is always <= 6 == prefix.len().
        let prefix_len = artist_prefix.len().min(6);
        let mut prefix = [0u8; 6];
        // SAFETY: i comes from .enumerate() over .take(6), so i < 6 == prefix.len().
        #[allow(clippy::indexing_slicing)]
        for (i, &b) in artist_prefix.as_bytes().iter().take(6).enumerate() {
            prefix[i] = b.to_ascii_lowercase();
        }

        let idx_path = library_idx_path(self.root.as_str());
        let meta_path = library_meta_path(self.root.as_str());
        let mut idx_file = self
            .storage
            .open_file(idx_path.as_str())
            .await
            .map_err(ReaderError::Storage)?;
        let mut meta_file = self
            .storage
            .open_file(meta_path.as_str())
            .await
            .map_err(ReaderError::Storage)?;

        // Binary search: find first entry where sort_key[0..prefix_len] >= prefix[0..prefix_len].
        let total = self.manifest.track_count;
        let mut lo = 0u32;
        let mut hi = total;
        while lo < hi {
            let mid = lo.saturating_add(hi.saturating_sub(lo) / 2);
            let entry = read_index_entry(&mut idx_file, mid).await?;
            // SAFETY: prefix_len <= 6 <= 16 == entry.sort_key.len();
            //         prefix_len <= 6 == prefix.len(). Both slices are in-bounds.
            #[allow(clippy::indexing_slicing)]
            if entry.sort_key[..prefix_len] < prefix[..prefix_len] {
                lo = mid.saturating_add(1);
            } else {
                hi = mid;
            }
        }

        let mut results = Vec::new();
        let mut i = lo;
        while i < total && results.len() < 64 {
            let entry = read_index_entry(&mut idx_file, i).await?;
            // SAFETY: same bounds as binary search above.
            #[allow(clippy::indexing_slicing)]
            if entry.sort_key[..prefix_len] != prefix[..prefix_len] {
                break;
            }
            let meta = read_track_meta(&mut meta_file, &entry).await?;
            results.push(meta);
            i = i.saturating_add(1);
        }
        Ok(results)
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Read a single [`IndexEntry`] from the index file at position `index`.
///
/// Seeks to `index * IndexEntry::SIZE` then reads exactly 24 bytes.
async fn read_index_entry<F, E>(file: &mut F, index: u32) -> Result<IndexEntry, ReaderError<E>>
where
    F: File<Error = E>,
    E: core::fmt::Debug,
{
    // index * 24: u64::from(u32::MAX) * 24 = ~103 GB — well within u64 range.
    let offset = u64::from(index).saturating_mul(IndexEntry::SIZE as u64);
    file.seek(offset).await.map_err(ReaderError::Storage)?;
    let mut buf = [0u8; IndexEntry::SIZE];
    read_exact(file, &mut buf).await.map_err(ReaderError::Storage)?;
    IndexEntry::decode(&buf).map_err(ReaderError::Format)
}

/// Read a [`TrackMeta`] from the meta file at the offset described by `entry`.
///
/// Seeks to `entry.meta_offset`, reads `entry.meta_size` bytes, and decodes via postcard.
///
/// # Stack usage
///
/// Uses a 600-byte stack buffer which covers the worst-case postcard encoding of
/// `TrackMeta` (title=128 + artist=64 + album=64 + file_path=256 + scalars + varints ≈ 540 B).
/// Validated by the writer test `track_meta_worst_case_fits_in_writer_buffer`.
async fn read_track_meta<F, E>(
    file: &mut F,
    entry: &IndexEntry,
) -> Result<TrackMeta, ReaderError<E>>
where
    F: File<Error = E>,
    E: core::fmt::Debug,
{
    file.seek(u64::from(entry.meta_offset))
        .await
        .map_err(ReaderError::Storage)?;

    // meta_size comes from LibraryWriter which encodes into a 600-byte postcard_buf;
    // so meta_size <= 600 by construction. Cast to usize: u32 fits in usize on all targets.
    #[allow(clippy::cast_possible_truncation)]
    let size = entry.meta_size as usize;

    // 600-byte stack buffer covers worst-case TrackMeta postcard encoding (~540 B).
    // large_stack_arrays fires at 512 B; suppressed here with justification above.
    #[allow(clippy::large_stack_arrays)]
    let mut buf = [0u8; 600];

    let n = read_exact_n(file, &mut buf, size)
        .await
        .map_err(ReaderError::Storage)?;

    // SAFETY: n <= size <= 600 == buf.len(); buf[..n] is always in-bounds.
    #[allow(clippy::indexing_slicing)]
    postcard::from_bytes(&buf[..n]).map_err(|_| ReaderError::Format(LibraryError::DecodeError))
}

/// Read exactly `buf.len()` bytes from `file`, retrying on short reads.
async fn read_exact<F: File>(file: &mut F, buf: &mut [u8]) -> Result<(), F::Error> {
    let mut pos = 0;
    while pos < buf.len() {
        // SAFETY: pos < buf.len() so buf[pos..] is a valid non-empty slice.
        #[allow(clippy::indexing_slicing)]
        let n = file.read(&mut buf[pos..]).await?;
        if n == 0 {
            break;
        }
        pos = pos.saturating_add(n);
    }
    Ok(())
}

/// Read up to `n` bytes into `buf[..n]`, retrying on short reads.
///
/// Returns the number of bytes actually read (may be < n at EOF).
async fn read_exact_n<F: File>(file: &mut F, buf: &mut [u8], n: usize) -> Result<usize, F::Error> {
    let limit = n.min(buf.len());
    let mut pos = 0;
    while pos < limit {
        // SAFETY: pos < limit <= buf.len(), so buf[pos..limit] is in-bounds.
        #[allow(clippy::indexing_slicing)]
        let read = file.read(&mut buf[pos..limit]).await?;
        if read == 0 {
            break;
        }
        pos = pos.saturating_add(read);
    }
    Ok(pos)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::large_stack_arrays,
    clippy::cast_possible_truncation
)]
mod tests {
    use super::*;
    use crate::binary::sort_key_for;
    use crate::writer::LibraryWriter;
    use platform::storage_local::LocalFileStorage;
    use tempfile::TempDir;

    fn make_meta(n: u32, artist: &str, album: &str) -> TrackMeta {
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
            title: heapless::String::try_from(format!("Track {:02}", n).as_str()).unwrap(),
            artist: heapless::String::try_from(artist).unwrap(),
            album: heapless::String::try_from(album).unwrap(),
            file_path: heapless::String::try_from(
                format!("/soul/music/{}/{}/{:02}.flac", artist, album, n).as_str(),
            )
            .unwrap(),
        }
    }

    fn build_library(root: &str, tracks: &[(u32, &str, &str)]) {
        let mut entries: Vec<_> = tracks
            .iter()
            .map(|(n, ar, al)| {
                let key = sort_key_for(ar, al, *n as u16, 1);
                (key, make_meta(*n, ar, al))
            })
            .collect();
        entries.sort_by_key(|(k, _)| *k);
        let mut w = LibraryWriter::new(root).unwrap();
        for (key, meta) in entries {
            w.add_track(key, meta).unwrap();
        }
        w.finish(1, 0).unwrap();
    }

    #[tokio::test]
    async fn reader_loads_manifest() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        build_library(root, &[(1, "Artist", "Album"), (2, "Artist", "Album")]);
        let storage = LocalFileStorage::new(root);
        let reader = SoulLibraryReader::open(storage, root).await.unwrap();
        assert_eq!(reader.track_count(), 2);
    }

    #[tokio::test]
    async fn reader_track_returns_correct_meta() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        build_library(root, &[(5, "ZArtist", "ZAlbum")]);
        let storage = LocalFileStorage::new(root);
        let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
        let meta = reader.track(0).await.unwrap();
        assert_eq!(meta.track_number, 5);
        assert_eq!(meta.artist.as_str(), "ZArtist");
    }

    #[tokio::test]
    async fn reader_track_out_of_range_returns_err() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        build_library(root, &[(1, "A", "B")]);
        let storage = LocalFileStorage::new(root);
        let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
        assert!(reader.track(999).await.is_err());
    }

    #[tokio::test]
    async fn reader_page_returns_sorted_tracks() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        build_library(
            root,
            &[
                (1, "Amon Tobin", "Foley Room"),
                (2, "Amon Tobin", "Foley Room"),
                (1, "Portishead", "Dummy"),
            ],
        );
        let storage = LocalFileStorage::new(root);
        let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
        let page = reader.page(0, 3).await.unwrap();
        assert_eq!(page.len(), 3);
        assert_eq!(page[0].artist.as_str(), "Amon Tobin");
        assert_eq!(page[2].artist.as_str(), "Portishead");
    }

    #[tokio::test]
    async fn reader_search_by_artist_finds_subset() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        build_library(
            root,
            &[
                (1, "Amon Tobin", "Foley Room"),
                (2, "Amon Tobin", "Foley Room"),
                (1, "Portishead", "Dummy"),
            ],
        );
        let storage = LocalFileStorage::new(root);
        let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
        let results = reader.search_by_artist("Amon").await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].artist.as_str(), "Amon Tobin");
    }

    #[tokio::test]
    async fn reader_manifest_missing_returns_err() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        let storage = LocalFileStorage::new(root);
        assert!(SoulLibraryReader::open(storage, root).await.is_err());
    }
}
