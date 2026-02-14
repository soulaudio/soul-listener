//! Comprehensive power consumption simulation tests

use eink_emulator::{DisplayDriver, Emulator, PowerProfile};
use std::time::Duration;
use tokio::time::sleep;

#[tokio::test]
async fn test_idle_power_consumption() {
    let mut emulator = Emulator::headless(250, 122);

    // Start in idle, wait much longer to accumulate measurable energy
    // With integer math and 150µA: 150 × 33 × 10000 / 36_000_000 = ~1.375µWh
    sleep(Duration::from_millis(10000)).await;

    // Trigger state update
    emulator.update_buffer().await.unwrap();

    let stats = emulator.power_stats();

    println!("Stats: energy={}µWh, idle={}ms, active={}ms",
             stats.total_energy_uwh, stats.idle_time_ms, stats.active_time_ms);

    // Should have consumed some energy in idle
    assert!(
        stats.total_energy_uwh > 0,
        "Idle state should consume energy (got {}µWh after {}ms idle)",
        stats.total_energy_uwh, stats.idle_time_ms
    );
    assert!(
        stats.idle_time_ms >= 10000,
        "Should have at least 10000ms idle time"
    );
}

#[tokio::test]
async fn test_refresh_power_base_boost() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Perform a full refresh with flashes
    emulator.refresh_full().await.unwrap();

    let stats = emulator.power_stats();

    // Peak current should be higher than base (includes boost from flashes)
    let base_current = PowerProfile::WAVESHARE_2_13_V4.refresh_current_ua;
    let min_expected = base_current + PowerProfile::WAVESHARE_2_13_V4.refresh_boost_ua;

    assert!(
        stats.peak_current_ua >= min_expected,
        "Peak current ({}) should be at least base ({}) + boost ({})",
        stats.peak_current_ua,
        base_current,
        PowerProfile::WAVESHARE_2_13_V4.refresh_boost_ua
    );
}

#[tokio::test]
async fn test_sleep_power_consumption() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Enter sleep mode
    emulator.sleep().await.unwrap();
    sleep(Duration::from_millis(100)).await;

    // Wake up
    emulator.wake().await.unwrap();

    let stats = emulator.power_stats();

    assert!(
        stats.sleep_time_ms >= 100,
        "Should have at least 100ms sleep time"
    );

    // Sleep current should be very low
    let sleep_ua = PowerProfile::WAVESHARE_2_13_V4.sleep_current_ua;
    assert!(
        sleep_ua < 5000,
        "Sleep current should be < 5mA ({}µA)",
        sleep_ua
    );
}

#[tokio::test]
async fn test_init_sequence_power() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Perform initialization
    emulator.initialize().await.unwrap();

    let stats = emulator.power_stats();

    // Initialization should consume energy
    assert!(
        stats.total_energy_uwh > 0,
        "Initialization should consume energy"
    );
    assert!(
        stats.active_time_ms >= 1900,
        "Initialization should take ~1970ms"
    );
}

#[tokio::test]
async fn test_buffer_transfer_power() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Transfer buffer with delays to ensure measurable time
    sleep(Duration::from_millis(10)).await;
    emulator.update_buffer().await.unwrap();
    sleep(Duration::from_millis(10)).await;

    // Force another state transition to capture final state
    emulator.update_buffer().await.unwrap();

    let stats = emulator.power_stats();

    // Buffer transfer is instant but we should capture the idle time around it
    assert!(
        stats.total_runtime_ms() >= 15,
        "Should have at least 15ms runtime (got {}ms)",
        stats.total_runtime_ms()
    );
}

#[tokio::test]
async fn test_total_energy_calculation() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Perform multiple operations
    emulator.update_buffer().await.unwrap();
    sleep(Duration::from_millis(50)).await;

    emulator.refresh_full().await.unwrap();
    sleep(Duration::from_millis(50)).await;

    let stats = emulator.power_stats();

    // Total energy should increase with each operation
    // With corrected realistic values (54mA refresh for ~2s), expect ~100µWh
    assert!(stats.total_energy_uwh > 10, "Should have consumed >10µWh");
}

#[tokio::test]
async fn test_battery_life_estimation() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Simulate some usage
    for _ in 0..3 {
        emulator.refresh_full().await.unwrap();
        sleep(Duration::from_millis(10)).await;
    }

    let stats = emulator.power_stats();

    // Estimate battery life with 3000mAh battery
    let battery_life = stats.estimated_battery_life_hours(3000);

    // Should get a reasonable estimate (> 0 hours)
    assert!(
        battery_life > 0.0,
        "Should estimate positive battery life"
    );

    // With refreshes, average current should be significant
    assert!(stats.average_current_ua > 0, "Average current should be > 0");
}

#[tokio::test]
async fn test_average_current_calculation() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Mix of operations: idle + refresh
    sleep(Duration::from_millis(100)).await; // Idle
    emulator.update_buffer().await.unwrap();

    emulator.refresh_partial().await.unwrap(); // Active
    sleep(Duration::from_millis(100)).await; // Idle

    // Force final state update
    emulator.update_buffer().await.unwrap();

    let stats = emulator.power_stats();

    // Average should be positive and less than or equal to peak
    assert!(
        stats.average_current_ua > 0,
        "Average current should be > 0"
    );
    assert!(
        stats.average_current_ua <= stats.peak_current_ua,
        "Average ({}) should be <= peak ({})",
        stats.average_current_ua,
        stats.peak_current_ua
    );

    // Average should be reasonable (between idle and peak)
    let idle_ua = PowerProfile::WAVESHARE_2_13_V4.idle_current_ua;
    assert!(
        stats.average_current_ua >= idle_ua / 2,
        "Average should be at least half of idle current"
    );
}

