# Digital Audio Player (DAP) - Embedded Rust Project

## Project Overview

This is a professional-grade Digital Audio Player firmware written in Rust using the Embassy async framework. The project demonstrates industry-standard embedded Rust patterns with complete hardware abstraction, enabling development and testing without physical hardware.

**Key Features:**
- **E-ink Display**: Optimized rendering with simulator support for local development
- **Audio Playback**: FLAC, MP3, WAV support with DMA streaming
- **Input System**: Abstracted input layer (rotary encoder, buttons) with keyboard simulation
- **Bluetooth**: Wireless audio streaming and control
- **SD Card**: FAT32 file system with embedded-sdmmc
- **Vertical Slice Architecture**: Feature-based organization for maintainability
- **100% Testable**: Hardware abstractions enable host-based testing

## Target Hardware

**MCU:** STM32H743ZI (Cortex-M7, 480 MHz, FPU/DSP)
- 2 MB dual-bank Flash (firmware only; assets offloaded to QSPI NOR)
- ~992 KB SRAM: 128 KB DTCM (CPU-only, no DMA) + 512 KB AXI SRAM (D1, DMA-accessible) + 256 KB SRAM1/2 (D2) + 96 KB SRAM3/4
- SAI1/2 for I²S/TDM audio (DMA)
- SDMMC1 for microSD (4-bit, UHS-I)
- USB OTG HS with internal PHY (UAC2 + USB-C data)
- FMC for external SDRAM
- QUADSPI for external NOR flash (XiP capable)

**Display:** Good Display GDEM0397T81P via SPI + DMA
- 3.97", 800×480, 235 PPI, Carta 1200 panel, SSD1677 controller
- Full refresh 3 s · Fast refresh 1.5 s · Partial refresh 300 ms
- Deep sleep: ~1 µA; active refresh: ~34 mA average

**Audio:**
- DAC: ES9038Q2M (ESS Technology) — 32-bit/768 kHz PCM, DSD512 (native + DoP), 128 dB DNR, −120 dB THD+N, I²C programmable (volume, filter, oversampling)
- Headphone amp: TPA6120A2 (TI) — class-AB, 250 mA

**Bluetooth:** STM32WB55RGV6 co-processor
- Arm Cortex-M4 (64 MHz) + M0+ (32 MHz), integrated 2.4 GHz radio
- BLE 5.0 / BLE Audio (LE Audio, LC3 codec)
- Connected to STM32H743 via UART (HCI protocol)
- ST provides and maintains the certified BT stack firmware
- Same ARM toolchain, debuggable with probe-rs — no second vendor

> **Classic BT (A2DP) is out of scope for v1.** It requires a dedicated chip
> from another vendor ecosystem (CSR/Qualcomm). The STM32WB55 handles BLE
> control (v1) and BLE Audio streaming to LE Audio headphones (v2).

**PMIC:** BQ25895 (TI) — USB-C PD, LiPo charge, I²C control
**Storage:** microSD via SDMMC1 (FAT32)
**Battery:** Flat LiPo, 2000–4000 mAh

**External SDRAM:** via FMC (Flexible Memory Controller) — holds music library index cache, album art thumbnail cache, large audio decode scratch, UI overflow buffers
- **Option A — 64 MB:** IS42S16320G-7TL (ISSI) — 32M × 16-bit, 143 MHz, TSOP-54, ~$3
- **Option B — 32 MB:** W9825G6KH-6 (Winbond) — 16M × 16-bit, 166 MHz, TSOP-54, ~$2
- DMA buffers for real-time audio stay in internal AXI SRAM to avoid FMC latency
- Mapped at 0xC0000000 via FMC bank 5/6; SDRAM controller built into STM32H7

**External QSPI NOR Flash:** via QUADSPI — holds read-only assets offloaded from internal 2 MB flash (fonts, icons, waveform LUTs, OTA staging partition)
- **Option A — 16 MB:** W25Q128JV (Winbond) — SPI/QSPI, 133 MHz, SOIC-8/WSON-8, ~$1.50
- **Option B — 8 MB:** W25Q64JV (Winbond) — SPI/QSPI, 133 MHz, SOIC-8/WSON-8, ~$1.00
- Supports XiP (eXecute in Place) if code needs to overflow internal flash
- Internal 2 MB flash reserved for compiled firmware only; QSPI holds fonts + icons + LUTs

## Architecture Principles

### 1. Hardware Abstraction via Traits
All hardware interactions go through trait-based abstractions:
- `DisplayDriver` trait for e-ink vs simulator
- `InputDevice` trait for physical buttons vs keyboard
- `BluetoothAdapter` trait for BT module vs mock
- `Storage` trait for SD card vs filesystem

