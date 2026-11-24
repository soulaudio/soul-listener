# The atopile toolchain: what works, what doesn't, and why

Measured 2026-11-24 against real installs and the live services. Every claim
here was verified by running something, not read off a docs page — the docs for
the CLI are largely 404 now, and the comments this file replaces were confidently
wrong in two places.

## The short version

| | |
|---|---|
| Pin | **`atopile==0.15.9`**, Python **3.14** only |
| Offline check | **works** — syntax, imports, field resolution, instance graph, ERC |
| `assert` rules + DRC | **cannot run in CI, by anyone** — needs interactive sign-in |
| `atopile/setup-atopile` | **unusable** at any working version |
| Upstream direction | CLI soft-deprecated; the product is now a browser IDE |

## Why the pin is 0.15.9 and not something more conservative

The previous pin was `^0.12.5`, with a comment explaining that it was the stable
line and 0.14 was a pre-release. That reasoning was sound when written and is now
void, because the service the old line depends on is gone.

Part resolution happens against a remote components API whose hostname changed:

| atopile | components endpoint | resolves in DNS? |
|---|---|---|
| 0.12.5 | `components.atopileapi.com` | **no** |
| 0.15.7 | `components.atopileapi.com` | **no** |
| 0.15.8, 0.15.9 | `legacy.atopileapi.com` | yes (401 — auth required) |

Checked from a machine where `github.com`, `atopile.io`, `docs.atopile.io` and
`pypi.org` all resolve, so this is not a local DNS quirk, and not the CI-runner
quirk the old workflow comment claimed.

**Consequence: 0.12.x and 0.15.7 can never complete a build again.** No
configuration fixes it. "Stay on the stable line" stopped being a safe default
the moment the endpoint was retired, which is the general lesson worth keeping:
a version pin to a tool that calls a network service is also a pin to that
service.

### The failure that hid this for days

The old `hardware.yml` carried a `KNOWN BROKEN` notice attributing the failure to
an upstream bug:

```
faebryk/libs/picker/api/picker_lib.py:221  cause = cause.__cause__
AttributeError: 'str' object has no attribute '__cause__'
```

That crash is real and reproduces locally, but it is atopile's error handler
failing while trying to unwrap an exception to find a `gaierror`. The `gaierror`
was the actual problem — DNS — and the handler's own bug **masked** it. Diagnosis
went to the visible traceback rather than the cause for three days. Worth
remembering next time a stack trace points at a library's internals: the frame
that crashed is not always the frame that failed.

## What you can check without signing in, exactly

Part picking on 0.15.8+ raises `Sign-in required` unless a stored session exists.
Everything before that stage still runs. The boundary was measured by breaking a
real module:

| mutation | caught offline? |
|---|---|
| syntax error | **yes** |
| reference to a non-existent field | **yes** |
| shorted power rail (ERC fault) | **yes** |
| an `assert` that is plainly false | **no** |

The last row matters and it is counter-intuitive. `assert xtal.frequency within
24MHz to 26MHz` changed to `10MHz to 12MHz`, against a crystal declared at
25 MHz, **passes** the offline check. `assert` statements are not predicates
evaluated during instantiation — they are constraints handed to a solver, and the
solve plus `post-solve-checks` run *after* the picker.

So the gate is two-tier, and neither tier should be described as the other:

- **Offline** — structure and connectivity. This is the pre-commit hook and CI.
- **Authenticated** — the above plus part picking, the constraint solve, the
  `assert` design rules and DRC. Developer machine only.

### The authenticated tier cannot be automated

`ato auth` has exactly three subcommands: `login`, `logout`, `status`. `login` is
a browser OAuth flow that writes a session to disk. There is no token environment
variable, no API-key field, and no OIDC path — `atopile/auth/session.py` and
`faebryk/libs/picker/api/api.py` contain no `getenv` for a credential.

This is not "we have not set up the secret yet". There is no secret to set up. To
check the `assert` rules and DRC, a person runs `ato auth login` and then
`cargo xtask hardware check` locally.

## Why not `atopile/setup-atopile`

The official action does not install from PyPI; it pulls
`ghcr.io/atopile/atopile-kicad:<version>` and shims `ato` through `docker run`.
Checked against the GHCR registry:

