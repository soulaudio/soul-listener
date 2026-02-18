# System Architecture

## Overview

The DAP firmware follows a **vertical slice architecture** with **hardware abstraction layers** enabling development without physical hardware. The system is designed around Embassy's async executor with feature-based module organization.

## Architectural Layers

```
┌─────────────────────────────────────────────────────────┐
│          Application Layer (firmware crate)              │
│  - Main async tasks                                      │
│  - Feature orchestration                                 │
│  - User workflows                                        │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│         Feature Layers (Vertical Slices)                 │
│  ┌──────────┬──────────┬──────────┬──────────┐          │
│  │ playback │    ui    │ library  │bluetooth │          │
│  │          │          │          │          │          │
│  │ Complete │ Complete │ Complete │ Complete │          │
│  │ features │ features │ features │ features │          │
│  └──────────┴──────────┴──────────┴──────────┘          │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│     Platform Abstraction Layer (platform crate)         │
│  - Trait definitions                                     │
│  - Multiple implementations (hardware + simulator)       │
│  - embedded-hal trait implementations                    │
└─────────────────────────────────────────────────────────┘
                          ↓
┌─────────────────────────────────────────────────────────┐
│              Hardware Layer                              │
│  - Embassy HAL (embassy-stm32)                          │
│  - Peripheral drivers                                    │
│  - DMA controllers                                       │
└─────────────────────────────────────────────────────────┘
```

## Core Design Principles

### 1. Vertical Slice Architecture

Each feature is a **complete vertical slice** containing all layers needed for that feature:

**Example: Playback Slice**
```
crates/playback/
├── src/
│   ├── lib.rs              # Public API
│   ├── decoder.rs          # Audio decoding (FLAC, MP3)
│   ├── streaming.rs        # DMA buffer management
│   ├── dsp.rs              # Filters, EQ, resampling
│   └── state.rs            # Playback state machine
└── tests/
    └── playback_tests.rs   # Integration tests
```

**Benefits:**
- Features can be developed independently
- Clear boundaries between features
- Easy to test in isolation
- Minimal coupling between slices

### 2. Hardware Abstraction

All hardware access goes through **trait-based abstractions**:

```rust
// Trait definition (platform agnostic)
pub trait DisplayDriver: DrawTarget {
    async fn refresh_full(&mut self) -> Result<(), Self::Error>;
    async fn refresh_partial(&mut self) -> Result<(), Self::Error>;
    async fn sleep(&mut self) -> Result<(), Self::Error>;
}

// Hardware implementation
impl<SPI, DC, BUSY> DisplayDriver for EinkDisplay<SPI, DC, BUSY> { }

// Simulator implementation
impl DisplayDriver for SimulatorDisplay { }
```

**Key Abstraction Layers:**
- **Display**: E-ink vs SDL2 simulator
- **Input**: GPIO buttons vs keyboard events
- **Audio**: I2S DMA vs host audio output
- **Storage**: SD card vs host filesystem
- **Bluetooth**: STM32WB55 UART/HCI vs mock adapter

### 3. Dependency Injection via Generics

Use Rust's type system for compile-time dependency injection:

```rust
pub struct PlaybackEngine<D, S>
where
    D: DisplayDriver,
    S: Storage,
{
    display: D,
    storage: S,
    // ...
}

impl<D, S> PlaybackEngine<D, S>
where
    D: DisplayDriver,
    S: Storage,
{
    pub fn new(display: D, storage: S) -> Self {
        Self { display, storage }
    }

    pub async fn play_track(&mut self, path: &str) -> Result<()> {
        let file = self.storage.open(path).await?;
        // Decode and play...
        self.display.refresh_partial().await?;
        Ok(())
    }
}
```

**Benefits:**
- Zero runtime overhead
- Compile-time verification
- Easy to mock for testing
- No dynamic dispatch needed

## Module Organization

### Workspace Structure

```
eink-emulator/
├── Cargo.toml                    # Workspace root
└── crates/
    ├── platform/                 # HAL abstractions
    │   ├── display/
    │   ├── input/
    │   ├── audio/
    │   ├── storage/
    │   └── bluetooth/
    ├── playback/                 # Audio playback feature
    ├── ui/                       # User interface feature
    ├── library/                  # Music library feature
    ├── bluetooth/                # Bluetooth feature
    ├── simulator/                # Desktop simulator
    └── firmware/                 # Main application
```

### Platform Crate Structure

```rust
// platform/src/lib.rs
#![no_std]

pub mod display;
pub mod input;
pub mod audio;
pub mod storage;
pub mod bluetooth;

// Re-export common types
pub use display::DisplayDriver;
pub use input::{InputDevice, InputEvent};
pub use audio::AudioCodec;
pub use storage::Storage;
pub use bluetooth::BluetoothAdapter;
```

### Feature Crate Independence

Each feature crate:
- Depends only on `platform` crate (traits)
- Has no knowledge of other features
- Publishes its own API
- Contains its own tests

## Concurrency Architecture

### Embassy Task Model

