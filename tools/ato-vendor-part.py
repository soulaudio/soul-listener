#!/usr/bin/env python3
"""Vendor a selected part into `hardware/elec/src/parts/`, for offline builds.

    tools/ato-vendor-part.py --list
    tools/ato-vendor-part.py --mpn SN74LVC1G3157DCKR \\
        --footprint Package_TO_SOT_SMD:SOT-363_SC-70-6 --designator U
    tools/ato-vendor-part.py --check

# Why vendoring is the point, not an optimisation

atopile 0.15.8+ resolves generic components (`new Capacitor` with a capacitance)
against a remote service that now requires an interactive sign-in, and the 0.15
CLI line is soft-deprecated with a removal date the maintainers say they will
announce. The components service used by the *previous* releases was already
retired once without a migration path — which is why `components.atopileapi.com`
no longer resolves and this project's builds were dead for days.

A part carrying the `has_part_picked` trait is pruned from the pick tree
entirely, so the picker never runs for it. Measured end to end on a two-part
board with no credentials and no network reachable to the parts API:

    ✓ Picking parts [0.06s]    ✓ bom [0.06s]
    ✓ step [5.38s]             ✓ glb [4.14s]    ✓ 3d-image [17.44s]

So vendored parts are the only durable way to build this design, and the only
way to get a 3D render without signing in. Each vendored part is three files in
git that will still work when the service behind the CLI is gone.

# What a vendored part is

A directory under `paths.parts` holding:

  <Mfr>_<MPN>/
    <Mfr>_<MPN>.ato     the component: trait block + pin map
    <footprint>.kicad_mod
    <symbol>.kicad_sym  (optional; only the schematic view uses it)
    <model>.step        (optional; only the 3D targets use it)

`footprint`, `symbol` and `model` in the trait are filenames **relative to the
`.ato` file's own directory**, not project-relative.

# The supplier number, and an honest limitation

`has_part_picked::by_supplier` takes `supplier_id` and `supplier_partno`, and
`supplier_id` must be the literal `"lcsc"` — anything else raises
`Supplier <x> not supported`. The number is never validated offline, but it must
be NON-EMPTY: with a blank one, every stage passes except `bom`, which the 3D
targets depend on, so the build stops before rendering.

This project sources from TME, Digi-Key and Mouser and does not collect LCSC
numbers. So `--lcsc` is optional and a placeholder is written when it is
omitted — which means **atopile's generated BOM is wrong for that part**. That
is an acceptable trade here because atopile's BOM is not this project's BOM:
`hardware/bom/snapshot.json` and `PROPOSALS.md` carry the real offers, with
stock, price breaks and per-distributor links. `--check` reports every
placeholder so they are never mistaken for real data.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
PARTS = ROOT / "hardware/elec/src/parts"
SELECTED = ROOT / "hardware/bom/selected.json"

PLACEHOLDER = "LCSC-UNKNOWN"


def kicad_footprint_dir() -> pathlib.Path | None:
    """Locate KiCad's shipped footprint libraries.

    Vendoring from KiCad's own library rather than hand-drawing pads: these are
    the reference footprints, IPC-compliant and reviewed by more people than
    this project will ever have. `SOT-363_SC-70-6` and `C_0402_1005Metric` are
    both there.
    """
    try:
        out = subprocess.run(
            ["nix", "eval", "--raw", "nixpkgs#kicad.passthru.libraries.footprints"],
            capture_output=True, text=True, timeout=120, check=False,
        )
        if out.returncode == 0 and out.stdout.strip():
            p = pathlib.Path(out.stdout.strip()) / "share/kicad/footprints"
            if p.is_dir():
                return p
    except Exception:
        pass
    for guess in (
        "/usr/share/kicad/footprints",
        "/Applications/KiCad/KiCad.app/Contents/SharedSupport/footprints",
    ):
        p = pathlib.Path(guess)
        if p.is_dir():
            return p
    return None


def selected_parts() -> list[dict]:
    if not SELECTED.exists():
        sys.exit(
            f"no {SELECTED.relative_to(ROOT)} — run "
            f"`cargo run -p dap-arch-tests --example propose` first"
        )
    return json.loads(SELECTED.read_text()).get("selected", [])


def slug(s: str) -> str:
    return re.sub(r"[^A-Za-z0-9]+", "_", s).strip("_")


def ato_for(entry: dict, footprint_file: str, symbol_file: str | None,
            model_file: str | None, designator: str, lcsc: str) -> str:
    mfr, mpn = entry["manufacturer"], entry["mpn"]
    ident = f"{slug(mfr)}_{slug(mpn)}"
    ds = entry.get("datasheet_url") or ""

    traits = [
        f'    trait is_atomic_part<manufacturer="{mfr}", partnumber="{mpn}", '
        f'footprint="{footprint_file}"'
        + (f', symbol="{symbol_file}"' if symbol_file else "")
        + (f', model="{model_file}"' if model_file else "")
        + ">",
        f'    trait has_part_picked::by_supplier<supplier_id="lcsc", '
        f'supplier_partno="{lcsc}", manufacturer="{mfr}", partno="{mpn}">',
        f'    trait has_designator_prefix<prefix="{designator}">',
    ]
    if ds:
        traits.append(f'    trait has_datasheet<datasheet="{ds}">')

    lcsc_note = ""
    if lcsc == PLACEHOLDER:
        lcsc_note = (
            "#\n"
            "# SUPPLIER NUMBER IS A PLACEHOLDER. This project sources from TME,\n"
            "# Digi-Key and Mouser and does not collect LCSC numbers, but the trait\n"
            "# requires a non-empty one or the `bom` target fails and the 3D targets\n"
            "# never run. Consequence: atopile's generated BOM is WRONG for this part.\n"
            "# The real offers are in hardware/bom/snapshot.json — distributor, stock,\n"
            "# MOQ, price breaks and links. `ato-vendor-part.py --check` lists every\n"
            "# placeholder so none is mistaken for real data.\n"
        )

    return f"""# {mpn} — {mfr}
