#!/usr/bin/env bash
# tools/mcu-stackprobe/check-stack-qemu.sh
#
# DYNAMIC on-target stack measurement under QEMU (firmware roadmap Phase A1/A2).
#
# Runs the mcu-stackprobe decode workload on `qemu-system-arm -machine mps2-an500`
# (a real Cortex-M7 FPGA image) and uses guest-side STACK-PAINTING to report the
# REAL dynamic stack high-water of soul-audio-core's open->decode->pump path. This
# is the dynamic complement to check-stack.sh's static `-Z emit-stack-sizes` gate:
# the static gate only sums MAIN_FRAME + the single LARGEST other frame, which
# badly undercounts a deep construction call chain. This runs the actual code and
# measures how far the stack really grew.
#
# QEMU is NOT vendored by this repo — install it separately:
#   Windows: winget install SoftwareFreedomConservancy.QEMU
#   Linux:   apt-get install qemu-system-arm   (package provides qemu-system-arm)
#   macOS:   brew install qemu
#
# This is now a BLOCKING CI gate (firmware roadmap Phase A2/A4 — wired into
# .github/workflows/ci.yml's `mcu-stack-budget-dynamic` job, which apt-installs
# qemu-system-arm on the runner). Two thresholds, both enforced below:
#   1. REGRESSION RATCHET — the worst measured per-format open transient must
#      stay under DYNAMIC_STACK_BUDGET_KIB (default 384; current worst is the
#      Matroska-FLAC episode at 297 KiB, so ~29% margin for codegen/upstream
#      drift). Override the env var to ratchet DOWN as the transient shrinks.
#   2. SAFETY FLOOR — the worst transient must also fit the board's AXI-SRAM
#      stack headroom (592 KiB). Redundant while the budget < headroom, but kept
#      explicit so raising the budget can never silently exceed the board.
# The earlier spurious "~1 MB blocker" that made a hard gate look impossible was
# a measurement bug (depth-from-RAM-top vs growth-below-call-site), fixed
# 2026-11-19; the real transient has always fit. See this script's metric note.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
cd "$SCRIPT_DIR"

# Locate qemu-system-arm: PATH first, then the default Windows install dir.
if ! command -v qemu-system-arm &>/dev/null; then
  if [ -x "/c/Program Files/qemu/qemu-system-arm.exe" ]; then
    export PATH="/c/Program Files/qemu:$PATH"
  else
    echo "ERROR: qemu-system-arm not found on PATH. Install QEMU (see this script's header)." >&2
    exit 1
  fi
fi
echo "== mcu-stackprobe dynamic stack measurement (QEMU mps2-an500, Cortex-M7) =="
echo "qemu: $(command -v qemu-system-arm)"

# Build + run the stack-painting harness. The .cargo/config.toml `runner` launches
# QEMU with semihosting; the guest prints its measurement and exits the emulator.
#
# RUSTFLAGS is set EXPLICITLY (mirroring check-stack.sh) and must carry the link
# args: the env var does NOT merge with .cargo/config.toml's `rustflags` — it
# REPLACES them wholesale (env wins, full stop). CI exports `RUSTFLAGS=-D warnings`
# globally, which would otherwise drop the inherited `-C link-arg=-Tlink.x
# -C link-arg=-Tdefmt.x` this cortex-m-rt crate links with — producing an ELF with
# no vector table that boots to a garbage SP/PC and QEMU-locks-up
# ("Lockup: can't escalate to HardFault", PC=0) instead of running. Passing the
# link args here restores a bootable image under any ambient RUSTFLAGS. (No
# `-Z emit-stack-sizes` — that's the static gate's nightly-only concern; the
# dynamic measurement is pure runtime stack-painting on stable.)
#
# Capture rc explicitly (don't let `set -e` kill us at the assignment) so a build
# error OR a QEMU abort surfaces its full output in the CI log instead of a bare
# "exit 134" — the output is otherwise swallowed into $OUT and lost.
set +e
OUT="$(RUSTFLAGS='-C link-arg=-Tlink.x -C link-arg=-Tdefmt.x' cargo run --release --features qemu 2>&1)"
RC=$?
set -e
if [ "$RC" -ne 0 ]; then
  echo "ERROR: 'cargo run --release --features qemu' exited $RC. Full build/QEMU output:" >&2
  printf '%s\n' "$OUT" >&2
  exit 1
fi
LINE="$(printf '%s\n' "$OUT" | grep -E 'mcu-stackprobe-qemu:' || true)"
if [ -z "$LINE" ]; then
  echo "ERROR: no measurement line in QEMU output:" >&2
  printf '%s\n' "$OUT" >&2
  exit 1
fi
echo "$LINE"