```rust
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Initialize hardware
    let peripherals = init_hardware();

    // Spawn independent tasks
    spawner.spawn(audio_task(peripherals.sai)).unwrap();
    spawner.spawn(ui_task(peripherals.spi)).unwrap();
    spawner.spawn(input_task(peripherals.exti)).unwrap();
    spawner.spawn(bluetooth_task(peripherals.uart)).unwrap();
    spawner.spawn(storage_task(peripherals.sdmmc)).unwrap();
}
```

### Inter-Task Communication

**Embassy Channels:**
```rust
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

// Command channel: UI → Playback
static PLAYBACK_COMMANDS: Channel<
    CriticalSectionRawMutex,
    PlaybackCommand,
    8
> = Channel::new();

// Status channel: Playback → UI
static PLAYBACK_STATUS: Channel<
    CriticalSectionRawMutex,
    PlaybackStatus,
    4
> = Channel::new();
```

**Signal for State Updates:**
```rust
use embassy_sync::signal::Signal;

// Current track info
static CURRENT_TRACK: Signal<
    CriticalSectionRawMutex,
    TrackInfo
> = Signal::new();
```

**PubSubChannel for Events:**
```rust
use embassy_sync::pubsub::PubSubChannel;

// System events (battery, errors, etc.)
static SYSTEM_EVENTS: PubSubChannel<
    CriticalSectionRawMutex,
    SystemEvent,
    4,
    2,  // 2 subscribers
    1   // 1 publisher
> = PubSubChannel::new();
```

## State Management

### Typestate Pattern for Hardware

```rust
// GPIO pin states encoded in types
pub struct Pin<MODE> {
    _mode: PhantomData<MODE>,
}

pub struct Input;
pub struct Output;

impl Pin<Input> {
    pub fn read(&self) -> bool { /* ... */ }
    pub fn into_output(self) -> Pin<Output> { /* ... */ }
}

impl Pin<Output> {
    pub fn set_high(&mut self) { /* ... */ }
    pub fn set_low(&mut self) { /* ... */ }
    // Can't call read() - not available in Output mode
}
```

### Enum-Based State Machines

```rust
pub enum PlaybackState {
    Stopped,
    Playing { position: Duration, track: TrackId },
    Paused { position: Duration, track: TrackId },
    Seeking { target: Duration, track: TrackId },
}

impl PlaybackState {
    pub fn handle_event(self, event: PlaybackEvent) -> Self {
        match (self, event) {
            (Self::Stopped, PlaybackEvent::Play(track)) => {
                Self::Playing { position: Duration::ZERO, track }
            }
            (Self::Playing { position, track }, PlaybackEvent::Pause) => {
                Self::Paused { position, track }
            }
            // ... all transitions
            (state, _) => state, // Invalid transitions keep current state
        }
    }
}
```

## Memory Architecture

### Static Allocation

```rust
use static_cell::StaticCell;

// Audio buffers (allocated at compile time)
static AUDIO_BUFFER_0: StaticCell<[i16; 4096]> = StaticCell::new();
static AUDIO_BUFFER_1: StaticCell<[i16; 4096]> = StaticCell::new();

// Display framebuffer
static FRAMEBUFFER: StaticCell<[u8; 240 * 320 / 8]> = StaticCell::new();

// File buffers
static FILE_BUFFER: StaticCell<[u8; 512]> = StaticCell::new();
```

### Heapless Collections

```rust
use heapless::{String, Vec};

// Playlist (max 100 tracks)
type Playlist = Vec<TrackId, 100>;

// Track name (max 64 chars)
type TrackName = String<64>;

// Directory path (max 128 chars)
type Path = String<128>;
```

### Memory Map (STM32H743ZI)

```
0x0800_0000 ┌──────────────────┐
            │   Flash (2MB)    │  ← dual-bank; Bank1 + Bank2
            │                  │
            │   - Code         │
            │   - Const data   │
            │   - String table │
0x081F_FFFF └──────────────────┘

0x2000_0000 ┌──────────────────┐
            │  DTCM RAM (128KB)│
            │                  │
            │   - Stack        │
            │   - Fast vars    │
0x2001_FFFF └──────────────────┘

0x2400_0000 ┌──────────────────┐
            │  AXI SRAM (512KB)│
            │                  │
            │   - Audio buffers│
            │   - Framebuffer  │
0x2407_FFFF └──────────────────┘

0x3000_0000 ┌──────────────────┐
            │ SRAM1/2 (128KB)  │
            │                  │
            │   - DMA buffers  │
0x3001_FFFF └──────────────────┘
```

## Error Handling Strategy

### Typed Errors

```rust
// Library error types (using thiserror)
#[derive(Error, Debug)]
pub enum StorageError {
    #[error("File not found: {0}")]
    NotFound(String<128>),

    #[error("I/O error")]
    Io(#[from] embedded_sdmmc::Error),

    #[error("Invalid path")]
    InvalidPath,
}

// Application error type (using anyhow)
pub type Result<T> = anyhow::Result<T>;
```

### Error Propagation

