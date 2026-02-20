//! MPU (Memory Protection Unit) configuration for STM32H743 / Cortex-M7
//!
//! # Purpose
//!
//! The Cortex-M7 has a 16 KB D-cache that is enabled by default in Embassy.
//! Any DMA buffer in a cacheable memory region will suffer **silent data
//! corruption** unless either:
//!   (a) The MPU marks the region as non-cacheable/strongly-ordered, OR
//!   (b) The software performs explicit cache maintenance (`dsb` + cache
//!       invalidate) after every DMA RX transfer.
//!
//! The recommended approach for embedded is (a): configure the MPU to mark
//! all DMA-accessible SRAM regions as non-cacheable before any peripheral
//! init. This is documented in ST Application Note AN4838 and AN4839.
//!
//! # References
//!
//! - ARM Cortex-M7 TRM DDI0489F — MPU Region Attribute and Size Register
//! - ST AN4838 — Introduction to MPU Management on STM32 MCUs
//! - ST AN4839 — Level 1 cache on STM32F7 and STM32H7
//! - [Cache in ARM Cortex M7: MPU Configuration](https://blog.embeddedexpert.io/?p=2739)
//! - [STM32 DMA not working — cache coherency FAQ](https://community.st.com/t5/stm32-mcus/dma-is-not-working-on-stm32h7-devices/ta-p/49498)
//! - [ARM MPU Region Attribute Register](https://developer.arm.com/documentation/dui0646/latest/Cortex-M7-Peripherals/Optional-Memory-Protection-Unit/MPU-Region-Attribute-and-Size-Register)
//!
//! # STM32H743 Memory Domains and DMA Constraints
//!
//! | Region          | Address       | Size   | DMA1/DMA2 | BDMA   | Notes                  |
//! |-----------------|---------------|--------|-----------|--------|------------------------|
//! | DTCM            | 0x2000_0000   | 128 KB | NO        | NO     | CPU-only tightly coupled RAM |
//! | AXI SRAM (D1)   | 0x2400_0000   | 512 KB | YES       | NO     | Primary DMA buffer pool |
//! | SRAM1/2 (D2)    | 0x3000_0000   | 256 KB | YES       | NO     | Task stacks, collections |
//! | SRAM3 (D2)      | 0x3004_0000   | 32 KB  | YES       | NO     | USB buffers            |
//! | SRAM4 (D3)      | 0x3800_0000   | 64 KB  | NO        | YES    | BDMA peripherals only  |
//!
//! BDMA can only reach the D3 domain (SRAM4 at 0x3800_0000). It cannot access
//! AXI SRAM or any D1/D2 memory. The BDMA-only peripherals are:
//! - **SPI6** — D3 domain SPI
//! - **I2C4** — D3 domain I2C
//! - **LPUART1** — D3 low-power UART
//! - **ADC3** — D3 domain ADC
//! - **SAI4** — D3 domain SAI
//! - **DFSDM2** — D3 domain digital filter
//!
//! DTCM is not reachable by *any* DMA controller — it has a dedicated CPU path
//! that bypasses the AXI bus matrix. Placing DMA buffers in DTCM causes silent
//! failures (DMA sees stale/zero data; the CPU sees correct data via its local
//! path). Confirmed by Embassy-STM32 documentation and ST community FAQs.
//!
//! # MPU Region Requirements (Cortex-M7, ARM DDI0489F §B3.5)
//!
//! - Minimum region size: **32 bytes** (SIZE field = 4)
//! - Size must be a **power of 2**
//! - Base address must be **aligned to the region size**
//! - ARM MPU SIZE field encoding: `SIZE = log2(size_bytes) − 1`
//!   - 32 B   → SIZE = 4  (2^5, trailing_zeros = 5, 5 − 1 = 4)
//!   - 64 KB  → SIZE = 15 (2^16, trailing_zeros = 16, 16 − 1 = 15)
//!   - 512 KB → SIZE = 18 (2^19, trailing_zeros = 19, 19 − 1 = 18)
//!   - 1 MB   → SIZE = 19 (2^20, trailing_zeros = 20, 20 − 1 = 19)
//!
//! # Memory Attribute Bits (TEX, S, C, B)
//!
//! For non-cacheable DMA buffers (normal memory, non-cacheable):
//! - **TEX = 001, S = 0, C = 0, B = 0**
//!
//! For strongly ordered (peripheral registers):
//! - **TEX = 000, S = 1, C = 0, B = 0**
//!
//! For write-back, no write-allocate (normal cached RAM):
//! - **TEX = 000, S = 0, C = 1, B = 1**
//!
//! For write-through, no write-allocate:
//! - **TEX = 000, S = 0, C = 1, B = 0**

/// MPU configuration error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpuError {
    /// Region size is not a power of two (ARM MPU requirement: §B3.5 DDI0489F).
    SizeNotPowerOfTwo,
    /// Region size is zero.
    SizeZero,
    /// Base address is not aligned to the region size.
    ///
    /// ARM requires: `base_addr % size == 0`.
    AddressMisaligned,
    /// Region size is below the minimum 32-byte floor imposed by Cortex-M7 MPU.
    SizeTooSmall,
}

