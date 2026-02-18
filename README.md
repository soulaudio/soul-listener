# Soul Listener

[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)

A Digital Audio Player (DAP) firmware written in Rust, targeting STM32H7 with an e-ink display. Includes a full desktop emulator for developing and testing without hardware.

## Hardware

| Component | Part | Interface | Notes |
|-----------|------|-----------|-------|
| MCU | STM32H743ZI | — | 480 MHz Cortex-M7, FPU/DSP, 2 MB flash, 1 MB SRAM |
| Display | 4.2" e-ink (400×300) | SPI + DMA | Waveshare v2 or equivalent; draws no power between refreshes |
| Audio DAC | PCM5242 (TI) | I²S via SAI1 | 32-bit / 384 kHz, integrated PLL, charge-pump headphone driver |
| Headphone amp | TPA6120A2 (TI) | Analog | 250 mA, class-AB, low-THD headphone output stage |
| BLE co-processor | STM32WB55RGV6 | UART (HCI) | Arm M4+M0+, BLE 5.0; ST provides certified BT firmware |
| USB | STM32H7 internal HS PHY | USB-C | UAC2 (USB audio class 2) + USB-C charging; no extra IC |
| PMIC / charger | BQ25895 (TI) | I²C | USB-C PD negotiation, LiPo charge, power-path management |
| Storage | microSD | SDMMC1 (4-bit) | FAT32, UHS-I |
| Battery | Flat LiPo | — | 2000–4000 mAh depending on enclosure |
| Input | Rotary encoder + 4 buttons | GPIO / EXTI | Alps EC11 or equivalent |

### Bluetooth architecture

The STM32WB55 is the correct choice for BLE on this hardware — it runs on the same ARM toolchain, is debuggable with the same probe-rs/STM32CubeIDE tooling, and ST ships a certified, OTA-updatable BT stack firmware so you never write or certify a BT stack yourself.

**v1 — BLE control only**: companion app control (track skip, volume, now-playing metadata) over a custom GATT profile. The WB55 connects to the H743 via UART using the HCI protocol; Embassy handles the host-side stack.

**v2 — BLE Audio**: stream audio to LE Audio headphones using LC3 codec over BLE ISO channels. Requires BLE 5.2 (WB55 supports this). Requires compatible headphones.

**Classic BT A2DP (pairing with existing Bluetooth headphones) is explicitly out of scope.** It requires a chip from a separate vendor ecosystem (CSR/Qualcomm) and a separate BT stack — that complexity belongs in a different hardware revision, not in v1.

## Crates

```
crates/
├── eink/
│   ├── eink-emulator/   # Desktop e-ink emulator (winit + softbuffer)
│   ├── eink-specs/      # Display specifications (pure data)
│   ├── eink-system/     # Layout / styling engine
│   ├── eink-components/ # Standard UI components
│   └── eink-testing/    # Testing utilities
├── platform/            # Hardware abstraction layer (HAL traits)
├── firmware/            # Main firmware + display_emulator example
├── firmware-ui/         # Hot-reloadable UI rendering dylib
└── playback/            # Audio playback engine
```

## Quick start

### Desktop emulator

```bash
# Run the emulator (no hardware required)
cargo run --example display_emulator --features emulator -p firmware

# With debug inspector (Chrome DevTools-style overlay)
cargo run --example display_emulator --features emulator,debug -p firmware
```

### Firmware

```bash
# Build for STM32H743
cargo build --release --target thumbv7em-none-eabihf -p firmware

# Flash with probe-rs
cargo embed --release -p firmware
```

### Tests

```bash
cargo test --workspace
cargo test -p eink-emulator --features debug
```

## Debug inspector

Press **Ctrl+3** inside the emulator to open the inspector panel. Hover components to inspect layout, box model, and attributes. **Tab** cycles inspector tabs (LYT → BOX → CMP). **Ctrl+1/2** toggle borders and panel visibility.

## License

[GNU Affero General Public License v3.0](LICENSE)
