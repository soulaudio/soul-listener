//! Custom LUT Waveform Demo
//!
//! Demonstrates loading and using custom waveform LUTs for maximum emulation accuracy.
//! Shows:
//! - Creating custom waveforms programmatically
//! - Loading LUTs from JSON
//! - Saving/loading binary LUT format
//! - Comparing default vs custom waveform behavior
//! - Temperature effects with custom LUTs
//! - Ghosting differences

use eink_emulator::lut::{LutPhase, WaveformLut, WaveformLutSet};
use eink_emulator::{Emulator, PixelState, WaveformMode};
use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use std::fs;

fn main() {
    println!("=== Custom LUT Waveform Demo ===\n");

    demo_1_create_custom_lut();
    demo_2_json_format();
    demo_3_binary_format();
    demo_4_physics_comparison();
    demo_5_temperature_effects();
    demo_6_complex_waveforms();

    println!("\n=== Demo Complete ===");
}

/// Demo 1: Create custom LUT programmatically
fn demo_1_create_custom_lut() {
    println!("Demo 1: Creating Custom LUT Programmatically");
    println!("--------------------------------------------");

    // Create a custom GC16 waveform with optimized phase sequence
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

    let lut = WaveformLut::new(WaveformMode::GC16, phases.clone(), (20, 30));

    println!("  Waveform: GC16");
    println!("  Phases: {}", lut.phases.len());
    println!("  Total duration: {}ms", lut.total_duration_ms);
    println!(
        "  Ghosting contribution: {:.2}%",
        lut.ghosting_contribution() * 100.0
    );
    println!("  DC balance: {:.3}", lut.dc_balance());
    println!(
        "  Temperature range: {}°C to {}°C",
        lut.temperature_range.0, lut.temperature_range.1
    );

    // Validate
    match lut.validate() {
        Ok(()) => println!("  ✓ Validation passed"),
        Err(e) => println!("  ✗ Validation failed: {}", e),
    }

    println!();
}

/// Demo 2: JSON format (human-readable)
fn demo_2_json_format() {
    println!("Demo 2: JSON Format");
    println!("-------------------");

    let json = r#"{
  "version": 1,
  "temperature": 25,
  "waveforms": {
    "GC16": {
      "phases": [
        {"voltage": -15, "duration_us": 10000},
        {"voltage": 15, "duration_us": 10000},
        {"voltage": -10, "duration_us": 8000},
        {"voltage": 10, "duration_us": 8000},
        {"voltage": -5, "duration_us": 5000},
        {"voltage": 5, "duration_us": 5000}
      ]
    },
    "DU4": {
      "phases": [
        {"voltage": -10, "duration_us": 5000},
        {"voltage": 10, "duration_us": 5000}
      ]
    }
  }
}"#;

    println!("  Loading LUTs from JSON...");

    match WaveformLutSet::from_json(json) {
        Ok(lut_set) => {
            println!("  ✓ Successfully loaded LUTs");

            if let Some(gc16) = lut_set.get_lut(WaveformMode::GC16) {
                println!("    GC16: {} phases, {}ms", gc16.phases.len(), gc16.total_duration_ms);
            }

            if let Some(du4) = lut_set.get_lut(WaveformMode::DU4) {
                println!("    DU4: {} phases, {}ms", du4.phases.len(), du4.total_duration_ms);
            }

            // Save to file
            match fs::write("example_luts.json", json) {
                Ok(()) => println!("  ✓ Saved to example_luts.json"),
                Err(e) => println!("  ✗ Failed to save: {}", e),
            }

            // Roundtrip test
            match lut_set.to_json() {
                Ok(regenerated_json) => {
                    println!("  ✓ JSON roundtrip successful");
                    match WaveformLutSet::from_json(&regenerated_json) {
                        Ok(_) => println!("  ✓ Regenerated JSON parses correctly"),
                        Err(e) => println!("  ✗ Regenerated JSON parse failed: {}", e),
                    }
                }
                Err(e) => println!("  ✗ JSON generation failed: {}", e),
            }
        }
        Err(e) => println!("  ✗ Failed to load: {}", e),
    }

    println!();
}

/// Demo 3: Binary format (hardware-compatible)
fn demo_3_binary_format() {
    println!("Demo 3: Binary Format (IT8951/UC8151 Compatible)");
    println!("------------------------------------------------");

    // Create LUT set
    let mut lut_set = WaveformLutSet::new();

    let gc16_phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 10000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 10000,
        },
    ];
    lut_set.set_lut(WaveformLut::new(WaveformMode::GC16, gc16_phases, (20, 30)));

    // Convert to binary
    let binary = lut_set.to_bytes();
    println!("  Binary size: {} bytes", binary.len());
    println!("  Magic: {:?}", &binary[0..4]);
    println!("  Version: {}", binary[4]);
    println!("  Temperature: {}°C", binary[5] as i8);
    println!("  Mode count: {}", binary[6]);

    // Save to file
    match fs::write("example_luts.bin", &binary) {
        Ok(()) => println!("  ✓ Saved to example_luts.bin"),
        Err(e) => println!("  ✗ Failed to save: {}", e),
    }

    // Roundtrip test
    match WaveformLutSet::from_bytes(&binary) {
        Ok(loaded) => {
            println!("  ✓ Binary roundtrip successful");
            if loaded.get_lut(WaveformMode::GC16).is_some() {
                println!("  ✓ GC16 LUT loaded correctly");
            }
        }
        Err(e) => println!("  ✗ Failed to reload: {}", e),
    }

    println!();
}

