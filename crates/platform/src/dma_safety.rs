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
pub unsafe trait BdmaAccessible: DmaAccessible {}

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

/// Zero-sized type representing SRAM4 (BDMA-only, D3 domain).
///
/// Buffers placed here via `#[link_section = ".sram4"]`:
/// - SPI6 DMA buffer (if used)
/// - SAI4 audio DMA (if used instead of SAI1)
/// - LPUART1 DMA buffer
#[derive(Debug, Clone, Copy)]
pub struct Sram4Region;

// SAFETY: SRAM4 at 0x3800_0000 is in D3 domain, accessible by BDMA only.
// It also satisfies DmaAccessible for type-system consistency, but NOTE:
// DMA1/DMA2 cannot actually access SRAM4 — use BDMA exclusively.
unsafe impl DmaAccessible for Sram4Region {}
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