### 2. Vertical Slice Organization
Features organized by domain, not technical layer:
```
crates/
├── playback/          # Complete playback feature
├── ui/                # Display and UI management
├── library/           # Music library management
├── bluetooth/         # Bluetooth audio/control
└── platform/          # Hardware abstraction layer
```

### 3. Compile-Time Safety
- Typestate pattern for peripheral states
- Zero-sized references for peripheral access
- const generics for compile-time buffer sizing
- No runtime allocations (use `heapless` collections)

### 4. Testing Strategy
- Unit tests: Hardware-independent logic on host
- Integration tests: Mock HAL implementations
- Simulator: Full UI testing without hardware
- Hardware-in-Loop: Final validation with probe-rs

## Development Workflow

### Local Development (Simulator)
```bash
# Run with display simulator
cargo run --features simulator

# Run tests on host
cargo test

# With keyboard input simulation
cargo run --features simulator,keyboard-input
```

### Hardware Development
```bash
# Build for target
cargo build --release --target thumbv7em-none-eabihf

# Flash and debug with probe-rs
cargo embed --release

# Run on-device tests
cargo test --features hardware
```

### Code Quality
```bash
# Format check
cargo fmt --check

# Linting
cargo clippy -- -D warnings

# Check for target compatibility
cargo check --target thumbv7em-none-eabihf --no-default-features
```

## Project Structure

```
eink-emulator/
├── Cargo.toml              # Workspace root
├── CLAUDE.md               # This file
├── memory.x                # Linker script for target
├── .cargo/
│   └── config.toml         # Target and runner configuration
├── crates/
│   ├── platform/           # HAL abstractions
│   │   ├── display/        # Display traits and drivers
│   │   ├── input/          # Input abstractions
│   │   ├── bluetooth/      # BT abstractions
│   │   ├── storage/        # Storage abstractions
│   │   └── audio/          # Audio codec drivers
│   ├── playback/           # Audio playback engine
│   ├── ui/                 # UI framework and widgets
│   ├── library/            # Music library management
│   ├── simulator/          # Desktop simulator
│   └── firmware/           # Main firmware application
├── docs/
│   ├── ARCHITECTURE.md     # System architecture
│   ├── ABSTRACTIONS.md     # HAL abstraction guide
│   ├── TESTING.md          # Testing strategy
│   ├── DEVELOPMENT.md      # Development guide
│   ├── BEST_PRACTICES.md   # Embedded Rust best practices
│   └── AUDIO.md            # Audio architecture
└── tests/
    ├── integration/        # Integration tests
    └── hardware/           # HIL tests
```

## Key Dependencies

### Core Framework
- `embassy-executor` - Async executor for embedded
- `embassy-time` - Timers without overflow
- `embassy-sync` - Channels and synchronization

### Hardware Support
- `embassy-stm32` - STM32H7 HAL with async support
- `embedded-hal` - Hardware abstraction traits (v1.0)
- `embedded-hal-async` - Async HAL traits

### Display
- `embedded-graphics` - 2D graphics primitives
- `embedded-layout` - Layout management
- `epd-waveshare` - E-ink display driver
- `embedded-graphics-simulator` - Desktop testing

### Audio
- `biquad` - IIR filters for DSP
- `rubato` - Sample rate conversion
- `dasp` - Digital audio signal processing (no_std)
- FFI to C decoders (minimp3, libfoxenflac)

### Storage
- `embedded-sdmmc` - FAT16/FAT32 filesystem
- `ekv` - Key-value store for settings

### Utilities
- `heapless` - Static collections
- `defmt` - Efficient logging
- `serde` - Serialization (no_std)

## Build Configuration

### Optimization for Size and Performance

```toml
[profile.release]
opt-level = "z"           # Optimize for size
lto = true                # Link-time optimization
codegen-units = 1         # Better optimization
debug = true              # Keep symbols for debugging
panic = "abort"           # No unwinding
strip = false             # Keep symbols
overflow-checks = true    # Safety checks
```

### Feature Flags

```toml
[features]
default = ["simulator"]
simulator = ["embedded-graphics-simulator", "keyboard-input"]
hardware = ["embassy-stm32", "probe-rs-target"]
keyboard-input = []       # Keyboard → button mapping
defmt-logging = ["defmt", "defmt-rtt"]
std = []                  # Enable std for host testing
```

## Hardware Abstraction Examples

### Display Abstraction
```rust
// platform/display/mod.rs
pub trait DisplayDriver {
    type Error;

    fn refresh_full(&mut self) -> Result<(), Self::Error>;
    fn refresh_partial(&mut self) -> Result<(), Self::Error>;
    fn sleep(&mut self) -> Result<(), Self::Error>;
}

impl<DI> DisplayDriver for EinkDisplay<DI> { /* hardware */ }
impl DisplayDriver for SimulatorDisplay { /* simulator */ }
```

