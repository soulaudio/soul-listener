# Soul Library — Binary Index + Storage Abstraction Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement a persistent binary music library index that works identically on embedded (SD card via SDMMC) and desktop (local filesystem), with a scan tool for dev and Criterion benchmarks for performance validation.

**Architecture:** The library crate holds format-agnostic binary types (ManifestBin, IndexEntry, TrackMeta) and a generic reader parameterised over `platform::Storage`. The platform crate provides two concrete Storage implementations: `LocalFileStorage` (std::fs, used by emulator) and `SdmmcStorage` (Embassy SDMMC, stub for now). An xtask `scan-library` CLI tool scans a local folder and writes the binary files so development can proceed without Soul Player.

**Tech Stack:** `postcard` (no_std serialisation), `crc32fast` (CRC32), `walkdir` (xtask only), `heapless` (no_std strings), `criterion` (benchmarks), `embassy-time` (embedded async sleep in reader)

---

## Binary Format Specification

```
{soul_root}/
├── manifest.bin     — 64 bytes fixed: magic+version+counts+checksums
├── library.idx      — 24 bytes × N tracks: sort_key[16] + meta_offset[4] + meta_size[4]
├── library.meta     — postcard-encoded TrackMeta per track (variable, ~100-200 bytes each)
└── art/
    └── {ab}/        — first 2 hex chars of album_id (256 subdirs, avoids FAT32 cliff)
        └── {album_id:08x}.raw  — 2bpp grayscale 240×240 = 14 400 bytes, pre-dithered
```

**Sort key (16 bytes):** `artist_ascii_lower[6] + album_ascii_lower[6] + track_num_be[2] + disc_num_be[2]`
Big-endian integers sort correctly in a byte-level memcmp, enabling O(log N) binary search without decoding TrackMeta.

**Validation reference:** Rockbox tagcache uses the same split fixed-index + variable-metadata pattern; production-proven for millions of tracks.

---

## Task 1: Add Dependencies

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/library/Cargo.toml`
- Modify: `xtask/Cargo.toml`

**Step 1: Write the failing compile check**

```bash
cargo check -p library --features std 2>&1 | head -5
```
Expected: error about `postcard` not found (or succeeds if already present — check first).

**Step 2: Add workspace deps**

In `Cargo.toml` `[workspace.dependencies]`, add:
```toml
# Binary serialisation — no_std, postcard v1 stable wire format
postcard = { version = "1", default-features = false, features = ["heapless"] }
# CRC32 — both std writer and no_std reader paths
crc32fast = { version = "1", default-features = false }
# Directory walk — xtask scan-library only (std)
walkdir = "2"
```

Also update the heapless entry to add serde:
```toml
heapless = { version = "0.9", features = ["serde"] }
```

**Step 3: Add to `crates/library/Cargo.toml`**

```toml
[dependencies]
platform  = { path = "../platform" }
heapless  = { workspace = true }
postcard  = { workspace = true }
crc32fast = { workspace = true }
serde     = { version = "1", default-features = false, features = ["derive"] }
```

**Step 4: Add to `xtask/Cargo.toml`**

```toml
[dependencies]
# ... existing deps ...
walkdir = { workspace = true }
```

**Step 5: Verify compile**

```bash
cargo check -p library --features std
cargo check -p library
```
Expected: both succeed (no new errors).

**Step 6: Commit**

```bash
git add Cargo.toml crates/library/Cargo.toml xtask/Cargo.toml
git commit -m "chore(deps): add postcard, crc32fast, walkdir, heapless/serde for soul library"
```

---

## Task 2: Soul Root Path Constants

**Files:**
- Create: `crates/platform/src/soul_library.rs`
- Modify: `crates/platform/src/lib.rs`

**Step 1: Write failing test**

Create `crates/platform/src/soul_library.rs` with just the module declaration, then add this test at the bottom of the file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_path_is_under_soul_root() {
        assert_eq!(manifest_path(SOUL_ROOT), "/soul/manifest.bin");
    }

    #[test]
    fn library_idx_path_is_under_soul_root() {
        assert_eq!(library_idx_path(SOUL_ROOT), "/soul/library.idx");
    }

    #[test]
    fn library_meta_path_is_under_soul_root() {
        assert_eq!(library_meta_path(SOUL_ROOT), "/soul/library.meta");
    }

    #[test]
    fn art_path_uses_two_level_sharding() {
        // album_id = 0xABCD1234 → subdir "ab", file "abcd1234.raw"
        let path = art_path(SOUL_ROOT, 0xABCD_1234);
        assert_eq!(path.as_str(), "/soul/art/ab/abcd1234.raw");
    }

    #[test]
    fn art_path_for_zero_album_id() {
        let path = art_path(SOUL_ROOT, 0x0000_0000);
        assert_eq!(path.as_str(), "/soul/art/00/00000000.raw");
    }

    #[test]
    fn art_path_for_max_album_id() {
        let path = art_path(SOUL_ROOT, 0xFFFF_FFFF);
        assert_eq!(path.as_str(), "/soul/art/ff/ffffffff.raw");
    }

    #[test]
    fn custom_root_builds_correct_paths() {
        assert_eq!(manifest_path("/music"), "/music/manifest.bin");
    }
}
```

**Step 2: Run test — expect compile failure** (functions not yet defined)

```bash
cargo test -p platform 2>&1 | head -20
```

**Step 3: Implement `soul_library.rs`**

```rust
//! Soul library file layout constants and path builders.
//!
//! The DAP SD card (and local emulator root) follows this layout:
//!
//! ```text
//! {soul_root}/
//! ├── manifest.bin    — 64 B fixed header (counts, checksums)
//! ├── library.idx     — 24 B × N sorted index entries
//! ├── library.meta    — postcard-encoded TrackMeta blobs
//! └── art/
//!     └── {hi:02x}/   — first byte of album_id as hex (256 subdirs)
//!         └── {album_id:08x}.raw  — 2bpp 240×240 pre-dithered album art
//! ```
//!
//! `SOUL_ROOT` is the single source of truth for the SD card mount point.
//! Override it per deployment (SD card, emulator path) by calling the
//! path-builder functions with the actual root string.

use heapless::String;

/// Default root directory on the SD card (FAT32 volume root).
pub const SOUL_ROOT: &str = "/soul";

/// Absolute path to the manifest file.
///
/// Always `{root}/manifest.bin`.
#[must_use]
pub fn manifest_path(root: &str) -> String<64> {
    build_path(root, "/manifest.bin")
}

/// Absolute path to the index file.
///
/// Always `{root}/library.idx`.
#[must_use]
pub fn library_idx_path(root: &str) -> String<64> {
    build_path(root, "/library.idx")
}

/// Absolute path to the metadata blob file.
///
/// Always `{root}/library.meta`.
#[must_use]
pub fn library_meta_path(root: &str) -> String<64> {
    build_path(root, "/library.meta")
}

