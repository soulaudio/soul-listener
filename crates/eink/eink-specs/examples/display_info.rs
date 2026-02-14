//! Display Information Example
//!
//! Demonstrates accessing display specifications and computing derived properties.
//!
//! Run with: cargo run --example display_info

use eink_specs::displays::*;
use eink_specs::DisplaySpec;

fn print_display_info(spec: &DisplaySpec) {
    println!("\n{}", "=".repeat(60));
    println!("Display: {}", spec.name);
    println!("{}", "=".repeat(60));

    println!("\n📐 Physical Specifications:");
    println!("  Resolution: {}×{} pixels", spec.width, spec.height);
    println!("  Aspect Ratio: {:.2}:1", spec.aspect_ratio());
    println!("  Diagonal: {:.2}\"", spec.diagonal_inches());
    println!("  Controller: {:?}", spec.controller);
    println!("  Panel Type: {:?}", spec.panel_type);
    println!("  Grayscale Levels: {}", spec.grayscale_levels);

    println!("\n⏱️  Refresh Timing:");
    println!(
        "  Full Refresh: {}ms ({} flashes)",
        spec.full_refresh_ms, spec.flash_count_full
    );
    println!("  Partial Refresh: {}ms", spec.partial_refresh_ms);
    println!("  Fast Refresh: {}ms", spec.fast_refresh_ms);

    println!("\n👻 Ghosting Rates:");
    println!(
        "  Partial: {:.1}% per refresh",
        spec.ghosting_rate_partial * 100.0
    );
    println!(
        "  Fast: {:.1}% per refresh",
        spec.ghosting_rate_fast * 100.0
    );

    println!("\n🌡️  Temperature Ranges:");
    println!(
        "  Optimal: {}°C to {}°C",
        spec.temp_optimal_min, spec.temp_optimal_max
    );
    println!(
        "  Operating: {}°C to {}°C",
        spec.temp_operating_min, spec.temp_operating_max
    );

    println!("\n❄️  Temperature Compensation:");
    println!(
        "  At 25°C: {}ms (normal)",
        spec.adjusted_refresh_ms(spec.full_refresh_ms, 25)
    );
    println!(
        "  At -5°C: {}ms (50% slower)",
        spec.adjusted_refresh_ms(spec.full_refresh_ms, -5)
    );
    println!(
        "  At 45°C: {}ms (20% slower)",
        spec.adjusted_refresh_ms(spec.full_refresh_ms, 45)
    );
}

fn main() {
    println!("E-Ink Display Specifications");
    println!("============================\n");

    println!("Waveshare Displays:");
    print_display_info(&WAVESHARE_2_13_V4);
    print_display_info(&WAVESHARE_2_9_V2);
    print_display_info(&WAVESHARE_4_2_V2);
    print_display_info(&WAVESHARE_7_5_V2);

    println!("\n\nGood Display Panels:");
    print_display_info(&GDEW0213I5F);
    print_display_info(&GDEW029T5);
    print_display_info(&GDEW042T2);
    print_display_info(&GDEW075T7);

    println!("\n\n💡 Usage Tips:");
    println!("  - Use full refresh every 5-10 partial refreshes to clear ghosting");
    println!("  - Fast refresh accumulates ghosting faster, use sparingly");
    println!("  - Adjust timing based on temperature for optimal results");
    println!("  - Pearl panels have higher ghosting than Carta panels");
}
