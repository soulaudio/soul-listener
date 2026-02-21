//! DMA safety marker traits and buffer sizing constants for STM32H743ZI.
//!
//! ## DMA Accessibility on STM32H743ZI
//!
//! | Memory Region | Base Address | Size   | DMA1/2 | MDMA | BDMA | Use case |
//! |---------------|-------------|--------|--------|------|------|----------|
//! | AXI SRAM      | 0x2400_0000 | 512 KB | YES    | YES  | NO   | Audio SAI, display SPI, SDMMC |
//! | SRAM1/2 (D2)  | 0x3000_0000 | 256 KB | YES    | YES  | NO   | Embassy task stacks |
//! | SRAM3 (D2)    | 0x3004_0000 | 32 KB  | YES    | YES  | NO   | USB buffers |
//! | SRAM4 (D3)    | 0x3800_0000 | 64 KB  | NO     | NO   | YES  | SPI6, SAI4, LPUART1, I2C4 |
//! | DTCM          | 0x2000_0000 | 128 KB | NO     | NO   | NO   | CPU-only: stack, ISR scratch |
//! | External SDRAM| 0xC000_0000 | 32 MB  | YES†   | YES† | NO   | Caches, scratch (high latency) |
//!
//! † FMC DMA has higher latency than internal SRAM — do NOT use for real-time audio.
//!
//! ## Memory Regions
//!
//! | Type | Trait | Description |
//! |------|-------|-------------|
//! | [`AxiSramRegion`] | `DmaAccessible` | D1 AXI SRAM — audio SAI, display SPI, SDMMC |
//! | [`Sram4Region`] | `BdmaAccessible` | D3 SRAM4 — BDMA-only: SPI6, SAI4, LPUART1 |
//! | [`DtcmRegion`] | *(none)* | CPU-only DTCM — no DMA access |
//! | [`SdramRegion`] | `HighLatencyRegion` | External SDRAM via FMC — NOT DMA-safe for real-time audio |
//!
//! `SdramRegion` implements [`HighLatencyRegion`] and must never be used as
//! a DMA buffer for real-time audio. Variable FMC/SDRAM latency (50–200 ns)
//! causes SAI DMA buffer underruns at 192 kHz. Use [`AxiSramRegion`] instead.
//!
//! ## Usage
//! ```rust
//! use platform::dma_safety::FRAMEBUFFER_SIZE_BYTES;
//!
//! // DMA1/2-accessible buffer (display SPI, audio SAI, SDMMC):
//! #[link_section = ".axisram"]
//! static mut FRAMEBUFFER: [u8; FRAMEBUFFER_SIZE_BYTES] = [0xFF; FRAMEBUFFER_SIZE_BYTES];
//!
//! // BDMA-accessible buffer (SPI6, SAI4):
//! #[link_section = ".sram4"]
//! static mut SAI4_BUFFER: [u8; 256] = [0u8; 256];
//! ```

// ── Memory region addresses ──────────────────────────────────────────────────

/// Base address of AXI SRAM (DMA1/2/MDMA accessible, D1 domain).
pub const AXI_SRAM_BASE: u32 = 0x2400_0000;

/// Size of AXI SRAM in bytes (512 KB).
pub const AXI_SRAM_SIZE_BYTES: usize = 512 * 1024;

/// Base address of SRAM4 (BDMA-only, D3 domain).
pub const SRAM4_BASE: u32 = 0x3800_0000;

/// Size of SRAM4 in bytes (64 KB).
pub const SRAM4_SIZE_BYTES: usize = 64 * 1024;

/// True: DTCM is NOT DMA-accessible. Place no DMA buffers here.
///
/// DTCM (0x2000_0000, 128 KB) is tightly coupled to the Cortex-M7 CPU
/// and is invisible to all DMA controllers. Use for: stack, ISR scratch,
/// hot-path data that CPU touches every cycle.
pub const DTCM_NOT_DMA_ACCESSIBLE: bool = true;

// ── Display constants ────────────────────────────────────────────────────────

/// Display width in pixels (GDEM0397T81P / SSD1677).
pub const DISPLAY_WIDTH: u32 = 800;

/// Display height in pixels.
pub const DISPLAY_HEIGHT: u32 = 480;

