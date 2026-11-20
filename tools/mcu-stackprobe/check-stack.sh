#!/usr/bin/env bash
# tools/mcu-stackprobe/check-stack.sh
#
# Static CI gate for soul-audio-core's on-target (thumbv7em-none-eabihf /
# Cortex-M7) open-path stack transient -- see
# docs/plans/2026-11-17-firmware-selfimproving-loop-roadmap.md Phase A4.
#
# Builds the mcu-stackprobe harness (src/main.rs: open_seekable ->
# Deck::load -> Deck::pump, run TWICE back to back -- a 44.1 kHz WAV episode
# (forces a real Resampler construction too), then a Matroska/WebM-wrapped
# FLAC episode, the worst-case open-path format -- with the Deck in a
# `static` so it's off main's own stack -- board-representative) on nightly
# with `-Z emit-stack-sizes`, reads the linked ELF's `.stack_sizes` section
# via `llvm-readobj --stack-sizes`, and asserts two budgets:
#
#   1. main-frame budget -- the `__cortex_m_rt_main` entry frame, i.e. the
#      REAL open-path stack transient this whole harness exists to measure.
#      Baseline measured 2026-11-17 (commit d97c820), WAV-only: 326,096 B.
#      Re-measured 2026-11-18 once the probe actually exercises the
#      Matroska-FLAC worst case too (src/main.rs's episode 2) AND after
#      soul-audio-core shrank MatroskaFlacReader's embedded-profile
#      SimpleBlock cap 48 KiB -> 28 KiB (soul-audio-core commit 7f01902):
#      179,096 B -- SMALLER than the stale WAV-only baseline, not larger:
#      with two episodes in one `main` body, LLVM's inliner pulls several
#      calls (open_seekable/Deck::load) back out of `main` into their own
#      frames rather than inlining them twice, shifting weight onto those
#      functions instead -- see point 2.
#   2. largest-other-frame budget -- the single largest OTHER function frame
#      on the path (today:
#      MatroskaDecoder<SliceSource,..>::new_in, which now has
#      from_demuxer_in's A_FLAC arm AND the nested
#      FlacDecoder<MatroskaFlacReader<..>,..>::new_in inlined into it --
#      the Phase A3 "one edge" this gate previously tracked as a separate,
#      smaller-looking frame purely because it was reachable but unexercised).
#      Baseline measured 2026-11-17 (WAV-only, Matroska-FLAC type-reachable
#      but never actually decoded): 152,024 B. Re-measured 2026-11-18 (both
#      episodes actually exercised + the 28 KiB cap): 124,560 B.
#
# A future decoder/format addition that inflates either number past budget
# fails this script (and CI) immediately, with the real measured numbers
# printed so the failure is self-explanatory without re-running anything
# by hand.
#
# This is purely STATIC: `-Z emit-stack-sizes` records each function's own
# worst-case stack-frame size at compile time, straight from the linked
# ELF; it never runs the ARM binary. No QEMU (needs admin rights on this
# machine -- deferred, see the roadmap's Phase A1/A2).
#
# Why a shell script and not an xtask subcommand: mcu-stackprobe is
# deliberately its OWN standalone single-crate `[workspace]` (own
# Cargo.lock, nightly-only `-Z emit-stack-sizes`, its own .cargo/config.toml
# override -- see that file's comment) kept OUTSIDE the root workspace that
# xtask is a member of, so it never inherits dap's/the root's chip
# assumptions. Folding this gate into xtask would mean either making
# mcu-stackprobe a root-workspace member (defeating its whole reason for
# being standalone) or shelling out from xtask anyway -- so a plain,
# portable (bash on Linux CI; Git Bash locally on Windows) script living
# right next to the crate it measures is the simpler, more local fix.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
cd "$SCRIPT_DIR"

TARGET="thumbv7em-none-eabihf"
ELF="target/${TARGET}/release/mcu-stackprobe"

# ~25-35% headroom above the measured baselines cited above (the task's
# "~20% above" guidance, rounded to clean KiB numbers). Bump these -- with a
# comment updating the cited baseline -- if a legitimate feature addition
# grows the open path for a good reason.
MAIN_FRAME_BUDGET=$((224 * 1024))     # 224 KiB; baseline 179,096 B (~174.9 KiB)
# 208 KiB; baseline 185,048 B (~180.7 KiB). Re-based 2026-11-20: the largest
# other frame is now `dispatch::open_seekable` itself, which inlines every
# decoder constructor into one frame (previously the gate caught the un-inlined
# `MatroskaDecoder::new_in` at 124,560 B; the stack-metric-correction + per-format
# measurement-harness changes shifted the inlining so the whole open_seekable
# frame is the measured peak). This is a STATIC per-function number only — the
# DYNAMIC on-target open transient (QEMU, `check-stack-qemu.sh`) is ~297 KiB
# worst-case and fits the board's 592 KiB headroom with ~295 KiB to spare, so
# this larger static frame is well within budget on real hardware.
LARGEST_FRAME_BUDGET=$((208 * 1024))  # 208 KiB; baseline 185,048 B (~180.7 KiB)

