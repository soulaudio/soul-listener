//! License gate: the **AAC decoder must NEVER be compiled into the shipped DAP
//! firmware.** AAC is a Via LA patent — **$15,000 one-time + $0.98 per unit** —
//! so each distributed device bundling an AAC decoder owes a per-unit royalty (a
//! clean-room pure-Rust decoder is still patent-bearing). The decoder lives in
//! `soul-audio-core` behind an opt-in `aac` cargo feature (default **OFF**); the
//! DAP's `embedded-profile` never enables it. This test asserts that — in the
//! resolved build this crate sees — the feature is off, so the patented decoder
//! is not linked into the firmware.
//!
//! If AAC playback is ever wanted, it would have to be re-added deliberately (and
//! licensed) — never by flipping this feature on. Rationale + cross-repo picture:
//! `docs/research/2026-11-18-licensing-ip-and-workarounds.md` §5.

/// `soul_audio_core::AAC_ENABLED` is `cfg!(feature = "aac")` evaluated inside
/// soul-audio-core, so it reflects how core was actually built in this graph.
#[test]
#[allow(clippy::assertions_on_constants)]
fn aac_decoder_is_never_enabled_on_the_dap() {
    assert!(
        !soul_audio_core::AAC_ENABLED,
        "AAC LICENSE VIOLATION: soul-audio-core was resolved with the `aac` feature \
         enabled, pulling the patented AAC decoder into the DAP firmware. AAC is a Via LA \
         patent ($15,000 one-time + $0.98/unit) and is DROPPED from every shipped soul \
         product. Remove `features = [\"aac\"]` from whatever dependency added it. See \
         docs/research/2026-11-18-licensing-ip-and-workarounds.md §5."
    );
}
