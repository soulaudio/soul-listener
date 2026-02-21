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

    // Build (sort_key, TrackMeta) entries directly to avoid &str lifetime issues.
    let mut entries: Vec<([u8; 16], TrackMeta)> = Vec::new();
    for album_id in 0..10u32 {
        let album = format!("Album {:02}", album_id);
        for tn in 1u16..=20 {
            let sid = album_id.saturating_mul(20).saturating_add(u32::from(tn));
            let key = sort_key_for("Various", &album, tn, 1);
            let meta = build_meta(sid, album_id, tn, 1, "Various", &album, "Track");
            entries.push((key, meta));
        }
    }
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
