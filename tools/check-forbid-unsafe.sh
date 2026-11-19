#!/usr/bin/env bash
# tools/check-forbid-unsafe.sh
#
# CI coverage gate for Rule 2 (no `unsafe` -- ANSSI/safety-critical Rust
# guidance; see docs/plans/2026-11-17-architecture-enforcement-fitness-functions.md).
#
# Every embedded-path crate in THIS repo must declare
# `#![forbid(unsafe_code)]` at its crate root (src/lib.rs or src/main.rs).
# A rule that only lives in a doc drifts the first time a new crate is added
# to the embedded path, or an existing `forbid` is weakened to `deny`/`allow`
# during a refactor -- this script makes that drift fail CI instead of
# silently slipping through review.
#
# Scope: soul-audio-core itself is a path dependency into the SIBLING
# soul-audio-core repo (its own CI already gates it -- see that repo's
# `crates/*/src/lib.rs`). This script only enumerates crates that physically
# live in THIS repo's embedded path: dap-core, dap-sim, board-h7a3, the
# standalone eink-ui toolkit, the salvaged eink-ssd1677 e-ink driver, and
# eink-specs (pure panel-spec data). The v1 firmware stack that used to be
# listed here (eink-system/eink-components, and the allowlisted host-only
# eink-emulator that needed Win32 unsafe) was retired, so there are no longer
# any allowlisted unsafe exceptions -- every listed crate must forbid it.
#
# This script does NOT try to compile anything -- it is a fast, static grep
# over known crate-root files. The actual enforcement is the `#![forbid]`
# attribute itself (a hard compile error on any `unsafe` in that crate);
# this script's only job is making sure every crate on the list still HAS it.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." &>/dev/null && pwd)"
cd "$REPO_ROOT"

# "<crate-name>:<path to its crate-root file>"
ENFORCED_CRATES=(
  "dap-core:dap/core/src/lib.rs"
  "dap-sim:dap/sim/src/lib.rs"
  # board-h7a3 became forbid(unsafe) once ui-4 moved the DAP onto the no-alloc
  # eink-ui toolkit and it dropped its embedded_alloc heap (the heap init was its
  # only unsafe block). Now enforced, not allowlisted.
  "board-h7a3:dap/board-h7a3/src/main.rs"
  # eink-ui: the standalone no_std/no-alloc UI toolkit the DAP now draws with.
  "eink-ui:eink-ui/src/lib.rs"
  # eink-ssd1677: the SSD1677 e-ink driver salvaged from the retired v1 firmware
  # (its one keepable piece); no_std, unsafe-free, forbid(unsafe) at its root.
  "eink-ssd1677:dap/eink-ssd1677/src/lib.rs"
  "eink-specs:crates/eink/eink-specs/src/lib.rs"
)

echo "== forbid(unsafe_code) coverage gate =="
echo "repo root: $REPO_ROOT"
echo

FAIL=0

echo "-- enforced (must have #![forbid(unsafe_code)]) --"
for entry in "${ENFORCED_CRATES[@]}"; do
  name="${entry%%:*}"
  path="${entry#*:}"
  if [ ! -f "$path" ]; then
    echo "FAIL: $name -- expected crate-root file not found at $path" >&2
    FAIL=1
    continue
  fi
  if grep -Fq '#![forbid(unsafe_code)]' "$path"; then
    echo "OK:   $name ($path)"
  else
    echo "FAIL: $name ($path) is MISSING #![forbid(unsafe_code)]" >&2
    FAIL=1
  fi
done

echo
if [ "$FAIL" -ne 0 ]; then
  echo "forbid-unsafe-coverage: FAILED -- see above." >&2
  exit 1
fi
echo "forbid-unsafe-coverage: OK"
