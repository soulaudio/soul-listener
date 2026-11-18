//! `eink-ui` — a `no_std`, `embedded-graphics`-native, immediate-mode UI
//! toolkit specialized for e-ink displays.
//!
//! # Core model
//!
//! Each frame you construct lightweight widget *values* and render them in
//! a single measure -> layout -> draw pass (see [`widget::Widget`]). There is
//! no retained tree and no diffing machinery, which fits e-ink's infrequent,
//! full-frame-oriented updates.
//!
//! By default this crate is **no-alloc**: composition is bounded (const
//! generics / fixed-size slices / [`heapless`]) with zero heap use. The
//! `alloc` feature unlocks unbounded children and owned/dynamic text for
//! desktop or large targets; `std` (which implies `alloc`) additionally
//! pulls in the [`testing`] module's framebuffer + PNG export for golden
//! tests.
//!
//! # Hard separation
//!
//! This crate depends on *only* `embedded-graphics` and `heapless` (plus an
//! optional `image` dependency gated behind `std`, used solely by the
//! golden-test harness). It knows nothing about any particular product or
//! board and is designed to be lifted out into its own repository.
#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]

#[cfg(feature = "alloc")]
extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod layout;
pub mod refresh;
pub mod theme;
pub mod widget;
pub mod widgets;

#[cfg(feature = "std")]
pub mod testing;

pub use layout::{Align, Axis, Children, Justify};
pub use refresh::{DirtyRegion, DirtyTracker, RefreshKind, RefreshPolicy};
pub use theme::{Role, Space, TextScale, Theme};
pub use widget::{Constraints, DrawError, Size, Widget};
pub use widgets::{col, row, Button, Container, Flex, List, ProgressBar, Spacer, Stack, Text};
