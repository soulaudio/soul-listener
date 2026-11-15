# soul-listener — Logic Layer + Headless Hardware Simulation — Design Spec

**Date:** 2026-11-16
**Status:** Draft for review
**Builds on:** `docs/plans/2026-11-15-v2-migration-design.md` (reprioritizes it: logic + simulation first; concrete hardware adapters and UI later).

## 1. Direction (what changed)

- **Use `soul-audio-core` fully; delete v1's own audio** (`playback`, `library`, `nanomp3`, `rubato`, old `firmware/audio/*`). soul-audio-core is the only audio brain. *(Proven: it compiles+links for the H7A3 — commit `59091c8`.)*
- **Logic-first, simulation-first.** Build the board-agnostic logic (audio orchestration + battery/power) and a **headless hardware-simulation harness** that runs and tests the whole device with **zero hardware**. Defer the **UI** and all **concrete hardware adapters** (SAI/CS43131/SD/MT6701/BQ21080).
- **Swappable audio output.** The logic depends only on the `Sink` *port*, never on SAI. Switching the audio output (SAI → USB-audio → a different DAC) is a new `impl Sink` + one line in the composition root — not a rewrite.

## 2. Architecture — hexagonal (ports & adapters)

The logic (hexagon interior) depends only on ports (traits). Adapters implement them. We build the **simulation** adapters now (host) and the **hardware** adapters later; the interior never changes between them.

```
dap-core (interior — board-agnostic, no embassy, host-runnable, no_std-capable)
  app     : the DAP coordinator (transport + power state; UI added later)
  audio   : assembles soul-audio-core Engine/Deck/control/queue over the Sink + Storage ports
  power   : the battery/power service (new logic — see §4)
  ports   : Sink (re-exported from soul-audio-core), Storage, Power, Controls  (Display added in the UI phase)
        │
   ┌────┴─────────────────────────────┐              ┌───────────────────────────────┐
   │ dap-sim (host/std) — adapters +   │   LATER      │ board-h7a3 (embassy) — adapters│
   │ the "virtual DAP" harness:        │  ─────────▶  │ SAI/USB Sink, CS43131, SDMMC,  │
   │  SimSink (analysis/file),         │              │ MT6701 Controls, BQ21080/      │
   │  SimStorage (files + .soulidx),   │              │ MAX17048 Power                 │
   │  SimBattery (scriptable),         │              │ (built once hardware exists)   │
   │  SimControls (scripted)           │              └───────────────────────────────┘
   └───────────────────────────────────┘
```

**Rule:** `dap-core` and `dap-sim` are **embassy-free and host-runnable**. `embassy` appears only in `board-h7a3`. Ports are generic trait bounds (monomorphized — no `dyn`, no `alloc` in `dap-core`), preserving soul-audio-core's invariants.

## 3. Audio logic — assembly, not reinvention

soul-audio-core already ships the audio "best practices": gapless (`Deck`), resampling, normalization, volume, the `Engine` queue (next/prev/repeat), `soul-audio-control` (transport facade, prev-3s), and `soul-audio-service` (a *"headless, device-free, sink-generic AudioPlayer actor"*). `dap-core::audio` **wires these** over the `Sink` + `Storage` ports. No decoder/resampler/queue logic is written here.

## 4. Power logic — the genuinely-new board-agnostic service (`dap-core::power`)

Behind a `Power`/`BatteryMonitor` port (the IC driver is a later adapter). Scope (full):

- **State:** smoothed SoC %, charge/discharge/charging/full state, voltage/current/temperature readings (as the port provides them), runtime estimation (time-to-empty / time-to-full).
- **Safety behaviors:** low-battery warning threshold → **auto-pause** + notify; critical threshold → **safe shutdown** (flush, persist, power-off intent).
- **Charge/thermal management:** charge-current intent, temperature-aware charging (reduce/stop charge outside a safe band), battery-health / cycle-count tracking.
- **Power profiles & sleep:** `Performance` / `BatterySaver` profiles (emit a clock-scaling *intent* the board adapter honors), idle-timeout → sleep, sleep/wake transitions.