/// MPU memory attributes for a region.
///
/// These map to the TEX, S, C, B bit fields in the ARM MPU Region Attribute
/// and Size Register (RASR). See ARM DDI0489F §B3.5.4 for the encoding table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MpuAttributes {
    /// Strongly ordered — all accesses complete in program order, no buffering,
    /// no caching. Always shareable. Use for peripheral MMIO registers.
    ///
    /// TEX=000, S=1, C=0, B=0
    StronglyOrdered,

    /// Non-cacheable normal memory, suitable for CPU↔DMA shared buffers.
    ///
    /// TEX=001, S=0, C=0, B=0
    ///
    /// This is the correct attribute for DMA ping-pong buffers in AXI SRAM and
    /// SRAM4. Using `WriteBackNoWriteAllocate` or `WriteThrough` on DMA buffers
    /// causes silent data corruption because the D-cache line may hold stale data
    /// that is never written back before the DMA peripheral reads it, or the CPU
    /// reads a cached line that the DMA has updated behind the cache's back.
    NonCacheable,

    /// Write-back, no write-allocate — normal cached RAM.
    ///
    /// TEX=000, S=0, C=1, B=1
    ///
    /// Use for code/data that is exclusively CPU-accessed with no DMA sharing.
    WriteBackNoWriteAllocate,

    /// Write-through, no write-allocate — conservative caching policy.
    ///
    /// TEX=000, S=0, C=1, B=0
    ///
    /// Used for instruction-cache regions and read-mostly data. Writes always
    /// go to memory but reads are served from cache on a hit.
    WriteThrough,
}

/// A validated MPU region descriptor.
///
/// Construction via [`MpuRegion::new`] enforces the ARM Cortex-M7 MPU
/// alignment and size invariants at runtime so that callers cannot produce
/// an invalid hardware configuration.
#[derive(Debug, Clone, Copy)]
pub struct MpuRegion {
    base: u32,
    size: u32,
    attrs: MpuAttributes,
}

impl MpuRegion {
    /// Create a new MPU region, validating size and alignment.
    ///
    /// # Errors
    ///
    /// - [`MpuError::SizeZero`] if `size == 0`
    /// - [`MpuError::SizeTooSmall`] if `size < 32` (Cortex-M7 minimum)
    /// - [`MpuError::SizeNotPowerOfTwo`] if `size` is not a power of two
    /// - [`MpuError::AddressMisaligned`] if `base % size != 0`
    pub fn new(base: u32, size: u32, attrs: MpuAttributes) -> Result<Self, MpuError> {
        if size == 0 {
            return Err(MpuError::SizeZero);
        }
        if size < 32 {
            return Err(MpuError::SizeTooSmall);
        }
        if !size.is_power_of_two() {
            return Err(MpuError::SizeNotPowerOfTwo);
        }
        if !base.is_multiple_of(size) {
            return Err(MpuError::AddressMisaligned);
        }
        Ok(Self { base, size, attrs })
    }

    /// Encode the size as the ARM MPU `SIZE` field value (`log2(size) − 1`).
    ///
    /// The Cortex-M7 RASR register stores the region size as a 5-bit field
    /// where `SIZE = log2(size_in_bytes) − 1`. Because `size` must be a power
    /// of two, `log2(size)` is simply the number of trailing zero bits.
    ///
    /// Examples:
    /// - 32 B   = 2^5  → trailing_zeros = 5 → SIZE = 4
    /// - 64 KB  = 2^16 → trailing_zeros = 16 → SIZE = 15
    /// - 512 KB = 2^19 → trailing_zeros = 19 → SIZE = 18
    /// - 1 MB   = 2^20 → trailing_zeros = 20 → SIZE = 19
    ///
    /// # Errors
    ///
    /// - [`MpuError::SizeZero`] if `size == 0`
    /// - [`MpuError::SizeNotPowerOfTwo`] if `size` is not a power of two
    // trailing_zeros() returns u32; n fits in u8 for all valid MPU region sizes (≤ 2^31)
    #[allow(clippy::cast_possible_truncation)]
    pub fn encode_size(size: u32) -> Result<u8, MpuError> {
        if size == 0 {
            return Err(MpuError::SizeZero);
        }
        if !size.is_power_of_two() {
            return Err(MpuError::SizeNotPowerOfTwo);
        }
        // size = 2^n  →  trailing_zeros = n  →  SIZE field = n - 1
        let n = size.trailing_zeros();
        // n is at least 0; saturating_sub prevents underflow on size == 1
        // (size == 1 would give n=0, SIZE=u8::MAX — but size < 32 is caught by
        // `new`; `encode_size` can be called directly so we handle gracefully).
        Ok((n as u8).saturating_sub(1))
    }

    /// Base address of this region.
    #[must_use]
    pub fn base(&self) -> u32 {
        self.base
    }

    /// Size of this region in bytes.
    #[must_use]
    pub fn size(&self) -> u32 {
        self.size
    }

    /// Exclusive end address of this region (`base + size`).
    #[must_use]
    #[allow(clippy::arithmetic_side_effects)] // Safety: base + size fits u32 for valid MPU regions
    pub fn end(&self) -> u32 {
        self.base + self.size
    }

