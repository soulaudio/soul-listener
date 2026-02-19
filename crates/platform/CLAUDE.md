# Platform — Hardware Abstraction Layer

**Context for AI Assistant**

This crate is the Hardware Abstraction Layer (HAL) for the SoulAudio DAP. Every physical peripheral is expressed as a Rust trait here; no hardware-specific code lives outside this crate. This enables 100% host-based testing and clean feature-flag-controlled hardware vs. simulator builds.

## Purpose

- Define traits for all hardware components
- Provide mock implementations for testing (`mocks` module)
- No Embassy or STM32 concrete types leak into feature crates (playback, ui, library…)
- All new hardware must get a trait here before a driver is written in `firmware`

## Module Map

| Module | Hardware it abstracts | Key trait(s) |
|---|---|---|
| `display` | Good Display GDEM0397T81P (SSD1677, SPI) | `DisplayDriver`, `EinkDisplay` |
| `audio` | ES9038Q2M DAC via SAI/I2S | `AudioCodec` |
| `input` | Rotary encoder + buttons via GPIO | `InputDevice` |
| `storage` | microSD via SDMMC1 (FAT32) | `Storage`, `File` |
| `bluetooth` | STM32WB55 via UART (HCI) | `BluetoothAdapter` |
| `sdram` | External SDRAM via FMC | `ExternalRam`, `RamRegion` |
| `asset_store` | External QSPI NOR flash | `AssetStore`, `AssetKey` |
| `gpio` | STM32 GPIO with typestate | `Pin`, `InputPin`, `OutputPin` |
| `peripheral` | SPI / I2C / UART bus abstractions | `SpiPeripheral`, `I2cPeripheral`, … |
| `dma` | DMA transfer ownership | `DmaChannel`, `DmaTransfer`, `CircularBuffer` |
| `power` | Sleep modes, clock gating, PMIC | `PowerManager`, `PowerMonitor` |
| `config` | Board-level constants | — |
| `mocks` | Test doubles for all traits | `MockDisplay`, `MockInput`, … |

## Memory Subsystems

### External SDRAM (`sdram.rs`)

Connected via FMC (Flexible Memory Controller). Two chip options:

| Option | Chip | Capacity | Interface | Package | Cost |
|---|---|---|---|---|---|
| A | IS42S16320G-7TL (ISSI) | 64 MB | 32M × 16-bit | TSOP-54 | ~$3 |
| B | W9825G6KH-6 (Winbond) | 32 MB | 16M × 16-bit | TSOP-54 | ~$2 |

Mapped at `0xC000_0000` via FMC bank 5/6.

**Canonical region layout (32 MB):**

```
0xC000_0000  Library index cache    4 MB   ~13k tracks @ 300 B/track
0xC040_0000  Album art cache        8 MB   ~500 thumbnails @ 16 KB each
0xC0C0_0000  Audio decode scratch   4 MB   FLAC working mem + DSD512 ring buf
0xC100_0000  UI overflow / spare   16 MB   Future expansion
```

**DMA rule:** Real-time audio DMA buffers must stay in internal AXI SRAM (`0x2400_0000`). SDRAM is for large, non-latency-critical data only. FMC bus arbitration adds variable latency that is unacceptable on the SAI/I2S critical path.

### External QSPI NOR Flash (`asset_store.rs`)

Connected via QUADSPI. Two chip options:

| Option | Chip | Capacity | Speed | Package | Cost |
|---|---|---|---|---|---|
| A | W25Q128JV (Winbond) | 16 MB | 133 MHz | SOIC-8 / WSON-8 | ~$1.50 |
| B | W25Q64JV (Winbond) | 8 MB | 133 MHz | SOIC-8 / WSON-8 | ~$1.00 |

Mapped at `0x9000_0000` in XiP (memory-mapped read) mode via QUADSPI.
Internal 2 MB flash is reserved for compiled firmware only.

**Partition layout:**

```
0x9000_0000  Asset index table    4 KB   Offset + size per AssetKey
0x9000_1000  Fonts               ~500 KB  5 sizes (12/16/24/32/48px), Latin+
0x9008_0000  Icons               ~200 KB  100 icons, 64×64, 2bpp sprite sheet
0x900B_0000  Waveform LUTs        ~50 KB  SSD1677 custom LUT tables
0x900C_0000  OTA staging         ~1.5 MB  Full firmware image for updates
0x901C_0000  Reserved            remainder
```

Assets are written once during factory programming. The OTA partition is the only region written at runtime.

**Asset keys** are defined in the `AssetKey` enum. Add new entries there when adding new asset types.

## Internal SRAM Allocation Rules

```
DTCM (128 KB, 0x2000_0000)   → Hot paths, ISR handlers — NO DMA ACCESS
AXI SRAM (512 KB, 0x2400_0000) → All DMA buffers: SAI audio, SPI display, SDMMC
SRAM1/2 (256 KB, 0x3000_0000)  → Embassy task stacks, heapless collections
SRAM3/4 (96 KB, 0x3004_0000)   → USB buffers, small working sets
```

Never place DMA target/source buffers in DTCM — the DMA bus cannot reach it.

## Adding a New Hardware Abstraction

1. Create `src/<name>.rs` with the trait definition and associated types
2. Follow existing style: `impl Future` return types (not `async fn` in traits yet), `type Error: Debug`
3. Add `pub mod <name>;` to `lib.rs`
4. Re-export the main trait from `lib.rs`
5. Add a `Mock<Name>` implementation to `mocks/mod.rs` with tests
6. Add the peripheral to the `Peripheral` enum in `power.rs` if it has a clock gate

## Mock Usage in Tests

```rust
use platform::mocks::{MockDisplay, MockInput, MockAudio, MockExternalRam, MockAssetStore};
use platform::asset_store::AssetKey;

// Pre-load an asset
let mut assets = MockAssetStore::new();
assets.insert(AssetKey::Font16, include_bytes!("../fixtures/font16.bin").to_vec());

// Simulate 32 MB SDRAM
let mut ram = MockExternalRam::default_32mb();
ram.write(0, &track_index_bytes).unwrap();
```

## Feature Flags

| Flag | Effect |
|---|---|
| *(none)* | Trait definitions only, `no_std` |
| `std` | Enables `mocks` module, `std::error::Error` impls |
| `simulator` | Desktop simulator hook points |
| `hardware` | Physical hardware target marker |
| `defmt` | Adds `defmt::Format` derives to all enums/structs |

Tests always run with `std` implied by the test harness.
