//! DMA buffer alignment for Cortex-M7 cache coherency.
//!
//! The STM32H743ZI Cortex-M7 has a 32-byte cacheline. Any buffer accessed by
//! a DMA peripheral must be aligned to at least 32 bytes to prevent cache
//! coherency bugs where the CPU cache and DMA controller disagree on memory
//! state.
//!
//! # The Problem
//!
//! When D-cache is enabled (embassy-stm32 enables it during init), the CPU
//! may cache DMA buffer contents. If DMA writes to memory while the CPU has
//! a stale cache line, the CPU will read the old (pre-DMA) data. Conversely,
//! if the CPU writes to a buffer before DMA reads it, those writes may sit
//! in cache and never reach RAM before DMA starts.
//!
//! # The Solution
//!
//! Either:
//! 1. Place buffers in non-cacheable SRAM (`.axisram` section is configured
//!    as non-cacheable by the MPU in `firmware::boot::hardware`) — preferred.
//! 2. Use cache maintenance operations (SCB::clean_dcache_by_address, etc.)
//!    before/after every DMA transfer — complex and error-prone.
//!
//! The `Align32` wrapper enforces proper alignment. Combined with placement
//! in `.axisram` via `#[link_section = ".axisram"]`, this fully prevents
//! cache coherency issues.
//!
//! # References
//! - ST AN4839: Level 1 cache on STM32F7 Series and STM32H7 Series
//! - ST AN4838: MPU programming model for STM32
//! - ARM DDI0489F §B3.5: Cache coherency

/// A `#[repr(align(32))]` wrapper that enforces 32-byte alignment for
/// Cortex-M7 DMA-accessible buffers.
///
/// All static buffers accessed by DMA peripherals (SAI audio, SPI display,
/// SDMMC storage) must use this wrapper to guarantee that cache operations
/// do not corrupt DMA data.
///
/// # Example
///
/// ```ignore
/// use firmware::dma::Align32;
///
/// #[link_section = ".axisram"]
/// #[allow(dead_code)]
/// static mut SAI_DMA_BUF: Align32<[u8; 8192]> = Align32([0u8; 8192]);
/// ```
#[derive(Clone, Copy)]
#[repr(align(32))]
pub struct Align32<T>(
    /// The inner value. Must be public so callers can construct and destructure the wrapper.
    pub T,
);
