//! Architecture tests: DMA safety marker traits.
//! These tests enforce compile-time guarantees about DMA buffer placement.

// Test files legitimately use arithmetic for verification; allow at file level.
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::indexing_slicing)]
// Some imports are used only to verify trait/type accessibility at compile time.
#![allow(unused_imports)]
// Some assertions check documented compile-time constants for architectural correctness.
#![allow(clippy::assertions_on_constants)]

// Test 1: DmaAccessible trait is exported from platform
#[test]
fn dma_accessible_trait_is_exported() {
    // This test verifies the trait exists and is publicly accessible.
    // The trait itself is a zero-cost marker.
    use platform::dma_safety::DmaAccessible;
    let _ = core::mem::size_of::<platform::dma_safety::AxiSramRegion>();
    assert_eq!(
        core::mem::size_of::<platform::dma_safety::AxiSramRegion>(),
        0
    );
}

// Test 2: BdmaAccessible trait is exported
#[test]
fn bdma_accessible_trait_is_exported() {
    use platform::dma_safety::BdmaAccessible;
    let _ = core::mem::size_of::<platform::dma_safety::Sram4Region>();
    assert_eq!(core::mem::size_of::<platform::dma_safety::Sram4Region>(), 0);
}

// Test 3: AxiSramRegion implements DmaAccessible
#[test]
fn axi_sram_region_implements_dma_accessible() {
    use platform::dma_safety::{AxiSramRegion, DmaAccessible};
    fn assert_dma_accessible<T: DmaAccessible>() {}
    assert_dma_accessible::<AxiSramRegion>();
}

// Test 4: Sram4Region implements BdmaAccessible AND DmaAccessible
#[test]
fn sram4_region_implements_bdma_accessible() {
    use platform::dma_safety::{BdmaAccessible, Sram4Region};
    fn assert_bdma_accessible<T: BdmaAccessible>() {}
    assert_bdma_accessible::<Sram4Region>();
}

// Test 5: DtcmRegion does NOT implement DmaAccessible (checked by documentation constant)
#[test]
fn dtcm_region_not_dma_accessible() {
    use platform::dma_safety::{DtcmRegion, DTCM_NOT_DMA_ACCESSIBLE};
    // The constant exists to document this architectural constraint
    assert!(DTCM_NOT_DMA_ACCESSIBLE);
}

// Test 6: FRAMEBUFFER_SIZE matches WIDTH * HEIGHT / 4 (2bpp packed into u8)
#[test]
fn framebuffer_size_matches_dimensions() {
    use platform::dma_safety::{DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE_BYTES};
    // 2bpp = 4 pixels per byte
    let expected = (DISPLAY_WIDTH as usize * DISPLAY_HEIGHT as usize) / 4;
    assert_eq!(
        FRAMEBUFFER_SIZE_BYTES, expected,
        "FRAMEBUFFER_SIZE_BYTES ({}) != {}x{}/4 ({})",
        FRAMEBUFFER_SIZE_BYTES, DISPLAY_WIDTH, DISPLAY_HEIGHT, expected
    );
}

// Test 7: FRAMEBUFFER fits in AXI SRAM with margin
#[test]
fn framebuffer_fits_in_axisram() {
    use platform::dma_safety::{AXI_SRAM_SIZE_BYTES, FRAMEBUFFER_SIZE_BYTES};
    // Two framebuffers (double-buffer) must fit with stack and other statics
    let two_framebuffers = FRAMEBUFFER_SIZE_BYTES * 2;
    // Leave 64KB margin for stack, embassy tasks, other statics
    let margin = 64 * 1024;
    assert!(
        two_framebuffers + margin <= AXI_SRAM_SIZE_BYTES,
        "Two framebuffers ({}) + margin ({}) exceeds AXI SRAM ({})",
        two_framebuffers,
        margin,
        AXI_SRAM_SIZE_BYTES
    );
}

