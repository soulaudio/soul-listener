//! Binary index types for the Soul Library format.
//!
//! Two-file layout:
//! - `library.idx`  — fixed 24-byte `IndexEntry` per track, sorted by `sort_key`
//! - `library.meta` — postcard-encoded `TrackMeta` per track (variable size)
//!
//! Both are prefixed by a 64-byte `ManifestBin` in `manifest.bin`.

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Error variants for library binary decode operations.
#[derive(Debug, PartialEq, Eq)]
pub enum LibraryError {
    /// manifest.bin magic bytes are not b"SOUL"
    BadMagic,
    /// manifest.bin version is not recognised by this implementation
    UnsupportedVersion,
    /// postcard decode failed (corrupt or truncated data)
    DecodeError,
}

// ---------------------------------------------------------------------------
// ManifestBin — 64-byte fixed header in manifest.bin
// ---------------------------------------------------------------------------

/// 64-byte manifest stored at `{soul_root}/manifest.bin`.
///
/// All multi-byte integers are little-endian.
///
/// Layout (64 bytes total):
/// ```text
/// [0..4]   magic            b"SOUL"
/// [4]      version          u8 = 1
/// [5..8]   _pad             [u8; 3]
/// [8..12]  track_count      u32 le
/// [12..16] album_count      u32 le
/// [16..24] export_timestamp u64 le  (Unix seconds)
/// [24..28] idx_checksum     u32 le  (CRC32 of library.idx)
/// [28..32] meta_checksum    u32 le  (CRC32 of library.meta)
/// [32..64] _pad             [u8; 32]
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestBin {
    pub track_count: u32,
    pub album_count: u32,
    pub export_timestamp: u64,
    pub idx_checksum: u32,
    pub meta_checksum: u32,
}

impl ManifestBin {
    pub const SIZE: usize = 64;
    pub const MAGIC: &'static [u8; 4] = b"SOUL";
    pub const VERSION: u8 = 1;

    /// Encode the manifest into a 64-byte buffer.
    ///
    /// # Safety (lint allow)
    /// All range indices are compile-time constants within `[0, SIZE)`.
    /// The buffer is `[u8; Self::SIZE]` so all slices are always valid.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..4].copy_from_slice(Self::MAGIC);
        buf[4] = Self::VERSION;
        buf[8..12].copy_from_slice(&self.track_count.to_le_bytes());
        buf[12..16].copy_from_slice(&self.album_count.to_le_bytes());
        buf[16..24].copy_from_slice(&self.export_timestamp.to_le_bytes());
        buf[24..28].copy_from_slice(&self.idx_checksum.to_le_bytes());
        buf[28..32].copy_from_slice(&self.meta_checksum.to_le_bytes());
        buf
    }

    /// Decode a manifest from a 64-byte buffer.
    ///
    /// # Errors
    ///
    /// Returns [`LibraryError::BadMagic`] if bytes `[0..4]` are not `b"SOUL"`.
    /// Returns [`LibraryError::UnsupportedVersion`] if byte `[4]` is not [`ManifestBin::VERSION`].
    /// Returns [`LibraryError::DecodeError`] if a fixed-size sub-slice cannot be
    /// converted (structurally unreachable for a `&[u8; 64]` argument).
    ///
    /// # Safety (lint allow)
    /// All range indices are compile-time constants within `[0, SIZE)`.
    /// The buffer is `&[u8; Self::SIZE]` so all slices are always valid.
    #[allow(clippy::indexing_slicing)]
    pub fn decode(buf: &[u8; Self::SIZE]) -> Result<Self, LibraryError> {
        if buf.get(0..4) != Some(Self::MAGIC.as_ref()) {
            return Err(LibraryError::BadMagic);
        }
        if buf.get(4).copied() != Some(Self::VERSION) {
            return Err(LibraryError::UnsupportedVersion);
        }
        Ok(Self {
            track_count: u32::from_le_bytes(
                buf[8..12].try_into().map_err(|_| LibraryError::DecodeError)?,
            ),
            album_count: u32::from_le_bytes(
                buf[12..16].try_into().map_err(|_| LibraryError::DecodeError)?,
            ),
            export_timestamp: u64::from_le_bytes(
                buf[16..24].try_into().map_err(|_| LibraryError::DecodeError)?,
            ),
            idx_checksum: u32::from_le_bytes(
                buf[24..28].try_into().map_err(|_| LibraryError::DecodeError)?,
            ),
            meta_checksum: u32::from_le_bytes(
                buf[28..32].try_into().map_err(|_| LibraryError::DecodeError)?,
            ),
        })
    }
}

// ---------------------------------------------------------------------------
// IndexEntry — 24-byte record in library.idx
// ---------------------------------------------------------------------------

