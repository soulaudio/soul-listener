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
//!   `soul-audio-index` / `dap-core` types against those budgets:
//!   - `any_decoder_fits_the_h7a3_decoder_budget` — GREEN (the
//!     `embedded-profile` feature drops the dead surround-Opus variant).
//!   - `deck_fits_the_h7a3_audio_path_budget` — `#[ignore]`d RED: each
//!     `PRELOAD` slot still holds a fully-built decoder; needs JIT preload.
//!   - `engine_overhead_is_tiny_on_top_of_the_deck_it_wraps` — GREEN: the
//!     `Engine`'s own bookkeeping (cursor + repeat queue + preload handles)
//!     adds negligible overhead on top of whatever `Deck` it wraps — the
//!     Deck/decoder overage above is not the sequencing layer's fault.
//!   - `index_reader_fits_the_catalog_index_budget` — GREEN: the `.soulidx`
//!     reader is a no-alloc header cache, nowhere near its 128 KiB budget.
//!   - `audio_player_fits_the_h7a3_audio_path_budget` — `#[ignore]`d RED for
//!     the same reason as `deck_fits_...`: `AudioPlayer` wraps an `Engine`
//!     which wraps a `Deck`, inheriting the same per-slot overage.
//!
//! A failing (non-ignored) run here means: the DAP's real-code footprint, as
//! shaped today, will not fit the H7A3 board. An `#[ignore]`d failure is not
//! a regression — it is this crate pinning the *exact* overage so the fix is
//! driven by a number. See each ignored test's reason string for the fix.
