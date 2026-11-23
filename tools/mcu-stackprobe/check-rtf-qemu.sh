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

# --- parse the two stages -------------------------------------------------
# DECODE ONLY: each format at its OWN native rate, so the resampler is bypassed.
DECODE_WORST="$(printf '%s\n' "$OUT" \
  | sed -nE 's/.*decode_only worst_format=([a-z0-9-]+) worst_ticks_per_audio_s=([0-9]+).*/\2/p' | head -1)"
DECODE_WORST_FMT="$(printf '%s\n' "$OUT" \
  | sed -nE 's/.*decode_only worst_format=([a-z0-9-]+) worst_ticks_per_audio_s=([0-9]+).*/\1/p' | head -1)"
# FULL PIPELINE: 44.1 kHz source into a 48 kHz sink, so the resampler runs.
PIPELINE="$(printf '%s\n' "$OUT" \
  | sed -nE 's/.*resampler .*flac_resampled_per_s=([0-9]+) .*/\1/p' | head -1)"
RESAMPLER="$(printf '%s\n' "$OUT" \
  | sed -nE 's/.*resampler .*resampler_per_s=([0-9]+) .*/\1/p' | head -1)"
RESAMPLE_X100="$(printf '%s\n' "$OUT" \
  | sed -nE 's/.*resampled_vs_native_centi=([0-9]+).*/\1/p' | head -1)"
FLAC_NATIVE="$(printf '%s\n' "$OUT" \
  | sed -nE 's/.*resampler flac_native_per_s=([0-9]+) .*/\1/p' | head -1)"
SPEEDUP_I="$(printf '%s\n' "$OUT" | sed -nE 's/.*speedup_centi i_vs_off=([0-9]+).*/\1/p' | head -1)"
SPEEDUP_ID="$(printf '%s\n' "$OUT" | sed -nE 's/.*speedup_centi .*id_vs_off=([0-9]+).*/\1/p' | head -1)"

for v in DECODE_WORST PIPELINE RESAMPLER RESAMPLE_X100 FLAC_NATIVE; do
  eval "val=\${$v:-}"
  if [ -z "$val" ]; then
    echo "ERROR: could not parse $v from the probe output." >&2
    exit 1
  fi
done

# --- the two ratchets -----------------------------------------------------
# Separate budgets, because these are different engineering quantities and a
# regression in either has a different fix. Instructions, not cycles, and exact
# under -icount, so neither needs a compiler-drift allowance beyond real codegen
# change (the probe's toolchain is pinned by rust-toolchain.toml next door).
BUDGET_DECODE="${DECODE_INSN_BUDGET_PER_AUDIO_S:-11000000}"
BUDGET_PIPELINE="${PIPELINE_INSN_BUDGET_PER_AUDIO_S:-22000000}"

echo "decode only, worst format (${DECODE_WORST_FMT}), at its NATIVE rate:  ${DECODE_WORST}"
echo "  ratchet (DECODE_INSN_BUDGET_PER_AUDIO_S):                          ${BUDGET_DECODE}"
echo "full pipeline (44.1 kHz source -> 48 kHz sink, resampler engaged):   ${PIPELINE}"
echo "  ratchet (PIPELINE_INSN_BUDGET_PER_AUDIO_S):                        ${BUDGET_PIPELINE}"
echo
echo "RESAMPLER COST, attributed: ${RESAMPLER} instructions per second of audio"
echo "  same fixture, same codec, same cache state — the only difference between"
echo "  the pipeline figure and FLAC at its native rate (${FLAC_NATIVE}) is whether"
echo "  the resampler runs. That makes it ${RESAMPLE_X100}/100 x the native-rate cost, and the"
echo "  single heaviest stage in the DAP's audio path — more than the heaviest"
echo "  DECODER (${DECODE_WORST_FMT}, ${DECODE_WORST}). The board carries a dual MCLK oscillator"
echo "  specifically so the SAI can clock natively and skip it; power::model's"
echo "  native_rate_clocking_needs_lower_clock_than_resampling asserts the"
echo "  direction, and this is the magnitude."

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
echo "minimum core clock to hold realtime, as a function of CPI (CPI is what the"
echo "caches change, and the part that still needs silicon):"
printf '  %-10s %-22s %s\n' "CPI" "native-rate (${DECODE_WORST_FMT})" "resampled"
for cpi_centi in 120 150 300 610 800; do
  native_mhz=$(( DECODE_WORST * cpi_centi / 100 / 1000000 ))
  resamp_mhz=$(( PIPELINE * cpi_centi / 100 / 1000000 ))
  printf '  %d.%02d%-5s %-22s %s\n' \
    $((cpi_centi / 100)) $((cpi_centi % 100)) "" \
    ">= ${native_mhz} MHz" ">= ${resamp_mhz} MHz"
done
echo "  (CPI 6.10 is the value the power model's 50x host derate implicitly assumes"
echo "   for the resampled pipeline — see the placement doc S15/S16.)"

FAIL=0
if [ "$DECODE_WORST" -gt "$BUDGET_DECODE" ]; then
  echo >&2
  echo "FAIL (regression): worst decode-only cost (${DECODE_WORST} instructions per audio" >&2
  echo "       second, ${DECODE_WORST_FMT}) exceeds the ratchet (${BUDGET_DECODE}) by $((DECODE_WORST - BUDGET_DECODE))." >&2
  echo "       A decoder got more expensive. This raises the minimum clock even for" >&2
  echo "       native-rate playback, which is the low-power path — reduce it, or raise" >&2
  echo "       DECODE_INSN_BUDGET_PER_AUDIO_S deliberately." >&2
  FAIL=1
fi
if [ "$PIPELINE" -gt "$BUDGET_PIPELINE" ]; then
  echo >&2
  echo "FAIL (regression): full-pipeline cost (${PIPELINE} instructions per audio second)" >&2
  echo "       exceeds the ratchet (${BUDGET_PIPELINE}) by $((PIPELINE - BUDGET_PIPELINE)). A decoder," >&2
  echo "       resampler or container change made the DAP do materially more work per" >&2
  echo "       second of audio. Reduce it, or raise PIPELINE_INSN_BUDGET_PER_AUDIO_S" >&2
  echo "       deliberately." >&2
  FAIL=1
fi
if [ "$FAIL" -ne 0 ]; then
  exit 1
fi
echo
echo "OK: decode-only ${DECODE_WORST} <= ${BUDGET_DECODE} ($((BUDGET_DECODE - DECODE_WORST)) to spare)"
echo "    pipeline    ${PIPELINE} <= ${BUDGET_PIPELINE} ($((BUDGET_PIPELINE - PIPELINE)) to spare)"