/// Absolute path to a pre-dithered album art file.
///
/// Uses two-level sharding: `{root}/art/{hi:02x}/{album_id:08x}.raw`
/// where `hi` is `(album_id >> 24) & 0xFF`.  This caps each subdir at
/// 256 entries — well under the FAT32 performance cliff (~50 000 files/dir).
#[must_use]
pub fn art_path(root: &str, album_id: u32) -> String<80> {
    let hi = (album_id >> 24) as u8;
    let mut s = String::<80>::new();
    // root + "/art/" + hi_hex + "/" + album_id_hex + ".raw"
    // Push each part; silently truncate if root is too long (should never happen in practice).
    let _ = s.push_str(root);
    let _ = s.push_str("/art/");
    push_hex2(&mut s, hi);
    let _ = s.push('/');
    push_hex8(&mut s, album_id);
    let _ = s.push_str(".raw");
    s
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

fn build_path(root: &str, suffix: &str) -> String<64> {
    let mut s = String::<64>::new();
    let _ = s.push_str(root);
    let _ = s.push_str(suffix);
    s
}

const HEX: &[u8; 16] = b"0123456789abcdef";

fn push_hex2<const N: usize>(s: &mut String<N>, byte: u8) {
    let _ = s.push(HEX[(byte >> 4) as usize] as char);
    let _ = s.push(HEX[(byte & 0xF) as usize] as char);
}

fn push_hex8<const N: usize>(s: &mut String<N>, val: u32) {
    push_hex2(s, (val >> 24) as u8);
    push_hex2(s, (val >> 16) as u8);
    push_hex2(s, (val >> 8) as u8);
    push_hex2(s, val as u8);
}
```

**Step 4: Add `pub mod soul_library;` to `crates/platform/src/lib.rs`** and re-export:
```rust
pub mod soul_library;
pub use soul_library::SOUL_ROOT;
```

**Step 5: Run tests — expect all pass**

```bash
cargo test -p platform
```
Expected: 6 new tests pass, 0 fail.

**Step 6: Commit**

```bash
git add crates/platform/src/soul_library.rs crates/platform/src/lib.rs
git commit -m "feat(platform): add soul_library path constants and art sharding helpers"
```

---

## Task 3: Binary Types — Manifest, IndexEntry, TrackMeta, sort_key_for

**Files:**
- Create: `crates/library/src/binary.rs`
- Modify: `crates/library/src/lib.rs`

**Step 1: Write failing tests**

Create `crates/library/src/binary.rs` with empty module stubs, then add these tests at the bottom:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // --- ManifestBin ---

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
        let m = ManifestBin { track_count: 0, album_count: 0, export_timestamp: 0, idx_checksum: 0, meta_checksum: 0 };
        let mut bytes = m.encode();
        bytes[4] = 99; // bad version
        assert!(ManifestBin::decode(&bytes).is_err());
    }

    // --- IndexEntry ---

    #[test]
    fn index_entry_size_is_24_bytes() {
        assert_eq!(IndexEntry::SIZE, 24);
    }

    #[test]
    fn index_entry_roundtrip() {
        let key = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 1, 0, 1];
        let e = IndexEntry { sort_key: key, meta_offset: 0x0001_0000, meta_size: 180 };
        let bytes = e.encode();
        assert_eq!(bytes.len(), 24);
        let decoded = IndexEntry::decode(&bytes);
        assert_eq!(decoded.sort_key, key);
        assert_eq!(decoded.meta_offset, 0x0001_0000);
        assert_eq!(decoded.meta_size, 180);
    }

    // --- sort_key_for ---

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
        // Track 2 of same album sorts after track 1
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

    // --- TrackMeta postcard roundtrip ---

    #[test]
    fn track_meta_postcard_roundtrip() {
        let mut meta = TrackMeta {
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
        // Should be well under 512 bytes
        assert!(encoded.len() < 256, "TrackMeta too large: {} bytes", encoded.len());
    }
}
```

**Step 2: Run to confirm failure**

```bash
cargo test -p library 2>&1 | head -20
```

**Step 3: Implement `binary.rs`**

```rust
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
    /// Number of tracks (= number of IndexEntry records in library.idx).
    pub track_count: u32,
    /// Number of unique albums (used to pre-allocate art cache).
    pub album_count: u32,
    /// Unix timestamp of last export from Soul Player (seconds since epoch).
    pub export_timestamp: u64,
    /// CRC32 of the entire `library.idx` file content.
    pub idx_checksum: u32,
    /// CRC32 of the entire `library.meta` file content.
    pub meta_checksum: u32,
}

impl ManifestBin {
    /// Byte length of the encoded manifest.
    pub const SIZE: usize = 64;
    /// Expected first 4 bytes of every valid manifest.
    pub const MAGIC: &'static [u8; 4] = b"SOUL";
    /// Current format version written by this implementation.
    pub const VERSION: u8 = 1;

    /// Encode to a 64-byte array.
    #[must_use]
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..4].copy_from_slice(Self::MAGIC);
        buf[4] = Self::VERSION;
        // [5..8] padding — already zero
        buf[8..12].copy_from_slice(&self.track_count.to_le_bytes());
        buf[12..16].copy_from_slice(&self.album_count.to_le_bytes());
        buf[16..24].copy_from_slice(&self.export_timestamp.to_le_bytes());
        buf[24..28].copy_from_slice(&self.idx_checksum.to_le_bytes());
        buf[28..32].copy_from_slice(&self.meta_checksum.to_le_bytes());
        // [32..64] padding — already zero
        buf
    }

    /// Decode from a 64-byte array.
    ///
    /// Returns `Err(LibraryError::BadMagic)` if bytes 0-3 are not `b"SOUL"`.
    /// Returns `Err(LibraryError::UnsupportedVersion)` if byte 4 is not `1`.
    pub fn decode(buf: &[u8; Self::SIZE]) -> Result<Self, LibraryError> {
        if &buf[0..4] != Self::MAGIC {
            return Err(LibraryError::BadMagic);
        }
        if buf[4] != Self::VERSION {
            return Err(LibraryError::UnsupportedVersion);
        }
        Ok(Self {
            track_count: u32::from_le_bytes(buf[8..12].try_into().unwrap_or([0u8; 4])),
            album_count: u32::from_le_bytes(buf[12..16].try_into().unwrap_or([0u8; 4])),
            export_timestamp: u64::from_le_bytes(buf[16..24].try_into().unwrap_or([0u8; 8])),
            idx_checksum: u32::from_le_bytes(buf[24..28].try_into().unwrap_or([0u8; 4])),
            meta_checksum: u32::from_le_bytes(buf[28..32].try_into().unwrap_or([0u8; 4])),
        })
    }
}

// ---------------------------------------------------------------------------
// IndexEntry — 24-byte record in library.idx
// ---------------------------------------------------------------------------

/// One 24-byte record in `library.idx`, sorted by `sort_key`.
///
/// Layout (24 bytes total):
/// ```text
/// [0..16]  sort_key     [u8; 16]  (artist[6] + album[6] + track_be[2] + disc_be[2])
/// [16..20] meta_offset  u32 le    (byte offset in library.meta)
/// [20..24] meta_size    u32 le    (byte count in library.meta)
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexEntry {
    /// 16-byte sort key: `artist_lower[6] + album_lower[6] + track_num_be[2] + disc_num_be[2]`.
    pub sort_key: [u8; 16],
    /// Byte offset of this track's `TrackMeta` blob in `library.meta`.
    pub meta_offset: u32,
    /// Byte length of this track's `TrackMeta` blob in `library.meta`.
    pub meta_size: u32,
}

impl IndexEntry {
    /// Byte length of one encoded index entry.
    pub const SIZE: usize = 24;

    /// Encode to a 24-byte array.
    #[must_use]
    pub fn encode(&self) -> [u8; Self::SIZE] {
        let mut buf = [0u8; Self::SIZE];
        buf[0..16].copy_from_slice(&self.sort_key);
        buf[16..20].copy_from_slice(&self.meta_offset.to_le_bytes());
        buf[20..24].copy_from_slice(&self.meta_size.to_le_bytes());
        buf
    }

    /// Decode from a 24-byte array (infallible — all bit patterns are valid).
    #[must_use]
    pub fn decode(buf: &[u8; Self::SIZE]) -> Self {
        let mut sort_key = [0u8; 16];
        sort_key.copy_from_slice(&buf[0..16]);
        Self {
            sort_key,
            meta_offset: u32::from_le_bytes(buf[16..20].try_into().unwrap_or([0u8; 4])),
            meta_size: u32::from_le_bytes(buf[20..24].try_into().unwrap_or([0u8; 4])),
        }
    }
}

// ---------------------------------------------------------------------------
// sort_key_for — deterministic 16-byte sort key
// ---------------------------------------------------------------------------

/// Build the 16-byte sort key used in `IndexEntry`.
///
/// - `artist[0..6]` — first 6 bytes of artist, ASCII-lowercased, zero-padded
/// - `album[6..12]`  — first 6 bytes of album title, ASCII-lowercased, zero-padded
/// - `track_num[12..14]` — big-endian u16 (sorts numerically)
/// - `disc_num[14..16]`  — big-endian u16
///
/// All keys from the same album sort consecutively; tracks within an album
/// sort by (disc_num, track_num).
///
/// Only ASCII characters are lowercased; non-ASCII bytes pass through unchanged.
/// This is intentional: Unicode comparison is locale-dependent; byte-order gives
/// a stable, deterministic sort across all targets.
#[must_use]
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
// TrackMeta — postcard-serialised per-track metadata
// ---------------------------------------------------------------------------

/// Full metadata for one track, stored as a postcard blob in `library.meta`.
///
/// `heapless::String<N>` fields must not exceed their stated capacity —
/// the Soul Player export enforces this before writing.
///
/// Wire format: postcard v1 (length-prefixed COBS).  The format is stable
/// across postcard 1.x.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TrackMeta {
    /// Unique track ID assigned by Soul Player.
    pub soul_id: u32,
    /// Unique album ID assigned by Soul Player.
    pub album_id: u32,
    /// Track number within disc (1-based).
    pub track_number: u16,
    /// Disc number (1-based; 1 for single-disc releases).
    pub disc_number: u16,
    /// Release year (0 if unknown).
    pub year: u16,
    /// Audio format: 0=FLAC, 1=MP3, 2=WAV.
    pub format: u8,
    /// Channel count (1=mono, 2=stereo).
    pub channels: u8,
    /// Playback duration in whole seconds.
    pub duration_secs: u32,
    /// Sample rate in Hz (e.g. 44 100, 48 000, 96 000, 192 000).
    pub sample_rate: u32,
    /// Display title (up to 128 UTF-8 bytes).
    pub title: heapless::String<128>,
    /// Artist name (up to 64 UTF-8 bytes).
    pub artist: heapless::String<64>,
    /// Album title (up to 64 UTF-8 bytes).
    pub album: heapless::String<64>,
    /// Full path on the FAT32 volume (up to 256 bytes).
    pub file_path: heapless::String<256>,
}
```

Add `pub mod binary;` to `crates/library/src/lib.rs` and re-export:
```rust
pub mod binary;
pub use binary::{IndexEntry, LibraryError, ManifestBin, TrackMeta, sort_key_for};
```

**Step 4: Run tests — expect all pass**

```bash
cargo test -p library
```
Expected: all new tests pass.

**Step 5: Commit**

```bash
git add crates/library/src/binary.rs crates/library/src/lib.rs
git commit -m "feat(library): binary types — ManifestBin, IndexEntry, TrackMeta, sort_key_for"
```

---

## Task 4: LibraryWriter (std feature)

**Files:**
- Create: `crates/library/src/writer.rs`
- Modify: `crates/library/Cargo.toml` (crc32fast dep already present from Task 1)
- Modify: `crates/library/src/lib.rs`

LibraryWriter is only compiled with the `std` feature (used by xtask, not on embedded).

**Step 1: Write failing tests**

```rust
// In crates/library/src/writer.rs (at the bottom):
#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::{sort_key_for, TrackMeta};
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
            title: heapless::String::try_from(alloc::format!("Track {}", n).as_str()).unwrap(),
            artist: heapless::String::try_from("Test Artist").unwrap(),
            album: heapless::String::try_from("Test Album").unwrap(),
            file_path: heapless::String::try_from(
                alloc::format!("/soul/music/ta/ta/{:02}.flac", n).as_str()
            ).unwrap(),
        }
    }

    #[test]
    fn writer_creates_three_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        let mut w = LibraryWriter::new(root).unwrap();
        let key = sort_key_for("Test Artist", "Test Album", 1, 1);
        w.add_track(key, sample_meta(1)).unwrap();
        w.finish(42, 1_700_000_000).unwrap();

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
        let manifest = ManifestBin::decode(&bytes.as_slice().try_into().unwrap()).unwrap();
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
        let manifest = ManifestBin::decode(&manifest_bytes.as_slice().try_into().unwrap()).unwrap();

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
```

Add `tempfile = "3"` to `[dev-dependencies]` in `crates/library/Cargo.toml` (std-only test dep):
```toml
[dev-dependencies]
tempfile = "3"
```

**Step 2: Confirm failure**

```bash
cargo test -p library --features std 2>&1 | head -20
```

**Step 3: Implement `writer.rs`**

```rust
//! LibraryWriter — build and write the Soul binary library files.
//!
//! Only compiled with the `std` feature (used by the `scan-library` xtask).