| tag | manifest |
|---|---|
| `0.15.9`, `0.15.8` | **404** |
| `0.15.7`, `0.15.0` | 200 |
| `0.12.5` | 200 |

So **no version has both a working parts endpoint and a container image.** The
action resolves `requires-atopile` against PyPI, picks the highest match — 0.15.9
— and then fails on `docker pull`. Installing the wheel directly is the only
route, which is what the workflow does.

KiCad is not needed for the checks: the `default` target aggregate (bom,
manifest, variable-report, pinout, stackup, power-tree, datasheets) emits nothing
that requires it.

## Migrating 0.12 → 0.15: what actually broke

Applied to this repo on 2026-11-24. All of it was mechanical and verified by the
compiler, module by module.

1. **Two builds may not share an entry point.** `default`, `check-only` and
   `bom-only` all entered at `main.ato:SoulAudioDAP`; 0.15.x refuses to load the
   project. The distinction they encoded is an invocation concern, so it moved to
   the command line (`-t bom`).
2. **`__default__` is not a target.** Now just `default`.
3. **Two-terminal passives expose `unnamed[0]`/`unnamed[1]`, not `p1`/`p2`.**
   270 renames across 9 files. Only instances *declared* as a stdlib
   `Capacitor`/`Resistor`/`Inductor` were touched — a custom `component` may
   legitimately declare `signal p1`, and renaming those breaks a correct file.
4. **`Inductor.value` → `Inductor.inductance`.** 7 sites.
5. **`import A, B` on one line is deprecated.** 9 lines split.
6. **ppm is not commensurable with an absolute unit.** `25MHz +/- 30ppm` is a
   `TypeError`. This turned out to be a *better* model arriving, not a
   regression: the stdlib now has a `Crystal` with `frequency` in Hz and
   `frequency_tolerance`, `frequency_temperature_tolerance` and
   `frequency_ageing` separately, in ppm. So the ppm moved to where it belongs.
7. **The local `Crystal` module shadowed the new stdlib type** and was deleted.
   Shadowing a stdlib name is a documented pitfall; the stdlib version is
   strictly richer (it also models `load_capacitance` and
   `equivalent_series_resistance`, both of which this design's comments already
   specified in prose).

Result: all eight per-module checks and the full board clear every stage through
ERC.

Timings, measured cold through `cargo run` (an earlier draft of this file quoted
warm numbers and understated both by roughly 5x):

| | |
|---|---|
| one module | 20-35 s |
| the full board alone | 82 s |
| every module + the board (`hardware check`) | ~5 min |

Which is why the pre-commit hook checks only the changed slice, and why a change
to a shared file — where it cannot tell which slice is affected — is the one case
that costs five minutes.

## The uncomfortable part: the CLI is being wound down

From the maintainers' own post, [atopile.io/blog/atopile-v16](https://atopile.io/blog/atopile-v16)
(2026-08-06):

> "atopile 0.16 now runs the full IDE in your browser: app.atopile.io."
>
> "We are soft-deprecating the `0.15` line. Version `0.15.8` exists to keep an
> active project moving while you finish or plan a migration; it is not a second
> product line that we will maintain indefinitely. We will announce a removal
> date in the future."
>
> "The compiler and graph core will remain open source, so you can always build
> your projects."

The CLI says so itself on every invocation: *"atopile 0.15.8 is the last CLI
release and is in maintenance mode only."* `docs.atopile.io/atopile/*` is 404 —
the documentation moved inside the hosted editor. 0.16 also introduces a new,
incompatible surface syntax with no published migration guide.

**What this means for this repo.** Everything above is worth having: the design
compiles, the slices are checked, and a commit cannot silently break the
structure. But it is built on a CLI whose vendor has announced it will eventually
be removed, and the part-picking half already depends on a hosted service we
cannot automate against.

Three options, stated plainly rather than hidden in a comment:

1. **Stay on 0.15.9.** Works today. The offline tier keeps working even if the
   service disappears, because it never calls it. Part picking stops the day the
   endpoint is retired or the session flow changes.
2. **Move to the hosted 0.16 workspace.** Supported direction, but it means a
   new language dialect, no local CI gate, and the design living in someone
   else's editor. Worth evaluating before the V2 board is routed, not after.
3. **Pin the parts.** Replace value-specified generic passives with explicit
   parts so the picker never needs the API. Removes the dependency entirely and
   gives up the value-driven picking that is most of atopile's appeal. Several
   hundred passives, so this is a real cost, but it is the only route to a fully
   offline, reproducible build.