# Parse the open-path TRANSIENT and compare to the board's AXI-SRAM stack headroom
# (~592 KiB; see dap/board-h7a3/src/main.rs's APP_CELL note and the arch-tests
# `board_static_footprint_leaves_stack_room_in_linked_axi_sram` guard).
#
# IMPORTANT (metric corrected 2026-11-19): the number that matters is the
# "open-path transient" = how far the decode path grows the stack BELOW the paint
# point (`painted_hi - deepest`), NOT the old "peak" = depth from the top of RAM
# (`_stack_start - deepest`). The old peak also counted this harness's own bloated
# `main` frame (it holds two episodes' decoders + both fixtures + every
# measurement local at once — ~780 KiB), which a real board task never carries.
# Comparing that against the board headroom was apples-to-oranges and
# over-reported the transient by ~3x (it is what produced the spurious "~1 MB,
# exceeds headroom" blocker). The transient is the portable, board-relevant number.
TRANSIENT_KIB="$(printf '%s\n' "$LINE" | sed -nE 's/.*open-path transient = ([0-9]+) KiB.*/\1/p' | head -1)"
if [ -z "$TRANSIENT_KIB" ]; then
  echo "ERROR: could not parse the open-path transient from the measurement line." >&2
  exit 1
fi

# Paint-validation: `after_paint` must be 0 (nothing below the paint point was
# touched between paint and the first measurement). A non-zero value means the
# stack-painting rig itself is unsound, so every number below is untrustworthy —
# fail rather than gate on a bogus measurement.
AFTER_PAINT_B="$(printf '%s\n' "$LINE" | sed -nE 's/.*after_paint=([0-9]+) B.*/\1/p' | head -1)"
if [ -n "$AFTER_PAINT_B" ] && [ "$AFTER_PAINT_B" -ne 0 ]; then
  echo "ERROR: paint validation failed (after_paint=${AFTER_PAINT_B} B, expected 0) — the" >&2
  echo "       stack-painting measurement is unsound; refusing to gate on it." >&2
  exit 1
fi

# The gated number is the WORST transient across every format the probe exercises
# — the two episodes' peak (`open-path transient`) AND each per-format sweep value
# (wav / mka-flac / wavpack / opus-webm / alac-caf) — so a regression in ANY single
# format trips the gate, not just whichever one happens to dominate today.
PERFMT_LINE="$(printf '%s\n' "$OUT" | grep -E 'per-format open transient' || true)"
WORST_KIB="$TRANSIENT_KIB"
WORST_WHICH="open-path(two-episode peak)"
if [ -n "$PERFMT_LINE" ]; then
  # Extract every `name=NN` pair; keep the largest.
  while read -r name val; do
    [ -n "$val" ] || continue
    if [ "$val" -gt "$WORST_KIB" ]; then WORST_KIB="$val"; WORST_WHICH="$name"; fi
  done < <(printf '%s\n' "$PERFMT_LINE" | grep -oE '[a-z0-9-]+=[0-9]+' | tr '=' ' ')
fi

# Thresholds: a regression ratchet (tight, catches drift early) and the board's
# real AXI-SRAM stack headroom (the hard safety floor). Both must hold.
BUDGET_KIB="${DYNAMIC_STACK_BUDGET_KIB:-384}"
BOARD_HEADROOM_KIB=592
echo "---"
echo "worst dynamic open transient (growth below the call site): ${WORST_KIB} KiB  (${WORST_WHICH})"
echo "regression-ratchet budget (DYNAMIC_STACK_BUDGET_KIB):      ${BUDGET_KIB} KiB"
echo "board AXI-SRAM stack headroom (safety floor):              ${BOARD_HEADROOM_KIB} KiB"

FAIL=0
if [ "$WORST_KIB" -gt "$BUDGET_KIB" ]; then
  echo >&2
  echo "FAIL (regression): the worst dynamic open transient (${WORST_KIB} KiB, ${WORST_WHICH}) exceeds the" >&2
  echo "       ratchet budget (${BUDGET_KIB} KiB) by $((WORST_KIB - BUDGET_KIB)) KiB. A decoder/format change grew the" >&2
  echo "       on-target stack. Reduce it (externalize scratch in soul-audio-core), or — if this" >&2
  echo "       growth is understood and accepted — raise DYNAMIC_STACK_BUDGET_KIB deliberately." >&2
  FAIL=1
fi
if [ "$WORST_KIB" -gt "$BOARD_HEADROOM_KIB" ]; then
  echo >&2
  echo "FAIL (safety): the worst dynamic open transient (${WORST_KIB} KiB) exceeds the board's AXI-SRAM" >&2
  echo "       stack headroom (${BOARD_HEADROOM_KIB} KiB) — a task cannot open this format within its stack." >&2
  FAIL=1
fi
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
echo
echo "OK: worst dynamic open transient ${WORST_KIB} KiB is within the ${BUDGET_KIB} KiB ratchet budget"
echo "    ($((BUDGET_KIB - WORST_KIB)) KiB to spare) and the ${BOARD_HEADROOM_KIB} KiB board headroom."
