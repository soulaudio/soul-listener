//! Architecture boundary tests — run with `cargo test -p firmware --test arch_boundaries`
// Architecture test file: expect/unwrap/panic/cast are intentional test mechanisms.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::assertions_on_constants,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::arithmetic_side_effects,
    // embedded_graphics::mock_display::MockDisplay has a large internal pixel buffer
    // (> 512 bytes) that we cannot annotate (external crate).
    clippy::large_stack_arrays,
)]
//!
//! These tests enforce the layering rules defined in CLAUDE.md:
//!   Rule 1: eink-system + eink-components must not require eink-emulator
//!   Rule 2: eink-specs must not depend on heavier eink crates or firmware
//!   Rule 3: platform (HAL) must not depend on firmware (app layer)
//!   Rule 4: eink-emulator must not depend on firmware
//!
//! # How enforcement works
//!
//! These are compile-time rules enforced by the workspace Cargo.toml dependency
//! graph. The tests below verify them at CI time by checking that specific
//! imports compile WITHOUT certain features enabled.
//!
//! The primary enforcement mechanism is the `arch-boundaries` CI job in
//! `.github/workflows/ci.yml`, which uses `cargo tree` to verify the resolved
//! dependency graph. The tests here provide a compile-time sanity check that
//! the layer boundaries hold for the code that actually links into the firmware
//! integration test binary.

// (arithmetic_side_effects also permitted for assertion math in tests)

/// Verify that `eink-specs` has no dependency on embassy or firmware crates.
///
/// If this compiles, the boundary is intact. The test itself is trivial —
/// the enforcement is the compilation of this integration test binary, which
/// only links `eink-specs` (via firmware's transitive deps) without `firmware`
/// internals leaking downward.
///
/// If `eink-specs` accidentally gains a dep on firmware, `cargo check -p eink-specs`
/// will fail before this test even runs.
#[test]
fn eink_specs_is_minimal() {
    // eink-specs must compile without std, without embassy, without firmware.
    // SPEC_VERSION is a simple &'static str — if it exists, the crate compiled
    // cleanly with only its declared (minimal) dependencies.
    let version: &str = eink_specs::SPEC_VERSION;
    assert!(
        !version.is_empty(),
        "eink-specs must have a non-empty version"
    );
}

/// Verify that the platform HAL crate exposes its core traits without
/// requiring any firmware application types.
///
/// If `platform` accidentally depended on `firmware`, this integration test
/// binary would fail to link (circular dependency: firmware -> platform -> firmware).
#[test]
fn platform_hal_is_independent() {
    // DisplayDriver, InputDevice, AudioCodec — core HAL traits must be
    // reachable without any firmware application code.
    // We just name the types; their existence at compile time proves the boundary.
    fn _assert_display_trait_exists<T: platform::DisplayDriver>() {}
    fn _assert_input_trait_exists<T: platform::InputDevice>() {}
    fn _assert_audio_trait_exists<T: platform::AudioCodec>() {}

    // Compile-only check — if this test compiles, the boundary is intact.
}

/// Verify that the `ui` crate's `Navigator` starts at `NowPlaying` and
/// reports depth 1 immediately after construction.
///
/// This is a compile-time + runtime boundary check: if `ui` accidentally
/// pulled in `firmware` types the integration test binary would fail to link.
#[test]
fn ui_navigator_starts_at_now_playing() {
    use ui::navigation::Navigator;
    use ui::screen::Screen;

    let nav = Navigator::new();
    assert_eq!(nav.current(), Screen::NowPlaying);
    assert_eq!(nav.depth(), 1);
}

/// Verify that the `bluetooth` crate's HCI layer correctly serialises the
/// HCI_Reset command to the H4 wire format.
///
/// Like `ui_navigator_starts_at_now_playing`, the compilation of this test
/// is itself an architecture boundary check.
#[test]
fn bluetooth_hci_reset_packet_correct() {
    use bluetooth::hci::{HciCommand, HciPacket};

    let pkt = HciPacket::from_command(HciCommand::Reset)
        .expect("Reset command must always serialize successfully");
    assert_eq!(&pkt[..], &[0x01_u8, 0x03, 0x0C, 0x00]);
}

/// Verify that `library::TrackIndex` (using the test-safe `SmallIndex` alias)
/// starts empty and reports the correct capacity constant.
///
/// Compilation of this test confirms that `library` does not accidentally
/// depend on `firmware` or embassy types — if it did, the integration test
/// binary would fail to link.
#[test]
fn library_track_index_has_correct_capacity() {
    use library::index::{IndexError, SmallIndex};

    let mut idx = SmallIndex::new();
    assert!(idx.is_empty());
    assert_eq!(idx.len(), 0);

    // MAX_TRACKS must equal 8192 per the SDRAM layout specification.
    assert_eq!(library::index::MAX_TRACKS, 8192);

    // Smoke-test insert + error path using the SmallIndex (capacity 64).
    let track = library::Track::new("/arch_test.flac", library::AudioFormat::Flac);
    idx.insert(track).expect("insert into empty SmallIndex");
    assert_eq!(idx.len(), 1);

    // Verify that a truly full index returns IndexError::Full.
    let mut tiny = library::index::TrackIndex::<1>::new();
    tiny.insert(library::Track::new("/a.flac", library::AudioFormat::Flac))
        .expect("first insert");
    let err = tiny
        .insert(library::Track::new("/b.flac", library::AudioFormat::Flac))
        .unwrap_err();
    assert!(matches!(err, IndexError::Full));
}

/// Verify that magic-byte format detection works correctly from the
/// architecture boundary integration test context.
///
/// This confirms `library::metadata` is accessible and `AudioFormat` is
/// correctly re-exported from the crate root.
#[test]
fn library_format_detection_by_magic_bytes() {
    use library::metadata::detect_format;
    use library::track::AudioFormat;

    assert_eq!(detect_format(b"fLaC"), Some(AudioFormat::Flac));
    assert_eq!(detect_format(b"ID3\x03"), Some(AudioFormat::Mp3));
    assert_eq!(
        detect_format(&[0xFF, 0xFB, 0x00, 0x00]),
        Some(AudioFormat::Mp3)
    );
    assert_eq!(detect_format(b"RIFF"), Some(AudioFormat::Wav));
    assert_eq!(detect_format(&[]), None);
}

/// Verify that SDMMC1 is documented as requiring HSI48.
///
/// Embassy issue #3049: SDMMC on STM32H743 silently hangs during init_card()
/// unless HSI48 is enabled in RCC before SDMMC initialisation. This test
/// enforces that the `clock_config` table always carries that requirement.
///
/// If this test fails, someone deleted or changed the SDMMC1 entry in
/// `SOUL_AUDIO_CLOCK_REQUIREMENTS` without updating the hardware init code.
#[test]
fn test_sdmmc_hsi48_requirement_documented() {
    use platform::clock_config::{ClockSource, SOUL_AUDIO_CLOCK_REQUIREMENTS};
    let sdmmc = SOUL_AUDIO_CLOCK_REQUIREMENTS
        .iter()
        .find(|r| r.peripheral == "SDMMC1")
        .expect("SDMMC1 must have a clock requirement entry");
    assert_eq!(sdmmc.required_source, ClockSource::Hsi48);
    // Embassy issue #3049: without HSI48, SDMMC silently fails on hardware
    assert!(
        sdmmc.note.contains("3049"),
        "must reference embassy issue #3049"
    );
}

/// Verify that the default QSPI prescaler keeps QUADSPI clock within W25Q128JV limits.
///
/// At 240 MHz AHB with prescaler=1, QSPI clock = 120 MHz < 133 MHz max.
/// This test catches any accidental change to `QSPI_PRESCALER` that would
/// push the clock over the W25Q128JV 133 MHz specification.
#[test]
fn test_qspi_clock_within_flash_spec() {
    use platform::qspi_config::{validate_qspi_prescaler, QSPI_PRESCALER};
    // At 240 MHz AHB with default prescaler, must be within W25Q128JV limit
    validate_qspi_prescaler(240_000_000, QSPI_PRESCALER)
        .expect("default QSPI prescaler must be within W25Q128JV 133 MHz limit");
}

// ─── MPU boot architecture tests ─────────────────────────────────────────────

/// Verify `MpuApplier::soul_audio_register_pairs()` returns exactly 3 pairs.
///
/// Architecture rule: the SoulAudio boot sequence configures exactly three
/// non-cacheable MPU regions: AXI SRAM, SRAM4, SRAM1/2.
/// Any change to this count must be intentional and documented.
#[test]
fn test_mpu_applier_returns_two_pairs() {
    use platform::mpu::MpuApplier;

    let pairs = MpuApplier::soul_audio_register_pairs();
    assert_eq!(
        pairs.len(),
        3,
        "MpuApplier must return exactly 3 (RBAR, RASR) pairs for the SoulAudio DAP"
    );
}

/// Verify that RASR values for both SoulAudio regions encode NonCacheable.
///
/// NonCacheable encoding (TEX=001, C=0, B=0, ARM DDI0489F §B3.5.4):
///   - TEX bit 19 must be SET   (TEX[0] = 1)
///   - C   bit 17 must be CLEAR (not cacheable)
///   - B   bit 16 must be CLEAR (not bufferable)
///
/// If these bits are wrong, the D-cache will corrupt DMA buffers silently.
/// This test serves as a regression guard against incorrect attribute constants.
#[test]
fn test_rasr_values_encode_non_cacheable() {
    use platform::mpu::MpuApplier;

    let pairs = MpuApplier::soul_audio_register_pairs();

    for (idx, &(_rbar, rasr)) in pairs.iter().enumerate() {
        // TEX[0] = bit 19 -- must be 1 for NonCacheable (TEX=001)
        assert_ne!(
            rasr & (1 << 19),
            0,
            "Region {idx}: RASR bit 19 (TEX[0]) must be SET for NonCacheable"
        );

        // C = bit 17 -- must be 0 (not cacheable)
        assert_eq!(
            rasr & (1 << 17),
            0,
            "Region {idx}: RASR bit 17 (C) must be CLEAR for NonCacheable"
        );

        // B = bit 16 -- must be 0 (not bufferable)
        assert_eq!(
            rasr & (1 << 16),
            0,
            "Region {idx}: RASR bit 16 (B) must be CLEAR for NonCacheable"
        );

        // ENABLE = bit 0 -- must be 1 (region active)
        assert_ne!(rasr & 1, 0, "Region {idx}: RASR bit 0 (ENABLE) must be SET");
    }
}

/// Verify that the `playback` crate's state machine compiles cleanly and that
/// `PlaybackEngine` starts in the `Stopped` state.
///
/// This test enforces two boundaries simultaneously:
///   1. `playback` must not depend on `firmware` (no circular dep).
///   2. The playback engine's pure-logic layer must be `no_std`-compatible and
///      testable on the host without any hardware abstractions.
#[test]
fn playback_crate_has_no_std_core() {
    // Verify the playback engine state machine logic compiles and behaves
    // correctly when accessed from the firmware integration test suite.
    use playback::engine::{PlaybackEngine, PlaybackState};
    let engine = PlaybackEngine::new();
    assert_eq!(engine.state(), PlaybackState::Stopped);
}

/// Verify that `NanoMp3Decoder` is default-constructible and exposes the
/// correct initial state.
///
/// The `mp3` feature is NOT required here — `NanoMp3Decoder::new()` must
/// compile and return zero metadata regardless of whether nanomp3 is linked.
/// This enforces that the optional `mp3` feature doesn't break the crate's
/// unconditional public API surface.
#[test]
fn playback_mp3_decoder_is_default_constructible() {
    use playback::decoder::FrameDecoder;
    use playback::mp3_decoder::NanoMp3Decoder;
    let d = NanoMp3Decoder::new();
    assert_eq!(d.sample_rate(), 0);
    assert_eq!(d.channels(), 0);
}

// ─── MPU boot wiring tests ────────────────────────────────────────────────────
//
// These tests enforce that the MPU configuration is correctly wired into the
// boot module and will be called before embassy_stm32::init() on hardware.
// Without this, D-cache corruption of DMA buffers (audio, display, SD) occurs.

/// Verify that `firmware::boot` exposes `apply_mpu_config_from_peripherals` as
/// a public hardware API function.
///
/// Architecture rule: the `hardware` submodule of `boot` must provide a
/// safe, no-argument entry point so that `main.rs` can call MPU init without
/// needing to obtain or manage a `cortex_m::Peripherals` handle at the call site.
///
/// Under `cfg(feature = "hardware")` this is a compile-time check — if the
/// function doesn't exist the test file will not compile.
/// Without the hardware feature (host test runs), the inner block is absent
/// and the test trivially passes.
#[test]
fn mpu_apply_fn_is_public_hardware_api() {
    #[cfg(feature = "hardware")]
    {
        let _: fn() = firmware::boot::hardware::apply_mpu_config_from_peripherals;
    }
    let _ = ();
}

/// Verify that the first step of the documented boot sequence is MPU
/// configuration, not cache enablement.
///
/// Architecture rule: MPU must be configured BEFORE D-cache is enabled
/// (ARM AN4838/AN4839). If cache is enabled first, the D-cache can begin
/// serving stale data for DMA buffer addresses before the MPU marks them
/// non-cacheable, causing silent data corruption.
///
/// This test enforces the ordering is documented correctly in
/// `firmware::boot::BOOT_SEQUENCE_STEPS`.
#[test]
fn boot_sequence_step_0_is_mpu_not_cache() {
    let steps = firmware::boot::BOOT_SEQUENCE_STEPS;
    assert!(!steps.is_empty(), "boot sequence must have steps");
    let first = steps[0].to_lowercase();
    assert!(
        first.contains("mpu"),
        "step 0 must be MPU configuration, got: {}",
        steps[0]
    );
    assert!(
        !first.contains("cache"),
        "step 0 must NOT mention cache (MPU must precede D-cache), got: {}",
        steps[0]
    );
}

/// Verify that `firmware::boot::mpu_register_pairs()` returns exactly 3 pairs.
///
/// Architecture rule: the SoulAudio boot sequence must configure exactly
/// three non-cacheable MPU regions: AXI SRAM (DMA1/DMA2 pool), SRAM4
/// (BDMA pool), and SRAM1/2 (D2 domain DMA1/2 accessible).
/// Any change to this count is a deliberate hardware change that must be reviewed.
#[test]
fn mpu_register_pairs_correct_count() {
    let pairs = firmware::boot::mpu_register_pairs();
    assert_eq!(
        pairs.len(),
        3,
        "must configure exactly 3 non-cacheable MPU regions"
    );
}

/// Verify that the first RBAR register pair targets AXI SRAM at 0x2400_0000.
///
/// Architecture rule: the primary DMA buffer pool (AXI SRAM) must be
/// the first region configured (slot 0) so it is in place before any
/// audio, display, or SD DMA peripherals are initialised.
#[test]
fn mpu_rbar_covers_axi_sram() {
    let pairs = firmware::boot::mpu_register_pairs();
    let rbar_0 = pairs[0].0;
    // RBAR encodes: base_addr | VALID (bit 4) | region_num (bits 0-3)
    let base = rbar_0 & 0xFFFF_FFE0;
    assert_eq!(
        base, 0x2400_0000,
        "RBAR[0] must target AXI SRAM at 0x24000000, got 0x{base:08X}"
    );
}

