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
    debug_assert!(
        root.len().saturating_add(20) <= 80,
        "art_path: root ({} bytes) + art suffix (20 bytes) exceeds String<80>",
        root.len()
    );
    // SAFETY: album_id >> 24 gives bits 31..24; casting to u8 keeps only
    // the low 8 bits, which is exactly what we want for the shard byte.
    #[allow(clippy::cast_possible_truncation)]
    let hi = (album_id >> 24) as u8;
    let mut s = String::<80>::new();
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
    debug_assert!(
        root.len().saturating_add(suffix.len()) <= 64,
        "build_path: root ({} bytes) + suffix ({} bytes) exceeds String<64>",
        root.len(),
        suffix.len()
    );
    let mut s = String::<64>::new();
    let _ = s.push_str(root);
    let _ = s.push_str(suffix);
    s
}

// SAFETY: HEX has exactly 16 elements. Both `byte >> 4` and `byte & 0xF`
// produce values in 0..=15, so the indices are always in-bounds.
#[allow(clippy::indexing_slicing)]
const HEX: &[u8; 16] = b"0123456789abcdef";

#[allow(clippy::indexing_slicing, clippy::cast_possible_truncation)]
fn push_hex2<const N: usize>(s: &mut String<N>, byte: u8) {
    // byte >> 4 is in 0..=15; byte & 0xF is in 0..=15 — both within HEX bounds.
    let _ = s.push(HEX[(byte >> 4) as usize] as char);
    let _ = s.push(HEX[(byte & 0xF) as usize] as char);
}

#[allow(clippy::cast_possible_truncation)]
fn push_hex8<const N: usize>(s: &mut String<N>, val: u32) {
    // Each shift + cast extracts exactly 8 bits — truncation is intentional.
    push_hex2(s, (val >> 24) as u8);
    push_hex2(s, (val >> 16) as u8);
    push_hex2(s, (val >> 8) as u8);
    push_hex2(s, val as u8);
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn manifest_path_is_under_soul_root() {
        assert_eq!(manifest_path(SOUL_ROOT).as_str(), "/soul/manifest.bin");
    }

    #[test]
    fn library_idx_path_is_under_soul_root() {
        assert_eq!(library_idx_path(SOUL_ROOT).as_str(), "/soul/library.idx");
    }

    #[test]
    fn library_meta_path_is_under_soul_root() {
        assert_eq!(library_meta_path(SOUL_ROOT).as_str(), "/soul/library.meta");
    }

    #[test]
    fn art_path_uses_two_level_sharding() {
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
        assert_eq!(manifest_path("/music").as_str(), "/music/manifest.bin");
    }
}
