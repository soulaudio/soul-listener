#!/usr/bin/env bash
# tools/mcu-stackprobe/check-rtf-qemu.sh
#
# DECODE WORK gate + cache-ratio rig. Runs the real
# `open_seekable -> Deck::load -> Deck::pump` path on thumbv7em (Cortex-M7)
# codegen under QEMU and reports the instructions executed per second of decoded
# audio, three ways: caches off, I-cache, I+D.
#
# WHAT THIS GATES, AND WHAT IT CANNOT
#
# Gated: INSTRUCTIONS per second of audio. Under `-icount shift=0` QEMU's virtual
# clock advances one nanosecond per guest instruction, so the guest's SysTick
# becomes an exact instruction counter — fully deterministic, identical run to
# run, and therefore a sound regression ratchet. A decoder change that makes the
# DAP do materially more work per second of audio fails here, with no hardware.
#
# NOT gated, and NOT measurable here: the CACHE RATIO. QEMU's TCG is a functional
# emulator — no caches, no flash/AXI wait states, no store buffer, no dual-issue.
# All three passes execute the same instructions, so the emulated speedup is
# exactly 1.00x. That is the ABSENCE of a measurement, not a result, and this
# script prints it as such. ST's figure (a disabled D-cache roughly halves M7
# performance) stays a citation until the same bin runs on board-h7a3 silicon,
# where DWT CYCCNT exists and the probe selects it automatically.
#
# WHY `-icount` IS MANDATORY
#
# Without it, SysTick tracks host-paced virtual time and the output is noise that
# looks like data. Measured on this machine, the same three passes reported
# 203.4M / 239.9M / 243.9M "ticks" — an apparent 0.84x "speedup", i.e. caches
# appearing to make decoding 16% SLOWER. Nothing about that is real; it is the
# host's scheduling jitter. This script therefore always passes `-icount shift=0`
# and refuses to interpret a run without it.
#
# DERIVING A TARGET RTF
#
#   target_RTF ~= f_cpu / (instructions_per_audio_second * CPI)
#
# CPI is exactly what the caches change, so it is the part that needs silicon.
# For context, this replaces the power model's current input, which is a HOST x86
# benchmark divided by `power::model::HOST_TO_TARGET_DERATE` (50x) — a projection
# its own doc disclaims. The instruction count here is target-ISA ground truth;
# only CPI remains estimated, which is a much smaller unknown than a 50x guess.
#
# Install QEMU separately (Linux CI: apt-get install qemu-system-arm). Like the
# other probes, RUSTFLAGS is set explicitly with the cortex-m-rt link args, since
# CI's ambient `RUSTFLAGS=-D warnings` would otherwise REPLACE the config-file
# rustflags and drop `-Tlink.x`, producing a no-vector-table image that QEMU locks
# up on (see check-stack-qemu.sh's note).
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
echo "== mcu-rtfprobe decode-work measurement (QEMU mps2-an500, Cortex-M7, -icount) =="
echo "qemu: $(command -v qemu-system-arm)"

TARGET=thumbv7em-none-eabihf
ELF="target/$TARGET/release/mcu-rtfprobe"

set +e
BUILD="$(RUSTFLAGS='-C link-arg=-Tlink.x -C link-arg=-Tdefmt.x' \
  cargo build --release --features rtf --bin mcu-rtfprobe 2>&1)"
RC=$?
set -e
if [ "$RC" -ne 0 ]; then
  echo "ERROR: building mcu-rtfprobe exited $RC. Full output:" >&2
  printf '%s\n' "$BUILD" >&2
  exit 1
fi

# `-icount shift=0`: 1 guest instruction = 1 ns of virtual time, so SysTick
# counts instructions exactly. Without this the numbers are host noise.
set +e
OUT="$(qemu-system-arm -cpu cortex-m7 -machine mps2-an500 -nographic -icount shift=0 \
  -semihosting-config enable=on,target=native -kernel "$ELF" 2>&1)"
RC=$?
set -e
if [ "$RC" -ne 0 ]; then
  echo "ERROR: QEMU exited $RC. Full output:" >&2
  printf '%s\n' "$OUT" >&2
  exit 1
fi

VALIDATE="$(printf '%s\n' "$OUT" | grep -E 'mcu-rtfprobe: VALIDATE' || true)"
if [ -z "$VALIDATE" ]; then
  echo "ERROR: no VALIDATE line in QEMU output:" >&2
  printf '%s\n' "$OUT" >&2
  exit 1
fi
printf '%s\n' "$OUT" | grep -E 'mcu-rtfprobe:' || true
echo "---"

