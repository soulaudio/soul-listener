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

pub mod layout;
pub mod style;
pub mod flex;
pub mod containers;
pub mod render;

pub mod prelude {
    pub use crate::layout::*;
    pub use crate::style::*;
    pub use crate::flex::*;
    pub use crate::containers::*;
    pub use crate::render::*;
}