#[cfg(not(feature = "std"))]
compile_error!("writer.rs requires the `std` feature");

extern crate alloc;

use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;

use crc32fast::Hasher;

use crate::binary::{IndexEntry, ManifestBin, TrackMeta};

/// Error type for LibraryWriter operations.
#[derive(Debug)]
pub enum WriterError {
    /// An I/O error from std::io
    Io(std::io::Error),
    /// postcard serialisation failed
    Postcard(postcard::Error),
}

impl From<std::io::Error> for WriterError {
    fn from(e: std::io::Error) -> Self { Self::Io(e) }
}
impl From<postcard::Error> for WriterError {
    fn from(e: postcard::Error) -> Self { Self::Postcard(e) }
}

/// Builds `manifest.bin`, `library.idx`, and `library.meta` under `soul_root`.
///
/// Tracks must be added in sorted order (by sort_key).  The caller is
/// responsible for sorting before calling `add_track`.
///
/// # Example
/// ```no_run
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// use library::writer::LibraryWriter;
/// use library::binary::sort_key_for;
/// let mut w = LibraryWriter::new("/tmp/soul")?;
/// // ... add tracks ...
/// w.finish(/*album_count=*/1, /*timestamp=*/0)?;
/// # Ok(()) }
/// ```
pub struct LibraryWriter {
    root: PathBuf,
    idx_buf: Vec<u8>,
    meta_buf: Vec<u8>,
}

impl LibraryWriter {
    /// Create a new writer targeting `soul_root`.
    ///
    /// Creates the root directory if it does not exist.
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
    /// Tracks must be added in ascending sort_key order.
    pub fn add_track(&mut self, sort_key: [u8; 16], meta: TrackMeta) -> Result<(), WriterError> {
        let meta_offset = self.meta_buf.len() as u32;
        let mut postcard_buf = [0u8; 512];
        let encoded = postcard::to_slice(&meta, &mut postcard_buf)?;
        let meta_size = encoded.len() as u32;

        let entry = IndexEntry { sort_key, meta_offset, meta_size };
        self.idx_buf.extend_from_slice(&entry.encode());
        self.meta_buf.extend_from_slice(encoded);

        Ok(())
    }

