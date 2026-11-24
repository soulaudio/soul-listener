---
name: atopile
description: "Authoring and reviewing this repo's atopile (.ato) hardware design: the pinned 0.15.9 CLI, what builds offline, the vendored-parts path, the floorplan contract, and the syntax traps that actually bite. Use when touching anything under hardware/."
---

# atopile in this repo

Read this before editing anything under `hardware/`. Two facts here will
otherwise cost a day, and both contradict what the public atopile docs say.

Language rules in §3 are adapted from atopile's own `.claude/skills/ato/SKILL.md`
(MIT, Copyright (c) 2024 atopile). Everything else is this repo's, and where the
two disagree **this file wins** — theirs targets the hosted app's MCP tools and
predates 0.15.8.

## 1. The two facts

**The pin is `atopile==0.15.9` and it is not a preference.** Every earlier
version resolves parts against `components.atopileapi.com`, which no longer
exists in DNS. "Stay on the stable line" is not available. Python 3.14 only.

**`assert` statements are not checked by the offline build.** They are solver
constraints, the solve runs *after* part picking, and picking needs an
interactive sign-in. Changing `assert xtal.frequency within 24MHz to 26MHz` to
`10MHz to 12MHz` against a 25 MHz crystal **passes** `cargo xtask hardware
check`. What the offline check does catch, measured by breaking real modules:
syntax errors, references to non-existent fields, and ERC faults such as a
shorted power rail.

Full background: `docs/hardware/ATOPILE-TOOLCHAIN.md`.

## 2. Commands

```sh
cargo xtask hardware check                     # every module + board + floorplan
cargo xtask hardware check-changed <files>     # one slice (the pre-commit hook)
cargo xtask hardware render                    # SVG/GLB/PNG via kicad-cli
tools/ato-diagram.py                           # -> hardware/elec/CIRCUIT.md
tools/ato-placement.py                         # floorplan self-consistency
tools/ato-vendor-part.py --check               # vendored-part state
```

Invoke atopile through **`tools/ato`**, never a bare `ato`. The wrapper installs
it on first use (Python 3.14 from nixpkgs, `libstdc++` on the loader path, a
cache dir under `~/.cache/soul-listener`) because none of that works by default
on NixOS. `cargo xtask hardware` falls back to it automatically.

```sh
tools/ato --version
tools/ato auth login     # from a TERMINAL — a browser sign-in does not create a CLI session
tools/ato build -b check-clock
```

## 3. Writing `.ato`

**Wire at interface level, not pin level.** Use the stdlib `ElectricPower`,
`I2C`, `SPI`, `UART`, `ElectricLogic`, `ElectricSignal`. Connect through `.line`
and `.reference`:

```ato
i2c.sda.line ~ chip.SDA
i2c.sda.reference ~ power_3v3
```

**Instantiate with `new`.** `r = Resistor` is an error; `r = new Resistor` is not.
Arrays: `caps = new Capacitor[4]`.

**Use the real parameter names.** `.resistance`, `.capacitance`, `.inductance` —
never `.value`. That rename is one of the 0.12→0.15 breaks this repo already hit:
`Inductor.value` no longer exists.

**Two-terminal passives expose `unnamed[0]` / `unnamed[1]`, not `p1` / `p2`.**
270 renames in this repo came from that one change. A *custom* `component` may
legitimately declare `signal p1` — only the stdlib types changed.

**Tolerance must be commensurable.** `25MHz +/- 30ppm` is a `TypeError`. The
stdlib `Crystal` models it properly: `frequency` in Hz, and
`frequency_tolerance`, `frequency_temperature_tolerance`, `frequency_ageing`
separately in ppm. Also `load_capacitance` and `equivalent_series_resistance`.

**Never zero tolerance on a picked part.** `resistance = 10kohm` matches no real
part. Always `+/- N%`.

**One import per line.** `import A, B` is deprecated and this repo has already
split all nine occurrences.

**Don't shadow a stdlib type.** This repo had a local `Crystal` module that
silently lost to the stdlib's, producing `Field xtal.p1 could not be resolved`.
Check the stdlib before naming a module.

**`assert`: one comparison, `within` for bounds, `is` for identity.**
`assert 1V < x < 5V` is not supported. `assert x is 3.3V` is deprecated — write
`assert x within 3.3V +/- 0%`.

**Statements are order-independent.** Blocks feed a solver; nothing executes
top-to-bottom. There is no `if`/`while`/`match`, no functions, no classes.
Express conditionals as constraints or as alternative modules.

