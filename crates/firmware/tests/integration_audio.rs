//! Audio integration tests — verify the full audio driver stack.
// Integration test file: expect/unwrap/panic are intentional test mechanisms.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
)]
//!
//! Tests the interaction between platform traits (AudioCodec) and
//! firmware driver implementations (MockDac, MockAmp).
//!
//! Note: MockDac and MockAmp are accessed via their module paths because the
//! top-level firmware re-exports are gated on `cfg(any(test, feature = "emulator"))`,
//! which does not apply when the library is compiled for integration testing.
//!
//! Run with: cargo test -p firmware --test integration_audio

use firmware::audio::amp::mock::MockAmp;
use firmware::audio::amp::AmpDriver;
use firmware::audio::dac::mock::MockDac;
use platform::{AudioCodec, AudioConfig, OversamplingFilter};

/// Verify MockDac implements AudioCodec correctly
#[tokio::test]
async fn test_mock_dac_implements_audio_codec() {
    let mut dac = MockDac::new();
    let config = AudioConfig::default();

    // init should succeed
    dac.init(config)
        .await
        .expect("mock DAC init should succeed");

    // volume 0-100 should succeed
    dac.set_volume(0).await.expect("volume 0 should succeed");
    dac.set_volume(100)
        .await
        .expect("volume 100 should succeed");
    dac.set_volume(50).await.expect("volume 50 should succeed");

    // volume > 100 should fail
    assert!(dac.set_volume(101).await.is_err(), "volume 101 must fail");
}

/// Verify AudioConfig validation is enforced before driver use
#[test]
fn test_audio_config_validation_before_use() {
    let valid = AudioConfig::default();
    assert!(valid.validate().is_ok());

    let invalid = AudioConfig {
        sample_rate: 0,
        ..AudioConfig::default()
    };
    assert!(invalid.validate().is_err());

    let too_high = AudioConfig {
        sample_rate: 1_000_000,
        ..AudioConfig::default()
    };
    assert!(too_high.validate().is_err());
}

/// Verify MockAmp enable/disable cycle tracks state correctly
#[tokio::test]
async fn test_mock_amp_enable_disable() {
    let mut amp = MockAmp::new();

    assert!(!amp.is_enabled());
    amp.enable().await.expect("enable should succeed");
    assert!(amp.is_enabled());
    amp.disable().await.expect("disable should succeed");
    assert!(!amp.is_enabled());
}

/// Verify filter variants compile and are distinct
#[tokio::test]
async fn test_mock_dac_filter_variants() {
    let mut dac = MockDac::new();
    dac.init(AudioConfig::default()).await.unwrap();

    use OversamplingFilter::{
        ApodizingFastRollOff, BrickWall, FastRollOffLinearPhase, FastRollOffMinimumPhase,
        HybridFastRollOff, SlowRollOffLinearPhase, SlowRollOffMinimumPhase,
    };
    for filter in [
        FastRollOffLinearPhase,
        SlowRollOffLinearPhase,
        FastRollOffMinimumPhase,
        SlowRollOffMinimumPhase,
        ApodizingFastRollOff,
        BrickWall,
        HybridFastRollOff,
    ] {
        dac.set_filter(filter)
            .await
            .expect("filter set should succeed");
    }
}

/// Verify that MockDac start/stop cycle updates the started flag correctly
#[tokio::test]
async fn test_mock_dac_start_stop_lifecycle() {
    let mut dac = MockDac::new();

    // starts not playing
    assert!(!dac.started);

    dac.start().await.expect("start should succeed");
    assert!(dac.started, "started must be true after start()");

    dac.stop().await.expect("stop should succeed");
    assert!(!dac.started, "started must be false after stop()");
}

/// Verify sample write accumulation counts correctly
#[tokio::test]
// LARGE_STACK_ARRAYS: 256- and 512-element test buffers exercise write_samples
// accumulation; this is a host integration test with no stack size constraint.
#[allow(clippy::large_stack_arrays)]
async fn test_mock_dac_samples_accumulate() {
    let mut dac = MockDac::new();
    let samples_a = [0i32; 256];
    let samples_b = [0i32; 512];

    dac.write_samples(&samples_a)
        .await
        .expect("write_samples should succeed");
    assert_eq!(dac.samples_written, 256);

    dac.write_samples(&samples_b)
        .await
        .expect("second write_samples should succeed");
    assert_eq!(dac.samples_written, 768);
}

/// Verify MockAmp enable count is tracked correctly for multi-cycle
#[tokio::test]
async fn test_mock_amp_counts_enable_disable() {
    let mut amp = MockAmp::new();

    for _ in 0..3 {
        amp.enable().await.unwrap();
        amp.disable().await.unwrap();
    }

    assert_eq!(amp.enable_count, 3);
    assert_eq!(amp.disable_count, 3);
    assert!(!amp.is_enabled());
}
