#!/usr/bin/env bash
# tools/check-arch-boundaries.sh
#
# Architecture fitness function for dap-core's INTERNAL module layering (an
# "architecture test for structure"). The repo is hexagonal: a dependency-free
# `ports` boundary, feature slices that depend only on `ports`, and a thin
# composition layer on top. That structure is clean today — this gate keeps it
# clean by failing CI the moment a module imports across a forbidden boundary
# (e.g. the `audio` slice reaching into `display`, or a `ports` trait reaching
# up into a feature slice). See docs/ARCHITECTURE.md for the layering it encodes.
#
# It is a fast static grep over real `use crate::…` statements (doc-link
# references like `[crate::app::App]` are ignored — only `use` imports count,
# since those are the actual compile-time dependencies). No build.
#
# Layers (inner → outer), and what each module may import from `crate::`:
#   ports                    → (nothing intra-crate — the innermost contract)
#   audio | power | display  → ports            (leaf feature slices)
#   screens                  → ports, display   (UI views over display primitives)
#   app | firmware | stub    → anything         (composition / run-loop / adapters)
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
REPO_ROOT="$(cd -- "$SCRIPT_DIR/.." &>/dev/null && pwd)"
CORE="$REPO_ROOT/dap/core/src"
cd "$REPO_ROOT"

# "<module>:<forbidden-sibling-regex>" — a module FAILS if it `use crate::X`s any
# X in its forbidden set. (Modules absent from this list — app/firmware/stub —
# are the composition layer and may import anything.)
RULES=(
  "ports:app|audio|power|display|screens|firmware|stub"
  "audio:app|power|display|screens|firmware|stub"
  "power:app|audio|display|screens|firmware|stub"
  "display:app|audio|power|screens|firmware|stub"
  "screens:app|audio|power|firmware|stub"
)

echo "== dap-core architecture-boundary gate =="
echo "core: $CORE"
echo

FAIL=0
for entry in "${RULES[@]}"; do
  module="${entry%%:*}"
  forbidden="${entry#*:}"
  # The module is either a file (audio.rs) and/or a folder (power/, screens/).
  files=()
  [ -f "$CORE/$module.rs" ] && files+=("$CORE/$module.rs")
  [ -d "$CORE/$module" ] && while IFS= read -r f; do files+=("$f"); done < <(find "$CORE/$module" -name '*.rs')
  if [ "${#files[@]}" -eq 0 ]; then
    echo "FAIL: module '$module' has no source files at $CORE/$module[.rs] (renamed? update this gate)" >&2
    FAIL=1
    continue
  fi
  # Real imports only: lines that are `use crate::…` (optionally `pub use`),
  # not doc comments. Catch `use crate::X` and `use crate::{…, X, …}` / `X::…`.
  hits="$(grep -rhnE '^\s*(pub[[:space:]]+)?use[[:space:]]+crate::' "${files[@]}" 2>/dev/null \
            | grep -E "crate::($forbidden)\b|crate::\{[^}]*\b($forbidden)\b" || true)"
  if [ -n "$hits" ]; then
    echo "FAIL: '$module' imports across a forbidden boundary:" >&2
    printf '  %s\n' "$hits" >&2
    FAIL=1
  else
    echo "OK:   $module (no forbidden imports; allowed: $(
      case "$module" in
        ports) echo "none" ;;
        screens) echo "ports, display" ;;
        *) echo "ports" ;;
      esac))"
  fi
done

echo
if [ "$FAIL" -ne 0 ]; then
  echo "arch-boundaries: FAILED — a module crossed a layer boundary (see above)." >&2
  echo "Either route the dependency through a port/contract type, move the shared type" >&2
  echo "into \`ports\`, or (if the layering itself should change) update this gate + the" >&2
  echo "docs/ARCHITECTURE.md layering it encodes — deliberately, in review." >&2
  exit 1
fi
echo "arch-boundaries: OK"