    /// Write all files and return.
    ///
    /// `album_count` is the number of unique album IDs seen during scanning.
    /// `export_timestamp` is a Unix timestamp (seconds since epoch); pass 0 for dev builds.
    pub fn finish(self, album_count: u32, export_timestamp: u64) -> Result<(), WriterError> {
        let track_count = (self.idx_buf.len() / IndexEntry::SIZE) as u32;

        let mut idx_hasher = Hasher::new();
        idx_hasher.update(&self.idx_buf);
        let idx_checksum = idx_hasher.finalize();

        let mut meta_hasher = Hasher::new();
        meta_hasher.update(&self.meta_buf);
        let meta_checksum = meta_hasher.finalize();

        // Write library.idx
        fs::write(self.root.join("library.idx"), &self.idx_buf)?;

        // Write library.meta
        fs::write(self.root.join("library.meta"), &self.meta_buf)?;

        // Write manifest.bin last (atomic-ish: only written when idx+meta are complete)
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
```

Add to `crates/library/src/lib.rs`:
```rust
#[cfg(feature = "std")]
pub mod writer;
```

**Step 4: Run tests — expect all pass**

```bash
cargo test -p library --features std
```

**Step 5: Commit**

```bash
git add crates/library/src/writer.rs crates/library/src/lib.rs crates/library/Cargo.toml
git commit -m "feat(library): add LibraryWriter to build manifest+idx+meta files (std feature)"
```

---

## Task 5: LocalFileStorage

**Files:**
- Create: `crates/platform/src/storage_local.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/platform/Cargo.toml`

**Step 1: Write failing tests**

```rust
// In crates/platform/src/storage_local.rs (tests section):
#[cfg(test)]
mod tests {
    use super::*;
    use platform::Storage;
    use tempfile::TempDir;

    async fn setup_file(dir: &TempDir, name: &str, content: &[u8]) {
        let path = dir.path().join(name);
        std::fs::write(path, content).unwrap();
    }

    #[tokio::test]
    async fn local_storage_read_full_file() {
        let tmp = TempDir::new().unwrap();
        setup_file(&tmp, "test.bin", b"hello world").await;
        let root = tmp.path().to_str().unwrap();
        let mut storage = LocalFileStorage::new(root);
        let mut file = storage.open_file("test.bin").await.unwrap();
        let mut buf = [0u8; 11];
        let n = file.read(&mut buf).await.unwrap();
        assert_eq!(n, 11);
        assert_eq!(&buf, b"hello world");
    }

    #[tokio::test]
    async fn local_storage_size_matches() {
        let tmp = TempDir::new().unwrap();
        setup_file(&tmp, "size.bin", &[0u8; 64]).await;
        let root = tmp.path().to_str().unwrap();
        let mut storage = LocalFileStorage::new(root);
        let file = storage.open_file("size.bin").await.unwrap();
        assert_eq!(file.size(), 64);
    }

    #[tokio::test]
    async fn local_storage_seek_and_read() {
        let tmp = TempDir::new().unwrap();
        setup_file(&tmp, "seek.bin", b"ABCDEFGH").await;
        let root = tmp.path().to_str().unwrap();
        let mut storage = LocalFileStorage::new(root);
        let mut file = storage.open_file("seek.bin").await.unwrap();
        file.seek(4).await.unwrap();
        let mut buf = [0u8; 4];
        file.read(&mut buf).await.unwrap();
        assert_eq!(&buf, b"EFGH");
    }

    #[tokio::test]
    async fn local_storage_exists_true() {
        let tmp = TempDir::new().unwrap();
        setup_file(&tmp, "exists.bin", b"x").await;
        let root = tmp.path().to_str().unwrap();
        let mut storage = LocalFileStorage::new(root);
        assert!(storage.exists("exists.bin").await.unwrap());
    }

    #[tokio::test]
    async fn local_storage_exists_false() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        let mut storage = LocalFileStorage::new(root);
        assert!(!storage.exists("missing.bin").await.unwrap());
    }
}
```

Add `tokio = { workspace = true }` to `[dev-dependencies]` in `crates/platform/Cargo.toml`.

**Step 2: Run to confirm failure**

```bash
cargo test -p platform --features std 2>&1 | head -20
```

**Step 3: Implement `storage_local.rs`**

```rust
//! Local filesystem Storage implementation for the desktop emulator.
//!
//! `LocalFileStorage` implements `platform::Storage` using `std::fs`.
//! It is used when the `std` feature is enabled (emulator builds).
//! The `soul_root` is the directory under which all relative paths are resolved.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use crate::storage::{File, Storage};

// ---------------------------------------------------------------------------
// Error
// ---------------------------------------------------------------------------

/// Error type for local filesystem operations.
#[derive(Debug)]
pub struct LocalStorageError(pub std::io::Error);

impl core::fmt::Display for LocalStorageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "LocalStorageError: {}", self.0)
    }
}

// ---------------------------------------------------------------------------
// LocalFile
// ---------------------------------------------------------------------------

/// An open file on the local filesystem.
pub struct LocalFile {
    inner: fs::File,
    size: u64,
}

impl File for LocalFile {
    type Error = LocalStorageError;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        self.inner.read(buf).map_err(LocalStorageError)
    }

    async fn seek(&mut self, pos: u64) -> Result<u64, Self::Error> {
        self.inner.seek(SeekFrom::Start(pos)).map_err(LocalStorageError)
    }

    fn size(&self) -> u64 {
        self.size
    }
}

// ---------------------------------------------------------------------------
// LocalFileStorage
// ---------------------------------------------------------------------------

/// A `platform::Storage` implementation backed by `std::fs`.
///
/// Paths are resolved relative to `soul_root`.
///
/// ```no_run
/// # async fn example() {
/// use platform::storage_local::LocalFileStorage;
/// use platform::Storage;
/// let mut storage = LocalFileStorage::new("/home/user/soul");
/// let file = storage.open_file("manifest.bin").await.unwrap();
/// # }
/// ```
pub struct LocalFileStorage {
    root: PathBuf,
}

impl LocalFileStorage {
    /// Create a new storage rooted at `soul_root`.
    pub fn new(soul_root: &str) -> Self {
        Self { root: PathBuf::from(soul_root) }
    }

    /// Create from the `MUSIC_PATH` environment variable.
    ///
    /// Returns `None` if `MUSIC_PATH` is not set.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        std::env::var("MUSIC_PATH").ok().map(|p| Self::new(&p))
    }

    fn resolve(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
}

impl Storage for LocalFileStorage {
    type Error = LocalStorageError;
    type File = LocalFile;

    async fn open_file(&mut self, path: &str) -> Result<Self::File, Self::Error> {
        let full = self.resolve(path);
        let file = fs::File::open(&full).map_err(LocalStorageError)?;
        let meta = file.metadata().map_err(LocalStorageError)?;
        Ok(LocalFile { inner: file, size: meta.len() })
    }

    async fn exists(&mut self, path: &str) -> Result<bool, Self::Error> {
        Ok(self.resolve(path).exists())
    }
}
```

Add to `crates/platform/src/lib.rs` (inside `#[cfg(feature = "std")]` guard):
```rust
#[cfg(feature = "std")]
pub mod storage_local;
```

Add `std` feature to `platform/Cargo.toml` if not already listed:
```toml
[features]
std = []
```

**Step 4: Run tests — expect all pass**

```bash
cargo test -p platform --features std
```

**Step 5: Commit**

```bash
git add crates/platform/src/storage_local.rs crates/platform/src/lib.rs crates/platform/Cargo.toml
git commit -m "feat(platform): add LocalFileStorage — std::fs impl of Storage trait for emulator"
```

---

## Task 6: SoulLibraryReader

**Files:**
- Create: `crates/library/src/reader.rs`
- Modify: `crates/library/src/lib.rs`

The reader is `no_std` and works with any type implementing `platform::Storage`.

**Step 1: Write failing tests**

These tests use `LocalFileStorage` + a real (tiny) library written by `LibraryWriter`:

```rust
// In crates/library/src/reader.rs (tests section, cfg(test) requires std+platform/std):
#[cfg(test)]
mod tests {
    use super::*;
    use crate::binary::{sort_key_for, TrackMeta};
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
            title: heapless::String::try_from(
                alloc::format!("Track {:02}", n).as_str()
            ).unwrap(),
            artist: heapless::String::try_from(artist).unwrap(),
            album: heapless::String::try_from(album).unwrap(),
            file_path: heapless::String::try_from(
                alloc::format!("/soul/music/{}/{}/{:02}.flac", artist, album, n).as_str()
            ).unwrap(),
        }
    }

    fn build_library(root: &str, tracks: &[(u32, &str, &str)]) {
        // Sort before writing
        let mut entries: Vec<_> = tracks.iter().map(|(n, ar, al)| {
            let key = sort_key_for(ar, al, *n as u16, 1);
            (key, make_meta(*n, ar, al))
        }).collect();
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
    async fn reader_page_returns_correct_tracks() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        build_library(root, &[
            (1, "Artist", "Album"),
            (2, "Artist", "Album"),
            (3, "Artist", "Album"),
        ]);

        let storage = LocalFileStorage::new(root);
        let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
        let page = reader.page(0, 2).await.unwrap();
        assert_eq!(page.len(), 2);
        assert_eq!(page[0].track_number, 1);
        assert_eq!(page[1].track_number, 2);
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
    async fn reader_search_by_artist_finds_tracks() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        build_library(root, &[
            (1, "Amon Tobin", "Foley Room"),
            (2, "Amon Tobin", "Foley Room"),
            (1, "Portishead", "Dummy"),
        ]);

        let storage = LocalFileStorage::new(root);
        let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
        let results = reader.search_by_artist("Amon To").await.unwrap();
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].artist.as_str(), "Amon Tobin");
    }

    #[tokio::test]
    async fn reader_manifest_missing_returns_err() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().to_str().unwrap();
        let storage = LocalFileStorage::new(root);
        let result = SoulLibraryReader::open(storage, root).await;
        assert!(result.is_err());
    }
}
```

**Step 2: Run to confirm failure**

```bash
cargo test -p library --features std 2>&1 | head -20
```

**Step 3: Implement `reader.rs`**

