//! Music library management — FAT32 scan, metadata parsing, track database.
//!
//! # Modules
//!
//! - [`track`] — `Track` record and `AudioFormat` enum
//! - [`index`] — `TrackIndex<N>` fixed-capacity catalogue
//! - [`scanner`] — directory walk and extension filtering
//! - [`metadata`] — magic-byte format detection

#![cfg_attr(not(test), no_std)]
#![deny(clippy::unwrap_used)]
#![deny(clippy::panic)]
#![deny(clippy::expect_used)]
// TODO: Add rustdoc to all public items (tracked as tech debt)
#![allow(missing_docs)]


pub mod index;
pub mod metadata;
pub mod scanner;
pub mod track;

// Top-level re-exports for convenience
pub use index::{FullIndex, IndexError, SmallIndex, TrackIndex, MAX_TRACKS};
pub use metadata::detect_format;
pub use scanner::{ScanEntry, Scanner};
pub use track::{AudioFormat, Track};
