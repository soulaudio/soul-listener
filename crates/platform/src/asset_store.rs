//! QSPI NOR flash asset store abstraction
//!
//! Read-only access to static assets (fonts, icons, waveform LUTs) stored in
//! external QSPI NOR flash via the STM32H7 QUADSPI peripheral.
//!
//! # Hardware
//!
//! - **Option A:** W25Q128JV (Winbond) — 16 MB, SPI/QSPI, 133 MHz, SOIC-8
//! - **Option B:** W25Q64JV  (Winbond) —  8 MB, SPI/QSPI, 133 MHz, SOIC-8
//!
//! Mapped at `0x9000_0000` in `XiP` (memory-mapped) mode after QUADSPI
//! initialisation. The internal 2 MB flash is reserved for compiled firmware
//! only; all read-only assets live here.
//!
//! # Flash Partition Layout
//!
//! ```text
//! 0x9000_0000  ┌──────────────────────┐
//!              │  Asset index table   │   4 KB  (offset + size per key)
//! 0x9000_1000  ├──────────────────────┤
//!              │  Fonts               │   ~500 KB  (5 sizes, Latin+)
//! 0x9008_0000  ├──────────────────────┤
//!              │  Icons               │   ~200 KB  (100 icons, 64×64, 2bpp)
//! 0x900B_0000  ├──────────────────────┤
//!              │  Waveform LUTs       │    ~50 KB  (SSD1677 custom LUTs)
//! 0x900C_0000  ├──────────────────────┤
//!              │  OTA staging         │   ~1.5 MB  (full firmware image)
//! 0x901C_0000  ├──────────────────────┤
//!              │  Reserved / spare    │  remainder
//!              └──────────────────────┘
//! ```
//!
//! Assets are written once during factory programming via `cargo xtask flash-assets`
//! (to be implemented). The OTA staging partition is erased/written at runtime
//! by the firmware update task.

/// Read-only asset store backed by QSPI NOR flash.
///
/// On hardware, reads go directly through the `XiP` memory-mapped window
/// (zero-copy). In tests, a mock implementation returns pre-loaded bytes.
pub trait AssetStore {
    /// Error type
    type Error: core::fmt::Debug;

    /// Read up to `buf.len()` bytes of `key` into `buf`, starting at
    /// `offset` within the asset.
    ///
    /// Returns the number of bytes actually read (may be less than
    /// `buf.len()` if `offset + buf.len() > asset_size(key)`).
    fn read_asset(
        &self,
        key: AssetKey,
        offset: usize,
        buf: &mut [u8],
    ) -> Result<usize, Self::Error>;

    /// Return the size in bytes of `key`, or `Err` if the key is absent.
    fn asset_size(&self, key: AssetKey) -> Result<usize, Self::Error>;

    /// Return `true` if `key` is present in the store.
    fn asset_exists(&self, key: AssetKey) -> bool;
}

/// Catalogue of well-known asset keys stored in QSPI NOR flash.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AssetKey {
    // ── Fonts ────────────────────────────────────────────────────────────────
    /// Bitmap font, 12 px — metadata labels, timestamps
    Font12,
    /// Bitmap font, 16 px — secondary UI text
    Font16,
    /// Bitmap font, 24 px — primary UI text, menu items
    Font24,
    /// Bitmap font, 32 px — artist/album names on Now Playing
    Font32,
    /// Bitmap font, 48 px bold — track title on Now Playing screen
    Font48Bold,

    // ── Icons ────────────────────────────────────────────────────────────────
    /// Packed icon sprite sheet, 64×64 px, 2bpp, all UI icons
    Icons,

    // ── E-ink waveform LUTs ──────────────────────────────────────────────────
    /// Custom SSD1677 waveform LUT table (replaces OTP defaults)
    WaveformLut,

    // ── OTA ──────────────────────────────────────────────────────────────────
    /// OTA firmware staging partition (written at runtime, read on reboot)
    OtaStaging,
}