    /// Memory attributes assigned to this region.
    #[must_use]
    pub fn attrs(&self) -> MpuAttributes {
        self.attrs
    }

    /// Check whether this region overlaps with `other`.
    ///
    /// Two regions overlap when one's address range intersects the other's.
    /// Regions that share only a boundary point (end of one == start of other)
    /// do NOT overlap.
    #[must_use]
    pub fn overlaps(&self, other: &Self) -> bool {
        self.base < other.end() && other.base < self.end()
    }
}

/// DMA controller variants present on STM32H743.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaController {
    /// DMA1 — general-purpose, D1 domain. Cannot reach D3 (SRAM4).
    Dma1,
    /// DMA2 — general-purpose, D1 domain. Cannot reach D3 (SRAM4).
    Dma2,
    /// BDMA — basic DMA, D3 domain. Can only reach SRAM4 (0x3800_0000).
    ///
    /// BDMA-only peripherals: SPI6, I2C4, LPUART1, ADC3, SAI4, DFSDM2.
    Bdma,
}

/// Named DMA-accessible (or inaccessible) memory regions on STM32H743.
///
/// Use [`DmaRegion::is_dma_accessible`] to check compatibility with a given
/// [`DmaController`] before allocating DMA buffers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaRegion {
    /// DTCM — 128 KB at 0x2000_0000.
    ///
    /// Tightly-coupled to the Cortex-M7 core via a dedicated 64-bit port.
    /// The AXI bus matrix (and therefore any DMA controller) **cannot reach
    /// DTCM**. Placing DMA source/destination buffers here causes silent
    /// failures: the DMA sees stale memory while the CPU sees correct data
    /// through its local path.
    Dtcm,

    /// AXI SRAM (D1 domain) — 512 KB at 0x2400_0000.
    ///
    /// Primary pool for DMA buffers: SAI audio ping-pong, SPI display
    /// framebuffer, SDMMC sector buffers. DMA1 and DMA2 accessible.
    /// BDMA cannot reach this region.
    AxiSram,

    /// SRAM1/SRAM2 (D2 domain) — 256 KB at 0x3000_0000.
    ///
    /// Embassy task stacks, heapless collections. DMA1 and DMA2 accessible.
    Sram12,

    /// SRAM3 (D2 domain) — 32 KB at 0x3004_0000.
    ///
    /// USB buffers, small working sets. DMA1 and DMA2 accessible.
    /// Note: the hardware block is 32 KB; SRAM4 starts at 0x3800_0000.
    Sram34,

    /// SRAM4 (D3 domain) — 64 KB at 0x3800_0000.
    ///
    /// Accessible **only** by BDMA. Must be used for buffers belonging to
    /// BDMA-only peripherals: SPI6, I2C4, LPUART1, ADC3, SAI4, DFSDM2.
    Sram4,
}

impl DmaRegion {
    /// AXI SRAM region — DMA1/DMA2 accessible, 512 KB at 0x2400_0000.
    pub const AXI_SRAM: Self = Self::AxiSram;

    /// SRAM4 region — BDMA-only, 64 KB at 0x3800_0000.
    pub const SRAM4: Self = Self::Sram4;

    /// DTCM region — no DMA access at all, 128 KB at 0x2000_0000.
    pub const DTCM: Self = Self::Dtcm;

    /// Base address of this region.
    #[must_use]
    pub fn base(&self) -> u32 {
        match self {
            Self::Dtcm => 0x2000_0000,
            Self::AxiSram => 0x2400_0000,
            Self::Sram12 => 0x3000_0000,
            Self::Sram34 => 0x3004_0000,
            Self::Sram4 => 0x3800_0000,
        }
    }

    /// Size of this region in bytes.
    ///
    /// Note: `Sram34` reports 96 KB to reflect the combined SRAM3 (32 KB)
    /// and the adjacent SRAM4 within D2; however, for MPU regions the size
    /// must be rounded up to the next power of two (128 KB) when configuring
    /// hardware. See [`SoulAudioMpuConfig`] for pre-computed safe values.
    #[must_use]
    pub fn size(&self) -> u32 {
        match self {
            Self::Dtcm => 128 * 1024,
            Self::AxiSram => 512 * 1024,
            Self::Sram12 => 256 * 1024,
            Self::Sram34 => 96 * 1024, // Not a power of 2; round to 128 KB for MPU
            Self::Sram4 => 64 * 1024,
        }
    }

    /// Return `true` if this region is accessible by `ctrl` for DMA transfers.
    ///
    /// | Region  | DMA1 | DMA2 | BDMA |
    /// |---------|------|------|------|
    /// | DTCM    | NO   | NO   | NO   |
    /// | AXI SRAM| YES  | YES  | NO   |
    /// | SRAM1/2 | YES  | YES  | NO   |
    /// | SRAM3/4 | YES  | YES  | NO   |
    /// | SRAM4   | NO   | NO   | YES  |
    #[must_use]
    pub fn is_dma_accessible(&self, ctrl: DmaController) -> bool {
        match (self, ctrl) {
            // DTCM: no DMA controller can reach it
            (Self::Dtcm, _) => false,
            // SRAM4: only BDMA
            (Self::Sram4, DmaController::Bdma) => true,
            (Self::Sram4, _) => false,
            // All other regions: DMA1/DMA2 yes, BDMA no
            (_, DmaController::Bdma) => false,
            (_, DmaController::Dma1 | DmaController::Dma2) => true,
        }
    }
}

