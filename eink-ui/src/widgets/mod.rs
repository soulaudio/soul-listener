//! The widget library. Leaves: [`Text`], [`Container`], [`Spacer`],
//! [`ProgressBar`], [`Button`]. Flexbox containers: [`Flex`] (via [`row`] /
//! [`col`]) and [`Stack`]. The browse primitive: [`List`]. `Icon` and an
//! `alloc`-gated owned-text `Label` follow in a later phase.

mod button;
mod container;
mod flex;
mod icon;
mod list;
mod progress;
mod spacer;
mod stack;
mod text;

pub use button::Button;
pub use container::Container;
pub use flex::{col, row, Flex};
pub use icon::{Glyph, Icon};
pub use list::List;
pub use progress::ProgressBar;
pub use spacer::Spacer;
pub use stack::Stack;
pub use text::Text;
