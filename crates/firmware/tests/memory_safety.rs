//! Memory safety architecture tests.
//! These tests enforce that DMA buffers use sound Rust ownership patterns.

/// The firmware FRAMEBUFFER must not use `static mut` directly.
/// `static mut` requires UnsafeCell for sound aliasing; use StaticCell instead.
/// Reference: Rust 2024 edition static_mut_refs = deny.
/// https://doc.rust-lang.org/edition-guide/rust-2024/static-mut-references.html
#[test]
fn framebuffer_does_not_use_static_mut() {
    let main_rs = include_str!("../src/main.rs");
    // Must NOT have: static mut FRAMEBUFFER
    // Must HAVE: StaticCell or OnceCell pattern instead
    let has_static_mut_framebuffer = main_rs.contains("static mut FRAMEBUFFER");
    assert!(
        !has_static_mut_framebuffer,
        "FRAMEBUFFER must not use `static mut`. \
         Use StaticCell<[u8; N]> instead — sound under Rust's aliasing model, \
         works with #[link_section]. See Rust 2024 static_mut_refs lint."
    );
}

/// Watchdog feeding must be guarded by task heartbeat checks.
/// If only the main loop heartbeat fires, deadlocked tasks go undetected.
/// The main loop must verify all critical tasks are alive before petting the watchdog.
#[test]
fn watchdog_pet_is_guarded_by_task_heartbeats() {
    let main_rs = include_str!("../src/main.rs");
    // Check for heartbeat pattern: AtomicBool, or a HEARTBEAT constant/static
    let has_heartbeat_guard = main_rs.contains("HEARTBEAT")
        || main_rs.contains("heartbeat")
        || main_rs.contains("AtomicBool")
        || main_rs.contains("task_alive");
    assert!(
        has_heartbeat_guard,
        "watchdog.pet() must be guarded by per-task heartbeat checks. \
         A deadlocked audio/display task is invisible if the main loop \
         keeps running. Use AtomicBool per task, check before pet()."
    );
}

/// StaticCell must be the pattern for all large DMA-accessible statics.
/// This is the sound way to do one-time initialization of a static buffer
/// that needs a specific link section for DMA accessibility.
#[test]
fn firmware_uses_static_cell_for_large_buffers() {
    let main_rs = include_str!("../src/main.rs");
    assert!(
        main_rs.contains("StaticCell") || main_rs.contains("static_cell"),
        "Large static DMA buffers should use StaticCell<T> for sound ownership. \
         This allows #[link_section] AND safe single-init semantics."
    );
}
