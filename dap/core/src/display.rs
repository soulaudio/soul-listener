//! Display port: the capability a board (`board-h7a3`) or a host simulator
//! must supply to draw UI onto the physical e-ink panel.
//!
//! The actual drawing toolkit — flexbox layout, widgets, panel timing specs
//! — is *not* reinvented here. It already exists, embassy-free and
//! `no_std`/`embedded-graphics`-based, in this repo's root workspace
//! (`crates/eink/eink-system`, `crates/eink/eink-components`,
//! `crates/eink/eink-specs`) and is consumed in place via the path deps in
//! `dap/Cargo.toml`'s `[workspace.dependencies]`. This module's own
//! contribution is narrow: [`Display`] (the bound screen code writes
//! against) and [`EinkRefresh`]/[`DisplayError`] (the one capability the
//! toolkit — which only knows how to compute a layout and draw pixels into
//! a `DrawTarget` — doesn't model: telling the physical panel to actually
//! show what was drawn). The rest of this module is a curated re-export
//! surface so screen code (a later task) can pull everything it needs from
//! `dap_core::display` alone, without its own path deps into `crates/eink`.

use embedded_graphics::draw_target::DrawTarget;
pub use embedded_graphics::pixelcolor::Gray4;
pub use embedded_graphics::primitives::Rectangle;

/// Why an [`EinkRefresh`] call failed.
///
/// Deliberately coarse and `Copy` (no borrowed/board-specific data): the two
/// concrete faults a SPI-attached e-ink controller (e.g. the
/// SSD1680/IL0373/UC8151 family — see [`Controller`]) can hit from a refresh
/// call are "the transfer itself failed" and "the panel's `BUSY` line never
/// cleared within the controller's own refresh-time budget" (see
/// [`DisplaySpec::full_refresh_duration`]/[`DisplaySpec::partial_refresh_duration`]).
/// A board driver maps its concrete HAL error (SPI, GPIO) to one of these; a
/// host simulator can return `Ok` unconditionally or script a failure for
/// tests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayError {
    /// The transfer to the controller (SPI write, command sequence, etc.)
    /// itself failed.
    Bus,
    /// The controller's `BUSY` line didn't clear within the expected
    /// refresh-time budget.
    Timeout,
}

/// The e-ink-specific half of showing a frame: committing whatever's been
/// drawn into a [`Display`]'s pixel buffer (via its `DrawTarget` impl) out
/// to the physical panel.
///
/// E-ink panels don't show new pixel data for free the way an LCD/OLED
/// does on every write — the controller must be told to run a *refresh
/// waveform* (a sequence of voltage pulses that physically flips the
/// panel's ink particles), and that step is slow enough (hundreds of ms to a
/// few seconds, see [`DisplaySpec`]) that it's a distinct, explicit
/// operation rather than something [`DrawTarget::draw_iter`] can trigger on
/// its own.
pub trait EinkRefresh {
    /// Run a full-panel refresh waveform: slower and flashier (it typically
    /// flashes the whole panel black/white a few times — see
    /// [`DisplaySpec::flash_count_full`]) but clears any ghosting
    /// accumulated by prior partial refreshes. Use when swapping to a
    /// materially different screen (track change, menu navigation).
    ///
    /// # Errors
    /// See [`DisplayError`].
    fn full_refresh(&mut self) -> Result<(), DisplayError>;

    /// Run a partial refresh limited to `area`: faster and flash-free, but
    /// accumulates ghosting over repeated use — see
    /// [`DisplaySpec::ghosting_rate_partial`]. Use for small, frequent
    /// updates within an otherwise-static screen (a progress tick, a volume
    /// HUD).
    ///
    /// # Errors
    /// See [`DisplayError`].
    fn partial_refresh(&mut self, area: Rectangle) -> Result<(), DisplayError>;
}

/// The board's e-ink panel capability: anything screen-rendering code draws
/// onto with `embedded-graphics` 0.8 primitives (fixed to the panel's
/// 4-bit-grayscale [`Gray4`] color space) *and* can command to physically
/// refresh.
///
/// A plain bound alias, blanket-implemented for every type that satisfies
/// it, rather than a hand-written trait with its own methods: the
/// [`DrawTarget`] half is the eink-system/eink-components toolkit's own
/// contract (every container/widget re-exported below renders through it),
/// and [`EinkRefresh`] is this crate's one addition. Deliberately
/// `dyn`-free, matching every other port in this crate
/// ([`crate::ports::Storage`], [`crate::ports::Power`],
/// [`crate::ports::Controls`]): a concrete, monomorphized implementor — a
/// real SSD1680/IL0373 SPI driver on `board-h7a3`, or a host
/// `eink-emulator`/`embedded-graphics-simulator` window — no trait objects.
pub trait Display: DrawTarget<Color = Gray4> + EinkRefresh {}

impl<T> Display for T where T: DrawTarget<Color = Gray4> + EinkRefresh {}

// ── Toolkit re-exports ──────────────────────────────────────────────────
//
// Named (not glob) re-exports, so this surface only changes when this file
// changes, and so eink-system's `render::LayoutResult` (an older,
// `Box`-based tree type the layout/container code below doesn't actually
// use) doesn't collide with `layout::LayoutResult` (the one `Layout::layout`
// — and every `Layout` impl: `VStack`/`HStack`/`Spacer`/`FlexLayout` —
// actually returns).
pub use eink_components::{
    button::{Button, ButtonStyle},
    icon::{Icon, IconType},
    label::{Label, LabelBuilder, TextSize},
    progress_bar::ProgressBar,
};
pub use eink_specs::{
    displays, quirks_for_controller, ColorMode, Controller, ControllerQuirks, DisplaySpec,
    PanelType, Quirk,
};
pub use eink_system::{
    containers::{HStack, Spacer, VStack},
    flex::{ChildLayout, FlexLayout},
    layout::{Constraints, Layout, LayoutResult},
    render::{
        is_visible, render_background, render_layout_tree, render_layout_with_background,
        Renderable,
    },
    style::{Align, Dimension, Edges, FlexDirection, Justify, Style},
};
