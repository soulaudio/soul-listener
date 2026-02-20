//! Comprehensive tests for custom LUT waveform support

// Integration test file — cast/arithmetic/unwrap lints are overly strict for
// e-ink timing/voltage math tests where panics on failure are intentional.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::arithmetic_side_effects,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
)]

use eink_emulator::lut::{LutError, LutPhase, WaveformLut, WaveformLutSet};
use eink_emulator::{PixelState, WaveformMode};

// ============================================================================
// LUT Data Structure Tests
// ============================================================================

#[test]
fn test_lut_phase_creation() {
    let phase = LutPhase {
        voltage: -15,
        duration_us: 10000,
    };
    assert_eq!(phase.voltage, -15);
    assert_eq!(phase.duration_us, 10000);
}

#[test]
fn test_waveform_lut_duration_calculation() {
    let phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: -10,
            duration_us: 8000,
        },
    ];

    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));

    // Total: 10000 + 10000 + 8000 = 28000µs = 28ms
    assert_eq!(lut.total_duration_ms, 28);
}

#[test]
fn test_ghosting_contribution_calculation() {
    // High voltage, long duration phases should contribute more ghosting
    let high_phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 20000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 20000,
        },
    ];

    // Low voltage, short duration phases should contribute less ghosting
    let low_phases = vec![
        LutPhase {
            voltage: -5,
            duration_us: 5000,
        },
        LutPhase {
            voltage: 5,
            duration_us: 5000,
        },
    ];

    let high_lut = WaveformLut::new(WaveformMode::GC16, high_phases, (20, 30));
    let low_lut = WaveformLut::new(WaveformMode::DU4, low_phases, (20, 30));

    let high_ghosting = high_lut.ghosting_contribution();
    let low_ghosting = low_lut.ghosting_contribution();

    assert!(high_ghosting > low_ghosting);
}

#[test]
fn test_dc_balance_symmetric_waveform() {
    // Perfectly symmetric waveform should have near-zero DC balance
    let phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 10000,
        },
    ];

    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
    let dc = lut.dc_balance();

    assert!(dc.abs() < 0.1, "Symmetric waveform DC balance: {}", dc);
}

#[test]
fn test_dc_balance_asymmetric_waveform() {
    // Asymmetric waveform should have net DC bias
    let phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 15000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 5000,
        }, // Less time positive
    ];

    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
    let dc = lut.dc_balance();

    // Should be negative (more time at negative voltage)
    assert!(
        dc < -0.5,
        "Asymmetric waveform should have negative DC: {}",
        dc
    );
}

// ============================================================================
// LUT Validation Tests
// ============================================================================

#[test]
fn test_validation_valid_lut() {
    let phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 10000,
        },
    ];

    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
    assert!(lut.validate().is_ok());
}

#[test]
fn test_validation_invalid_voltage_too_low() {
    let phases = vec![LutPhase {
        voltage: -25, // Too low
        duration_us: 10000,
    }];

    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
    let result = lut.validate();

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LutError::InvalidVoltage(-25)));
}

#[test]
fn test_validation_invalid_voltage_too_high() {
    let phases = vec![LutPhase {
        voltage: 25, // Too high
        duration_us: 10000,
    }];

    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
    let result = lut.validate();

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LutError::InvalidVoltage(25)));
}

#[test]
fn test_validation_invalid_duration() {
    let phases = vec![LutPhase {
        voltage: 15,
        duration_us: 0, // Zero duration
    }];

    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
    let result = lut.validate();

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LutError::InvalidDuration(0)));
}

// ============================================================================
// WaveformLutSet Tests
// ============================================================================

#[test]
fn test_lut_set_get_set_operations() {
    let mut lut_set = WaveformLutSet::new();

    // Initially empty
    assert!(lut_set.get_lut(WaveformMode::GC16).is_none());

    // Add a LUT
    let phases = vec![LutPhase {
        voltage: -15,
        duration_us: 10000,
    }];
    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
    lut_set.set_lut(lut.clone());

    // Should be present now
    let retrieved = lut_set.get_lut(WaveformMode::GC16);
    assert!(retrieved.is_some());
    assert_eq!(retrieved.unwrap().mode, WaveformMode::GC16);

    // Other modes should still be None
    assert!(lut_set.get_lut(WaveformMode::DU4).is_none());
    assert!(lut_set.get_lut(WaveformMode::A2).is_none());
}