## 4. What atopile does and does not own

| | |
|---|---|
| Footprints and nets in the PCB | **atopile** (`update-pcb`, in place) |
| Board outline | **atopile**, from `main.ato`'s `RectangularBoardShape` |
| Tracks, vias, zones | **you**, in KiCad — atopile writes none and preserves yours |
| Net classes, trace widths, diff-pair rules, DRU | **KiCad only** — not expressible in `.ato` at all |
| Component placement | **you**, in KiCad; intent is declared in `placement.yaml` |

**Do not hand-draw `Edge.Cuts`.** When a `RectangularBoardShape` is declared,
atopile deletes every Edge.Cuts primitive on every build. Change the board size
in `main.ato` *and* `placement.yaml` together —
`board_outline_matches_the_floorplan` asserts they agree.

Three more behaviours worth knowing before you debug something:

- **Every build rewrites the layout file**, even with no changes (there is a
  `# TODO this is always false` in atopile's own change detection). Never use
  mtime as a change signal.
- **Removing a net does not remove its copper** — segments are set to net 0, so
  routing survives geometrically and DRC complains.
- **Board appearance is overwritten each build** (soldermask black, ENIG).

## 5. Placement

`hardware/elec/placement.yaml` declares a zone per module, each with a `why`
traceable to that module's own comment, plus adjacency and separation rules.
`tools/ato-placement.py` checks the spec against itself now and against the PCB
once footprints exist. `tools/ato-outline.py` draws the zones onto
`Dwgs.User`/`Cmts.User` so whoever routes the board can see the floorplan.

Adding a module means adding a zone, or `every_module_has_a_placement_zone`
fails.

## 6. Vendored parts — the offline path

A part carrying `has_part_picked` is pruned from the pick tree, so the picker
never runs for it. That is the only way to build, BOM or render 3D without
signing in, and the only arrangement that will still work when the 0.15 line is
withdrawn.

```sh
tools/ato-vendor-part.py --mpn <MPN> --footprint <Lib>:<Footprint> --designator U
```

Scaffolds `hardware/elec/src/parts/<Mfr>_<MPN>/` from `hardware/bom/selected.json`
with the trait block filled in, and copies the footprint from KiCad's own
library. It leaves the **pin map empty on purpose** — pin numbers come from the
datasheet, and deriving them from pad numbers produces a plausible, unreviewed,
wrong netlist.

Two constraints that are easy to trip:

- `supplier_id` must be the literal `"lcsc"`; anything else raises.
- `supplier_partno` must be **non-empty** or the `bom` target fails, and the 3D
  targets depend on `bom`. This repo has no LCSC numbers, so a placeholder is
  written and atopile's BOM is consequently wrong for that part. The real offers
  are in `hardware/bom/snapshot.json`. `--check` lists every placeholder.

`--keep-picked-parts` is **not** an offline mechanism: it pins the choice via
`is_pickable_*`, so the picker still runs and still needs the API.

## 7. Pins: one source, two consumers

A component's pin assignment lives **only** in its `.ato` definition. After
editing any `~ pin` line, run:

```sh
tools/ato-pinmap.py          # regenerates pinmap.json + dap/hw-spec/src/pinmap.rs
```

Both generated files are committed on purpose, so a pin change appears in the
diff where a reviewer with the datasheet open can catch a transposition.
`the_pinmap_is_current` fails the build when they are stale.

Never declare a pin in Rust. `dap-core` may not name one at all; `board-h7a3`
reads `dap_hw_spec::pinmap`. Both are gated.

Component identity flows the same way: the spec's `Preference::Preferred` →
`selected.json` → the vendored part's `is_atomic_part` trait, and
`vendored_parts_match_the_specs_picks` fails if a vendored part outlives its pick.

## 8. When a build fails

1. `Multiple build targets share the same entry point` — two builds in
   `ato.yaml` point at the same module. One build per entry point; use
   `-t <target>` for artefact variants.
2. `Target __default__ not recognized` — it is `default` now.
3. `Field X.p1 could not be resolved` — stdlib two-terminal rename, or a local
   module shadowed by a stdlib type of the same name.
4. `Tolerance unit ppm is not commensurable` — move the ppm onto a
   `*_tolerance` parameter.
5. `Sign-in required` at `Picking parts` — expected offline. Every stage before
   it should be green; if one is not, that is the real failure.
6. `Board outline is malformed` — no `RectangularBoardShape` declared.

Build one module at a time (`-b check-power`) before the whole board. A module
is 20–35 s; the full design is about five minutes.
