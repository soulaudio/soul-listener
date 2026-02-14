# Testing Strategy

## Testing Philosophy

**Pyramid of Testing:**
```
        ▲
       ╱│╲
      ╱ │ ╲       Small number of HIL tests
     ╱  │  ╲      (Hardware-In-Loop, on device)
    ╱───┼───╲
   ╱    │    ╲    Medium number of integration tests
  ╱     │     ╲   (Mock HAL, on host)
 ╱──────┼──────╲
╱       │       ╲ Large number of unit tests
────────┴────────  (Pure logic, on host)
```

**Goals:**
1. **Fast Feedback**: Most tests run in <1s on host
2. **High Coverage**: >80% code coverage for business logic
3. **Confidence**: Integration tests verify abstractions work
4. **Validation**: HIL tests confirm hardware behavior

## Test Levels

### Level 1: Unit Tests (Host-Based)

**Target**: Pure business logic with no hardware dependencies

**Setup:**
```toml
# Cargo.toml for library crates
[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }  # For async tests

[lib]
# Enable std for tests
```

```rust
// lib.rs
#![cfg_attr(not(test), no_std)]

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_track_metadata_parsing() {
        let raw_metadata = b"...";
        let metadata = parse_metadata(raw_metadata).unwrap();

        assert_eq!(metadata.artist, "Artist Name");
        assert_eq!(metadata.title, "Track Title");
        assert_eq!(metadata.sample_rate, 44100);
    }

    #[tokio::test]
    async fn test_async_logic() {
        let result = async_function().await;
        assert!(result.is_ok());
    }
}
```

**What to Test:**
- Metadata parsing
- State machine transitions
- Buffer management logic
- Audio format detection
- Playlist operations
- File path validation

**Run:**
```bash
cargo test --lib
```

### Level 2: Integration Tests (Mock HAL)

**Target**: Feature interactions using mock hardware implementations

**Setup:**
```rust
// tests/integration/mod.rs
use platform::{DisplayDriver, InputDevice, Storage};

// Mock implementations
struct MockDisplay {
    refresh_count: usize,
    last_draw: Vec<DrawOperation>,
}

impl DisplayDriver for MockDisplay {
    type Error = core::convert::Infallible;

    async fn refresh_full(&mut self) -> Result<(), Self::Error> {
        self.refresh_count += 1;
        Ok(())
    }

    // ... other methods
}

struct MockInput {
    events: VecDeque<InputEvent>,
}

impl InputDevice for MockInput {
    async fn wait_for_event(&mut self) -> InputEvent {
        self.events.pop_front().unwrap_or(InputEvent::ButtonPress(Button::Menu))
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        self.events.pop_front()
    }
}

struct MockStorage {
    files: HashMap<String, Vec<u8>>,
}

impl Storage for MockStorage {
    // ... implement with in-memory data
}
```

**Test Example:**
```rust
#[tokio::test]
async fn test_playback_flow() {
    // Setup mocks
    let mut display = MockDisplay::new();
    let mut input = MockInput::new();
    let mut storage = MockStorage::new();

    // Add test file
    storage.add_file("/music/test.flac", include_bytes!("test.flac"));

    // Queue input events
    input.add_event(InputEvent::ButtonPress(Button::Play));

    // Create player with mocks
    let mut player = MusicPlayer::new(display, input, storage);

    // Run test scenario
    player.initialize().await.unwrap();
    player.load_track("/music/test.flac").await.unwrap();
    player.start_playback().await.unwrap();

    // Verify behavior
    assert_eq!(display.refresh_count, 2);  // Init + playback start
    assert!(player.is_playing());
}
```

**What to Test:**
- Feature workflows end-to-end
- Error handling across boundaries
- State synchronization between components
- Channel communication patterns
- Timeout behaviors

**Run:**
```bash
cargo test --test integration
```

### Level 3: Simulator Tests (Visual Validation)

**Target**: UI rendering and user interactions

**Setup:**
```rust
// simulator/src/main.rs
use embedded_graphics_simulator::{SimulatorDisplay, Window};
use platform::SimulatorInput;

fn main() {
    let mut display = SimulatorDisplay::new(Size::new(400, 300));
    let mut input = SimulatorInput::new();
    let mut window = Window::new("DAP Simulator", &output_settings);

    loop {
        // Update UI
        update_ui(&mut display, &app_state);

        // Show in window
        window.update(&display);

        // Handle events
        if let Some(event) = window.poll_event() {
            input.handle_keyboard(event);
        }
    }
}
```

**Manual Test Cases:**
1. **Library Navigation**
   - Scroll through tracks
   - Enter/exit folders
   - Search functionality

2. **Playback Controls**
   - Play/Pause
   - Next/Previous track
   - Seek within track
   - Volume control

3. **Visual Appearance**
   - Text rendering clarity
   - Icon alignment
   - Progress bar updates
   - Album art display

4. **E-ink Simulation**
   - Ghosting effects
   - Refresh modes (full vs partial)
   - Update latency

**Run:**
```bash
cargo run --bin simulator
```

### Level 4: Hardware-In-Loop Tests

**Target**: Final validation on actual hardware

