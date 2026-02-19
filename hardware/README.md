# Hardware Design

## Overview

The SoulAudio DAP hardware consists of:
- Main processing board (STM32H743ZIT6 + peripherals)
- Audio DAC board (ES9038Q2M + TPA6120A2 headphone amp)
- Power management (BQ25895 PMIC, LiPo battery)
- Display module (Good Display GDEM0397T81P, 3.97" E-ink)
- BT co-processor module (STM32WB55RGV6)
- Interconnects and connectors

## Design Files

### Schematics (`schematics/`)

KiCad 7.0+ project files:
```
schematics/
├── soulaudio-dap.kicad_pro      # Main project
├── soulaudio-dap.kicad_sch      # Top-level schematic
├── mcu.kicad_sch                # MCU and power
├── audio.kicad_sch              # ES9038Q2M DAC + TPA6120A2 amp
├── display.kicad_sch            # E-ink display interface
├── power.kicad_sch              # Battery and BQ25895 charging
├── bluetooth.kicad_sch          # STM32WB55RGV6 co-processor
└── connectors.kicad_sch         # External connectors
```

**To open:**
```bash
kicad schematics/soulaudio-dap.kicad_pro
```

### PCB Design (`pcb/`)

```
pcb/
├── soulaudio-dap.kicad_pcb      # Main board layout
├── audio-codec.kicad_pcb        # Audio daughterboard
└── power-module.kicad_pcb       # Power module
```

**Specifications:**
- **Layers**: 4-layer PCB
- **Dimensions**: 100mm × 60mm × 1.6mm
- **Material**: FR-4
- **Finish**: ENIG (gold)
- **Impedance**: Controlled 50Ω traces for I²S

### Bill of Materials (`bom/`)

```
bom/
├── main-board-bom.csv           # Main PCB components
├── audio-board-bom.csv          # Audio codec board
├── power-board-bom.csv          # Power module
├── mechanical-bom.csv           # Screws, standoffs, etc.
└── cable-bom.csv                # Cables and connectors
```

**Total BOM Cost** (estimated, qty 100):
- Electronics: ~$45 per unit
- Mechanical: ~$25 per unit
- PCB: ~$8 per unit
- Assembly: ~$15 per unit
- **Total**: ~$93 per unit

### Manufacturing Files (`manufacturing/`)

#### Gerber Files (`gerber/`)
Standard RS-274X format, ready for fabrication:
```
gerber/
├── soulaudio-dap-F_Cu.gbr       # Front copper
├── soulaudio-dap-B_Cu.gbr       # Back copper
├── soulaudio-dap-In1_Cu.gbr     # Inner layer 1
├── soulaudio-dap-In2_Cu.gbr     # Inner layer 2
├── soulaudio-dap-F_Mask.gbr     # Front soldermask
├── soulaudio-dap-B_Mask.gbr     # Back soldermask
├── soulaudio-dap-F_Silkscreen.gbr
├── soulaudio-dap-B_Silkscreen.gbr
├── soulaudio-dap-Edge_Cuts.gbr
├── soulaudio-dap-F_Paste.gbr    # Front paste (stencil)
├── soulaudio-dap-B_Paste.gbr    # Back paste
└── soulaudio-dap-drl.gbr        # Drill file
```

**Export from KiCad:**
```
File → Fabrication Outputs → Gerbers (.gbr)
File → Fabrication Outputs → Drill Files (.drl)
```

#### Assembly Files (`assembly/`)
```
assembly/
├── soulaudio-dap-top-pos.csv    # Pick-and-place (top)
├── soulaudio-dap-bot-pos.csv    # Pick-and-place (bottom)
└── assembly-drawing.pdf         # Assembly reference
```

#### Stencil (`stencil/`)
```
stencil/
├── top-stencil.gbr              # Solder paste stencil (top)
├── bot-stencil.gbr              # Solder paste stencil (bottom)
└── stencil-spec.txt             # Stencil specifications
```

**Stencil Specs:**
- **Thickness**: 0.12mm (4.7mil)
- **Material**: Stainless steel
- **Finish**: Electropolished

## Component Selection

### Key Components

| Component | Part Number | Function | Cost (qty 100) |
|-----------|-------------|----------|----------------|
| MCU | STM32H743ZIT6 | Main processor | $12.50 |
| DAC | ES9038Q2M | 32-bit hi-fi DAC | $7.00 |
| Headphone Amp | TPA6120A2 | Class-AB amp | $3.50 |
| Display | GDEM0397T81P | 3.97" E-ink | $22.00 |
| BT Co-proc | STM32WB55RGV6 | BLE 5.0 / LE Audio | $6.50 |
| PMIC | BQ25895RTWT | USB-C PD + charge | $2.80 |
| AVDD LDO | LT3042 | Ultra-low-noise 3.3 V for ES9038Q2M | $4.20 |
| SD Slot | DM3AT-SF-PEJM5 | microSD | $0.65 |
| USB-C | TYPE-C-31-M-12 | USB connector | $0.45 |

### Datasheets (`datasheets/`)

All component datasheets organized by category:
```
datasheets/
├── mcu/
│   └── STM32H743ZIT6.pdf
├── audio/
│   ├── ES9038Q2M.pdf
│   ├── TPA6120A2.pdf
│   └── audio-jack-spec.pdf
├── display/
│   └── GDEM0397T81P.pdf
├── power/
│   ├── BQ25895.pdf
│   ├── LT3042.pdf
│   └── lipo-battery-spec.pdf
└── connectors/
    ├── usb-c-spec.pdf
    └── microsd-spec.pdf
```

## Design Considerations

### Power Budget

| Component | Current (mA) | Notes |
|-----------|--------------|-------|
| STM32H743 @ 480 MHz | 280 | Active |
| STM32H743 sleep | 2 | WFI |
| ES9038Q2M DAC | 10 | Playback |
| TPA6120A2 amp | 35 | Driving 32Ω headphones |
| GDEM0397T81P display | 34 | During refresh only |
| Display sleep | 0.001 | ~1 µA |
| STM32WB55 BLE | 15 | Active BLE |
| microSD read | 100 | Peak |
| **Total active** | **~476 mA** | All active |
| **Total playback** | **~342 mA** | Display asleep |

**Battery Life** (3000 mAh):
- Active use: ~6.3 hours
- Audio playback: ~8.8 hours
- Standby: ~60 days

### Audio Path

```
SD Card → STM32H743 (decode: FLAC/MP3/WAV) → SAI1 I²S → ES9038Q2M → TPA6120A2 → 3.5mm
```

**Audio Specs:**
- **Sample Rates**: 44.1 / 48 / 88.2 / 96 / 176.4 / 192 / 352.8 / 384 / 768 kHz PCM
- **DSD**: DSD64 / DSD128 / DSD256 / DSD512 (native + DoP)
- **Bit Depth**: 16 / 24 / 32-bit
- **DNR**: 128 dB
- **THD+N**: −120 dB

### I²S Configuration

```
STM32 SAI1 → ES9038Q2M (slave mode)
- MCLK: 49.152 MHz (512 × fs at 96 kHz) or 45.1584 MHz (512 × fs at 88.2 kHz)
- BCLK: 6.144 MHz (64 × 96 kHz)
- LRCLK: 96 kHz
- DATA: 32-bit I²S
- I²C: control bus (volume, filter, mute, DSD mode)
```

### Display Interface

```
STM32 SPI1 → GDEM0397T81P (SSD1677 controller)
- Clock: 10 MHz
- Mode: SPI Mode 0
- Data: 8-bit
- Control: DC, RST, BUSY pins
- Resolution: 800×480, 235 PPI
- Full refresh: 3 s | Fast refresh: 1.5 s | Partial refresh: 300 ms
```

## Assembly Instructions

### PCB Assembly Process

1. **Solder Paste Application**
   - Use stencil for precise paste application
   - Inspect for bridges and insufficient paste

2. **Component Placement**
   - Use pick-and-place machine or manual placement
   - Start with smallest components (0402, QFN)
   - End with large components (connectors)

3. **Reflow Soldering**
   - Profile: Pb-free SAC305
   - Peak temp: 245°C
   - Time above liquidus: 60-90s

4. **Inspection**
   - Visual inspection for tombstones, bridges
   - X-ray for BGA/QFN packages
   - Automated optical inspection (AOI)

5. **Testing**
   - Flying probe test for shorts/opens
   - Functional test with test firmware

### Hand Assembly Notes

For prototypes or small runs:
- Solder FPC connectors last (heat sensitive)
- Use low-temp solder paste for display connector
- Hot air rework station for QFN packages (ES9038Q2M TSSOP-24, BQ25895RTWT)
- Microscope for 0402 components
- Note: LT3042 AVDD LDO must be populated for ES9038Q2M analog supply

## Testing and Validation

### Electrical Tests

1. **Power-On Test**
   - Check 3.3V rail and LT3042 AVDD output for ES9038Q2M
   - Verify current consumption
   - Test USB-C power delivery via BQ25895

2. **MCU Test**
   - SWD connection
   - Flash test firmware
   - GPIO toggle test

3. **Audio Test**
   - I²S communication (SAI1 → ES9038Q2M)
   - I²C register access (volume, filter selection)
   - DAC output measurement via TPA6120A2
   - Frequency response and THD+N measurement

4. **Display Test**
   - SPI communication to SSD1677 controller
   - Full/fast/partial refresh cycle test
   - Ghosting assessment

5. **Bluetooth Test**
   - UART HCI link to STM32WB55RGV6
   - BLE 5.0 advertising and connection
   - LE Audio streaming

### Simulations (`simulations/`)

LTspice simulations for:
```
simulations/
├── power-supply.asc             # BQ25895 charging circuit
├── audio-filter.asc             # ES9038Q2M output filter
├── battery-charger.asc          # Charging circuit
└── output-amplifier.asc         # TPA6120A2 headphone amp
```

## Manufacturing Partners

### Recommended PCB Fabs

- **JLCPCB**: Low-cost, good for prototypes
- **PCBWay**: Higher quality, better for production
- **OSH Park**: US-based, excellent quality

### Assembly Services

- **JLCPCB Assembly**: Integrated with PCB fab
- **MacroFab**: US-based, good for small runs
- **Seeed Studio**: Fusion service

## Compliance

### Certifications Required

- **FCC Part 15B**: Unintentional radiator (display, clocks)
- **FCC Part 15C**: Intentional radiator (Bluetooth — STM32WB55RGV6)
- **CE**: European compliance
- **RoHS**: Restriction of hazardous substances
- **WEEE**: Waste electrical equipment directive

### EMI/EMC Considerations

- Ground plane on layer 2
- Ferrite beads on power rails
- Shielding for STM32WB55 BLE co-processor
- Controlled impedance traces for I²S
- Separate analog ground pour for ES9038Q2M AGND / AVDD domain
- LT3042 ultra-low-noise LDO isolates DAC analog supply from digital noise

## Revision History

| Rev | Date | Changes |
|-----|------|---------|
| A | 2026-01-15 | Initial design |
| B | 2026-02-01 | Added ESP32-C3 for BT |
| C | 2026-02-10 | Improved power supply |
| D | 2026-02-19 | Replace PCM5242 with ES9038Q2M + TPA6120A2; update BT to STM32WB55; correct all component references |

## Tools Required

### Software
- KiCad 7.0+ (schematic and PCB)
- LTspice (power simulations)
- Gerber viewer (verify outputs)

### Hardware
- Oscilloscope (>100MHz)
- Logic analyzer (I²S, SPI, I²C debugging)
- Multimeter
- Hot air rework station
- Soldering iron with fine tip
- ESD-safe workstation

## Support

For hardware questions:
- Check `docs/hardware/` for detailed documentation
- Open issue on GitHub
- Email: hardware@example.com

## License

Hardware designs are licensed under CERN-OHL-P v2.
See [LICENSE-HARDWARE](../LICENSE-HARDWARE) for details.
