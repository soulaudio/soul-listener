# Reducing the Matroska-FLAC decoder-open stack transient (Phase A3)

**Status:** planned — measured, not yet implemented.
**Repo of record for the code:** sibling `soul-audio-core` (path-pinned into `dap/`).
**Depends on:** the AAC feature commit (`soul-audio-core` `81fc0ce`) as its base;
the decoder-memory-externalization spec §7 (`soul-audio-core`
`docs/superpowers/specs/2026-11-08-dap-export-and-blobs.md`) as the design it
finishes.
**Blocks:** the firmware actually opening a track on-chip. See the
self-improving-loop roadmap (`docs/plans/2026-11-17-firmware-selfimproving-loop-roadmap.md`)
Phase A3.

## The problem, measured

The board has **592 KiB** of AXI-SRAM stack headroom (see
`dap/board-h7a3/src/main.rs` and the `arch-tests`
`board_static_footprint_leaves_stack_room_in_linked_axi_sram` guard). The
worst-case track-open — a Matroska/WebM-wrapped FLAC stream — needs far more
than that.

Dynamic, on-target measurement (QEMU `mps2-an500`, Cortex-M7, embedded-profile,
`tools/mcu-stackprobe/check-stack-qemu.sh`), 2026-11-19:

```
open-path peak = 1080 KiB (1,106,768 B); open+pump peak = 1080 KiB; of 3409 KiB paintable
```

**1080 KiB — it exceeds the board's stack headroom by 488 KiB, and the whole
peak is in `open`, not `pump`.** A track cannot be opened on-chip until this is
roughly quartered (target **≤256 KiB**, which leaves the rest of the 592 KiB for
the UI/driver call stack above the decode path).

The static `-Z emit-stack-sizes` gate (`check-stack.sh`, MAIN 179 KiB + LARGEST
124 KiB) badly undercounts this: it reports the two biggest *single* frames, but
the open path stacks *many* large frames at once.

### Where the bytes are (per-function `.stack_sizes`, thumbv7em, release)

```
  179,096  __cortex_m_rt_main                                       (the harness; stands in for the board main)
   79,336  dispatch::open_seekable<SliceSource>                     holds the AnyDecoder result enum + temporaries
  124,560  formats::matroska::MatroskaDecoder::new_in              Matroska arm: moves in MatroskaDemuxer, builds FlacDecoder
  111,064  formats::flac::FlacDecoder<MatroskaFlacReader>::new_in   moves in MatroskaFlacReader, builds FlacDemuxer around it
   48,304  formats::matroska::MatroskaFlacReader::new               builds the 46.5 KiB reader
   17,656  demux::matroska::MatroskaDemuxer::new
    8,608  demux::flac::FlacDemuxer::new
```

Decisive detail: the dynamic peak (1,106,768 B) is **larger than the sum of
every distinct static frame** (1,024,896 B). That is only possible if the same
large values are live in more than one frame at once — i.e. big structs being
**moved by value through a deep construction chain**, so the source copy and the
destination copy both occupy stack during each `memcpy`.

### The inline byte buffers that get copied through the chain

The nesting is `FlacDecoder< FlacDemuxer< MatroskaFlacReader< MatroskaDemuxer<S> > > >`,
and each layer owns the next **by value**, with these inline arrays:

| Struct | Inline byte buffer | Size (embedded-profile) |
|---|---|---|
| `demux::matroska::MatroskaDemuxer` | `codec_private: [u8; MAX_CODEC_PRIVATE]` | 16 KiB |
| `demux::matroska::MatroskaDemuxer` | `lace_sizes: [u32; 256]` | 1 KiB |
| `formats::matroska::MatroskaFlacReader` | `codec_private: [u8; MAX_FLAC_CODEC_PRIVATE]` | 1 KiB |
| `formats::matroska::MatroskaFlacReader` | `frame: [u8; MAX_FLAC_FRAME]` | 28 KiB |
| `demux::flac::FlacDemuxer` | `seek_points: [(u64,u64); 512]` | 8 KiB |
| `formats::flac::FlacDecoder` | `bytes: [u8; BYTE_BUF]` | 4 KiB |

≈ **58 KiB of inline byte buffers**, carried inside a value that is constructed
at the leaf and then moved up through `MatroskaFlacReader::new` →
`FlacDecoder::new_in` → `MatroskaDecoder::new_in` → `open_seekable`. During each
move both copies are live, so that 58 KiB is paid several times over — the
mechanism behind the 1080 KiB.

