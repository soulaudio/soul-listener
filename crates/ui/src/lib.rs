//! Application UI layer — screen definitions, navigation state, component composition.
//!
//! This crate is `no_std` by default; it only uses `core` + `heapless`.

#![cfg_attr(not(test), no_std)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::panic)]
#![deny(clippy::expect_used)]


pub mod navigation;
pub mod now_playing;
pub mod screen;
