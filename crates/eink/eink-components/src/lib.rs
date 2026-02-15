//! E-Ink UI Components
//!
//! Reusable UI components built on eink-system layout engine.
//!
//! # Components
//!
//! - `Button` - Clickable button with label
//! - `Label` - Static text display
//! - `ProgressBar` - Visual progress indicator
//! - `Icon` - Simple icon representation
//!
//! # Example
//!
//! ```no_run
//! use eink_components::prelude::*;
//! use eink_system::prelude::*;
//!
//! let button = Button::new("Play")
//!     .style(ButtonStyle::primary());
//! ```

#![cfg_attr(not(feature = "std"), no_std)]

pub mod button;
pub mod icon;
pub mod label;
pub mod progress_bar;

pub mod prelude {
    pub use crate::button::*;
    pub use crate::icon::*;
    pub use crate::label::*;
    pub use crate::progress_bar::*;
}