```rust
//! SoulLibraryReader — no_std generic reader for the Soul binary library.
//!
//! Parameterised over any `platform::Storage` implementation.
//! Uses O(1) seeks into `library.idx` (fixed 24-byte records) and
//! O(log N) binary search for artist/album lookup.

extern crate alloc;

use alloc::vec::Vec;

use platform::storage::{File, Storage};
use platform::soul_library::{library_idx_path, library_meta_path, manifest_path};

use crate::binary::{IndexEntry, LibraryError, ManifestBin, TrackMeta};

/// Error type returned by `SoulLibraryReader` operations.
#[derive(Debug)]
pub enum ReaderError<E: core::fmt::Debug> {
    /// An I/O error from the underlying Storage implementation
    Storage(E),
    /// Binary format error (bad magic, bad version, corrupt data)
    Format(LibraryError),
    /// Track index out of range
    OutOfRange,
}

impl<E: core::fmt::Debug> From<LibraryError> for ReaderError<E> {
    fn from(e: LibraryError) -> Self { Self::Format(e) }
}

/// Generic reader for the Soul binary library.
///
/// Opens `manifest.bin`, `library.idx`, and `library.meta` from the given
/// storage root and provides page-based and random-access track retrieval.
pub struct SoulLibraryReader<S: Storage> {
    storage: S,
    root: heapless::String<64>,
    manifest: ManifestBin,
}

impl<S: Storage> SoulLibraryReader<S> {
    /// Open the library at `soul_root`.
    ///
    /// Reads and validates `manifest.bin`.  Does not pre-load the full index
    /// into memory — all subsequent reads are on-demand.
    pub async fn open(mut storage: S, soul_root: &str) -> Result<Self, ReaderError<S::Error>> {
        let manifest_p = manifest_path(soul_root);
        let mut file = storage
            .open_file(manifest_p.as_str())
            .await
            .map_err(ReaderError::Storage)?;

        let mut buf = [0u8; ManifestBin::SIZE];
        read_exact(&mut file, &mut buf).await.map_err(ReaderError::Storage)?;
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

    /// Load a page of `TrackMeta` starting at `offset` (0-based track index).
    ///
    /// Returns up to `count` tracks.  If `offset >= track_count()`, returns empty vec.
    pub async fn page(
        &mut self,
        offset: u32,
        count: u32,
    ) -> Result<Vec<TrackMeta>, ReaderError<S::Error>> {
        let total = self.manifest.track_count;
        if offset >= total {
            return Ok(Vec::new());
        }
        let available = (total.saturating_sub(offset)).min(count);
        let mut result = Vec::with_capacity(available as usize);

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

        for i in 0..available {
            let track_idx = offset.saturating_add(i);
            let entry = self.read_index_entry(&mut idx_file, track_idx).await?;
            let meta = self.read_track_meta(&mut meta_file, &entry).await?;
            result.push(meta);
        }

        Ok(result)
    }

    /// Load a single `TrackMeta` by track index (0-based).
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

        let entry = self.read_index_entry(&mut idx_file, index).await?;
        self.read_track_meta(&mut meta_file, &entry).await
    }

    /// Search for tracks whose sort_key starts with the artist prefix.
    ///
    /// The prefix is lowercased and limited to 6 bytes (the artist portion of sort_key).
    /// Uses binary search to find the first matching entry, then scans forward.
    ///
    /// Returns up to 64 matching `TrackMeta` records.
    pub async fn search_by_artist(
        &mut self,
        artist_prefix: &str,
    ) -> Result<Vec<TrackMeta>, ReaderError<S::Error>> {
        let mut prefix = [0u8; 6];
        for (i, &b) in artist_prefix.as_bytes().iter().take(6).enumerate() {
            prefix[i] = b.to_ascii_lowercase();
        }
        let prefix_len = artist_prefix.len().min(6);

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

        // Binary search: find first entry where sort_key[0..prefix_len] >= prefix[0..prefix_len]
        let total = self.manifest.track_count;
        let mut lo = 0u32;
        let mut hi = total;
        while lo < hi {
            let mid = lo.saturating_add((hi.saturating_sub(lo)) / 2);
            let entry = self.read_index_entry(&mut idx_file, mid).await?;
            if entry.sort_key[..prefix_len] < prefix[..prefix_len] {
                lo = mid.saturating_add(1);
            } else {
                hi = mid;
            }
        }

        // Collect up to 64 matching tracks
        let mut results = Vec::new();
        let mut i = lo;
        while i < total && results.len() < 64 {
            let entry = self.read_index_entry(&mut idx_file, i).await?;
            if entry.sort_key[..prefix_len] != prefix[..prefix_len] {
                break;
            }
            let meta = self.read_track_meta(&mut meta_file, &entry).await?;
            results.push(meta);
            i = i.saturating_add(1);
        }
        Ok(results)
    }

    // ---
    // Private helpers
    // ---

    async fn read_index_entry(
        &self,
        file: &mut S::File,
        index: u32,
    ) -> Result<IndexEntry, ReaderError<S::Error>> {
        let offset = u64::from(index).saturating_mul(IndexEntry::SIZE as u64);
        file.seek(offset).await.map_err(ReaderError::Storage)?;
        let mut buf = [0u8; IndexEntry::SIZE];
        read_exact(file, &mut buf).await.map_err(ReaderError::Storage)?;
        Ok(IndexEntry::decode(&buf))
    }

    async fn read_track_meta(
        &self,
        file: &mut S::File,
        entry: &IndexEntry,
    ) -> Result<TrackMeta, ReaderError<S::Error>> {
        file.seek(u64::from(entry.meta_offset))
            .await
            .map_err(ReaderError::Storage)?;
        let size = entry.meta_size as usize;
        let mut buf = alloc::vec![0u8; size];
        read_exact(file, &mut buf).await.map_err(ReaderError::Storage)?;
        postcard::from_bytes(&buf).map_err(|_| ReaderError::Format(LibraryError::DecodeError))
    }
}

/// Read exactly `buf.len()` bytes from `file`, retrying on short reads.
async fn read_exact<F: File>(file: &mut F, buf: &mut [u8]) -> Result<(), F::Error> {
    let mut pos = 0;
    while pos < buf.len() {
        let n = file.read(&mut buf[pos..]).await?;
        if n == 0 {
            break; // EOF — caller must handle short buffer
        }
        pos = pos.saturating_add(n);
    }
    Ok(())
}
```

Add to `crates/library/src/lib.rs`:
```rust
pub mod reader;
pub use reader::SoulLibraryReader;
```

**Step 4: Run tests**

```bash
cargo test -p library --features std
```
Expected: all pass.

**Step 5: Compile check no_std path**

```bash
cargo check -p library
```
Expected: succeeds (no std-only code in reader.rs without cfg guard — confirm `alloc` is available).

> Note: the library crate may need `extern crate alloc;` at the top of `lib.rs` if it's truly `no_std`. Check if `Vec` works; if not, switch `page()` return type to `heapless::Vec<TrackMeta, 64>` and remove the `alloc` usage.

**Step 6: Commit**

```bash
git add crates/library/src/reader.rs crates/library/src/lib.rs
git commit -m "feat(library): add SoulLibraryReader — generic no_std binary reader with binary search"
```

---

## Task 7: SdmmcStorage Stub

**Files:**
- Create: `crates/platform/src/storage_sdmmc.rs`
- Modify: `crates/platform/src/lib.rs`

This is a stub that compiles for the `hardware` feature. Full SDMMC implementation is tracked separately (blocked on Embassy SDMMC init).

**Step 1: Write compile-only test**

```rust
// In storage_sdmmc.rs tests — no async needed, just check types compile
#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::{File, Storage};

    // Compile-time check: SdmmcStorage implements Storage
    fn _assert_implements_storage() {
        fn _check<S: Storage>() {}
        // _check::<SdmmcStorage>(); // Uncomment when fully implemented
    }

    #[test]
    fn sdmmc_error_is_debug() {
        let e = SdmmcError::NotImplemented;
        assert_eq!(format!("{:?}", e), "NotImplemented");
    }
}
```

**Step 2: Implement `storage_sdmmc.rs`**

