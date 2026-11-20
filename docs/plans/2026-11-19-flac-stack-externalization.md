# Matroska-FLAC decoder-open stack transient — measurement correction (Phase A3)

**Status:** CORRECTED. An earlier revision of this doc (and the roadmap, the
memory note, and `check-stack-qemu.sh`'s warning) claimed the Matroska-FLAC
track-open transient was **~1080 KiB**, exceeding the board's 592 KiB AXI-SRAM
stack headroom and blocking on-chip track open. **That was a measurement bug.**
Measured correctly, the worst-case open transient is **~315 KiB and fits**, with
~277 KiB to spare. The board is **not** blocked by the decode-open path.

**Repo of record for the measurement tool:** `tools/mcu-stackprobe/` (this repo).
**Supersedes the "externalize to cut 1 MB" plan** this file previously held.

## The bug

`tools/mcu-stackprobe` paints the free stack, runs the decode workload, then
scans for the deepest overwritten sentinel. The reported "peak" was computed as:

```
used = _stack_start (top of RAM) − deepest_overwritten_address
```

i.e. the whole stack depth **from the top of RAM**. But the harness's own `main`
is enormous — it holds *two* episodes' decoders (`decoder`, `decoder2`), both
fixtures, the `Deck`-building temporaries, and every measurement local at once —
so its frame sits ~780 KiB below the top of RAM before the decode path is even
called. That ~780 KiB got added to every reading and then compared against the
*board's* 592 KiB headroom. A real board task never carries the probe's `main`
frame, so the comparison was apples-to-oranges and over-reported the transient
by ~3×. (Measuring immediately after `paint`, with nothing run, reported 737 KiB
"used" — the tell.)

The correct, layout-independent metric is the growth **below the call site**:

```
transient = painted_hi (sp at the paint point) − deepest_overwritten_address
```

This is the extra stack a task incurs by calling into `open_seekable`, which is
exactly what has to fit under the board's free-stack headroom. Validated: it
reads **0** immediately after `paint`.

## Corrected numbers

QEMU `mps2-an500` (Cortex-M7, embedded-profile), `check-stack-qemu.sh`,
2026-11-19, **full buffers** (28 KiB SimpleBlock frame, 16 KiB codec_private,
512-point seek table — nothing shrunk):

| open path | transient (growth below call site) |
|---|---|
| WAV (+ resampler construction) | **78 KiB** |
| Matroska/WebM-wrapped FLAC (worst case) | **315 KiB** |
| board AXI-SRAM stack headroom | 592 KiB |

**315 KiB fits with 277 KiB to spare.** `open+pump` does not exceed `open`.

For context, the per-function `-Z emit-stack-sizes` chain below `main`
(`open_seekable` 79K → `MatroskaDecoder::new_in` 124K → `FlacDecoder::new_in`
111K → `MatroskaFlacReader::new` 48K → demuxers) sums to ~380 KiB of *static*
frames, of which ~315 KiB is live simultaneously at the deepest point — fully
consistent with the corrected dynamic number, and with `main` (the other ~700 KiB
of the old "1080") correctly excluded.

## What this changes

- **No board blocker.** The firmware can open any supported track (incl.
  Matroska-FLAC) within its stack. Roadmap Phase A3 is resolved by *measuring it
  right*, not by a refactor.
- **The §7 byte-store externalization is now optional margin work, not a
  prerequisite.** It would roughly halve the 315 KiB (externalizing the ~58 KiB
  of inline byte buffers that the by-value construction chain multiplies ~5×),
  buying headroom for the task's own frame + UI/driver stack above the decode
  path. Worth doing eventually; **not** urgent, and explicitly **not** a board
  blocker. Tracked as a nice-to-have, de-scoped from the critical path.
- **`forbid(unsafe_code)` and the decoder API are untouched** — nothing was
  changed in `soul-audio-core` for this correction; the fix is entirely in the
  measurement tool.

## The fix (this change)

- `tools/mcu-stackprobe/src/main.rs`: `measure()` now returns
  `(transient_below_paint, total_from_ram_top)`; the report leads with the
  transient and labels the old top-of-RAM number "probe-total (context)".
  Added an `after_paint` self-check (expects 0) so the metric can't silently
  regress.
- `tools/mcu-stackprobe/check-stack-qemu.sh`: parses the transient, compares it
  to the 592 KiB headroom, prints **OK … fits** (or a real warning only if a
  future change genuinely overflows), and documents the corrected metric in its
  header.

## Cross-references to fix (done alongside this)

- The memory note "QEMU stack finding" (claimed the transient exceeds headroom) —
  corrected.
- `check-stack.sh`'s static gate is unaffected (it measures per-function frames,
  which were never wrong); its budgets stay.
