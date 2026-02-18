//! Partial Window and Dirty Region Tracking Demo
//!
//! This example demonstrates how to use the partial window and dirty
//! region tracking features of the eink-emulator.

use eink_emulator::{Emulator, PartialWindow};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("E-Ink Partial Window Demo\n");

    // Create a headless emulator
    let mut emulator = Emulator::headless(250, 122);

    // ========================================================================
    // Example 1: Manual Dirty Region Tracking
    // ========================================================================
    println!("Example 1: Manual Dirty Region Tracking");
    println!("=========================================");

    // Draw some rectangles and manually mark them as dirty
    Rectangle::new(Point::new(10, 10), Size::new(30, 20))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)?;
    emulator.mark_dirty(Rectangle::new(Point::new(10, 10), Size::new(30, 20)));

    Rectangle::new(Point::new(60, 60), Size::new(40, 30))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
        .draw(&mut emulator)?;
    emulator.mark_dirty(Rectangle::new(Point::new(60, 60), Size::new(40, 30)));

    // Get the bounding box of all dirty regions
    if let Some(bounds) = emulator.get_dirty_bounds() {
        println!("Dirty regions count: {}", emulator.dirty_regions().len());
        println!("Dirty bounds: {:?}", bounds);
        println!("  Top-left: ({}, {})", bounds.top_left.x, bounds.top_left.y);
        println!("  Size: {}×{}", bounds.size.width, bounds.size.height);
        println!("  Note: Bounds are auto-aligned to 8-pixel boundaries\n");
    }

    // Clear dirty regions after refresh
    emulator.clear_dirty();
    println!("Dirty regions cleared\n");

    // ========================================================================
    // Example 2: Partial Window with Alignment
    // ========================================================================
    println!("Example 2: Partial Window with Alignment");
    println!("==========================================");

    // Create a partial window from an unaligned rectangle
    let unaligned_rect = Rectangle::new(Point::new(15, 25), Size::new(50, 35));
    let window = PartialWindow::new(unaligned_rect);

    println!("Original rectangle:");
    println!(
        "  Top-left: ({}, {})",
        unaligned_rect.top_left.x, unaligned_rect.top_left.y
    );
    println!(
        "  Size: {}×{}",
        unaligned_rect.size.width, unaligned_rect.size.height
    );

    println!("\nAligned rectangle:");
    println!(
        "  Top-left: ({}, {})",
        window.aligned_rect.top_left.x, window.aligned_rect.top_left.y
    );
    println!(
        "  Size: {}×{}",
        window.aligned_rect.size.width, window.aligned_rect.size.height
    );
    println!("  Was aligned: {}", window.was_aligned);
    println!("  Area: {} pixels\n", window.area());

    // ========================================================================
    // Example 3: Auto-Dirty Tracking (Conceptual)
    // ========================================================================
    println!("Example 3: Auto-Dirty Tracking");
    println!("================================");

    // Enable auto-dirty tracking
    emulator.enable_auto_dirty_tracking(true);
    println!(
        "Auto-dirty tracking enabled: {}",
        emulator.is_auto_dirty_tracking_enabled()
    );

    // Note: Auto-dirty tracking is currently limited due to embedded-graphics
    // draw_iter implementation constraints. When enabled, it will track
    // dirty regions automatically during drawing operations.

    println!("\nIn a future version, drawing operations will automatically");
    println!("track dirty regions without manual mark_dirty() calls.\n");

    // ========================================================================
    // Example 4: Workflow Pattern
    // ========================================================================
    println!("Example 4: Typical Workflow Pattern");
    println!("=====================================");

    // 1. Draw content
    Rectangle::new(Point::new(100, 50), Size::new(60, 40))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(1)))
        .draw(&mut emulator)?;

    // 2. Mark dirty
    emulator.mark_dirty(Rectangle::new(Point::new(100, 50), Size::new(60, 40)));

    // 3. Get bounds for refresh optimization
    if let Some(bounds) = emulator.get_dirty_bounds() {
        println!("Refreshing region: {:?}", bounds);
        // In a real application, you would refresh only this region:
        // emulator.refresh_partial_window(bounds).await?;
    }

    // 4. Clear after refresh
    emulator.clear_dirty();
    println!("Workflow complete!\n");

    // ========================================================================
    // Example 5: Multiple Region Merging
    // ========================================================================
    println!("Example 5: Multiple Region Merging");
    println!("====================================");

    // Mark several small regions
    emulator.mark_dirty(Rectangle::new(Point::new(10, 10), Size::new(10, 10)));
    emulator.mark_dirty(Rectangle::new(Point::new(25, 25), Size::new(10, 10)));
    emulator.mark_dirty(Rectangle::new(Point::new(40, 40), Size::new(10, 10)));
    emulator.mark_dirty(Rectangle::new(Point::new(55, 55), Size::new(10, 10)));

    println!("Marked {} separate regions", emulator.dirty_regions().len());

    if let Some(bounds) = emulator.get_dirty_bounds() {
        println!("Merged into single bounding box:");
        println!("  Position: ({}, {})", bounds.top_left.x, bounds.top_left.y);
        println!("  Size: {}×{}", bounds.size.width, bounds.size.height);
        println!("\nThis optimization reduces refresh operations from");
        println!("4 partial refreshes to 1 partial refresh!\n");
    }

    println!("Demo complete!");
    Ok(())
}