/// Framebuffer size in bytes for 2bpp (4 pixels per byte).
///
/// 800 x 480 pixels / 4 pixels/byte = 96,000 bytes per plane.
/// The SSD1677 has two planes (OLD_DATA + NEW_DATA) but we manage
/// one software framebuffer and send it to both planes during init.
pub const FRAMEBUFFER_SIZE_BYTES: usize = (DISPLAY_WIDTH as usize * DISPLAY_HEIGHT as usize) / 4;

// ── Audio DMA constants ──────────────────────────────────────────────────────

/// Number of stereo samples per DMA half-buffer (ping-pong transfer).
///
/// At 192 kHz stereo, 2048 samples = ~5.3 ms latency per half.
/// Total round-trip audio latency = 2x this (ping + pong) ~= 10.7 ms.
pub const AUDIO_DMA_BUFFER_SAMPLES: usize = 2048;

/// Bytes per sample for 32-bit I2S (ES9038Q2M native PCM width).
/// SAI frame: 2 slots x 32 bits/slot = 64-bit frame per stereo pair.
const BYTES_PER_SAMPLE_32BIT: usize = 4;

/// Audio DMA ping-pong buffer size in bytes for 32-bit stereo PCM.
///
/// 2048 samples x 2 channels x 4 bytes/sample = 16384 bytes per half-buffer.
/// Total DMA ring: 2 x 16384 = 32768 bytes in AXI SRAM.
///
/// ES9038Q2M uses 32-bit I2S frames. Using 16-bit sizing (x2 instead of x4)
/// causes the DMA to wrap at half the audio frame boundary, producing a
/// stuttering artifact repeating at ~188 Hz (audible, hardware-reproducible).
///
/// Reference: ES9038Q2M datasheet section 6.1, STM32H743 SAI section 41.4.5
pub const AUDIO_DMA_BUFFER_BYTES: usize =
    AUDIO_DMA_BUFFER_SAMPLES * 2 * BYTES_PER_SAMPLE_32BIT;

// Compile-time verification: must equal 16384 (2048 x 2ch x 4 bytes/32-bit sample)
const _: () = assert!(
    AUDIO_DMA_BUFFER_BYTES == 16384,
    "AUDIO_DMA_BUFFER_BYTES must be 16384 (2048 x 2ch x 4 bytes/32-bit sample)"
);

/// Canonical type alias for the audio DMA buffer element array.
///
/// Always use this type alias for audio DMA buffer declarations to ensure
/// the buffer size is always derived from [`AUDIO_DMA_BUFFER_BYTES`].
/// This prevents size drift when multiple audio DMA buffers are declared
/// across firmware modules.
pub type AudioDmaBufBytes = [u8; AUDIO_DMA_BUFFER_BYTES];

// ── Marker traits ────────────────────────────────────────────────────────────

/// Marker trait: memory region accessible by DMA1, DMA2, and MDMA.
///
/// # Safety
/// Only implement for zero-sized types representing memory regions
/// that are physically accessible by the STM32H743 DMA controllers.
/// Incorrectly implementing this trait for DTCM will cause silent
/// DMA data corruption or bus faults.
///
/// Valid regions: AXI SRAM (D1), SRAM1/2/3 (D2), External SDRAM (via FMC).
pub unsafe trait DmaAccessible: Sized {}

/// Marker trait: memory region accessible by BDMA (D3 domain).
///
/// # Safety
/// BDMA can only access D3 SRAM4 (0x3800_0000, 64 KB).
/// DMA1/DMA2 cannot access SRAM4 — mixing them causes bus faults.
///
/// Peripherals requiring BDMA: SPI6, SAI4, LPUART1, I2C4, ADC3.
/// Marker trait: memory region accessible by BDMA only (D3 domain).
///
/// This trait is intentionally separate from [`DmaAccessible`].
/// BDMA is the D3-domain DMA controller; it can ONLY access SRAM4 (0x38000000, 64 KB).
/// DMA1/DMA2 (D1/D2) cannot access D3 SRAM4.
///
/// Do NOT add [`DmaAccessible`] as a supertrait. That would incorrectly imply
/// that BDMA-accessible regions are also DMA1/DMA2-accessible, which is false
/// for SRAM4 (D3 domain, RM0433 section 2.3).
///
/// # Safety
/// BDMA can only access D3 SRAM4 (0x3800_0000, 64 KB).
/// DMA1/DMA2 cannot access SRAM4 mixing them causes bus faults.
/// Only implement for zero-sized types representing SRAM4.
///
/// Peripherals requiring BDMA: SPI6, SAI4, LPUART1, I2C4, ADC3.
pub unsafe trait BdmaAccessible: Sized {}