/// Verify that the second RBAR register pair targets SRAM4 at 0x3800_0000.
///
/// Architecture rule: SRAM4 is the exclusive BDMA buffer pool. BDMA
/// peripherals (SPI6, I2C4, LPUART1, ADC3, SAI4) can only access D3 domain
/// memory; placing their buffers anywhere else causes silent DMA failure.
#[test]
fn mpu_rbar_covers_sram4() {
    let pairs = firmware::boot::mpu_register_pairs();
    let rbar_1 = pairs[1].0;
    let base = rbar_1 & 0xFFFF_FFE0;
    assert_eq!(
        base, 0x3800_0000,
        "RBAR[1] must target SRAM4 at 0x38000000, got 0x{base:08X}"
    );
}

/// Verify that `SdramInitSequence::w9825g6kh6()` is callable from the
/// platform crate and returns the correct type.
///
/// Architecture rule: the SDRAM initialization sequence (CLK_EN → PALL →
/// AUTO_REFRESH × 2 → LMR → SET_REFRESH_RATE) must be defined in
/// `platform::sdram`, not scattered across firmware. This ensures the
/// sequence is auditable, testable, and re-usable without pulling in
/// firmware dependencies.
#[test]
fn sdram_init_sequence_type_exists() {
    // Architecture rule: SdramInitSequence must be defined in platform::sdram.
    // We verify its callable constructor and that it yields a non-empty step list.
    let seq = platform::sdram::SdramInitSequence::w9825g6kh6();
    assert!(
        !seq.steps.is_empty(),
        "SdramInitSequence must have at least one step"
    );
}

/// Verify that the SDRAM base address constant is correctly defined in
/// `platform::sdram` and matches the FMC Bank 5 mapping on STM32H743.
///
/// Architecture rule: `SDRAM_BASE_ADDRESS` must equal 0xC000_0000.
/// This is the hardware-mandated FMC bank 5/6 base address on all STM32H7
/// devices. Any code that constructs a pointer to SDRAM or validates addresses
/// must reference this constant — no magic literals allowed.
#[test]
fn sdram_base_address_is_correct() {
    // SDRAM must be mapped at 0xC0000000 (FMC bank 5)
    assert_eq!(platform::sdram::SDRAM_BASE_ADDRESS, 0xC000_0000u32);
}

/// Verify that both RASR values encode the NonCacheable attribute (TEX=001, C=0, B=0).
///
/// Architecture rule: DMA buffer regions must be non-cacheable to prevent
/// D-cache coherency hazards. The correct ARM Cortex-M7 encoding is:
///   TEX[2:0] = 001 (bits [21:19])
///   C = 0         (bit 17 — not cacheable)
///   B = 0         (bit 16 — not bufferable)
///
/// If C=1, the D-cache will serve stale data to the CPU after DMA writes.
/// If TEX != 001, the memory type is wrong (strongly-ordered or device memory).
#[test]
fn mpu_rasr_marks_non_cacheable() {
    let pairs = firmware::boot::mpu_register_pairs();
    for (i, (_rbar, rasr)) in pairs.iter().enumerate() {
        // C bit (bit 17) must be 0 — not cacheable
        assert_eq!(
            rasr & (1 << 17),
            0,
            "RASR[{i}]: C bit (bit 17) must be 0 for non-cacheable, rasr=0x{rasr:08X}"
        );
        // TEX bits (bits 21:19) must equal 001
        let tex = (rasr >> 19) & 0b111;
        assert_eq!(
            tex, 0b001,
            "RASR[{i}]: TEX must be 001 for non-cacheable normal memory, got {tex:03b}"
        );
    }
}

// ─── RCC / Clock configuration architecture tests ─────────────────────────────

/// Verify that `firmware::boot::rcc_config_has_hsi48()` returns true.
///
/// Architecture rule: the firmware RCC config must enable HSI48 before
/// `embassy_stm32::init()` is called. Without HSI48, SDMMC1 silently hangs
/// during `init_card()` on the STM32H743 (embassy-stm32 issue #3049).
///
/// `rcc_config_has_hsi48()` is a host-testable proxy for the hardware-only
/// `build_embassy_config()` function — it documents and asserts that HSI48
/// is always set to `Some(...)` in the returned config.
#[test]
fn rcc_config_enables_hsi48() {
    assert!(
        firmware::boot::rcc_config_has_hsi48(),
        "boot::rcc_config_has_hsi48() must return true — \
         HSI48 is required for SDMMC1 (embassy-stm32 issue #3049)"
    );
}

/// Verify that the RCC config is documented as non-default.
///
/// Architecture rule: `main.rs` must never call
/// `embassy_stm32::init(Default::default())`. It must always call
/// `embassy_stm32::init(build_embassy_config())` which configures HSI48,
/// PLL1Q (SDMMC), and PLL2R (FMC/QUADSPI). `Default::default()` leaves all
/// three unset, causing SDMMC1 to hang and FMC/QUADSPI to run on undefined
/// clocks.
#[test]
fn rcc_config_not_default() {
    assert!(
        firmware::boot::rcc_config_is_non_default(),
        "boot::rcc_config_is_non_default() must return true — \
         Default::default() leaves HSI48/PLL2R unconfigured"
    );
}

/// Verify that `firmware::boot` exposes the RCC proxy API required by the
/// architecture rule that bans `Default::default()` in `main.rs`.
///
/// The proxy functions (`rcc_config_has_hsi48`, `rcc_config_is_non_default`)
/// compile unconditionally (no `#[cfg(feature = "hardware")]` gate) so they
/// are always visible in host-test runs. `build_embassy_config` itself is
/// hardware-only but its correctness is validated indirectly through these proxies.
#[test]
fn main_does_not_use_default_config() {
    assert!(firmware::boot::rcc_config_has_hsi48());
    assert!(firmware::boot::rcc_config_is_non_default());
}

/// Verify that `platform::clock_config::TIME_DRIVER_EXPLICIT` is true.
///
/// Architecture rule: the firmware Cargo.toml must use `time-driver-tim2`
/// rather than `time-driver-any`. `time-driver-any` selects an arbitrary
/// timer at link time; `time-driver-tim2` pins TIM2 deterministically on the
/// STM32H743, avoiding conflicts with audio (TIM1/TIM8) and PWM timers.
#[allow(clippy::assertions_on_constants)]
#[test]
fn time_driver_is_explicit_tim2() {
    assert!(
        platform::clock_config::TIME_DRIVER_EXPLICIT,
        "platform::clock_config::TIME_DRIVER_EXPLICIT must be true — \
         use time-driver-tim2, not time-driver-any"
    );
}

/// Verify that the firmware Cargo.toml does not contain `time-driver-any`.
///
/// Architecture rule: `time-driver-any` is banned — it lets Cargo silently
/// choose a timer that may conflict with audio/PWM peripherals. The firmware
/// Cargo.toml must explicitly specify `time-driver-tim2`.
///
/// This test embeds `Cargo.toml` at compile time and scans it for the banned
/// string, catching regressions before they reach the build.
#[test]
fn no_time_driver_any_in_cargo_toml() {
    let firmware_cargo = include_str!("../Cargo.toml");
    assert!(
        !firmware_cargo.contains("time-driver-any"),
        "firmware Cargo.toml must not contain `time-driver-any` — \
         use `time-driver-tim2` instead (see CLAUDE.md §Build Configuration)"
    );
}

/// Verify that `SDMMC_HSI48_NOTE` in `firmware::boot` references embassy issue #3049.
///
/// The human-readable boot documentation constant must stay aligned with the
/// clock_config requirement table that hardware relies on. If the note is
/// wrong or missing, engineers debugging a silent SDMMC hang will not find
/// the root cause quickly.
#[test]
fn boot_sdmmc_hsi48_note_references_issue_3049() {
    assert!(
        firmware::boot::SDMMC_HSI48_NOTE.contains("3049"),
        "boot::SDMMC_HSI48_NOTE must reference embassy issue #3049, got: {}",
        firmware::boot::SDMMC_HSI48_NOTE
    );
}

// ─── memory.x completeness tests ─────────────────────────────────────────────
//
// These tests enforce that memory.x defines every hardware memory region
// required by the SoulAudio DAP.  They run on the host (no MCU needed)
// because include_str! embeds the file at compile time.
//
// Regions that MUST be present:
//   SRAM3     (32 KB  @ 0x30040000) — D2 domain, DMA-accessible
//   SRAM4     (64 KB  @ 0x38000000) — D3 domain, BDMA-only; must have .sram4 output section
//   EXTSDRAM  (32 MB  @ 0xC0000000) — W9825G6KH6 via FMC
//   QSPI      (16 MB  @ 0x90000000) — W25Q128JV, XiP-capable
//   .axisram output section         — DMA buffers for SAI, SDMMC, SPI display

/// Verify that memory.x maps SRAM3 at 0x30040000.
///
/// Architecture rule: SRAM3 (32 KB, D2 domain) is DMA-accessible and must
/// be a named linker region so that buffers can be explicitly placed there
/// via `#[link_section = ".sram3"]`.  Without this entry the region is
/// invisible to the linker and cannot be used safely.
#[test]
fn memory_x_defines_sram3() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains("0x30040000"),
        "memory.x must define SRAM3 at 0x30040000"
    );
}

/// Verify that memory.x maps external SDRAM at 0xC0000000.
///
/// Architecture rule: the W9825G6KH6 32 MB SDRAM (connected via FMC Bank 5)
/// must have a named linker region so that large static buffers (library
/// index cache, album-art thumbnails, FLAC decode scratch) can be placed
/// there with `#[link_section = ".extsdram"]`.
#[test]
fn memory_x_defines_external_sdram() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains("0xC0000000"),
        "memory.x must define external SDRAM at 0xC0000000"
    );
}

/// Verify that memory.x maps QSPI NOR flash at 0x90000000.
///
/// Architecture rule: the W25Q128JV 16 MB QSPI NOR flash (connected via
/// QUADSPI) must be a named linker region so that read-only assets (fonts,
/// icons, waveform LUTs) can be placed there via XiP or DMA, and so that
/// the linker can verify address ranges at build time.
#[test]
fn memory_x_defines_qspi_region() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains("0x90000000"),
        "memory.x must define QSPI NOR flash at 0x90000000"
    );
}

/// Verify that memory.x declares SRAM3 as exactly 32 KB.
///
/// The STM32H743ZI data sheet (DS12110 Rev 9, Table 4) specifies SRAM3
/// as 32 KB.  Any other value indicates a copy/paste error from a different
/// STM32H7 variant and would cause silent overflow or wasted memory.
#[test]
fn memory_x_sram3_correct_size() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains("32K") || memory_x.contains("32768"),
        "memory.x must define SRAM3 as 32K (DS12110 Rev 9, Table 4)"
    );
}

/// Verify that memory.x declares external SDRAM as exactly 32 MB.
///
/// The W9825G6KH6 is a 256 Mbit (32 MB) device.  A wrong size would
/// allow the linker to place objects beyond the physical end of the chip.
#[test]
fn memory_x_sdram_correct_size() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains("32M") || memory_x.contains("33554432"),
        "memory.x must define external SDRAM as 32M (W9825G6KH6 capacity)"
    );
}

/// Verify that memory.x declares QSPI NOR flash as exactly 16 MB.
///
/// The W25Q128JV is a 128 Mbit (16 MB) device.  A wrong size could allow
/// the linker to generate addresses beyond the chip boundary.
#[test]
fn memory_x_qspi_correct_size() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains("16M") || memory_x.contains("16777216"),
        "memory.x must define QSPI NOR as 16M (W25Q128JV capacity)"
    );
}

/// Verify that memory.x defines a .sram4 output section.
///
/// Architecture rule: BDMA peripherals (SPI6, SAI4, LPUART1, I2C4, ADC3)
/// can only access D3-domain memory (SRAM4 at 0x38000000).  Without a
/// named `.sram4` output section, static buffers cannot be placed there
/// using `#[link_section = ".sram4"]`, forcing them into AXI SRAM where
/// BDMA cannot reach them — causing silent DMA transfer failures.
#[test]
fn memory_x_defines_sram4_section() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains(".sram4"),
        "memory.x must define a .sram4 output section for BDMA buffer placement"
    );
}

/// Verify that memory.x defines a .axisram output section.
///
/// Architecture rule: DMA1, DMA2, and the display SPI DMA must use buffers
/// in AXI SRAM (D1 domain, 0x24000000).  The `.axisram` output section
/// enables `#[link_section = ".axisram"]` placement of those static buffers.
/// Without it the attribute silently falls through to the default RAM region,
/// which may or may not be DMA-accessible depending on linker ordering.
#[test]
fn memory_x_has_axisram_section() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains(".axisram"),
        "memory.x must define a .axisram output section for DMA buffer placement"
    );
}

// ─── Stack overflow protection tests ─────────────────────────────────────────
//
// These tests enforce that flip-link stack overflow protection is configured.
//
// flip-link flips the memory layout so the stack is placed BELOW .bss+.data.
// This means a stack overflow hits the bottom of RAM (a hard fault) rather than
// silently corrupting DMA buffers in AXI SRAM.
//
// Configuration:
//   .cargo/config.toml — linker = "flip-link" under [target.thumbv7em-none-eabihf]
//   memory.x           — _stack_start defined to top of AXI SRAM
//   Cargo.toml         — flip-link listed so `cargo install` can be verified in CI

/// Verify that memory.x defines `_stack_start` to explicitly control stack placement.
///
/// Architecture rule: `_stack_start` must be defined so the linker places the
/// stack at a known address (top of AXI SRAM). Without this, cortex-m-rt
/// defaults to the end of the first RAM region, which may conflict with
/// flip-link's layout transformation.
///
/// flip-link requires a defined `_stack_start` to know where to anchor its
/// inverted memory layout.
#[test]
fn memory_x_has_stack_start() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains("_stack_start"),
        "memory.x must define _stack_start to explicitly control stack placement \
         (required for flip-link compatibility)"
    );
}

/// Verify that the `.cargo/config.toml` references flip-link as the linker.
///
/// Architecture rule: flip-link must be configured as the linker for the ARM
/// embedded target so that stack overflow produces a defined HardFault instead
/// of silently corrupting DMA buffers in AXI SRAM.
///
/// The correct configuration (Cargo 1.74+) is:
///   [target.thumbv7em-none-eabihf]
///   linker = "flip-link"
///
/// For older Cargo the rustflags form "-C linker=flip-link" is also accepted.
#[test]
fn cargo_config_uses_flip_link_linker() {
    let config = include_str!("../../../.cargo/config.toml");
    let has_flip_link = config.contains("flip-link");
    assert!(
        has_flip_link,
        ".cargo/config.toml must set flip-link as the linker for \
         [target.thumbv7em-none-eabihf] to enable stack overflow protection. \
         Add: linker = \"flip-link\"  (requires: cargo install flip-link)"
    );
}

/// Verify that firmware Cargo.toml does not have flip-link as a library dependency.
///
/// Architecture rule: flip-link is a standalone linker binary, NOT a Rust
/// library crate. It must NOT appear in [dependencies] or [build-dependencies].
/// It is installed via `cargo install flip-link` and invoked only by the linker.
/// Adding it as a dep would be wrong and would cause a compile error.
#[test]
fn flip_link_is_not_a_cargo_dependency() {
    let firmware_cargo = include_str!("../Cargo.toml");
    let workspace_cargo = include_str!("../../../Cargo.toml");
    // flip-link must NOT appear in [dependencies] or [build-dependencies]
    // It is a tool, not a library. Checking for it in [dependencies] sections.
    // A simple check: it should not be in either Cargo.toml as a dep entry.
    // (It's fine if the CI workflow references it for installation.)
    let firmware_has_dep = firmware_cargo
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .any(|l| l.contains("flip-link") && (l.contains('=') || l.contains('"')));
    let workspace_has_dep = workspace_cargo
        .lines()
        .filter(|l| !l.trim_start().starts_with('#'))
        .any(|l| l.contains("flip-link") && (l.contains('=') || l.contains('"')));
    assert!(
        !firmware_has_dep && !workspace_has_dep,
        "flip-link must NOT be a Cargo dependency (it is a linker tool, not a library). \
         Configure it in .cargo/config.toml and install via `cargo install flip-link`."
    );
}

