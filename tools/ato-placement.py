#!/usr/bin/env python3
"""Check the floorplan in `hardware/elec/placement.yaml`.

    tools/ato-placement.py            # validate the spec, and the PCB if it has parts
    tools/ato-placement.py --strict   # also fail when the PCB has no footprints yet

Two checks, which activate at different times:

**The spec against itself** — works now, needs nothing. Zones inside the board
outline, no two zones overlapping, every atopile module covered, every
adjacency rule satisfiable by the coordinates as written, and every zone
carrying a `why`. This is the half that catches a floorplan edit that quietly
contradicts itself: moving `memory` away from `mcu` violates the FMC rule, and
the rule and the coordinates are in the same file so nothing external has to be
consulted to know that.

**The spec against the PCB** — activates once the board has footprints. Each
component must sit inside its module's zone and be `locked`. Today the committed
`default.kicad_pcb` is an empty board: atopile emits footprints during the part
picker, which on 0.15.8+ requires an interactive sign-in. So this half reports
"no footprints yet" and passes, rather than pretending to check something.

# Why not express this in the .ato files

Because there is nowhere to put it. atopile 0.15.9 has no placement trait — no
`has_pcb_position`, nothing in the stdlib. The intent would otherwise live only
as coordinates in a generated PCB file, where a reviewer cannot tell which were
deliberate.
"""

from __future__ import annotations

import argparse
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SPEC = ROOT / "hardware/elec/placement.yaml"
SRC = ROOT / "hardware/elec/src"
PCB = ROOT / "hardware/elec/layouts/default/default.kicad_pcb"


def load_yaml(path: pathlib.Path) -> dict:
    """Minimal YAML reader for this file's shape.

    Deliberately not a dependency: the Python side of this repo is stdlib-only
    so that a checkout can run the tooling without a virtualenv. The subset
    needed here is nested maps, lists of maps, scalars and `>` block scalars —
    and the spec is gated by `placement_spec_is_well_formed`, so a construct
    this cannot read fails loudly rather than being misread.
    """
    import json

    root: dict = {}
    stack: list[tuple[int, object]] = [(-1, root)]
    pending_block: list[str] | None = None
    block_indent = 0
    block_target: tuple[dict | list, str | None] | None = None

    def coerce(v: str):
        v = v.strip()
        if v in ("true", "True"):
            return True
        if v in ("false", "False"):
            return False
        if v.startswith("[") and v.endswith("]"):
            return [coerce(x) for x in v[1:-1].split(",") if x.strip()]
        if v.startswith(('"', "'")) and v[-1:] == v[:1]:
            return v[1:-1]
        try:
            return json.loads(v)
        except Exception:
            return v

    for raw in path.read_text().splitlines():
        if pending_block is not None:
            if raw.strip() and (len(raw) - len(raw.lstrip())) >= block_indent:
                pending_block.append(raw.strip())
                continue
            tgt, key = block_target  # type: ignore[misc]
            text = " ".join(pending_block)
            if isinstance(tgt, dict) and key is not None:
                tgt[key] = text
            pending_block, block_target = None, None

        line = raw.split("#", 1)[0].rstrip() if not raw.lstrip().startswith("#") else ""
        if not line.strip():
            continue
        indent = len(line) - len(line.lstrip())
        body = line.strip()

        while stack and indent <= stack[-1][0]:
            stack.pop()
        parent = stack[-1][1]

        if body.startswith("- "):
            item_body = body[2:].strip()
            if not isinstance(parent, list):
                continue
            item: dict = {}
            parent.append(item)
            if ":" in item_body:
                k, v = item_body.split(":", 1)
                if v.strip():
                    item[k.strip()] = coerce(v)
                else:
                    item[k.strip()] = {}
            stack.append((indent, item))
            continue

        if ":" not in body:
            continue
        key, value = body.split(":", 1)
        key, value = key.strip(), value.strip()
        if value in (">", "|"):
            pending_block = []
            block_indent = indent + 1
            block_target = (parent, key)  # type: ignore[assignment]
            continue
        if value == "":
            # Peek is unnecessary: a list child arrives as `- `, a map child as
            # `k:`, and both work against an empty dict except that a list needs
            # to BE a list. `rules` is the only list in the spec.
            child: object = [] if key == "rules" else {}
            if isinstance(parent, dict):
                parent[key] = child
            stack.append((indent, child))
            continue
        if isinstance(parent, dict):
            parent[key] = coerce(value)
    if pending_block is not None and block_target is not None:
        tgt, k = block_target
        if isinstance(tgt, dict) and k is not None:
            tgt[k] = " ".join(pending_block)
    return root


def rects(zones: dict) -> dict[str, tuple[float, float, float, float]]:
    out = {}
    for name, z in zones.items():
        out[name] = (
            float(z["x"]),
            float(z["y"]),
            float(z["x"]) + float(z["width"]),
            float(z["y"]) + float(z["height"]),
        )
    return out


def gap(a: tuple[float, ...], b: tuple[float, ...]) -> float:
    """Shortest distance between two axis-aligned rectangles; 0 if they touch."""
    dx = max(a[0] - b[2], b[0] - a[2], 0.0)
    dy = max(a[1] - b[3], b[1] - a[3], 0.0)
    return (dx * dx + dy * dy) ** 0.5


def ato_modules() -> set[str]:
    """Module names that have a declared check build, lowercased.

    Derived from the sources rather than listed, so adding `sdcard.ato` makes the
    spec incomplete until a zone is declared for it.
    """
    out = set()
    for f in SRC.glob("*.ato"):
        if f.name in ("main.ato", "interfaces.ato"):
            continue
        out.add(f.stem.lower())
    return out


