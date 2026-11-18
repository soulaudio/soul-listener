//! Host (std) simulation adapters implementing `dap-core`'s ports, for
//! desktop development and the headless host tests (`tests/audio_headless.rs`,
//! `tests/power_headless.rs`, `tests/device_headless.rs`, `tests/ui_headless.rs`).
#![forbid(unsafe_code)]

mod battery;
mod controls;
mod display;
mod sink;
mod storage;

pub use battery::{BatteryKeyframe, SimBattery};
pub use controls::SimControls;
pub use display::SimDisplay;
pub use sink::SimSink;
pub use storage::{FileSource, SimStorage, SimStorageError};

/// Host `scratch_fn` for `dap_core::audio::AudioPlayer`/`dap_core::app::App`:
/// allocates a fresh, correctly-sized `Box<[i32]>` per track open.
///
/// The decoder owns this scratch and drops it with the decoder (see
/// `AudioPlayer`'s doc comment) -- on the host there's no pool to manage, so
/// a plain per-open heap allocation is the right tool; every sim test,
/// bench, and example in this crate passes this as the `scratch_fn` factory.
/// The board (future, PCB-blocked) will instead hand out a `&'static mut
/// [i32]` from a static pool -- see `board-h7a3/src/main.rs`'s smoke test for
/// the no_std shape of that.
pub fn sac_scratch() -> Box<[i32]> {
    vec![0i32; soul_audio_core::decoder_scratch_len()].into_boxed_slice()
}
