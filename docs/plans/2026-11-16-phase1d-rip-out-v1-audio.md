# Phase 1d — Rip out v1 audio — Implementation Plan

> REQUIRED SUB-SKILL: superpowers:subagent-driven-development. Steps use `- [ ]`.
> Spec: `docs/plans/2026-11-16-logic-and-headless-simulation-design.md` §7. Final Phase-1 step.

**Goal:** Delete v1's superseded audio (`playback`, `library` crates; `nanomp3`, `rubato` deps; the dead `firmware/audio` + `platform/audio` code) now that soul-audio-core + `dap-core` fully replace them — leaving the root workspace building its kept salvage crates (`ui`, `eink/*`, `platform`-minus-audio) and `dap/` untouched.

**Architecture:** `playback`/`library` are leaf crates (verified: nothing depends on them). Delete them cleanly. `firmware` + `platform` hold dead audio modules; remove those modules where the crate still compiles, else quarantine the crate (keep its dir for salvage in the UI/hardware phases). Update CI + arch-boundary tests that referenced the removed crates.

**Tech Stack:** Rust workspace edits. No new code.

## Global Constraints

- Do NOT touch `dap/` — it must stay green (`cd dap && cargo test` = 23 tests).
- Keep the UI-salvage crates (`ui`, `eink/*`, `platform`'s display/input/power/storage, `firmware-ui` if it still builds) — they're reused in the later UI phase.
- Commits `sebastianstupak <sebastian.stupak@pm.me>`, no AI trailers. Branch `migration/phase-1d`.

---

### Task 1: Delete `playback`/`library`, prune deps, fix references

**Files:** Delete `crates/playback/`, `crates/library/`; modify root `Cargo.toml`; possibly `.github/workflows/ci.yml`, arch-boundary test(s), `firmware/`, `platform/`.

- [ ] **Step 1 (confirm leaves):** `grep -rn --include=Cargo.toml -E '"?(playback|library)"?\s*=\s*\{?\s*(path|workspace)' crates` → expect NO hits outside `playback`/`library` themselves (nothing depends on them). If something unexpectedly does, STOP and report.
- [ ] **Step 2:** `git rm -r crates/playback crates/library`. Remove their two lines from root `Cargo.toml` `members`. Remove `nanomp3 = "0.1"` and `rubato = "0.15"` from `[workspace.dependencies]` (verified unused elsewhere).
- [ ] **Step 3 (arch tests + CI):** find the architecture-boundary test (e.g. `grep -rln 'playback\|library' crates --include=*.rs` and any `arch_boundaries`/`architecture` test) and remove its `playback`/`library` slice assertions so it compiles. In `.github/workflows/ci.yml`, remove any job/step that builds or tests `playback`/`library` specifically (leave the dap + salvage jobs).
- [ ] **Step 4 (dead audio modules — best-effort, don't cascade):** remove `firmware/src/audio/` and `platform/src/audio*.rs` (+ the audio-silicon drivers `es9038q2m.rs`, `tpa6120a2*`) **only if** the crate still compiles afterward (fix the now-dangling `mod`/`use` lines). If removing them cascades into large edits, instead **quarantine the whole `firmware` crate** (comment it out of `members`, keep the dir — it holds the SSD1677 driver + input, salvaged in the UI/hardware phases) and leave `platform`'s audio in place (dead but compiling). Record which path you took and why.
- [ ] **Step 5 (verify):**
  - From repo root: `cargo build` → the kept root-workspace members build. (If `firmware`/`firmware-ui` were quarantined, note it.)
  - `cd dap && cargo test` → still 23 green (untouched).
  - `grep -rn 'playback\|library\|nanomp3\|rubato' Cargo.toml` → no stale refs.
- [ ] **Step 6:** Commit `refactor: rip out v1 audio (delete playback/library, drop nanomp3/rubato)` (+ note any quarantine in the message body).

---

## Deliverable

v1's audio implementations are gone; soul-audio-core (+ `dap-core`) is the sole audio brain. The root workspace builds its remaining salvage crates; `dap/` is untouched and green. **Phase 1 is complete** — the headless logic + simulation goal (audio + power + controls, all testable with zero hardware, swappable sink) is fully delivered. Remaining work (separate future efforts): the UI phase (salvage `ui`/`eink`/`firmware-ui`, lift `platform`'s display/input ports into `dap-core`) and hardware adapters (SAI/CS43131/SD/MT6701/BQ21080) on real silicon.
