use eink_testing::TestEmulator;

#[test]
fn test_emulator_creation() {
    let emu = TestEmulator::new(296, 128);
    assert_eq!(emu.component_count(), 0);
}

#[test]
fn test_query_by_test_id_empty() {
    let emu = TestEmulator::new(296, 128);
    assert!(emu.query_by_test_id("nonexistent").is_none());
}

#[test]
fn test_query_all_empty() {
    let emu = TestEmulator::new(296, 128);
    assert!(emu.query_all().is_empty());
}
