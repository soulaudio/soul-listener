//! Application UI layer — screen definitions, navigation state, component composition.
//!
//! This crate is `no_std` by default; it only uses `core` + `heapless`.

#![cfg_attr(not(test), no_std)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::panic)]
#![deny(clippy::expect_used)]
// TODO: Add rustdoc to all public items (tracked as tech debt)
#![allow(missing_docs)]


pub mod navigation;
pub mod now_playing;
pub mod screen;
