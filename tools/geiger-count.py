#!/usr/bin/env python3
"""Parse cargo-geiger JSON output and report workspace unsafe counts.

Usage:
  cargo geiger --output Json 2>/dev/null | python3 tools/geiger-count.py
  cargo geiger --output Json 2>/dev/null | python3 tools/geiger-count.py --check .geiger-baseline
"""

import json
import sys
import argparse


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", metavar="BASELINE_FILE",
                        help="Fail if count exceeds this file's value")
    args = parser.parse_args()

    data = json.load(sys.stdin)
    total = 0
    rows = []

    for pkg in data.get("packages", []):
        if not pkg.get("is_local", False):
            continue
        name = pkg.get("package", {}).get("name", "?")
        used = pkg.get("unsafety", {}).get("used", {})
        count = sum(used.values())
        if count > 0:
            rows.append((name, count))
        total += count

    print(f"Workspace unsafe count: {total}")
    for name, count in sorted(rows):
        print(f"  {name}: {count}")

    if args.check:
        try:
            with open(args.check) as f:
                baseline = int(f.read().strip())
            if total > baseline:
                print(f"FAIL: count {total} exceeds baseline {baseline}")
                print(f"  If this is intentional: update .geiger-baseline with {total}")
                sys.exit(1)
            print(f"OK: {total} <= baseline {baseline}")
        except FileNotFoundError:
            print(f"WARNING: baseline file not found; writing {total} to {args.check}")
            with open(args.check, "w") as f:
                f.write(str(total))


if __name__ == "__main__":
    main()
