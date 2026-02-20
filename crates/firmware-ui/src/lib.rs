//! firmware-ui - Hot-Reloadable UI Rendering Layer
// `no_std` only when explicitly requested via the `no-std` feature.
// The default build (no features) stays `std` so the cdylib crate-type
// compiles cleanly on the host in `cargo clippy --all-targets`.
// The xtask embedded check passes `--features no-std` to verify
// embedded-compatibility of this crate.
#![cfg_attr(feature = "no-std", no_std)]

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

pub mod screens;

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
    assert!(
        !emulator_ptr.is_null(),
        "render_ui: emulator_ptr must not be null"
    );
    let emulator = unsafe { &mut *emulator_ptr };
    if let Err(e) = render::render_onto_emulator(emulator) {
        eprintln!("[firmware-ui] render_ui error: {:?}", e);
    }
}

/// Manual ABI version -- bump this whenever the C-ABI signature of `render_ui`
/// (or any other `#[no_mangle]` exported function) changes.
/// The binary checks this at startup to ensure the loaded dylib matches.
#[cfg(feature = "hot-reload")]
pub const ABI_VERSION: u32 = 1;

/// ABI version check -- exported so the binary can verify the loaded dylib
/// was compiled with the same ABI contract.
#[cfg(feature = "hot-reload")]
#[no_mangle]
pub extern "C" fn ui_abi_version() -> u32 {
    ABI_VERSION
}

/// Load-time version of this dylib for hot-reload change detection.
///
/// Uses a [`std::sync::OnceLock`] initialised with the current nanosecond
/// timestamp at first call.  Because `OnceLock` is a fresh static each time
/// the dylib is loaded into memory, the value changes between hot-reloads even
/// though the package version string stays constant.
#[cfg(feature = "hot-reload")]
#[no_mangle]
pub extern "C" fn ui_version() -> u64 {
    use std::sync::OnceLock;
    static VERSION: OnceLock<u64> = OnceLock::new();
    *VERSION.get_or_init(|| {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(42)
    })
}
