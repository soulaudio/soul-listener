# Embedded Rust Best Practices

This document consolidates industry-standard best practices for embedded Rust development based on the Embedded Rust Book, production projects, and community wisdom.

## Code Organization

### Module Structure

**✅ DO: Organize by domain/feature**
```rust
// Good structure
crates/
├── playback/         # Complete playback feature
├── ui/               # UI feature with all layers
└── library/          # Library management feature
```

**❌ DON'T: Organize by technical layer**
```rust
// Avoid this
crates/
├── models/           # Data structures
├── services/         # Business logic
└── drivers/          # Hardware access
```

### Visibility

**Use appropriate visibility levels:**
```rust
pub mod sensor;           // Public API
mod driver;               // Internal implementation
pub(crate) mod util;      // Shared within crate
pub(super) mod helper;    // Parent module only
```

**Make minimal API surface:**
```rust
// lib.rs - Only expose what's needed
pub use playback::Player;
pub use ui::Screen;

// Internal details stay private
mod internal {
    pub(crate) struct InternalState;
}
```

## Memory Management

### Static Allocation

**✅ DO: Use `static_cell` for runtime-init statics**
```rust
use static_cell::StaticCell;

static BUFFER: StaticCell<[u8; 1024]> = StaticCell::new();

fn init() {
    let buf = BUFFER.init([0u8; 1024]);
    // Use buf as &'static mut [u8]
}
```

**✅ DO: Use `heapless` for collections**
```rust
use heapless::{Vec, String};

// Fixed capacity known at compile time
let mut playlist: Vec<TrackId, 100> = Vec::new();
let mut name: String<64> = String::new();
```

**❌ DON'T: Use dynamic allocation without justification**
```rust
// Avoid in embedded contexts
use alloc::vec::Vec;
let mut list = Vec::new();  // Can fail at runtime!
```

### Buffer Sizing

**✅ DO: Use const generics for compile-time sizing**
```rust
struct AudioBuffer<const N: usize> {
    samples: [i16; N],
    write_pos: usize,
}

// Size known at compile time
type PlaybackBuffer = AudioBuffer<4096>;
```

**✅ DO: Document memory requirements**
```rust
/// Audio processing buffer.
///
/// Memory requirement: 8KB (4096 samples × 2 bytes)
pub struct ProcessingBuffer {
    data: [i16; 4096],
}
```

### Stack vs Static

**Stack**: Local variables, temporary data
```rust
fn process_audio() {
    let temp_buffer = [0i16; 256];  // Small, temporary
    // ...
}
```

**Static**: Long-lived, shared data
```rust
static DMA_BUFFER: StaticCell<[u8; 8192]> = StaticCell::new();
```

## Error Handling

### Library Errors (thiserror)

**✅ DO: Define typed errors in libraries**
```rust
use thiserror_no_std::Error;

#[derive(Error, Debug)]
pub enum StorageError {
    #[error("File not found: {0}")]
    NotFound(String),

    #[error("I/O error")]
    Io(#[from] IoError),

    #[error("Invalid path")]
    InvalidPath,
}
```

### Application Errors (anyhow)

**✅ DO: Use `anyhow` in application code**
```rust
use anyhow::{Context, Result};

pub async fn load_track(path: &str) -> Result<Track> {
    let file = open_file(path)
        .context("Failed to open audio file")?;

    let metadata = parse_metadata(&file)
        .context("Invalid track metadata")?;

    Ok(Track { file, metadata })
}
```

### Error Propagation

**✅ DO: Use `?` operator for propagation**
```rust
pub async fn process() -> Result<(), Error> {
    let data = read_sensor().await?;
    let result = process_data(data)?;
    write_output(result).await?;
    Ok(())
}
```

**❌ DON'T: Use `unwrap()` or `expect()` in production**
```rust
// NEVER in production code
let value = risky_operation().unwrap();  // Will panic!

// Use proper error handling
let value = risky_operation().map_err(|e| /* handle */)?;
```

## no_std Limitations

### What's Available

✅ **Core library (`core`)**
- Primitives: integers, floats, bools
- Collections: slices, arrays, Option, Result
- Traits: Iterator, From, Into
- Atomics and SIMD

✅ **With `alloc`**
- Vec, String, Box, Arc, Rc
- BTreeMap, BTreeSet (no HashMap/HashSet)

### What's NOT Available

❌ **Standard library features**
- Thread spawning (`std::thread`)
- File I/O (`std::fs`)
- Networking (`std::net`)
- Environment variables
- Default heap allocator
- Panic runtime

### Alternatives

| std Feature | no_std Alternative |
|-------------|-------------------|
| `std::collections::HashMap` | `heapless::FnvIndexMap` or `BTreeMap` |
| `std::thread` | RTIC or Embassy tasks |
| `std::fs::File` | `embedded-sdmmc` |
| `println!` | `defmt::info!` or `panic_probe` |
| `std::time` | `embassy-time` |