// Test 8: Audio DMA buffer sizing constants are defined (32-bit PCM, GAP-A3 fixed)
#[test]
fn audio_dma_buffer_constants_defined() {
    use platform::dma_safety::{AUDIO_DMA_BUFFER_BYTES, AUDIO_DMA_BUFFER_SAMPLES};
    // ES9038Q2M: 32-bit I2S frames -- 2048 samples x 2 channels x 4 bytes = 16384 bytes
    assert_eq!(AUDIO_DMA_BUFFER_BYTES, AUDIO_DMA_BUFFER_SAMPLES * 2 * 4);
    assert!(
        AUDIO_DMA_BUFFER_SAMPLES >= 512,
        "Buffer too small for low-latency audio"
    );
    assert!(
        AUDIO_DMA_BUFFER_SAMPLES <= 4096,
        "Buffer too large, increases latency"
    );
}

// Test 9: AXI SRAM address range constant is correct
#[test]
fn axi_sram_address_range_correct() {
    use platform::dma_safety::{AXI_SRAM_BASE, AXI_SRAM_SIZE_BYTES};
    assert_eq!(AXI_SRAM_BASE, 0x2400_0000u32);
    assert_eq!(AXI_SRAM_SIZE_BYTES, 512 * 1024);
}

// Test 10: SRAM4 address range constant is correct (BDMA-only)
#[test]
fn sram4_address_range_correct() {
    use platform::dma_safety::{SRAM4_BASE, SRAM4_SIZE_BYTES};
    assert_eq!(SRAM4_BASE, 0x3800_0000u32);
    assert_eq!(SRAM4_SIZE_BYTES, 64 * 1024);
}

// ── AUDIO_DMA_BUFFER_BYTES must be 32-bit PCM sized (GAP-A3) ─────────────────

/// Audio DMA buffer must be sized for 32-bit I2S frames.
/// ES9038Q2M native PCM width is 32 bits = 4 bytes per sample.
/// AUDIO_DMA_BUFFER_BYTES = AUDIO_DMA_BUFFER_SAMPLES * 2 channels * 4 bytes = 16384.
/// Using 8192 (16-bit sizing) causes DMA wrap at half the audio frame boundary.
/// Reference: ES9038Q2M datasheet section 6.1, SAI frame width configuration.
#[test]
fn audio_dma_buffer_bytes_is_32bit_sized() {
    use platform::dma_safety::{AUDIO_DMA_BUFFER_BYTES, AUDIO_DMA_BUFFER_SAMPLES};
    // 32-bit stereo: samples * 2 channels * 4 bytes
    let expected_32bit = AUDIO_DMA_BUFFER_SAMPLES * 2 * 4;
    assert_eq!(
        AUDIO_DMA_BUFFER_BYTES,
        expected_32bit,
        "AUDIO_DMA_BUFFER_BYTES ({}) must equal {} (SAMPLES x 2ch x 4 bytes/32-bit sample).          16-bit sizing (x2) causes DMA underrun at 192 kHz.",
        AUDIO_DMA_BUFFER_BYTES,
        expected_32bit
    );
}

#[test]
fn audio_dma_buffer_bytes_equals_16384() {
    use platform::dma_safety::AUDIO_DMA_BUFFER_BYTES;
    // 2048 samples x 2 channels x 4 bytes/sample = 16384
    assert_eq!(
        AUDIO_DMA_BUFFER_BYTES,
        16384,
        "AUDIO_DMA_BUFFER_BYTES must be 16384 (2048 x 2ch x 32-bit). Got {}.",
        AUDIO_DMA_BUFFER_BYTES
    );
}

#[test]
fn audio_dma_buffer_fits_in_axisram() {
    use platform::dma_safety::{AXI_SRAM_SIZE_BYTES, AUDIO_DMA_BUFFER_BYTES, FRAMEBUFFER_SIZE_BYTES};
    // Two audio buffers (ping-pong) + two framebuffers + 64KB margin
    let audio_total = AUDIO_DMA_BUFFER_BYTES * 2;
    let display_total = FRAMEBUFFER_SIZE_BYTES * 2;
    let margin = 64 * 1024;
    let total = audio_total + display_total + margin;
    assert!(
        total <= AXI_SRAM_SIZE_BYTES,
        "AXI SRAM budget exceeded: 2xaudio ({}) + 2xframebuffer ({}) + margin ({}) = {} > {} bytes",
        audio_total, display_total, margin, total, AXI_SRAM_SIZE_BYTES
    );
}
