#!/usr/bin/env bash
# tools/mcu-stackprobe/check-tick-qemu.sh
#
# DYNAMIC whole-firmware-tick stack gate under QEMU — the board-accurate
# companion to check-stack-qemu.sh. That script measures soul-audio-core's decode
# path in isolation (open_seekable → Deck); this one runs dap-core's REAL
# `firmware::step` — `App::tick` (drain controls, poll power, pump the deck,
# including the first-tick track-OPEN) THEN `App::render` (the e-ink now-playing
# view: eink-ui flex layout + widget draw) — the exact run-loop the board ships.
#
# Why it matters: the board reaches the decoder construction THROUGH
# App::tick → AudioPlayer::pump → Engine::poll_open, and those wrapper frames are
# real on-target stack the isolated decode probe never counts. CI's measured
# whole-tick worst is ~308 KiB, vs the isolated decode path's ~219 KiB — so this
# is the number that actually answers "does the firmware fit its stack?" (and
# even the higher 393 KiB local reading fits the board's 592 KiB AXI-SRAM
# headroom).
#
# Those figures are post-pin-bump (soul-audio-core #111 + #113) as measured by
# CI. They were 414/297 KiB before it; the two upstream slices took CI's whole
# tick down by 106 KiB — more than the 71 KiB the placement doc predicted from
# the isolated decode measurements, because the wrapper frames the tick adds
# hold decoder-sized locals too, so they shrink with the type as well.
#
# REPRODUCIBILITY — this probe's number depends on the COMPILER, so the
# toolchain is pinned in `rust-toolchain.toml` next to this script and the
# budget below is set against that pinned measurement.
#
# The spread was found the hard way: at byte-identical content (pin c0c35c38) CI
# measured 308 KiB and a local run measured 393 KiB — 27% apart, while the
# isolated decode probe differed by only 1.4% (219 vs 222). It was not noise and
# not the environment. CI ran rustc 1.100.0 and the local default was 1.95.0;
# re-running locally under 1.100.0 reproduces CI EXACTLY, to the byte
# (316,056 B), per-format values included. Inlining and frame layout changed
# between those releases, and the whole-tick probe is far more sensitive to it
# than the isolated decode path because it measures through the `App::tick` ->
# `pump` -> `poll_open` wrapper chain, where the caller frames live or get
# inlined away.
#
# So the gate is only meaningful with the compiler pinned — otherwise a new
# stable release moves the gated number with no source change. See
# `rust-toolchain.toml`.
#
# The `mcu-tickprobe` bin wires a real `App` exactly as board-h7a3/src/main.rs
# does, except Storage is an in-memory Matroska-FLAC fixture (so the first tick
# actually opens+decodes, unlike a StubStorage board boot). Same guest-side
# stack-painting as check-stack-qemu.sh.
#
# Install QEMU separately (Linux CI: apt-get install qemu-system-arm). Like the
# decode gate, RUSTFLAGS is set explicitly with the cortex-m-rt link args, since
# CI's ambient `RUSTFLAGS=-D warnings` would otherwise REPLACE the config-file
# rustflags and drop `-Tlink.x`, producing a no-vector-table image that QEMU-locks
# up (see check-stack-qemu.sh's note).
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
cd "$SCRIPT_DIR"

if ! command -v qemu-system-arm &>/dev/null; then
  if [ -x "/c/Program Files/qemu/qemu-system-arm.exe" ]; then
    export PATH="/c/Program Files/qemu:$PATH"
  else
    echo "ERROR: qemu-system-arm not found on PATH. Install QEMU (apt-get install qemu-system-arm)." >&2
    exit 1
  fi
fi
echo "== mcu-tickprobe whole-firmware-tick stack measurement (QEMU mps2-an500, Cortex-M7) =="
echo "qemu: $(command -v qemu-system-arm)"

# Build + run the whole-tick harness. RUSTFLAGS carries the link args explicitly
# (see the header); no `-Z emit-stack-sizes` (runtime stack-painting, stable).
# rc captured so a build error or QEMU abort surfaces its full output.
set +e
OUT="$(RUSTFLAGS='-C link-arg=-Tlink.x -C link-arg=-Tdefmt.x' \
  cargo run --release --features tick --bin mcu-tickprobe 2>&1)"
RC=$?
set -e
if [ "$RC" -ne 0 ]; then
  echo "ERROR: 'cargo run --features tick --bin mcu-tickprobe' exited $RC. Full build/QEMU output:" >&2
  printf '%s\n' "$OUT" >&2
  exit 1
fi
LINE="$(printf '%s\n' "$OUT" | grep -E 'mcu-tickprobe-qemu:' || true)"
if [ -z "$LINE" ]; then
  echo "ERROR: no measurement line in QEMU output:" >&2
  printf '%s\n' "$OUT" >&2
  exit 1
fi
echo "$LINE"

# Paint-validation: after_paint must be 0, else the measurement is unsound.
AFTER_PAINT_B="$(printf '%s\n' "$LINE" | sed -nE 's/.*after_paint=([0-9]+) B.*/\1/p' | head -1)"
if [ -n "$AFTER_PAINT_B" ] && [ "$AFTER_PAINT_B" -ne 0 ]; then
  echo "ERROR: paint validation failed (after_paint=${AFTER_PAINT_B} B, expected 0) — refusing to gate." >&2
  exit 1
fi

TICK_KIB="$(printf '%s\n' "$LINE" | sed -nE 's/.*whole-tick transient = ([0-9]+) KiB.*/\1/p' | head -1)"
if [ -z "$TICK_KIB" ]; then
  echo "ERROR: could not parse the whole-tick transient from the measurement line." >&2
  exit 1
fi

# Thresholds: a regression ratchet (tight) and the board's AXI-SRAM stack
# headroom (hard safety floor). Both must hold.
BUDGET_KIB="${DYNAMIC_TICK_BUDGET_KIB:-352}"
BOARD_HEADROOM_KIB=592
echo "---"
echo "whole-firmware-tick stack transient (growth below the call site): ${TICK_KIB} KiB"
echo "regression-ratchet budget (DYNAMIC_TICK_BUDGET_KIB):              ${BUDGET_KIB} KiB"
echo "board AXI-SRAM stack headroom (safety floor):                     ${BOARD_HEADROOM_KIB} KiB"

FAIL=0
if [ "$TICK_KIB" -gt "$BUDGET_KIB" ]; then
  echo >&2
  echo "FAIL (regression): whole-tick transient (${TICK_KIB} KiB) exceeds the ratchet budget" >&2
  echo "       (${BUDGET_KIB} KiB) by $((TICK_KIB - BUDGET_KIB)) KiB. A change to the decode path, the App" >&2
  echo "       wiring, or the render path grew the on-target stack. Reduce it, or — if the" >&2
  echo "       growth is understood and accepted — raise DYNAMIC_TICK_BUDGET_KIB deliberately." >&2
  FAIL=1
fi
if [ "$TICK_KIB" -gt "$BOARD_HEADROOM_KIB" ]; then
  echo >&2
  echo "FAIL (safety): whole-tick transient (${TICK_KIB} KiB) exceeds the board's AXI-SRAM stack" >&2
  echo "       headroom (${BOARD_HEADROOM_KIB} KiB) — the firmware cannot run one tick within its stack." >&2
  FAIL=1
fi
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
echo
echo "OK: whole-tick transient ${TICK_KIB} KiB is within the ${BUDGET_KIB} KiB ratchet budget"
echo "    ($((BUDGET_KIB - TICK_KIB)) KiB to spare) and the ${BOARD_HEADROOM_KIB} KiB board headroom."
