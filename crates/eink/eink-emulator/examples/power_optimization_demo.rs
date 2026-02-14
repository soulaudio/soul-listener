//! Power optimization demonstration
//!
//! Shows how to use power tracking to optimize battery life for e-ink displays.
//! Demonstrates:
//! - Real-time power statistics
//! - Battery life estimation
//! - Comparison of full vs partial refresh energy
//! - Power breakdown by state

use eink_emulator::{DisplayDriver, Emulator, WaveformMode};
use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use embedded_graphics::text::Text;
use std::time::Instant;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    println!("=== E-Ink Display Power Optimization Demo ===\n");

    let mut emulator = Emulator::headless(250, 122);

    // Enable power tracking (it's on by default, but explicit for demo)
    emulator.enable_power_tracking(true);

    demo_idle_power(&mut emulator).await;
    demo_refresh_comparison(&mut emulator).await;
    demo_realistic_usage(&mut emulator).await;
    demo_battery_estimation(&mut emulator).await;
    demo_power_breakdown(&mut emulator).await;

    println!("\n=== Demo Complete ===");
}

/// Demonstrate idle power consumption
async fn demo_idle_power(emulator: &mut Emulator) {
    println!("📊 Demo 1: Idle Power Consumption");
    println!("   Measuring power when display shows static image...\n");

    emulator.reset_power_stats();

    // Display static content
    Text::new(
        "Static Content",
        Point::new(10, 20),
        MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
    )
    .draw(emulator)
    .unwrap();

    emulator.refresh_full().await.unwrap();

    // Simulate idle time (e-reader showing page)
    let idle_duration = Duration::from_millis(1000);
    println!("   Idling for {}ms...", idle_duration.as_millis());

    let start = Instant::now();
    sleep(idle_duration).await;

    // Trigger state update
    emulator.update_buffer().await.unwrap();

    let elapsed = start.elapsed();
    let stats = emulator.power_stats();
    let idle_ma = stats.average_current_ua as f32 / 1000.0;

    println!("   ✓ Idle time: {}ms", elapsed.as_millis());
    println!("   ✓ Average current: {:.3}mA", idle_ma);
    println!("   ✓ Energy consumed: {:.3}mWh", stats.total_energy_uwh as f32 / 1000.0);
    println!();
}

/// Compare power consumption of different refresh modes
async fn demo_refresh_comparison(emulator: &mut Emulator) {
    println!("📊 Demo 2: Refresh Mode Power Comparison");
    println!("   Comparing energy usage: Full vs Partial vs Fast refresh\n");

    // Full refresh
    emulator.reset_power_stats();
    emulator.refresh_full().await.unwrap();
    let full_stats = emulator.power_stats().clone();

    // Partial refresh
    emulator.reset_power_stats();
    emulator.refresh_partial().await.unwrap();
    let partial_stats = emulator.power_stats().clone();

    // Fast refresh
    emulator.reset_power_stats();
    emulator.refresh_fast().await.unwrap();
    let fast_stats = emulator.power_stats().clone();

    println!("   Full Refresh (GC16):");
    println!("     - Peak current: {:.1}mA", full_stats.peak_current_ua as f32 / 1000.0);
    println!("     - Energy: {:.3}mWh", full_stats.total_energy_uwh as f32 / 1000.0);
    println!("     - Duration: {}ms", full_stats.active_time_ms);

    println!("\n   Partial Refresh (DU4):");
    println!("     - Peak current: {:.1}mA", partial_stats.peak_current_ua as f32 / 1000.0);
    println!("     - Energy: {:.3}mWh", partial_stats.total_energy_uwh as f32 / 1000.0);
    println!("     - Duration: {}ms", partial_stats.active_time_ms);

    println!("\n   Fast Refresh (DU):");
    println!("     - Peak current: {:.1}mA", fast_stats.peak_current_ua as f32 / 1000.0);
    println!("     - Energy: {:.3}mWh", fast_stats.total_energy_uwh as f32 / 1000.0);
    println!("     - Duration: {}ms", fast_stats.active_time_ms);

    let full_energy = full_stats.total_energy_uwh as f32;
    let partial_energy = partial_stats.total_energy_uwh as f32;
    let savings = ((full_energy - partial_energy) / full_energy) * 100.0;

    println!("\n   💡 Energy savings: Partial uses {:.1}% less energy than Full", savings);
    println!();
}

