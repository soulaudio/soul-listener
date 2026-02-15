//! Hardware Quirk Simulation Tests
//!
//! Tests for controller-specific hardware quirks and limitations.

use eink_emulator::{DisplayDriver, Emulator};
use eink_specs::{quirks_for_controller, ColorMode, Controller, DisplaySpec, PanelType};

/// Create test display spec with specific controller
fn test_spec_with_controller(controller: Controller) -> DisplaySpec {
    DisplaySpec {
        name: "Test Display",
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
    }
}

#[tokio::test]
async fn test_quirk_enable_disable_toggle() {
    let mut emulator = Emulator::headless(250, 122);

    // Quirks enabled by default
    assert!(emulator.quirks_enabled());

    // Disable quirks
    emulator.disable_quirks();
    assert!(!emulator.quirks_enabled());
    assert!(emulator.active_quirk().is_none());

    // Re-enable quirks
    emulator.enable_quirks();
    assert!(emulator.quirks_enabled());
}

#[tokio::test]
async fn test_it8951_panel_specific_quirk() {
    let spec = test_spec_with_controller(Controller::IT8951);
    let spec_ref: &'static DisplaySpec = Box::leak(Box::new(spec));
    let mut emulator = Emulator::headless_with_spec(spec_ref);

    // IT8951 has panel-specific quirk that triggers on init
    let result = emulator.check_quirks("init");

    // Should log warning but not fail (quirk logs, doesn't error)
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_ssd1680_refresh_rate_quirk() {
    let spec = test_spec_with_controller(Controller::SSD1680);
    let spec_ref: &'static DisplaySpec = Box::leak(Box::new(spec));
    let mut emulator = Emulator::headless_with_spec(spec_ref);

    // SSD1680 has uncontrollable refresh rate quirk
    let result = emulator.check_quirks("refresh");

    // Should log warning but not fail
    assert!(result.is_ok());

    // Active quirk should be set
    assert!(emulator.active_quirk().is_some());
}

#[tokio::test]
async fn test_uc8151_rotation_glitch() {
    let spec = test_spec_with_controller(Controller::UC8151);
    let spec_ref: &'static DisplaySpec = Box::leak(Box::new(spec));
    let mut emulator = Emulator::headless_with_spec(spec_ref);

    // UC8151 has rotation glitch quirk
    let result = emulator.check_quirks("rotation");

    // Should fail with error
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("QUIRK TRIGGERED"));
    assert!(err_msg.contains("rotation"));

    // Active quirk should be set
    assert!(emulator.active_quirk().is_some());
}

#[tokio::test]
async fn test_uc8151_spi_hang() {
    let spec = test_spec_with_controller(Controller::UC8151);
    let spec_ref: &'static DisplaySpec = Box::leak(Box::new(spec));
    let mut emulator = Emulator::headless_with_spec(spec_ref);

    // UC8151 has SPI write hang quirk
    let result = emulator.check_quirks("spi_write");

    // Should fail with error
    assert!(result.is_err());
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("QUIRK TRIGGERED"));
    assert!(err_msg.contains("SPI"));

    // Active quirk should be set
    assert!(emulator.active_quirk().is_some());
}

#[tokio::test]
async fn test_quirk_error_messages() {
    let spec = test_spec_with_controller(Controller::UC8151);
    let spec_ref: &'static DisplaySpec = Box::leak(Box::new(spec));
    let mut emulator = Emulator::headless_with_spec(spec_ref);

    // Test that error messages are clear and actionable
    let result = emulator.check_quirks("rotation");
    assert!(result.is_err());

    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("⚠️"));
    assert!(err_msg.contains("QUIRK"));
}

#[tokio::test]
async fn test_multiple_quirks_on_same_controller() {
    let spec = test_spec_with_controller(Controller::UC8151);
    let spec_ref: &'static DisplaySpec = Box::leak(Box::new(spec));
    let mut emulator = Emulator::headless_with_spec(spec_ref);

    // UC8151 has multiple quirks
    let quirks = quirks_for_controller(Controller::UC8151);
    assert_eq!(quirks.len(), 2); // Rotation glitch + SPI hang

    // Test rotation quirk
    let result1 = emulator.check_quirks("rotation");
    assert!(result1.is_err());

    // Clear active quirk by disabling/re-enabling
    emulator.disable_quirks();
    emulator.enable_quirks();

    // Test SPI quirk
    let result2 = emulator.check_quirks("spi_write");
    assert!(result2.is_err());
}

#[tokio::test]
async fn test_quirk_not_triggered_when_disabled() {
    let spec = test_spec_with_controller(Controller::UC8151);
    let spec_ref: &'static DisplaySpec = Box::leak(Box::new(spec));
    let mut emulator = Emulator::headless_with_spec(spec_ref);

    // Disable quirks
    emulator.disable_quirks();

    // Operation that would normally trigger quirk
    let result = emulator.check_quirks("rotation");

    // Should succeed when quirks disabled
    assert!(result.is_ok());
    assert!(emulator.active_quirk().is_none());
}

#[tokio::test]
async fn test_no_quirks_for_ssd1619() {
    let spec = test_spec_with_controller(Controller::SSD1619);
    let spec_ref: &'static DisplaySpec = Box::leak(Box::new(spec));
    let emulator = Emulator::headless_with_spec(spec_ref);

    // SSD1619 has no known quirks
    let quirks = quirks_for_controller(Controller::SSD1619);
    assert_eq!(quirks.len(), 0);

    // Check shouldn't trigger anything
    assert!(emulator.quirks_enabled());
}

#[tokio::test]
async fn test_quirk_does_not_affect_normal_operations() {
    let spec = test_spec_with_controller(Controller::SSD1680);
    let spec_ref: &'static DisplaySpec = Box::leak(Box::new(spec));
    let mut emulator = Emulator::headless_with_spec(spec_ref);

    // Normal operations should still work even with quirks enabled
    emulator.refresh_full().await.unwrap();
    emulator.refresh_partial().await.unwrap();

    // Quirks are just warnings/errors for specific operations
    assert!(emulator.quirks_enabled());
}

#[test]
fn test_controller_quirks_documentation() {
    // Verify all quirks have descriptions
    for controller in [
        Controller::IT8951,
        Controller::SSD1680,
        Controller::UC8151,
        Controller::IL0373,
    ] {
        let quirks = quirks_for_controller(controller);
        for quirk in quirks {
            let desc = quirk.description();
            assert!(!desc.is_empty(), "Quirk description should not be empty");
            assert!(desc.len() > 20, "Quirk description should be detailed");
        }
    }
}

#[test]
fn test_it8951_has_expected_quirks() {
    let quirks = quirks_for_controller(Controller::IT8951);
    assert_eq!(quirks.len(), 2);

    // Should have panel-specific and limited library support quirks
    use eink_specs::Quirk;
    assert!(quirks
        .iter()
        .any(|q| matches!(q, Quirk::PanelSpecific { .. })));
    assert!(quirks
        .iter()
        .any(|q| matches!(q, Quirk::LimitedLibrarySupport { .. })));
}

#[test]
fn test_all_quirks_have_type_names() {
    use eink_specs::Quirk;

    let quirk1 = Quirk::RotationGlitch {
        description: "test",
    };
    assert_eq!(quirk1.quirk_type(), "RotationGlitch");

    let quirk2 = Quirk::SpiWriteHang {
        description: "test",
    };
    assert_eq!(quirk2.quirk_type(), "SpiWriteHang");

    let quirk3 = Quirk::PanelSpecific {
        description: "test",
    };
    assert_eq!(quirk3.quirk_type(), "PanelSpecific");
}