## Type Safety Patterns

### Newtype Pattern

**✅ DO: Wrap primitives for type safety**
```rust
pub struct Celsius(f32);
pub struct Fahrenheit(f32);
pub struct Hertz(u32);

impl Celsius {
    pub fn new(value: f32) -> Self {
        assert!(value >= -273.15);
        Self(value)
    }

    pub fn to_fahrenheit(self) -> Fahrenheit {
        Fahrenheit(self.0 * 1.8 + 32.0)
    }
}

// Prevents mixing units
fn set_temperature(temp: Celsius) {
    // Compiler prevents: set_temperature(Fahrenheit(72.0));
}
```

### Typestate Pattern

**✅ DO: Encode states in types**
```rust
pub struct Pin<MODE> {
    pin_num: u8,
    _mode: PhantomData<MODE>,
}

pub struct Input;
pub struct Output;

impl Pin<Input> {
    pub fn read(&self) -> bool { /* ... */ }

    pub fn into_output(self) -> Pin<Output> {
        // Configure pin as output
        Pin {
            pin_num: self.pin_num,
            _mode: PhantomData,
        }
    }
}

impl Pin<Output> {
    pub fn set_high(&mut self) { /* ... */ }
    pub fn set_low(&mut self) { /* ... */ }
    // read() not available - compile error if called
}

// Usage
let pin = pins.pa5;  // Pin<Input>
// pin.set_high();   // ERROR: method not found
let mut pin = pin.into_output();
pin.set_high();      // OK
```

### Const Generics

**✅ DO: Use for compile-time validation**
```rust
struct Buffer<T, const N: usize> {
    data: [T; N],
}

impl<T: Copy, const N: usize> Buffer<T, N> {
    pub const fn new(init: T) -> Self {
        Self {
            data: [init; N],
        }
    }

    pub const fn capacity(&self) -> usize {
        N  // Known at compile time
    }
}

// Size checked at compile time
type AudioBuffer = Buffer<i16, 4096>;
```

## Concurrency Patterns

### Embassy Task Organization

**✅ DO: One task per major responsibility**
```rust
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    spawner.spawn(audio_task()).unwrap();
    spawner.spawn(ui_task()).unwrap();
    spawner.spawn(input_task()).unwrap();
}

#[embassy_executor::task]
async fn audio_task() {
    // Audio processing loop
}

#[embassy_executor::task]
async fn ui_task() {
    // Display update loop
}

#[embassy_executor::task]
async fn input_task() {
    // Input handling loop
}
```

### Inter-Task Communication

**✅ DO: Use Embassy channels for messages**
```rust
use embassy_sync::channel::Channel;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;

static CMD_CHANNEL: Channel<CriticalSectionRawMutex, Command, 8> = Channel::new();

// Sender
CMD_CHANNEL.send(Command::Play).await;

// Receiver
let cmd = CMD_CHANNEL.receive().await;
```

**✅ DO: Use Signal for state updates**
```rust
use embassy_sync::signal::Signal;

static VOLUME: Signal<CriticalSectionRawMutex, u8> = Signal::new();

// Writer
VOLUME.signal(75);

// Reader
let vol = VOLUME.wait().await;
```

### Resource Sharing

**✅ DO: Use Mutex for shared mutable state**
```rust
use embassy_sync::mutex::Mutex;

static STATE: Mutex<CriticalSectionRawMutex, AppState> = Mutex::new(AppState::new());

async fn update_state() {
    let mut state = STATE.lock().await;
    state.track_position += 1;
    // Mutex released when `state` goes out of scope
}
```

**❌ DON'T: Share raw mutable statics**
```rust
// UNSAFE and wrong
static mut COUNTER: u32 = 0;

async fn increment() {
    unsafe {
        COUNTER += 1;  // Data race!
    }
}
```

## Peripheral Access

### Singleton Pattern

**✅ DO: Use take() pattern for peripherals**
```rust
let peripherals = embassy_stm32::init(Default::default());

// Each peripheral can only be taken once
let spi = Spi::new(peripherals.SPI1, /* ... */);
// let spi2 = Spi::new(peripherals.SPI1, /* ... */);  // ERROR: value used after move
```

### Safe Register Access

**✅ DO: Use PAC/HAL abstractions**
```rust
// Good: Type-safe HAL
let mut led = Output::new(p.PA5, Level::Low, Speed::Low);
led.set_high();

// Avoid: Direct register manipulation
// unsafe { (*GPIOA::ptr()).odr.write(|w| w.bits(0x20)); }
```

### DMA Transfers