```rust
//! SDMMC-backed Storage stub for the hardware target.
//!
//! This is a placeholder that compiles but always returns `NotImplemented`.
//! The full implementation requires Embassy SDMMC init (blocked on PLL1Q config).
//!
//! # TODO
//! Replace the stub body in `open_file` and `exists` with Embassy SDMMC calls
//! once `firmware::boot::build_embassy_config()` configures the SDMMC clock.

use crate::storage::{File, Storage};

/// Error type for SDMMC storage operations.
#[derive(Debug)]
pub enum SdmmcError {
    /// This stub operation is not yet implemented.
    NotImplemented,
    /// Underlying SDMMC I/O error (future use).
    Io,
}

/// Placeholder file for SDMMC (stub — always EOF).
pub struct SdmmcFile;

impl File for SdmmcFile {
    type Error = SdmmcError;

    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        Err(SdmmcError::NotImplemented)
    }

    async fn seek(&mut self, _pos: u64) -> Result<u64, Self::Error> {
        Err(SdmmcError::NotImplemented)
    }

    fn size(&self) -> u64 {
        0
    }
}

/// SDMMC-backed Storage — stub implementation.
///
/// Construct with `SdmmcStorage::new(sdmmc_peripheral)` once Embassy SDMMC
/// is wired.  For now, all operations return `SdmmcError::NotImplemented`.
pub struct SdmmcStorage;

impl SdmmcStorage {
    /// Create a new (stub) SDMMC storage instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for SdmmcStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for SdmmcStorage {
    type Error = SdmmcError;
    type File = SdmmcFile;

    async fn open_file(&mut self, _path: &str) -> Result<Self::File, Self::Error> {
        Err(SdmmcError::NotImplemented)
    }

    async fn exists(&mut self, _path: &str) -> Result<bool, Self::Error> {
        Err(SdmmcError::NotImplemented)
    }
}
```

Add to `crates/platform/src/lib.rs`:
```rust
pub mod storage_sdmmc;
```

**Step 3: Verify compile (no hardware target needed)**

```bash
cargo check -p platform
```
Expected: succeeds.

**Step 4: Commit**

```bash
git add crates/platform/src/storage_sdmmc.rs crates/platform/src/lib.rs
git commit -m "feat(platform): add SdmmcStorage stub — hardware Storage impl (full impl blocked on SDMMC init)"
```

---

## Task 8: xtask scan-library

**Files:**
- Create: `xtask/src/scan_library.rs`
- Modify: `xtask/src/main.rs`

The `scan-library` command scans a local directory tree, infers metadata from folder structure (`{Artist}/{Album}/{tracknum} - {title}.ext`), and writes the Soul binary files.

**Step 1: Write tests**

```rust
// In xtask/src/scan_library.rs:
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_fake_library(dir: &TempDir) {
        // Artist/Album/NN - Title.flac structure
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
        // File without leading number — track_number defaults to 0
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
```

**Step 2: Implement `scan_library.rs`**

```rust
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
pub fn run_scan(music_dir: &Path, soul_root: &Path) -> Result<()> {
    let entries = scan_audio_files(music_dir)?;
    println!("Found {} audio files", entries.len());

    // Infer metadata and assign sequential IDs
    let mut metas: Vec<([u8; 16], TrackMeta)> = entries
        .iter()
        .enumerate()
        .filter_map(|(i, path)| {
            let meta = infer_meta_from_path(path, (i as u32).saturating_add(1), 1).ok()?;
            let key = sort_key_for(
                meta.artist.as_str(),
                meta.album.as_str(),
                meta.track_number,
                meta.disc_number,
            );
            Some((key, meta))
        })
        .collect();

    // Sort by key before writing
    metas.sort_by_key(|(k, _)| *k);

    let root_str = soul_root.to_str().ok_or_else(|| anyhow::anyhow!("invalid soul_root path"))?;
    let mut writer = LibraryWriter::new(root_str)?;
    let mut album_ids = std::collections::HashSet::new();
    for (key, meta) in metas {
        album_ids.insert(meta.album_id);
        writer.add_track(key, meta)?;
    }
    writer.finish(album_ids.len() as u32, unix_now())?;

    println!("Written to: {}", soul_root.display());
    Ok(())
}

/// Recursively collect all audio file paths under `dir`.
pub fn scan_audio_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for entry in WalkDir::new(dir).sort_by_file_name() {
        let entry = entry?;
        if entry.file_type().is_file() {
            let ext = entry.path().extension()
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
pub fn infer_meta_from_path(path: &Path, soul_id: u32, album_id: u32) -> Result<TrackMeta> {
    let components: Vec<&str> = path
        .iter()
        .filter_map(|c| c.to_str())
        .collect();

    let n = components.len();
    let artist = if n >= 3 { components[n.saturating_sub(3)] } else { "" };
    let album = if n >= 2 { components[n.saturating_sub(2)] } else { "" };
    let filename = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let (track_number, title) = parse_filename(filename);

    let format = match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
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
        file_path: heapless::String::try_from(
            path.to_str().unwrap_or("")
        ).unwrap_or_default(),
    })
}

/// Parse `"02 - Track Title"` → `(2, "Track Title")`.
/// Returns `(0, filename)` if no leading number found.
fn parse_filename(filename: &str) -> (u16, &str) {
    // Match "NN - Title" or "NN. Title" pattern
    let mut chars = filename.char_indices().peekable();
    let mut num_end = 0;
    let mut has_num = false;
    while let Some((i, c)) = chars.peek() {
        if c.is_ascii_digit() {
            num_end = *i + 1;
            has_num = true;
            chars.next();
        } else {
            break;
        }
    }
    if !has_num {
        return (0, filename);
    }
    let num: u16 = filename[..num_end].parse().unwrap_or(0);
    let rest = filename[num_end..].trim_start_matches(|c: char| c == ' ' || c == '-' || c == '.');
    (num, rest)
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}
```

Add to `xtask/src/main.rs`:

```rust
mod scan_library;

// In Commands enum:
/// Scan a local music folder and write Soul binary library files
ScanLibrary {
    /// Directory containing music files (Artist/Album/track structure)
    #[arg(long)]
    music_dir: std::path::PathBuf,
    /// Output directory for binary library files (manifest.bin, library.idx, library.meta)
    #[arg(long)]
    soul_root: std::path::PathBuf,
},

// In match cli.command:
Commands::ScanLibrary { music_dir, soul_root } => scan_library::run(&music_dir, &soul_root),
```

Add `library = { path = "../crates/library", features = ["std"] }` to `xtask/Cargo.toml`.

**Step 3: Run tests**

```bash
cargo test -p xtask 2>&1
```

**Step 4: Verify CLI works**

```bash
cargo xtask scan-library --help
```
Expected: shows `--music-dir` and `--soul-root` args.

**Step 5: Commit**

```bash
git add xtask/src/scan_library.rs xtask/src/main.rs xtask/Cargo.toml
git commit -m "feat(xtask): add scan-library command — scan local music folder, write Soul binary files"
```

---

## Task 9: cargo dev `--music-path` Flag

**Files:**
- Modify: `xtask/src/main.rs` (Dev command)
- Modify: `xtask/src/dev.rs`

**Step 1: Write test**

```rust
// In xtask/src/dev.rs tests:
#[test]
fn music_path_flag_accepted() {
    // Just confirm the struct compiles with the new field — smoke test
    let args = DevArgs {
        headless: false,
        hot_reload: false,
        music_path: Some("/tmp/music".into()),
    };
    assert_eq!(args.music_path.unwrap().to_str().unwrap(), "/tmp/music");
}
```

**Step 2: Add `--music-path` to `Dev` command**

In `xtask/src/main.rs`, update the `Dev` variant:
```rust
Dev {
    #[arg(long)]
    headless: bool,
    #[arg(long)]
    hot_reload: bool,
    /// Local music directory — passed as MUSIC_PATH env var to the emulator.
    /// The emulator's LocalFileStorage reads this to locate library files.
    #[arg(long)]
    music_path: Option<std::path::PathBuf>,
},
```

Update `match` arm:
```rust
Commands::Dev { headless, hot_reload, music_path } => dev::run(headless, hot_reload, music_path.as_deref()),
```

**Step 3: Update `dev::run` and `start_emulator`**