/// A single 24-byte entry in `library.idx`.
///
/// Entries are sorted by `sort_key` ascending to allow binary search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    pub sort_key: [u8; 16],
    pub meta_offset: u32,
    pub meta_size: u32,
}

impl IndexEntry {
    pub const SIZE: usize = 24;

    /// Encode the entry into a 24-byte buffer.
    ///
    /// # Safety (lint allow)
    /// All range indices are compile-time constants within `[0, SIZE)`.
    /// The buffer is `[u8; Self::SIZE]` so all slices are always valid.
    #[must_use]
    #[allow(clippy::indexing_slicing)]
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..16].copy_from_slice(&self.sort_key);
        buf[16..20].copy_from_slice(&self.meta_offset.to_le_bytes());
        buf[20..24].copy_from_slice(&self.meta_size.to_le_bytes());
        buf
    }

    /// Decode an entry from a 24-byte buffer.
    ///
    /// # Errors
    ///
    /// Returns [`LibraryError::DecodeError`] if a fixed-size sub-slice cannot be
    /// converted (structurally unreachable for a `&[u8; 24]` argument).
    ///
    /// # Safety (lint allow)
    /// All range indices are compile-time constants within `[0, SIZE)`.
    /// The buffer is `&[u8; Self::SIZE]` so all slices are always valid.
    /// All sub-slices are exactly 4 bytes and the `try_into()` conversions
    /// are infallible at this type; `map_err` handles the case should the
    /// signature ever change.
    #[allow(clippy::indexing_slicing)]
    pub fn decode(buf: &[u8; Self::SIZE]) -> Result<Self, LibraryError> {
        let mut sort_key = [0u8; 16];
        sort_key.copy_from_slice(&buf[0..16]);
        Ok(Self {
            sort_key,
            meta_offset: u32::from_le_bytes(
                buf[16..20].try_into().map_err(|_| LibraryError::DecodeError)?,
            ),
            meta_size: u32::from_le_bytes(
                buf[20..24].try_into().map_err(|_| LibraryError::DecodeError)?,
            ),
        })
    }
}

// ---------------------------------------------------------------------------
// sort_key_for
// ---------------------------------------------------------------------------

/// Build a 16-byte sort key from artist, album, track number, and disc number.
///
/// Layout:
/// - bytes `[0..6]`   — first 6 bytes of `artist`, lowercased, zero-padded
/// - bytes `[6..12]`  — first 6 bytes of `album`, lowercased, zero-padded
/// - bytes `[12..14]` — `track_num` big-endian (high byte first)
/// - bytes `[14..16]` — `disc_num` big-endian (high byte first)
#[must_use]
#[allow(clippy::cast_possible_truncation)] // intentional: >> 8 keeps only the low 8 bits
#[allow(clippy::indexing_slicing)] // SAFETY: i comes from .take(6).enumerate() so i < 6 < 16; key[12..15] are literal constants < 16
pub fn sort_key_for(artist: &str, album: &str, track_num: u16, disc_num: u16) -> [u8; 16] {
    let mut key = [0u8; 16];
    for (i, &b) in artist.as_bytes().iter().take(6).enumerate() {
        key[i] = b.to_ascii_lowercase();
    }
    for (i, &b) in album.as_bytes().iter().take(6).enumerate() {
        key[6_usize.saturating_add(i)] = b.to_ascii_lowercase();
    }
    key[12] = (track_num >> 8) as u8;
    key[13] = track_num as u8;
    key[14] = (disc_num >> 8) as u8;
    key[15] = disc_num as u8;
    key
}

// ---------------------------------------------------------------------------
// TrackMeta
// ---------------------------------------------------------------------------

/// Variable-length per-track metadata stored in `library.meta` via postcard.
///
/// Strings are bounded by `heapless::String` capacities to stay `no_std`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackMeta {
    pub soul_id: u32,
    pub album_id: u32,
    pub track_number: u16,
    pub disc_number: u16,
    pub year: u16,
    pub format: u8,
    pub channels: u8,
    pub duration_secs: u32,
    pub sample_rate: u32,
    pub title: heapless::String<128>,
    pub artist: heapless::String<64>,
    pub album: heapless::String<64>,
    pub file_path: heapless::String<256>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]
mod tests {
    use super::*;

    #[test]
    fn manifest_size_is_64_bytes() {
        assert_eq!(ManifestBin::SIZE, 64);
    }

    #[test]
    fn manifest_roundtrip() {
        let m = ManifestBin {
            track_count: 1234,
            album_count: 56,
            export_timestamp: 1_700_000_000,
            idx_checksum: 0xDEAD_BEEF,
            meta_checksum: 0xCAFE_BABE,
        };
        let bytes = m.encode();
        assert_eq!(bytes.len(), 64);
        let decoded = ManifestBin::decode(&bytes).unwrap();
        assert_eq!(decoded.track_count, 1234);
        assert_eq!(decoded.album_count, 56);
        assert_eq!(decoded.export_timestamp, 1_700_000_000);
        assert_eq!(decoded.idx_checksum, 0xDEAD_BEEF);
        assert_eq!(decoded.meta_checksum, 0xCAFE_BABE);
    }

