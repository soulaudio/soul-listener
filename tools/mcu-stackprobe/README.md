# mcu-stackprobe

Bare-metal `thumbv7em-none-eabihf` harness that forces soul-audio-core's real
`open_seekable` → `Deck::load` → `Deck::pump` call chain into a linked ELF so
`check-stack.sh` can measure on-target stack-frame sizes with
`-Z emit-stack-sizes`. See `src/main.rs`'s module doc comment for the full
measurement rationale (the two episodes it exercises, why they're both
needed, etc).

## This binary is ALSO the audio no-alloc gate (Rule 1)

This crate deliberately declares **no `#[global_allocator]`** anywhere in
its dependency graph. That is not an oversight — it is the second thing this
binary proves, for free, every time it links successfully:

> If any crate on the open→decode→pump path (`soul-audio-core`, and
> transitively anything it depends on) ever starts pulling in `alloc`
> (a `Box`, a `Vec`, a boxed `heapless::Vec`, …), the link fails with
> `error: no global memory allocator found but one is required`
> ([rust#54010]) — because there is nothing in this binary's graph to
> satisfy that requirement.

A successful build of `mcu-stackprobe` is therefore a **link-time proof**
that the audio decode path is alloc-free on real thumbv7em codegen, not just
on the host. This is the enforcement mechanism for Rule 1 (MISRA Dir 4.12,
no heap allocation) as it applies to the audio path specifically — see
`docs/plans/2026-11-17-architecture-enforcement-fitness-functions.md`.

The CI job that builds this crate (`mcu-stack-budget` in
`.github/workflows/ci.yml`) therefore doubles as both:

1. the stack-budget gate (its original, documented purpose — `main`'s frame
   and the largest other frame stay under their measured-baseline-plus-
   margin budgets), and
2. the audio no-alloc gate (implicit: the build step itself would fail to
   link, before the stack-size checks ever run, the moment the audio path
   stops being alloc-free).

**Display-path scope note:** this gate covers the *audio* decode path only.
`eink-system` currently pulls `alloc` (a documented, tracked gap — see
`dap/sim/tests/alloc_policy.rs`), so a *board-h7a3 whole-firmware* no-alloc
link gate isn't wired yet; `board-h7a3/src/main.rs` currently carries a
4 KB `embedded_alloc` shim precisely to satisfy that link requirement. The
full-stack gate activates once the eink-ui no-alloc redesign removes
`eink-system`'s allocation.

[rust#54010]: https://github.com/rust-lang/rust/issues/54010
