#!/usr/bin/env bash
# tools/mcu-stackprobe/check-stack.sh
#
# Static CI gate for soul-audio-core's on-target (thumbv7em-none-eabihf /
# Cortex-M7) open-path stack transient -- see
# docs/plans/2026-11-17-firmware-selfimproving-loop-roadmap.md Phase A4.
#
# Builds the mcu-stackprobe harness (src/main.rs: open_seekable ->
# Deck::load -> Deck::pump, with the Deck in a `static` so it's off main's
# own stack -- board-representative) on nightly with `-Z emit-stack-sizes`,
# reads the linked ELF's `.stack_sizes` section via
# `llvm-readobj --stack-sizes`, and asserts two budgets:
#
#   1. main-frame budget -- the `__cortex_m_rt_main` entry frame, i.e. the
#      REAL open-path stack transient this whole harness exists to measure.
#      Baseline measured 2026-11-17 (commit d97c820): 326,096 B.
#   2. largest-other-frame budget -- the single largest OTHER function
#      frame on the path (today: FlacDecoder<MatroskaFlacReader<..>>::new_in,
#      which builds the Matroska-wrapped-FLAC decoder's residual/warmup
#      scratch in-place -- the "one edge" tracked as Phase A3). Baseline
#      measured 2026-11-17: 152,024 B.
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
# (e.g. closing the Phase A3 Matroska-FLAC edge with a real byte-arena
# seam) grows the open path for a good reason.
MAIN_FRAME_BUDGET=$((400 * 1024))     # 400 KiB; baseline 326,096 B (~318.6 KiB)
LARGEST_FRAME_BUDGET=$((200 * 1024))  # 200 KiB; baseline 152,024 B (~148.5 KiB)

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