#[tokio::test]
async fn test_peak_current_tracking() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Start with idle (low current)
    sleep(Duration::from_millis(50)).await;
    emulator.update_buffer().await.unwrap();

    // Then full refresh (high current)
    emulator.refresh_full().await.unwrap();

    let stats = emulator.power_stats();

    // Peak should be significantly higher than idle
    let idle_current = PowerProfile::WAVESHARE_2_13_V4.idle_current_ua;
    let refresh_current = PowerProfile::WAVESHARE_2_13_V4.refresh_current_ua;

    assert!(
        stats.peak_current_ua > refresh_current,
        "Peak ({}) should be higher than base refresh current ({})",
        stats.peak_current_ua,
        refresh_current
    );

    assert!(
        stats.peak_current_ua > idle_current * 10,
        "Peak should be much higher than idle"
    );
}

#[tokio::test]
async fn test_enable_disable_power_tracking() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Disable power tracking
    emulator.enable_power_tracking(false);
    assert!(!emulator.is_power_tracking_enabled());

    // Perform operations
    emulator.refresh_full().await.unwrap();
    sleep(Duration::from_millis(50)).await;

    let stats = emulator.power_stats();

    // Should not accumulate energy when disabled
    assert_eq!(stats.total_energy_uwh, 0, "Should not track when disabled");
    assert_eq!(stats.idle_time_ms, 0);
    assert_eq!(stats.active_time_ms, 0);

    // Re-enable
    emulator.enable_power_tracking(true);
    assert!(emulator.is_power_tracking_enabled());

    // Now it should track
    emulator.reset_power_stats();
    emulator.refresh_full().await.unwrap();

    let stats = emulator.power_stats();
    assert!(stats.total_energy_uwh > 0, "Should track when enabled");
}

#[tokio::test]
async fn test_state_percentages() {
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Spend time in different states
    sleep(Duration::from_millis(70)).await; // Idle: 70ms
    emulator.update_buffer().await.unwrap();

    emulator.refresh_partial().await.unwrap(); // Active: ~300ms
    sleep(Duration::from_millis(20)).await; // Idle: 20ms
    emulator.update_buffer().await.unwrap();

    emulator.sleep().await.unwrap();
    sleep(Duration::from_millis(10)).await; // Sleep: 10ms
    emulator.wake().await.unwrap();

    let stats = emulator.power_stats();
    let percentages = stats.state_percentages();

    // Verify percentages sum to ~100%
    let total = percentages.idle + percentages.active + percentages.sleep;
    assert!(
        (total - 100.0).abs() < 1.0,
        "Percentages should sum to ~100%: {}",
        total
    );

    // Verify each percentage is reasonable
    assert!(percentages.idle > 0.0, "Should have idle time");
    assert!(percentages.active > 0.0, "Should have active time");
    assert!(percentages.sleep > 0.0, "Should have sleep time");
}

#[tokio::test]
async fn test_power_stats_reset() {
    let mut emulator = Emulator::headless(250, 122);

    // Accumulate some stats
    emulator.refresh_full().await.unwrap();
    sleep(Duration::from_millis(50)).await;

    let stats_before = emulator.power_stats().clone();
    assert!(stats_before.total_energy_uwh > 0);

    // Reset
    emulator.reset_power_stats();

    let stats_after = emulator.power_stats();
    assert_eq!(stats_after.total_energy_uwh, 0);
    assert_eq!(stats_after.average_current_ua, 0);
    assert_eq!(stats_after.peak_current_ua, 0);
    assert_eq!(stats_after.idle_time_ms, 0);
    assert_eq!(stats_after.active_time_ms, 0);
    assert_eq!(stats_after.sleep_time_ms, 0);
}

#[tokio::test]
async fn test_different_waveform_power() {
    let mut emulator = Emulator::headless(250, 122);

    // Full refresh (3 flashes)
    emulator.reset_power_stats();
    emulator.refresh_full().await.unwrap();
    let full_peak = emulator.power_stats().peak_current_ua;

    // Partial refresh (0 flashes)
    emulator.reset_power_stats();
    emulator.refresh_partial().await.unwrap();
    let partial_peak = emulator.power_stats().peak_current_ua;

    // Full refresh should have higher peak due to flashes
    assert!(
        full_peak > partial_peak,
        "Full refresh ({}) should use more power than partial ({})",
        full_peak,
        partial_peak
    );
}

#[tokio::test]
async fn test_realistic_usage_scenario() {
    // Simulate e-reader page reading session
    let mut emulator = Emulator::headless(250, 122);
    emulator.reset_power_stats();

    // Initial full refresh
    emulator.refresh_full().await.unwrap();

    // Read pages with realistic idle periods
    // Real e-reader usage: 30+ seconds reading per page, 300ms page turn
    for _ in 0..5 {
        sleep(Duration::from_millis(500)).await; // Reading
        emulator.refresh_partial().await.unwrap(); // Page turn
    }

    // Final page reading time
    sleep(Duration::from_millis(1000)).await;

    // Force final state transition
    emulator.update_buffer().await.unwrap();

    let stats = emulator.power_stats();
    let percentages = stats.state_percentages();

    // With realistic usage, should spend more time idle than active
    // (reading takes longer than page turns)
    assert!(
        percentages.idle > percentages.active,
        "Should spend more time idle ({:.1}%) than active ({:.1}%)",
        percentages.idle,
        percentages.active
    );

    // Estimate battery life
    let battery_life = stats.estimated_battery_life_hours(3000);
    assert!(battery_life > 0.1, "Should get > 0.1 hour of battery life");
}
