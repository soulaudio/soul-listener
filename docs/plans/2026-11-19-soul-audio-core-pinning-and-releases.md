# soul-audio-core pinning, releases & freezing — decision + migration plan

> Decision record for how the firmware (`soulaudio/soul-listener`, the `dap/`
> workspace) depends on the co-developed library (`soulaudio/soul-audio-core`):
> how builds are frozen/reproducible, how releases are cut, and how local
> side-by-side editing is preserved. Supersedes the `.github/soul-audio-core.pin`
> + composite-checkout scheme.

## Current scheme (what we're replacing)

- `dap/Cargo.toml` uses a **path dependency**: `soul-audio-core = { path = "../../soul-audio-core/crates/soul-audio-core", features=["embedded-profile"] }` (+ `-engine`, `-index`). Great for local co-dev, but Cargo.lock does **not** lock a path dep to a revision.
- Reproducible CI is bolted on separately: `.github/soul-audio-core.pin` holds a 40-char SHA; a composite action (`.github/actions/checkout-soul-audio-core`) checks the sibling out at that SHA; `tools/soul-audio-core-pin.sh` bumps it; a non-blocking job warns when the pin falls behind `main`.
- Problems: the pin points at **arbitrary commits, not releases**; the whole machinery (pin file, bump script, checkout action, drift job) is bespoke and non-idiomatic; nothing gives release discipline.

## Decision

**Migrate to a Cargo-native git dependency pinned by immutable `rev`, frozen by a committed `Cargo.lock` + `--locked`, with a gitignored `[patch]`→path for side-by-side editing — and give `soul-audio-core` real tagged releases.** The `rev = "…"  # vX.Y.Z` comment + `Cargo.lock` *is* the pin record; the separate `.pin` file becomes unnecessary.

Rationale (full comparison table in the research that produced this doc): git-dep-by-rev is the only option that is simultaneously (a) natively reproducible (lockfile records + `--locked` enforces + Cargo hash-checks the fetch), (b) release-disciplined when paired with tags, and (c) still one-command local co-dev via `[patch]` — while deleting the entire bespoke pin/checkout surface. Submodules are redundant with Cargo.lock; publishing to crates.io is premature pre-1.0 (6 interdependent crates, forced API stability); `cargo vendor` is hostile to daily co-dev (keep it only as a release-time archival artifact).

## Mechanics

### A. `dap/Cargo.toml` (committed) — git dep by rev, one source for all core crates
```toml
[workspace.dependencies]
# Pinned to the immutable commit tag vX.Y.Z resolves to (tag in the comment for humans).
soul-audio-core   = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<40hex>", default-features = false, features = ["embedded-profile"] }  # vX.Y.Z
soul-audio-engine = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<40hex>" }  # vX.Y.Z
soul-audio-index  = { git = "https://github.com/soulaudio/soul-audio-core", rev = "<40hex>" }  # vX.Y.Z

# keep the perf overrides
[profile.dev.package.soul-audio-core]  { opt-level = 3 }
[profile.test.package.soul-audio-core] { opt-level = 3 }
```
**Commit `dap/Cargo.lock`** — it records `source = "git+https://github.com/soulaudio/soul-audio-core?rev=<sha>#<sha>"`, freezing the build.

### B. Local co-dev — gitignored override (never committed)
`dap/.cargo/config.toml` (add to `.gitignore`; commit a `config.toml.example`):
```toml
[patch."https://github.com/soulaudio/soul-audio-core"]
soul-audio-core   = { path = "../../soul-audio-core/crates/soul-audio-core" }
soul-audio-engine = { path = "../../soul-audio-core/crates/soul-audio-engine" }
soul-audio-index  = { path = "../../soul-audio-core/crates/soul-audio-index" }
```
`[patch]` is read from the workspace-root manifest OR `.cargo/config.toml`; it redirects all crates from that git source to the editable sibling, so you keep editing `../soul-audio-core` with zero publish. (Do **not** use the weaker `paths = […]` config form — it only works for registry crates.)

### C. CI reproducibility — replaces the composite checkout action
Firmware CI just runs `cargo … --locked`. Cargo fetches the pinned `rev` itself and refuses lockfile drift. No `.pin`, no sibling-checkout step, nothing to keep in sync. A moved upstream tag can't change the build (bound to the SHA).

### D. Bumping core — replaces `tools/soul-audio-core-pin.sh`
`tools/bump-core.sh <tag>`: resolve the tag to its SHA (`git ls-remote --tags <url> <tag>`), rewrite the `rev = …  # <tag>` lines, `cargo update --locked`, open a PR. For automated bump PRs use **Renovate** (updates Cargo git deps by tag/rev; Dependabot's git-dep support is weaker). This replaces the "pin behind" advisory job with an actionable PR.

## soul-audio-core side (releases)

- Set `repository` to the GitHub URL (currently `"local"`).
- Adopt **release-plz** (CI Release-PR: semver bump + CHANGELOG via git-cliff + **cargo-semver-checks** — the guard that catches an API break before it reaches firmware). It cuts annotated `vX.Y.Z` tags; the existing `cd-release.yml` (`on: push tags v*`) stays as the artifact/SBOM half.
- Enable **GitHub Immutable Releases** + a `v*` tag ruleset ("Restrict updates") so tags/assets can't be force-moved.
- Single shared workspace version (all core crates move together → one pinned rev covers the set, no version skew). Pre-1.0 semver: `0.x.(y+1)` for breaking.

## Migration plan

1. **core:** fix `repository`; add CHANGELOG + release-plz; cut the first real tag; enable immutable releases + `v*` ruleset.
2. **firmware:** replace the three `path = …` deps with `{ git = …, rev = <sha-of-tag> }  # vX.Y.Z`; keep the `[profile.*.package.soul-audio-core]` overrides.
3. generate + commit `dap/Cargo.lock`; verify the git source line shows the SHA.
4. add `dap/.cargo/config.toml` `[patch]` block; gitignore it; commit `config.toml.example`.
5. **delete** `.github/soul-audio-core.pin`, `.github/actions/checkout-soul-audio-core`, `tools/soul-audio-core-pin.sh`, the pin-drift job; switch firmware CI to `--locked`.
6. replace the drift-warn job with Renovate (or `tools/bump-core.sh`).
7. at each firmware **release**, archive `cargo vendor` output for offline rebuildability insurance.

## Note on sequencing vs the open-transient finding

The QEMU Phase-A1 measurement (`tools/mcu-stackprobe/check-stack-qemu.sh`) showed the worst-case decoder-open transient is ~1 MB — a soul-audio-core change is needed to reduce it. That work is compatible with this scheme: do it on the sibling (local `[patch]` picks it up immediately), then cut a new core tag + bump the firmware `rev` when it lands. If anything, it's a good first exercise of the new release flow.