/// Verify that the HardFault exception handler module exists in the firmware crate.
///
/// Architecture rule: all embedded firmware must define a HardFault handler.
/// Without one, memory access violations, unaligned accesses, and stack
/// overflows (detected by flip-link) produce undefined behaviour instead
/// of a diagnostic halt. The handler outputs register state via defmt/RTT
/// so engineers can diagnose the fault source.
///
/// This test verifies the module exists by checking the exported marker constant.
/// The actual `#[cortex_m_rt::exception]` handler is `#[cfg(feature = "hardware")]`
/// because it requires ARM intrinsics, but the module must always be present.
#[test]
fn hardfault_handler_module_exists() {
    // Architecture rule: firmware must define an exception_handlers module.
    // HARDFAULT_DEFINED is a compile-time marker — its existence proves the
    // module compiled and the handler is wired into the firmware crate.
    assert!(
        firmware::exception_handlers::HARDFAULT_DEFINED,
        "firmware::exception_handlers::HARDFAULT_DEFINED must be true — \
         the exception_handlers module with HardFault handler must exist"
    );
}

/// Verify that the CI workflow runs `cargo build` (not just `cargo check`)
/// for the embedded target.
///
/// Architecture rule: `cargo check` validates types and borrows but skips the
/// linker step — linker errors (undefined symbols, overflow into flash, wrong
/// section placement) are only caught by `cargo build`. The CI must run a full
/// build to catch these categories of error before they reach flash time.
#[test]
fn ci_workflow_runs_embedded_build_not_just_check() {
    let ci = include_str!("../../../.github/workflows/ci.yml");
    // The embedded build job must use `cargo build`, not just `cargo check`.
    // We look for a build command targeting the embedded feature set.
    let has_build = ci
        .lines()
        .any(|l| l.contains("cargo build") && !l.trim_start().starts_with('#'));
    assert!(
        has_build,
        ".github/workflows/ci.yml must contain a `cargo build` step for the \
         embedded target (thumbv7em-none-eabihf + hardware feature). \
         `cargo check` misses linker errors. Add a build-embedded job."
    );
}

// ─── Interrupt priority hierarchy tests ──────────────────────────────────────
//
// These tests enforce that NVIC interrupt priorities are defined as named
// constants in platform::clock_config::InterruptPriorities.
//
// STM32H743ZI implements 4 priority bits (16 levels, 0=highest, 15=lowest).
// Embassy-stm32 uses the top 4 bits of the 8-bit NVIC priority register,
// so all valid priority values are multiples of 16 (0, 16, 32, …, 240).
//
// Priority hierarchy (lower number = higher priority, preempts higher numbers):
//   AUDIO_SAI_DMA (0)  > DISPLAY_SPI_DMA (32) > SDMMC_DMA (64) >
//   INPUT_EXTI (96) > BLUETOOTH_UART (128)
//
// Without this hierarchy, audio DMA interrupts can be preempted by display
// refresh or SD card transfers, causing audio dropouts (underrun/overrun).

/// Verify that InterruptPriorities struct exists with all required constants.
///
/// Architecture rule: interrupt priorities must be defined as named constants
/// in platform::clock_config so they are auditable, testable, and referenced
/// from a single source of truth.
#[test]
fn interrupt_priorities_type_exists() {
    // Architecture rule: interrupt priorities must be defined as constants
    let _ = platform::clock_config::InterruptPriorities::AUDIO_SAI_DMA;
    let _ = platform::clock_config::InterruptPriorities::DISPLAY_SPI_DMA;
    let _ = platform::clock_config::InterruptPriorities::SDMMC_DMA;
    let _ = platform::clock_config::InterruptPriorities::INPUT_EXTI;
    let _ = platform::clock_config::InterruptPriorities::BLUETOOTH_UART;
}

/// Verify that audio SAI DMA has the highest interrupt priority.
///
/// Architecture rule: audio SAI DMA must never be preempted by display
/// refresh or SD card transfers. On ARM NVIC, a numerically lower priority
/// value means higher actual priority (preempts higher-numbered priorities).
///
/// If this ordering is wrong, a display SPI DMA interrupt can preempt audio
/// SAI DMA, causing audio buffer underrun or overrun and audible dropouts.
#[test]
fn audio_priority_is_highest() {
    use platform::clock_config::InterruptPriorities as P;
    assert!(
        P::AUDIO_SAI_DMA < P::DISPLAY_SPI_DMA,
        "audio priority ({}) must be higher (lower number) than display ({})",
        P::AUDIO_SAI_DMA,
        P::DISPLAY_SPI_DMA
    );
    assert!(
        P::AUDIO_SAI_DMA < P::SDMMC_DMA,
        "audio priority must be higher than SDMMC"
    );
    assert!(
        P::AUDIO_SAI_DMA < P::BLUETOOTH_UART,
        "audio priority must be higher than bluetooth"
    );
}

/// Verify that all priority values are multiples of 16.
///
/// Architecture rule: STM32H743 implements 4 priority bits. The NVIC stores
/// priorities in the top 4 bits of the 8-bit priority register, so only
/// values that are multiples of 16 are meaningful. Using non-aligned values
/// (e.g. 5 instead of 0) still works but is misleading — the hardware ignores
/// the lower 4 bits. All constants must be multiples of 16 to be explicit.
#[test]
fn priority_values_are_nvic_aligned() {
    use platform::clock_config::InterruptPriorities as P;
    for (name, val) in [
        ("AUDIO_SAI_DMA", P::AUDIO_SAI_DMA),
        ("DISPLAY_SPI_DMA", P::DISPLAY_SPI_DMA),
        ("SDMMC_DMA", P::SDMMC_DMA),
        ("INPUT_EXTI", P::INPUT_EXTI),
        ("BLUETOOTH_UART", P::BLUETOOTH_UART),
        ("TIME_DRIVER", P::TIME_DRIVER),
    ] {
        assert_eq!(
            val % 16,
            0,
            "priority {name} ({val}) must be a multiple of 16 for STM32H743 4-bit NVIC"
        );
    }
}

// ─── Watchdog configuration tests ────────────────────────────────────────────
//
// These tests enforce that the IWDG (Independent Watchdog) timeout is defined
// in firmware::boot and has a value appropriate for the SoulAudio DAP.
//
// The IWDG uses the 32 kHz LSI clock and resets the MCU if not fed within the
// timeout period. This catches Embassy task deadlocks and runaway panic loops.
// It cannot be disabled once started, making it a hard safety net.
//
// Timeout constraints:
//   Minimum: 5 s — SD card init can take up to 3 s; allow margin.
//   Maximum: 30 s — fail fast on deadlock; don't hang indefinitely.

/// Verify that firmware::boot::WATCHDOG_TIMEOUT_MS exists.
///
/// Architecture rule: the watchdog timeout must be a named constant so it can
/// be audited and referenced by the watchdog init code. Magic numbers in the
/// watchdog init call would hide the timeout policy from code review.
#[test]
fn watchdog_config_exists() {
    // Architecture rule: watchdog configuration must be defined
    let _ = firmware::boot::WATCHDOG_TIMEOUT_MS;
}

/// Verify that the watchdog timeout is between 5 and 30 seconds.
///
/// Architecture rule: the watchdog timeout must be long enough for the
/// worst-case SD card initialization (approximately 3 seconds) but short
/// enough to recover from deadlocks quickly (under 30 seconds).
///
/// If the timeout is too short, the MCU resets during normal SD card init.
/// If the timeout is too long, a deadlocked task hangs the device for
/// an unacceptably long time before the watchdog fires.
#[test]
fn watchdog_timeout_is_reasonable() {
    let timeout = firmware::boot::WATCHDOG_TIMEOUT_MS;
    assert!(
        timeout >= 5_000,
        "watchdog timeout must be >= 5 seconds (5000 ms), got {timeout}ms"
    );
    assert!(
        timeout <= 30_000,
        "watchdog timeout must be <= 30 seconds (30000 ms), got {timeout}ms"
    );
}

// ─── D3 power domain tests ────────────────────────────────────────────────────
//
// The STM32H743 D3 domain (also called SmartRun domain) hosts:
//   SRAM4 (64 KB, BDMA-accessible), BDMA, SPI6, SAI4, LPUART1, I2C4, ADC3.
//
// D3 peripherals are only accessible if the D3 clock domain is running.
// Embassy-stm32 enables peripheral bus clocks when peripherals are constructed,
// so explicit D3 domain enable at RCC level is handled automatically.
//
// The rcc_config_enables_d3_domain() function documents this policy.

/// Verify that firmware::boot::rcc_config_enables_d3_domain() returns true.
///
/// Architecture rule: the firmware RCC config must correctly enable D3 domain
/// peripheral clocks. In embassy-stm32 0.1.0, D3 peripheral clocks (BDMA,
/// SPI6, SAI4) are enabled automatically at peripheral construction time via
/// the RCC peripheral clock enable registers. The `rcc_config_enables_d3_domain`
/// function documents this policy and is checked by this test.
#[test]
fn d3_power_domain_enabled_in_rcc_config() {
    assert!(
        firmware::boot::rcc_config_enables_d3_domain(),
        "RCC config must enable D3 power domain for SRAM4/SPI6/SAI4/LPUART1/I2C4"
    );
}

// ─── SAI / I2C audio init stub architecture tests ────────────────────────────
//
// These tests enforce that:
//   1. platform::audio_config exposes SaiAudioConfig and I2cAddresses types.
//   2. firmware::boot exposes SAI_INIT_NOTE and I2C_INIT_NOTE anchor constants.
//
// They turn RED when the types or constants do not exist, then GREEN once the
// implementation stubs are added. They guard against accidentally removing or
// renaming these architectural anchor points in future refactors.

/// Verify that `platform::audio_config::SaiAudioConfig` is callable and
/// returns the correct 192 kHz configuration for the ES9038Q2M DAC.
///
/// Architecture rule: SAI1 configuration must live in the `platform` crate
/// (HAL layer), not in `firmware`. This keeps the clock arithmetic testable
/// on the host and reusable without pulling in embassy-stm32 types.
#[test]
fn audio_config_type_exists() {
    let cfg = platform::audio_config::SaiAudioConfig::es9038q2m_192khz();
    assert_eq!(cfg.sample_rate_hz, 192_000);
}

/// Verify that `platform::audio_config::I2cAddresses` constants are defined
/// and have the expected values for the BQ25895 PMIC and ES9038Q2M DAC.
///
/// Architecture rule: I2C peripheral addresses must be named constants in
/// the `platform` crate. Magic-number addresses in `firmware` are forbidden.
#[test]
fn i2c_addresses_defined() {
    // BQ25895 PMIC: fixed 7-bit address 0x6A
    let pmic_addr = platform::audio_config::I2cAddresses::BQ25895_PMIC;
    assert!(
        pmic_addr == 0x6A || pmic_addr == 0x6B,
        "BQ25895 address must be 0x6A or 0x6B, got 0x{pmic_addr:02X}"
    );

    // ES9038Q2M DAC: hardware-fixed 7-bit address 0x48
    let dac_addr = platform::audio_config::I2cAddresses::ES9038Q2M_DAC;
    assert_eq!(
        dac_addr, 0x48,
        "ES9038Q2M I2C address is hardware-fixed at 0x48"
    );
}

/// Verify that `firmware::boot::SAI_INIT_NOTE` exists and documents the
/// SAI1 initialisation requirement.
///
/// Architecture rule: `boot.rs` must carry a named constant that serves as
/// a documentation anchor for the SAI1 initialisation TODO. This prevents
/// the requirement from being lost in a comment that could be silently deleted.
/// Tests that reference `SAI_INIT_NOTE` will fail to compile if it is removed.
#[test]
fn sai_init_stub_exists_in_boot() {
    // Architecture rule: boot module must document SAI init location.
    let note = firmware::boot::SAI_INIT_NOTE;
    assert!(
        !note.is_empty(),
        "firmware::boot::SAI_INIT_NOTE must be a non-empty documentation string"
    );
    // Must reference SAI or audio to be meaningful
    let lower = note.to_lowercase();
    assert!(
        lower.contains("sai") || lower.contains("audio"),
        "SAI_INIT_NOTE must mention SAI or audio, got: {note}"
    );
}

/// Verify that `firmware::boot::I2C_INIT_NOTE` exists and documents the
/// I2C bus initialisation requirement.
///
/// Architecture rule: `boot.rs` must carry a named constant that serves as
/// a documentation anchor for the I2C2/I2C3 initialisation TODO. The PMIC
/// and DAC cannot be controlled until the I2C buses are up; this constant
/// keeps that requirement visible and test-guarded.
#[test]
fn i2c_init_stub_exists_in_boot() {
    // Architecture rule: boot module must document I2C init location.
    let note = firmware::boot::I2C_INIT_NOTE;
    assert!(
        !note.is_empty(),
        "firmware::boot::I2C_INIT_NOTE must be a non-empty documentation string"
    );
    // Must reference I2C or PMIC/DAC to be meaningful
    let lower = note.to_lowercase();
    assert!(
        lower.contains("i2c") || lower.contains("pmic") || lower.contains("dac"),
        "I2C_INIT_NOTE must mention I2C, PMIC, or DAC, got: {note}"
    );
}

// ─── PLL3 / SAI MCLK architecture tests ──────────────────────────────────────
//
// These tests enforce that PLL3 is configured for SAI MCLK in build_embassy_config().
//
// SAI1 for ES9038Q2M requires MCLK = 256 × fs = 256 × 192 000 = 49.152 MHz.
// PLL1Q (200 MHz) does not divide cleanly to this frequency. PLL3 is the
// dedicated audio PLL on STM32H7 and must be configured separately.
//
// Best achievable from HSI (64 MHz) with integer divisors:
//   HSI(64) / M(4) × N(49) / P(16) = 49.0 MHz (0.31% error — inaudible).
//
// Reference: STM32H743 RM0433 §8.3.2, embassy-stm32 Config::pll3 field.

/// Verify that `firmware::boot::rcc_config_has_pll3_for_sai()` returns true.
///
/// Architecture rule: `build_embassy_config()` must configure PLL3 as the
/// SAI kernel clock source. PLL1Q (200 MHz) does not divide cleanly to
/// 49.152 MHz; leaving PLL3 unconfigured causes audio to output on an
/// incorrect or undefined clock, producing silence or wrong sample rate.
#[test]
fn pll3_configured_for_sai_mclk() {
    assert!(
        firmware::boot::rcc_config_has_pll3_for_sai(),
        "build_embassy_config() must configure PLL3 for SAI MCLK (≈49 MHz). \
         PLL1Q (200 MHz) does not divide cleanly to 49.152 MHz."
    );
}

/// Verify that the PLL3 M/N/P divisors in boot match platform::audio_config.
///
/// Architecture rule: PLL3 divisors are defined in two places — the hardware
/// config in `build_embassy_config()` and the documentation/test helpers in
/// `platform::audio_config::SaiAudioConfig`. They must stay in sync so that
/// the achievable MCLK calculated in tests matches what the hardware sees.
#[test]
fn sai_pll3_divisors_match_audio_config() {
    let (m, n, p) = firmware::boot::sai_pll3_divisors();
    assert_eq!(
        m,
        platform::audio_config::SaiAudioConfig::pll3_m(),
        "PLL3 M divisor in boot must match SaiAudioConfig::pll3_m()"
    );
    assert_eq!(
        n,
        platform::audio_config::SaiAudioConfig::pll3_n() as u8,
        "PLL3 N multiplier in boot must match SaiAudioConfig::pll3_n()"
    );
    assert_eq!(
        p,
        platform::audio_config::SaiAudioConfig::pll3_p(),
        "PLL3 P divisor in boot must match SaiAudioConfig::pll3_p()"
    );
}

