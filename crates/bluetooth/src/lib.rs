//! Bluetooth audio/control — STM32WB55 HCI interface, BLE Audio (LE Audio, LC3).
//!
//! This crate is `no_std` by default; it only uses `core` + `heapless`.

#![cfg_attr(not(test), no_std)]
// TODO: Add rustdoc to all public items (tracked as tech debt)
#![allow(missing_docs)]

pub mod hci;
pub mod state;