All of this is a **state machine over port inputs emitting intents/events** — fully testable on the host via `SimBattery` (see §5). The board adapter later maps intents onto real silicon (clock mux, charger IC, power rails).

## 5. Headless simulation — the "virtual DAP" (`dap-sim`)

A host harness composing **all** sim adapters + `dap-core`, so the entire device runs and is tested with no hardware:

- **`SimSink`** — implements `soul_audio_core::Sink`; records/analyzes the PCM it receives (frame counts, rate, continuity for gapless, FFT for pitch/level — mirroring soul-audio-core's own `render`/e2e checks), or writes a file. This is also the first real `Sink` the swap-test validates.
- **`SimStorage`** — a file-backed `Storage`/`Source` serving real test audio (reuse `soul-audio-core/testdata/`) + a baked `.soulidx`, so the real catalog/index + decode path run.
- **`SimBattery`** — a scriptable `Power` source: drive SoC/voltage/temperature/charge over a timeline (discharge curve, plug/unplug, overheat) and assert the service's reactions (auto-pause at low, shutdown at critical, profile/sleep transitions).
- **`SimControls`** — scripted `Controls` events (wheel ticks, press, long-press) driving transport.
- **Harness:** a host binary + `cargo test` suite that runs scenarios end-to-end: *play a gapless queue through SimSink while SimBattery discharges to critical and asserts a clean auto-pause→shutdown*, etc. (Display sim = the existing v1 `eink-emulator`, wired in during the UI phase.)

This delivers your "complete headless, simulate as much hardware as possible" goal: audio, storage, power, and input are all simulated; only the real ICs are absent, and they slot in behind the same ports.

## 6. Workspace structure

The new code is the Embassy-0.6 / soul-audio-core world (already a separate workspace from the Embassy-0.1 old crates — the `links` collision). Promote it to a clean multi-crate workspace:

```
dap/                         # v2 workspace root (Embassy-0.6 + soul-audio-core)
├── Cargo.toml               # [workspace] members = core, sim, board-h7a3 ; soul-audio-core dep + profile override
├── core/                    # dap-core  (ports + audio + power + app) — no embassy, host + target
├── sim/                     # dap-sim   (sim adapters + virtual-DAP harness) — host/std
└── board-h7a3/              # the hardware bin (moved from crates/board-h7a3) — embassy; hardware adapters later
```

`board-h7a3` moves from `crates/board-h7a3` → `dap/board-h7a3` (small; 3 files + its own config). The old Embassy-0.1 root workspace keeps shrinking as §7 deletes crates; the two unify once `platform`'s Embassy dep is gone.

## 7. Rip out v1 audio

Delete `crates/{playback,library}` and the audio deps (`nanomp3`, `rubato`); remove `firmware/audio/*` and `platform/audio*`; rewire/retire `firmware-ui`'s dependence on `library`/`playback` (its screens are salvaged later in the UI phase). CI jobs that build these are removed with them (part of the tracked CI migration).

## 8. Testing

Everything is `cargo test` on the host via `dap-sim`: decode/gapless/resample/volume/queue through `SimSink`; the full power state machine through `SimBattery`; transport through `SimControls`. No hardware, no board. `board-h7a3` stays compile-only (cross-build gate) until real silicon exists.

## 9. Phasing

1. **Scaffold `dap/` workspace + `dap-core` ports** (Sink/Storage/Power/Controls) + the audio assembly over soul-audio-core; **`dap-sim` SimStorage + SimSink + harness** proving a gapless/resample/volume queue headless.
2. **Power service** (`dap-core::power`, full scope §4) + **SimBattery**; harness proves low-batt→pause, critical→shutdown, profiles/sleep.
3. **SimControls + transport** wired through the app; harness drives play/next/prev/seek via scripted input.
4. **Rip out v1 audio** (§7).
5. *(Later)* UI phase (eink-emulator + screens); hardware adapters (SAI/CS43131/SD/MT6701/BQ21080) on real silicon.

## 10. Open / deferred

- Concrete hardware adapters + on-silicon bring-up — deferred (no board).
- UI — deferred to its own phase (salvages v1 `ui`/`eink`/`firmware-ui`).
- `Clock`/RTC and settings-persistence ports — add when the app logic first needs them (YAGNI).