    #[test]
    fn manifest_decode_rejects_bad_magic() {
        let mut bytes = [0u8; 64];
        bytes[0..4].copy_from_slice(b"NOPE");
        bytes[4] = ManifestBin::VERSION;
        assert!(ManifestBin::decode(&bytes).is_err());
    }

    #[test]
    fn manifest_decode_rejects_wrong_version() {
        let m = ManifestBin {
            track_count: 0,
            album_count: 0,
            export_timestamp: 0,
            idx_checksum: 0,
            meta_checksum: 0,
        };
        let mut bytes = m.encode();
        bytes[4] = 99;
        assert!(ManifestBin::decode(&bytes).is_err());
    }

    #[test]
    fn index_entry_size_is_24_bytes() {
        assert_eq!(IndexEntry::SIZE, 24);
    }

    #[test]
    fn index_entry_roundtrip() {
        let key = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 1, 0, 1];
        let e = IndexEntry {
            sort_key: key,
            meta_offset: 0x0001_0000,
            meta_size: 180,
        };
        let bytes = e.encode();
        assert_eq!(bytes.len(), 24);
        let decoded = IndexEntry::decode(&bytes).unwrap();
        assert_eq!(decoded.sort_key, key);
        assert_eq!(decoded.meta_offset, 0x0001_0000);
        assert_eq!(decoded.meta_size, 180);
    }

    #[test]
    fn sort_key_for_pads_short_strings() {
        let key = sort_key_for("AB", "CD", 1, 1);
        assert_eq!(&key[0..6], b"ab\0\0\0\0");
        assert_eq!(&key[6..12], b"cd\0\0\0\0");
    }

    #[test]
    fn sort_key_for_truncates_long_strings() {
        let key = sort_key_for("ABCDEFGHIJ", "KLMNOPQRST", 1, 1);
        assert_eq!(&key[0..6], b"abcdef");
        assert_eq!(&key[6..12], b"klmnop");
    }

    #[test]
    fn sort_key_for_track_num_big_endian() {
        let key = sort_key_for("a", "b", 0x0102, 0x0304);
        assert_eq!(key[12], 0x01);
        assert_eq!(key[13], 0x02);
        assert_eq!(key[14], 0x03);
        assert_eq!(key[15], 0x04);
    }

    #[test]
    fn sort_key_order_correct() {
        let k1 = sort_key_for("artist", "album", 1, 1);
        let k2 = sort_key_for("artist", "album", 2, 1);
        assert!(k1 < k2);
    }

    #[test]
    fn sort_key_artist_order_correct() {
        let ka = sort_key_for("aaa", "album", 1, 1);
        let kb = sort_key_for("bbb", "album", 1, 1);
        assert!(ka < kb);
    }

    #[test]
    fn track_meta_postcard_roundtrip() {
        let meta = TrackMeta {
            soul_id: 42,
            album_id: 7,
            track_number: 3,
            disc_number: 1,
            year: 2024,
            format: 0,
            channels: 2,
            duration_secs: 240,
            sample_rate: 44_100,
            title: heapless::String::try_from("Test Track").unwrap(),
            artist: heapless::String::try_from("Test Artist").unwrap(),
            album: heapless::String::try_from("Test Album").unwrap(),
            file_path: heapless::String::try_from("/soul/music/a/b/03.flac").unwrap(),
        };
        let mut buf = [0u8; 512];
        let encoded = postcard::to_slice(&meta, &mut buf).unwrap();
        let decoded: TrackMeta = postcard::from_bytes(encoded).unwrap();
        assert_eq!(decoded.soul_id, 42);
        assert_eq!(decoded.track_number, 3);
        assert_eq!(decoded.title.as_str(), "Test Track");
    }

    #[test]
    fn track_meta_serialised_size_is_reasonable() {
        let meta = TrackMeta {
            soul_id: 1,
            album_id: 1,
            track_number: 1,
            disc_number: 1,
            year: 2024,
            format: 0,
            channels: 2,
            duration_secs: 300,
            sample_rate: 44_100,
            title: heapless::String::try_from("A Title").unwrap(),
            artist: heapless::String::try_from("An Artist").unwrap(),
            album: heapless::String::try_from("An Album").unwrap(),
            file_path: heapless::String::try_from("/soul/m/artist/album/01.flac").unwrap(),
        };
        let mut buf = [0u8; 512];
        let encoded = postcard::to_slice(&meta, &mut buf).unwrap();
        assert!(
            encoded.len() < 256,
            "TrackMeta too large: {} bytes",
            encoded.len()
        );
    }
}
