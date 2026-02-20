//! E-Ink Layout System
//!
//! Flexbox-inspired layout engine for embedded e-ink displays.
//!
//! # Architecture
//!
//! - Core types: Dimension, Edges, Style, Constraints
//! - Flexbox engine: Full flexbox layout algorithm
//! - Containers: VStack, HStack, Spacer
//! - Rendering: Integration with embedded-graphics
//!
//! # Example
//!
//! ```no_run
//! use eink_system::prelude::*;
//!
//! let layout = VStack::<4>::new()
//!     .gap(8)
//!     .children([
//!         // Components here
//!     ]);
//! ```

#![cfg_attr(not(feature = "std"), no_std)]
#![warn(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::missing_errors_doc)]
// Pedantic lints that are too noisy for a layout/style engine crate:
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_lossless)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::return_self_not_must_use)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::should_panic_without_expect)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::no_effect_underscore_binding)]
#![allow(clippy::unused_self)]
#![allow(clippy::explicit_iter_loop)]
#![allow(clippy::unreadable_literal)]
// hex ARGB color constants in debug.rs
// Desktop/test crate — embedded_graphics MockDisplay has a 64×64 internal
// pixel buffer (> 512 bytes) that we cannot annotate (external crate).
#![allow(clippy::large_stack_arrays)]
// TODO: Add rustdoc to all public items (tracked as tech debt)
#![allow(missing_docs)]

pub mod containers;
#[cfg(feature = "debug")]
pub mod debug;
pub mod flex;
pub mod layout;
pub mod render;
pub mod style;

pub mod prelude {
    // Style system (public API)
    pub use crate::style::*;

    // Flex layout (public API - includes ChildLayout)
    pub use crate::flex::{ChildLayout, FlexLayout};

    // Containers (public API)
    pub use crate::containers::*;

    // Render utilities (public API)
    pub use crate::render::*;

    // Layout traits (public API)
    pub use crate::layout::{Constraints, Layout, LayoutResult};
}
