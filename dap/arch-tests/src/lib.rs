//! Architecture test: the STM32H7A3 audio-decode-path SRAM budget.
//!
//! The H7A3 has one AXI SRAM pool big enough (and DMA-capable) to hold the
//! audio path: ~1 MB total, shared with the e-ink framebuffer, the
//! catalog/SD scan buffers, and DMA staging (stacks live separately, in the
//! 128 KB DTCM, and are not part of this budget). `tests/budget.rs` encodes
//! the split as named consts and asserts that `soul_audio_core::AnyDecoder`
//! and `soul_audio_core::Deck` (at the DAP's real `PRELOAD = 2`) each fit
//! their slice.
//!
//! **These tests are EXPECTED TO FAIL today.** `AnyDecoder`'s `MultistreamOpusDecoder`
//! (surround Opus) variant alone blows the decoder budget, and each `Deck`
//! preload slot holds a fully-built decoder + resampler, blowing the
//! audio-path budget at `PRELOAD = 2`. A failing run is not a regression —
//! it is this crate doing its job: pinning the *exact* overage so the fix is
//! driven by a number, not a guess.
//!
//! A failure here means: the audio footprint, as `soul-audio-core` is shaped
//! today, will not fit the H7A3 board. The fix is **not** to drop any audio
//! format — every format `soul-audio-core` supports today is retained. The
//! fix is two independent changes to `soul-audio-core`:
//!
//! 1. An `embedded-profile` for `AnyDecoder` that drops the **dead**
//!    multichannel/surround Opus variant (`MultistreamOpusDecoder`) — dead
//!    weight on a stereo DAP, since `Deck` already rejects any decoder that
//!    doesn't resolve to mono/stereo (`DeckError::ChannelMismatch`). Every
//!    other variant (WAV, AIFF, FLAC, MP3, ALAC, AAC, single-stream Opus,
//!    Matroska, WavPack, CAF) stays exactly as it is.
//! 2. JIT (just-in-time) preload: today each `PRELOAD` slot holds an
//!    already-built `AnyDecoder` + resampler (~343 KB per slot); the fix
//!    defers building the next slot's decoder until it is actually needed,
//!    so the preload depth stops multiplying the per-decoder footprint.
//!
//! Until both land, this crate stays red by design and is **not** wired into
//! the CI gate (`.github/workflows/ci.yml`) — see the task that created it.