#
# VENDORED PART. Generated by tools/ato-vendor-part.py; the pin map below is
# the part that needs human attention.
#
# Vendored so the build needs no part picker: `has_part_picked` prunes this
# component from the pick tree, so atopile never calls the parts service for it.
# That is what makes an offline build — and therefore any 3D render — possible
# at all on 0.15.x. See docs/hardware/ATOPILE-TOOLCHAIN.md.
{lcsc_note}#
# PIN MAP: EMPTY AND DELIBERATELY SO. Pin numbers come from the datasheet, and
# generating them from a footprint's pad numbers would produce a plausible,
# unreviewed, wrong netlist — the single most expensive kind of mistake a board
# can carry. Add `signal <NAME> ~ pin <N>` lines below against the datasheet:
#   {ds or "(no datasheet URL recorded)"}
#
# `footprint`/`symbol`/`model` are filenames relative to THIS directory.
#pragma experiment("TRAITS")
import is_atomic_part
import has_part_picked
import has_designator_prefix
{"import has_datasheet" if ds else ""}

component {ident}_package:
{chr(10).join(traits)}

    # TODO: pin map from the datasheet.
"""


def vendor(mpn: str, footprint: str, designator: str, lcsc: str) -> int:
    entries = selected_parts()
    entry = next((e for e in entries if e["mpn"] == mpn), None)
    if entry is None:
        print(f"{mpn!r} is not a selected part. Available:", file=sys.stderr)
        for e in entries:
            print(f"  {e['mpn']:<24} {e['requirement']}", file=sys.stderr)
        return 1

    if ":" not in footprint:
        print("--footprint must be <Library>:<FootprintName>, e.g. "
              "Package_TO_SOT_SMD:SOT-363_SC-70-6", file=sys.stderr)
        return 1
    lib, fp = footprint.split(":", 1)

    fp_root = kicad_footprint_dir()
    if fp_root is None:
        print("could not find KiCad's footprint libraries. Try:\n"
              "  nix shell nixpkgs#kicad --command tools/ato-vendor-part.py ...",
              file=sys.stderr)
        return 1
    src = fp_root / f"{lib}.pretty" / f"{fp}.kicad_mod"
    if not src.exists():
        print(f"no footprint {src}", file=sys.stderr)
        return 1

    ident = f"{slug(entry['manufacturer'])}_{slug(mpn)}"
    out = PARTS / ident
    out.mkdir(parents=True, exist_ok=True)
    shutil.copy2(src, out / f"{fp}.kicad_mod")
    (out / f"{ident}.ato").write_text(
        ato_for(entry, f"{fp}.kicad_mod", None, None, designator, lcsc)
    )
    print(f"vendored {mpn} -> {out.relative_to(ROOT)}/")
    print(f"  {fp}.kicad_mod   (from KiCad {lib})")
    print(f"  {ident}.ato      trait block written; PIN MAP IS EMPTY")
    if lcsc == PLACEHOLDER:
        print(f"  supplier number is a PLACEHOLDER ({PLACEHOLDER}) — atopile's BOM "
              f"will be wrong for this part")
    print("\nNext: add the pin map from the datasheet, then instantiate it in a module.")
    return 0


#: Parts that predate vendoring and carry a pin map but no footprint.
#:
#: These are the V1 design's components. They were written as pin maps only —
#: every footprint reference in them is a PROSE COMMENT ("Place
#: ES9038Q2M.kicad_sym and TSSOP-28.kicad_mod in this directory"), never a
#: declaration — so atopile warns "No pickers and no footprint … will not appear
#: in netlist or pcb" and the board cannot be laid out. That is pre-existing
#: debt, not something this tool introduced, and it is recorded as a number so
#: the count cannot quietly grow while nobody is looking.
KNOWN_FOOTPRINTLESS = 21


def check(strict: bool = False) -> int:
    """Report the state of every vendored part."""
    if not PARTS.is_dir():
        print("no vendored parts yet")
        return 0
    rows, problems, scaffolded = [], [], []
    for d in sorted(p for p in PARTS.iterdir() if p.is_dir()):
        atos = list(d.glob("*.ato"))
        if not atos:
            continue
        raw = atos[0].read_text()
        # Strip comments before counting. This tool's own generated header
        # contains the instruction `signal <NAME> ~ pin <N>`, which matched the
        # pin-count pattern and reported a freshly vendored part as having one
        # pin — a checker fooled by its own documentation.
        src = "\n".join(
            "" if l.lstrip().startswith("#") else l for l in raw.splitlines()
        )
        has_fp = bool(list(d.glob("*.kicad_mod")))
        has_model = bool(list(d.glob("*.step")))
        placeholder = PLACEHOLDER in raw
        # A pin map with no `pin` lines is a part that cannot be wired.
        pins = len(re.findall(r"~\s*pin\s+\S+", src))
        rows.append((d.name, has_fp, has_model, pins, placeholder))
        if pins == 0:
            # An empty pin map only matters if something instantiates the part.
            # Freshly scaffolded and not yet used is a normal intermediate state
            # — the pin map comes from a datasheet a person has to read. An
            # INSTANTIATED part with no pins is a real defect: it contributes a
            # footprint to the board and connects to nothing.
            ident = f"{d.name}_package"
            used = any(
                ident in f.read_text()
                for f in (ROOT / "hardware/elec/src").rglob("*.ato")
                if f.parent != d
            )
            if used:
                problems.append(
                    f"{d.name}: pin map is EMPTY but the component is INSTANTIATED. "
                    f"It will place a footprint that connects to nothing."
                )
            else:
                scaffolded.append(d.name)
    print(f"{'part':<40} {'fp':<4} {'3d':<4} {'pins':>5}  supplier")
    for name, fp, model, pins, ph in rows:
        print(f"{name:<40} {'yes' if fp else 'NO':<4} {'yes' if model else '-':<4} "
              f"{pins:>5}  {'PLACEHOLDER' if ph else 'real'}")
    if scaffolded:
        print(f"\nscaffolded, pin map not yet written (not instantiated, so harmless): "
              f"{', '.join(scaffolded)}")
        print("  add `signal <NAME> ~ pin <N>` from the datasheet before using them")
    footprintless = [n for n, fp, _, _, _ in rows if not fp]
    print(f"\n{len(rows)} part(s); {len(footprintless)} with no footprint "
          f"(known baseline {KNOWN_FOOTPRINTLESS})")
    if len(footprintless) > KNOWN_FOOTPRINTLESS:
        problems.append(
            f"{len(footprintless)} parts now lack a .kicad_mod, up from the known "
            f"baseline of {KNOWN_FOOTPRINTLESS}. A part with no footprint cannot be "
            f"placed, so the board silently omits it. Either vendor a footprint or "
            f"raise the baseline with a reason."
        )
    elif footprintless and not strict:
        print("  (the V1 design's parts; see KNOWN_FOOTPRINTLESS in this file)")

    if problems:
        print("\nPROBLEMS:")
        for p in problems:
            print(f"  - {p}")
        return 1
    if not rows:
        print("(none)")
    return 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--list", action="store_true", help="list selected parts")
    ap.add_argument("--check", action="store_true", help="report vendored-part state")
    ap.add_argument("--strict", action="store_true",
                    help="with --check: also list the known footprint-less V1 parts")
    ap.add_argument("--mpn", help="which selected part to vendor")
    ap.add_argument("--footprint", help="<KiCadLibrary>:<FootprintName>")
    ap.add_argument("--designator", default="U", help="designator prefix (U, R, C, J…)")
    ap.add_argument("--lcsc", default=PLACEHOLDER,
                    help="LCSC part number, if known (else a placeholder is written)")
    args = ap.parse_args()

    if args.list:
        for e in selected_parts():
            print(f"  {e['mpn']:<24} {e['requirement']:<16} {e['manufacturer']}")
        return 0
    if args.check:
        return check(strict=args.strict)
    if not args.mpn or not args.footprint:
        ap.print_help()
        return 1
    return vendor(args.mpn, args.footprint, args.designator, args.lcsc)


if __name__ == "__main__":
    sys.exit(main())
