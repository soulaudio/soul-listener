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
    // Style system (public API)
    pub use crate::style::*;

    // Flex layout (public API - includes ChildLayout)
    pub use crate::flex::{FlexLayout, ChildLayout};

    // Containers (public API)
    pub use crate::containers::*;

    // Render utilities (public API)
    pub use crate::render::*;

    // Layout traits (public API)
    pub use crate::layout::{Constraints, Layout, LayoutResult};
}
