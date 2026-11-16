//! Screens: discrete, self-contained render functions over
//! [`crate::display::Display`] — the board-agnostic half of the DAP's UI.
//!
//! [`crate::app::App::render`] is the only caller today, driving
//! [`now_playing`] — the one screen Phase 2a has. A later phase (2b) adds
//! the wheel-driven Browse screen alongside it.

pub mod now_playing;