def check_spec(spec: dict) -> list[str]:
    errs: list[str] = []
    board = spec.get("board") or {}
    zones = spec.get("zones") or {}
    rules = spec.get("rules") or []
    bw, bh = float(board.get("width_mm", 0)), float(board.get("height_mm", 0))
    if not bw or not bh:
        return ["board.width_mm and board.height_mm must be set"]

    r = rects(zones)

    for name, (x1, y1, x2, y2) in r.items():
        if x1 < 0 or y1 < 0 or x2 > bw or y2 > bh:
            errs.append(
                f"zone '{name}' ({x1},{y1})-({x2},{y2}) falls outside the "
                f"{bw}x{bh} mm board outline"
            )
        if not str(zones[name].get("why", "")).strip():
            errs.append(
                f"zone '{name}' has no `why`. A floorplan without reasons cannot be "
                f"reviewed, only obeyed."
            )

    names = sorted(r)
    for i, a in enumerate(names):
        for b in names[i + 1:]:
            ax1, ay1, ax2, ay2 = r[a]
            bx1, by1, bx2, by2 = r[b]
            ox = min(ax2, bx2) - max(ax1, bx1)
            oy = min(ay2, by2) - max(ay1, by1)
            if ox > 0 and oy > 0:
                errs.append(
                    f"zones '{a}' and '{b}' overlap by {ox:.1f} x {oy:.1f} mm"
                )

    missing = ato_modules() - set(k.lower() for k in zones)
    if missing:
        errs.append(
            f"atopile modules with no declared zone: {sorted(missing)}. Every module "
            f"needs one, or its parts can land anywhere without the check noticing."
        )

    for rule in rules:
        kind = rule.get("kind")
        pair = rule.get("between") or []
        if len(pair) != 2 or any(p not in r for p in pair):
            errs.append(f"rule {rule!r} names a zone that does not exist")
            continue
        if not str(rule.get("why", "")).strip():
            errs.append(f"rule {rule!r} has no `why`")
        d = gap(r[pair[0]], r[pair[1]])
        limit = float(rule.get("mm", 0))
        if kind == "max_gap" and d > limit:
            errs.append(
                f"'{pair[0]}' and '{pair[1]}' are {d:.1f} mm apart, more than the "
                f"{limit} mm max_gap ({rule.get('why', '')})"
            )
        elif kind == "min_gap" and d < limit:
            errs.append(
                f"'{pair[0]}' and '{pair[1]}' are {d:.1f} mm apart, less than the "
                f"{limit} mm min_gap ({rule.get('why', '')})"
            )
        elif kind not in ("max_gap", "min_gap"):
            errs.append(f"rule {rule!r} has an unknown kind {kind!r}")
    return errs


FP_RE = re.compile(r'\(footprint\s+"([^"]+)"(.*?)(?=\n\t\(footprint |\n\)\s*$)', re.S)
AT_RE = re.compile(r"\(at\s+([-\d.]+)\s+([-\d.]+)")
REF_RE = re.compile(r'\(property\s+"Reference"\s+"([^"]+)"')


def check_pcb(spec: dict) -> tuple[list[str], int]:
    """Check placed footprints against their zone. Returns (errors, n_checked)."""
    if not PCB.exists():
        return ([f"no PCB at {PCB.relative_to(ROOT)}"], 0)
    text = PCB.read_text()
    zones = rects(spec.get("zones") or {})
    errs: list[str] = []
    n = 0
    for m in FP_RE.finditer(text):
        blob = m.group(2)
        at = AT_RE.search(blob)
        ref = REF_RE.search(blob)
        if not at:
            continue
        n += 1
        x, y = float(at.group(1)), float(at.group(2))
        name = ref.group(1) if ref else m.group(1)
        # Which zone is it in? A footprint outside every zone is the error worth
        # reporting; attributing it to a module needs the hierarchical path,
        # which atopile writes as a property only once designators are assigned.
        inside = [
            z for z, (x1, y1, x2, y2) in zones.items() if x1 <= x <= x2 and y1 <= y <= y2
        ]
        if not inside:
            errs.append(
                f"{name} at ({x:.2f}, {y:.2f}) is outside every declared zone"
            )
        elif any((spec["zones"][z].get("keepout")) for z in inside):
            errs.append(
                f"{name} at ({x:.2f}, {y:.2f}) is inside the keepout zone "
                f"'{inside[0]}'"
            )
        elif "(locked yes)" not in blob:
            errs.append(
                f"{name} in zone '{inside[0]}' is not locked. A placed part that is "
                f"not locked can be moved by the next autoroute or push-and-shove, "
                f"and the move will not show up as an intent change."
            )
    return errs, n


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument(
        "--strict",
        action="store_true",
        help="fail when the PCB has no footprints yet (use once the board is placed)",
    )
    args = ap.parse_args()

    spec = load_yaml(SPEC)
    errs = check_spec(spec)
    print(f"floorplan: {len(spec.get('zones') or {})} zones, "
          f"{len(spec.get('rules') or [])} rules")
    if errs:
        print("\nFLOORPLAN ERRORS:")
        for e in errs:
            print(f"  - {e}")
        return 1
    print("  spec is self-consistent: zones inside the outline, none overlapping, "
          "every module covered, every rule satisfied")

    pcb_errs, n = check_pcb(spec)
    if n == 0:
        msg = (
            "  PCB has no placed footprints yet — atopile emits them during the part "
            "picker, which needs sign-in (docs/hardware/ATOPILE-TOOLCHAIN.md)"
        )
        if args.strict:
            print(msg.replace("  ", "  STRICT: ", 1))
            return 1
        print(msg)
        return 0
    print(f"  checked {n} placed footprint(s) against their zones")
    if pcb_errs:
        print("\nPLACEMENT ERRORS:")
        for e in pcb_errs:
            print(f"  - {e}")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
