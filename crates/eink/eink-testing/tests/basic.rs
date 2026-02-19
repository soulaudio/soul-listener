use eink_testing::TestEmulator;

#[test]
fn test_emulator_creation() {
    let t = TestEmulator::new(296, 128);
    assert_eq!(t.component_count(), 0);
    assert!(t.components().is_empty());
}

#[test]
fn test_query_by_test_id_empty() {
    let t = TestEmulator::new(296, 128);
    assert!(t.query_by_test_id("nonexistent").is_none());
}

#[test]
fn test_register_and_query() {
    let mut t = TestEmulator::new(296, 128);
    assert!(t.components().is_empty());
    t.register_component("header", "Container", (0, 0), (296, 30));
    assert_eq!(t.component_count(), 1);
    assert!(t.query_by_test_id("header").is_some());
    assert!(t.query_by_test_id("footer").is_none());
}

#[test]
fn test_pixel_defaults_to_white() {
    let t = TestEmulator::new(296, 128);
    use embedded_graphics::pixelcolor::Gray4;
    use embedded_graphics::prelude::*;
    assert_eq!(t.pixel_at(0, 0), Some(Gray4::WHITE));
    assert_eq!(t.pixel_at(100, 64), Some(Gray4::WHITE));
}
