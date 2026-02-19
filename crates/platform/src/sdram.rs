//! External SDRAM abstraction via FMC
//!
//! Provides a trait for accessing external SDRAM connected through the STM32H7
//! Flexible Memory Controller (FMC). Used for large allocations that exceed
//! internal SRAM capacity: library index cache, album art thumbnails, and
//! large audio decode scratch buffers.
//!
//! # Hardware
//!
//! - **Option A:** IS42S16320G-7TL (ISSI) — 64 MB (32M × 16-bit), TSOP-54
//! - **Option B:** W9825G6KH-6 (Winbond) — 32 MB (16M × 16-bit), TSOP-54
//!
//! Mapped at `0xC0000000` via FMC bank 5/6 after initialization by the
//! Embassy STM32 FMC driver. Accesses use the CPU cache and are subject to
//! MPU region configuration.
//!
//! # Memory Region Layout (32 MB target)
//!
//! ```text
//! 0xC000_0000  ┌─────────────────────┐
//!              │  Library index cache │  4 MB  (~13k tracks @ 300 B)
//! 0xC040_0000  ├─────────────────────┤
//!              │  Album art cache     │  8 MB  (~500 thumbs @ 16 KB)
//! 0xC0C0_0000  ├─────────────────────┤
//!              │  Audio decode scratch│  4 MB  (FLAC + DSD512 ring buf)
//! 0xC100_0000  ├─────────────────────┤
//!              │  UI overflow / spare │ 16 MB  (future expansion)
//! 0xC200_0000  └─────────────────────┘
//! ```
//!
//! # DMA note
//!
//! SDRAM is accessible by DMA — audio DMA buffers for real-time paths should
//! still prefer internal AXI SRAM (`0x2400_0000`) to avoid FMC arbitration
//! latency. Use SDRAM for large, non-latency-critical buffers only.

/// External SDRAM interface.
///
/// Implementations provide safe, bounds-checked access to a region of
/// SDRAM. On hardware this wraps a `*mut u8` slice over the FMC window;
/// in tests it wraps a heap-allocated `Vec<u8>`.
pub trait ExternalRam {
    /// Error type
    type Error: core::fmt::Debug;

    /// Read bytes from external RAM at `offset`.
    ///
    /// Returns `Err` if `offset + buf.len() > capacity()`.
    fn read(&self, offset: usize, buf: &mut [u8]) -> Result<(), Self::Error>;

    /// Write bytes to external RAM at `offset`.
    ///
    /// Returns `Err` if `offset + data.len() > capacity()`.
    fn write(&mut self, offset: usize, data: &[u8]) -> Result<(), Self::Error>;

    /// Zero-fill `len` bytes starting at `offset`.
    fn zero(&mut self, offset: usize, len: usize) -> Result<(), Self::Error>;

    /// Total SDRAM capacity in bytes.
    fn capacity(&self) -> usize;
}

/// Well-known SDRAM region descriptors.
///
/// Defines the canonical layout for a 32 MB SDRAM device.
/// All offsets are relative to the FMC base address (`0xC000_0000`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RamRegion {
    /// Byte offset from SDRAM base
    pub offset: usize,
    /// Region length in bytes
    pub len: usize,
}

impl RamRegion {
    /// Music library index cache — 4 MB
    ///
    /// Holds serialised track records for browse/search without SD I/O.
    /// Supports ~13,000 tracks at ~300 B per record.
    pub const LIBRARY_INDEX: Self = Self {
        offset: 0,
        len: 4 * 1024 * 1024,
    };

    /// Album art thumbnail cache — 8 MB
    ///
    /// Pre-scaled thumbnails (96×96 px, 2bpp = 2.3 KB each).
    /// Fits ~500 album thumbnails resident in memory.
    pub const ALBUM_ART: Self = Self {
        offset: 4 * 1024 * 1024,
        len: 8 * 1024 * 1024,
    };

    /// Audio decoder scratch — 4 MB
    ///
    /// Working memory for FLAC frame decode (~128 KB) and DSD512
    /// streaming ring buffer (~1.1 MB for 200 ms at stereo DSD512).
    pub const AUDIO_SCRATCH: Self = Self {
        offset: 12 * 1024 * 1024,
        len: 4 * 1024 * 1024,
    };

    /// UI overflow / reserved — 16 MB
    ///
    /// Available for future features: waveform display buffers,
    /// additional font glyph caches, OTA download staging.
    pub const UI_OVERFLOW: Self = Self {
        offset: 16 * 1024 * 1024,
        len: 16 * 1024 * 1024,
    };
}
