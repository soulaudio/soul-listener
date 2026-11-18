# Architecture enforcement — fitness functions so the stack can't drift

> Each embedded-standard rule the audit identified becomes a **gate that fails
> CI on drift** (an "architecture fitness function"), across the whole stack
> (soul-audio-core → dap-core → eink → board). Research-backed; see the audit
> for the industry baseline (MISRA Dir 4.12 no-alloc, ISO-26262/IEC-61508,
> ANSSI Rust rules, safety-critical-rust-coding-guidelines, cargo-call-stack).

## The rules and their enforcement

| # | Rule (standard) | Enforcement (fails CI on drift) | Status |
|---|---|---|---|
| 1 | **No heap allocation** (MISRA Dir 4.12) | An embedded-path binary with **no `#[global_allocator]`** — the linker errors (`no global memory allocator found`) the instant any crate pulls `alloc` ([rust#54010]). Plus the runtime counting-allocator test (`no_alloc.rs`, 0 allocs/format). | **Audio path: enforced** (`tools/mcu-stackprobe` has no allocator → soul-audio-core + the open/decode path is link-proven alloc-free). **Display path: open** — `eink-system` pulls `alloc`, so board-h7a3 carries a documented 4 KB shim; the full-stack gate activates once the eink-ui no-alloc redesign (C) lands. |
| 2 | **No `unsafe`** (ANSSI, safety-critical Rust) | `#![forbid(unsafe_code)]` at every embedded-path crate root + a **CI coverage check** that greps every such crate and fails if one lacks it. Host-only exceptions (eink-emulator/winit) explicitly allowlisted. | **Partial** — soul-audio-core ✓, dap-core ✓; **missing on eink-system/components/specs and dap-sim**. Add + gate. |
| 3 | **No unbounded recursion** (bounded-stack prerequisite) | `#![deny(unconditional_recursion)]` on embedded crates (catches the obvious) + the **stack-budget gate** as the backstop (real recursion blows the budget) + the invariant documented. Full call-graph proof = `cargo-call-stack` (a future stronger gate; currently pinned to a dead nightly). | **Add** the deny + document. |
| 4 | **Bounded stack** (WCSU + margin) | `mcu-stack-budget` CI gate: `-Z emit-stack-sizes` on the real thumbv7em binary decoding the **worst-case format**, budget + ~30% margin; verified to go red on regression. | **Enforced** ✓ |
| 5 | **Bounded SRAM** (per-region budget) | `dap-arch-tests/budget.rs`: `size_of` per region (incl. the externalized scratch pool), honest total asserted ≤ 1.18 MB. | **Enforced** ✓ |
| 6 | **Battery regression** (DVFS/energy model) | `dap-core::power::model` tests: clock-headroom-per-format ≤ VOS3, runtime ≥ floor, wakeup-rate. Honest: a datasheet-grounded **model**, not a measurement; real-power HIL regression = Phase E. | **Enforced (as a model)** ✓ |

[rust#54010]: https://github.com/rust-lang/rust/issues/54010

## Why fitness functions (not just docs)
A rule that lives only in a doc drifts the first time someone adds a crate, a
`Box`, a recursive helper, or a fat decoder in a PR. A rule wired to a failing
CI gate **cannot** drift silently — the PR goes red. That's the whole point:
lock each standard behind a gate so "we fit the H7A3, alloc-free, unsafe-free,
bounded-stack" stays true forever, not just today.

## Implementation (this change)
- **Rule 2**: add `#![forbid(unsafe_code)]` to `eink-system`/`eink-components`/
  `eink-specs` (no_std toolkit) and `dap-sim`; allowlist `eink-emulator`
  (winit/host). Add a CI check (`tools/check-forbid-unsafe.sh` or an xtask step)
  that enumerates embedded-path crates and fails on any missing it.
- **Rule 3**: `#![deny(unconditional_recursion)]` on soul-audio-core, dap-core,
  the eink no_std crates; document the no-recursion invariant.
- **Rule 1 (audio, make explicit)**: a dedicated no-`#[global_allocator]`
  thumbv7em check covering `dap-core` (the full logic layer, not just
  soul-audio-core) — if `dap-core`'s audio path ever pulls `alloc`, the link
  fails. Document `mcu-stackprobe`'s no-allocator property as the gate.
- **Tracking (Rule 1, display)**: a test/doc marker for the `eink-system` alloc
  leak so the full-stack no-alloc gate is wired and flips green the moment C
  removes `eink-system`'s `Box`/boxed-`heapless::Vec`.

## Deferred (gated on hardware or C)
- Full-stack no-alloc link gate → after eink-ui (C) makes `eink-system` no-alloc.
- QEMU dynamic stack-watermark (Rule 4 cross-check) → one-time admin QEMU install.
- `cargo-call-stack` call-graph recursion/WCSU proof → needs a compatible nightly.
- Real-power HIL battery regression (Rule 6) → Phase E (Joulescope/Otii on board).
