# Soul Listener

[![License: AGPL-3.0](https://img.shields.io/badge/license-AGPL--3.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)

A Digital Audio Player (DAP) firmware written in Rust, targeting the STM32H7A3 with an e-ink display. Developed and validated hardware-free via host simulation (`dap-sim`) and on-target memory/stack fitness tests, so the CI-green artifact is the artifact the board flashes.

## Hardware

| Component | Part | Interface | Notes |
|-----------|------|-----------|-------|
| MCU | STM32H743ZI | — | 480 MHz Cortex-M7, FPU/DSP, 2 MB flash, 1 MB SRAM |
| Display | Good Display GDEM0397T81P | SPI + DMA | 3.97", 800×480, 235 PPI, SSD1677 controller, Carta 1200 panel |
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

## Workspaces

The firmware is split across three Cargo workspaces (the abandoned v1 Embassy-0.1
stack was retired; its one keepable piece, the SSD1677 driver, moved into `dap/`):

```
dap/                 # ACTIVE STM32H7A3 firmware (its own workspace)
├── core/            # dap-core: board-agnostic logic + ports (no_std, no embassy)
├── sim/             # dap-sim: host simulation adapters + RTF benches
├── board-h7a3/      # the embassy 0.6 / HAL entry point (thumbv7em binary)
├── arch-tests/      # H7A3 memory-map + per-region SRAM budget fitness tests
└── eink-ssd1677/    # SSD1677/SSD2677 e-ink panel driver (salvaged from v1)
eink-ui/             # standalone no_std/no-alloc e-ink UI toolkit (own workspace)
crates/eink/eink-specs/   # pure panel-spec data (dap consumes it cross-workspace)
xtask/               # dev + gate tooling (cargo xtask precommit|verify)
```

`dap/` path-depends the sibling `soul-audio-core` repo (checked out next to this
one) for decoding.

## Quick start

```bash
# Host checks for the active firmware (needs ../soul-audio-core checked out):
cd dap && cargo test               # dap-core / dap-sim / arch-tests / eink-ssd1677
cd dap && cargo clippy --workspace --all-targets -- -D warnings

# Cross-build the board binary for the STM32H7A3:
cd dap && cargo build -p board-h7a3 --target thumbv7em-none-eabihf --features hardware

# The e-ink UI toolkit:
cd eink-ui && cargo test --features std

# The full pre-push gate (fmt + clippy + build + test across all workspaces):
cargo xtask verify
```

## License

[GNU Affero General Public License v3.0](LICENSE)
