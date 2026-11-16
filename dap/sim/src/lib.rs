//! Host (std) simulation adapters implementing `dap-core`'s ports, for
//! desktop development and the headless host tests (`tests/audio_headless.rs`,
//! `tests/power_headless.rs`, `tests/device_headless.rs`, `tests/ui_headless.rs`).

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
