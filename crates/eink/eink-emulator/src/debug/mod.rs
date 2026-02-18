//! Debug system for e-ink emulator
//!
//! Provides visual debug overlays, interactive inspector, and power monitoring.

#[cfg(feature = "debug")]
pub mod state;

#[cfg(feature = "debug")]
pub mod overlay;

#[cfg(feature = "debug")]
pub mod panel;

#[cfg(feature = "debug")]
pub mod power_graph;

#[cfg(feature = "debug")]
pub mod manager;

#[cfg(feature = "debug")]
pub mod inspector;

#[cfg(feature = "debug")]
pub use inspector::{Inspector, InspectorTab};

#[cfg(feature = "debug")]
pub use state::*;

#[cfg(feature = "debug")]
pub use manager::*;

#[cfg(feature = "debug")]
pub use overlay::OverlayRenderer;

#[cfg(feature = "debug")]
pub use panel::DebugPanel;

#[cfg(feature = "debug")]
pub use panel::PanelInfo;

#[cfg(feature = "debug")]
pub use power_graph::PowerGraph;