/// Demo 4: Compare default vs custom waveform behavior
fn demo_4_physics_comparison() {
    println!("Demo 4: Physics Comparison (Default vs Custom LUT)");
    println!("--------------------------------------------------");

    // Create gentle custom waveform
    let gentle_phases = vec![
        LutPhase {
            voltage: -8,
            duration_us: 3000,
        },
        LutPhase {
            voltage: 8,
            duration_us: 3000,
        },
    ];
    let gentle_lut = WaveformLut::new(WaveformMode::DU4, gentle_phases, (20, 30));

    // Create aggressive custom waveform
    let aggressive_phases = vec![
        LutPhase {
            voltage: -15,
            duration_us: 15000,
        },
        LutPhase {
            voltage: 15,
            duration_us: 15000,
        },
    ];
    let aggressive_lut = WaveformLut::new(WaveformMode::DU4, aggressive_phases, (20, 30));

    // Test with pixel states
    let mut pixel_default = PixelState::new();
    let mut pixel_gentle = PixelState::new();
    let mut pixel_aggressive = PixelState::new();

    // Apply refreshes
    pixel_default.partial_refresh(15, 0.15, 25); // Standard DU4 rate
    pixel_gentle.partial_refresh_with_lut(15, &gentle_lut, 25);
    pixel_aggressive.partial_refresh_with_lut(15, &aggressive_lut, 25);

    println!("  Transition: 0 → 15 (white to black)");
    println!("  Temperature: 25°C");
    println!();
    println!("  Default waveform:");
    println!("    Ghosting: {:.3}", pixel_default.ghosting);
    println!("    DC balance: {:.3}", pixel_default.dc_balance);
    println!();
    println!("  Gentle custom LUT (±8V, 6ms):");
    println!("    Ghosting: {:.3}", pixel_gentle.ghosting);
    println!("    DC balance: {:.3}", pixel_gentle.dc_balance);
    println!(
        "    LUT characteristics: {:.3} ghosting, {:.3} DC",
        gentle_lut.ghosting_contribution(),
        gentle_lut.dc_balance()
    );
    println!();
    println!("  Aggressive custom LUT (±15V, 30ms):");
    println!("    Ghosting: {:.3}", pixel_aggressive.ghosting);
    println!("    DC balance: {:.3}", pixel_aggressive.dc_balance);
    println!(
        "    LUT characteristics: {:.3} ghosting, {:.3} DC",
        aggressive_lut.ghosting_contribution(),
        aggressive_lut.dc_balance()
    );

    println!();
}

/// Demo 5: Temperature effects with custom LUTs
fn demo_5_temperature_effects() {
    println!("Demo 5: Temperature Effects");
    println!("---------------------------");

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

    // LUT optimized for 20-30°C
    let lut = WaveformLut::new(WaveformMode::DU4, phases, (20, 30));

    println!(
        "  LUT temperature range: {}°C to {}°C",
        lut.temperature_range.0, lut.temperature_range.1
    );
    println!();

    let temperatures = [5, 15, 25, 35, 45];

    for &temp in &temperatures {
        let mut pixel = PixelState::new();
        pixel.partial_refresh_with_lut(15, &lut, temp);

        let in_range = temp >= lut.temperature_range.0 && temp <= lut.temperature_range.1;
        let status = if in_range { "✓ in range" } else { "✗ out of range" };

        println!(
            "  {}°C {}: current={}, ghosting={:.3}",
            temp, status, pixel.current, pixel.ghosting
        );
    }

    println!();
}

/// Demo 6: Complex multi-stage waveforms
fn demo_6_complex_waveforms() {
    println!("Demo 6: Complex Multi-Stage Waveforms");
    println!("-------------------------------------");

    // Simulate IT8951 GC16 waveform (simplified)
    let it8951_gc16_phases = vec![
        // Stage 1: Drive to black (clear previous state)
        LutPhase {
            voltage: -15,
            duration_us: 12000,
        },
        // Stage 2: Drive to white (clear to white)
        LutPhase {
            voltage: 15,
            duration_us: 12000,
        },
        // Stage 3: Intermediate level 1
        LutPhase {
            voltage: -12,
            duration_us: 9000,
        },
        LutPhase {
            voltage: 12,
            duration_us: 9000,
        },
        // Stage 4: Intermediate level 2
        LutPhase {
            voltage: -8,
            duration_us: 6000,
        },
        LutPhase {
            voltage: 8,
            duration_us: 6000,
        },
        // Stage 5: Fine adjustment
        LutPhase {
            voltage: -4,
            duration_us: 4000,
        },
        LutPhase {
            voltage: 4,
            duration_us: 4000,
        },
    ];

    let it8951_lut = WaveformLut::new(WaveformMode::GC16, it8951_gc16_phases, (15, 35));

    println!("  IT8951-style GC16 Waveform:");
    println!("    Stages: {}", it8951_lut.phases.len() / 2);
    println!("    Total duration: {}ms", it8951_lut.total_duration_ms);
    println!(
        "    Ghosting contribution: {:.2}%",
        it8951_lut.ghosting_contribution() * 100.0
    );
    println!("    DC balance: {:.3}", it8951_lut.dc_balance());

    // Validate
    match it8951_lut.validate() {
        Ok(()) => println!("    ✓ Validation passed"),
        Err(e) => println!("    ✗ Validation failed: {}", e),
    }

    println!();
    println!("  Phase breakdown:");
    for (i, phase) in it8951_lut.phases.iter().enumerate() {
        println!(
            "    Phase {}: {:+3}V for {:.1}ms",
            i + 1,
            phase.voltage,
            phase.duration_us as f32 / 1000.0
        );
    }

    println!();
}
