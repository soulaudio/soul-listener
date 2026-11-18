//! `eink-ssd1677` — a `no_std` async driver for the SSD1677 / SSD2677 e-ink
//! controller (Good Display GDEM0397T81P, 800×480, Carta 1200).
//!
//! Salvaged verbatim from the retired v1 firmware (`crates/firmware/src/display/
//! driver.rs`) when that stack was deleted — the one piece of v1 worth keeping.
//! It is self-contained: the small display-driver traits it used to import from
//! the v1 `platform` crate now live in [`mod@traits`], so this crate depends only
//! on `embedded-hal`/`embedded-hal-async` 1.0 and `embedded-graphics` 0.8.
//!
//! A Phase-E `board-h7a3` display adapter will wrap [`Ssd1677`] (which is an
//! `embedded_graphics::DrawTarget`) into `dap_core`'s `Display` port. It is not
//! yet wired into the board (all ports are stubs until hardware exists); this
//! crate keeps the driver compiling and its byte-level init/refresh/BUSY tests
//! green in the `dap` workspace's host test job in the meantime.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

pub mod driver;
pub mod traits;

pub use driver::{
    Command, DisplayError, Ssd1677, BYTES_PER_ROW, FRAMEBUFFER_SIZE_1BPP, UPDATE_FULL,
    UPDATE_FULL_FAST, UPDATE_PARTIAL, UPDATE_POWER_OFF, UPDATE_POWER_ON,
};
pub use traits::{DisplayDriver, DisplayInfo, EinkDisplay, RefreshMode};

/// Display width in pixels (GDEM0397T81P).
pub const DISPLAY_WIDTH: u32 = 800;

/// Display height in pixels (GDEM0397T81P).
pub const DISPLAY_HEIGHT: u32 = 480;

/// Framebuffer size in bytes (800×480 at 1bpp = 48,000 bytes).
///
/// The SSD1677 B/W RAM uses 1bpp: 1 = white, 0 = black, MSB-first.
pub const FRAMEBUFFER_SIZE: usize = FRAMEBUFFER_SIZE_1BPP;