**✅ DO: Use async for DMA**
```rust
// DMA transfer with async/await
let buf = [0u8; 512];
spi.read(&mut buf).await?;  // CPU sleeps during transfer

// No manual interrupt handling needed
```

## Performance Optimization

### Compiler Optimization

```toml
[profile.release]
opt-level = "z"          # Optimize for size
lto = true               # Link-time optimization
codegen-units = 1        # Better optimization
overflow-checks = true   # Keep safety checks
```

### Inline Hints

**✅ DO: Use inline judiciously**
```rust
#[inline]
pub fn fast_path() {
    // Small, frequently called function
}

#[inline(never)]
pub fn slow_path() {
    // Large function, rarely called
}
```

### Loop Optimization

**✅ DO: Help the optimizer**
```rust
// Explicit length known to optimizer
for i in 0..samples.len() {
    samples[i] = process(samples[i]);
}

// Better: iterator with known size hint
samples.iter_mut().for_each(|s| *s = process(*s));
```

### SIMD When Appropriate

```rust
#[cfg(target_feature = "neon")]
use core::arch::arm::*;

#[cfg(target_feature = "neon")]
unsafe fn process_simd(data: &mut [f32]) {
    // Use NEON intrinsics for parallelism
}
```

## Documentation

### Module Documentation

**✅ DO: Document modules with `//!`**
```rust
//! Audio playback engine.
//!
//! This module provides the core playback functionality including:
//! - Format decoding (FLAC, MP3, WAV)
//! - Sample rate conversion
//! - Buffer management
//!
//! # Example
//!
//! ```no_run
//! let mut player = Player::new();
//! player.load_track("/music/song.flac").await?;
//! player.play().await?;
//! ```
```

### Function Documentation

**✅ DO: Document with `///`**
```rust
/// Loads a track from the filesystem.
///
/// # Arguments
///
/// * `path` - Absolute path to the audio file
///
/// # Errors
///
/// Returns `StorageError::NotFound` if the file doesn't exist.
/// Returns `DecodeError::UnsupportedFormat` if the format isn't recognized.
///
/// # Example
///
/// ```no_run
/// # use player::Player;
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut player = Player::new();
/// player.load_track("/music/song.flac").await?;
/// # Ok(())
/// # }
/// ```
pub async fn load_track(&mut self, path: &str) -> Result<Track, Error> {
    // ...
}
```

### Safety Documentation

**✅ DO: Document unsafe code**
```rust
/// Reads a value from the DMA buffer.
///
/// # Safety
///
/// Must not be called while DMA transfer is in progress.
/// Caller must ensure DMA controller has completed the transfer.
pub unsafe fn read_dma_buffer(&self) -> &[u8] {
    &self.buffer
}
```

## Build Configuration

### Feature Flags

**✅ DO: Use features for optional functionality**
```toml
[features]
default = ["defmt-logging"]
simulator = ["embedded-graphics-simulator"]
hardware = ["embassy-stm32"]
defmt-logging = ["defmt", "defmt-rtt"]
std = []  # For host-based testing
```

**✅ DO: Document features**
```rust
//! # Features
//!
//! - `simulator`: Enables desktop simulator
//! - `hardware`: Enables STM32 hardware support
//! - `defmt-logging`: Efficient logging via defmt
#![cfg_attr(not(feature = "std"), no_std)]
```

### Conditional Compilation

**✅ DO: Use cfg for platform-specific code**
```rust
#[cfg(feature = "simulator")]
mod simulator {
    pub fn init() {
        println!("Simulator mode");
    }
}

#[cfg(feature = "hardware")]
mod hardware {
    pub fn init() {
        // Initialize hardware
    }
}

#[cfg(not(any(feature = "simulator", feature = "hardware")))]
compile_error!("Must enable either 'simulator' or 'hardware' feature");
```

## Testing

**✅ DO: Test hardware-independent logic**
```rust
#![cfg_attr(not(test), no_std)]

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_state_transition() {
        let state = PlaybackState::Stopped;
        let state = state.handle_event(Event::Play);
        assert!(matches!(state, PlaybackState::Playing { .. }));
    }
}
```

**✅ DO: Use mocks for HAL testing**
```rust
#[cfg(test)]
mod tests {
    struct MockDisplay;

    impl DisplayDriver for MockDisplay {
        type Error = ();
        async fn refresh(&mut self) -> Result<(), ()> { Ok(()) }
    }

    #[test]
    async fn test_ui_update() {
        let mut display = MockDisplay;
        update_ui(&mut display).await.unwrap();
    }
}
```

## Code Style

### Naming Conventions

- Types: `PascalCase`
- Functions: `snake_case`
- Constants: `SCREAMING_SNAKE_CASE`
- Modules: `snake_case`

**✅ DO: Use descriptive names**
```rust
// Good
let track_duration_ms = 180_000;
let sample_rate_hz = 44100;

