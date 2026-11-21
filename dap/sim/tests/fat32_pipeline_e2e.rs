//! Headless, hardware-free proof that the **real** FAT32 `Storage` adapter —
//! not the `SimStorage` stub — drives the whole audio path end to end: a real
//! WAV sitting in a FAT32 filesystem on a RAM "SD card" is opened through
//! `storage_fat32::SdStorage` (`embedded-sdmmc` over a `BlockDevice`), decoded +
//! resampled by `soul-audio-core`, and lands in a `Sink` — all through
//! `dap_core::audio::AudioPlayer`, exactly as the board will run it once the
//! real SDMMC `BlockDevice` replaces this RAM one (Phase E).
//!
//! This is the integration the standalone `storage-fat32/tests/fat32.rs` (byte
//! round-trip) and `audio_headless.rs` (`SimStorage` → decode) each cover only
//! half of: here the real card adapter feeds the real decode pipeline.

use std::cell::RefCell;

use dap_core::audio::AudioPlayer;
use dap_sim::{sac_scratch, SimSink};
use embedded_sdmmc::{
    Block, BlockCount, BlockDevice, BlockIdx, TimeSource, Timestamp, VolumeIdx, VolumeManager,
};
use soul_audio_core::{Progress, Quality};
use storage_fat32::SdStorage;

const PRELOAD: usize = 0;

// --- A RAM-backed SD "card" + dummy clock (same shape as storage-fat32's own
// test rig), so the whole thing is hardware-free. ---

struct RamDisk {
    data: RefCell<Vec<u8>>,
}
#[derive(Debug)]
struct RamError;
impl core::fmt::Display for RamError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "ram oob")
    }
}
impl core::error::Error for RamError {}
impl BlockDevice for RamDisk {
    type Error = RamError;
    fn read(&self, blocks: &mut [Block], start: BlockIdx) -> Result<(), RamError> {
        let d = self.data.borrow();
        for (i, blk) in blocks.iter_mut().enumerate() {
            let off = (start.0 as usize + i) * 512;
            blk.contents
                .copy_from_slice(d.get(off..off + 512).ok_or(RamError)?);
        }
        Ok(())
    }
    fn write(&self, blocks: &[Block], start: BlockIdx) -> Result<(), RamError> {
        let mut d = self.data.borrow_mut();
        for (i, blk) in blocks.iter().enumerate() {
            let off = (start.0 as usize + i) * 512;
            d.get_mut(off..off + 512)
                .ok_or(RamError)?
                .copy_from_slice(&blk.contents);
        }
        Ok(())
    }
    fn num_blocks(&self) -> Result<BlockCount, RamError> {
        Ok(BlockCount((self.data.borrow().len() / 512) as u32))
    }
}

struct DummyTime;
impl TimeSource for DummyTime {
    fn get_timestamp(&self) -> Timestamp {
        Timestamp {
            year_since_1970: 55,
            zero_indexed_month: 0,
            zero_indexed_day: 0,
            hours: 0,
            minutes: 0,
            seconds: 0,
        }
    }
}

const PART_START_LBA: u32 = 2048;

/// A partitioned (MBR, type 0x0C) FAT32 image carrying one root file, `TONE.WAV`,
/// whose bytes are the real `soul-audio-core/testdata/tone.wav`.
fn make_fat32_card_with_tone() -> Vec<u8> {
    let wav = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../../soul-audio-core/testdata/tone.wav"),
    )
    .expect("read tone.wav fixture");

    let size = 48 * 1024 * 1024usize;
    let total_sectors = (size / 512) as u32;
    let mut buf = vec![0u8; size];

    let part_sectors = total_sectors - PART_START_LBA;
    let e = 446;
    buf[e + 4] = 0x0C; // FAT32 (LBA)
    buf[e + 8..e + 12].copy_from_slice(&PART_START_LBA.to_le_bytes());
    buf[e + 12..e + 16].copy_from_slice(&part_sectors.to_le_bytes());
    buf[510] = 0x55;
    buf[511] = 0xAA;

    let part_off = PART_START_LBA as usize * 512;
    {
        let cursor = std::io::Cursor::new(&mut buf[part_off..]);
        let opts = fatfs::FormatVolumeOptions::new().fat_type(fatfs::FatType::Fat32);
        fatfs::format_volume(fscommon::BufStream::new(cursor), opts).expect("format fat32");
    }
    {
        let cursor = std::io::Cursor::new(&mut buf[part_off..]);
        let fs = fatfs::FileSystem::new(fscommon::BufStream::new(cursor), fatfs::FsOptions::new())
            .expect("mount for write");
        use std::io::Write;
        fs.root_dir()
            .create_file("TONE.WAV")
            .unwrap()
            .write_all(&wav)
            .unwrap();
    }
    buf
}

type Player = AudioPlayer<SimSink, SdStorage<RamDisk, DummyTime>, Box<[i32]>, PRELOAD>;

fn build_player() -> Player {
    let dev = RamDisk {
        data: RefCell::new(make_fat32_card_with_tone()),
    };
    // Leak the VolumeManager for the `&'static` the adapter's lifetime-free
    // `Storage` port needs (exactly the `StaticCell` pattern the board uses).
    let vm: &'static VolumeManager<RamDisk, DummyTime> =
        Box::leak(Box::new(VolumeManager::new(dev, DummyTime)));
    static NAMES: &[&str] = &["TONE.WAV"];
    let storage = SdStorage::new(vm, VolumeIdx(0), NAMES).expect("mount FAT32 card");

    let mut player = AudioPlayer::new(SimSink::new(), storage, Quality::Fast, sac_scratch);
    player.set_queue(1, 0);
    player.play();
    player
}

#[test]
fn real_fat32_card_decodes_through_the_full_pipeline() {
    // Generous worker stack — same reason as `audio_headless.rs`: `Deck`/`Engine`
    // embed `AnyDecoder` by value and move it through unoptimized debug frames.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(run)
        .expect("spawn worker")
        .join()
        .expect("worker panicked");
}

fn run() {
    let mut player = build_player();

    // One pump opens TONE.WAV straight off the FAT32 card and decodes a block.
    player.pump().expect("pump");
    let info = player
        .decode_info()
        .expect("track 0 loaded from the FAT32 card");
    assert_eq!(info.decoded.sample_rate, 44_100, "tone.wav is 44.1 kHz");
    assert_eq!(info.device_rate, 48_000, "sink declares 48 kHz");
    assert!(
        info.resampling,
        "44.1 kHz source into a 48 kHz sink resamples"
    );

    // Run to the end of the one-track queue.
    for _ in 0..1_000_000 {
        if player.pump().expect("pump") == Progress::Ended {
            break;
        }
    }

    let sink = player.sink();
    let channels = sink.channels();
    let recorded = sink.recorded();
    let total_frames = recorded.len() / channels;

    // 88_200 src frames @44.1k resample to exactly 96_000 @48k (88_200*48/44.1),
    // with slack for the resampler's block-quantized startup/drain.
    assert!(
        total_frames.abs_diff(96_000) < 2_000,
        "decoded {total_frames} frames off the card, expected ~96_000"
    );
    // The real card actually produced audio, not silence.
    let energy: f64 = recorded.iter().map(|&s| (s as f64).abs()).sum();
    assert!(
        energy > 0.0,
        "pipeline produced only silence from the FAT32 card — the storage→decode path is broken"
    );
}