/// Pre-computed MPU region configurations for the SoulAudio DAP.
///
/// Apply these regions during hardware initialisation, **before** enabling any
/// DMA peripheral. Failure to do so allows the Cortex-M7 D-cache to serve
/// stale data for DMA buffers, producing silent data corruption in audio
/// output, display refresh, and SD card I/O.
///
/// # Usage Pattern
///
/// ```
/// # use platform::mpu::{SoulAudioMpuConfig, MpuRegion};
/// let axi_region = SoulAudioMpuConfig::axi_sram_dma_region();
/// let sram4_region = SoulAudioMpuConfig::sram4_bdma_region();
/// // Pass these to the hardware MPU driver (firmware crate) for programming.
/// ```
pub struct SoulAudioMpuConfig;

impl SoulAudioMpuConfig {
    /// AXI SRAM non-cacheable DMA region — 512 KB at 0x2400_0000.
    ///
    /// This region covers the primary DMA buffer pool used by:
    /// - SAI1/SAI2 audio ping-pong buffers (I²S to ES9038Q2M)
    /// - SPI DMA for display framebuffer transfers (SSD1677)
    /// - SDMMC1 sector buffers (microSD FAT32)
    ///
    /// Must be applied **before** any of the above peripherals are
    /// initialised. Without this, the D-cache produces silent data corruption
    /// on every DMA transfer.
    ///
    /// Attributes: `NonCacheable` (TEX=001, S=0, C=0, B=0)
    #[must_use]
    pub fn axi_sram_dma_region() -> MpuRegion {
        // Safety: 0x2400_0000 is a 512 KB-aligned address (512*1024 = 0x80000,
        // 0x2400_0000 % 0x80000 == 0). Parameters are statically correct.
        #[allow(clippy::expect_used)]
        MpuRegion::new(0x2400_0000, 512 * 1024, MpuAttributes::NonCacheable)
            .expect("AXI SRAM MPU region parameters are statically valid")
    }

    /// SRAM4 non-cacheable BDMA region — 64 KB at 0x3800_0000.
    ///
    /// This region must be non-cacheable for BDMA peripherals:
    /// SPI6, I2C4, LPUART1, ADC3, SAI4, DFSDM2.
    ///
    /// Buffers for these peripherals **must** reside in SRAM4; placing them
    /// anywhere else causes BDMA hard faults or silent failures.
    ///
    /// Attributes: `NonCacheable` (TEX=001, S=0, C=0, B=0)
    #[must_use]
    pub fn sram4_bdma_region() -> MpuRegion {
        // Safety: 0x3800_0000 is a 64 KB-aligned address (64*1024 = 0x10000,
        // 0x3800_0000 % 0x10000 == 0). Parameters are statically correct.
        #[allow(clippy::expect_used)]
        MpuRegion::new(0x3800_0000, 64 * 1024, MpuAttributes::NonCacheable)
            .expect("SRAM4 MPU region parameters are statically valid")
    }

    /// SRAM1+SRAM2 non-cacheable region — 256 KB at 0x3000_0000.
    ///
    /// SRAM1 and SRAM2 are in the D2 domain. DMA1 and DMA2 can access them
    /// via the AHB bus matrix. Embassy task stacks and heapless collections
    /// live here. Without an MPU non-cacheable region, the Cortex-M7 D-cache
    /// serves stale CPU data while DMA sees the uncached version — silent
    /// data corruption on any SDMMC descriptor table or similar DMA use.
    ///
    /// SRAM1: 128 KB at 0x3000_0000
    /// SRAM2: 128 KB at 0x3002_0000
    /// Combined: 256 KB, base 0x3000_0000, power-of-2 aligned.
    ///
    /// Reference: ST AN4839, STM32H743ZI DS12110 Rev 9 Table 4.
    ///
    /// Attributes:  (TEX=001, S=0, C=0, B=0)
    #[must_use]
    pub fn sram12_dma_region() -> MpuRegion {
        // Safety: 0x3000_0000 is a 256 KB-aligned address (256*1024 = 0x40000,
        // 0x3000_0000 % 0x40000 == 0). Parameters are statically correct.
        #[allow(clippy::expect_used)]
        MpuRegion::new(0x3000_0000, 256 * 1024, MpuAttributes::NonCacheable)
            .expect("SRAM1/2 MPU region parameters are statically valid")
    }
}