Change signatures:
```rust
pub fn run(headless: bool, hot_reload: bool, music_path: Option<&std::path::Path>) -> Result<()> { ... }
fn start_emulator(headless: bool, hot_reload: bool, music_path: Option<&std::path::Path>) -> Result<Option<Child>> { ... }
```

In `start_emulator`, before spawning the child:
```rust
if let Some(mp) = music_path {
    cmd.env("MUSIC_PATH", mp);
} else if let Ok(mp) = std::env::var("MUSIC_PATH") {
    // Forward MUSIC_PATH from parent environment if already set
    cmd.env("MUSIC_PATH", mp);
}
```

**Step 4: Verify**

```bash
cargo xtask dev --help
# Should show: --music-path <MUSIC_PATH>

cargo xtask scan-library --music-dir ~/Music --soul-root /tmp/soul
cargo xtask dev --music-path /tmp/soul
```

**Step 5: Commit**

```bash
git add xtask/src/main.rs xtask/src/dev.rs
git commit -m "feat(xtask): add --music-path flag to cargo dev; forwards MUSIC_PATH env var to emulator"
```

---

## Task 10: End-to-End Integration Tests (No Mocks)

**Files:**
- Create: `crates/library/tests/e2e_soul_library.rs`
- Create: `crates/library/tests/fixtures/` (tiny test library)

These tests exercise the full pipeline: `LibraryWriter` → real files on disk → `SoulLibraryReader<LocalFileStorage>` → verify correct results. Zero mocks.

**Step 1: Create fixture data helper**

Create `crates/library/tests/e2e_soul_library.rs`:

```rust
//! End-to-end tests: LibraryWriter → disk → SoulLibraryReader<LocalFileStorage>.
//!
//! No mocks. Uses tempfiles. Tests the complete pipeline as it runs on real hardware
//! (with LocalFileStorage substituting for SdmmcStorage).

use library::binary::{sort_key_for, TrackMeta};
use library::reader::SoulLibraryReader;
use library::writer::LibraryWriter;
use platform::storage_local::LocalFileStorage;
use tempfile::TempDir;

fn build_meta(
    soul_id: u32,
    album_id: u32,
    track_number: u16,
    disc_number: u16,
    artist: &str,
    album: &str,
    title: &str,
) -> TrackMeta {
    TrackMeta {
        soul_id,
        album_id,
        track_number,
        disc_number,
        year: 2024,
        format: 0,
        channels: 2,
        duration_secs: 240,
        sample_rate: 44_100,
        title: heapless::String::try_from(title).expect("title fits"),
        artist: heapless::String::try_from(artist).expect("artist fits"),
        album: heapless::String::try_from(album).expect("album fits"),
        file_path: heapless::String::try_from(
            format!("/soul/music/{}/{}/{:02}.flac", artist, album, track_number).as_str()
        ).expect("path fits"),
    }
}

/// Build a library with the given track list, sorted automatically.
fn build_library(root: &str, tracks: &[(u32, u32, u16, u16, &str, &str, &str)]) {
    let mut entries: Vec<_> = tracks
        .iter()
        .map(|&(sid, aid, tn, dn, ar, al, ti)| {
            let key = sort_key_for(ar, al, tn, dn);
            (key, build_meta(sid, aid, tn, dn, ar, al, ti))
        })
        .collect();
    entries.sort_by_key(|(k, _)| *k);

    let mut w = LibraryWriter::new(root).expect("writer");
    for (key, meta) in entries {
        w.add_track(key, meta).expect("add_track");
    }
    w.finish(1, 0).expect("finish");
}

// (soul_id, album_id, track_num, disc_num, artist, album, title)
fn five_track_library() -> Vec<(u32, u32, u16, u16, &'static str, &'static str, &'static str)> {
    vec![
        (1, 1, 1, 1, "Amon Tobin", "Foley Room", "Foley Room Remix"),
        (2, 1, 2, 1, "Amon Tobin", "Foley Room", "Kitchen Sink"),
        (3, 1, 3, 1, "Amon Tobin", "Foley Room", "Surge"),
        (4, 2, 1, 1, "Portishead", "Dummy", "Mysterons"),
        (5, 2, 2, 1, "Portishead", "Dummy", "Sour Times"),
    ]
}

// ---------------------------------------------------------------------------
// E2E Tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn e2e_track_count_matches_written() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_str().unwrap();
    build_library(root, &five_track_library());

    let storage = LocalFileStorage::new(root);
    let reader = SoulLibraryReader::open(storage, root).await.unwrap();
    assert_eq!(reader.track_count(), 5);
}

#[tokio::test]
async fn e2e_first_page_sorted_by_artist_album_track() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_str().unwrap();
    build_library(root, &five_track_library());

    let storage = LocalFileStorage::new(root);
    let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
    let page = reader.page(0, 5).await.unwrap();

    // "Amon Tobin" sorts before "Portishead"
    assert_eq!(page[0].artist.as_str(), "Amon Tobin");
    assert_eq!(page[0].track_number, 1);
    assert_eq!(page[1].track_number, 2);
    assert_eq!(page[2].track_number, 3);
    assert_eq!(page[3].artist.as_str(), "Portishead");
    assert_eq!(page[4].track_number, 2);
}

#[tokio::test]
async fn e2e_page_with_offset_skips_correctly() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_str().unwrap();
    build_library(root, &five_track_library());

    let storage = LocalFileStorage::new(root);
    let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
    let page = reader.page(3, 2).await.unwrap();
    assert_eq!(page.len(), 2);
    assert_eq!(page[0].artist.as_str(), "Portishead");
}

#[tokio::test]
async fn e2e_track_by_index_exact() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_str().unwrap();
    build_library(root, &five_track_library());

    let storage = LocalFileStorage::new(root);
    let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
    // Index 3 = first Portishead track (Mysterons)
    let meta = reader.track(3).await.unwrap();
    assert_eq!(meta.title.as_str(), "Mysterons");
}

#[tokio::test]
async fn e2e_track_out_of_range_returns_err() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_str().unwrap();
    build_library(root, &five_track_library());

    let storage = LocalFileStorage::new(root);
    let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
    assert!(reader.track(999).await.is_err());
}

#[tokio::test]
async fn e2e_search_by_artist_prefix_finds_subset() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_str().unwrap();
    build_library(root, &five_track_library());

    let storage = LocalFileStorage::new(root);
    let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
    let results = reader.search_by_artist("Amon").await.unwrap();
    assert_eq!(results.len(), 3);
    for r in &results {
        assert_eq!(r.artist.as_str(), "Amon Tobin");
    }
}

#[tokio::test]
async fn e2e_search_by_nonexistent_artist_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_str().unwrap();
    build_library(root, &five_track_library());

    let storage = LocalFileStorage::new(root);
    let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
    let results = reader.search_by_artist("Zzz").await.unwrap();
    assert!(results.is_empty());
}

#[tokio::test]
async fn e2e_large_library_all_tracks_readable() {
    // 200 tracks across 10 albums — verifies no offset arithmetic errors
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_str().unwrap();

    let mut tracks = Vec::new();
    for album_id in 0..10u32 {
        let album = format!("Album {:02}", album_id);
        for tn in 1u16..=20 {
            let sid = album_id.saturating_mul(20).saturating_add(u32::from(tn));
            tracks.push((sid, album_id, tn, 1u16, "Various", album.as_str(), "Track"));
        }
    }
    // Need owned strings for the static lifetime trick — write a separate builder
    let mut entries: Vec<_> = tracks
        .iter()
        .map(|(sid, aid, tn, dn, ar, al, ti)| {
            let key = sort_key_for(ar, al, *tn, *dn);
            (key, build_meta(*sid, *aid, *tn, *dn, ar, al, ti))
        })
        .collect();
    entries.sort_by_key(|(k, _)| *k);
    let mut w = LibraryWriter::new(root).unwrap();
    for (key, meta) in entries {
        w.add_track(key, meta).unwrap();
    }
    w.finish(10, 0).unwrap();

    let storage = LocalFileStorage::new(root);
    let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
    assert_eq!(reader.track_count(), 200);

    // Read last track — catches off-by-one in offset arithmetic
    let last = reader.track(199).await.unwrap();
    assert!(last.soul_id > 0);
}
```