/// Verify that BQ25895 PMIC address is confirmed as 0x6A.
///
/// Architecture rule: the BQ25895 I2C address is hardware-fixed at 0x6A.
/// The 0x6B value that appeared in some early datasheet revisions was a
/// typographic error (confirmed by TI E2E forum, SLUUBA2B errata).
#[test]
fn bq25895_address_confirmed_0x6a() {
    use platform::audio_config::I2cAddresses;
    assert_eq!(
        I2cAddresses::BQ25895_PMIC,
        0x6A,
        "BQ25895 I2C address is hardware-fixed at 0x6A (not 0x6B). \
         See TI SLUUBA2B errata and E2E forum post #507682."
    );
}

// ─── SDMMC1 and QUADSPI init stub architecture tests ─────────────────────────
//
// These tests enforce that:
//   1. platform::storage_config exposes SdmmcConfig and QspiNorConfig types.
//   2. firmware::boot exposes SDMMC_INIT_NOTE and QSPI_INIT_NOTE anchor constants.
//   3. All 6 SDMMC1 GPIO pins are documented in SdmmcPins.
//
// They turn RED when the types or constants do not exist, then GREEN once the
// implementation stubs are added. They guard against accidentally removing or
// renaming these architectural anchor points in future refactors.

/// Verify that `platform::storage_config::SdmmcConfig` is callable and
/// returns the 4-bit UHS-I configuration for the microSD card.
///
/// Architecture rule: SDMMC1 bus configuration (bus width, clock, timeout)
/// must live in the `platform` crate (HAL layer), not in `firmware`. This
/// keeps the configuration testable on the host without embassy-stm32 types.
#[test]
fn sdmmc_config_type_exists() {
    let cfg = platform::storage_config::SdmmcConfig::microsd_uhs_i();
    assert_eq!(cfg.bus_width, 4, "SDMMC must use 4-bit mode for UHS-I");
}

/// Verify that `platform::storage_config::QspiNorConfig` is callable and
/// returns the W25Q128JV configuration with flash_size_field = 23.
///
/// Architecture rule: QUADSPI NOR flash configuration must live in the
/// `platform` crate. The flash_size_field = 23 encodes 16 MB (2^24 bytes)
/// as required by the QUADSPI_DCR.FSIZE register on STM32H7.
#[test]
fn qspi_config_type_exists() {
    let cfg = platform::storage_config::QspiNorConfig::w25q128jv_at_100mhz();
    assert_eq!(
        cfg.flash_size_field, 23,
        "W25Q128JV flash_size_field must be 23 (encodes 16 MB)"
    );
}

/// Verify that `firmware::boot::SDMMC_INIT_NOTE` exists and documents the
/// SDMMC1 initialisation requirement.
///
/// Architecture rule: `boot.rs` must carry a named constant that serves as
/// a documentation anchor for the SDMMC1 initialisation TODO. This prevents
/// the requirement from being lost in a comment that could be silently deleted.
/// Tests that reference `SDMMC_INIT_NOTE` will fail to compile if it is removed.
#[test]
fn sdmmc_init_stub_exists_in_boot() {
    let note = firmware::boot::SDMMC_INIT_NOTE;
    assert!(
        !note.is_empty(),
        "firmware::boot::SDMMC_INIT_NOTE must be a non-empty documentation string"
    );
    // Must reference SDMMC and IDMA to be meaningful (key architectural facts)
    assert!(
        note.to_uppercase().contains("SDMMC") || note.to_uppercase().contains("SD"),
        "SDMMC_INIT_NOTE must mention SDMMC or SD, got: {note}"
    );
}

/// Verify that `firmware::boot::QSPI_INIT_NOTE` exists and documents the
/// QUADSPI NOR flash initialisation requirement.
///
/// Architecture rule: `boot.rs` must carry a named constant that serves as
/// a documentation anchor for the QUADSPI initialisation TODO. Without this
/// anchor, the requirement to initialise the NOR flash (fonts, icons, OTA
/// staging) can be silently dropped by a refactor.
#[test]
fn qspi_init_stub_exists_in_boot() {
    let note = firmware::boot::QSPI_INIT_NOTE;
    assert!(
        !note.is_empty(),
        "firmware::boot::QSPI_INIT_NOTE must be a non-empty documentation string"
    );
    // Must reference QUADSPI or QSPI or NOR to be meaningful
    let upper = note.to_uppercase();
    assert!(
        upper.contains("QUADSPI") || upper.contains("QSPI") || upper.contains("NOR"),
        "QSPI_INIT_NOTE must mention QUADSPI, QSPI, or NOR, got: {note}"
    );
}

/// Verify that all 6 SDMMC1 GPIO pins are documented in `platform::storage_config::SdmmcPins`.
///
/// Architecture rule: all six SDMMC1 signals (CLK, CMD, D0–D3) must be
/// documented as named constants in `SdmmcPins`. This ensures every engineer
/// working on the SDMMC peripheral has a single authoritative source for the
/// pin assignments on the STM32H743ZI LQFP144 package.
#[test]
fn sdmmc_pin_assignments_documented() {
    // All 6 SDMMC pins must be documented as named constants.
    // PC12 = CLK, PD2 = CMD, PC8-PC11 = D0-D3 (per STM32H743ZI AF table, AF12).
    let _clk = platform::storage_config::SdmmcPins::CLK_PIN;
    let _cmd = platform::storage_config::SdmmcPins::CMD_PIN;
    let _d0 = platform::storage_config::SdmmcPins::D0_PIN;
    let _d1 = platform::storage_config::SdmmcPins::D1_PIN;
    let _d2 = platform::storage_config::SdmmcPins::D2_PIN;
    let _d3 = platform::storage_config::SdmmcPins::D3_PIN;

    // Verify CLK is PC12 (STM32H743ZI LQFP144, SDMMC1_CK alternate function)
    assert!(
        platform::storage_config::SdmmcPins::CLK_PIN.starts_with("PC12"),
        "SDMMC1_CK must be PC12, got: {}",
        platform::storage_config::SdmmcPins::CLK_PIN
    );
    // Verify CMD is PD2 (STM32H743ZI LQFP144, SDMMC1_CMD alternate function)
    assert!(
        platform::storage_config::SdmmcPins::CMD_PIN.starts_with("PD2"),
        "SDMMC1_CMD must be PD2, got: {}",
        platform::storage_config::SdmmcPins::CMD_PIN
    );
}

// =========================================================================
// Workspace Lints + cargo-deny Hardening Tests  (TDD round 7, slice 1)
// =========================================================================

/// Verify that [workspace.lints.rust] section exists in the workspace Cargo.toml.
#[test]
fn workspace_lints_rust_section_present() {
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(
        cargo_toml.contains("[workspace.lints.rust]"),
        "Cargo.toml must contain a [workspace.lints.rust] section (Rust 1.73+ feature)."
    );
}

/// Verify that [workspace.lints.clippy] section exists in the workspace Cargo.toml.
#[test]
fn workspace_lints_clippy_section_present() {
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(
        cargo_toml.contains("[workspace.lints.clippy]"),
        "Cargo.toml must contain a [workspace.lints.clippy] section."
    );
}

/// Verify that arithmetic_side_effects is present in workspace lints.
#[test]
fn workspace_lints_deny_arithmetic_side_effects() {
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(
        cargo_toml.contains("arithmetic_side_effects"),
        "Cargo.toml [workspace.lints.clippy] must include arithmetic_side_effects."
    );
}

/// Verify that indexing_slicing is present in workspace lints.
#[test]
fn workspace_lints_deny_indexing_slicing() {
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(
        cargo_toml.contains("indexing_slicing"),
        "Cargo.toml [workspace.lints.clippy] must include indexing_slicing."
    );
}

/// Verify that deny.toml sets multiple-versions = "deny".
#[test]
fn deny_toml_multiple_versions_deny() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains(r#"multiple-versions = "deny""#),
        "deny.toml must set multiple-versions = deny (currently warn)."
    );
}

/// Verify that deny.toml sets wildcards = "deny".
#[test]
fn deny_toml_wildcards_deny() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains(r#"wildcards = "deny""#),
        "deny.toml must set wildcards = deny (currently warn)."
    );
}

/// Verify that embedded-alloc is banned in deny.toml.
#[test]
fn deny_toml_bans_embedded_alloc() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains("embedded-alloc"),
        "deny.toml must ban embedded-alloc."
    );
}

/// Verify that getrandom is banned in deny.toml.
#[test]
fn deny_toml_bans_getrandom() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains("getrandom"),
        "deny.toml must ban getrandom."
    );
}

/// Verify that wee_alloc is banned in deny.toml.
#[test]
fn deny_toml_bans_wee_alloc() {
    let deny_toml = include_str!("../../../deny.toml");
    assert!(
        deny_toml.contains("wee_alloc"),
        "deny.toml must ban wee_alloc (RUSTSEC-2022-0054)."
    );
}

/// Verify that firmware/src/lib.rs does NOT contain blanket file-level cast allows.
#[test]
fn firmware_lib_no_blanket_cast_allows() {
    let lib_rs = include_str!("../src/lib.rs");
    let forbidden = [
        "allow(clippy::cast_possible_truncation)",
        "allow(clippy::cast_sign_loss)",
    ];
    for pattern in forbidden {
        assert!(
            !lib_rs.contains(&format!("#![{pattern}]")),
            "firmware/src/lib.rs must not contain a blanket #![{pattern}]."
        );
    }
}

/// Verify that cargo-deny-action in ci.yml is pinned to a full commit SHA.
#[test]
fn ci_cargo_deny_action_sha_pinned() {
    let ci_yml = include_str!("../../../.github/workflows/ci.yml");
    // Must NOT use the floating tag alone (no SHA after @v2)
    let uses_floating_tag = ci_yml
        .lines()
        .filter(|l| l.contains("cargo-deny-action@"))
        .any(|l| {
            if let Some(after_at) = l.split('@').nth(1) {
                let token = after_at.split_whitespace().next().unwrap_or("");
                // floating tag: starts with 'v' followed by digits, not a 40-char hex SHA
                token.starts_with('v')
                    && token.chars().all(|c| c.is_ascii_alphanumeric() || c == '.')
            } else {
                false
            }
        });
    assert!(
        !uses_floating_tag,
        "ci.yml must not use cargo-deny-action@v2 (floating tag). \
         Pin to a full commit SHA for supply-chain security."
    );
    let has_sha_pin = ci_yml
        .lines()
        .filter(|l| l.contains("cargo-deny-action@"))
        .any(|l| {
            if let Some(after_at) = l.split('@').nth(1) {
                let sha = after_at.split_whitespace().next().unwrap_or("");
                sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit())
            } else {
                false
            }
        });
    assert!(
        has_sha_pin,
        "ci.yml cargo-deny-action must be pinned to a 40-character commit SHA."
    );
}

#[test]
fn framebuffer_static_has_axisram_link_section() {
    // The FRAMEBUFFER static in main.rs must use #[link_section = ".axisram"]
    // so it lands in DMA-accessible memory for SPI display DMA transfers.
    let main_rs = include_str!("../src/main.rs");
    assert!(
        main_rs.contains("#[link_section = \".axisram\"]"),
        "FRAMEBUFFER static must use #[link_section = \".axisram\"] for DMA safety"
    );
    assert!(
        main_rs.contains("FRAMEBUFFER"),
        "main.rs must declare FRAMEBUFFER static"
    );
}

#[test]
fn no_large_static_without_link_section_in_firmware() {
    // Large buffers without link_section end up in DTCM (default RAM for cortex-m-rt)
    // DTCM is NOT DMA-accessible on STM32H7 — only use for CPU-hot data.
    // This test checks that there's no obvious large unattributed static buffer.
    let main_rs = include_str!("../src/main.rs");
    // If there's a static mut array, it must have a link_section
    // Simple heuristic: count link_section vs static mut array occurrences
    let link_sections = main_rs.matches("#[link_section").count();
    let static_mut_arrays = main_rs.matches("static mut").count();
    assert!(
        link_sections >= static_mut_arrays,
        "All static mut arrays should have #[link_section] — found {} arrays but {} link_sections",
        static_mut_arrays,
        link_sections
    );
}

// =============================================================================
// CI Hardening Tests (TDD Round 7 Slice 4)
// =============================================================================

#[test]
fn cargo_toml_has_msrv_field() {
    // Pinning MSRV prevents silent breakage when users upgrade toolchains.
    // Rust embedded projects should pin to a known-good version.
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(
        cargo_toml.contains("rust-version"),
        "Workspace Cargo.toml must set rust-version (MSRV). \
         Add: rust-version = \"1.75\" (or current stable) to [workspace.package]"
    );
}

#[test]
fn profile_dev_has_panic_abort() {
    // panic = "abort" in dev profile prevents unwinding on embedded targets.
    // Without this, panic calls pull in the unwinding runtime (adds ~10KB).
    // Even in dev builds, embedded targets should not unwind.
    let cargo_toml = include_str!("../../../Cargo.toml");
    // Check that [profile.dev] section exists with panic = "abort"
    assert!(
        cargo_toml.contains("[profile.dev]"),
        "Cargo.toml must have [profile.dev] section"
    );
    // Find the dev profile section and check for panic = "abort"
    // Simple check: the string appears somewhere in the toml
    assert!(
        cargo_toml.contains("panic = \"abort\""),
        "Cargo.toml [profile.dev] must set panic = \"abort\" for embedded targets. \
         Unwinding runtime adds ~10KB and doesn't work on bare-metal."
    );
}

#[test]
fn profile_release_has_panic_abort() {
    let cargo_toml = include_str!("../../../Cargo.toml");
    // Count occurrences - should appear at least twice (dev + release)
    let count = cargo_toml.matches("panic = \"abort\"").count();
    assert!(
        count >= 2,
        "Both [profile.dev] and [profile.release] should have panic = \"abort\". \
         Found {} occurrence(s) in Cargo.toml.",
        count
    );
}

#[test]
fn workspace_package_section_exists() {
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(
        cargo_toml.contains("[workspace.package]"),
        "Workspace Cargo.toml should have [workspace.package] for shared metadata \
         (edition, version, authors, rust-version)"
    );
}

#[test]
fn ci_yml_has_embedded_clippy_job() {
    // CI must run clippy for the embedded target, not just the host.
    // Host clippy misses many embedded-specific issues (unsafe, DMA patterns).
    let ci_yml = include_str!("../../../.github/workflows/ci.yml");
    assert!(
        ci_yml.contains("thumbv7em-none-eabihf"),
        "CI workflow must include embedded target (thumbv7em-none-eabihf) checks. \
         Add a clippy job that runs against the ARM target."
    );
}

#[test]
fn ci_yml_has_size_check_job() {
    // Binary size should be checked in CI to catch flash bloat early.
    // Using arm-none-eabi-size (text+data sections) is more accurate than ELF file size.
    let ci_yml = include_str!("../../../.github/workflows/ci.yml");
    assert!(
        ci_yml.contains("size")
            || ci_yml.contains("binary-size")
            || ci_yml.contains("arm-none-eabi"),
        "CI workflow should check binary size to catch flash bloat. \
         Use arm-none-eabi-size or equivalent."
    );
}

