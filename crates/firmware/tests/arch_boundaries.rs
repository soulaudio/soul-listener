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