No decision is recorded here because it is not a tooling decision. It should be
taken together with the V2 board migration, since that rewrites
`hardware/elec/src` anyway — and doing both at once is cheaper than doing them a
year apart.

## Seeing the circuit

`tools/ato-diagram.py` renders the design from the **sources** — board block
diagram, per-module internals, passives and design rules — into
`hardware/elec/CIRCUIT.md`, with `--html` for a standalone page.

It reads the `.ato` files rather than asking atopile because atopile's own views
(`pinout`, `power-tree`, `2d-image`, `glb`) are artifacts emitted *after* the
picker, so on an unauthenticated machine there is no way to get a picture out of
it at all. What the diagram gives up in exchange: it shows connections as
written, follows no solver unification, and knows nothing of footprints,
designators or placement. It is the structure, which is what a reviewer looks at.

## Placement, and locking it down

atopile 0.15.9 has **no in-language placement** — no `has_pcb_position`, no
layout trait, nothing in the stdlib. Placement lives in the `.kicad_pcb`, which
atopile modifies in place and preserves across builds.

That works, but it leaves the intent nowhere: a reviewer opening the PCB sees
coordinates and cannot tell which are deliberate. Worse, the constraints driving
those coordinates were already written as prose in the `.ato` sources — *"Keep
FMC traces short (< 10 mm)"*, *"Place antenna at board edge"* — where nothing
checked them.

So `hardware/elec/placement.yaml` states the floorplan: a zone per module, each
with a `why` traceable to the module's own comment, plus adjacency and
separation rules. `tools/ato-placement.py` checks it in two halves:

| half | status |
|---|---|
| spec against itself — zones inside the outline, no overlaps, every module covered, every rule satisfied, every zone justified | **runs now**, in CI and the pre-commit hook |
| spec against the PCB — each footprint inside its zone, and `locked` | waits for footprints, which wait for sign-in |

The constraints are settled; the coordinates are a first floorplan and will move
during layout. What should not move without a stated reason is the adjacency.

Verified by mutation: moving `memory` 15 mm from `mcu` trips the FMC rule,
overlapping two zones is caught, a zone crossing the board edge is caught, and
deleting a `why` fails.

### Three locks on the board

atopile has no netlist hand-off; the PCB *is* the output and every build rewrites
it. `hardware/ato.yaml`'s `default` build therefore sets:

- `keep_designators` — a renumbered board invalidates every assembly note.
- `keep_net_names` — renamed nets break netlist-based review.
- `keep_picked_parts` — **the one that costs money.** Without it a rebuild
  re-queries the parts service and can substitute a different manufacturer's
  equivalent 100 nF 0402, invisibly in a source diff, after the reel is bought.

`--frozen` is the fourth and cannot run yet: it compares a rebuilt PCB against
the committed one, and rebuilding needs the picker. It sits in the workflow
disabled with that reason attached, because the alternative is forgetting that
the one check protecting the layout is the one we cannot run.

## 3D rendering — and a correction

**An earlier version of this document said 3D was blocked on sign-in. That was
wrong, and the correction matters because it changes what is possible today.**

A part carrying the `has_part_picked` trait is pruned from the pick tree
entirely, so the picker never runs for it. Measured end to end on a two-part
board with no credentials:

```
✓ Picking parts [0.06s]   ✓ bom [0.06s]
✓ step [5.38s]            ✓ glb [4.14s]   ✓ 3d-image [17.44s]
```

So the real rule is: **3D needs vendored parts, not an account.** What was true
is narrower than what was written — *generic* components (`new Capacitor` with a
capacitance) need the picker, and this design is almost entirely generic
components, so nothing renders yet. The fix is vendoring, not authenticating.

Two constraints found by testing rather than reading:

- `supplier_id` must be the literal `"lcsc"`.
- `supplier_partno` must be **non-empty**. With a blank one every stage passes
  except `bom` — and the three 3D targets depend on `bom`, so the build stops
  before rendering. That is why `tools/ato-vendor-part.py` writes a placeholder
  and reports it rather than leaving the field empty.

### Rendering what exists now

`cargo xtask hardware render` drives `kicad-cli` against the committed PCB and
needs neither atopile nor a sign-in. It currently shows the outline and the
floorplan annotation, which is still the fastest way to check the board shape.