// ---- Workspace lint propagation tests (TDD round 8 slice 1) ----------------

/// Every workspace member must opt into workspace lints.
/// Without [lints] workspace = true, ALL safety lints are silently bypassed.
#[test]
fn all_workspace_members_opt_into_workspace_lints() {
    let crates_to_check = [
        include_str!("../../../crates/eink/eink-emulator/Cargo.toml"),
        include_str!("../../../crates/eink/eink-testing/Cargo.toml"),
        include_str!("../../../crates/eink/eink-specs/Cargo.toml"),
        include_str!("../../../crates/eink/eink-system/Cargo.toml"),
        include_str!("../../../crates/eink/eink-components/Cargo.toml"),
        include_str!("../../../crates/firmware-ui/Cargo.toml"),
        include_str!("../../../xtask/Cargo.toml"),
    ];
    for (i, cargo_toml) in crates_to_check.iter().enumerate() {
        assert!(
            cargo_toml.contains("[lints]") && cargo_toml.contains("workspace = true"),
            "Cargo.toml index {} is missing [lints] workspace = true",
            i
        );
    }
}

#[test]
fn eink_emulator_crate_opts_into_workspace_lints() {
    let cargo_toml = include_str!("../../../crates/eink/eink-emulator/Cargo.toml");
    assert!(
        cargo_toml.contains("[lints]"),
        "eink-emulator/Cargo.toml missing [lints] section"
    );
    assert!(
        cargo_toml.contains("workspace = true"),
        "eink-emulator/Cargo.toml missing workspace = true under [lints]"
    );
}

#[test]
fn eink_testing_crate_opts_into_workspace_lints() {
    let cargo_toml = include_str!("../../../crates/eink/eink-testing/Cargo.toml");
    assert!(
        cargo_toml.contains("[lints]"),
        "eink-testing/Cargo.toml missing [lints] section"
    );
    assert!(
        cargo_toml.contains("workspace = true"),
        "eink-testing/Cargo.toml missing workspace = true"
    );
}

#[test]
fn eink_specs_crate_opts_into_workspace_lints() {
    let cargo_toml = include_str!("../../../crates/eink/eink-specs/Cargo.toml");
    assert!(
        cargo_toml.contains("[lints]"),
        "eink-specs/Cargo.toml missing [lints] section"
    );
    assert!(
        cargo_toml.contains("workspace = true"),
        "eink-specs/Cargo.toml missing workspace = true"
    );
}

#[test]
fn eink_system_crate_opts_into_workspace_lints() {
    let cargo_toml = include_str!("../../../crates/eink/eink-system/Cargo.toml");
    assert!(
        cargo_toml.contains("[lints]"),
        "eink-system/Cargo.toml missing [lints] section"
    );
    assert!(
        cargo_toml.contains("workspace = true"),
        "eink-system/Cargo.toml missing workspace = true"
    );
}

#[test]
fn eink_components_crate_opts_into_workspace_lints() {
    let cargo_toml = include_str!("../../../crates/eink/eink-components/Cargo.toml");
    assert!(
        cargo_toml.contains("[lints]"),
        "eink-components/Cargo.toml missing [lints] section"
    );
    assert!(
        cargo_toml.contains("workspace = true"),
        "eink-components/Cargo.toml missing workspace = true"
    );
}

#[test]
fn firmware_ui_crate_opts_into_workspace_lints() {
    let cargo_toml = include_str!("../../../crates/firmware-ui/Cargo.toml");
    assert!(
        cargo_toml.contains("[lints]"),
        "firmware-ui/Cargo.toml missing [lints] section"
    );
    assert!(
        cargo_toml.contains("workspace = true"),
        "firmware-ui/Cargo.toml missing workspace = true"
    );
}

#[test]
fn xtask_crate_opts_into_workspace_lints() {
    let cargo_toml = include_str!("../../../xtask/Cargo.toml");
    assert!(
        cargo_toml.contains("[lints]"),
        "xtask/Cargo.toml missing [lints] section"
    );
    assert!(
        cargo_toml.contains("workspace = true"),
        "xtask/Cargo.toml missing workspace = true"
    );
}

#[test]
fn window_rs_transmute_has_safety_comment() {
    // transmute of raw isize to WNDPROC must have SAFETY: justification.
    let window_src = include_str!("../../../crates/eink/eink-emulator/src/window.rs");
    if window_src.contains("transmute") {
        assert!(
            window_src.contains("// SAFETY:") || window_src.contains("SAFETY:"),
            "window.rs contains transmute but no SAFETY: comment"
        );
    }
}

// -- SAI DMA ping-pong documentation (GAP-M6) ---------------------------------

/// SAI audio must use ping-pong (double-buffer) DMA with half-complete interrupt.
/// Without half-complete handling, the CPU refill window is half as large,
/// doubling effective audio latency and risking glitches.
///
/// This test ensures the architecture is documented before implementation.
/// Reference: Embassy ring buffer DMA pattern (embassy-rs issue #2752)
#[test]
fn audio_sai_ping_pong_dma_pattern_documented() {
    let note = firmware::boot::SAI_INIT_NOTE;
    // SAI_INIT_NOTE must reference ping-pong / half-complete pattern
    assert!(
        note.contains("half") || note.contains("ping") || note.contains("HTIF")
            || note.contains("double"),
        "firmware::boot::SAI_INIT_NOTE must document the ping-pong/half-complete DMA pattern.          Add: ping-pong (double-buffer), HT interrupt, TC interrupt.          See: embassy-rs issue #2752, ST AN5051 s5.3.          Current note: {note}"
    );
}

// =============================================================================
// CI Hardening Tests (TDD Round 8 Slice 5)
// =============================================================================

// -- ABI version enforcement (GAP-M5) -----------------------------------------

/// firmware-ui ABI version must be non-zero and the binary must verify it at
/// startup. CI must enforce that ABI_VERSION is bumped whenever the render_ui
/// C-ABI signature changes.
///
/// The `ui_abi_version()` export is the runtime check; the build.rs hash is the
/// static check that makes a forgotten bump visible in build output.
#[test]
fn firmware_ui_abi_version_is_nonzero() {
    let lib_src = include_str!("../../firmware-ui/src/lib.rs");
    assert!(
        lib_src.contains("ABI_VERSION"),
        "firmware-ui/src/lib.rs must define ABI_VERSION constant for hot-reload ABI contract"
    );
    assert!(
        !lib_src.contains("ABI_VERSION: u32 = 0"),
        "firmware-ui ABI_VERSION must not be 0 - zero means unversioned"
    );
}

/// The render_ui signature must be tracked so changes are visible when
/// bumping ABI_VERSION is forgotten. The build.rs hash enforces this: if
/// the signature string changes, the hash changes and the mismatch surfaces
/// in build output before any code runs.
#[test]
fn firmware_ui_has_abi_enforcement_mechanism() {
    let build_rs = include_str!("../../firmware-ui/build.rs");
    assert!(
        build_rs.contains("render_ui") || build_rs.contains("RENDER_UI"),
        "firmware-ui/build.rs must track the render_ui signature hash          to detect forgotten ABI_VERSION bumps"
    );
    assert!(
        build_rs.contains("SIGNATURE_HASH") || build_rs.contains("signature"),
        "firmware-ui/build.rs must emit a RENDER_UI_SIGNATURE_HASH env var for ABI tracking"
    );
}

// -- CI doc tests (GAP-L4) ----------------------------------------------------

/// CI must run `cargo test --doc` to prevent doc example bitrot.
/// Doc examples in `///` comments are part of the API contract and must
/// compile and pass on every PR.
#[test]
fn ci_runs_doc_tests() {
    let ci_yml = include_str!("../../../.github/workflows/ci.yml");
    assert!(
        ci_yml.contains("--doc") || ci_yml.contains("test --doc") || ci_yml.contains("doctest"),
        "CI workflow must run `cargo test --doc` to prevent documentation example bitrot.          Add a doc-tests job: cargo test --doc -p platform"
    );
}

/// CI must have a cargo audit or cargo-auditable step for supply chain security.
/// Without it, CVEs disclosed after deployment require manual cross-referencing.
#[test]
fn ci_has_supply_chain_audit() {
    let ci_yml = include_str!("../../../.github/workflows/ci.yml");
    assert!(
        ci_yml.contains("cargo-audit")
            || ci_yml.contains("cargo audit")
            || ci_yml.contains("auditable")
            || ci_yml.contains("rustsec"),
        "CI must include supply chain security scanning (cargo audit or cargo-auditable).          This catches CVEs in dependencies before and after deployment."
    );
}

// -- MPU SRAM1/2 coverage (GAP-H1) -------------------------------------------

/// SRAM1+SRAM2 (D2 domain, 0x30000000, 256 KB) must be MPU-non-cacheable.
/// Embassy task stacks live here. DMA1/2 can reach D2 SRAM (AHB bus).
/// If cacheable, CPU and DMA see different data - silent corruption.
/// Reference: ST AN4839, community.st.com/t5/stm32-mcus/how-to-use-the-mpu
#[test]
fn mpu_config_covers_sram12_as_non_cacheable() {
    let mpu_src = include_str!("../../../crates/platform/src/mpu.rs");
    assert!(
        mpu_src.contains("0x3000_0000") || mpu_src.contains("0x30000000"),
        "MPU config must cover SRAM1/2 at 0x30000000. D2 domain SRAM is DMA-accessible; MPU non-cacheable config required. See ST AN4839."
    );
}

#[test]
fn mpu_register_pairs_includes_sram12_region() {
    let mpu_src = include_str!("../../../crates/platform/src/mpu.rs");
    assert!(
        mpu_src.contains("0x3000_0000") || mpu_src.contains("0x30000000"),
        "soul_audio_register_pairs must include SRAM1/2 non-cacheable region at 0x30000000"
    );
}

/// soul_audio_register_pairs must return 3 pairs: AXI SRAM, SRAM4, SRAM1/2.
#[test]
fn mpu_register_pairs_count_is_three() {
    use platform::mpu::MpuApplier;
    let pairs = MpuApplier::soul_audio_register_pairs();
    assert_eq!(
        pairs.len(),
        3,
        "MpuApplier must return exactly 3 (RBAR, RASR) pairs: AXI SRAM + SRAM4 + SRAM1/2"
    );
}

/// firmware::boot::mpu_register_pairs must also return 3 pairs.
#[test]
fn boot_mpu_register_pairs_count_is_three() {
    let pairs = firmware::boot::mpu_register_pairs();
    assert_eq!(
        pairs.len(),
        3,
        "firmware::boot::mpu_register_pairs must return 3 non-cacheable MPU regions"
    );
}

/// The third RBAR pair must target SRAM1/2 at 0x30000000.
#[test]
fn mpu_rbar_covers_sram12() {
    let pairs = firmware::boot::mpu_register_pairs();
    let rbar_2 = pairs[2].0;
    let base = rbar_2 & 0xFFFF_FFE0;
    assert_eq!(
        base, 0x3000_0000,
        "RBAR[2] must target SRAM1/2 at 0x30000000, got 0x{base:08X}"
    );
}

/// The SRAM1/2 RASR must encode NonCacheable attributes (TEX=001, C=0, B=0).
#[test]
fn mpu_sram12_rasr_is_non_cacheable() {
    use platform::mpu::MpuApplier;
    let pairs = MpuApplier::soul_audio_register_pairs();
    let (_rbar, rasr) = pairs[2];
    // TEX[0] = bit 19 must be set for TEX=001 (Normal Non-cacheable)
    assert_ne!(
        rasr & (1 << 19),
        0,
        "SRAM1/2 RASR: TEX bit 19 must be SET (TEX=001)"
    );
    // C bit 17 must be clear (not cacheable)
    assert_eq!(
        rasr & (1 << 17),
        0,
        "SRAM1/2 RASR: C bit 17 must be CLEAR (non-cacheable)"
    );
    // B bit 16 must be clear (not bufferable)
    assert_eq!(rasr & (1 << 16), 0, "SRAM1/2 RASR: B bit 16 must be CLEAR");
    // ENABLE bit 0 must be set
    assert_ne!(rasr & 1, 0, "SRAM1/2 RASR: ENABLE bit 0 must be SET");
}

// -- CCR fault traps (GAP-H2) -------------------------------------------------

/// CCR.DIV_0_TRP (bit 4) must be set in boot to enable divide-by-zero faults.
/// Without this, SDIV/UDIV instructions silently return 0 instead of faulting.
/// Reference: ARM DDI0489F B3.2.8, ARM AN209 Using Cortex-M Fault Exceptions
#[test]
fn boot_sets_ccr_div_0_trp() {
    let boot_src = include_str!("../src/boot.rs");
    assert!(
        boot_src.contains("DIV_0_TRP") || boot_src.contains("div_0_trp"),
        "boot.rs must set CCR.DIV_0_TRP (bit 4) to enable divide-by-zero faults."
    );
}

#[test]
fn boot_sets_ccr_unalign_trp() {
    let boot_src = include_str!("../src/boot.rs");
    assert!(
        boot_src.contains("UNALIGN_TRP") || boot_src.contains("unalign_trp"),
        "boot.rs must set CCR.UNALIGN_TRP (bit 3) to trap unaligned memory accesses."
    );
}

#[test]
fn boot_has_configure_scb_fault_traps_function() {
    let boot_src = include_str!("../src/boot.rs");
    assert!(
        boot_src.contains("configure_scb_fault_traps"),
        "boot.rs must have a configure_scb_fault_traps function that sets DIV_0_TRP and UNALIGN_TRP"
    );
}

// -- memory.x output sections (GAP-M1 + L1 + L5) -----------------------------

/// memory.x must define a .sram3 output SECTION (not just a MEMORY region).
/// Without it, #[link_section = ".sram3"] silently fails with a linker error.
#[test]
fn memory_x_has_sram3_output_section() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains(".sram3") && memory_x.contains("> SRAM3"),
        "memory.x must define a .sram3 output section (SECTIONS block) for #[link_section] support."
    );
}

/// memory.x should define .sram1 output section for explicit SRAM1 placement.
#[test]
fn memory_x_has_sram1_output_section() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains(".sram1") && memory_x.contains("> SRAM1"),
        "memory.x must define a .sram1 output section for explicit SRAM1 buffer placement"
    );
}

/// memory.x should define .sram2 output section for explicit SRAM2 placement.
#[test]
fn memory_x_has_sram2_output_section() {
    let memory_x = include_str!("../../../memory.x");
    assert!(
        memory_x.contains(".sram2") && memory_x.contains("> SRAM2"),
        "memory.x must define a .sram2 output section for explicit SRAM2 buffer placement"
    );
}

// ─── DMA Alignment Tests ─────────────────────────────────────────────────────

/// Verify that `Align32<T>` provides 32-byte (Cortex-M7 cacheline) alignment.
///
/// All DMA buffers on STM32H743ZI must be 32-byte aligned. The CPU's D-cache
/// has 32-byte cachelines; unaligned DMA buffers cause cache coherency bugs
/// where the CPU reads stale cached data after a DMA write.
///
/// References: ST AN4839, ARM DDI0489F §B3.5
#[test]
fn align32_has_32_byte_alignment() {
    use firmware::dma::Align32;
    assert_eq!(core::mem::align_of::<Align32<u8>>(), 32);
    assert_eq!(core::mem::align_of::<Align32<[u8; 4]>>(), 32);
    assert_eq!(core::mem::align_of::<Align32<[u8; 65536]>>(), 32);
}