- `dap/board-h7a3/src/main.rs`'s `APP_CELL` note reasoned from static size about a
  single-copy App move; the dynamic probe shows construction adds ~0 below the
  call site, so the board's "it fits" conclusion stands (for the better reason
  now measured). The board now also builds the `App` with `StaticCell::init_with`
  (in-place) instead of `init` (construct-then-move) — see "Best practices" below.

## Stack-measurement best practices (web-researched, 2026-11-19)

Cross-checked against current references; these are why the fix above is the
*right* fix, not just *a* fix.

- **Measure the SP delta from a reference point, not absolute depth.** Memfault's
  "Measuring Stack Usage the Hard Way" describes per-call measurement as: *re-apply
  the fill pattern across the unused stack just before the call, then take the
  difference between the saved and measured stack pointers.* That delta is exactly
  our corrected `painted_hi - deepest`. Measuring from the top of RAM
  (`_stack_start - deepest`) folds in the caller's own frame — the bug we hit.
- **Stack painting under-reports partially-written buffers.** A stack buffer that
  is allocated but only partly written keeps its fill pattern in the untouched
  bytes, so the scan counts it as unused. Our 315 KiB is therefore a *lower bound*;
  the 277 KiB of headroom is the real cushion against that conservatism.
- **Validate the rig with a zero-point and a known depth.** We added an
  `after_paint` read that must be 0 (catches the metric regressing to an absolute
  reference again); the harness was already validated against a known recursion
  depth. `cortex-m-stack` (crates.io) packages this same paint/scan if we ever want
  to drop the hand-rolled version.
- **Static vs dynamic are complementary.** `-Z emit-stack-sizes` (per-function
  frames; our `check-stack.sh`) and `cargo-call-stack` (whole-program max over the
  call graph, Cortex-M only) bound the worst case *statically* but can't see
  overlap/elision; the QEMU paint/scan measures what actually happens. Keep both —
  the static gate caught nothing wrong here (its per-function numbers were always
  right); only the dynamic *total* metric was miscomputed.
- **Large value into `static`: prefer `StaticCell::init_with` over `init`.** The
  docs are explicit — *"If T is big, [init] is likely to cause stack overflows.
  Consider init_with instead"* — because `init` constructs on the stack then moves
  (relying on RVO/NRVO that Rust does not guarantee), while `init_with`'s closure
  constructs in place. Applied to the board's `App`. For the optional margin
  refactor, `pin-init` (`pin_init!`, from Rust-for-Linux) is the heavier tool for
  in-place-initializing big structs without `unsafe` in *our* code; Rust's
  in-place-initialization project goal (2025H2) may make this first-class later.

Sources: Memfault *Measuring Stack Usage the Hard Way*
(interrupt.memfault.com/blog/measuring-stack-usage); `static_cell` docs
(docs.rs/static_cell — `StaticCell::init_with`); `cargo-call-stack`
(github.com/japaric/cargo-call-stack) and japaric's *static stack analysis*
(blog.japaric.io/stack-analysis); `cortex-m-stack` (crates.io/crates/cortex-m-stack);
`pin-init` (rust.docs.kernel.org/pin_init); Rust in-place-initialization goal
(goals.rust-lang.org/2025h2/in-place-initialization.html).