echo "== mcu-stackprobe stack-budget gate =="
echo "target:              $TARGET"
echo "main-frame budget:   $MAIN_FRAME_BUDGET B"
echo "largest-frame budget: $LARGEST_FRAME_BUDGET B"
echo

if ! command -v rustup >/dev/null 2>&1; then
  echo "error: rustup not found on PATH" >&2
  echo "       this gate needs: nightly toolchain, its thumbv7em-none-eabihf target, and its llvm-tools-preview component" >&2
  exit 1
fi

# Idempotent (rustup no-ops if already installed). CI installs these as
# separate, clearly-logged steps (see .github/workflows/ci.yml's
# mcu-stack-budget job); doing it here too keeps this script self-sufficient
# for a local run.
rustup target add "$TARGET" --toolchain nightly >/dev/null
rustup component add llvm-tools-preview --toolchain nightly >/dev/null 2>&1 || true

echo "-- building mcu-stackprobe (nightly, release, $TARGET, -Z emit-stack-sizes) --"
# IMPORTANT: the RUSTFLAGS env var does not MERGE with .cargo/config.toml's
# `rustflags` array -- it overrides it outright (cargo's documented
# precedence: env RUSTFLAGS wins over config-file rustflags, full stop, no
# combining). This crate's link needs `-Tlink.x -Tdefmt.x` (see
# .cargo/config.toml's own comment on why those come from the inherited
# root config rather than this crate's own), so every rustflag this build
# needs -- the two link args AND the measurement flag -- must be supplied
# together in this ONE RUSTFLAGS value, or the link fails ("region 'FLASH'"/
# "cannot find linker script" style errors) or the measurement flag silently
# doesn't apply.
RUSTFLAGS="-C link-arg=-Tlink.x -C link-arg=-Tdefmt.x -Z emit-stack-sizes" \
  cargo +nightly build --release --target "$TARGET"

if [ ! -f "$ELF" ]; then
  echo "error: expected ELF not found at $ELF after a successful build" >&2
  exit 1
fi

SYSROOT="$(rustc +nightly --print sysroot)"
HOST_TRIPLE="$(rustc +nightly -vV | sed -n 's/^host: //p')"
LLVM_READOBJ=""
for cand in \
  "$SYSROOT/lib/rustlib/$HOST_TRIPLE/bin/llvm-readobj" \
  "$SYSROOT/lib/rustlib/$HOST_TRIPLE/bin/llvm-readobj.exe"
do
  if [ -f "$cand" ]; then
    LLVM_READOBJ="$cand"
    break
  fi
done
if [ -z "$LLVM_READOBJ" ]; then
  echo "error: llvm-readobj not found under $SYSROOT/lib/rustlib/$HOST_TRIPLE/bin/" >&2
  echo "       install it with: rustup component add llvm-tools-preview --toolchain nightly" >&2
  exit 1
fi

echo "-- reading .stack_sizes via llvm-readobj --"
STACKSIZES_JSON="$(mktemp)"
trap 'rm -f "$STACKSIZES_JSON"' EXIT
"$LLVM_READOBJ" --stack-sizes --elf-output-style=JSON "$ELF" > "$STACKSIZES_JSON"

eval "$(python3 "$SCRIPT_DIR/parse-stack-sizes.py" "$STACKSIZES_JSON")"

echo
echo "measured:"
echo "  main frame (__cortex_m_rt_main): $MAIN_FRAME B"
echo "    $MAIN_NAME"
echo "  largest other frame:            $LARGEST_FRAME B"
echo "    $LARGEST_NAME"
echo

FAIL=0
if [ "$MAIN_FRAME" -gt "$MAIN_FRAME_BUDGET" ]; then
  echo "FAIL: main frame $MAIN_FRAME B exceeds budget $MAIN_FRAME_BUDGET B (over by $((MAIN_FRAME - MAIN_FRAME_BUDGET)) B)" >&2
  FAIL=1
fi
if [ "$LARGEST_FRAME" -gt "$LARGEST_FRAME_BUDGET" ]; then
  echo "FAIL: largest other frame $LARGEST_FRAME B exceeds budget $LARGEST_FRAME_BUDGET B (over by $((LARGEST_FRAME - LARGEST_FRAME_BUDGET)) B)" >&2
  echo "      ($LARGEST_NAME)" >&2
  FAIL=1
fi

if [ "$FAIL" -ne 0 ]; then
  echo
  echo "mcu-stack-budget: FAILED -- the open-path stack transient grew past its CI budget." >&2
  echo "Either shrink the path (e.g. externalize a new decoder's scratch instead of" >&2
  echo "stack-allocating it) or, if the growth is deliberate and the on-chip SRAM fit" >&2
  echo "is re-verified, bump the budget constant in this script with a comment citing" >&2
  echo "the new measured baseline." >&2
  exit 1
fi

echo "mcu-stack-budget: OK"
echo "  main:    ${MAIN_FRAME} / ${MAIN_FRAME_BUDGET} B"
echo "  largest: ${LARGEST_FRAME} / ${LARGEST_FRAME_BUDGET} B"