/// Pure register-value computation for the Cortex-M7 MPU.
///
/// All methods compute the RBAR and RASR u32 values needed to program
/// the ARMv7-M MPU registers. No `cortex_m` peripheral types are used here,
/// so this is fully host-testable.
///
/// # ARMv7-M MPU register encoding (ARM DDI0489F §B3.5)
///
/// ## RBAR layout
///
/// ```text
/// [31:5]  ADDR   — region base address (upper 27 bits, [4:0] must be zero)
/// [4]     VALID  — 1 = use REGION field to select the region; 0 = use MPU_RNR
/// [3:0]   REGION — hardware region slot number (0–15) when VALID=1
/// ```
///
/// ## RASR layout
///
/// ```text
/// [28]    XN    — Execute Never (1 = no instruction fetches)
/// [26:24] AP    — data Access Permission (0b011 = full privileged+user RW)
/// [21:19] TEX   — Type EXtension (001 = normal, non-cacheable)
/// [18]    S     — Shareable
/// [17]    C     — Cacheable
/// [16]    B     — Bufferable
/// [15:8]  SRD   — Subregion Disable (0 = all 8 subregions enabled)
/// [5:1]   SIZE  — log2(region_bytes) − 1
/// [0]     ENABLE — 1 = region active when MPU is enabled
/// ```
///
/// ## NonCacheable DMA region encoding
///
/// For TEX=001, S=0, C=0, B=0, AP=0b011, XN=1:
///
/// | Field | Bits  | Value | Hex contribution |
/// |-------|-------|-------|-----------------|
/// | XN    | [28]  | 1     | 0x1000_0000     |
/// | AP    | [26:24]| 0b011| 0x0300_0000     |
/// | TEX   | [21:19]| 0b001| 0x0008_0000     |
/// | S, C, B | [18:16] | 0 | 0x0000_0000   |
/// | SRD   | [15:8]| 0     | 0x0000_0000     |
///
/// Combined attribute mask (without SIZE and ENABLE): **0x1308_0000**
///
/// Per-region RASR values (SIZE field in bits [5:1], ENABLE=1 in bit [0]):
/// - 512 KB: SIZE = 18 = 0x12 → bits[5:1] = 0x24 → +ENABLE = 0x25 → RASR = **0x1308_0025**
/// - 64 KB:  SIZE = 15 = 0x0F → bits[5:1] = 0x1E → +ENABLE = 0x1F → RASR = **0x1308_001F**
pub struct MpuApplier;

impl MpuApplier {
    /// Attribute mask for NonCacheable DMA regions.
    ///
    /// Encodes TEX=001, S=0, C=0, B=0, AP=0b011, XN=1 per ARM DDI0489F §B3.5.4.
    /// Does **not** include the SIZE or ENABLE bits.
    pub const NON_CACHEABLE_ATTR_MASK: u32 = 0x1308_0000;

    /// Compute the RASR value for a NonCacheable DMA region.
    ///
    /// Combines [`NON_CACHEABLE_ATTR_MASK`] with the SIZE field and ENABLE bit.
    ///
    /// # Arguments
    ///
    /// * `size_field` — ARM MPU SIZE field = `log2(size_bytes) - 1`.
    ///   Use [`MpuRegion::encode_size`] to compute from a byte count.
    ///
    /// # Return
    ///
    /// RASR u32 with NonCacheable attributes, the given SIZE, and ENABLE=1.
    ///
    /// # Examples
    ///
    /// | Region   | Size   | SIZE field | RASR       |
    /// |----------|--------|------------|------------|
    /// | AXI SRAM | 512 KB | 18 (0x12)  | 0x1308_0025 |
    /// | SRAM4    |  64 KB | 15 (0x0F)  | 0x1308_001F |
    #[must_use]
    pub fn non_cacheable_rasr(size_field: u8) -> u32 {
        Self::NON_CACHEABLE_ATTR_MASK
            | (u32::from(size_field) << 1) // SIZE field occupies RASR bits [5:1]
            | 1 // ENABLE bit [0]
    }

    /// Compute the RBAR value for a region.
    ///
    /// Sets VALID=1 so the 4-bit REGION field selects the hardware slot,
    /// overriding the MPU_RNR register for this write.
    ///
    /// # Arguments
    ///
    /// * `base` — Base address (must be SIZE-aligned per ARM §B3.5.3).
    /// * `region_number` — Hardware MPU region slot 0–15.
    ///
    /// # Return
    ///
    /// RBAR u32: `base | (1 << 4) | (region_number & 0xF)`
    #[must_use]
    pub fn rbar(base: u32, region_number: u8) -> u32 {
        base | (1 << 4) | (u32::from(region_number) & 0xF)
    }

