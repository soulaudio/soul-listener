#!/usr/bin/env python3
"""Generate the pin map from the `.ato` sources — one source of truth, two consumers.

    tools/ato-pinmap.py            # regenerate both outputs
    tools/ato-pinmap.py --check    # fail if either is stale

Writes:
  hardware/elec/pinmap.json      machine-readable, for tools and review
  dap/hw-spec/src/pinmap.rs      a no_std const table, for the firmware

# Why the `.ato` file is the source and not the Rust

A component's pin assignment is a schematic fact: `signal CS_NEG ~ pin 1` is what
becomes a pad connection in the netlist and a pin on the footprint. If the
firmware declared it instead, the authoritative copy would be the one that does
NOT drive the board, and the netlist would silently be derived from the other.

So the direction is fixed: `.ato` is written by a person against a datasheet,
and everything else is generated from it. The firmware gets a generated table
rather than its own constants, and `the_pinmap_is_current` fails the build if the
table and the sources disagree.

# Why the generated Rust is committed rather than built

Three reasons, and the first is the one that matters:

1. **A pin change should appear in review.** A build-time generator hides it;
   a committed table puts `("CS_NEG", "1")` into the diff, where a reviewer with
   the datasheet open can catch a transposition. Pin errors are the most
   expensive class of board defect and the cheapest to catch by reading.
2. `dap-hw-spec` is `no_std` and dependency-free. Parsing anything at build time
   would mean a `build.rs` and a parser dependency in the crate the whole
   firmware pins itself against.
3. The arch tests can compare a committed file. They cannot easily compare
   something that only exists inside a build directory.

# What this does NOT do

It does not invent pins. A component with no `~ pin` lines produces an empty
entry, which is honest: `tools/ato-vendor-part.py` scaffolds parts with the pin
map deliberately blank, because deriving pin numbers from a footprint's pads
produces a plausible, unreviewed, wrong netlist.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
SRC = ROOT / "hardware/elec/src"
JSON_OUT = ROOT / "hardware/elec/pinmap.json"
RUST_OUT = ROOT / "dap/hw-spec/src/pinmap.rs"
SELECTED = ROOT / "hardware/bom/selected.json"

BLOCK_RE = re.compile(r"^(component|module)\s+(\w+)\s*:", re.M)
# `signal CS_NEG  ~ pin 1` and the bare `A ~ pin 1` form. Pin designators are
# kept as STRINGS: a BGA pin is `A1`, and coercing to an integer would lose it.
PIN_RE = re.compile(r"^\s*(?:signal\s+)?(\w+)\s*~\s*pin\s+\"?([\w.]+)\"?", re.M)
MPN_RE = re.compile(r'partnumber="([^"]+)"')
MFR_RE = re.compile(r'manufacturer="([^"]+)"')


def strip_comments(src: str) -> str:
    return "\n".join("" if l.lstrip().startswith("#") else l for l in src.splitlines())


def collect() -> dict:
    """Every component's pin map, keyed by component name."""
    components: dict[str, dict] = {}
    for f in sorted(SRC.rglob("*.ato")):
        raw = f.read_text()
        code = strip_comments(raw)
        marks = list(BLOCK_RE.finditer(code))
        for i, m in enumerate(marks):
            end = marks[i + 1].start() if i + 1 < len(marks) else len(code)
            body = code[m.start():end]
            pins = PIN_RE.findall(body)
            if not pins and m.group(1) == "module":
                # A module with no pins is a composition, not a component.
                continue
            name = m.group(2)
            # Identity from the vendored trait block, when present.
            mpn = MPN_RE.search(body)
            mfr = MFR_RE.search(body)
            dup = {}
            for sig, pin in pins:
                dup.setdefault(sig, []).append(pin)
            components[name] = {
                "file": str(f.relative_to(ROOT)),
                "kind": m.group(1),
                "manufacturer": mfr.group(1) if mfr else None,
                "mpn": mpn.group(1) if mpn else None,
                # Sorted by pin for review: a reader checking a datasheet reads
                # down the pin column, not alphabetically by signal.
                "pins": [
                    {"signal": s, "pin": p}
                    for s, p in sorted(pins, key=lambda sp: _pin_key(sp[1]))
                ],
                "signals_on_multiple_pins": {
                    s: ps for s, ps in dup.items() if len(ps) > 1
                },
            }
    return components


def _pin_key(pin: str):
    """Sort 1,2,10 numerically and A1,B2 lexically, without mixing them up."""
    return (0, int(pin), "") if pin.isdigit() else (1, 0, pin)


def build_json(components: dict) -> str:
    return json.dumps(
        {
            "readme": (
                "GENERATED from hardware/elec/src/**/*.ato by tools/ato-pinmap.py. "
                "Do not edit. The .ato component definitions are the single source of "
                "truth for pin assignment; this file and dap/hw-spec/src/pinmap.rs are "
                "both derived from them, and the_pinmap_is_current fails the build if "
                "either goes stale."
            ),
            "components": components,
        },
        indent=1,
        sort_keys=True,
    ) + "\n"


def rust_ident(s: str) -> str:
    return re.sub(r"[^A-Za-z0-9_]", "_", s)


