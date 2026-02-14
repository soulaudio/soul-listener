//! Tri-color display support tests
//!
//! Tests for Spectra 6 and Kaleido 3 color e-ink displays

use eink_emulator::{ColorMode, EinkColor, Framebuffer, SpectraColor};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::GrayColor;

#[test]
fn test_eink_color_gray4_conversion() {
    let color = EinkColor::from_gray4(Gray4::BLACK);
    assert!(color.is_grayscale());
    assert!(!color.is_color());

    let rgba = color.to_rgba();
    assert_eq!(rgba, 0x000000FF); // Black with alpha
}

#[test]
fn test_eink_color_spectra6_conversion() {
    let color = EinkColor::Spectra6 {
        bw: Gray4::new(2),
        color: SpectraColor::Red,
    };
    assert!(color.is_color());
    assert!(!color.is_grayscale());

    let rgba = color.to_rgba();
    // Red channel should be 255
    let r = (rgba >> 24) & 0xFF;
    assert_eq!(r, 255);
}

#[test]
fn test_eink_color_kaleido3_conversion() {
    let color = EinkColor::Kaleido3 { r: 15, g: 8, b: 0 };
    assert!(color.is_color());

    let rgba = color.to_rgba();
    let r = (rgba >> 24) & 0xFF;
    let g = (rgba >> 16) & 0xFF;
    let b = (rgba >> 8) & 0xFF;

    assert_eq!(r, 255); // 15 * 17 = 255
    assert_eq!(g, 136); // 8 * 17 = 136
    assert_eq!(b, 0); // 0 * 17 = 0
}

#[test]
fn test_spectra6_rgba_output() {
    // Test all Spectra colors
    let test_cases = [
        (SpectraColor::None, "gray"),
        (SpectraColor::Red, "red"),
        (SpectraColor::Yellow, "yellow"),
        (SpectraColor::Blue, "blue"),
        (SpectraColor::Green, "green"),
    ];

    for (spectra_color, name) in test_cases {
        let color = EinkColor::Spectra6 {
            bw: Gray4::new(2), // Mid-gray base
            color: spectra_color,
        };
        let rgba = color.to_rgba();

        // Check that color channels are in expected ranges
        let r = (rgba >> 24) & 0xFF;
        let g = (rgba >> 16) & 0xFF;
        let b = (rgba >> 8) & 0xFF;

        match spectra_color {
            SpectraColor::Red => {
                assert_eq!(r, 255, "Red channel should be 255 for {} pigment", name);
            }
            SpectraColor::Blue => {
                assert_eq!(b, 255, "Blue channel should be 255 for {} pigment", name);
            }
            SpectraColor::Yellow => {
                assert_eq!(r, 255, "Red should be 255 for {}", name);
                assert_eq!(g, 255, "Green should be 255 for {}", name);
            }
            SpectraColor::Green => {
                assert_eq!(g, 255, "Green channel should be 255 for {} pigment", name);
            }
            SpectraColor::None => {
                assert_eq!(r, g, "R and G should match for {}", name);
                assert_eq!(g, b, "G and B should match for {}", name);
            }
        }
    }
}

#[test]
fn test_kaleido3_rgba_output() {
    // Test pure colors
    let red = EinkColor::Kaleido3 { r: 15, g: 0, b: 0 };
    assert_eq!(red.to_rgba(), 0xFF0000FF);

    let green = EinkColor::Kaleido3 { r: 0, g: 15, b: 0 };
    assert_eq!(green.to_rgba(), 0x00FF00FF);

    let blue = EinkColor::Kaleido3 { r: 0, g: 0, b: 15 };
    assert_eq!(blue.to_rgba(), 0x0000FFFF);

    let white = EinkColor::Kaleido3 {
        r: 15,
        g: 15,
        b: 15,
    };
    assert_eq!(white.to_rgba(), 0xFFFFFFFF);

    let black = EinkColor::Kaleido3 { r: 0, g: 0, b: 0 };
    assert_eq!(black.to_rgba(), 0x000000FF);
}