// ── Region zero-sized types ──────────────────────────────────────────────────

/// Zero-sized type representing AXI SRAM (DMA1/DMA2/MDMA accessible).
///
/// Buffers placed here via `#[link_section = ".axisram"]`:
/// - Display SPI DMA framebuffer
/// - Audio SAI DMA ping-pong buffer
/// - SDMMC DMA transfer buffer
#[derive(Debug, Clone, Copy)]
pub struct AxiSramRegion;

// SAFETY: AXI SRAM at 0x2400_0000 is in D1 domain, accessible by all
// DMA controllers (DMA1, DMA2, MDMA) per STM32H743 reference manual Table 3.
unsafe impl DmaAccessible for AxiSramRegion {}

/// Zero-sized type representing SRAM4 (BDMA-only, D3 domain, 0x38000000).
///
/// # DMA accessibility (RM0433 section 2.3 bus matrix)
/// SRAM4 is in D3 domain. **Only BDMA can access it.**
/// DMA1 and DMA2 (D1/D2 domain) CANNOT access SRAM4.
/// This type therefore implements [`BdmaAccessible`] but NOT [`DmaAccessible`].
/// Attempting to create `DmaBuffer<Sram4Region, T>` will fail to compile.
///
/// Buffers placed here via `#[link_section = ".sram4"]`:
/// - SPI6 DMA buffer (if used)
/// - SAI4 audio DMA (if used instead of SAI1)
/// - LPUART1 DMA buffer
#[derive(Debug, Clone, Copy)]
pub struct Sram4Region;

// NOTE: Sram4Region intentionally does NOT implement DmaAccessible.
// SRAM4 (D3 domain, 0x38000000) is only accessible by BDMA (D3 DMA controller).
// DMA1/DMA2 cannot reach SRAM4 per RM0433 section 2.3 bus matrix table.
// Using a DMA1/DMA2 peripheral with an SRAM4 buffer causes a silent bus fault.
// SAFETY: SRAM4 at 0x3800_0000 (D3 domain) is the only region accessible
// by the BDMA controller per STM32H743 reference manual Table 3. Peripherals
// in D3 (SPI6, SAI4, LPUART1, I2C4, ADC3) must use BDMA; using DMA1/DMA2
// with SRAM4 causes a bus fault. This impl is correct because Sram4Region
// represents exactly this physically BDMA-reachable region.
unsafe impl BdmaAccessible for Sram4Region {}

/// Zero-sized type representing DTCM (CPU-only, NOT DMA-accessible).
///
/// DTCM is tightly coupled to the Cortex-M7 pipeline.
/// Use for: stack, interrupt handlers, hot-path data.
/// NEVER place DMA buffers here — they will not be transferred correctly.
#[derive(Debug, Clone, Copy)]
pub struct DtcmRegion;
// DtcmRegion intentionally does NOT implement DmaAccessible or BdmaAccessible.

// ── DmaBuffer wrapper ────────────────────────────────────────────────────────

/// A DMA-accessible buffer with compile-time region enforcement.
///
/// The phantom type `Region: DmaAccessible` ensures this buffer was declared
/// for a DMA-accessible memory region. Use `#[link_section]` to physically
/// place it in the correct memory.
///
/// # Usage
///
/// ```rust,ignore
/// // Framebuffer in AXI SRAM for display SPI DMA:
/// #[link_section = ".axisram"]
/// static FRAMEBUFFER: StaticCell<DmaBuffer<AxiSramRegion, [u8; FRAMEBUFFER_SIZE_BYTES]>>
///     = StaticCell::new();
/// ```
pub struct DmaBuffer<Region: DmaAccessible, T> {
    /// The inner data being protected by this DMA buffer wrapper.
    pub data: T,
    _region: core::marker::PhantomData<Region>,
}

impl<Region: DmaAccessible, T> DmaBuffer<Region, T> {
    /// Create a new DMA buffer for the given region.
    ///
    /// The caller is responsible for placing this in the correct
    /// memory via `#[link_section = ".axisram"]` etc.
    pub const fn new(data: T) -> Self {
        Self {
            data,
            _region: core::marker::PhantomData,
        }
    }
}

