# soul-listener v2 Migration — Design Spec

**Date:** 2026-11-15
**Status:** Draft for review
**Supersedes (in part):** `docs/AUDIO.md`, `docs/ABSTRACTIONS.md` audio sections (v1, Feb 2026).

## 1. Context

This repo is now the **canonical product monorepo** (the Forgejo mirror is archived
and points here; history was normalized to a single `pm.me` identity with AI
attribution stripped). v1 (Feb 2026) was an all-in-one DAP: in-repo audio
(`playback` + `library` + `nanomp3` + `rubato`), `platform`/`firmware` on
**STM32H743 + external SDRAM + ES9038Q2M/TPA6120 + EC11 buttons**, atopile
`hardware/`, and a stub `mechanical/`.

v2 **decomposes**: the audio engine now lives in the separate, reusable
**`soul-audio-core`** (Apache-2.0); this repo *consumes* it. Hardware choices
evolved, and the FreeCAD enclosure + BOM/SBOM migrate in from the archived mirror.

## 2. Goals

- Replace **all** in-repo audio with `soul-audio-core`, wired through hexagonal
  ports (`Sink`/`Source`), **SAI/I²S** output adapter for now.
- **Salvage the eink/display/UI stack** as much as possible (modernize in place).
- Retarget **H743 → H7A3**; add the MT6701 scroll-wheel + v2 power adapters.
- Consolidate **mechanical** (FreeCAD enclosure) + BOM/SBOM into this repo; **keep
  atopile** for electronics (update for v2 parts).
- Generate a real **SBOM** now that the stack is settled.

## 3. Non-goals

- No restructuring of `soul-audio-core` — consumed as a dependency; small upstream
  PRs only if the firmware genuinely needs them (e.g. per-codec feature gates).
