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

## Running it

```sh
# once — Python 3.14 is mandatory
uv tool install --python 3.14 atopile==0.15.9

cargo xtask hardware check                       # every module, then the board
cargo xtask hardware check-changed <files...>    # one slice (the pre-commit hook)

ato auth login                                   # then the assert rules and DRC work
ato build -b default -t bom                      # a BOM, once signed in
```

On NixOS the wheel needs `libstdc++` on the loader path:

```sh
nix shell nixpkgs#stdenv.cc.cc.lib --command bash -c '
  export LD_LIBRARY_PATH="$(dirname "$(find /nix/store -maxdepth 4 -name libstdc++.so.6 | head -1)"):$LD_LIBRARY_PATH"
  cargo xtask hardware check'
```

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