    /// Return `(RBAR, RASR)` pairs for the SoulAudio MPU configuration.
    ///
    /// | Index | Region   | Base        | Size   | Slot | RBAR        | RASR        |
    /// |-------|----------|-------------|--------|------|-------------|-------------|
    /// | 0     | AXI SRAM | 0x2400_0000 | 512 KB | 0    | 0x2400_0010 | 0x1308_0025 |
    /// | 1     | SRAM4    | 0x3800_0000 |  64 KB | 1    | 0x3800_0011 | 0x1308_001F |
    /// | 2     | SRAM1/2  | 0x3000_0000 | 256 KB | 2    | 0x3000_0012 | 0x1308_0023 |
    ///
    /// Apply pair 0 first, then pair 1. This ordering must be maintained
    /// so that the primary DMA pool (AXI SRAM) is configured before BDMA-only
    /// SRAM4 buffers are activated.
    ///
    /// # Safety (hardware boot context)
    ///
    /// This function is pure math — it only computes register values and does
    /// not touch hardware registers. The caller (firmware boot code) is
    /// responsible for writing these pairs to the actual MPU RBAR/RASR registers
    /// before enabling D-cache and before any DMA peripheral is initialised.
    #[must_use]
    pub fn soul_audio_register_pairs() -> [(u32, u32); 3] {
        let axi_region = SoulAudioMpuConfig::axi_sram_dma_region();
        let sram4_region = SoulAudioMpuConfig::sram4_bdma_region();
        let sram12_region = SoulAudioMpuConfig::sram12_dma_region();

        #[allow(clippy::expect_used)]
        let axi_size =
            MpuRegion::encode_size(axi_region.size()).expect("AXI SRAM size is statically valid");
        #[allow(clippy::expect_used)]
        let sram4_size =
            MpuRegion::encode_size(sram4_region.size()).expect("SRAM4 size is statically valid");
        #[allow(clippy::expect_used)]
        let sram12_size =
            MpuRegion::encode_size(sram12_region.size()).expect("SRAM1/2 size is statically valid");

        [
            (
                Self::rbar(axi_region.base(), 0),
                Self::non_cacheable_rasr(axi_size),
            ),
            (
                Self::rbar(sram4_region.base(), 1),
                Self::non_cacheable_rasr(sram4_size),
            ),
            (
                Self::rbar(sram12_region.base(), 2),
                Self::non_cacheable_rasr(sram12_size),
            ),
        ]
    }
}

// ─── Original tests (must stay passing) ──────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // ── Test A ────────────────────────────────────────────────────────────────

    #[test]
    fn test_region_size_must_be_power_of_two() {
        // Valid sizes (powers of two, >= 32)
        assert!(MpuRegion::new(0x2400_0000, 512 * 1024, MpuAttributes::NonCacheable).is_ok());
        assert!(MpuRegion::new(0x3000_0000, 256 * 1024, MpuAttributes::NonCacheable).is_ok());
        // Invalid sizes (not power of 2)
        assert!(MpuRegion::new(0x2400_0000, 100_000, MpuAttributes::NonCacheable).is_err());
        assert!(MpuRegion::new(0x2400_0000, 0, MpuAttributes::NonCacheable).is_err());
        assert!(MpuRegion::new(0x2400_0000, 3 * 1024, MpuAttributes::NonCacheable).is_err());
    }

    // ── Test B ────────────────────────────────────────────────────────────────

    #[test]
    fn test_region_address_must_be_aligned_to_size() {
        let size = 128 * 1024;
        // Aligned: 0x2400_0000 % (128*1024) == 0 → Ok
        assert!(MpuRegion::new(0x2400_0000, size, MpuAttributes::NonCacheable).is_ok());
        // Misaligned: 0x2400_1000 % (128*1024) != 0 → Err
        assert!(MpuRegion::new(0x2400_1000, size, MpuAttributes::NonCacheable).is_err());
    }

    // ── Test C ────────────────────────────────────────────────────────────────

    #[test]
    fn test_known_safe_dma_regions() {
        // AXI SRAM is DMA1/DMA2 accessible: 0x2400_0000, 512 KB
        let r = DmaRegion::AXI_SRAM;
        assert_eq!(r.base(), 0x2400_0000u32);
        assert_eq!(r.size(), 512 * 1024);
        assert!(r.is_dma_accessible(DmaController::Dma1));
        assert!(r.is_dma_accessible(DmaController::Dma2));
        assert!(!r.is_dma_accessible(DmaController::Bdma)); // BDMA cannot reach AXI SRAM

        // SRAM4 is only accessible to BDMA: 0x3800_0000, 64 KB
        let r = DmaRegion::SRAM4;
        assert!(r.is_dma_accessible(DmaController::Bdma));
        assert!(!r.is_dma_accessible(DmaController::Dma1));

        // DTCM: NO DMA access at all
        let r = DmaRegion::DTCM;
        assert!(!r.is_dma_accessible(DmaController::Dma1));
        assert!(!r.is_dma_accessible(DmaController::Dma2));
        assert!(!r.is_dma_accessible(DmaController::Bdma));
    }

    // ── Test D ────────────────────────────────────────────────────────────────

    #[test]
    fn test_regions_do_not_overlap() {
        // Non-overlapping: r1 = [0x2400_0000, 0x2404_0000), r2 = [0x2404_0000, 0x2408_0000)
        // They share a boundary point but do not intersect.
        let r1 = MpuRegion::new(0x2400_0000, 256 * 1024, MpuAttributes::NonCacheable).unwrap();
        let r2 = MpuRegion::new(0x2404_0000, 256 * 1024, MpuAttributes::NonCacheable).unwrap();
        assert!(!r1.overlaps(&r2));

        // Overlapping: r3 = [0x2400_0000, 0x2408_0000) (512 KB)
        //              r4 = [0x2404_0000, 0x2408_0000) (256 KB) — r4 is inside r3
        let r3 = MpuRegion::new(0x2400_0000, 512 * 1024, MpuAttributes::NonCacheable).unwrap();
        let r4 = MpuRegion::new(0x2404_0000, 256 * 1024, MpuAttributes::NonCacheable).unwrap();
        assert!(r3.overlaps(&r4)); // r4 is inside r3
    }

    // ── Test E ────────────────────────────────────────────────────────────────

    #[test]
    fn test_mpu_size_field_encoding() {
        // ARM SIZE field = log2(size) - 1
        // 32 B   = 2^5  → SIZE = 4
        // 64 KB  = 2^16 → SIZE = 15
        // 512 KB = 2^19 → SIZE = 18
        assert_eq!(MpuRegion::encode_size(32), Ok(4u8));
        assert_eq!(MpuRegion::encode_size(64 * 1024), Ok(15u8));
        assert_eq!(MpuRegion::encode_size(512 * 1024), Ok(18u8));
        assert_eq!(
            MpuRegion::encode_size(100_000),
            Err(MpuError::SizeNotPowerOfTwo)
        );
    }

    // ── Additional correctness checks ─────────────────────────────────────────

    #[test]
    fn test_encode_size_additional_values() {
        // 1 MB = 2^20 → SIZE = 19
        assert_eq!(MpuRegion::encode_size(1024 * 1024), Ok(19u8));
        // 128 KB = 2^17 → SIZE = 16
        assert_eq!(MpuRegion::encode_size(128 * 1024), Ok(16u8));
        // Zero is an error
        assert_eq!(MpuRegion::encode_size(0), Err(MpuError::SizeZero));
    }

    #[test]
    fn test_soul_audio_mpu_config_regions_are_valid() {
        let axi = SoulAudioMpuConfig::axi_sram_dma_region();
        assert_eq!(axi.base(), 0x2400_0000);
        assert_eq!(axi.size(), 512 * 1024);
        assert_eq!(axi.attrs(), MpuAttributes::NonCacheable);

        let sram4 = SoulAudioMpuConfig::sram4_bdma_region();
        assert_eq!(sram4.base(), 0x3800_0000);
        assert_eq!(sram4.size(), 64 * 1024);
        assert_eq!(sram4.attrs(), MpuAttributes::NonCacheable);
    }

    #[test]
    fn test_soul_audio_regions_do_not_overlap() {
        let axi = SoulAudioMpuConfig::axi_sram_dma_region();
        let sram4 = SoulAudioMpuConfig::sram4_bdma_region();
        assert!(!axi.overlaps(&sram4));
    }
}

