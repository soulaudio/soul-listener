//! End-to-end host test, no SD hardware: build a partitioned FAT32 image with the
//! independent `fatfs` crate, mount it through `SdStorage` over a RAM block
//! device, and read files back through the `embedded_io` Source ladder — proving
//! the real adapter path (`SdStorage::open` -> `SdSource` -> decoder reads).

use std::cell::RefCell;

use dap_core::ports::Storage;
use embedded_io::{Read, Seek, SeekFrom};
use embedded_sdmmc::{Block, BlockCount, BlockDevice, BlockIdx, TimeSource, Timestamp, VolumeIdx};
use storage_fat32::{SdError, SdStorage};

// --- Host test infrastructure: a RAM-backed BlockDevice + a dummy clock ---

struct RamDisk {
    data: RefCell<Vec<u8>>,
}
impl RamDisk {
    fn new(data: Vec<u8>) -> Self {
        Self {
            data: RefCell::new(data),
        }
    }
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

/// Big fixture file bytes (spans many sectors — exercises streaming).
fn big_bytes() -> Vec<u8> {
    (0..5000u32).map(|i| (i % 251) as u8).collect()
}

/// A partitioned (MBR, type 0x0C) FAT32 image with two root files.
fn make_fat32_image() -> Vec<u8> {
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
        let root = fs.root_dir();
        use std::io::Write;
        root.create_file("HELLO.TXT")
            .unwrap()
            .write_all(b"hello from fat32")
            .unwrap();
        root.create_file("AUDIO.BIN")
            .unwrap()
            .write_all(&big_bytes())
            .unwrap();
    }
    buf
}

/// Leak a VolumeManager over the image to get the `&'static` the adapter needs,
/// then build `SdStorage` over the two root files.
fn storage() -> SdStorage<RamDisk, DummyTime> {
    let dev = RamDisk::new(make_fat32_image());
    let vm: &'static embedded_sdmmc::VolumeManager<RamDisk, DummyTime> =
        Box::leak(Box::new(embedded_sdmmc::VolumeManager::new(dev, DummyTime)));
    static NAMES: &[&str] = &["HELLO.TXT", "AUDIO.BIN"];
    SdStorage::new(vm, VolumeIdx(0), NAMES).expect("mount FAT32")
}

fn read_all<S: Read>(src: &mut S) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = [0u8; 300]; // deliberately not a sector multiple
    loop {
        let n = src.read(&mut buf).expect("read");
        if n == 0 {
            break;
        }
        out.extend_from_slice(&buf[..n]);
    }
    out
}

#[test]
fn opens_and_reads_small_file_through_source_ladder() {
    let mut st = storage();
    let mut src = st.open(0).expect("open track 0");
    assert_eq!(read_all(&mut src), b"hello from fat32");
}

#[test]
fn streams_multi_sector_file_byte_exact() {
    let mut st = storage();
    let mut src = st.open(1).expect("open track 1");
    assert_eq!(read_all(&mut src), big_bytes());
}

#[test]
fn seek_then_read_lands_at_the_right_bytes() {
    let mut st = storage();
    let mut src = st.open(1).expect("open track 1");
    let expected = big_bytes();
    // Seek absolute, read.
    let p = src.seek(SeekFrom::Start(1000)).expect("seek start");
    assert_eq!(p, 1000);
    let mut b = [0u8; 16];
    src.read(&mut b).expect("read after seek");
    assert_eq!(&b, &expected[1000..1016]);
    // Seek relative to end.
    src.seek(SeekFrom::End(-4)).expect("seek end");
    let mut tail = [0u8; 4];
    src.read(&mut tail).expect("read tail");
    assert_eq!(&tail, &expected[expected.len() - 4..]);
}

#[test]
fn open_out_of_range_errors() {
    let mut st = storage();
    assert!(matches!(st.open(9), Err(SdError::OutOfRange)));
}
