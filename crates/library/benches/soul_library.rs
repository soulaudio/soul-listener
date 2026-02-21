//! Criterion benchmarks for Soul Library binary format.
//!
//! Run: cargo bench -p library --features std --bench soul_library
//!
//! Results show:
//!   binary_search_*k   — O(log N) seek performance vs library size
//!   page_load_20       — realistic "browse" scenario (20 tracks at offset 0)
//!   scan_write_*k      — LibraryWriter throughput for varying library sizes

#![allow(
    clippy::unwrap_used,              // benchmark helpers use unwrap for brevity
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation, // intentional u32→u16 truncation after .min(u16::MAX)
    missing_docs,                     // criterion_group! macro generates undocumented items
)]

use std::time::Duration;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use library::binary::{TrackMeta, sort_key_for};
use library::reader::SoulLibraryReader;
use library::writer::LibraryWriter;
use platform::storage_local::LocalFileStorage;
use tempfile::TempDir;
use tokio::runtime::Builder;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn make_meta(n: u32) -> TrackMeta {
    let artist = format!("Artist {:04}", n % 100);
    let album = format!("Album {:04}", n % 500);
    let title = format!("Track {:04}", n);
    // n % 20 is at most 19, plus 1 = 20 — fits in u16.
    // saturating_add avoids arithmetic_side_effects; .min(u16::MAX) prevents truncation.
    let track_num: u16 = (n % 20).saturating_add(1).min(u32::from(u16::MAX)) as u16;
    let file_path = format!(
        "/soul/music/{}/{}/{:02}.flac",
        artist,
        album,
        (n % 20).saturating_add(1),
    );
    TrackMeta {
        soul_id: n,
        album_id: n % 500,
        track_number: track_num,
        disc_number: 1,
        year: 2024,
        format: 0,
        channels: 2,
        duration_secs: 240,
        sample_rate: 44_100,
        title: heapless::String::try_from(title.as_str()).unwrap_or_default(),
        artist: heapless::String::try_from(artist.as_str()).unwrap_or_default(),
        album: heapless::String::try_from(album.as_str()).unwrap_or_default(),
        file_path: heapless::String::try_from(file_path.as_str()).unwrap_or_default(),
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
    let rt = Builder::new_current_thread().enable_all().build().unwrap();
    let mut group = c.benchmark_group("binary_search");
    group.measurement_time(Duration::from_secs(10));

    for track_count in [1_000u32, 10_000, 100_000] {
        let tmp = build_temp_library(track_count);
        let root = tmp.path().to_str().unwrap().to_owned();

        group.bench_with_input(
            BenchmarkId::new("tracks", track_count),
            &root,
            |b, root| {
                b.to_async(&rt).iter(|| async {
                    let storage = LocalFileStorage::new(root);
                    let mut reader = SoulLibraryReader::open(storage, root).await.unwrap();
                    // "Artist 05" is middle of alphabet — exercises binary search fully
                    let _ = reader.search_by_artist("Artist 05").await.unwrap();
                });
            },
        );
    }
    group.finish();
}

fn bench_page_load(c: &mut Criterion) {
    let rt = Builder::new_current_thread().enable_all().build().unwrap();
    let tmp = build_temp_library(10_000);
    let root = tmp.path().to_str().unwrap().to_owned();

    c.bench_function("page_load_20_tracks", |b| {
        b.to_async(&rt).iter(|| async {
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
                            let key = sort_key_for(
                                meta.artist.as_str(),
                                meta.album.as_str(),
                                meta.track_number,
                                1,
                            );
                            (key, meta)
                        })
                        .collect();
                    entries.sort_by_key(|(k, _)| *k);
                    let mut w = LibraryWriter::new(root).unwrap();
                    for (key, meta) in entries {
                        w.add_track(key, meta).unwrap();
                    }
                    w.finish(500, 0).unwrap();
                    tmp // keep TempDir alive until end of iter
                });
            },
        );
    }
    group.finish();
}

criterion_group!(benches, bench_binary_search, bench_page_load, bench_scan_write);
criterion_main!(benches);
