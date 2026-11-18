//! The widget library (layer 3). v1 ships [`Text`] and [`Container`];
//! `Row`/`Col`/`Stack`/`Spacer`/`Button`/`ProgressBar`/`Icon`/`List` follow
//! in later phases (see the design spec's phasing section).

mod container;
mod text;

pub use container::Container;
pub use text::Text;
