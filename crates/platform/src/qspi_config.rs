//! QUADSPI configuration for W25Q128JV NOR flash in memory-mapped (XiP) mode.
//!
//! XiP = eXecute in Place: the QUADSPI controller presents the flash as a
//! memory-mapped read-only window at `0x9000_0000`. The CPU can fetch
//! instructions and data directly from it without explicit SPI transactions.
//!
//! # Hardware
//!
//! **Flash chip:** W25Q128JV (Winbond) — 16 MB, 133 MHz max, SOIC-8
//!
//! **Fast Read Quad I/O command (0xEB):**
//! - 8-bit instruction phase (single wire)
//! - 24-bit address phase (quad wires)
//! - 4 dummy cycles (quad wires, required for continuous read mode at ≥ 80 MHz)
//! - N-byte data phase (quad wires)
//!
//! In XiP memory-mapped mode, the QUADSPI controller generates the 0xEB command
//! automatically for every cache-line fetch from the 0x9000_0000 window.
//!
//! # Embassy / PAC Note
//!
//! Embassy-stm32 issue \#3149: `embassy_stm32::qspi` does **not** implement
//! memory-mapped mode. XiP must be enabled via PAC-level register writes:
//!
//! ```text
//! QUADSPI.CCR: FMODE = 0b11 (memory-mapped)
//!              IMODE = 0b01 (1-wire instruction)
//!              ADMODE = 0b11 (4-wire address)
//!              DMODE = 0b11 (4-wire data)
//!              DCYC = 4     (dummy cycles)
//!              INSTRUCTION = 0xEB
//! QUADSPI.AR  = 0x0000_0000 (auto-incremented by hardware)
//! ```
//!
//! # Sources
//!
//! - W25Q128JV datasheet (Winbond, rev. L 2021): §8.2.14 Fast Read Quad I/O
//! - STM32H743 Reference Manual RM0433: §24.3 QUADSPI functional description
//! - Vivonomicon "Bare Metal STM32 Part 12" QSPI guide:
//!   <https://vivonomicon.com/2020/08/08/bare-metal-stm32-programming-part-12-using-quad-spi-flash-memory/>

/// QUADSPI clock prescaler for target read frequency.
///
/// `QUADSPI_CLK = AHB_CLK / (QSPI_PRESCALER + 1)`
///
/// At AHB = 240 MHz: `prescaler = 1` → 120 MHz (within W25Q128JV 133 MHz max).
/// At AHB = 480 MHz: `prescaler = 3` → 120 MHz (same target frequency).
///
/// We target 240 MHz AHB (half of sysclk=480 MHz via AHB prescaler=2).
pub const QSPI_PRESCALER: u8 = 1;

/// Flash size field for `QUADSPI_DCR.FSIZE`.
///
/// Hardware formula: addressable bytes = 2^(`FSIZE` + 1).
/// W25Q128JV = 16 MB = 16,777,216 bytes = 2^24 → `FSIZE = 23`.
pub const QSPI_FLASH_SIZE: u8 = 23;

/// Number of dummy cycles for Fast Read Quad I/O (command 0xEB).
///
/// W25Q128JV datasheet §8.2.14: 4 dummy cycles required when using the
/// Fast Read Quad I/O (0xEB) command in quad mode at speeds ≥ 80 MHz.
/// These cycles allow the flash internal circuitry to prepare output data.
pub const QSPI_DUMMY_CYCLES: u8 = 4;

/// Fast Read Quad I/O command byte for W25Q128JV.
///
/// 0xEB = Fast Read Quad I/O: sends a 24-bit address on all 4 SIO lines,
/// followed by `QSPI_DUMMY_CYCLES` dummy cycles, then streams data on 4 lines.
/// This is the highest-throughput single-chip read mode on W25Q128JV.
pub const QSPI_READ_CMD: u8 = 0xEB;

/// Base address of the QUADSPI memory-mapped region in the STM32H7 memory map.
///
/// This is a hardware constant from the STM32H743 memory map (RM0433 Table 1).
/// All asset flash addresses are offsets from this base.
pub const QSPI_BASE_ADDR: u32 = 0x9000_0000;

/// W25Q128JV maximum operating frequency (Hz).
///
/// From datasheet: VCC = 2.7–3.6 V, `f_R` (read frequency) max = 133 MHz.
/// Used in `validate_qspi_prescaler` to guard against over-clocking.
pub const QSPI_MAX_FREQ_HZ: u32 = 133_000_000;

/// Asset partition offsets within QSPI flash (relative to flash start, not
/// to `QSPI_BASE_ADDR`; add `QSPI_BASE_ADDR` to get the CPU address in XiP mode).
///
/// Written once during factory programming. The OTA partition is the only
/// region written at runtime by the firmware update task.
///
/// # Partition map (W25Q128JV, 16 MB total)
///
/// ```text
/// Offset       Size    Contents
/// 0x0000_0000   4 KB   Asset index table (offset + size per AssetKey)
/// 0x0000_1000 ~500 KB  Fonts (5 sizes: 12/16/24/32/48 px, Latin+)
/// 0x0008_0000 ~200 KB  Icons (100 icons, 64×64, 2bpp sprite sheet)
/// 0x000B_0000  ~50 KB  Waveform LUTs (SSD1677 custom EPD LUT tables)
/// 0x000C_0000 ~1.5 MB  OTA staging (full firmware image)
/// 0x001C_0000  rest    Reserved / spare
/// ```
pub mod partitions {
    /// Asset index table — 4 KB at flash offset 0.
    ///
    /// Stores a fixed-size record per `AssetKey`: (flash offset: u32, size: u32).
    /// Firmware reads this at boot to locate all other partitions.
    pub const ASSET_INDEX: u32 = 0x0000_0000;