**Setup:**
```toml
# Cargo.toml
[dev-dependencies]
embedded-test = "0.4"
defmt-test = "0.3"
```

**Test Example:**
```rust
// tests/hardware/display.rs
#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

use embedded_test::tests;
use embassy_stm32::Config;

#[tests]
mod tests {
    use super::*;

    #[init]
    async fn init() -> HardwareContext {
        let p = embassy_stm32::init(Config::default());

        let spi = Spi::new(/* ... */);
        let display = EinkDisplay::new(spi, /* ... */);

        HardwareContext { display }
    }

    #[test]
    async fn test_display_init(ctx: &mut HardwareContext) {
        ctx.display.init().await.unwrap();
        defmt::assert!(ctx.display.is_ready());
    }

    #[test]
    async fn test_display_refresh(ctx: &mut HardwareContext) {
        ctx.display.clear();
        ctx.display.refresh_full().await.unwrap();

        // Visual verification required
        defmt::info!("Display should be clear");
    }

    #[test]
    async fn test_i2s_audio_output(ctx: &mut HardwareContext) {
        // Generate test tone
        let samples = generate_tone(440.0, 44100, 1.0);

        ctx.codec.init(AudioConfig {
            sample_rate: 44100,
            channels: 2,
            bit_depth: 16,
        }).await.unwrap();

        ctx.codec.write_samples(&samples).await.unwrap();

        // Audio should be audible through headphones
    }
}
```

**Run:**
```bash
# Flash and run tests on device
cargo test --test hardware --features embedded-test

# With probe-rs
probe-rs test --chip STM32H743ZITx
```

**What to Test:**
- Peripheral initialization
- DMA transfers
- Interrupt handling
- Real timing constraints
- Power consumption
- Hardware-specific edge cases

## Test-Driven Development (TDD) Workflow

### 1. Write Failing Test

```rust
#[test]
fn test_parse_flac_metadata() {
    let data = include_bytes!("fixtures/test.flac");
    let metadata = FlacDecoder::parse_metadata(data).unwrap();

    assert_eq!(metadata.sample_rate, 44100);
    assert_eq!(metadata.channels, 2);
    // This will fail - not implemented yet
}
```

### 2. Implement Minimum Code

```rust
pub struct FlacDecoder;

impl FlacDecoder {
    pub fn parse_metadata(data: &[u8]) -> Result<Metadata, DecodeError> {
        // Minimal implementation to pass test
        Ok(Metadata {
            sample_rate: 44100,
            channels: 2,
            ..Default::default()
        })
    }
}
```

### 3. Refactor

```rust
pub fn parse_metadata(data: &[u8]) -> Result<Metadata, DecodeError> {
    // Proper implementation
    let mut reader = BitReader::new(data);

    // Verify FLAC signature
    if reader.read_bytes(4)? != b"fLaC" {
        return Err(DecodeError::InvalidFormat);
    }

    // Parse metadata blocks
    let metadata = parse_streaminfo(&mut reader)?;

    Ok(metadata)
}
```

### 4. Repeat for Next Feature

## Continuous Integration

### GitHub Actions Workflow

```yaml
# .github/workflows/ci.yml
name: CI

on:
  push:
    branches: [ main, develop ]
  pull_request:
    branches: [ main ]

jobs:
  format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt
      - run: cargo fmt --all -- --check

  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy
      - run: cargo clippy --all-targets --all-features -- -D warnings

  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Run tests
        run: cargo test --all-features

  build-embedded:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: thumbv7em-none-eabihf
      - name: Build for target
        run: |
          cargo build --release --target thumbv7em-none-eabihf --no-default-features --features hardware
      - name: Check binary size
        run: |
          arm-none-eabi-size target/thumbv7em-none-eabihf/release/firmware

  coverage:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - name: Install cargo-llvm-cov
        uses: taiki-e/install-action@cargo-llvm-cov
      - name: Generate coverage
        run: cargo llvm-cov --all-features --workspace --lcov --output-path lcov.info
      - name: Upload to codecov
        uses: codecov/codecov-action@v3
        with:
          files: lcov.info
```

## Test Data Management

### Fixtures

```
tests/
├── fixtures/
│   ├── audio/
│   │   ├── test_44100_stereo.flac
│   │   ├── test_48000_mono.mp3
│   │   └── test_96000_stereo.wav
│   ├── metadata/
│   │   ├── valid_metadata.json
│   │   └── invalid_metadata.json
│   └── images/
│       └── album_art.jpg
└── integration/
    └── playback_tests.rs
```

**Usage:**
```rust
#[test]
fn test_flac_decoding() {
    let data = include_bytes!("../fixtures/audio/test_44100_stereo.flac");
    let decoder = FlacDecoder::new();
    let samples = decoder.decode(data).unwrap();

    assert_eq!(samples.len(), EXPECTED_SAMPLE_COUNT);
}
```

## Property-Based Testing

For complex logic like DSP algorithms:

```toml
[dev-dependencies]
proptest = "1.0"
```