(FLAC's **decode planes** are already externalized to caller scratch `P` — spec
§7, `FlacDecoder::scratch` / `plane_mut`. This plan extends that same externalization
to the *byte* buffers above, and removes the nesting that multiplies them.)

## The fix (two steps, both `forbid(unsafe_code)`-clean, no public API change)

### Step 1 — cheap caps (safe, small)

Gate `MAX_SEEK_POINTS` (512) down under `embedded-profile` (e.g. 32). Verified
correctness-safe: `seek_point_before` falls back to `(0,0)` and the decoder
bisects when a point is absent (`demux/flac.rs` `seek_point_before` /
`has_seek_table`), so a smaller table only coarsens seek granularity, never
breaks it. Saves 8 KiB × (its copies) ≈ 15–25 KiB. **Do not** shrink the Matroska
`codec_private` cap this way — Vorbis-in-Matroska setup headers legitimately run
several KiB, so a smaller inline cap risks truncation. `codec_private` is handled
by Step 2 (external scratch, no stack cost, so it can stay worst-case sized).

Step 1 alone is cosmetic against a 488 KiB overshoot; it is a down payment, not
the fix.

### Step 2 — single-owner `MatroskaFlac<S, P>`, byte buffers externalized

Collapse the 4-level `FlacDecoder< FlacDemuxer< MatroskaFlacReader< MatroskaDemuxer >>>`
nest into **one** owner that is constructed **once** (one `new_in` frame instead
of four), holding only:

- the source `S` and the `MatroskaDemuxer` state (minus its big byte arrays),
- `scratch: P` (the existing `&'static mut [i32]` pool), and
- **`usize` offsets** into `scratch` for every byte buffer that is currently
  inline (`frame`, both `codec_private`s, `seek_points`, `bytes`).

The byte buffers live in `scratch` (viewed as bytes via `bytemuck`, exactly the
spec §7 pattern already used for the FLAC planes and the WavPack staging buffer —
no `unsafe`, arena-style: transient `&mut` view → bump-allocate → record `usize`
offsets → store offsets, never a borrow). Because the owner now holds pointers,
not arrays, nothing large is moved during construction, and there is only one
construction frame.

- `decoder_scratch_len()` (`dispatch.rs`) grows by the externalized byte total
  (≈ 58 KiB → add to the `i32` budget), which moves those bytes from the
  **stack** to the **already-static** `SCRATCH` pool — the whole point.
- The `Demuxer`/`Decoder`/`SeekableDecoder` trait surfaces are unchanged; this is
  an internal restructure of the Matroska-FLAC path only. (Matroska-Opus/-Vorbis/
  -AAC share `MatroskaDemuxer`; keep its public reader API, just move its inline
  `codec_private`/`lace_sizes` behind the scratch cursor so all Matroska codecs
  benefit and none regress.)

Projected: removing 3 redundant construction frames (~110+124+48 KiB) and the
multiplied 58 KiB of byte buffers takes the peak well under 256 KiB. **Re-measure,
don't trust the projection** — the loop is `check-stack-qemu.sh` after each change.

## Validation (hard gates, in order)

1. `cargo test -p soul-audio-core` **and** `--features embedded-profile` — the
   full decode matrix: `decode`, `fuzz`, `properties`, `reference_differential`,
   `robustness`, `io_errors`, `matrix`. No decode regression is the bar; the
   `reference_differential` + `properties` suites are the real safety net for a
   decode-core change.
2. `tools/mcu-stackprobe/check-stack-qemu.sh` → open-path peak **≤ 256 KiB**.
3. `tools/mcu-stackprobe/check-stack.sh` static gate still green (bump its
   `MAIN`/`LARGEST` baselines *down* with the new measured numbers).
4. `forbid(unsafe_code)` intact in `soul-audio-core` (crate-root attribute; the
   `bytemuck` views are safe-API).

## Landing (cross-repo, per the pinning doc PR #20)

Steps 1–2 are `soul-audio-core` commits on top of `81fc0ce`. To reach `dap` CI:
push `soul-audio-core`, then bump `dap`'s `soul-audio-core` pin to the new rev and
update the `arch-tests` budget constants + `check-stack.sh` baselines in one `dap`
PR. (Same push + pin-bump flow the AAC feature needs — they can land together.)