/// Simulate realistic e-reader usage
async fn demo_realistic_usage(emulator: &mut Emulator) {
    println!("📊 Demo 3: Realistic E-Reader Usage Pattern");
    println!("   Simulating: Initial full refresh + 10 page turns + reading time\n");

    emulator.reset_power_stats();

    // Initial page load (full refresh)
    draw_page(emulator, 1);
    emulator.refresh_full().await.unwrap();

    // Simulate reading and page turns
    for page in 2..=10 {
        // Reading time (idle)
        sleep(Duration::from_millis(100)).await;

        // Page turn (partial refresh)
        draw_page(emulator, page);
        emulator.refresh_partial().await.unwrap();
    }

    // Final reading time
    sleep(Duration::from_millis(200)).await;
    emulator.update_buffer().await.unwrap(); // Trigger final state update

    let stats = emulator.power_stats();
    let percentages = stats.state_percentages();

    println!("   Total runtime: {}ms", stats.total_runtime_ms());
    println!("   Average current: {:.2}mA", stats.average_current_ua as f32 / 1000.0);
    println!("   Peak current: {:.1}mA", stats.peak_current_ua as f32 / 1000.0);
    println!("   Total energy: {:.3}mWh", stats.total_energy_uwh as f32 / 1000.0);
    println!("\n   Time breakdown:");
    println!("     - Idle (reading): {:.1}%", percentages.idle);
    println!("     - Active (refresh): {:.1}%", percentages.active);
    println!("     - Sleep: {:.1}%", percentages.sleep);
    println!();
}

/// Estimate battery life for different battery capacities
async fn demo_battery_estimation(emulator: &mut Emulator) {
    println!("📊 Demo 4: Battery Life Estimation");
    println!("   Using realistic e-reader usage pattern from Demo 3\n");

    let stats = emulator.power_stats();

    // Common battery sizes
    let batteries = [
        (1000, "Small (1000mAh)"),
        (1500, "Medium (1500mAh)"),
        (3000, "Large (3000mAh)"),
        (5000, "XL (5000mAh)"),
    ];

    println!("   Estimated battery life:");
    for (capacity, name) in &batteries {
        let hours = stats.estimated_battery_life_hours(*capacity);
        let days = hours / 24.0;
        println!("     - {}: {:.1} hours ({:.1} days)", name, hours, days);
    }

    println!("\n   💡 Tip: Use partial refresh for page turns to extend battery life!");
    println!("      Full refresh every 5-10 pages to clear ghosting.\n");
}

/// Show detailed power breakdown
async fn demo_power_breakdown(emulator: &mut Emulator) {
    println!("📊 Demo 5: Detailed Power Breakdown");
    println!("   Analyzing power usage by operation type\n");

    // Measure each operation separately
    emulator.reset_power_stats();

    // Buffer transfer
    emulator.update_buffer().await.unwrap();
    let transfer_energy = emulator.power_stats().total_energy_uwh;

    // Idle
    emulator.reset_power_stats();
    sleep(Duration::from_millis(100)).await;
    emulator.update_buffer().await.unwrap();
    let idle_energy = emulator.power_stats().total_energy_uwh;

    // Partial refresh
    emulator.reset_power_stats();
    emulator.refresh_partial().await.unwrap();
    let partial_energy = emulator.power_stats().total_energy_uwh;

    // Full refresh
    emulator.reset_power_stats();
    emulator.refresh_full().await.unwrap();
    let full_energy = emulator.power_stats().total_energy_uwh;

    // Sleep
    emulator.reset_power_stats();
    emulator.sleep().await.unwrap();
    sleep(Duration::from_millis(100)).await;
    emulator.wake().await.unwrap();
    let sleep_energy = emulator.power_stats().total_energy_uwh;

    println!("   Energy per operation:");
    println!("     - Buffer transfer: {:.3}µWh", transfer_energy);
    println!("     - Idle (100ms): {:.3}µWh", idle_energy);
    println!("     - Partial refresh: {:.3}µWh", partial_energy);
    println!("     - Full refresh: {:.3}µWh", full_energy);
    println!("     - Sleep (100ms): {:.3}µWh", sleep_energy);

    println!("\n   Relative costs:");
    println!("     - Full refresh = {:.1}x Partial refresh", full_energy as f32 / partial_energy as f32);
    println!("     - Partial refresh = {:.1}x Idle (100ms)", partial_energy as f32 / idle_energy as f32);
    println!("     - Idle (100ms) = {:.1}x Sleep (100ms)", idle_energy as f32 / sleep_energy as f32);

    println!("\n   💡 Optimization strategies:");
    println!("      1. Minimize full refreshes (use partial when possible)");
    println!("      2. Enter sleep mode when inactive >1s");
    println!("      3. Batch updates to reduce buffer transfers");
    println!();
}

/// Helper: Draw a simple page with number
fn draw_page(emulator: &mut Emulator, page_num: u32) {
    // Clear previous content
    Rectangle::new(Point::zero(), emulator.size())
        .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
        .draw(emulator)
        .unwrap();

    // Draw page number
    let text = format!("Page {}", page_num);
    Text::new(
        &text,
        Point::new(10, 20),
        MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
    )
    .draw(emulator)
    .unwrap();

    // Draw some content to simulate text
    for i in 0..5 {
        Rectangle::new(
            Point::new(10, 35 + i * 15),
            Size::new(200, 2),
        )
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(emulator)
        .unwrap();
    }
}
