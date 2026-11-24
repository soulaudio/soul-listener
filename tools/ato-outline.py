#!/usr/bin/env python3
"""Draw the board outline and floorplan zones into the PCB, from the spec.

    tools/ato-outline.py            # write Edge.Cuts + zone rectangles
    tools/ato-outline.py --check    # fail if the PCB does not match the spec

# Why this exists

`hardware/elec/placement.yaml` declares a 100 x 60 mm board and a zone per
module. The committed `default.kicad_pcb` had neither — atopile emits an empty
board with layers and nothing on them, so `kicad-cli` reported *"Board outline is
malformed"* and any render showed nothing at all.

**The outline is NOT written here, and an earlier version of this tool got that
wrong.** atopile owns `Edge.Cuts` the moment a `RectangularBoardShape` is
declared in the design: `_clear_existing_outline` deletes every Edge.Cuts
primitive on every build. A hand-drawn outline and a declared one cannot
coexist — whichever runs last wins — so the outline is declared in `main.ato`
(where it is also parametrized, which is where it belongs) and this tool writes
only the floorplan annotation. `board_outline_matches_the_floorplan` keeps the
two numbers equal.

# What it writes, and on which layers

* **Dwgs.User** — one rectangle per module zone, plus its name. Documentation:
  whoever routes the board sees the floorplan in KiCad instead of having to hold
  it in their head, and a part dragged outside its zone is visibly outside it.
* **Cmts.User** — the keepout zones (the battery bay), hatched by convention.

Zone rectangles are deliberately NOT KiCad keepout zones. A real keepout changes
DRC behaviour, and the floorplan is intent rather than a rule the router must
obey — turning a first-pass floorplan into hard DRC constraints would fight the
layout engineer rather than inform them. `tools/ato-placement.py` is what
enforces it, and it reports rather than blocks.

# Idempotence

Everything written is wrapped between marker comments and regenerated wholesale,
so running this twice is a no-op and hand edits elsewhere in the file survive.
atopile preserves user edits to the PCB across builds, which is what makes
writing into it safe at all.
"""

from __future__ import annotations

import argparse
import pathlib
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PCB = ROOT / "hardware/elec/layouts/default/default.kicad_pcb"
BEGIN = "\t(gr_text \"ATO-FLOORPLAN-BEGIN\""
END_MARK = "ATO-FLOORPLAN-END"


def load_spec() -> dict:
    """Reuse the floorplan reader from `ato-placement.py`.

    Imported by path because the filename has a hyphen and is therefore not a
    normal module name. Sharing the reader matters: two parsers for one spec
    would eventually disagree about what the spec says, and the whole point is
    that the PCB and the checker are driven by the same numbers.
    """
    import importlib.util

    p = ROOT / "tools/ato-placement.py"
    spec = importlib.util.spec_from_file_location("ato_placement", p)
    assert spec and spec.loader
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod.load_yaml(mod.SPEC)


def rect(x1: float, y1: float, x2: float, y2: float, layer: str, width: float) -> str:
    return (
        f"\t(gr_rect\n"
        f"\t\t(start {x1:.3f} {y1:.3f})\n"
        f"\t\t(end {x2:.3f} {y2:.3f})\n"
        f"\t\t(stroke\n\t\t\t(width {width})\n\t\t\t(type solid)\n\t\t)\n"
        f"\t\t(fill no)\n"
        f"\t\t(layer \"{layer}\")\n"
        f"\t)\n"
    )


def text(s: str, x: float, y: float, layer: str, size: float = 1.2) -> str:
    return (
        f"\t(gr_text \"{s}\"\n"
        f"\t\t(at {x:.3f} {y:.3f} 0)\n"
        f"\t\t(layer \"{layer}\")\n"
        f"\t\t(effects\n\t\t\t(font\n\t\t\t\t(size {size} {size})\n"
        f"\t\t\t\t(thickness {size / 6:.3f})\n\t\t\t)\n"
        f"\t\t\t(justify left top)\n\t\t)\n"
        f"\t)\n"
    )


def build_block(spec: dict) -> str:
    board = spec["board"]
    zones = spec.get("zones") or {}
    bw, bh = float(board["width_mm"]), float(board["height_mm"])

    out = [text("ATO-FLOORPLAN-BEGIN", 0, -6, "Cmts.User", 1.0)]
    # No Edge.Cuts: `main.ato`'s RectangularBoardShape owns the outline, and
    # atopile deletes anything else on that layer every build.
    for name, z in sorted(zones.items()):
        x, y = float(z["x"]), float(z["y"])
        x2, y2 = x + float(z["width"]), y + float(z["height"])
        keepout = bool(z.get("keepout"))
        layer = "Cmts.User" if keepout else "Dwgs.User"
        out.append(rect(x, y, x2, y2, layer, 0.12 if keepout else 0.05))
        label = f"{name} (KEEPOUT)" if keepout else name
        out.append(text(label, x + 0.6, y + 0.6, layer, 1.0))
    out.append(text(END_MARK, 0, bh + 2, "Cmts.User", 1.0))
    return "".join(out)


def strip_block(text_in: str) -> str:
    """Remove a previously written block, so this is idempotent."""
    i = text_in.find(BEGIN)
    if i < 0:
        return text_in
    j = text_in.find(END_MARK, i)
    if j < 0:
        return text_in
    # Walk to the end of the gr_text element holding the end marker.
    k = text_in.find("\n\t)\n", j)
    if k < 0:
        return text_in
    return text_in[:i] + text_in[k + 4:]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--check", action="store_true",
                    help="exit non-zero if the PCB does not already match the spec")
    args = ap.parse_args()

    if not PCB.exists():
        print(f"no PCB at {PCB.relative_to(ROOT)}", file=sys.stderr)
        return 1
    spec = load_spec()
    original = PCB.read_text()
    block = build_block(spec)

    stripped = strip_block(original)
    # Insert before the final closing paren of the kicad_pcb s-expression.
    close = stripped.rstrip().rfind("\n)")
    if close < 0:
        print("could not find the end of the kicad_pcb expression", file=sys.stderr)
        return 1
    # rstrip the head before re-joining. Without it, strip_block leaves the
    # newline that preceded the old block and the insertion adds another, so the
    # file gained one blank line per run and `--check` reported a mismatch
    # against a file that was semantically identical.
    updated = stripped[:close].rstrip() + "\n" + block + "\n)\n"

    if args.check:
        if updated.strip() == original.strip():
            print("PCB outline and floorplan match placement.yaml")
            return 0
        print(
            "PCB outline/floorplan does NOT match hardware/elec/placement.yaml.\n"
            "Run `tools/ato-outline.py` and commit the result.\n"
            "The board edge and the zone rectangles are generated from the spec, so a "
            "mismatch means either the spec moved or the PCB was edited by hand.",
            file=sys.stderr,
        )
        return 1

    if updated.strip() == original.strip():
        print("PCB already matches placement.yaml — nothing to do")
        return 0
    PCB.write_text(updated)
    zones = spec.get("zones") or {}
    print(
        f"wrote {PCB.relative_to(ROOT)}: {len(zones)} floorplan zone(s) on "
        f"Dwgs.User/Cmts.User (the {spec['board']['width_mm']}x"
        f"{spec['board']['height_mm']} mm outline is main.ato's RectangularBoardShape)"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
