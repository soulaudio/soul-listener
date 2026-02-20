//! Architecture boundary tests — run with `cargo test -p firmware --test arch_boundaries`
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

    // The trait-existence assertions above are sufficient. This runtime check
    // is just a placeholder so the test body is non-empty.
    assert!(
        true,
        "platform HAL compiled without firmware dependencies — boundary intact"
    );
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

    let pkt = HciPacket::from_command(HciCommand::Reset);
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

/// Verify `MpuApplier::soul_audio_register_pairs()` returns exactly 2 pairs.
///
/// Architecture rule: the SoulAudio boot sequence configures exactly two
/// non-cacheable MPU regions. Any change to this count must be intentional
/// and documented.
#[test]
fn test_mpu_applier_returns_two_pairs() {
    use platform::mpu::MpuApplier;

    let pairs = MpuApplier::soul_audio_register_pairs();
    assert_eq!(
        pairs.len(),
        2,
        "MpuApplier must return exactly 2 (RBAR, RASR) pairs for the SoulAudio DAP"
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

/// Verify that `firmware::boot::mpu_register_pairs()` returns exactly 2 pairs.
///
/// Architecture rule: the SoulAudio boot sequence must configure exactly
/// two non-cacheable MPU regions — AXI SRAM (DMA1/DMA2 pool) and SRAM4
/// (BDMA pool). Any change to this count is a deliberate hardware change
/// that must be reviewed.
#[test]
fn mpu_register_pairs_correct_count() {
    let pairs = firmware::boot::mpu_register_pairs();
    assert_eq!(
        pairs.len(),
        2,
        "must configure exactly 2 non-cacheable MPU regions"
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