#[test]
fn test_lut_set_multiple_modes() {
    let mut lut_set = WaveformLutSet::new();

    // Add multiple LUTs
    let gc16_lut = WaveformLut::new(
        WaveformMode::GC16,
        vec![LutPhase {
            voltage: -15,
            duration_us: 10000,
        }],
        (20, 30),
    );

    let du4_lut = WaveformLut::new(
        WaveformMode::DU4,
        vec![LutPhase {
            voltage: -10,
            duration_us: 5000,
        }],
        (20, 30),
    );

    lut_set.set_lut(gc16_lut);
    lut_set.set_lut(du4_lut);

    // Both should be present
    assert!(lut_set.get_lut(WaveformMode::GC16).is_some());
    assert!(lut_set.get_lut(WaveformMode::DU4).is_some());

    // Others should be None
    assert!(lut_set.get_lut(WaveformMode::GL16).is_none());
}

// ============================================================================
// JSON Format Tests
// ============================================================================

#[test]
fn test_json_parsing_valid_simple() {
    let json = r#"{
        "version": 1,
        "temperature": 25,
        "waveforms": {
            "GC16": {
                "phases": [
                    {"voltage": -15, "duration_us": 10000},
                    {"voltage": 15, "duration_us": 10000}
                ]
            }
        }
    }"#;

    let lut_set = WaveformLutSet::from_json(json).unwrap();
    assert!(lut_set.gc16.is_some());

    let gc16 = lut_set.gc16.as_ref().unwrap();
    assert_eq!(gc16.phases.len(), 2);
    assert_eq!(gc16.phases[0].voltage, -15);
    assert_eq!(gc16.phases[0].duration_us, 10000);
    assert_eq!(gc16.phases[1].voltage, 15);
}

#[test]
fn test_json_parsing_multiple_waveforms() {
    let json = r#"{
        "version": 1,
        "temperature": 25,
        "waveforms": {
            "GC16": {
                "phases": [
                    {"voltage": -15, "duration_us": 10000}
                ]
            },
            "DU4": {
                "phases": [
                    {"voltage": -10, "duration_us": 5000}
                ]
            },
            "A2": {
                "phases": [
                    {"voltage": 15, "duration_us": 3000}
                ]
            }
        }
    }"#;

    let lut_set = WaveformLutSet::from_json(json).unwrap();

    assert!(lut_set.gc16.is_some());
    assert!(lut_set.du4.is_some());
    assert!(lut_set.a2.is_some());
    assert!(lut_set.gl16.is_none());
    assert!(lut_set.du.is_none());
}

#[test]
fn test_json_parsing_invalid_version() {
    let json = r#"{
        "version": 2,
        "temperature": 25,
        "waveforms": {}
    }"#;

    let result = WaveformLutSet::from_json(json);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        LutError::UnsupportedVersion(2)
    ));
}

#[test]
fn test_json_parsing_malformed() {
    let json = r#"{"invalid json"#;

    let result = WaveformLutSet::from_json(json);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LutError::ParseError(_)));
}

#[test]
fn test_json_roundtrip() {
    let mut lut_set = WaveformLutSet::new();

    let phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 10000,
        },
    ];
    lut_set.set_lut(WaveformLut::new(WaveformMode::GC16, phases, (20, 30)));

    // Convert to JSON and back
    let json = lut_set.to_json().unwrap();
    let lut_set2 = WaveformLutSet::from_json(&json).unwrap();

    // Should have same data
    assert!(lut_set2.gc16.is_some());
    let gc16 = lut_set2.gc16.as_ref().unwrap();
    assert_eq!(gc16.phases.len(), 2);
    assert_eq!(gc16.phases[0].voltage, -15);
    assert_eq!(gc16.phases[1].voltage, 15);
}

// ============================================================================
// Binary Format Tests
// ============================================================================

#[test]
fn test_binary_format_valid() {
    let mut data = Vec::new();

    // Magic
    data.extend_from_slice(b"WFM\0");

    // Version
    data.push(1);

    // Temperature
    data.push(25);

    // Mode count
    data.push(1);

    // GC16 mode (ID=0)
    data.push(0);

    // Phase count = 2
    data.extend_from_slice(&2u16.to_le_bytes());

    // Phase 1: -15V, 10000µs
    data.push((-15i8) as u8);
    data.extend_from_slice(&10000u16.to_le_bytes());

    // Phase 2: 15V, 10000µs
    data.push(15u8);
    data.extend_from_slice(&10000u16.to_le_bytes());

    let lut_set = WaveformLutSet::from_bytes(&data).unwrap();
    assert!(lut_set.gc16.is_some());

    let gc16 = lut_set.gc16.as_ref().unwrap();
    assert_eq!(gc16.phases.len(), 2);
    assert_eq!(gc16.phases[0].voltage, -15);
    assert_eq!(gc16.phases[1].voltage, 15);
}