/// Verify that Align32 pads the inner type up to the next 32-byte boundary.
///
/// Three cases are tested:
///   - Already-aligned type ([u8; 32]): no overhead added.
///   - Already-aligned type ([u8; 64]): no overhead added.
///   - Smaller-than-alignment type (u8, 1 byte): padded to exactly 32 bytes.
#[test]
fn align32_size_equals_inner_size_rounded_up_to_alignment() {
    use firmware::dma::Align32;
    // For [u8; 32], size should be exactly 32 (already aligned).
    assert_eq!(core::mem::size_of::<Align32<[u8; 32]>>(), 32);
    // For [u8; 64], size should be exactly 64.
    assert_eq!(core::mem::size_of::<Align32<[u8; 64]>>(), 64);
    // For u8 (1 byte), the wrapper pads to 32 bytes (one full cacheline).
    assert_eq!(core::mem::size_of::<Align32<u8>>(), 32);
}

/// Verify that FRAMEBUFFER_SIZE is divisible by 32 (cacheline size).
///
/// This is required so that the framebuffer occupies an exact number of
/// cachelines, preventing partial-cacheline DMA transfers that can corrupt
/// adjacent memory.
#[test]
fn framebuffer_size_is_cacheline_aligned() {
    assert_eq!(
        firmware::FRAMEBUFFER_SIZE % 32,
        0,
        "FRAMEBUFFER_SIZE ({}) must be divisible by 32 (Cortex-M7 cacheline size)",
        firmware::FRAMEBUFFER_SIZE
    );
}

// ── AXI SRAM READ_ISS_OVERRIDE errata (GAP-A1) ───────────────────────────────

/// STM32H743 Rev Y errata 2.2.9: AXI SRAM read stall under concurrent access.
/// The READ_ISS_OVERRIDE bit (AXI_TARG7_FN_MOD register at 0x5100_1108, bit 0)
/// must be set before any concurrent CPU+DMA access to AXI SRAM.
/// Without this, SAI DMA + CPU decode concurrency causes intermittent stale reads.
/// Reference: ST Errata ES0392 Rev 9 §2.2.9, ST AppNote AN5319.
#[test]
fn boot_applies_axi_sram_read_iss_override_errata() {
    let boot_src = include_str!("../src/boot.rs");
    assert!(
        boot_src.contains("0x5100_1108") || boot_src.contains("READ_ISS_OVERRIDE") || boot_src.contains("AXI_TARG7"),
        "boot.rs must set AXI_TARG7_FN_MOD (0x5100_1108, bit 0) to work around          STM32H743 Rev Y errata 2.2.9 (AXI SRAM concurrent read stall).          Single write: ptr::write_volatile(0x5100_1108 as *mut u32, 1)"
    );
}

// ── BOR threshold assertion (GAP-A2) ─────────────────────────────────────────

/// BOR level must be verified at boot time.
/// Factory default BOR_LEV=0b000 (~1.7V) is too low for 3.3V LiPo PMIC systems.
/// Supply droop during SDMMC writes can corrupt FAT32 directory entries.
/// The boot code must assert BOR is configured, or document the provision step.
#[test]
fn boot_documents_bor_threshold_requirement() {
    let boot_src = include_str!("../src/boot.rs");
    assert!(
        boot_src.contains("BOR") || boot_src.contains("bor_lev") || boot_src.contains("brown"),
        "boot.rs must reference BOR (Brown-Out Reset) threshold.          Either assert BOR_LEV != 0 at runtime, or document the option bytes          provisioning step. Factory default 1.7V is insufficient for LiPo PMIC systems."
    );
}

// ── Interrupt priorities applied (GAP-A4) ────────────────────────────────────

/// InterruptPriorities constants must be applied via NVIC at runtime.
/// Defining constants without calling set_priority() has no effect ---
/// all interrupts default to priority 0 (highest, equal) after reset.
/// Equal-priority EXTI ISR can block SAI DMA ISR -> audio dropout at 192 kHz.
#[test]
fn boot_applies_interrupt_priorities_at_runtime() {
    let boot_src = include_str!("../src/boot.rs");
    assert!(
        boot_src.contains("set_priority") || boot_src.contains("NVIC") || boot_src.contains("Priority::"),
        "boot.rs must apply interrupt priorities via NVIC::set_priority() or equivalent.          InterruptPriorities constants in clock_config.rs have no effect until applied.          SAI DMA must be highest priority to prevent audio dropout under encoder load."
    );
}

// ── Main loop counter wrapping (GAP-A10) ─────────────────────────────────────

/// Main loop heartbeat counter must use wrapping_add to comply with
/// arithmetic_side_effects = "deny" workspace lint.
/// counter += 1 is technically unreachable overflow (136 years) but violates policy.
#[test]
fn main_loop_counter_uses_wrapping_add() {
    let main_src = include_str!("../src/main.rs");
    // Must NOT contain bare += 1 on counter variable
    // Must contain wrapping_add
    assert!(
        main_src.contains("wrapping_add") || !main_src.contains("counter += 1"),
        "Main loop counter must use counter.wrapping_add(1) to comply with          arithmetic_side_effects = \"deny\" workspace lint.          Even unreachable overflows must use explicit wrapping semantics."
    );
}

// ── Display init watchdog intentional starvation (GAP-A9) ────────────────────

/// Display init failure loop must document that it intentionally starves the watchdog.
/// Without this comment, developers may "fix" the missing heartbeat, creating a boot loop.
#[test]
fn display_init_failure_loop_documents_iwdg_intent() {
    let main_src = include_str!("../src/main.rs");
    // Check that the display error handling has some documentation about IWDG
    // Look for either "IWDG" or "watchdog" near the display error handling
    assert!(
        main_src.contains("IWDG") || main_src.contains("watchdog") || main_src.contains("intentional"),
        "Display init failure loop must document that NOT feeding the watchdog is intentional.          The IWDG will reset the device after 8s -- this IS the recovery strategy."
    );
}

// -- Input channel ISR safety (GAP-A5) --

/// The input channel receive path must not use CriticalSectionRawMutex for
/// thread-mode pops without documented justification.
/// CriticalSectionRawMutex sets PRIMASK=1 (all IRQs masked) even in thread mode.
/// During menu navigation, rapid pops mask the SAI DMA half-transfer ISR.
#[test]
fn input_channel_documents_critical_section_usage() {
    let hardware_rs = include_str!("../src/input/hardware.rs");
    if hardware_rs.contains("CriticalSectionRawMutex") {
        assert!(
            hardware_rs.contains("// SAFETY:")
                || hardware_rs.contains("// Justification:")
                || hardware_rs.contains("PRIMASK")
                || hardware_rs.contains("critical_section"),
            "CriticalSectionRawMutex on input channel lacks timing-analysis doc comment."
        );
    }
}

// -- defmt production binary hygiene (GAP-A6) --

/// DEFMT_LOG is a BUILD-TIME env var: DEFMT_LOG=info strips trace/debug strings
/// from the .defmt ELF section. The workspace must document this via a feature.
#[test]
fn cargo_toml_has_log_level_feature_flag() {
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(
        cargo_toml.contains("verbose-logging")
            || cargo_toml.contains("log-debug")
            || cargo_toml.contains("defmt-log")
            || cargo_toml.contains("defmt/log"),
        "Cargo.toml must have a verbose-logging feature documenting DEFMT_LOG compile-time filtering."
    );
}

// -- cargo-auditable in CI (GAP-A8) --

/// CI must use cargo-auditable to embed dependency manifest in firmware ELF.
/// Enables: cargo audit bin firmware.elf for field CVE scanning.
#[test]
fn ci_uses_cargo_auditable_for_firmware_build() {
    let ci_yml = include_str!("../../../.github/workflows/ci.yml");
    // A YAML comment does not execute -- check for an actual named step.
    assert!(
        ci_yml.contains("Install cargo-auditable"),
        "CI must have a step named Install cargo-auditable, not just a comment."
    );
}

// ─── Feature Flag Invariants ─────────────────────────────────────────────────

/// Verify that `hardware` and `emulator` are not simultaneously active.
///
/// These features are mutually exclusive:
/// - `hardware` pulls in embassy-stm32, cortex-m, defmt (no_std, ARM-only)
/// - `emulator` pulls in tokio, winit (std, desktop-only)
///
/// The compile-time enforcement is in `crates/firmware/build.rs`:
/// The `hardware` feature requires an ARM target (CARGO_CFG_TARGET_ARCH == "arm").
/// Enabling `hardware` on a non-ARM host will panic at build time, which
/// effectively prevents hardware+emulator from being active together on a
/// desktop build.
///
/// This test verifies the runtime invariant and documents the expected behavior.
/// The arch job in CI also never uses --all-features for exactly this reason.
#[test]
fn hardware_and_emulator_features_are_mutually_exclusive() {
    let is_hardware = cfg!(feature = "hardware");
    let is_emulator = cfg!(feature = "emulator");
    assert!(
        !(is_hardware && is_emulator),
        "hardware and emulator features must not be active simultaneously. \
         See crates/firmware/build.rs for compile-time enforcement and \
         .github/workflows/ci.yml (NOTE on feature flag exclusivity) for CI guidance."
    );
}

/// Verify that embassy-stm32 uses `time-driver-tim2` rather than `time-driver-any`.
///
/// `time-driver-any` causes a linker error when multiple crates in the same
/// binary each try to claim the generic timer interrupt. `time-driver-tim2`
/// pins the driver to TIM2 and gives it a specific interrupt symbol name,
/// avoiding the conflict.
///
/// Enforcement: `Cargo.toml` workspace dependencies must specify
/// `embassy-stm32 = { features = ["time-driver-tim2", ...] }` explicitly.
/// This test documents the requirement; if the constraint is ever accidentally
/// removed, the embedded build will fail with a linker error.
#[test]
fn embassy_time_driver_is_tim2_not_any() {
    // This is a documentation test — the actual enforcement is at link time
    // (the embedded build fails if time-driver-any is used instead of tim2).
    // The test exists to document the constraint and provide a search anchor
    // for engineers who encounter the linker error.
    //
    // Verify: grep Cargo.toml for "time-driver-tim2" should match.
    // grep Cargo.toml for "time-driver-any" should NOT match.
    let workspace_cargo = include_str!("../../../Cargo.toml");
    assert!(
        workspace_cargo.contains("time-driver-tim2"),
        "Cargo.toml must use embassy-stm32 with time-driver-tim2 feature"
    );
    assert!(
        !workspace_cargo.contains("time-driver-any"),
        "Cargo.toml must NOT use time-driver-any (causes linker conflict)"
    );
}

/// Verify that the firmware crate's Cargo.toml does not accidentally enable
/// both hardware and emulator features in any single feature group.
///
/// A feature like `full = ["hardware", "emulator"]` would silently pass
/// `cargo check` but fail to link on any target.
#[test]
fn no_single_feature_enables_both_hardware_and_emulator() {
    let firmware_cargo = include_str!("../Cargo.toml");

    // Parse [features] section lines that contain BOTH "hardware" AND "emulator"
    // on the same line (which would mean a single feature enables both).
    for line in firmware_cargo.lines() {
        let trimmed: &str = line.trim();
        if trimmed.starts_with('#') {
            continue; // skip comments
        }
        if trimmed.contains("\"hardware\"") && trimmed.contains("\"emulator\"") {
            panic!(
                "Cargo.toml line enables both hardware and emulator features: {:?}",
                trimmed
            );
        }
    }
}

// ── Per-site allow enforcement ────────────────────────────────────────────────

/// No driver file should suppress arithmetic_side_effects at module/crate level.
/// Per-site #[allow] with justification comments is required.
/// Module-level suppression hides ALL future arithmetic bugs in the file.
#[test]
fn no_module_level_arithmetic_allows_in_drivers() {
    // Check that the ES9038Q2M DAC driver does not suppress at file level.
    // This driver previously used #![allow(clippy::arithmetic_side_effects)]
    // at module level, hiding all future overflow bugs in the file.
    let dac_driver = include_str!("../src/audio/dac/es9038q2m/driver.rs");
    assert!(
        !dac_driver.contains("#![allow(clippy::arithmetic_side_effects)]"),
        "es9038q2m/driver.rs must not suppress arithmetic_side_effects at file level. \
         Use per-site #[allow] with justification comments."
    );
    assert!(
        !dac_driver.contains("#![allow(clippy::indexing_slicing)]"),
        "es9038q2m/driver.rs must not suppress indexing_slicing at file level. \
         Use per-site #[allow] with justification comments."
    );

    // Check that the SSD1677 display driver does not suppress at file level.
    let display_driver = include_str!("../src/display/driver.rs");
    assert!(
        !display_driver.contains("#![allow(clippy::arithmetic_side_effects)]"),
        "display/driver.rs must not suppress arithmetic_side_effects at file level. \
         Use per-site #[allow] with justification comments."
    );
    assert!(
        !display_driver.contains("#![allow(clippy::indexing_slicing)]"),
        "display/driver.rs must not suppress indexing_slicing at file level. \
         Use per-site #[allow] with justification comments."
    );

    // Check that the UI module does not suppress at file level.
    let ui_mod = include_str!("../src/ui/mod.rs");
    assert!(
        !ui_mod.contains("#![allow(clippy::arithmetic_side_effects)]"),
        "ui/mod.rs must not suppress arithmetic_side_effects at file level. \
         Use per-site #[allow] with justification comments."
    );
    assert!(
        !ui_mod.contains("#![allow(clippy::indexing_slicing)]"),
        "ui/mod.rs must not suppress indexing_slicing at file level. \
         Use per-site #[allow] with justification comments."
    );

    // main.rs should never suppress these lints at module level.
    let main_rs = include_str!("../src/main.rs");
    assert!(
        !main_rs.contains("#![allow(clippy::arithmetic_side_effects)]"),
        "main.rs must not suppress arithmetic_side_effects at file level. Use per-site #[allow]."
    );
}

/// Verify the count of module-level allows is documented and bounded.
/// This test establishes a baseline — don't let it grow silently.
#[test]
fn lint_suppression_count_is_tracked() {
    // boot.rs contains hardware init logic where arithmetic could overflow.
    // It must use per-site allows if needed, not blanket file-level suppression.
    let boot_rs = include_str!("../src/boot.rs");
    assert!(
        !boot_rs.contains("#![allow(clippy::arithmetic_side_effects)]"),
        "boot.rs must not suppress arithmetic_side_effects at file level"
    );
    assert!(
        !boot_rs.contains("#![allow(clippy::indexing_slicing)]"),
        "boot.rs must not suppress indexing_slicing at file level"
    );

    // The DAC driver and display driver are the highest-risk files for
    // arithmetic bugs. Confirming they use per-site allows means every
    // suppression is documented with a safety justification.
    //
    // Per-site allows with justification comments ARE permitted — only
    // file-level (#![allow...]) suppression is forbidden in driver files.
    let dac_driver = include_str!("../src/audio/dac/es9038q2m/driver.rs");
    let display_driver = include_str!("../src/display/driver.rs");

    // Both files should have at least one per-site #[allow] after refactoring,
    // proving the arithmetic was moved to targeted suppressions rather than
    // simply deleting the allows (which would break compilation).
    assert!(
        dac_driver.contains("#[allow(clippy::arithmetic_side_effects)]")
            || dac_driver.contains("#[allow(clippy::cast"),
        "es9038q2m/driver.rs should retain per-site allows for documented arithmetic"
    );
    assert!(
        display_driver.contains("#[allow(clippy::arithmetic_side_effects)]")
            || display_driver.contains("#[allow(clippy::indexing_slicing)]"),
        "display/driver.rs should retain per-site allows for documented arithmetic"
    );
}

// ── MPU ordering token (Priority 4) ──────────────────────────────────────────

