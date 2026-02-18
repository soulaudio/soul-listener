# Soul Listener

[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)

A Digital Audio Player (DAP) firmware written in Rust, targeting STM32H7 with an e-ink display. Includes a full desktop emulator for developing and testing without hardware.

## Hardware

| Component | Part |
|-----------|------|
| MCU | STM32H743ZI — 480 MHz Cortex-M7 |
| Display | Waveshare 4.2" E-ink (400×300) |
| Audio codec | WM8960 — 24-bit / 192 kHz |
| Storage | MicroSD (FAT32) |
| Bluetooth | ESP32-C3 |
| Battery | 18650 Li-ion |
| Connectors | USB-C (charge + data), 3.5 mm |

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
# Build for STM32H7
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
