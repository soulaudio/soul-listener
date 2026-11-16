//! Host (std) simulation adapters implementing `dap-core`'s ports, for
//! desktop development and the headless audio test (`tests/audio_headless.rs`).

mod battery;
mod sink;
mod storage;

pub use battery::{BatteryKeyframe, SimBattery};
pub use sink::SimSink;
pub use storage::{FileSource, SimStorage, SimStorageError};
