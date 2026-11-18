//! Display port: the capability a board (`board-h7a3`) or a host simulator
//! must supply to draw UI onto the physical e-ink panel.
//!
//! The drawing toolkit — flexbox layout + widgets — is the standalone,
//! no_std/no-alloc `eink-ui` crate, which `crate::screens` depends on directly.
//! This module's own contribution is narrow: [`Display`] (the bound screen code
//! writes against) and [`EinkRefresh`]/[`DisplayError`] — the one capability the
//! toolkit, which only draws pixels into a `DrawTarget`, doesn't model: telling
//! the physical panel to actually show what was drawn. It also re-exports the
//! panel *hardware* specs from `eink-specs` (controller timing/quirks), which a
//! board's real driver needs.

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

// ── Panel hardware specs ────────────────────────────────────────────────
//
// The drawing *toolkit* (flexbox layout + widgets) is `eink-ui`, which screen
// code (`crate::screens`) depends on directly. This module re-exports only the
// panel *hardware data* — controller timing/quirks/dimension specs — which a
// board's real display driver needs and which is not UI-toolkit concern.
pub use eink_specs::{
    displays, quirks_for_controller, ColorMode, Controller, ControllerQuirks, DisplaySpec,
    PanelType, Quirk,
};
