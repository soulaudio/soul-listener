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
# live in THIS repo's embedded path: dap-core, dap-sim, and the no_std eink
# toolkit (eink-system/eink-components/eink-specs).
#
# Two crates are DOCUMENTED EXCEPTIONS -- allowlisted rather than forbidden
# -- because they genuinely need `unsafe` and either never ship on the
# embedded target, or need exactly one narrowly-scoped unsafe block:
#   - eink-emulator: host/winit desktop emulator, never built for the
#     embedded target; window.rs needs real `unsafe` for Win32 WndProc
#     subclassing (the WM_DPICHANGED fix). Documented in its own lib.rs.
#   - board-h7a3: the H7A3 firmware binary; main.rs has one real `unsafe`
#     block for `embedded_alloc::LlffHeap::init`'s documented
#     one-time-init precondition (see that call's own SAFETY comment).
#     Documented at the top of main.rs.
#
# This script does NOT try to compile anything -- it is a fast, static grep
# over known crate-root files. The actual enforcement is the `#![forbid]`
# attribute itself (a hard compile error on any `unsafe` in that crate);
# this script's only job is making sure every crate on the list still HAS
# the attribute, and that the two exceptions still carry their written
# justification.
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." &>/dev/null && pwd)"
cd "$REPO_ROOT"

# "<crate-name>:<path to its crate-root file>"
ENFORCED_CRATES=(
  "dap-core:dap/core/src/lib.rs"
  "dap-sim:dap/sim/src/lib.rs"
  "eink-system:crates/eink/eink-system/src/lib.rs"
  "eink-components:crates/eink/eink-components/src/lib.rs"
  "eink-specs:crates/eink/eink-specs/src/lib.rs"
)

# Allowlisted: genuinely need `unsafe`; the exemption must stay documented
# at the crate root (grepped for below), not just asserted in this script.
ALLOWLISTED_CRATES=(
  "eink-emulator:crates/eink/eink-emulator/src/lib.rs"
  "board-h7a3:dap/board-h7a3/src/main.rs"
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
echo "-- allowlisted (documented unsafe exception; forbid NOT required) --"
for entry in "${ALLOWLISTED_CRATES[@]}"; do
  name="${entry%%:*}"
  path="${entry#*:}"
  if [ ! -f "$path" ]; then
    echo "FAIL: $name -- expected crate-root file not found at $path" >&2
    FAIL=1
    continue
  fi
  if grep -Fq 'exempt' "$path"; then
    echo "OK:   $name ($path) -- exemption documented"
  else
    echo "FAIL: $name ($path) is allowlisted but has no documented exemption (expected the word 'exempt' in its crate-root doc comment)" >&2
    FAIL=1
  fi
done

echo
if [ "$FAIL" -ne 0 ]; then
  echo "forbid-unsafe-coverage: FAILED -- see above." >&2
  exit 1
fi
echo "forbid-unsafe-coverage: OK"