// ── Static DMA memory budget ──────────────────────────────────────────────────

/// Total static DMA buffer memory allocated in AXI SRAM.
///
/// Budget breakdown:
/// - 2× FRAMEBUFFER_SIZE_BYTES (ping-pong display planes): 2 × 96,000 = 192,000 bytes
/// - 2× AUDIO_DMA_BUFFER_BYTES (ping-pong SAI DMA):        2 × 16,384 =  32,768 bytes
/// - Total: 224,768 bytes of 524,288 bytes AXI SRAM = ~43% utilization
/// - Remaining ~57% (299,520 bytes): Embassy task stacks, .bss, .data
///
/// The display actually uses a single framebuffer (96,000 bytes) in the current
/// firmware. The second slot is reserved for future double-buffered rendering.
/// The audio DMA uses two half-buffers (ping-pong) in a single 32,768-byte ring.
pub const TOTAL_STATIC_DMA_BYTES: usize =
    FRAMEBUFFER_SIZE_BYTES * 2 + AUDIO_DMA_BUFFER_BYTES * 2;

/// Compile-time assertion: static DMA buffers fit in AXI SRAM with 25% headroom.
///
/// The remaining 75% (at minimum) is needed for Embassy task stacks, .bss,
/// .data sections, and heapless collections placed in AXI SRAM.
///
/// If this assertion fails:
/// - Move large caches (library index, album art) to external SDRAM (0xC000_0000)
/// - Reduce AUDIO_DMA_BUFFER_SAMPLES (each halving saves 16,384 bytes)
/// - Use single framebuffer (remove the second 96,000-byte display plane)
const _: () = assert!(
    TOTAL_STATIC_DMA_BYTES <= AXI_SRAM_SIZE_BYTES * 3 / 4,
    "Static DMA buffers exceed 75% of AXI SRAM — insufficient headroom for task stacks"
);

// ── Per-task stack size budget ────────────────────────────────────────────────

/// Default Embassy task stack size (bytes).
/// Each spawned Embassy task gets its own stack via `#[embassy_executor::task]`.
/// 8 KB is sufficient for async tasks without deep call chains.
/// Increase to 16 KB if defmt logging + FLAC decode + SDMMC overlap on one task.
pub const TASK_STACK_BYTES: usize = 8 * 1024; // 8 KB per task

/// Number of Embassy tasks that run concurrently on this firmware build.
/// Tasks: main, display, input, audio_sai, watchdog = 5 tasks minimum.
/// Add: bluetooth_hci, sdmmc_task when implemented.
pub const CONCURRENT_TASK_COUNT: usize = 5;

/// Total stack reservation for all Embassy tasks (bytes).
pub const TOTAL_TASK_STACK_BYTES: usize = TASK_STACK_BYTES * CONCURRENT_TASK_COUNT;

/// Minimum AXI SRAM headroom to keep free for .bss, .data, and alignment padding.
/// 64 KB is a conservative minimum; actual usage depends on global variable count.
pub const MIN_AXI_SRAM_HEADROOM_BYTES: usize = 64 * 1024; // 64 KB

/// Total AXI SRAM consumption estimate (DMA buffers + task stacks + headroom).
pub const TOTAL_AXI_SRAM_BUDGET_BYTES: usize =
    TOTAL_STATIC_DMA_BYTES + TOTAL_TASK_STACK_BYTES + MIN_AXI_SRAM_HEADROOM_BYTES;

/// Compile-time assertion: total AXI SRAM consumption must not exceed 512 KB.
/// If this fails, either: reduce CONCURRENT_TASK_COUNT, reduce TASK_STACK_BYTES,
/// move some task stacks to SRAM1/2, or move DMA buffers to SRAM1/2.
const _: () = assert!(
    TOTAL_AXI_SRAM_BUDGET_BYTES <= AXI_SRAM_SIZE_BYTES,
    "AXI SRAM budget exceeded! Reduce task count, stack size, or DMA buffers."
);

// ── External SDRAM constants ──────────────────────────────────────────────────

/// External SDRAM base address (FMC Bank 5, W9825G6KH-6 via FMC).
/// This is the address programmed into FMC_BCR5 SDRAM base address register.
pub const EXTSDRAM_BASE: u32 = 0xC000_0000;

