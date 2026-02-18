//! Tri-Color E-Ink Display Demo
//!
//! Demonstrates Spectra 6 (6-color) and Kaleido 3 (4096-color) display support.
//!
//! This example showcases:
//! - Creating color emulators with different color modes
//! - Drawing with Spectra 6 colors (red, yellow, blue, green)
//! - Drawing with Kaleido 3 RGB colors
//! - Color ghosting accumulation (2× faster than grayscale)
//! - Full color refresh timing (15s for Spectra 6, 500ms for Kaleido 3)
//!
//! Run with:
//! ```bash
//! cargo run --package eink-emulator --example tricolor_demo
//! ```

use eink_emulator::{
    ColorMode, EinkColor, Emulator, Framebuffer, SpectraColor,
};
use embedded_graphics::{
    pixelcolor::Gray4,
    prelude::*,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Tri-Color E-Ink Display Demo ===\n");

    // Demo 1: Spectra 6 (6-color display)
    println!("Demo 1: Spectra 6 (ACeP) - 6 Colors");
    spectra6_demo().await?;

    println!("\nDemo 2: Kaleido 3 - 4096 Colors");
    kaleido3_demo().await?;

    println!("\nDemo 3: Color Ghosting Comparison");
    ghosting_demo().await?;

    println!("\n=== Demo Complete ===");
    Ok(())
}

/// Demonstrate Spectra 6 color e-ink display
async fn spectra6_demo() -> Result<(), Box<dyn std::error::Error>> {
    // Create emulator with Spectra 6 display (for visual display, if needed)
    let _emulator = Emulator::with_spec(&eink_specs::displays::WAVESHARE_5_65_SPECTRA6);

    // Get a framebuffer with Spectra6 color mode
    let mut framebuffer = Framebuffer::with_color_mode(600, 448, ColorMode::Spectra6);

    println!("  Creating Spectra 6 display (600×448, 6 colors)");
    println!("  Colors: Black, White, Red, Yellow, Blue, Green");

    // Draw color swatches
    let colors = [
        (SpectraColor::Red, "Red", 50),
        (SpectraColor::Yellow, "Yellow", 150),
        (SpectraColor::Blue, "Blue", 250),
        (SpectraColor::Green, "Green", 350),
    ];

    for (color, name, x) in colors {
        // Draw colored rectangle
        for py in 100..200 {
            for px in x..(x + 80) {
                framebuffer.set_pixel(
                    px,
                    py,
                    EinkColor::Spectra6 {
                        bw: Gray4::new(2), // Mid-gray base
                        color,
                    },
                );
            }
        }

        // Label
        println!("  Drawing {} swatch at x={}", name, x);
    }

    // Draw title in black
    println!("  Drawing title");
    for (i, _ch) in "Spectra 6 Colors".chars().enumerate() {
        let x = 180 + (i as u32) * 7;
        for y in 50..60 {
            framebuffer.set_pixel(
                x,
                y,
                EinkColor::Spectra6 {
                    bw: Gray4::BLACK,
                    color: SpectraColor::None,
                },
            );
        }
    }

    println!("  Refresh duration: ~15 seconds (color particle movement)");
    println!("  Flash count: 30 (many flashes for color)");
    println!("  Temperature range: 0-50°C");

    // For visual demo, save screenshot
    // (In real demo with window, this would display)
    println!("  ✓ Spectra 6 demo complete");

    Ok(())
}

/// Demonstrate Kaleido 3 color e-ink display
async fn kaleido3_demo() -> Result<(), Box<dyn std::error::Error>> {
    // Create headless emulator with Kaleido 3 (for visual display, if needed)
    let _emulator = Emulator::headless(300, 400); // Typical Kaleido 3 size

    // Get a framebuffer with Kaleido3 color mode
    let mut framebuffer = Framebuffer::with_color_mode(300, 400, ColorMode::Kaleido3);

    println!("  Creating Kaleido 3 display (300×400, 4096 colors)");
    println!("  Color depth: 4-bit per channel (RGB)");

    // Draw RGB color gradient
    let colors = [
        ((15, 0, 0), "Red"),
        ((15, 15, 0), "Yellow"),
        ((0, 15, 0), "Green"),
        ((0, 15, 15), "Cyan"),
        ((0, 0, 15), "Blue"),
        ((15, 0, 15), "Magenta"),
    ];

    for (i, ((r, g, b), name)) in colors.iter().enumerate() {
        let x_start = 20 + (i as u32) * 45;

        // Draw colored rectangle
        for py in 100..180 {
            for px in x_start..(x_start + 40) {
                framebuffer.set_pixel(
                    px,
                    py,
                    EinkColor::Kaleido3 {
                        r: *r,
                        g: *g,
                        b: *b,
                    },
                );
            }
        }

        println!("  Drawing {} at RGB({},{},{})", name, r, g, b);
    }

    // Draw grayscale gradient (showing 4-bit precision)
    println!("  Drawing grayscale gradient");
    for level in 0..16 {
        let x_start = 20 + (level as u32) * 16;
        for py in 220..280 {
            for px in x_start..(x_start + 15) {
                framebuffer.set_pixel(
                    px,
                    py,
                    EinkColor::Kaleido3 {
                        r: level,
                        g: level,
                        b: level,
                    },
                );
            }
        }
    }

    println!("  Refresh duration: ~500ms (fast color update)");
    println!("  Flash count: 4 (moderate flashing)");
    println!("  Resolution: 300ppi B&W, 150ppi color");

    println!("  ✓ Kaleido 3 demo complete");

    Ok(())
}

/// Demonstrate color ghosting accumulation
async fn ghosting_demo() -> Result<(), Box<dyn std::error::Error>> {
    use eink_emulator::PixelState;

    println!("  Comparing B&W vs Color ghosting rates");

    // Black & white pixel
    let mut pixel_bw = PixelState::new();

    // Color pixel
    let mut pixel_color = PixelState::new_with_color();

    // Perform 5 partial refreshes
    println!("  Simulating 5 partial refreshes...");
    for i in 1..=5 {
        let target = if i % 2 == 0 { 15 } else { 0 };
        pixel_bw.partial_refresh(target, 0.15, 25);
        pixel_color.partial_refresh(target, 0.15, 25);

        let bw_ghosting = pixel_bw.ghosting;
        let color_ghosting = pixel_color.color_state.unwrap().color_ghosting;

        println!(
            "    Refresh {}: B&W ghosting={:.3}, Color ghosting={:.3}",
            i, bw_ghosting, color_ghosting
        );
    }

    // Check final ghosting levels
    let bw_final = pixel_bw.ghosting;
    let color_final = pixel_color.color_state.unwrap().color_ghosting;

    println!("\n  Final ghosting levels:");
    println!("    B&W:   {:.3}", bw_final);
    println!("    Color: {:.3} (2× rate)", color_final);

    assert!(
        color_final > bw_final,
        "Color particles should ghost more than B&W"
    );

    // Full refresh clears both
    println!("\n  Performing full refresh (GCC16)...");
    pixel_bw.full_refresh(8);
    pixel_color.full_refresh(8);

    let bw_after = pixel_bw.ghosting;
    let color_after = pixel_color.color_state.unwrap().color_ghosting;

    println!("    B&W ghosting:   {:.3} (cleared)", bw_after);
    println!("    Color ghosting: {:.3} (cleared)", color_after);

    assert_eq!(bw_after, 0.0);
    assert_eq!(color_after, 0.0);

    println!("  ✓ Ghosting demo complete");

    Ok(())
}
