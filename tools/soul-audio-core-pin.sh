#!/usr/bin/env bash
# tools/soul-audio-core-pin.sh
#
# Manage the soul-audio-core revision the DAP CI builds against
# (.github/soul-audio-core.pin). See that file's header for the rationale: CI
# pins a rev instead of floating on soul-audio-core@main so every DAP build is
# reproducible and picking up upstream changes is a DELIBERATE, reviewed bump.
#
# Usage:
#   tools/soul-audio-core-pin.sh            Show the current pin vs soul-audio-core main.
#   tools/soul-audio-core-pin.sh --latest   Pin to soul-audio-core main's current tip.
#   tools/soul-audio-core-pin.sh --set <sha> Pin to a specific soul-audio-core commit.
#
# After bumping, rebuild/test the dap workspace against the new rev (point your
# local ../../soul-audio-core at it, or let the DAP PR's CI do it) before merging.
set -euo pipefail

REPO="https://github.com/soulaudio/soul-audio-core"
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" &>/dev/null && pwd)"
PIN_FILE="$SCRIPT_DIR/../.github/soul-audio-core.pin"

current_pin() { grep -vE '^[[:space:]]*#|^[[:space:]]*$' "$PIN_FILE" | head -1 | tr -d '[:space:]'; }
remote_main() { git ls-remote "$REPO" refs/heads/main | cut -f1; }

write_pin() {
  local sha="$1"
  if ! [[ "$sha" =~ ^[0-9a-f]{40}$ ]]; then
    echo "ERROR: '$sha' is not a 40-char commit SHA" >&2
    exit 1
  fi
  # Preserve the header (comment + blank lines), replace the single SHA line.
  { grep -E '^[[:space:]]*#|^[[:space:]]*$' "$PIN_FILE"; echo "$sha"; } > "$PIN_FILE.tmp"
  mv "$PIN_FILE.tmp" "$PIN_FILE"
  echo "pinned soul-audio-core -> $sha"
}

case "${1:-}" in
  --set)
    [ -n "${2:-}" ] || { echo "usage: $0 --set <sha>" >&2; exit 1; }
    write_pin "$2"
    ;;
  --latest)
    main="$(remote_main)"
    [ -n "$main" ] || { echo "ERROR: could not read $REPO main" >&2; exit 1; }
    write_pin "$main"
    ;;
  "")
    pin="$(current_pin)"
    main="$(remote_main || true)"
    echo "soul-audio-core pin (.github/soul-audio-core.pin): $pin"
    echo "soul-audio-core main (remote tip):                 ${main:-<unreachable>}"
    if [ -n "$main" ] && [ "$pin" = "$main" ]; then
      echo "=> up to date with main."
    elif [ -n "$main" ]; then
      echo "=> pin differs from main. Review the diff:"
      echo "     $REPO/compare/$pin...$main"
      echo "   Then bump with: $0 --latest  (and re-verify the dap build)."
    fi
    ;;
  *)
    echo "usage: $0 [--latest | --set <sha>]" >&2
    exit 1
    ;;
esac