- **USB-DAC** and **Bluetooth** (v1's WB55) — deferred to a later revision.
- **exFAT** — FAT32 at launch (`embedded-sdmmc`).

## 4. Hardware deltas (v1 → v2)

| Subsystem | v1 | v2 |
|---|---|---|
| MCU | STM32H743ZI + external SDRAM | **STM32H7A3** (no external SDRAM, ~1.4 MB SRAM) |
| DAC / amp | ES9038Q2M + TPA6120A2 | **CS43131** (integrated), SAI **slave**, external MCLK oscillators |
| Display | GDEM0397T81P (SSD1677) | **GDEY0397T81P** — interchangeable twin, same SSD1677 |
| Input | EC11 rotary + 4 buttons | **MT6701 magnetic scroll-wheel** (twist + press) |
| Power | BQ25895 | **BQ21080 charger + MAX17048 gauge** |
| Electronics | atopile (`hardware/`) | **atopile (keep)**, updated for v2 parts |
| Audio engine | in-repo `playback`/`library` | **`soul-audio-core`** (external dep) |

## 5. Architecture — hexagonal (ports & adapters)

Reuse `soul-audio-core`'s audio ports (`Sink`, `Source`/`SeekSource`), its `Deck`
(gapless) + `Engine` (queue) + catalog/`.soulidx` reader. Extend the same pattern
to the device shell, lifting v1 `platform`'s trait *shapes* into a clean ports
layer:

- **Device ports** (board-agnostic): `Display` (= `embedded-graphics` `DrawTarget`
  + an e-ink refresh trait), `Controls`, `Power`, `Storage`.
- **Board adapters** (`board-h7a3`): SAI-I²S `Sink`, SD `SeekSource`, CS43131
  codec-control, MT6701 `Controls`, BQ21080/MAX17048 `Power`, SSD1677 `Display`.
- **Rule:** `embassy` is confined to the board layer; everything above is expressed
  in `embedded-hal` / `embedded-graphics` / `embedded-io` traits (ports stay
  generic/monomorphized — no `dyn`, no `alloc`).

## 6. Crate plan (evolve the v1 workspace)

| Action | Crates |
|---|---|
| **KEEP + modernize** | `eink-specs`/`-system`/`-components`/`-emulator`/`-testing`, `ui`, `firmware-ui`, the SSD1677 driver |
| **LIFT trait shapes → ports** | `platform` (`Display`/`Storage`/`Input`/`Power` trait defs); drop the embassy-bound wrapper + v1 adapters |
| **REWRITE** | `firmware` → `board-h7a3` (H7A3 feature, adapters, composition root) |
| **DELETE** | `playback`, `library`, `nanomp3`, `rubato`, `platform/audio*`, `firmware/audio/*` (ES9038Q2M, TPA6120, SAI-master) |
| **ADD** | `soul-audio-core` deps; a board-agnostic `dap` app layer; the wheel-driven browse UI |

## 7. Audio migration (remove the other audio solutions)

- **Remove:** `nanomp3` (MP3), `rubato` (resampler), the `playback` crate (decode
  glue + `PlaybackEngine` + ring buffer), the `library` crate (`.soul` index + FAT32
  scan), `platform/audio*`, `firmware/audio/*`.
- **Replace with:** `soul-audio-core` `Deck` + `Engine` + decoders + resampler +
  catalog/`.soulidx` reader.
- **Output:** a **SAI/I²S `Sink` adapter** (kept for now; swappable later — USB
  audio, a different I²S sink — because it's behind the port).
- Rewire `firmware-ui`'s now-playing to source track metadata from
  `soul-audio-core`'s catalog types (it currently borrows `library`/`playback`).
- **Prove it headless** via a `render`-style host test (decode → `Deck` → file
  sink; no DAC needed), mirroring `soul-audio-core`'s own e2e.

## 8. eink salvage (priority)

Keep the whole subsystem. The SSD1677 driver is already generic over
`embedded-hal-async` `SpiDevice`/`OutputPin` + `DrawTarget` and host-tested — port
= re-supply H7A3 SPI/GPIO pins at the board layer. Net work:

1. Add the **`GDEY0397T81P` alias** (same panel/controller as `GDEM0397T81P`).
2. Implement **4-gray** — the real gap: silicon path is **1bpp** today, though
   `platform`/emulator/`firmware-ui` already assume 2bpp/Gray4. Write the dual-plane
   + LUT encoding.
3. **H7A3 SPI/DMA pins**; finish **dirty-rect partial refresh** (RAM-window plumbing
   exists; refresh is whole-screen today) for snappy wheel-scrolling.
4. The **emulator + golden-image tests** carry over → develop/regress the UI with no
   hardware.

## 9. Input — scroll-wheel

v1 already modeled a rotary encoder (`InputEvent::RotaryIncrement`) and the emulator
already emulates a scroll wheel; `ui::Navigator` is input-model-agnostic. MT6701 maps
directly: **twist → `RotaryIncrement`, press → `Select`**. Net-new: the wheel-driven
**browse/list UI** (v1 only shipped now-playing + a static demo menu) and the
~40-line `InputEvent → nav` mapping.

## 10. Dependency reconciliation (Phase-0 risk)

v1 pins **ancient Embassy** (`embassy-stm32 =0.1.0`, `executor =0.6.3`,
`time =0.3.2`); `soul-audio-core` is on `embassy-stm32 0.6.0` / `executor 0.10` /
`time 0.5`. The bump is large, but lands on the **board/`firmware` crate we rewrite
anyway**; the salvaged **eink/ui crates are Embassy-free** and ride through. Add
`soul-audio-core` as a **git-pinned `rev` + local `path` override** for co-dev, plus
its `opt-level = 3` profile override (required, or debug builds decode slower than
real time).

## 11. BOM / SBOM

- **BOM:** migrate the Forgejo `BOM.md`, reconcile with v1's `hardware/bom`, update
  to v2 parts (H7A3 / CS43131 / GDEY0397T81P / MT6701 / BQ21080 / MAX17048 / microSD
  / USB-C / 3.5 mm).
- **SBOM:** generate a real firmware SBOM (**CycloneDX** via `cargo-cyclonedx`) now
  that firmware exists; wire into CI next to the existing `supply-chain/`.

## 12. Mechanical consolidation

Bring the FreeCAD enclosure (`soul-listener.FCStd`, `enclosure/` parts + build
scripts, `DISPLAY-IMPORT.md`) from the archived mirror into `mechanical/` (replacing
the stub), plus `BOM.md` / `SBOM.md`. **Files, not history.**

## 13. Phasing

| Phase | Work |
|---|---|
| **0** | Setup & dep reconcile: Embassy bump + `soul-audio-core` dep + CI cross-build/host-test gates |
| **1** | Audio rip-and-replace (hexagonal, SAI/I²S `Sink`); headless render test |
| **2** | eink salvage/modernize (4-gray, H7A3 pins, dirty-rect) |
| **3** | Board retarget (H7A3, MT6701 `Controls`, BQ21080/MAX17048 `Power`, CS43131) |
| **4** | `dap` app + wheel-driven browse UI |
| **5** | BOM/SBOM + mechanical consolidation |

## 14. Open decisions / risks

- **License:** v1 is **AGPL-3.0**; as sole author you may relicense v2 freely.
  Recommend keeping **AGPL for the product** (permissive engine + copyleft product).
  Decide before a public release.
- **4-gray on SSD1677** — net-new, the biggest display unknown.
- **CS43131 + SAI-slave + external oscillators** — new, pending the v2 PCB.
- **Embassy 0.1 → 0.6** API churn on the board layer.