#[test]
fn test_color_quantization() {
    let gray = EinkColor::Gray(Gray4::new(2));
    let quantized = gray.quantize(2); // Binary

    match quantized {
        EinkColor::Gray(g) => {
            let luma = g.luma();
            assert!(
                luma == 0 || luma == 3,
                "Should quantize to black or white, got {}",
                luma
            );
        }
        _ => panic!("Should remain grayscale"),
    }
}

#[test]
fn test_framebuffer_color_mode_creation() {
    let fb_gray = Framebuffer::with_color_mode(10, 10, ColorMode::Grayscale);
    assert_eq!(fb_gray.color_mode, ColorMode::Grayscale);

    let fb_spectra = Framebuffer::with_color_mode(10, 10, ColorMode::Spectra6);
    assert_eq!(fb_spectra.color_mode, ColorMode::Spectra6);

    let fb_kaleido = Framebuffer::with_color_mode(10, 10, ColorMode::Kaleido3);
    assert_eq!(fb_kaleido.color_mode, ColorMode::Kaleido3);
}

#[test]
fn test_framebuffer_rgba_conversion_grayscale() {
    let mut fb = Framebuffer::with_color_mode(2, 2, ColorMode::Grayscale);
    fb.set_pixel(0, 0, EinkColor::Gray(Gray4::new(0))); // Black
    fb.set_pixel(1, 1, EinkColor::Gray(Gray4::new(3))); // White

    let rgba = fb.to_rgba();
    assert_eq!(rgba.len(), 4);
    assert_eq!(rgba[0], 0x000000FF); // Black
    assert_eq!(rgba[3], 0xFFFFFFFF); // White (3 * 85 = 255)
}

#[test]
fn test_framebuffer_rgba_conversion_spectra6() {
    let mut fb = Framebuffer::with_color_mode(2, 2, ColorMode::Spectra6);

    fb.set_pixel(
        0,
        0,
        EinkColor::Spectra6 {
            bw: Gray4::WHITE,
            color: SpectraColor::Red,
        },
    );

    let rgba = fb.to_rgba();
    let r = (rgba[0] >> 24) & 0xFF;
    assert_eq!(r, 255, "Red channel should be 255 for Red pigment");
}

#[test]
fn test_framebuffer_rgba_conversion_kaleido3() {
    let mut fb = Framebuffer::with_color_mode(2, 2, ColorMode::Kaleido3);

    fb.set_pixel(0, 0, EinkColor::Kaleido3 { r: 15, g: 0, b: 0 });

    let rgba = fb.to_rgba();
    assert_eq!(rgba[0], 0xFF0000FF); // Pure red
}

#[test]
fn test_gray4_to_mode_conversion() {
    let fb_gray = Framebuffer::with_color_mode(10, 10, ColorMode::Grayscale);
    let converted = fb_gray.gray4_to_mode(Gray4::new(2));
    assert_eq!(converted, EinkColor::Gray(Gray4::new(2)));

    let fb_spectra = Framebuffer::with_color_mode(10, 10, ColorMode::Spectra6);
    let converted = fb_spectra.gray4_to_mode(Gray4::new(2));
    match converted {
        EinkColor::Spectra6 { bw, color } => {
            assert_eq!(bw.luma(), 2);
            assert_eq!(color, SpectraColor::None);
        }
        _ => panic!("Should be Spectra6 color"),
    }

    let fb_kaleido = Framebuffer::with_color_mode(10, 10, ColorMode::Kaleido3);
    let converted = fb_kaleido.gray4_to_mode(Gray4::new(2));
    match converted {
        EinkColor::Kaleido3 { r, g, b } => {
            assert_eq!(r, 10); // 2 * 5 = 10
            assert_eq!(g, 10);
            assert_eq!(b, 10);
        }
        _ => panic!("Should be Kaleido3 color"),
    }
}
