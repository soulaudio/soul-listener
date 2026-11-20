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
# This is a REPORTING tool, not (yet) a blocking CI gate: wiring it into CI needs
# QEMU in the runner image, and — until the open transient is reduced — a hard
# gate would be red (see the WARNING this prints). See the roadmap's A2/A4.
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
OUT="$(cargo run --release --features qemu 2>&1)"
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
BOARD_HEADROOM_KIB=592
echo "---"
echo "dynamic open-path stack transient (growth below the call site): ${TRANSIENT_KIB:-?} KiB"
echo "board AXI-SRAM stack headroom:                                   ${BOARD_HEADROOM_KIB} KiB"
if [ -z "$TRANSIENT_KIB" ]; then
  echo "ERROR: could not parse the open-path transient from the measurement line." >&2
  exit 1
fi
if [ "$TRANSIENT_KIB" -gt "$BOARD_HEADROOM_KIB" ]; then
  echo
  echo "*** WARNING: the dynamic open transient ($TRANSIENT_KIB KiB) EXCEEDS the board's stack"
  echo "***          headroom ($BOARD_HEADROOM_KIB KiB) by $((TRANSIENT_KIB - BOARD_HEADROOM_KIB)) KiB. A task cannot open a track"
  echo "***          within its stack. Reduce it in soul-audio-core (externalize decoder"
  echo "***          scratch) before the board can open this format."
else
  echo
  echo "OK: the open transient ($TRANSIENT_KIB KiB) fits within the board's stack headroom"
  echo "    ($BOARD_HEADROOM_KIB KiB), with $((BOARD_HEADROOM_KIB - TRANSIENT_KIB)) KiB to spare for the task's own frame."
fi