    /// Bitmap font data — starts at 4 KB offset.
    ///
    /// Contains 5 font sizes (12/16/24/32/48 px) in a compact glyph format.
    /// Total ~500 KB.
    pub const FONTS: u32 = 0x0000_1000;

    /// Icon sprite sheet — 64×64 px, 2bpp, all UI icons.
    ///
    /// ~200 KB; icon index baked into firmware as an enum-to-offset table.
    pub const ICONS: u32 = 0x0008_0000;

    /// SSD1677 waveform LUT tables — custom EPD refresh sequences.
    ///
    /// ~50 KB; replaces OTP defaults on the Good Display GDEM0397T81P panel.
    pub const WAVEFORM_LUTS: u32 = 0x000B_0000;

    /// OTA firmware staging — ~1.5 MB.
    ///
    /// A complete firmware image is downloaded here before the bootloader
    /// verifies and applies it. Erased at the start of each OTA session.
    pub const OTA_STAGING: u32 = 0x000C_0000;
}

/// Validate that a QUADSPI prescaler value produces a clock within W25Q128JV limits.
///
/// # Arguments
///
/// * `ahb_hz` — AHB bus frequency in Hz (e.g. `240_000_000` for 240 MHz).
/// * `prescaler` — The `QUADSPI_CR.PRESCALER` value (0–255); QSPI clock =
///   `ahb_hz / (prescaler + 1)`.
///
/// # Returns
///
/// `Ok(qspi_hz)` — actual QSPI clock in Hz, if within the 133 MHz spec.
/// `Err(&'static str)` — human-readable error if the clock exceeds 133 MHz.
///
/// # Example
///
/// ```rust
/// use platform::qspi_config::validate_qspi_prescaler;
/// let hz = validate_qspi_prescaler(240_000_000, 1).unwrap();
/// assert_eq!(hz, 120_000_000);
/// ```
pub fn validate_qspi_prescaler(ahb_hz: u32, prescaler: u8) -> Result<u32, &'static str> {
    let qspi_hz = ahb_hz / (u32::from(prescaler) + 1);
    if qspi_hz > QSPI_MAX_FREQ_HZ {
        return Err("QSPI clock exceeds W25Q128JV maximum of 133 MHz");
    }
    Ok(qspi_hz)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    /// At 240 MHz AHB with prescaler=1, QSPI clock = 120 MHz (within 133 MHz spec).
    #[test]
    fn test_qspi_prescaler_at_240mhz() {
        let result = validate_qspi_prescaler(240_000_000, QSPI_PRESCALER);
        assert_eq!(
            result,
            Ok(120_000_000),
            "prescaler=1 at 240 MHz AHB must yield 120 MHz QSPI clock"
        );
    }

    /// At 240 MHz AHB with prescaler=0, QSPI clock = 240 MHz which exceeds 133 MHz.
    ///
    /// This would over-clock the W25Q128JV and must be rejected.
    #[test]
    fn test_qspi_prescaler_zero_would_exceed_spec() {
        let result = validate_qspi_prescaler(240_000_000, 0);
        assert!(
            result.is_err(),
            "prescaler=0 at 240 MHz must be rejected (240 MHz > 133 MHz W25Q128JV max)"
        );
    }

    /// Flash size field: 2^(QSPI_FLASH_SIZE + 1) must equal 16 MB.
    ///
    /// This encodes the W25Q128JV capacity in the QUADSPI_DCR.FSIZE register.
    #[test]
    fn test_qspi_flash_size_field() {
        let bytes: u32 = 1u32 << (u32::from(QSPI_FLASH_SIZE) + 1);
        assert_eq!(
            bytes,
            16 * 1024 * 1024,
            "QSPI_FLASH_SIZE must encode 16 MB (W25Q128JV capacity)"
        );
    }

    /// ASSET_INDEX must be at the very start of flash (offset 0).
    #[test]
    fn test_partition_asset_index_at_base() {
        assert_eq!(
            partitions::ASSET_INDEX,
            0,
            "asset index table must be at flash offset 0"
        );
    }

    /// FONTS must come after ASSET_INDEX.
    #[test]
    fn test_partition_fonts_after_index() {
        assert!(
            partitions::FONTS > partitions::ASSET_INDEX,
            "font partition must follow the asset index table"
        );
    }

    /// OTA staging must be the last named partition (highest offset).
    #[test]
    fn test_partition_ota_is_last() {
        assert!(
            partitions::OTA_STAGING > partitions::WAVEFORM_LUTS,
            "OTA staging partition must follow waveform LUTs"
        );
        assert!(
            partitions::OTA_STAGING > partitions::ICONS,
            "OTA staging partition must follow icons"
        );
        assert!(
            partitions::OTA_STAGING > partitions::FONTS,
            "OTA staging partition must follow fonts"
        );
    }
}