```rust
use proptest::prelude::*;

proptest! {
    #[test]
    fn test_biquad_filter_stability(
        freq in 20.0f32..20000.0,
        q in 0.1f32..10.0,
    ) {
        let filter = BiquadFilter::lowpass(44100.0, freq, q);

        // Generate random input
        let input: Vec<f32> = (0..1000).map(|_| rand::random::<f32>() * 2.0 - 1.0).collect();

        // Filter should not produce NaN or infinity
        let output = filter.process(&input);
        prop_assert!(output.iter().all(|&x| x.is_finite()));
    }

    #[test]
    fn test_sample_rate_conversion(
        input_rate in 8000u32..192000,
        output_rate in 8000u32..192000,
    ) {
        let resampler = Resampler::new(input_rate, output_rate);

        let input_samples = vec![0.0f32; 1000];
        let output = resampler.process(&input_samples).unwrap();

        // Output length should match expected ratio
        let expected_len = (1000.0 * output_rate as f64 / input_rate as f64) as usize;
        prop_assert!((output.len() as isize - expected_len as isize).abs() < 10);
    }
}
```

## Benchmarking

For performance-critical code:

```toml
[dev-dependencies]
criterion = "0.5"

[[bench]]
name = "audio_processing"
harness = false
```

```rust
// benches/audio_processing.rs
use criterion::{black_box, criterion_group, criterion_main, Criterion};

fn benchmark_flac_decode(c: &mut Criterion) {
    let data = include_bytes!("../tests/fixtures/audio/test.flac");

    c.bench_function("flac_decode", |b| {
        b.iter(|| {
            let decoder = FlacDecoder::new();
            decoder.decode(black_box(data))
        })
    });
}

fn benchmark_biquad_filter(c: &mut Criterion) {
    let filter = BiquadFilter::lowpass(44100.0, 1000.0, 0.707);
    let input = vec![0.5f32; 4096];

    c.bench_function("biquad_process_4096", |b| {
        b.iter(|| filter.process(black_box(&input)))
    });
}

criterion_group!(benches, benchmark_flac_decode, benchmark_biquad_filter);
criterion_main!(benches);
```

## Code Coverage

```bash
# Generate coverage report
cargo llvm-cov --open

# Generate for specific test
cargo llvm-cov --test integration --open

# Generate lcov for CI
cargo llvm-cov --lcov --output-path coverage.lcov
```

**Coverage Goals:**
- Business logic: >90%
- HAL abstractions: >80%
- Integration tests: >70%
- Overall project: >80%

## Debugging Tests

### Using defmt for Logging

```rust
use defmt::*;

#[test]
async fn test_with_logging() {
    info!("Starting test");

    let result = function_under_test().await;

    debug!("Result: {:?}", result);

    assert!(result.is_ok());
}
```

### Visual Debugging with Simulator

```rust
// Run simulator with specific test state
#[cfg(feature = "simulator")]
fn main() {
    env_logger::init();

    let mut app = App::new();

    // Load specific test scenario
    app.load_test_scenario("playback_error");

    app.run();
}
```

## Test Organization Best Practices

1. **Separate concerns**: Unit tests in `src/`, integration tests in `tests/`
2. **Use modules**: Organize related tests in modules
3. **Name tests clearly**: `test_feature_when_condition_then_expected`
4. **Use fixtures**: Shared test data in `tests/fixtures/`
5. **Document test intent**: Explain what and why, not how

## Common Pitfalls

### ❌ Testing Implementation Details

```rust
// Bad: Testing internal state
#[test]
fn test_internal_buffer() {
    let player = Player::new();
    assert_eq!(player.internal_buffer.len(), 0);  // Tightly coupled
}
```

### ✅ Testing Behavior

```rust
// Good: Testing observable behavior
#[test]
async fn test_playback_starts() {
    let mut player = Player::new();
    player.load_track("test.flac").await.unwrap();
    player.play().await.unwrap();

    assert!(player.is_playing());  // Public API
}
```

### ❌ Flaky Tests (Time-Dependent)

```rust
// Bad: May fail due to timing
#[tokio::test]
async fn test_with_delay() {
    start_operation();
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert!(is_complete());  // Race condition
}
```

### ✅ Deterministic Tests

```rust
// Good: Wait for explicit signal
#[tokio::test]
async fn test_completion() {
    let (tx, rx) = oneshot::channel();
    start_operation(tx);
    rx.await.unwrap();
    assert!(is_complete());  // Deterministic
}
```

## Test Maintenance

- **Keep tests fast**: Unit tests <10ms, integration <100ms
- **Keep tests independent**: No shared mutable state
- **Keep tests readable**: Clear arrange-act-assert structure
- **Update tests with code**: Tests are first-class code
- **Delete obsolete tests**: Remove when features change

## Resources

- [Rust Testing Guide](https://doc.rust-lang.org/book/ch11-00-testing.html)
- [Embedded Testing](https://ferrous-systems.com/blog/test-embedded-app/)
- [Property-Based Testing](https://www.lpalmieri.com/posts/an-introduction-to-property-based-testing-in-rust/)
- [embedded-test](https://github.com/probe-rs/embedded-test)
- [defmt-test](https://defmt.ferrous-systems.com/)