// ─── MpuApplier tests ─────────────────────────────────────────────────────────
//
// Tests for register-value computation. A `MockMpu` records (RBAR, RASR) pairs
// in write order so tests can verify both values and sequencing without touching
// real hardware registers.

/// Tests for `MpuApplier` — pure register-value computation for ARMv7-M MPU.
#[cfg(test)]
#[allow(clippy::indexing_slicing)] // Tests index into known-length arrays
mod apply_tests {
    use super::*;

    /// A mock MPU that records (RBAR, RASR) pairs in write order.
    ///
    /// Mimics the hardware MPU register-write interface without touching
    /// physical registers. Ordered recording allows tests to assert both
    /// the values written and the write sequence.
    struct MockMpu {
        writes: Vec<(u32, u32)>,
    }

    impl MockMpu {
        fn new() -> Self {
            Self { writes: Vec::new() }
        }

        /// Record one (RBAR, RASR) pair, as real boot code would write to
        /// the hardware MPU_RBAR and MPU_RASR registers.
        fn write_region(&mut self, rbar: u32, rasr: u32) {
            self.writes.push((rbar, rasr));
        }

        /// Apply a full set of `(RBAR, RASR)` pairs in array order.
        fn apply_pairs(&mut self, pairs: &[(u32, u32)]) {
            for &(rbar, rasr) in pairs {
                self.write_region(rbar, rasr);
            }
        }
    }

    // ── Test 1 ────────────────────────────────────────────────────────────────

    /// SoulAudio MPU config must return exactly 2 region pairs.
    ///
    /// The design requires two non-cacheable regions:
    ///   Region 0 — AXI SRAM (DMA1/DMA2 pool, 512 KB)
    ///   Region 1 — SRAM4    (BDMA pool, 64 KB)
    #[test]
    fn test_mpu_region_count() {
        let pairs = MpuApplier::soul_audio_register_pairs();
        assert_eq!(
            pairs.len(),
            3,
            "SoulAudioMpuConfig must configure exactly 3 MPU regions (AXI SRAM + SRAM4 + SRAM1/2)"
        );
    }

    // ── Test 2 ────────────────────────────────────────────────────────────────