```rust
pub async fn load_track(&mut self, path: &str) -> Result<Track> {
    let file = self.storage
        .open(path)
        .await
        .context("Failed to open audio file")?;

    let metadata = self.decoder
        .parse_metadata(&file)
        .context("Failed to parse track metadata")?;

    Ok(Track { file, metadata })
}
```

### Panic Handling

```rust
use defmt_rtt as _;
use panic_probe as _;

// Panic handler logs via RTT and halts
// In release: panic = "abort" in Cargo.toml
```

## Performance Considerations

### DMA Usage

All bulk transfers use DMA:
- **Audio**: I2S DMA for continuous streaming
- **Display**: SPI DMA for framebuffer transfer
- **SD Card**: SDMMC DMA for file reads

### Zero-Copy Patterns

```rust
// Decoder writes directly to DMA buffer
pub async fn decode_frame(&mut self, output: &mut [i16]) -> Result<usize> {
    // Fill output buffer directly
    // No intermediate allocations
}
```

### Async for Efficiency

```rust
// CPU sleeps during I/O operations
let data = uart.read(&mut buffer).await?;  // DMA transfer, CPU sleeps
let result = codec.decode(&data).await?;   // Process on CPU
display.refresh().await?;                  // DMA transfer, CPU sleeps
```

## Build Configurations

### Feature Matrix

|Feature|Simulator|Hardware|Purpose|
|-------|---------|--------|-------|
|`simulator`|✓|✗|Desktop development|
|`hardware`|✗|✓|Embedded target|
|`keyboard-input`|✓|✗|Keyboard → button mapping|
|`defmt-logging`|✓|✓|Efficient logging|
|`std`|✓|✗|Host-based testing|

### Conditional Compilation

```rust
// Platform-specific initialization
#[cfg(feature = "simulator")]
fn main() {
    let display = SimulatorDisplay::new();
    let input = KeyboardInput::new();
    run_app(display, input);
}

#[cfg(feature = "hardware")]
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());
    let display = EinkDisplay::new(p.SPI1, p.PA5, p.PA6);
    let input = ButtonInput::new(p.EXTI);
    spawner.spawn(app_task(display, input)).unwrap();
}
```

## Security Considerations

### Memory Safety

- Rust's ownership prevents use-after-free, double-free, buffer overflows
- No unsafe code except in well-reviewed HAL layer
- All unsafe blocks documented with safety invariants

### Input Validation

```rust
// Validate file paths
pub fn is_valid_path(path: &str) -> bool {
    path.len() <= 128
        && !path.contains("..")
        && path.starts_with('/')
}

// Validate metadata
pub fn validate_track_metadata(meta: &Metadata) -> Result<()> {
    ensure!(meta.sample_rate > 0, "Invalid sample rate");
    ensure!(meta.channels > 0 && meta.channels <= 2, "Invalid channels");
    Ok(())
}
```

### Resource Limits

```rust
// Prevent resource exhaustion
const MAX_PLAYLIST_SIZE: usize = 1000;
const MAX_FILE_SIZE: u64 = 500 * 1024 * 1024; // 500MB
const MAX_OPEN_FILES: usize = 4;
```

## Extensibility Points

### Adding New Audio Codecs

1. Implement `AudioDecoder` trait in `playback` crate
2. Register decoder in codec factory
3. Add format detection logic

### Adding New Display Types

1. Implement `DisplayDriver` trait in `platform/display`
2. Add to conditional compilation in firmware
3. No changes needed in UI crate

### Adding New Input Devices

1. Implement `InputDevice` trait in `platform/input`
2. Map hardware events to `InputEvent` enum
3. Input handling code remains unchanged

## Testing Architecture

See `docs/TESTING.md` for comprehensive testing strategy.

**Quick Summary:**
- Unit tests: `cargo test` on host
- Integration tests: Mock HAL implementations
- Simulator tests: Visual validation
- HIL tests: `probe-rs` on hardware

## Future Considerations

### Potential Enhancements

- **Multi-core**: Use second Cortex-M7 core for DSP processing
- **USB Audio**: USB device class for PC connectivity
- **Streaming**: Network audio streaming (Spotify Connect, AirPlay)
- **DSP Effects**: Parametric EQ, crossfade, gapless playback
- **Power Management**: Dynamic voltage/frequency scaling

### Scalability

The architecture scales to:
- More audio formats (add decoder implementations)
- Different display technologies (add DisplayDriver impl)
- Alternative storage (implement Storage trait)
- Different MCU families (swap embassy-stm32 for embassy-nrf)

## References

- [Clean Architecture (Bob Martin)](https://blog.cleancoder.com/uncle-bob/2012/08/13/the-clean-architecture.html)
- [Vertical Slice Architecture](https://www.jimmybogard.com/vertical-slice-architecture/)
- [Embassy Framework](https://embassy.dev/)
- [Embedded Rust Book](https://docs.rust-embedded.org/book/)
- [Domain-Driven Design](https://martinfowler.com/bliki/DomainDrivenDesign.html)
