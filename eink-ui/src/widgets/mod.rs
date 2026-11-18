//! The widget library. Leaves: [`Text`], [`Container`], [`Spacer`]. Flexbox
//! containers: [`Flex`] (via [`row`] / [`col`]) and [`Stack`].
//! `Label`/`Button`/`ProgressBar`/`Icon`/`List` follow in later phases (see the
//! design spec's phasing section).

mod container;
mod flex;
mod spacer;
mod stack;
mod text;

pub use container::Container;
pub use flex::{col, row, Flex};
pub use spacer::Spacer;
pub use stack::Stack;
pub use text::Text;
