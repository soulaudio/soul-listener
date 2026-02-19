# Hardware Design — SoulAudio DAP

## Overview

The SoulAudio DAP hardware is written in **[Atopile](https://atopile.io) v0.12.5+**,
a code-first PCB DSL. `.ato` files compile to KiCad projects and run
design-rule assertions during CI — no manual ERC click-through required.

The design is organized as vertical slices mirroring the firmware crate
structure. See `CLAUDE.md` for the full module-to-firmware mapping.

## Quick Start

```bash
# Install Atopile CLI (requires Python 3.11–3.13)
pip install atopile==0.12.5

# Install locked dependencies
cd hardware
ato sync

# Validate design (ERC-equivalent assert checks, no KiCad needed)
ato check

# Full build → generates KiCad project in build/ (requires KiCad 8)
ato build
```

## Structure

```
hardware/
├── ato.yaml                    # Project manifest (entry: main.ato:SoulAudioDAP)
├── ato-lock.yaml               # Committed dependency lock file
├── CLAUDE.md                   # Vertical slice guide, design rules, TODOs
├── README.md                   # This file
├── elec/
│   ├── src/
│   │   ├── interfaces.ato      # Shared bus/port definitions
│   │   ├── main.ato            # Top-level board (wires all modules)
│   │   ├── mcu.ato             # STM32H743ZIT6 + crystals + decoupling
│   │   ├── audio.ato           # ES9038Q2M + TPA6120A2 + LT3042 AVDD
│   │   ├── power.ato           # BQ25895 PMIC + AP2112K-3.3 LDO
│   │   ├── display.ato         # GDEM0397T81P FPC + decoupling
│   │   ├── bluetooth.ato       # STM32WB55RGV6 BLE co-processor
│   │   ├── memory.ato          # W9825G6KH-6 SDRAM + W25Q128JVSIQ NOR flash
│   │   ├── input.ato           # EC11 encoder + 5 tactile buttons
│   │   └── parts/              # Atomic component footprint stubs
│   └── layouts/default/        # KiCad board layout (generated)
└── bom/sourcing/               # Per-distributor BOM CSVs
    ├── tme.csv                 # TME (CZ/EU) — primary European source
    ├── digikey-eu.csv          # Digi-Key EU (NL warehouse)
    └── mouser.csv              # Mouser EU (DE warehouse)
```

## System Architecture

```
USB-C (5 V) ──→ BQ25895 PMIC ──→ SYS rail (≈3.0–4.2 V)
                    │                      │
              LiPo battery        AP2112K-3.3 LDO
                                           │
                                   3.3 V digital rail
                                    │   │   │   │   │
                                   MCU Disp BLE Mem Input

STM32H743 (480 MHz Cortex-M7)
 ├── SAI1 I²S ──────────────────→ ES9038Q2M DAC → TPA6120A2 amp → 3.5 mm jack
 ├── SPI1 + DC/RST/BUSY ─────→ GDEM0397T81P (3.97" 800×480 e-ink)
 ├── USART1 HCI ──────────────→ STM32WB55RGV6 (BLE 5.0 / LE Audio co-proc)
 ├── FMC (16-bit) ────────────→ W9825G6KH-6 SDRAM (32 MB, 0xC000_0000)
 ├── QUADSPI ─────────────────→ W25Q128JVSIQ NOR flash (16 MB XiP, 0x9000_0000)
 ├── SDMMC1 ──────────────────→ microSD (FAT32)
 ├── I²C1 ────────────────────→ ES9038Q2M config (0x48) + BQ25895 (0x6A)
 ├── USB OTG HS ──────────────→ USB-C (UAC2 audio + charging)
 └── GPIOs ───────────────────→ EC11 encoder + 5 buttons
```

## Key Components

| Module     | Part               | Package    | Function                                    |
|------------|--------------------|------------|---------------------------------------------|
| MCU        | STM32H743ZIT6      | LQFP-144   | Main processor 480 MHz, 2 MB Flash, 1 MB RAM |
| DAC        | ES9038Q2M          | TSSOP-28   | 32-bit/768 kHz, 128 dB DNR, −120 dB THD+N  |
| Amp        | TPA6120A2          | MSOP-8 PP  | Class-AB headphone amp, 250 mA (needs ±5 V) |
| Analog LDO | LT3042EMSE         | MSOP-8E    | Ultra-low-noise 3.3 V AVDD for ES9038Q2M    |
| PMIC       | BQ25895RTWT        | WQFN-24    | USB-C BC1.2 + LiPo charge + power path      |
| BLE        | STM32WB55RGV6      | VFQFPN-68  | BLE 5.0 / LE Audio, HCI UART to H743        |
| SDRAM      | W9825G6KH-6        | TSOP-II-54 | 32 MB FMC 16-bit, library/art/decode cache  |
| NOR Flash  | W25Q128JVSIQ       | SOIC-8     | 16 MB QSPI, XiP fonts/icons/LUTs/OTA        |
| Display    | GDEM0397T81P       | Module     | 3.97" 800×480 235 PPI e-ink, SSD1677        |
| 3.3 V LDO  | AP2112K-3.3TRG1    | SOT-25     | SYS→3.3 V regulated, 600 mA                |

## PCB Specifications

- **Layers**: 4-layer, FR-4, 1.6 mm
- **Dimensions**: ~100 × 60 mm (TBD after layout)
- **Finish**: ENIG
- **Impedance**: Controlled 50 Ω (FMC, QUADSPI, I²S, SPI, USB)
- **FMC traces**: < 10 mm, ±0.5 mm length matching within bus groups
- **QSPI traces**: < 25 mm, 33 Ω series termination at MCU end
- **SDRAM decoupling**: 100 nF per VDD pin, 10 µF bulk
- **Analog ground**: Split pour; ES9038Q2M AGND star-connected at single point

## Sourcing

### EU / Czech Republic
- **[TME](https://www.tme.eu/en/)** — primary CZ distributor; REST API at api.tme.eu
  - Good for: ST MCUs/WB55, TI PMICs, passives, connectors
- **[Digi-Key EU](https://www.digikey.de)** — NL warehouse, ships CZ/SK
  - Good for: ES9038Q2M (ESS often not at TME), LT3042, W9825G6KH-6

### United States
- **[Digi-Key](https://www.digikey.com)** — broadest range
- **[Mouser](https://www.mouser.com)** — strong on TI/ADI/LTC

### Display (special order)
- **[Good Display](https://www.good-display.com)** — GDEM0397T81P not at distributors
  - Contact: buy@e-ink-display.com
  - MOQ: typically 5–10 pcs for sample order

Per-distributor BOM files: `bom/sourcing/`

## CI Validation

GitHub Actions runs `ato check` on every push/PR via
`.github/workflows/hardware.yml`. The check-only build target validates all
`assert` statements without requiring KiCad to be installed.

## PCB Manufacturing

Recommended fabs (EU-friendly):
- **[Eurocircuits](https://www.eurocircuits.com)** — Belgium-based, fast EU delivery, DRC API
- **[JLCPCB](https://jlcpcb.com)** — low cost prototypes, ships EU
- **[PCBWay](https://www.pcbway.com)** — good quality, reasonable EU shipping

## Known TODOs

See `CLAUDE.md` for the complete TODO table. Key items:

- [ ] Add ±5 V charge pump to `power.ato` for TPA6120A2
- [ ] Verify GDEM0397T81P FPC pinout from Good Display spec sheet
- [ ] Add atomic component stubs for EC11 encoder and tactile buttons
- [ ] Route QSPI series termination resistors into signal path
- [ ] Verify STM32H743 and STM32WB55 pin numbers against KiCad symbols
- [ ] Add chip antenna component stub (Johanson 0433AT62A0100E)
- [ ] Add battery JST-PH connector footprint to `power.ato`

## Revision History

| Rev | Date       | Changes                                                                  |
|-----|------------|--------------------------------------------------------------------------|
| D   | 2026-02-19 | Migrate to Atopile code-first design; vertical slice structure; SDRAM + NOR flash added |
| C   | 2026-02-10 | Improved power supply design                                             |
| B   | 2026-02-01 | Initial BOM and component selection                                      |
| A   | 2026-01-15 | Initial hardware specification                                           |
