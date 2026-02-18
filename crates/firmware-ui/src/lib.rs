//! firmware-ui - Hot-Reloadable UI Rendering Layer
//!
//! UI rendering logic for the SoulAudio DAP emulator.
//! Compiled as rlib (static) and cdylib (for hot-reload).
//!
//! # Architecture
//!
//! The binary (display_emulator) never reloads; only this dylib is swapped.
//! hot-lib-reloader recompiles and reloads the dylib on file changes.
//!
//! # Dependency Design
//!
//! The C ABI boundary uses eink_emulator::Emulator directly (not
//! firmware::EmulatorDisplay) to avoid a circular dependency:
//!   firmware -> firmware-ui (for the hot-reload feature)
//!   firmware-ui -> firmware (for EmulatorDisplay) -- CIRCULAR, AVOIDED
//!
//! The binary side unwraps EmulatorDisplay to its inner Emulator before
//! calling render_ui. The dylib then renders directly to the Emulator.
//!
//! # Usage
//!
//! Step 1: cargo build --package firmware-ui --features hot-reload
//! Step 2: cargo run --example display_emulator --features emulator,hot-reload

#[cfg(feature = "emulator")]
mod render;

#[cfg(feature = "emulator")]
pub use render::render_demo_menu;

/// Hot-reload entry point: render the DAP UI onto an eink_emulator::Emulator.
///
/// The binary side passes a pointer to the inner Emulator from EmulatorDisplay.
/// This avoids the circular dependency: firmware -> firmware-ui -> firmware.
///
/// # Safety
///
/// emulator_ptr must be a valid, exclusively-owned Emulator pointer.
#[cfg(feature = "hot-reload")]
#[no_mangle]
pub unsafe extern "C" fn render_ui(emulator_ptr: *mut eink_emulator::Emulator) {
    let emulator = unsafe { &mut *emulator_ptr };
    if let Err(e) = render::render_onto_emulator(emulator) {
        eprintln!("[firmware-ui] render_ui error: {:?}", e);
    }
}

/// Build version of this dylib for hot-reload change detection.
#[cfg(feature = "hot-reload")]
#[no_mangle]
pub extern "C" fn ui_version() -> u64 {
    let s = env!("CARGO_PKG_VERSION");
    s.bytes().fold(0u64, |acc, b| acc.wrapping_mul(31).wrapping_add(b as u64))
}