    /// AXI SRAM (pair index 0) RASR must encode NonCacheable attributes.
    ///
    /// Expected RASR = 0x1308_0025:
    ///   XN=1 (bit 28), AP=0b011 (bits 26:24), TEX=0b001 (bit 19), SIZE=18, ENABLE=1
    #[test]
    fn test_axi_sram_region_is_non_cacheable() {
        let pairs = MpuApplier::soul_audio_register_pairs();
        let (_rbar, rasr) = pairs[0];

        // TEX=001 → bit 19 must be set
        assert_ne!(
            rasr & (1 << 19),
            0,
            "TEX bit 19 must be set (TEX=001) for NonCacheable region"
        );
        // C (bit 17) must be clear
        assert_eq!(
            rasr & (1 << 17),
            0,
            "C (cacheable) bit 17 must be 0 for NonCacheable region"
        );
        // B (bit 16) must be clear
        assert_eq!(
            rasr & (1 << 16),
            0,
            "B (bufferable) bit 16 must be 0 for NonCacheable region"
        );

        // Full RASR: NON_CACHEABLE_ATTR_MASK | (SIZE=18 << 1) | ENABLE
        //   = 0x1308_0000 | 0x24 | 0x01 = 0x1308_0025
        assert_eq!(
            rasr, 0x1308_0025,
            "AXI SRAM RASR must be 0x1308_0025 (NonCacheable, 512 KB, ENABLE=1)"
        );
    }

    // ── Test 3 ────────────────────────────────────────────────────────────────

    /// SRAM4 (pair index 1) RASR must encode NonCacheable attributes.
    ///
    /// Expected RASR = 0x1308_001F:
    ///   XN=1 (bit 28), AP=0b011 (bits 26:24), TEX=0b001 (bit 19), SIZE=15, ENABLE=1
    #[test]
    fn test_sram4_region_is_non_cacheable() {
        let pairs = MpuApplier::soul_audio_register_pairs();
        let (_rbar, rasr) = pairs[1];

        // TEX=001 → bit 19 must be set
        assert_ne!(
            rasr & (1 << 19),
            0,
            "TEX bit 19 must be set (TEX=001) for NonCacheable region"
        );
        // C (bit 17) must be clear
        assert_eq!(
            rasr & (1 << 17),
            0,
            "C (cacheable) bit 17 must be 0 for NonCacheable region"
        );
        // B (bit 16) must be clear
        assert_eq!(
            rasr & (1 << 16),
            0,
            "B (bufferable) bit 16 must be 0 for NonCacheable region"
        );

        // Full RASR: NON_CACHEABLE_ATTR_MASK | (SIZE=15 << 1) | ENABLE
        //   = 0x1308_0000 | 0x1E | 0x01 = 0x1308_001F
        assert_eq!(
            rasr, 0x1308_001F,
            "SRAM4 RASR must be 0x1308_001F (NonCacheable, 64 KB, ENABLE=1)"
        );
    }

    // ── Test 4 ────────────────────────────────────────────────────────────────

    /// Regions must be applied in correct order: AXI SRAM (index 0) before SRAM4 (index 1).
    ///
    /// Boot sequence requirement: the primary DMA pool (AXI SRAM) is configured
    /// into hardware region slot 0, then BDMA-only SRAM4 into slot 1.
    /// `MockMpu` records write order so this can be asserted at test time.
    #[test]
    fn test_apply_config_sequence() {
        let mut mpu = MockMpu::new();
        let pairs = MpuApplier::soul_audio_register_pairs();
        mpu.apply_pairs(&pairs);

        assert_eq!(mpu.writes.len(), 3, "Expected exactly 3 MPU region writes");

        // First write: AXI SRAM — base 0x2400_0000, region slot 0
        // RBAR = 0x2400_0000 | VALID(bit4)=0x10 | REGION=0 = 0x2400_0010
        let (rbar0, rasr0) = mpu.writes[0];
        assert_eq!(
            rbar0, 0x2400_0010,
            "First write RBAR must be AXI SRAM at slot 0 (0x2400_0010)"
        );
        assert_eq!(
            rasr0, 0x1308_0025,
            "First write RASR must be AXI SRAM NonCacheable 512 KB (0x1308_0025)"
        );

        // Second write: SRAM4 — base 0x3800_0000, region slot 1
        // RBAR = 0x3800_0000 | VALID(bit4)=0x10 | REGION=1 = 0x3800_0011
        let (rbar1, rasr1) = mpu.writes[1];
        assert_eq!(
            rbar1, 0x3800_0011,
            "Second write RBAR must be SRAM4 at slot 1 (0x3800_0011)"
        );
        assert_eq!(
            rasr1, 0x1308_001F,
            "Second write RASR must be SRAM4 NonCacheable 64 KB (0x1308_001F)"
        );

        // Third write: SRAM1/2 at 0x3000_0000, region slot 2
        // RBAR = 0x3000_0000 | VALID(bit4)=0x10 | REGION=2 = 0x3000_0012
        // RASR: NON_CACHEABLE_ATTR_MASK | (SIZE=17 << 1) | ENABLE = 0x1308_0023
        let (rbar2, rasr2) = mpu.writes[2];
        assert_eq!(
            rbar2, 0x3000_0012,
            "Third write RBAR must be SRAM1/2 at slot 2 (0x3000_0012)"
        );
        assert_eq!(
            rasr2, 0x1308_0023,
            "Third write RASR must be SRAM1/2 NonCacheable 256 KB (0x1308_0023)"
        );
    }
}
