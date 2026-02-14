# Hardware Design

## Overview

The SoulAudio DAP hardware consists of:
- Main processing board (STM32H7 + peripherals)
- Audio codec board (WM8960)
- Power management board (battery, charging)
- Display module (E-ink 4.2")
- Interconnects and connectors

## Design Files

### Schematics (`schematics/`)

KiCad 7.0+ project files:
```
schematics/
├── soulaudio-dap.kicad_pro      # Main project
├── soulaudio-dap.kicad_sch      # Top-level schematic
├── mcu.kicad_sch                # MCU and power
├── audio.kicad_sch              # Audio codec
├── display.kicad_sch            # E-ink display interface
├── power.kicad_sch              # Battery and charging
├── bluetooth.kicad_sch          # ESP32-C3 module
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
- **Impedance**: Controlled 50Ω traces for I2S

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
| Audio Codec | WM8960 | 24-bit DAC/ADC | $3.20 |
| Display | Waveshare 4.2" | E-ink display | $18.50 |
| BT Module | ESP32-C3-MINI-1 | Bluetooth | $2.10 |
| Power IC | TPS63070 | Buck-boost | $2.45 |
| Battery Charger | BQ24072 | Li-ion charger | $1.80 |
| SD Card Slot | DM3AT-SF-PEJM5 | MicroSD connector | $0.65 |
| USB-C | TYPE-C-31-M-12 | USB connector | $0.45 |

### Datasheets (`datasheets/`)

All component datasheets organized by category:
```
datasheets/
├── mcu/
│   └── STM32H743ZI.pdf
├── audio/
│   ├── WM8960.pdf
│   └── audio-jack-spec.pdf
├── display/
│   └── waveshare-4.2inch-e-paper.pdf
├── power/
│   ├── TPS63070.pdf
│   ├── BQ24072.pdf
│   └── 18650-battery-spec.pdf
└── connectors/
    ├── usb-c-spec.pdf
    └── microsd-spec.pdf
```

## Design Considerations

### Power Budget

| Component | Current (mA) | Notes |
|-----------|--------------|-------|
| STM32H7 @ 480MHz | 280 | Active processing |
| STM32H7 sleep | 2 | WFI mode |
| E-ink display | 40 | During refresh only |
| E-ink sleep | 0.01 | Ultra low power |
| Audio codec | 35 | Playback |
| SD card | 100 | Read operations |
| Bluetooth | 50 | Active connection |
| **Total (active)** | **505 mA** | |
| **Total (playback)** | **330 mA** | Display sleep |

**Battery Life** (3000mAh):
- Active use: ~9 hours
- Audio playback: ~21 hours
- Standby: ~60 days

### Audio Path

```
SD Card → STM32 (decode) → I2S → WM8960 DAC → Line Out/Headphone
                                                      ↓
                                                  3.5mm Jack
```

**Audio Specs:**
- **Sample Rates**: 8kHz - 192kHz
- **Bit Depth**: 16/24-bit
- **SNR**: >100dB
- **THD+N**: <0.01%
- **Output Power**: 40mW @ 32Ω

### I2S Configuration

```
STM32 SAI1 → WM8960
- BCLK: 3.072 MHz (48kHz × 64)
- LRCLK: 48 kHz
- DATA: 24-bit samples
- MCLK: 12.288 MHz (256 × fs)
```

### Display Interface

```
STM32 SPI1 → E-ink Display
- Clock: 10 MHz
- Mode: SPI Mode 0
- Data: 8-bit
- Control: DC, RST, BUSY pins
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
- Hot air rework station for QFN packages
- Microscope for 0402 components

## Testing and Validation

### Electrical Tests

1. **Power-On Test**
   - Check 3.3V rail
   - Verify current consumption
   - Test USB-C power delivery

2. **MCU Test**
   - SWD connection
   - Flash test firmware
   - GPIO toggle test

3. **Audio Test**
   - I2S communication
   - DAC output measurement
   - Frequency response test

4. **Display Test**
   - SPI communication
   - Refresh cycle test
   - Ghosting assessment

### Simulations (`simulations/`)

LTspice simulations for:
```
simulations/
├── power-supply.asc             # Buck-boost converter
├── audio-filter.asc             # Anti-aliasing filter
├── battery-charger.asc          # Charging circuit
└── output-amplifier.asc         # Headphone amp
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
- **FCC Part 15C**: Intentional radiator (Bluetooth)
- **CE**: European compliance
- **RoHS**: Restriction of hazardous substances
- **WEEE**: Waste electrical equipment directive

### EMI/EMC Considerations

- Ground plane on layer 2
- Ferrite beads on power rails
- Shielding for Bluetooth module
- Controlled impedance traces for I2S

## Revision History

| Rev | Date | Changes |
|-----|------|---------|
| A | 2026-01-15 | Initial design |
| B | 2026-02-01 | Added ESP32-C3 for BT |
| C | 2026-02-10 | Improved power supply |

## Tools Required

### Software
- KiCad 7.0+ (schematic and PCB)
- LTspice (power simulations)
- Gerber viewer (verify outputs)

### Hardware
- Oscilloscope (>100MHz)
- Logic analyzer (I2S, SPI debugging)
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
