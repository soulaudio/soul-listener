//! Bluetooth audio/control — STM32WB55 HCI interface, BLE Audio (LE Audio, LC3).
//!
//! This crate is `no_std` by default; it only uses `core` + `heapless`.

#![cfg_attr(not(test), no_std)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::panic)]
#![deny(clippy::expect_used)]


pub mod hci;
pub mod state;