**Step 2: Run e2e tests**

```bash
cargo test -p library --features std --test e2e_soul_library
```
Expected: all 8 tests pass.

**Step 3: Commit**

```bash
git add crates/library/tests/e2e_soul_library.rs
git commit -m "test(library): add e2e tests — full LibraryWriter→SoulLibraryReader pipeline, no mocks"
```

---

## Task 11: Criterion Benchmarks

**Files:**
- Create: `crates/library/benches/soul_library.rs`
- Modify: `crates/library/Cargo.toml`

**Step 1: Add benchmark dependencies**

In `crates/library/Cargo.toml`:
```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio"] }
tempfile = "3"
tokio = { workspace = true }

[[bench]]
name = "soul_library"
harness = false
```

Add `criterion` to workspace deps in root `Cargo.toml`:
```toml
criterion = { version = "0.5", features = ["async_tokio"] }
```

**Step 2: Write `benches/soul_library.rs`**

```rust
//! Criterion benchmarks for Soul Library binary format.
//!
//! Run: cargo bench -p library --features std --bench soul_library
//!
//! Results show:
//!   binary_search_*k   — O(log N) seek performance vs library size
//!   page_load_20       — realistic "browse" scenario (20 tracks at offset 0)
//!   scan_write_*k      — LibraryWriter throughput for varying library sizes

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, async_executor::TokioExecutor, criterion_group, criterion_main};
use library::binary::{sort_key_for, TrackMeta};
use library::reader::SoulLibraryReader;
use library::writer::LibraryWriter;
use platform::storage_local::LocalFileStorage;
use tempfile::TempDir;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_meta(n: u32) -> TrackMeta {
    let artist = format!("Artist {:04}", n % 100);
    let album = format!("Album {:04}", n % 500);
    let title = format!("Track {:04}", n);
    TrackMeta {
        soul_id: n,
        album_id: n % 500,
        track_number: (n % 20) as u16 + 1,
        disc_number: 1,
        year: 2024,
        format: 0,
        channels: 2,
        duration_secs: 240,
        sample_rate: 44_100,
        title: heapless::String::try_from(title.as_str()).unwrap_or_default(),
        artist: heapless::String::try_from(artist.as_str()).unwrap_or_default(),
        album: heapless::String::try_from(album.as_str()).unwrap_or_default(),
        file_path: heapless::String::try_from(
            format!("/soul/music/{}/{}/{:02}.flac", artist, album, n % 20 + 1).as_str()
        ).unwrap_or_default(),
    }
}

fn build_temp_library(track_count: u32) -> TempDir {
    let tmp = TempDir::new().unwrap();
    let root = tmp.path().to_str().unwrap();
    let mut entries: Vec<_> = (0..track_count)
        .map(|n| {
            let meta = make_meta(n);
            let key = sort_key_for(meta.artist.as_str(), meta.album.as_str(), meta.track_number, 1);
            (key, meta)
        })
        .collect();
    entries.sort_by_key(|(k, _)| *k);
    let mut w = LibraryWriter::new(root).unwrap();
    for (key, meta) in entries {
        w.add_track(key, meta).unwrap();
    }
    w.finish(500, 0).unwrap();
    tmp
}

// ---------------------------------------------------------------------------
// Benchmarks
// ---------------------------------------------------------------------------

fn bench_binary_search(c: &mut Criterion) {
    let mut group = c.benchmark_group("binary_search");
    group.measurement_time(Duration::from_secs(10));

    for track_count in [1_000u32, 10_000, 100_000] {
        let tmp = build_temp_library(track_count);
        let root = tmp.path().to_str().unwrap().to_owned();

        group.bench_with_input(
            BenchmarkId::new("tracks", track_count),
            &root,
            |b, root| {
                b.to_async(TokioExecutor).iter(|| async {
                    let storage = LocalFileStorage::new(root);
                    let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
                    // Search for middle of alphabet — worst case for binary search
                    let _ = reader.search_by_artist("Artist 05").await.unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_page_load(c: &mut Criterion) {
    let tmp = build_temp_library(10_000);
    let root = tmp.path().to_str().unwrap().to_owned();

    c.bench_function("page_load_20_tracks", |b| {
        b.to_async(TokioExecutor).iter(|| async {
            let storage = LocalFileStorage::new(&root);
            let mut reader = SoulLibraryReader::open(storage, &root).await.unwrap();
            let _ = reader.page(0, 20).await.unwrap();
        });
    });
}

fn bench_scan_write(c: &mut Criterion) {
    let mut group = c.benchmark_group("scan_write");
    group.measurement_time(Duration::from_secs(15));

    for track_count in [1_000u32, 10_000] {
        group.bench_with_input(
            BenchmarkId::new("tracks", track_count),
            &track_count,
            |b, &n| {
                b.iter(|| {
                    let tmp = TempDir::new().unwrap();
                    let root = tmp.path().to_str().unwrap();
                    let mut entries: Vec<_> = (0..n)
                        .map(|i| {
                            let meta = make_meta(i);
                            let key = sort_key_for(meta.artist.as_str(), meta.album.as_str(), meta.track_number, 1);
                            (key, meta)
                        })
                        .collect();
                    entries.sort_by_key(|(k, _)| *k);
                    let mut w = LibraryWriter::new(root).unwrap();
                    for (key, meta) in entries {
                        w.add_track(key, meta).unwrap();
                    }
                    w.finish(500, 0).unwrap();
                    tmp // keep alive until end of iter
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_binary_search, bench_page_load, bench_scan_write);
criterion_main!(benches);
```

**Step 3: Run benchmarks (smoke-check)**

```bash
cargo bench -p library --features std --bench soul_library -- --test
```
Expected: all benchmarks pass in test mode (no timing output, just correctness check).

**Step 4: Run for real output (optional — takes ~2 min)**

```bash
cargo bench -p library --features std --bench soul_library
```
Expected: criterion outputs median latency for each benchmark. Record baseline numbers in a code comment.

**Step 5: Commit**

```bash
git add crates/library/benches/soul_library.rs crates/library/Cargo.toml Cargo.toml
git commit -m "bench(library): add Criterion benchmarks — binary_search 1k/10k/100k, page_load, scan_write"
```

---

## Verification Checklist

After all 11 tasks:

```bash
# 1. All library tests pass (unit + integration + e2e)
cargo test -p library --features std

# 2. Library compiles no_std (embedded target)
cargo check -p library

# 3. Platform tests pass
cargo test -p platform --features std

# 4. xtask scan-library works end-to-end
cargo xtask scan-library --music-dir /path/to/music --soul-root /tmp/soul
cargo xtask dev --music-path /tmp/soul

# 5. Emulator compiles with keyboard-input
cargo check -p firmware --features emulator,keyboard-input

# 6. Hardware compile check (no_std)
cargo check -p firmware --features hardware --target thumbv7em-none-eabihf

# 7. Full workspace tests still pass
cargo test --workspace

# 8. Clippy clean
cargo clippy --workspace -- -D warnings

# 9. Benchmarks run in test mode
cargo bench -p library --features std --bench soul_library -- --test
```

---

## Key Design Decisions (Rationale)

| Decision | Why |
|----------|-----|
| Two-file format (idx + meta) | O(1) seek to any track without scanning; O(log N) binary search on 24-byte sort keys |
| Postcard for meta | ~70% smaller than bincode; no_std native; stable v1 wire format across library versions |
| 16-byte sort key with big-endian track nums | Byte-level memcmp sorts correctly without decoding TrackMeta; disc then track ordering within albums |
| 2-level art sharding `art/{hi:02x}/` | FAT32 degrades severely above ~50k files per directory; 256 subdirs caps each at N_albums/256 |
| manifest.bin written last | If power fails mid-write, reader sees missing/old manifest and prompts rescan rather than reading corrupt idx |
| CRC32 checksums in manifest | Detect SD card corruption without full-file hash; crc32fast is fast on Cortex-M7 with hardware multiply |
| LocalFileStorage + SdmmcStorage same trait | Zero application code changes between targets; only feature flag selection differs |
| scan-library infers from path, not tags | Tag parsing requires lofty/id3 (std, heavy); Soul Player provides accurate tags in production |
