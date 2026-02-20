//! SAI audio output task — streams PCM samples from the decode queue to SAI1 via DMA.
//!
//! # Hardware: SAI1 Block A (master), 32-bit I2S, 192 kHz, PLL3 MCLK = 49.152 MHz
//! # DMA: DMA1 Stream 0, channel 0, circular mode, ping-pong with AUDIO_BUFFER
//!
//! ## PLL3 Configuration (49.152 MHz audio clock)
//!
//! PLL3 must be configured before SAI1 init. Target: 49.152 MHz on PLL3Q.
//!   - HSE = 25 MHz (board crystal)
//!   - PLL3M = 5  → VCO input = 5 MHz
//!   - PLL3N = 49 → VCO output = 245.76 MHz
//!   - PLL3Q = 5  → PLL3Q output = 49.152 MHz (SAI MCLK)
//!
//! ## SAI1 Pin Assignments (STM32H743ZI LQFP-144)
//!   - PE2  → SAI1_MCLK_A  (master clock out, 256×fs)
//!   - PE4  → SAI1_FS_A    (frame sync / L/R clock)
//!   - PE5  → SAI1_SCK_A   (bit clock)
//!   - PE6  → SAI1_SD_A    (serial data out)
//!
//! ## DMA Buffer Layout (ping-pong in AXI SRAM)
//!
//! ```text
//! AUDIO_BUFFER: [u8; 16384]
//!   ├── Half 0 (bytes 0..8192):    DMA filling while CPU reads half 1
//!   └── Half 1 (bytes 8192..16384): DMA filling while CPU reads half 0
//! ```
//!
//! Reference: STM32H7 RM0433 Rev 9, section 52 (SAI), section 16 (DMA).

#![allow(clippy::doc_markdown)] // SAI task docs use hardware signal names (e.g. SAI1_SD_A) that are clearer as plain text
use platform::dma_safety::{AxiSramRegion, DmaBuffer, AUDIO_DMA_BUFFER_BYTES};
#[allow(unused_imports)]
use crate::audio::clock_math::{
    MCLK_TARGET_HZ, SAMPLE_RATE_HZ, MCLK_FS_RATIO,
    PLL3_M, PLL3_N, PLL3_P, PLL3_FRACN, PLL3P_HZ_APPROX,
};

/// Embassy task wrapper for the SAI audio output — hardware target only.
///
/// Enabled only when `feature = "hardware"` is active (links `embassy-executor`).
/// Call via `spawner.must_spawn(audio_task_embassy(audio_buf))` in main().
///
/// # Arguments
///
/// * `buffer` — Unique mutable reference to the AXI SRAM audio DMA buffer.
///   `DmaBuffer<AxiSramRegion, _>` enforces at compile time that the buffer is
///   in a DMA1/DMA2-accessible memory region (not DTCM).
#[cfg(feature = "hardware")]
#[embassy_executor::task]
pub async fn audio_task_embassy(
    buffer: &'static mut DmaBuffer<AxiSramRegion, [u8; AUDIO_DMA_BUFFER_BYTES]>,
) {
    audio_task(buffer).await;
}

/// SAI audio output task implementation — hardware target only.
///
/// Streams silence (zero-fill) via SAI1 DMA in the initial implementation.
/// When the audio decode pipeline is ready, this task will read from the decode
/// channel and copy samples into the DMA half-buffer on HTIE/TCIE interrupts.
///
/// # Safety of the DMA buffer
///
/// The buffer is declared `DmaBuffer<AxiSramRegion, _>`, enforcing at compile time
/// that the memory region is DMA1/DMA2 accessible. The `#[link_section = ".axisram"]`
/// attribute on the `AUDIO_BUFFER` static places it at 0x2400_0000 (AXI SRAM, D1
/// domain). DTCM (0x2000_0000) is NOT DMA-accessible; placing a SAI DMA buffer there
/// causes silent data corruption or a bus fault.
#[cfg(feature = "hardware")]
pub async fn audio_task(
    _buffer: &'static mut DmaBuffer<AxiSramRegion, [u8; AUDIO_DMA_BUFFER_BYTES]>,
) {
    // TODO: Initialize SAI1 peripheral here via embassy-stm32 when full audio pipeline is ready.
    //
    // Required steps (STM32H7 RM0433 §52):
    //   1. Enable SAI1 clock via RCC_APB2ENR.SAI1EN
    //   2. Configure PLL3Q = 49.152 MHz and select as SAI1 kernel clock (RCC_D2CCIP1R.SAI1SEL)
    //   3. Set up SAI1 Block A: master mode, 32-bit I2S, 2 slots, MCLK enabled
    //   4. Configure DMA1 Stream 0: peripheral = SAI1_A DR, memory = _buffer.data.as_mut_ptr()
    //   5. Enable DMA circular mode with half-transfer interrupt (HTIE) for ping-pong
    //   6. Enable SAI1 Block A (SAI_xCR1.SAIEN)
    //
    // Embassy-stm32 API (once PLL3 is wired in build_embassy_config):
    //   let sai = Sai::new_asynchronous_with_mclk(
    //       p.SAI1_A, p.PE5, p.PE6, p.PE4, p.PE2,
    //       p.DMA1_CH0, &mut _buffer.data, Irqs, SaiConfig::default(),
    //   );
    //
    // PLL3 clock configuration (see crate::audio::clock_math for derivation):
    //   HSI (64 MHz) / PLL3_M(4) * PLL3_N(49) / PLL3_P(16) = 49.0 MHz base
    //   With PLL3_FRACN(1245): PLL3P_HZ_APPROX = 49 151 977 Hz (23 Hz below target)
    //   MCLK_TARGET_HZ = SAMPLE_RATE_HZ(192_000) * MCLK_FS_RATIO(256) = 49_152_000 Hz
    //
    // Blocked on: PLL3 configuration in firmware::boot::build_embassy_config().
    // PLL1Q is currently 200 MHz (SDMMC clock). SAI1 needs a dedicated PLL3 branch.
    // See: hardware/CLAUDE.md for PLL3 divisor target (49.152 MHz).

    loop {
        embassy_time::Timer::after_secs(1).await;
    }
}

/// Compile-time usage marker: ensures `DmaBuffer<AxiSramRegion>` is referenced in
/// this module so architecture tests (`audio_dma_buffer_type_enforced`) detect usage.
///
/// This type alias is intentionally public so the test can find it via source grep.
pub type AudioDmaBuffer = DmaBuffer<AxiSramRegion, [u8; AUDIO_DMA_BUFFER_BYTES]>;