def build_rust(components: dict) -> str:
    rows = []
    for name in sorted(components):
        c = components[name]
        pins = ", ".join(
            f'("{p["signal"]}", "{p["pin"]}")' for p in c["pins"]
        )
        mpn = f'Some("{c["mpn"]}")' if c["mpn"] else "None"
        mfr = f'Some("{c["manufacturer"]}")' if c["manufacturer"] else "None"
        rows.append(
            f"    ComponentPins {{\n"
            f'        component: "{name}",\n'
            f"        manufacturer: {mfr},\n"
            f"        mpn: {mpn},\n"
            f'        source: "{c["file"]}",\n'
            f"        pins: &[{pins}],\n"
            f"    }},"
        )
    body = "\n".join(rows)
    n = len(components)
    total = sum(len(c["pins"]) for c in components.values())
    return f'''//! Pin assignments, GENERATED from the atopile sources. Do not edit.
//!
//! Regenerate with `tools/ato-pinmap.py`; `the_pinmap_is_current` in
//! `dap-arch-tests` fails the build when this file and
//! `hardware/elec/src/**/*.ato` disagree.
//!
//! # Why the firmware reads a generated table
//!
//! A component's pin assignment is a schematic fact — `signal CS_NEG ~ pin 1`
//! is what becomes a pad connection in the netlist and a pin on the footprint.
//! Declaring it a second time in Rust would create a copy that does not drive
//! the board, and the two would drift the first time a pin moved. So the `.ato`
//! definition is written by a person against a datasheet and this table is
//! derived from it.
//!
//! The firmware must not name a pin any other way:
//! `the_firmware_core_names_no_pin_or_peripheral` forbids pin literals in
//! `dap-core`, and a driver in the board crate takes its pins as parameters.
//!
//! Pin designators are `&str`, not integers, because a BGA pin is `A1`.
//!
//! {n} component(s), {total} pin assignment(s).

/// One component's pin assignment, as its `.ato` definition states it.
#[derive(Debug, Clone, Copy)]
pub struct ComponentPins {{
    /// The `.ato` block name (`component W25Q128JVSIQ`).
    pub component: &'static str,
    /// From a vendored part's `is_atomic_part` trait, when it has one.
    pub manufacturer: Option<&'static str>,
    pub mpn: Option<&'static str>,
    /// Where it is defined, so a failure points at a file to open.
    pub source: &'static str,
    /// `(signal, pin)`, ordered by pin.
    pub pins: &'static [(&'static str, &'static str)],
}}

impl ComponentPins {{
    /// The pin a signal is on.
    #[must_use]
    pub fn pin(&self, signal: &str) -> Option<&'static str> {{
        let mut i = 0;
        while i < self.pins.len() {{
            if str_eq(self.pins[i].0, signal) {{
                return Some(self.pins[i].1);
            }}
            i += 1;
        }}
        None
    }}
}}

/// `str::eq` is not `const`-friendly here and this crate carries no deps; a
/// byte compare keeps the module usable from `no_std` without `core::str`
/// niceties that pull in formatting machinery.
fn str_eq(a: &str, b: &str) -> bool {{
    a.as_bytes() == b.as_bytes()
}}

/// Every component the atopile design defines, with its pins.
pub const COMPONENTS: &[ComponentPins] = &[
{body}
];

/// Look a component up by its `.ato` block name.
#[must_use]
pub fn component(name: &str) -> Option<&'static ComponentPins> {{
    let mut i = 0;
    while i < COMPONENTS.len() {{
        if str_eq(COMPONENTS[i].component, name) {{
            return Some(&COMPONENTS[i]);
        }}
        i += 1;
    }}
    None
}}
'''


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--check", action="store_true",
                    help="exit non-zero if either generated file is stale")
    args = ap.parse_args()

    components = collect()
    want_json = build_json(components)
    want_rust = build_rust(components)

    total = sum(len(c["pins"]) for c in components.values())
    dupes = {
        n: c["signals_on_multiple_pins"]
        for n, c in components.items()
        if c["signals_on_multiple_pins"]
    }

    if args.check:
        stale = []
        for path, want in ((JSON_OUT, want_json), (RUST_OUT, want_rust)):
            have = path.read_text() if path.exists() else ""
            if have != want:
                stale.append(str(path.relative_to(ROOT)))
        if stale:
            print(
                "pin map is STALE: " + ", ".join(stale) + "\n"
                "The .ato sources are the single source of truth; these files are "
                "generated from them. Run `tools/ato-pinmap.py` and commit the result.\n"
                "A pin change SHOULD appear in the diff — that is where a "
                "transposition gets caught.",
                file=sys.stderr,
            )
            return 1
        print(f"pin map is current: {len(components)} component(s), {total} assignment(s)")
        return 0

    JSON_OUT.write_text(want_json)
    RUST_OUT.write_text(want_rust)
    print(f"wrote {JSON_OUT.relative_to(ROOT)} and {RUST_OUT.relative_to(ROOT)}")
    print(f"  {len(components)} component(s), {total} pin assignment(s)")
    if dupes:
        # Legitimate (a chip with two VDD pins tied together) but worth seeing.
        print("  signals on more than one pin (usually tied supplies):")
        for n, d in sorted(dupes.items()):
            for sig, pins in sorted(d.items()):
                print(f"      {n}.{sig} -> pins {', '.join(pins)}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