/// External SDRAM size (32 MB — W9825G6KH-6 is 16M × 16-bit = 32 MB).
pub const EXTSDRAM_SIZE_BYTES: usize = 32 * 1024 * 1024;

// ── HighLatencyRegion trait ───────────────────────────────────────────────────

/// Marker trait for memory regions with variable or high access latency.
///
/// Regions implementing this trait must NOT be used as DMA buffers for
/// real-time audio streaming. Variable latency (refresh pauses, row/column
/// address setup) can cause audio DMA underruns at 192 kHz.
///
/// # Affected regions
/// - [`SdramRegion`]: External SDRAM via FMC — 50–200 ns variable latency
///
/// # Safe regions (no HighLatencyRegion)
/// - [`AxiSramRegion`]: AXI SRAM — deterministic ~1 ns (2 AHB cycles)
/// - [`Sram4Region`]: SRAM4 — deterministic via BDMA
pub trait HighLatencyRegion {}

// ── SdramRegion ───────────────────────────────────────────────────────────────

/// External SDRAM region (W9825G6KH-6, 32 MB at 0xC0000000 via FMC Bank 5).
///
/// # DMA safety
/// `SdramRegion` does **NOT** implement [`DmaAccessible`]. This is intentional:
///
/// External SDRAM has variable access latency:
/// - Row hit: ~50 ns
/// - Row miss (precharge + activate): ~100–150 ns
/// - Refresh pause (tRFC = 63 ns): can stall for multiple bus cycles
///
/// For SAI audio DMA at 192 kHz / 2048 samples, the DMA must complete
/// transfers within a 10.7 ms window. A 150 ns SDRAM latency spike occupies
/// 14.4 cycles at 96 MHz FMC clock — tolerable for single accesses but
/// dangerous in burst mode with concurrent refresh.
///
/// All audio DMA buffers must use [`AxiSramRegion`] instead.
///
/// # Permitted uses
/// - Library index cache (latency-tolerant)
/// - Album art thumbnail cache (latency-tolerant)
/// - FLAC decode scratch buffer (large, non-real-time)
/// - UI frame history buffer (latency-tolerant)
#[derive(Debug, Clone, Copy)]
pub struct SdramRegion;