Deliberately not `ato build -t glb`: atopile's 3D targets sit after the picker,
so they need vendored parts even to render an outline.

**Version note.** atopile writes KiCad 9 format (`version 20241229`) and nixpkgs
ships KiCad 10.0.4. Exporting is safe — verified by hashing
`default.kicad_pcb` before and after `export glb` and `export svg`: unchanged.
Do not let KiCad 10 *write* the file; a format migration would break `--frozen`.

### Where 3D models come from

Downloaded from EasyEDA by `ato create part` and vendored into `parts/` as
`.step` files, referenced from the `.kicad_mod` as
`${KIPRJMOD}/../../parts/<Mfr_MPN>/<name>.step`. KiCad's own 3D library is never
used and `KICAD*_3DMODEL_DIR` is never read — the only variable that matters is
`KIPRJMOD`, which atopile passes itself.

A part with no model, or a `(model …)` pointing at a missing file, is a **silent
no-op**: the build succeeds and the component simply does not appear. Good for
robustness, bad for noticing — check the `(model …)` paths yourself.

## Routing

atopile does **no routing**, and this is by design rather than omission.

`update-pcb` writes footprints, `(net …)` declarations and net assignments on
pads. Verified by counting s-expressions in a built two-part board:
`footprint` 2, `net` 7, `property` 14, and `segment`/`via`/`zone`/`gr_line`
**zero**. The transformer has `insert_track`/`insert_via`/`insert_zone` methods
with no callers — dead code as far as `ato build` is concerned.

So: **route by hand in KiCad; atopile preserves it.** The build is incremental,
not regenerative — it parses the existing PCB, mutates it, writes it back.
Preserved across builds: segments, arcs, vias, zones, footprint positions and
rotations, non-atopile footprints, free graphics, groups.

Net classes, trace widths, differential-pair rules, length matching and custom
design rules are **not expressible in `.ato` at all**. `DifferentialPair` exists
but its only PCB-visible effect is `_P`/`_N` net-name suffixes;
`differential_impedance` is solver-only and constrains termination resistors, not
trace geometry. Declare all of it in KiCad — atopile neither reads nor writes
`.kicad_dru`, and no build step writes `.kicad_pro`.

There *is* one real routing feature: **layout reuse.** A module that some build
target's `entry` points at becomes reusable, and its instances get copied
tracks, arcs, vias and zones with net numbers remapped. It is keyed off
`ato.yaml` build entries rather than `.ato` syntax, it is undocumented upstream,
and `_calculate_group_offset` carries `# TODO rotation?` — **translation only, no
rotation**. Worth knowing before relying on it for the repeated channels in an
audio path.

An `atopile/autolayout/` package exists in the wheel and does AI place-and-route
against a remote service (DeepPCB via `gateway.atopile.io`), but it has **zero
callers in the CLI** — it is reachable only from `ato serve` and the hosted app.

## Running it

**Use the wrapper.** `tools/ato` installs atopile on first use and supplies
everything the invocation needs:

```sh
tools/ato --version
tools/ato auth login        # then the assert rules and DRC work
tools/ato build -b default -t bom
```

`cargo xtask hardware` falls back to it when `ato` is not on PATH, so the repo's
hardware checks need no shell setup.

It exists because four things conspire on NixOS, and each is individually
annoying:

1. atopile 0.15.9 requires Python **3.14 exactly** (`>=3.14,<3.15`); a system
   `python3` of 3.13 cannot install it.
2. `uv` resolves that by downloading its own CPython, which **does not run on
   NixOS** — "cannot run dynamically linked executables intended for generic
   linux environments". The interpreter must come from nixpkgs.
3. The wheel's compiled extension needs `libstdc++.so.6` on the loader path,
   which NixOS does not provide globally.
4. `uv tool install` puts binaries in `~/.local/bin`, which is not on PATH here,
   and the shell config is home-manager-managed in the read-only nix store — so
   "add it to `.zshrc`" is not a two-second fix.

The wrapper writes one directory,
`${XDG_CACHE_HOME:-~/.cache}/soul-listener/atopile` — a cache, not config, safe
to delete, and it reinstalls. Nothing touches PATH or shell files.

Doing it by hand instead:

```sh
# once — Python 3.14 is mandatory, and must come from nixpkgs on NixOS
nix shell nixpkgs#uv nixpkgs#python314 --command \
  uv tool install --python "$(command -v python3.14)" atopile==0.15.9

cargo xtask hardware check                       # every module, the board, the floorplan
cargo xtask hardware check-changed <files...>    # one slice (the pre-commit hook)
tools/ato-diagram.py --html /tmp/circuit.html    # see the circuit
tools/ato-placement.py                           # check the floorplan alone

ato auth login                                   # then the assert rules and DRC work
ato build -b default -t bom                      # a BOM, once signed in
```

and then `libstdc++` on the loader path for every invocation:

```sh
export LD_LIBRARY_PATH="$(nix eval --raw nixpkgs#stdenv.cc.cc.lib)/lib:$LD_LIBRARY_PATH"
```

**Signing in must be done from a terminal, not a browser.** `ato auth login`
starts the flow itself and captures the token on a localhost callback; signing
into `app.atopile.io` in a browser creates a *web* session and leaves
`ato auth status` reporting `Signed out`.

## One source of truth for pins

A component's pin assignment exists in exactly **one** authoritative place: the
`.ato` definition, written by a person against a datasheet.

```
hardware/elec/src/**/*.ato          <- written by hand, the source of truth
        |  tools/ato-pinmap.py
        +--> hardware/elec/pinmap.json      for tools and review
        +--> dap/hw-spec/src/pinmap.rs      no_std const table, for firmware
```

The firmware never declares a pin. It reads `dap_hw_spec::pinmap`, and two gates
keep the direction fixed:

- `the_firmware_core_names_no_pin_or_peripheral` — `dap-core` may not name a pin
  at all; pins are board facts.
- `the_board_takes_its_pins_from_the_generated_map` — `board-h7a3` *may* name
  pins, since choosing them is its job, but only via `pinmap`.

**Why the generated Rust is committed rather than built.** A pin change should
appear in review: a committed table puts `("CS_NEG", "1")` into the diff, where
a reviewer with the datasheet open can catch a transposition. Pin errors are the
most expensive class of board defect and the cheapest to catch by reading. A
build-time generator would hide exactly that. Two lesser reasons: `dap-hw-spec`
is `no_std` and dependency-free, so parsing at build time would mean a `build.rs`
and a parser dependency in the crate the whole firmware pins itself against; and
the arch tests can compare a committed file but not one that exists only inside
a build directory.

`the_pinmap_is_current` runs the generator with `--check` rather than
re-implementing the parse in Rust — a second parser would be a second opinion
about what the sources say, and the two would disagree eventually.

Component *identity* flows the same way and is checked the same way:
`Preference::Preferred` in the spec → `selected.json` → the vendored part's
`is_atomic_part` trait, with `vendored_parts_match_the_specs_picks` asserting a
vendored part never outlives the pick it was generated from. That identity is
what reaches the netlist, the BOM and the assembly house.

Mutation-tested, all three caught: changing a pin in a `.ato` without
regenerating, a vendored part claiming a manufacturer the spec disagrees with,
and a hand-written pin constant in the board crate.

## Gates that keep this honest

In `dap/arch-tests/tests/hardware_slices.rs`, all text assertions over committed
files so they run without the toolchain:

- `every_ato_module_has_a_check_build` — a slice with no `check-*` build is one
  the hook and CI silently skip.
- `no_two_builds_share_an_entry_point` — the 0.15.x config error, caught at
  commit rather than by whoever next has the toolchain installed.
- `no_deprecated_atopile_config_keys` — `__default__`, `ato-version`,
  `component_lib`, and the pin itself being 0.15.8+.
- `the_hardware_check_is_wired_into_the_hook_and_ci` — the hook and workflow
  actually invoke the check.
- `the_pinmap_is_current` — the generated pin map matches the `.ato` sources.
- `the_board_takes_its_pins_from_the_generated_map` — no hand-written pin
  constants in the board crate.
- `vendored_parts_match_the_specs_picks` — a vendored part's identity still
  agrees with the spec's preference.
- `board_outline_matches_the_floorplan` and
  `the_outline_is_owned_by_the_design_not_the_tool` — the outline is declared in
  `main.ato` and the two size numbers agree.
- `every_module_has_a_placement_zone` and `the_pcb_is_locked_against_rebuild_drift`.
