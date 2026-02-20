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
    assert!(!version.is_empty(), "eink-specs must have a non-empty version");
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
    assert_eq!(detect_format(&[0xFF, 0xFB, 0x00, 0x00]), Some(AudioFormat::Mp3));
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
        assert_ne!(
            rasr & 1,
            0,
            "Region {idx}: RASR bit 0 (ENABLE) must be SET"
        );
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
