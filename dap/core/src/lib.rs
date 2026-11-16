//! Board-agnostic DAP logic and ports.
//!
//! `no_std`, no `unsafe`, no embassy: this crate must build for both the
//! host and embedded targets (e.g. `thumbv7em-none-eabihf`) without pulling
//! in any async runtime or hardware-specific dependency. Board crates
//! (e.g. `board-h7a3`) and host simulation adapters (`dap-sim`) implement
//! the ports defined here.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod app;
pub mod audio;
pub mod ports;
pub mod power;