# Soundness gates on the measurement itself, before gating the number.
COMPARABLE="$(printf '%s\n' "$VALIDATE" | sed -nE 's/.*comparable=([a-z]+).*/\1/p')"
if [ "$COMPARABLE" != "true" ]; then
  echo "ERROR: the three passes did not decode the same audio — they are not" >&2
  echo "       comparable, so no ratio or ratchet below is meaningful." >&2
  exit 1
fi
if printf '%s\n' "$VALIDATE" | grep -q 'hit_cap=true'; then
  echo "ERROR: a pass hit MAX_PUMPS and was truncated; the figures are partial." >&2
  exit 1
fi
if printf '%s\n' "$VALIDATE" | grep -q 'suspect_overflow=true'; then
  echo "ERROR: the probe flagged a possibly-missed SysTick wrap, so the count may" >&2
  echo "       be an undercount. Sample more often (see Counter::sample)." >&2
  exit 1
fi

INSN_PER_S="$(printf '%s\n' "$OUT" \
  | sed -nE 's/.*caches=off ticks=[0-9]+ ticks_per_audio_s=([0-9]+).*/\1/p' | head -1)"
if [ -z "$INSN_PER_S" ]; then
  echo "ERROR: could not parse instructions-per-audio-second from the output." >&2
  exit 1
fi

# Regression ratchet. 22,000,000 over a measured 19,998,974 (~10% margin). This
# is instructions, not cycles, and is deterministic under -icount — so unlike the
# stack budgets it needs no allowance for compiler drift beyond real codegen
# change. The probe's own toolchain is pinned by rust-toolchain.toml next door.
BUDGET_INSN="${DECODE_INSN_BUDGET_PER_AUDIO_S:-22000000}"
echo "instructions per second of decoded audio (caches=off, the shipped config): ${INSN_PER_S}"
echo "regression-ratchet budget (DECODE_INSN_BUDGET_PER_AUDIO_S):                ${BUDGET_INSN}"

SPEEDUP_I="$(printf '%s\n' "$OUT" | sed -nE 's/.*speedup_centi i_vs_off=([0-9]+).*/\1/p' | head -1)"
SPEEDUP_ID="$(printf '%s\n' "$OUT" | sed -nE 's/.*speedup_centi .*id_vs_off=([0-9]+).*/\1/p' | head -1)"
echo
echo "cache speedup, emulated: i=${SPEEDUP_I:-?}/100  i+d=${SPEEDUP_ID:-?}/100"
if [ "${SPEEDUP_ID:-100}" = "100" ] && [ "${SPEEDUP_I:-100}" = "100" ]; then
  echo "  ^ EXPECTED, and NOT a result. QEMU models no caches and no wait states, so"
  echo "    all three passes execute identical instructions. The real ratio needs"
  echo "    board-h7a3 silicon, where the probe picks DWT CYCCNT automatically."
else
  echo "  ^ UNEXPECTED under QEMU: emulation should show exactly 100/100. Either the"
  echo "    run lacked -icount, or QEMU gained a memory-system model. Investigate"
  echo "    before trusting either number." >&2
fi

echo
echo "target RTF implied at 280 MHz, as a function of CPI (CPI is what the caches"
echo "change, and the part that needs silicon):"
for cpi_centi in 120 150 300 500 800; do
  # RTF = f_cpu / (insn_per_s * CPI); x100 arithmetic throughout.
  rtf_centi=$(( 280000000 * 100 * 100 / (INSN_PER_S * cpi_centi) ))
  # Minimum clock in MHz to sustain realtime (RTF = 1.0) at this CPI.
  min_mhz=$(( INSN_PER_S * cpi_centi / 100 / 1000000 ))
  printf '  CPI %d.%02d -> RTF %d.%02dx realtime, needs >= %d MHz to hold realtime\n' \
    $((cpi_centi / 100)) $((cpi_centi % 100)) \
    $((rtf_centi / 100)) $((rtf_centi % 100)) \
    "$min_mhz"
done

if [ "$INSN_PER_S" -gt "$BUDGET_INSN" ]; then
  echo >&2
  echo "FAIL (regression): decode work (${INSN_PER_S} instructions per audio second)" >&2
  echo "       exceeds the ratchet budget (${BUDGET_INSN}) by $((INSN_PER_S - BUDGET_INSN)). A decoder," >&2
  echo "       resampler or container change made the DAP do materially more work per" >&2
  echo "       second of audio, which raises the minimum clock and therefore current" >&2
  echo "       draw. Reduce it, or — if the cost is understood and accepted — raise" >&2
  echo "       DECODE_INSN_BUDGET_PER_AUDIO_S deliberately." >&2
  exit 1
fi
echo
echo "OK: decode work ${INSN_PER_S} instructions per audio second is within the"
echo "    ${BUDGET_INSN} ratchet budget ($((BUDGET_INSN - INSN_PER_S)) to spare)."