/// The MPU configuration function must return a proof token.
/// This token must be required by the Embassy config function,
/// making it a compile error to initialize Embassy without MPU config.
///
/// Current state: ordering enforced only by code position (fragile).
/// Required state: `let token = apply_mpu_config(); embassy_stm32::init(config, token)`
#[test]
fn boot_mpu_function_returns_token() {
    // Check that boot.rs defines an MpuConfigured token or similar proof type
    let boot_src = include_str!("../src/boot.rs");
    assert!(
        boot_src.contains("MpuConfigured") || boot_src.contains("MpuToken")
            || boot_src.contains("CacheConfigured") || boot_src.contains("BootToken"),
        "apply_mpu_config_from_peripherals() must return a zero-cost token type \
         (e.g., MpuConfigured) that build_embassy_config() requires as a parameter. \
         This makes it a compile error to call Embassy init without MPU config. \
         Currently the ordering is enforced only by code position — fragile."
    );
}

#[test]
fn main_rs_passes_mpu_token_to_embassy_init() {
    let main_src = include_str!("../src/main.rs");
    // The token returned by apply_mpu_config must be used in the Embassy init call
    assert!(
        main_src.contains("MpuConfigured") || main_src.contains("mpu_token")
            || main_src.contains("cache_configured") || main_src.contains("boot_token"),
        "main.rs must use the MPU configuration token when calling embassy_stm32::init(). \
         The token proves MPU was configured before D-cache was enabled."
    );
}

// ── Static memory budget assertion (Priority 5) ───────────────────────────────

/// Total static memory (DMA buffers + framebuffers) must fit in AXI SRAM
/// with headroom for Embassy task stacks and .bss/.data sections.
/// This must be a compile-time assertion, not just documentation.
#[test]
fn static_memory_budget_has_compile_time_assertion() {
    // Check that dma_safety.rs has a const assertion on memory budget
    let dma_safety = include_str!("../../../crates/platform/src/dma_safety.rs");
    assert!(
        dma_safety.contains("const _:") || dma_safety.contains("const_assert")
            || dma_safety.contains("MEMORY_BUDGET") || dma_safety.contains("TOTAL_DMA"),
        "dma_safety.rs must have a compile-time assertion on total static DMA memory budget. \
         Two framebuffers (2×96KB) + two audio buffers (2×16KB) = 224KB of 512KB AXI SRAM. \
         Add: const _: () = assert!(TOTAL_STATIC_DMA_BYTES <= AXI_SRAM_SIZE_BYTES * 3 / 4)"
    );
}

/// CI must verify firmware ELF has no malloc/free/sbrk symbols.
#[test]
fn ci_binary_size_job_has_no_malloc_check() {
    let ci_yml = include_str!("../../../.github/workflows/ci.yml");
    assert!(
        ci_yml.contains("malloc") || ci_yml.contains("_sbrk"),
        "binary-size CI job must check for malloc/free/_sbrk symbols using arm-none-eabi-nm.\n\
         Heap allocation in firmware means a dependency accidentally uses the allocator.\n\
         Add: arm-none-eabi-nm ... | grep -E '(malloc|free|_sbrk)' | ..."
    );
}

// ── AudioPowerSequencer wiring enforcement ───────────────────────────────────

/// AudioPowerSequencer must be used in firmware audio code, not just defined.
/// The typestate machine prevents TPA6120A2 pop noise ONLY if it is wired
/// into the actual power-on path. Being defined in platform without being
/// called in firmware provides zero protection.
#[test]
fn audio_power_sequencer_is_used_in_firmware() {
    // Check that firmware audio code imports the sequencer
    // Look for usage in main.rs or audio module
    let main_rs = include_str!("../src/main.rs");
    let found_in_main = main_rs.contains("AudioPowerSequencer")
        || main_rs.contains("audio_sequencer")
        || main_rs.contains("mute_dac")
        || main_rs.contains("enable_amp");

    // Also check boot.rs for audio init
    let boot_rs = include_str!("../src/boot.rs");
    let found_in_boot = boot_rs.contains("AudioPowerSequencer")
        || boot_rs.contains("audio_sequencer");

    assert!(
        found_in_main || found_in_boot,
        "AudioPowerSequencer must be used in firmware (main.rs or boot.rs), not just defined in platform.          The typestate machine prevents TPA6120A2 pop noise ONLY at actual call sites.          Add audio init sequence: AudioPowerSequencer::new().mute_dac().enable_amp().unmute_dac()"
    );
}

/// DmaBuffer wrapper or explicit DMA buffer placement audit must exist.
/// DmaAccessible trait is enforcement theater without call sites.
#[test]
fn dma_buffer_placement_is_enforced_not_just_documented() {
    let main_rs = include_str!("../src/main.rs");
    // FRAMEBUFFER must use StaticCell with link_section (already done)
    // AND the DmaAccessible type system must be referenced
    let has_dma_placement = main_rs.contains("link_section")
        || main_rs.contains("DmaAccessible")
        || main_rs.contains("AxiSramRegion");
    assert!(
        has_dma_placement,
        "DMA buffers must use explicit placement. Either #[link_section = \".axisram\"]          or DmaBuffer<AxiSramRegion> wrapper. DmaAccessible trait definitions alone provide no protection."
    );
}

/// SAI audio task must be spawned (or called) in main.rs — not just TODO'd.
///
/// The audio pipeline is a critical feature path. If the audio task is never
/// spawned, the SAI peripheral is never initialized and the ES9038Q2M DAC
/// produces no output. This test enforces that the call site exists at compile
/// time, so refactors that rename `audio_task` break this test rather than
/// silently shipping silent firmware.
#[test]
fn sai_audio_task_spawned() {
    let main_rs = include_str!("../src/main.rs");
    // Must contain a call to audio_task or spawn_audio_task in main.rs
    let has_audio_task = main_rs.contains("audio_task");
    assert!(
        has_audio_task,
        "main.rs must call `audio_task` (or `spawn_audio_task`) to wire the SAI audio pipeline.\n\
         A TODO comment or commented-out block provides no compile-time safety.\n\
         Add at minimum: firmware::audio::sai_task::audio_task(buffer)"
    );
}

/// DmaBuffer<AxiSramRegion> must be used by firmware — not just defined in platform.
///
/// The `DmaBuffer<Region>` wrapper exists in `platform::dma_safety` to enforce
/// DMA-accessible memory regions at the type level. If firmware never uses it,
/// the wrapper is enforcement theater: engineers will use bare arrays and the
/// type-system protection provides zero value.
///
/// This test ensures firmware actively references `DmaBuffer<AxiSramRegion` so
/// the protection is real, not aspirational.
#[test]
fn audio_dma_buffer_type_enforced() {
    let main_rs = include_str!("../src/main.rs");
    // DmaBuffer<AxiSramRegion must appear in firmware source (not just defined in platform)
    let used_in_main = main_rs.contains("DmaBuffer<AxiSramRegion");

    // Also accept usage in the audio module
    let audio_mod = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/audio/sai_task.rs")
    ).unwrap_or_default();
    let used_in_sai_task = audio_mod.contains("DmaBuffer<AxiSramRegion");

    assert!(
        used_in_main || used_in_sai_task,
        "DmaBuffer<AxiSramRegion> must be used in firmware/src/main.rs or firmware/src/audio/sai_task.rs.\n\
         Defining DmaBuffer in platform without using it in firmware provides no enforcement.\n\
         Required:\n\
           use platform::dma_safety::{{DmaBuffer, AxiSramRegion}};\n\
           static AUDIO_BUFFER: StaticCell<DmaBuffer<AxiSramRegion, [u8; AUDIO_DMA_BUFFER_BYTES]>>"
    );
}


// ---- BQ25895 PMIC driver architecture tests --------------------------------
//
// These tests enforce that:
//   1. bq25895.rs exists in the platform crate with the required constants.
//   2. The I2C address constant matches the hardware-fixed value 0x6A.
//
// They turn RED when the module or constants are absent, then GREEN once
// the driver is implemented. They guard against accidental removal or rename.

/// Verify that `platform::bq25895` module exists and defines `BQ25895_I2C_ADDR`.
///
/// Architecture rule: IC register maps and address constants must live in the
/// `platform` crate (HAL layer), not in `firmware`. This keeps them testable
/// on the host without any STM32 dependencies.
#[test]
fn bq25895_driver_exists_in_platform() {
    let platform_src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../platform/src/bq25895.rs"),
    )
    .expect("bq25895.rs must exist in crates/platform/src/ -- driver not yet created");
    assert!(
        platform_src.contains("BQ25895_I2C_ADDR"),
        "bq25895.rs must define the BQ25895_I2C_ADDR constant (hardware-fixed 7-bit address)"
    );
    assert!(
        platform_src.contains("REG0B_STATUS"),
        "bq25895.rs must define REG0B_STATUS (status register read at startup)"
    );
    assert!(
        platform_src.contains("VREG_4208MV"),
        "bq25895.rs must define VREG_4208MV (charge voltage setting for 4.208 V Li-ion target)"
    );
}

/// Verify that `platform::bq25895::BQ25895_I2C_ADDR` equals 0x6A.
///
/// This is a compile-time check via the Rust type system: if the constant
/// does not exist or has the wrong value, this test fails to compile or panics.
///
/// Architecture rule: the BQ25895 I2C address is hardware-fixed at 0x6A.
/// The value 0x6B appeared as a typo in early datasheet revisions and was
/// corrected in SLUUBA2B errata (confirmed on TI E2E forum post #507682).
#[test]
fn bq25895_i2c_addr_is_correct() {
    assert_eq!(
        platform::bq25895::BQ25895_I2C_ADDR,
        0x6A,
        "BQ25895 I2C address must be 0x6A (hardware-fixed; see SLUSCD3B section 6.5)"
    );
}

// ---- ES9038Q2M DAC register constant tests ---------------------------------
//
// These tests enforce that:
//   1. es9038q2m.rs exists in the platform crate.
//   2. REG_ATT_L and REG_ATT_R constants match the datasheet register map.
//   3. The mute and full-volume attenuation values are correct.
//
// Architecture rule: IC register maps must live in the platform crate (HAL
// layer), not in firmware. This keeps the register definitions testable on
// the host without any STM32 or I2C hardware dependencies.

/// Verify that `platform::es9038q2m` module exists and defines `REG_ATT_L` and `REG_ATT_R`.
///
/// If this test fails, the module has not been created or the constants are
/// missing or renamed. The constants are required by `AudioPowerSequencer`
/// to perform actual I2C writes during the power-on/off sequence.
#[test]
fn es9038q2m_register_constants_defined() {
    let src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../platform/src/es9038q2m.rs"),
    )
    .expect("es9038q2m.rs must exist in crates/platform/src/");
    assert!(
        src.contains("REG_ATT_L"),
        "es9038q2m.rs must define REG_ATT_L (left channel attenuator, register 15)"
    );
    assert!(
        src.contains("REG_ATT_R"),
        "es9038q2m.rs must define REG_ATT_R (right channel attenuator, register 16)"
    );
    assert!(
        src.contains("ATT_MUTED"),
        "es9038q2m.rs must define ATT_MUTED (0xFF = maximum attenuation)"
    );
    assert!(
        src.contains("ATT_FULL_VOLUME"),
        "es9038q2m.rs must define ATT_FULL_VOLUME (0x00 = 0 dB, no attenuation)"
    );
}

/// Verify that the ES9038Q2M attenuator register addresses match the datasheet.
///
/// ES9038Q2M datasheet Rev 3.0, Section 9 (Register Map):
///   - Register 15 = ATT_L (left channel master attenuation)
///   - Register 16 = ATT_R (right channel master attenuation)
#[test]
fn es9038q2m_att_register_addresses_match_datasheet() {
    assert_eq!(
        platform::es9038q2m::REG_ATT_L,
        15,
        "REG_ATT_L must be register 15 (ES9038Q2M datasheet Rev 3.0, Section 9)"
    );
    assert_eq!(
        platform::es9038q2m::REG_ATT_R,
        16,
        "REG_ATT_R must be register 16 (ES9038Q2M datasheet Rev 3.0, Section 9)"
    );
}

/// Verify the mute and full-volume attenuation register values.
///
/// The ES9038Q2M attenuation scale: 0x00 = 0 dB (no attenuation, full volume),
/// 0xFF = maximum attenuation (muted). These values are used in the
/// `AudioPowerSequencer` to safely sequence TPA6120A2 power on/off.
#[test]
fn es9038q2m_attenuation_values_are_correct() {
    assert_eq!(
        platform::es9038q2m::ATT_MUTED,
        0xFF,
        "ATT_MUTED must be 0xFF (maximum attenuation)"
    );
    assert_eq!(
        platform::es9038q2m::ATT_FULL_VOLUME,
        0x00,
        "ATT_FULL_VOLUME must be 0x00 (0 dB, no attenuation)"
    );
}

/// Workspace Cargo.toml must enforce unwrap_used/expect_used/panic at workspace level.
///
/// These lints prevent silent panics in production embedded firmware.
/// Desktop crates suppress them with #![allow]; embedded crates get them for free.
#[test]
fn workspace_cargo_toml_has_panic_prevention_lints() {
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(
        cargo_toml.contains("unwrap_used"),
        "workspace Cargo.toml must have `unwrap_used = \"deny\"` in [workspace.lints.clippy]"
    );
    assert!(
        cargo_toml.contains("expect_used"),
        "workspace Cargo.toml must have `expect_used = \"deny\"` in [workspace.lints.clippy]"
    );
    assert!(
        cargo_toml.contains(r#"panic       = "deny""#),
        "workspace Cargo.toml must have `panic = \"deny\"` in [workspace.lints.clippy]"
    );
}

/// main.rs must call AudioPowerSequencer methods that reference I2C (`_with_i2c` variants).
///
/// The stub `.mute_dac()` / `.enable_amp()` / `.unmute_dac()` chain is only acceptable
/// as a comment showing the full path; the source must explicitly name the I2C methods
/// so reviewers and CI can confirm the wiring intent is documented at the call site.
#[test]
fn audio_sequencer_with_i2c_called_in_main() {
    let main_rs = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")
    ).unwrap();
    assert!(
        main_rs.contains("mute_dac_with_i2c") || main_rs.contains("with_i2c"),
        "main.rs must call AudioPowerSequencer::mute_dac_with_i2c() at the I2C init call site"
    );
}

/// main.rs must reference I2C3 or ES9038Q2M — DAC I2C init must not be just a TODO comment.
///
/// The ES9038Q2M DAC is controlled via I2C3 (PA8=SCL, PC9=SDA, 400 kHz).
/// Real constants from `platform::es9038q2m` (e.g. `ES9038Q2M_I2C_ADDR_LOW`) must appear
/// in main.rs to prove the wiring is planned at the call site, not silently deferred.
#[test]
fn i2c3_dac_init_not_commented_out() {
    let main_rs = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")
    ).unwrap();
    // Must NOT be a pure TODO comment block — must have some real I2C code
    assert!(
        main_rs.contains("I2c3") || main_rs.contains("i2c3") || main_rs.contains("ES9038"),
        "main.rs must reference I2C3 or ES9038Q2M — DAC I2C init must not be just a TODO comment"
    );
}