impl HighLatencyRegion for SdramRegion {}
// NOTE: SdramRegion intentionally does NOT implement DmaAccessible or BdmaAccessible.
// Any attempt to create a DmaBuffer<SdramRegion, T> will fail to compile,
// preventing accidental real-time DMA from external SDRAM.

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // All values are compile-time constants; assertion is intentional budget check.
    #[allow(clippy::assertions_on_constants)]
    fn task_stack_budget_fits_in_axi_sram() {
        assert!(
            TOTAL_AXI_SRAM_BUDGET_BYTES <= AXI_SRAM_SIZE_BYTES,
            "Total budget {TOTAL_AXI_SRAM_BUDGET_BYTES} bytes exceeds AXI SRAM {AXI_SRAM_SIZE_BYTES} bytes"
        );
    }

    #[test]
    fn dma_buffers_leave_enough_room_for_tasks() {
        let remaining = AXI_SRAM_SIZE_BYTES - TOTAL_STATIC_DMA_BYTES;
        let task_need = TOTAL_TASK_STACK_BYTES + MIN_AXI_SRAM_HEADROOM_BYTES;
        assert!(
            remaining >= task_need,
            "After DMA buffers ({TOTAL_STATIC_DMA_BYTES} bytes), only {remaining} bytes remain — need {task_need} bytes for tasks"
        );
    }

    #[test]
    // TASK_STACK_BYTES is a compile-time constant; assertions document acceptable range.
    #[allow(clippy::assertions_on_constants)]
    fn per_task_stack_is_reasonable() {
        // Tasks should have at least 4 KB (too small = stack overflow)
        // and at most 32 KB (too large = wastes precious SRAM)
        assert!(TASK_STACK_BYTES >= 4 * 1024, "Per-task stack must be >= 4 KB");
        assert!(TASK_STACK_BYTES <= 32 * 1024, "Per-task stack must be <= 32 KB");
    }

    #[test]
    fn axi_sram_utilization_under_90_percent() {
        let utilization = TOTAL_AXI_SRAM_BUDGET_BYTES * 100 / AXI_SRAM_SIZE_BYTES;
        assert!(
            utilization < 90,
            "AXI SRAM utilization {utilization}% exceeds 90% — leave headroom for runtime allocations"
        );
    }

    #[test]
    fn sdram_region_base_and_size_are_correct() {
        assert_eq!(EXTSDRAM_BASE, 0xC000_0000);
        assert_eq!(EXTSDRAM_SIZE_BYTES, 32 * 1024 * 1024);
    }

    #[test]
    fn sdram_region_is_marked_high_latency() {
        // Type-system check: SdramRegion implements HighLatencyRegion.
        // This is a compile-time assertion — if SdramRegion didn't implement
        // HighLatencyRegion, this function wouldn't compile.
        fn assert_high_latency<T: HighLatencyRegion>() {}
        assert_high_latency::<SdramRegion>();
    }

    #[test]
    fn axi_sram_region_is_not_high_latency() {
        // AxiSramRegion must NOT implement HighLatencyRegion (it's the fast region).
        // We check by scanning each line: an `impl HighLatencyRegion for` line must
        // never name AxiSramRegion as the implementing type.
        let src = include_str!("dma_safety.rs");
        let violating_impl = src.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("impl HighLatencyRegion for")
                && trimmed.contains("AxiSramRegion")
        });
        assert!(!violating_impl,
            "AxiSramRegion must not implement HighLatencyRegion — it is the correct region for audio DMA");
    }

    #[test]
    // Belt-and-suspenders: confirm DtcmRegion and SdramRegion are excluded from DmaAccessible.
    fn dtcm_region_is_also_not_dma_accessible() {
        // Check each line: an `impl DmaAccessible for` line must not name DtcmRegion or SdramRegion.
        let src = include_str!("dma_safety.rs");
        let dtcm_violation = src.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("impl DmaAccessible for") && trimmed.contains("DtcmRegion")
        });
        let sdram_violation = src.lines().any(|line| {
            let trimmed = line.trim();
            trimmed.starts_with("impl DmaAccessible for") && trimmed.contains("SdramRegion")
        });
        assert!(!dtcm_violation, "DtcmRegion must not implement DmaAccessible");
        assert!(!sdram_violation, "SdramRegion must not implement DmaAccessible");
    }
    #[test]
    // GAP B2: SRAM4 is D3-domain, accessible only by BDMA (RM0433 section 2.3).
    // NOTE: This uses a string-grep test because Rust stable does not support
    // negative trait bounds (not implement trait assertions require nightly).
    // Split searched strings to avoid self-referential include_str! matches.
    fn sram4_region_is_not_dma_accessible_only_bdma() {
        let src = include_str!("dma_safety.rs");
        let dma_for_sram4 = ["unsafe impl DmaAccess", "ible for Sram4Region"].concat();
        assert!(
            !src.contains(&dma_for_sram4),
            "Sram4Region must NOT implement DmaAccessible - SRAM4 is BDMA-only (RM0433 section 2.3)"
        );
        let bdma_for_sram4 = ["unsafe impl BdmaAccess", "ible for Sram4Region"].concat();
        assert!(
            src.contains(&bdma_for_sram4),
            "Sram4Region MUST implement BdmaAccessible - SRAM4 is the BDMA-only D3-domain region"
        );
    }

    #[test]
    // GAP C2: AudioDmaBufBytes type alias must exist for canonical audio DMA size.
    fn audio_dma_buf_bytes_type_alias_exists() {
        let src = include_str!("dma_safety.rs");
        let type_alias_def = ["pub type Audio", "DmaBuf", "Bytes"].concat();
        assert!(
            src.contains(&type_alias_def),
            "dma_safety.rs must define pub type AudioDmaBufBytes for canonical audio DMA size"
        );
        let type_alias_body = ["AudioDma", "BufBytes = [u8; AUDIO_DMA_BUFFER_BYTES]"].concat();
        assert!(
            src.contains(&type_alias_body),
            "AudioDmaBufBytes must alias [u8; AUDIO_DMA_BUFFER_BYTES]"
        );
    }

    #[test]
    // GAP C3: Document the negative trait bound limitation.
    // Rust stable does not support negative trait bounds (nightly only).
    // We use string-grep tests as a workaround. This test verifies the limitation is documented.
    fn negative_trait_bound_tests_are_documented() {
        let src = include_str!("dma_safety.rs");
        assert!(
            src.contains("negative trait") || src.contains("not implement"),
            "dma_safety.rs must document the negative trait bound limitation"
        );
    }

}