### Input Abstraction
```rust
// platform/input/mod.rs
pub trait InputDevice {
    async fn wait_for_event(&mut self) -> InputEvent;
}

pub enum InputEvent {
    ButtonPress(Button),
    RotaryIncrement(i32),
    Touch(Point),
}

// Hardware: actual GPIO
// Simulator: keyboard events
```

## Memory Architecture

### Memory Map

| Region | Size | Location | Usage |
|---|---|---|---|
| Internal Flash | 2 MB | 0x08000000 | Compiled firmware only |
| DTCM | 128 KB | 0x20000000 | Hot paths, ISR scratch (no DMA) |
| AXI SRAM | 512 KB | 0x24000000 | DMA buffers: audio SAI, display SPI, SDMMC |
| SRAM1/2 | 256 KB | 0x30000000 | Embassy task stacks, heapless collections |
| SRAM3/4 | 96 KB | 0x30040000 | USB buffers, small working sets |
| External SDRAM | 32–64 MB | 0xC0000000 | Library index cache, album art, decode scratch |
| External QSPI NOR | 8–16 MB | 0x90000000 | Fonts, icons, waveform LUTs, OTA staging |

### Allocation Rules
- All allocations static or stack-based — no heap
- Use `heapless::Vec` and `heapless::String` for collections
- **DMA buffers must live in AXI SRAM or SRAM1/2** — DTCM is not DMA-accessible
- Audio SAI DMA ping-pong buffers: AXI SRAM (low latency, D1 domain)
- Display framebuffer (2× 96 KB for 800×480 @ 2bpp): AXI SRAM
- Library browse cache (current page ~50 entries): SRAM1/2
- Full library index (5k+ tracks), album art cache: external SDRAM
- Fonts, icons, LUTs: external QSPI NOR (read via QUADSPI DMA or XiP)

### Stack Size Configuration
```rust
// memory.x — per-task stacks via Embassy
_stack_size = 16K;  /* Per task; adjust per task complexity */
```

### Buffer Sizing
```rust
// Audio DMA ping-pong in AXI SRAM — must be DMA-accessible
const AUDIO_BUFFER_SIZE: usize = 8192;  // 2048 samples × 2ch × 2B
static AUDIO_BUFFER: StaticCell<[u8; AUDIO_BUFFER_SIZE]> = StaticCell::new();

// Large decode scratch in external SDRAM
const FLAC_SCRATCH_SIZE: usize = 131_072;  // 128 KB
```

## Testing Philosophy

1. **Fast Feedback**: Most logic tested on host (cargo test)
2. **Abstraction Testing**: Mock implementations verify interfaces
3. **Simulator Validation**: Visual testing of UI without hardware
4. **Hardware Validation**: Final testing on actual device

See `docs/TESTING.md` for comprehensive testing guide.

## Error Handling

- **Libraries**: Use `thiserror` for typed errors
- **Application**: Use `anyhow::Result` for error propagation
- **Panic Handler**: defmt-based panic with RTT output
- **No unwrap()**: All production code uses proper error handling

## Concurrency Model

Embassy tasks for concurrent operations:
```rust
#[embassy_executor::task]
async fn audio_playback_task() { /* DMA streaming */ }

#[embassy_executor::task]
async fn ui_update_task() { /* Display refresh */ }

#[embassy_executor::task]
async fn input_handler_task() { /* Button/encoder */ }

#[embassy_executor::task]
async fn bluetooth_task() { /* BT audio */ }
```

Communication via Embassy channels:
```rust
static PLAYBACK_CMD: Channel<CriticalSectionRawMutex, PlaybackCommand, 4> = Channel::new();
```

## Performance Targets

- **Audio Latency**: < 50ms from decode to I2S output
- **UI Responsiveness**: < 100ms input → display update
- **Power Consumption**: < 50mA during playback (target)
- **Battery Life**: > 20 hours continuous playback

## Code Style

- Follow Rust standard formatting (rustfmt)
- Document all public APIs with `///` comments
- Use `#[must_use]` for important return values
- Prefer explicit types over inference in public APIs
- Use typestate pattern for state machines

## Resources

- [Embassy Book](https://embassy.dev/book/)
- [Embedded Rust Book](https://docs.rust-embedded.org/book/)
- [STM32H7 Reference Manual](https://www.st.com/resource/en/reference_manual/rm0433-stm32h742-stm32h743753-and-stm32h750-value-line-advanced-armbased-32bit-mcus-stmicroelectronics.pdf)
- [embedded-hal Documentation](https://docs.rs/embedded-hal)

## Contributing

1. Check existing abstractions before adding hardware-specific code
2. Write tests for all new functionality
3. Ensure simulator still works after changes
4. Run full CI checks locally before PR
5. Update documentation for architectural changes

## License

[Specify your license here]

## Maintainers

[Your contact information]