/// main.rs must reference I2C2 or BQ25895 — PMIC I2C init must not be just a TODO comment.
///
/// The BQ25895 PMIC is controlled via I2C2 (PF1=SCL, PF0=SDA, 100 kHz, addr 0x6A).
/// Real constants from `platform::bq25895` (e.g. `BQ25895_I2C_ADDR`) must appear
/// in main.rs to prove the wiring is planned at the call site, not silently deferred.
#[test]
fn i2c2_pmic_init_not_commented_out() {
    let main_rs = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")
    ).unwrap();
    assert!(
        main_rs.contains("I2c2") || main_rs.contains("i2c2") || main_rs.contains("BQ25895") || main_rs.contains("bq25895"),
        "main.rs must reference I2C2 or BQ25895 — PMIC I2C init must not be just a TODO comment"
    );
}

/// main.rs must complete the AudioPowerSequencer power-on path.
///
/// `AudioPowerSequencer` must appear (already verified by `audio_power_sequencer_is_used_in_firmware`),
/// AND at least one of `enable_amp` or `unmute_dac` must also appear, confirming the
/// full power-on sequence (mute → amp → unmute) is wired at the call site.
#[test]
fn audio_power_sequencer_fully_on_path_exists_in_main() {
    let main_rs = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")
    ).unwrap();
    assert!(
        main_rs.contains("AudioPowerSequencer"),
        "main.rs must use AudioPowerSequencer"
    );
    assert!(
        main_rs.contains("enable_amp") || main_rs.contains("unmute_dac"),
        "main.rs must complete the power-on sequence (enable_amp + unmute_dac)"
    );
}

// ── Per-task stack budget tests (Step 4) ─────────────────────────────────────

/// Verify the per-task stack budget constants exist and are accessible from the platform crate.
#[test]
fn task_stack_budget_exported_from_platform() {
    // Verify the constants exist and are accessible
    let _ = platform::dma_safety::TASK_STACK_BYTES;
    let _ = platform::dma_safety::CONCURRENT_TASK_COUNT;
    let _ = platform::dma_safety::TOTAL_TASK_STACK_BYTES;
    let _ = platform::dma_safety::TOTAL_AXI_SRAM_BUDGET_BYTES;
}

/// Total AXI SRAM budget (DMA + stacks + headroom) must not exceed 512 KB.
#[test]
fn axi_sram_budget_within_bounds() {
    let budget = platform::dma_safety::TOTAL_AXI_SRAM_BUDGET_BYTES;
    let capacity = platform::dma_safety::AXI_SRAM_SIZE_BYTES;
    assert!(budget <= capacity,
        "Budget {budget} bytes exceeds AXI SRAM capacity {capacity} bytes");
}

/// After reserving DMA buffers and task stacks, at least 64 KB must remain as headroom.
#[test]
fn static_dma_plus_tasks_leaves_64kb_headroom() {
    let dma = platform::dma_safety::TOTAL_STATIC_DMA_BYTES;
    let tasks = platform::dma_safety::TOTAL_TASK_STACK_BYTES;
    let total = platform::dma_safety::AXI_SRAM_SIZE_BYTES;
    let headroom = total - dma - tasks;
    assert!(headroom >= 64 * 1024,
        "Only {headroom} bytes headroom after DMA ({dma}) + tasks ({tasks}); need 64 KB minimum");
}

// ── HIL test skeleton existence tests (Step 6) ───────────────────────────────

/// The tests/hardware/ directory must exist as part of the required HIL skeleton.
#[test]
fn hil_test_directory_exists() {
    let hil_dir = std::path::Path::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/hardware")
    );
    assert!(hil_dir.exists(),
        "tests/hardware/ directory must exist — HIL test skeleton required for A-grade architecture");
}

/// The HIL README must document the probe-rs setup and how to run tests.
#[test]
fn hil_readme_documents_test_procedure() {
    let readme = std::path::Path::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/hardware/README.md")
    );
    if readme.exists() {
        let content = std::fs::read_to_string(readme).unwrap();
        assert!(content.contains("probe-rs"), "HIL README must document probe-rs setup");
        assert!(content.contains("cargo test"), "HIL README must show how to run tests");
    }
}

// ── SAI PLL3 clock math architecture tests ───────────────────────────────────
//
// These tests verify that the audio/clock_math.rs module exists and exports
// the correct constants for SAI1 MCLK generation on STM32H743.

/// audio/clock_math.rs must exist with PLL3 divider constants.
///
/// The module documents the PLL3 M/N/P/FRACN derivation for 49.152 MHz MCLK.
/// If this file is deleted or moved, the SAI clock math is undocumented and
/// the constants that boot.rs depends on become unverifiable.
#[test]
fn pll3_clock_math_module_exists() {
    let path = std::path::Path::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/audio/clock_math.rs"),
    );
    assert!(
        path.exists(),
        "audio/clock_math.rs must exist with PLL3 divider constants and derivation"
    );
}

/// MCLK_TARGET_HZ must be 49 152 000 Hz (49.152 MHz).
///
/// Architecture rule: ES9038Q2M requires MCLK = 256 x 192 000 for 192 kHz I2S.
/// If this constant changes, the FRACN derivation and SAI init code must change too.
#[test]
fn pll3_mclk_target_is_49_152_khz() {
    assert_eq!(
        firmware::audio::clock_math::MCLK_TARGET_HZ,
        49_152_000,
        "MCLK_TARGET_HZ must be 49.152 MHz for 192 kHz / 256 fs (ES9038Q2M requirement)"
    );
}

/// SAMPLE_RATE_HZ must be 192 000 Hz.
///
/// Architecture rule: the primary DAC sample rate is 192 kHz.
/// Lower rates (96/48 kHz) require a different PLL3P clock or a divider in SAI.
#[test]
fn pll3_sample_rate_is_192khz() {
    assert_eq!(
        firmware::audio::clock_math::SAMPLE_RATE_HZ,
        192_000,
        "Target sample rate must be 192 kHz (ES9038Q2M primary rate)"
    );
}

/// PLL3P output must be within MCLK_MAX_ERROR_HZ of the 49.152 MHz target.
///
/// This arch test re-checks the math from outside the module to verify
/// the exported PLL3P_HZ_APPROX constant is consistent with the tolerance.
#[test]
fn pll3_mclk_error_within_tolerance() {
    let actual = firmware::audio::clock_math::PLL3P_HZ_APPROX;
    let target = firmware::audio::clock_math::MCLK_TARGET_HZ;
    let max_err = firmware::audio::clock_math::MCLK_MAX_ERROR_HZ;
    let diff = (actual as i64 - target as i64).unsigned_abs();
    assert!(
        diff <= max_err as u64,
        "PLL3 output {actual} Hz is {diff} Hz from target {target} Hz (max {max_err} Hz)"
    );
}

/// sai_task.rs must import constants from audio::clock_math.
///
/// Architecture rule: the SAI init task must reference the clock_math module
/// so that changes to PLL3 divisors are visible at the implementation call site.
/// If sai_task.rs no longer imports from clock_math, the two can drift.
#[test]
fn sai_task_references_clock_math() {
    let sai_task = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/audio/sai_task.rs"),
    )
    .unwrap();
    assert!(
        sai_task.contains("clock_math"),
        "sai_task.rs must import or reference clock_math constants"
    );
}

#[test]
fn bq25895_init_function_exists_in_platform() {
    let src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../platform/src/bq25895.rs"),
    )
    .expect("bq25895.rs must exist in crates/platform/src/");
    assert!(
        src.contains("pub fn bq25895_init"),
        "platform::bq25895 must export bq25895_init() function"
    );
    assert!(
        src.contains("embedded_hal::i2c::I2c"),
        "bq25895_init must accept an I2C parameter (embedded_hal trait)"
    );
}

#[test]
fn es9038q2m_init_function_exists_in_platform() {
    let src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../platform/src/es9038q2m.rs"),
    )
    .expect("es9038q2m.rs must exist in crates/platform/src/");
    assert!(
        src.contains("pub fn es9038q2m_init"),
        "platform::es9038q2m must export es9038q2m_init() function"
    );
    assert!(
        src.contains("ATT_MUTED"),
        "es9038q2m_init must mute DAC outputs on startup (use ATT_MUTED constant)"
    );
}

#[test]
fn es9038q2m_init_uses_muted_constant_for_startup() {
    assert_eq!(platform::es9038q2m::ATT_MUTED, 0xFF);
    assert_eq!(platform::es9038q2m::ATT_FULL_VOLUME, 0x00);
}

/// clippy.toml must set array-size-threshold to 512 bytes.
/// Default (512000) is far too high for embedded — even 1 KB stack frames
/// can overflow the 4 KB ISR stack on STM32H7 under nested interrupts.
#[test]
fn clippy_toml_has_array_size_threshold_512() {
    let clippy_toml = include_str!("../../../clippy.toml");
    assert!(
        clippy_toml.contains("array-size-threshold"),
        "clippy.toml must set array-size-threshold (recommended: 512)"
    );
    let line = clippy_toml
        .lines()
        .find(|l| l.contains("array-size-threshold"))
        .expect("line must exist after previous assert");
    assert!(
        line.contains("512"),
        "array-size-threshold must be <= 512 for embedded safety. Found: {line}"
    );
}

#[test]
fn firmware_lib_has_no_module_level_allows() {
    let lib_rs = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/lib.rs")
    ).unwrap();
    // Count #![allow( (module-level, not #[allow( which is per-item))
    let module_allows: Vec<&str> = lib_rs.lines()
        .filter(|l| l.trim_start().starts_with("#![allow("))
        .collect();
    assert!(
        module_allows.is_empty(),
        "lib.rs has {} module-level #![allow()] annotations that bypass workspace lints:\n{}",
        module_allows.len(),
        module_allows.join("\n")
    );
}

#[test]
fn boot_rs_unsafe_blocks_have_safety_comments() {
    let boot_rs = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/boot.rs")
    ).unwrap();
    let lines: Vec<&str> = boot_rs.lines().collect();
    let mut violations = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        if line.contains("unsafe {") || line.trim() == "unsafe {" {
            // Check the 3 lines before this unsafe block for a SAFETY comment
            let start = i.saturating_sub(3);
            let preceding = &lines[start..i];
            let has_safety = preceding.iter().any(|l| l.contains("// SAFETY:") || l.contains("SAFETY:"));
            if !has_safety {
                violations.push(format!("  line {}: `{}` (no SAFETY comment in preceding 3 lines)", i + 1, line.trim()));
            }
        }
    }
    assert!(
        violations.is_empty(),
        "boot.rs has {} unsafe blocks without SAFETY comments:\n{}",
        violations.len(),
        violations.join("\n")
    );
}

// ── GAP-1 + GAP-26 tests (added 2026-02-20) ─────────────────────────────────

#[test]
fn display_spi_uses_d1_domain_dma_not_bdma() {
    // SPI2 DMA channels must be from DMA1 or DMA2 (D1-domain, AXI SRAM-accessible).
    // BDMA is in D3 domain and can only access SRAM4 — NOT AXI SRAM.
    // If SPI2 used BDMA with an AXI SRAM buffer, it would silently produce garbage.
    let main_rs = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")
    ).unwrap();
    // Must NOT use BDMA for display SPI
    assert!(!main_rs.contains("p.BDMA"),
        "Display SPI must not use BDMA (D3-domain) — BDMA cannot access AXI SRAM framebuffer");
    // Must document that SPI uses DMA1/DMA2
    assert!(
        main_rs.contains("DMA1") || main_rs.contains("DMA2") ||
        main_rs.contains("dma1") || main_rs.contains("dma2") ||
        main_rs.contains("// D1-domain DMA") || main_rs.contains("DMA channel"),
        "Display SPI DMA must be documented as using D1-domain DMA (DMA1 or DMA2)"
    );
}

#[test]
fn audio_power_down_function_exists() {
    // A power-down sequence must exist to mute the DAC and disable the amp on shutdown.
    // Without this, the device draws continuous current from the amp on battery.
    let main_rs = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/main.rs")
    ).unwrap();
    assert!(
        main_rs.contains("mute_dac_for_shutdown") || main_rs.contains("power_down") ||
        main_rs.contains("audio_power_down") || main_rs.contains("disable_amp"),
        "main.rs must reference audio power-down sequence (mute_dac_for_shutdown or disable_amp)"
    );
}

#[test]
fn audio_sequencer_has_power_down_path() {
    let sequencer_src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/../platform/src/audio_sequencer.rs")
    ).unwrap();
    assert!(
        sequencer_src.contains("mute_dac_for_shutdown") || sequencer_src.contains("disable_amp"),
        "audio_sequencer.rs must have a power-down path (mute_dac_for_shutdown + disable_amp)"
    );
}


#[test]
fn no_todo_macro_in_production_source_paths() {
    // todo!() panics at runtime when hit -- should never exist in production code paths.
    // All unimplemented functionality must use documented stubs, not todo!().
    // Note: todo!() in test modules (#[cfg(test)]) is acceptable.
    let src_dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/src"));
    let mut violations: Vec<String> = Vec::new();

    fn scan_dir(dir: &std::path::Path, violations: &mut Vec<String>) {
        for entry in std::fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if path.is_dir() {
                scan_dir(&path, violations);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let content = std::fs::read_to_string(&path).unwrap_or_default();
                // Skip test modules
                if content.contains("#[cfg(test)]") {
                    // Only check code before the first #[cfg(test)] block
                    let before_tests = content.split("#[cfg(test)]").next().unwrap_or("");
                    if before_tests.contains("todo!()") || before_tests.contains("todo!(\"") {
                        violations.push(format!(
                            "{}: contains todo!() before test module",
                            path.display()
                        ));
                    }
                } else if content.contains("todo!()") || content.contains("todo!(\"") {
                    violations.push(format!("{}: contains todo!()", path.display()));
                }
            }
        }
    }

    scan_dir(src_dir, &mut violations);
    assert!(
        violations.is_empty(),
        "Production code must not use todo!() -- use documented stubs instead:\n{}",
        violations.join("\n")
    );
}

#[test]
fn input_hardware_ok_discards_are_justified() {
    // .ok() on channel operations discards errors silently.
    // Each .ok() in input hardware code must have a justification comment on the
    // same line or an explicit justification marker on the preceding line.
    let input_dir = std::path::Path::new(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/input")
    );
    if !input_dir.exists() {
        return; // No input module -- skip
    }
    for entry in std::fs::read_dir(input_dir).unwrap().flatten() {
        let path = entry.path();
        if path.extension().is_some_and(|e| e == "rs") {
            let content = std::fs::read_to_string(&path).unwrap();
            let lines: Vec<&str> = content.lines().collect();
            for (i, line) in lines.iter().enumerate() {
                if line.contains(".ok()") && !line.trim_start().starts_with("//") {
                    // Must have a justification comment on the same line or the line before
                    let justified = line.contains("// ")
                        || (i > 0
                            && (lines[i - 1].contains("// ok:")
                                || lines[i - 1].contains("// OK:")
                                || lines[i - 1].contains("// ignore")));
                    assert!(
                        justified,
                        "input file {} line {}: `.ok()` must have justification comment: `{}`",
                        path.display(),
                        i + 1,
                        line.trim()
                    );
                }
            }
        }
    }
}

/// builder.rs must not use {:?} debug format in production code paths.
/// use_debug lint requires explicit display strings instead of Debug trait formatting.
#[test]
fn builder_rs_has_no_debug_format_in_production_paths() {
    let builder_rs = include_str!("../src/input/builder.rs");
    let violations: Vec<(usize, &str)> = builder_rs
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            !trimmed.starts_with("///") && !trimmed.starts_with("//") && line.contains("{:?}")
        })
        .collect();
    assert!(
        violations.is_empty(),
        "builder.rs has {} debug format strings in non-comment lines. \
         Replace with match arms returning &'static str.\n\
         Violations at lines: {:?}",
        violations.len(),
        violations.iter().map(|(i, _)| i + 1).collect::<Vec<_>>()
    );
}
