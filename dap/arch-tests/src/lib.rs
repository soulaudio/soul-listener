//! Architecture tests: the STM32H7A3's real memory map, and this DAP's
//! per-region SRAM budgets against it.
//!
//! `tests/budget.rs` carries the actual content:
//!
//! - A `memory_map` module: the H7A3's real memory regions (user SRAM, DTCM,
//!   ITCM, backup SRAM, flash, CPU clock), as documented consts citing the
//!   datasheet — ground truth, not this DAP's opinion about how to spend it.
//! - This DAP's split of `memory_map::USER_SRAM` into per-region budgets
//!   (audio path, e-ink framebuffer, catalog index, DMA staging) plus a
//!   `memory_budget_fits_user_sram` test that prints the whole split as a
//!   running-total/margin report and asserts it actually fits.
//! - Size guards pinning real `soul-audio-core` / `soul-audio-engine` /
//!   `soul-audio-index` / `dap-core` types against those budgets, all GREEN
//!   at the DAP's real `PRELOAD = 1`:
//!   - `any_decoder_fits_the_h7a3_decoder_budget` — GREEN (the
//!     `embedded-profile` feature drops the dead surround-Opus variant).
//!   - `deck_fits_the_h7a3_audio_path_budget` — GREEN: at `PRELOAD = 1` (one
//!     preloaded next track — all gapless needs), `Deck` fits the 704 KB
//!     audio-path budget with headroom.
//!   - `engine_overhead_is_tiny_on_top_of_the_deck_it_wraps` — GREEN: the
//!     `Engine`'s own bookkeeping (cursor + repeat queue + preload handles)
//!     adds negligible overhead on top of whatever `Deck` it wraps.
//!   - `index_reader_fits_the_catalog_index_budget` — GREEN: the `.soulidx`
//!     reader is a no-alloc header cache, nowhere near its 32 KiB budget.
//!   - `audio_player_fits_the_h7a3_audio_path_budget` — GREEN for the same
//!     reason as `deck_fits_...`: `AudioPlayer` wraps an `Engine` which wraps
//!     a `Deck`, inheriting the same (now-fitting) footprint.
//!
//! A failing run here means: the DAP's real-code footprint, as shaped today,
//! will not fit the H7A3 board. A future JIT-preload change (building the
//! next track's decoder lazily instead of holding it fully-built) would let
//! `AUDIO_PATH_BUDGET` shrink further, but isn't needed to pass today.

pub mod candidates;