#[test]
fn test_binary_format_invalid_magic() {
    let data = vec![b'B', b'A', b'D', 0, 1, 25, 0];

    let result = WaveformLutSet::from_bytes(&data);
    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), LutError::InvalidFormat(_)));
}

#[test]
fn test_binary_format_unsupported_version() {
    let mut data = Vec::new();
    data.extend_from_slice(b"WFM\0");
    data.push(2); // Unsupported version

    let result = WaveformLutSet::from_bytes(&data);
    assert!(result.is_err());
    assert!(matches!(
        result.unwrap_err(),
        LutError::UnsupportedVersion(2)
    ));
}

#[test]
fn test_binary_format_truncated() {
    let mut data = Vec::new();
    data.extend_from_slice(b"WFM\0");
    data.push(1); // Version
    data.push(25); // Temperature
    data.push(1); // Mode count
    data.push(0); // Mode ID
                  // Missing phase count and data

    let result = WaveformLutSet::from_bytes(&data);
    assert!(result.is_err());
}

#[test]
fn test_binary_roundtrip() {
    let mut lut_set = WaveformLutSet::new();

    let phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 10000,
        },
    ];
    lut_set.set_lut(WaveformLut::new(WaveformMode::GC16, phases, (20, 30)));

    // Convert to binary and back
    let binary = lut_set.to_bytes();
    let lut_set2 = WaveformLutSet::from_bytes(&binary).unwrap();

    // Should have same data
    assert!(lut_set2.gc16.is_some());
    let gc16 = lut_set2.gc16.as_ref().unwrap();
    assert_eq!(gc16.phases.len(), 2);
    assert_eq!(gc16.phases[0].voltage, -15);
    assert_eq!(gc16.phases[1].voltage, 15);
}

// ============================================================================
// Physics Simulation with LUTs
// ============================================================================

#[test]
fn test_pixel_state_with_lut() {
    let mut pixel = PixelState::new();

    let phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 10000,
        },
    ];

    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));

    // Initial state
    assert_eq!(pixel.current, 0);
    assert_eq!(pixel.ghosting, 0.0);

    // Refresh with LUT
    pixel.partial_refresh_with_lut(15, &lut, 25);

    // Should have updated
    assert!(pixel.current > 0);
    assert!(pixel.ghosting >= 0.0);
}

#[test]
fn test_lut_temperature_range_effect() {
    let mut pixel_in_range = PixelState::new();
    let mut pixel_out_range = PixelState::new();

    let phases = vec![LutPhase {
        voltage: 15,
        duration_us: 10000,
    }];

    let lut = WaveformLut::new(WaveformMode::DU4, phases, (20, 30));

    // In range temperature (25°C)
    pixel_in_range.partial_refresh_with_lut(15, &lut, 25);

    // Out of range temperature (5°C)
    pixel_out_range.partial_refresh_with_lut(15, &lut, 5);

    // In-range should reach closer to target
    assert!(
        pixel_in_range.current >= pixel_out_range.current,
        "In-range temp should be more effective"
    );
}

#[test]
fn test_lut_vs_default_waveform_comparison() {
    let mut pixel_lut = PixelState::new();
    let mut pixel_default = PixelState::new();

    // Custom LUT with low ghosting
    let phases = vec![LutPhase {
        voltage: 5,
        duration_us: 1000,
    }]; // Very gentle waveform

    let lut = WaveformLut::new(WaveformMode::DU4, phases, (20, 30));

    // LUT refresh
    pixel_lut.partial_refresh_with_lut(15, &lut, 25);

    // Default refresh (standard ghosting rate)
    pixel_default.partial_refresh(15, 0.15, 25);

    // LUT should have less ghosting due to gentler waveform
    assert!(
        pixel_lut.ghosting < pixel_default.ghosting,
        "Gentle LUT ({}) should have less ghosting than default ({})",
        pixel_lut.ghosting,
        pixel_default.ghosting
    );
}

#[test]
fn test_complex_lut_sequence() {
    let phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: -10,
            duration_us: 8000,
        },
        LutPhase {
            voltage: 10,
            duration_us: 8000,
        },
        LutPhase {
            voltage: -5,
            duration_us: 5000,
        },
        LutPhase {
            voltage: 5,
            duration_us: 5000,
        },
    ];

    let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));

    // Total duration
    assert_eq!(lut.total_duration_ms, 46); // 46ms total

    // DC balance should be near zero (symmetric)
    let dc = lut.dc_balance();
    assert!(dc.abs() < 0.2, "Complex symmetric LUT DC: {}", dc);

    // Should validate successfully
    assert!(lut.validate().is_ok());
}
