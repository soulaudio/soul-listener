//! Hardware Quirks Demonstration
//!
//! Demonstrates hardware-specific quirks and limitations of different e-ink controllers.
//! This example shows how to:
//! - Enable/disable quirk simulation
//! - Trigger controller-specific quirks
//! - Handle quirk errors
//! - View quirk warnings in the UI

use eink_emulator::Emulator;
use eink_specs::{quirks_for_controller, ColorMode, Controller, DisplaySpec, PanelType};

/// Create display spec with specific controller for testing
fn create_spec_with_controller(controller: Controller) -> &'static DisplaySpec {
    let spec = DisplaySpec {
        name: match controller {
            Controller::IT8951 => "Test IT8951",
            Controller::SSD1680 => "Test SSD1680",
            Controller::UC8151 => "Test UC8151",
            _ => "Test Display",
        },
        width: 250,
        height: 122,
        controller,
        panel_type: PanelType::Carta1000,
        color_mode: Some(ColorMode::Grayscale),
        grayscale_levels: 4,
        full_refresh_ms: 2000,
        partial_refresh_ms: 300,
        fast_refresh_ms: 260,
        ghosting_rate_partial: 0.15,
        ghosting_rate_fast: 0.25,
        flash_count_full: 3,
        temp_optimal_min: 15,
        temp_optimal_max: 35,
        temp_operating_min: 0,
        temp_operating_max: 50,
        quirks: Some(quirks_for_controller(controller)),
    };

    Box::leak(Box::new(spec))
}

#[tokio::main]
async fn main() {
    println!("=== E-Ink Controller Quirks Demonstration ===\n");

    // ============================================================================
    // Demo 1: IT8951 - Panel-Specific Quirks
    // ============================================================================
    println!("1. IT8951 Controller - Panel-Specific Parameters");
    println!("   Known quirks:");
    let it8951_quirks = quirks_for_controller(Controller::IT8951);
    for quirk in it8951_quirks {
        println!("   - {}: {}", quirk.quirk_type(), quirk.description());
    }
    println!();

    let spec_it8951 = create_spec_with_controller(Controller::IT8951);
    let mut emulator = Emulator::headless_with_spec(spec_it8951);

    // Trigger panel-specific quirk
    println!("   Triggering init operation (panel-specific quirk)...");
    let result = emulator.check_quirks("init");
    match result {
        Ok(_) => println!("   ✓ Init completed (quirk logged as warning)"),
        Err(e) => println!("   ✗ Init failed: {}", e),
    }
    println!();

    // ============================================================================
    // Demo 2: SSD1680 - Uncontrollable Refresh Rate
    // ============================================================================
    println!("2. SSD1680 Controller - Refresh Rate Issues");
    println!("   Known quirks:");
    let ssd1680_quirks = quirks_for_controller(Controller::SSD1680);
    for quirk in ssd1680_quirks {
        println!("   - {}: {}", quirk.quirk_type(), quirk.description());
    }
    println!();

    let spec_ssd1680 = create_spec_with_controller(Controller::SSD1680);
    let mut emulator = Emulator::headless_with_spec(spec_ssd1680);

    // Trigger refresh rate quirk
    println!("   Triggering refresh operation...");
    let result = emulator.check_quirks("refresh");
    match result {
        Ok(_) => println!("   ✓ Refresh completed (quirk logged as warning)"),
        Err(e) => println!("   ✗ Refresh failed: {}", e),
    }
    if let Some(quirk) = emulator.active_quirk() {
        println!(
            "   Active quirk: {}",
            quirk.chars().take(60).collect::<String>()
        );
    }
    println!();

    // ============================================================================
    // Demo 3: UC8151 - Multiple Quirks
    // ============================================================================
    println!("3. UC8151 Controller - Multiple Hardware Issues");
    println!("   Known quirks:");
    let uc8151_quirks = quirks_for_controller(Controller::UC8151);
    for quirk in uc8151_quirks {
        println!("   - {}: {}", quirk.quirk_type(), quirk.description());
    }
    println!();

    let spec_uc8151 = create_spec_with_controller(Controller::UC8151);
    let mut emulator = Emulator::headless_with_spec(spec_uc8151);

    // Trigger rotation glitch (critical error)
    println!("   Triggering rotation change (critical quirk)...");
    let result = emulator.check_quirks("rotation");
    match result {
        Ok(_) => println!("   ✓ Rotation succeeded"),
        Err(e) => println!("   ✗ Rotation failed: {}", e),
    }
    println!();

    // Reset emulator for next quirk
    emulator.disable_quirks();
    emulator.enable_quirks();

    // Trigger SPI hang (critical error)
    println!("   Triggering SPI write (critical quirk)...");
    let result = emulator.check_quirks("spi_write");
    match result {
        Ok(_) => println!("   ✓ SPI write succeeded"),
        Err(e) => println!("   ✗ SPI write failed: {}", e),
    }
    println!();

    // ============================================================================
    // Demo 4: Disabling Quirks for Testing
    // ============================================================================
    println!("4. Disabling Quirks for Idealized Testing");
    println!("   Creating UC8151 emulator with quirks disabled...");

    let mut emulator = Emulator::headless_with_spec(spec_uc8151);
    emulator.disable_quirks();

    println!("   Quirks enabled: {}", emulator.quirks_enabled());

    // Operations that would normally fail now succeed
    println!("   Attempting rotation (would fail with quirks enabled)...");
    let result = emulator.check_quirks("rotation");
    match result {
        Ok(_) => println!("   ✓ Rotation succeeded (quirks disabled)"),
        Err(e) => println!("   ✗ Rotation failed: {}", e),
    }

    println!("   Attempting SPI write (would fail with quirks enabled)...");
    let result = emulator.check_quirks("spi_write");
    match result {
        Ok(_) => println!("   ✓ SPI write succeeded (quirks disabled)"),
        Err(e) => println!("   ✗ SPI write failed: {}", e),
    }
    println!();

    // ============================================================================
    // Demo 5: Visual Demo (if not headless)
    // ============================================================================
    #[cfg(not(feature = "headless"))]
    {
        println!("5. Visual Demonstration (Window Mode)");
        println!("   Creating UC8151 emulator with visual window...");
        println!("   Quirk warnings will appear in the window title.");
        println!();

        let mut emulator = Emulator::with_spec(spec_uc8151);

        // Draw some content
        Rectangle::new(Point::new(10, 10), Size::new(230, 102))
            .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
            .draw(&mut emulator)
            .unwrap();

        Text::new(
            "UC8151 Controller",
            Point::new(20, 30),
            MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
        )
        .draw(&mut emulator)
        .unwrap();

        Text::new(
            "With Hardware Quirks",
            Point::new(20, 50),
            MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
        )
        .draw(&mut emulator)
        .unwrap();

        // Trigger a quirk (will show in window title)
        let _ = emulator.check_quirks("rotation");

        emulator.refresh_full().await.unwrap();

        println!("   Check the window title for quirk warning!");
        println!("   Close the window to continue...");

        emulator.run();
    }

    // ============================================================================
    // Summary
    // ============================================================================
    println!("\n=== Summary ===");
    println!("Hardware quirks simulation enables developers to:");
    println!("  1. Test against real controller limitations during development");
    println!("  2. Catch controller-specific bugs before hardware deployment");
    println!("  3. Disable quirks for idealized testing when needed");
    println!("  4. View quirk warnings in the emulator window title");
    println!("\nQuirks are enabled by default for realistic simulation.");
    println!("Use emulator.disable_quirks() for testing without limitations.");
}