// Bad
let d = 180_000;
let sr = 44100;
```

### Magic Numbers

**✅ DO: Use named constants**
```rust
const SAMPLE_RATE_HZ: u32 = 44100;
const BUFFER_SIZE_SAMPLES: usize = 4096;
const MAX_PLAYLIST_SIZE: usize = 1000;

fn configure() {
    codec.set_sample_rate(SAMPLE_RATE_HZ);
}
```

**❌ DON'T: Use unnamed literals**
```rust
// Bad
codec.set_sample_rate(44100);  // What is this number?
```

### Function Length

**✅ DO: Keep functions focused**
```rust
// Good: Single responsibility
async fn load_track(path: &str) -> Result<Track> {
    let file = open_file(path)?;
    let metadata = parse_metadata(&file)?;
    Ok(Track { file, metadata })
}
```

**❌ DON'T: Create god functions**
```rust
// Bad: Too many responsibilities
async fn do_everything() {
    // 500 lines of code doing many unrelated things
}
```

## Safety

### Unsafe Code

**✅ DO: Minimize unsafe**
```rust
// Confine unsafe to small, well-reviewed functions
pub fn create_buffer() -> &'static mut [u8; 1024] {
    unsafe {
        static mut BUFFER: [u8; 1024] = [0; 1024];
        &mut BUFFER
    }
}

// Use safe wrapper
pub fn safe_api() {
    let buffer = create_buffer();
    // Rest of code is safe
}
```

**✅ DO: Document safety invariants**
```rust
/// # Safety
///
/// Caller must ensure:
/// - Buffer is properly aligned
/// - Buffer size is at least `len` bytes
/// - No other references to buffer exist
pub unsafe fn write_dma(buffer: *mut u8, len: usize) {
    // ...
}
```

### Resource Ownership

**✅ DO: Use RAII pattern**
```rust
pub struct SpiTransaction<'a> {
    spi: &'a mut Spi,
}

impl<'a> SpiTransaction<'a> {
    pub fn new(spi: &'a mut Spi) -> Self {
        spi.acquire();
        Self { spi }
    }
}

impl Drop for SpiTransaction<'_> {
    fn drop(&mut self) {
        self.spi.release();
    }
}

// Automatically releases on drop
let transaction = SpiTransaction::new(&mut spi);
```

## Power Management

**✅ DO: Use async for automatic sleep**
```rust
#[embassy_executor::task]
async fn power_efficient_task() {
    loop {
        // Wait for event (CPU sleeps automatically)
        let event = input.wait_for_event().await;

        // Process
        handle_event(event);

        // CPU sleeps again while waiting
    }
}
```

**✅ DO: Disable unused peripherals**
```rust
pub fn optimize_power() {
    // Disable unused clocks
    RCC.apb1enr.modify(|_, w| w.tim2en().disabled());

    // Lower voltage when possible
    PWR.cr1.modify(|_, w| w.vos().scale2());
}
```

## Common Pitfalls

### ❌ Integer Overflow

```rust
// Bad: Can overflow
let result = a + b;

// Good: Checked arithmetic
let result = a.checked_add(b).ok_or(Error::Overflow)?;

// Or enable overflow checks in release
// [profile.release]
// overflow-checks = true
```

### ❌ Blocking in Async

```rust
// Bad: Blocks executor
async fn bad_async() {
    thread::sleep(Duration::from_secs(1));  // Blocks!
}

// Good: Async sleep
async fn good_async() {
    Timer::after_secs(1).await;  // Yields to executor
}
```

### ❌ Large Stack Allocations

```rust
// Bad: Large stack allocation
fn process() {
    let buffer = [0u8; 100_000];  // Stack overflow risk!
}

// Good: Static allocation
static BUFFER: StaticCell<[u8; 100_000]> = StaticCell::new();

fn process() {
    let buffer = BUFFER.init_with(|| [0u8; 100_000]);
}
```

## Tools

### Essential Tools

```bash
# Code formatting
cargo fmt

# Linting
cargo clippy -- -D warnings

# Security audit
cargo audit

# Unused dependencies
cargo machete

# Code coverage
cargo llvm-cov

# Binary size analysis
cargo bloat --release
```

### Continuous Integration

Set up automated checks:
- Format check: `cargo fmt --check`
- Clippy: `cargo clippy --all-targets -- -D warnings`
- Tests: `cargo test --all-features`
- Build for target: `cargo build --target thumbv7em-none-eabihf`

## Resources

- [Embedded Rust Book](https://docs.rust-embedded.org/book/)
- [The Rust Performance Book](https://nnethercote.github.io/perf-book/)
- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Effective Rust](https://www.lurklurk.org/effective-rust/)
- [Embassy Framework](https://embassy.dev/)
