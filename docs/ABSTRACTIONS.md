# Hardware Abstraction Layer (HAL) Guide

## Philosophy

The HAL provides **trait-based abstractions** that enable:
1. **Testability**: Mock implementations for host-based testing
2. **Portability**: Swap hardware without changing application code
3. **Safety**: Compile-time verification of hardware usage
4. **Zero-Cost**: No runtime overhead from abstractions

## Core Abstraction Patterns

### Pattern 1: Trait-Based Interfaces

Define hardware capabilities as traits, not concrete types:

```rust
// ✓ Good: Trait-based abstraction
pub trait DisplayDriver: embedded_graphics::DrawTarget {
    type Error;

    async fn refresh_full(&mut self) -> Result<(), Self::Error>;
    async fn refresh_partial(&mut self) -> Result<(), Self::Error>;
    async fn sleep(&mut self) -> Result<(), Self::Error>;
}

// ✗ Bad: Concrete type coupling
pub struct EinkDisplay { /* ... */ }

impl EinkDisplay {
    pub async fn refresh(&mut self) { /* ... */ }
}
```

### Pattern 2: Generic Over Traits

Application code uses generics, not concrete types:

```rust
// Application code
pub struct MusicPlayer<D, I, S>
where
    D: DisplayDriver,
    I: InputDevice,
    S: Storage,
{
    display: D,
    input: I,
    storage: S,
}

// Works with ANY implementation of these traits
impl<D, I, S> MusicPlayer<D, I, S>
where
    D: DisplayDriver,
    I: InputDevice,
    S: Storage,
{
    pub async fn run(&mut self) -> Result<()> {
        loop {
            let event = self.input.wait_for_event().await;
            self.handle_event(event).await?;
            self.display.refresh_partial().await?;
        }
    }
}
```

### Pattern 3: Multiple Implementations

Provide both hardware and simulator implementations:

```rust
// Hardware implementation
use epd_waveshare::epd4in2::Epd4in2;

pub struct HardwareDisplay<SPI, DC, BUSY> {
    epd: Epd4in2<SPI, DC, BUSY, RST>,
    buffer: [u8; BUFFER_SIZE],
}

impl<SPI, DC, BUSY> DisplayDriver for HardwareDisplay<SPI, DC, BUSY>
where
    SPI: embedded_hal_async::spi::SpiDevice,
    DC: embedded_hal::digital::OutputPin,
    BUSY: embedded_hal::digital::InputPin,
{
    type Error = DisplayError;

    async fn refresh_full(&mut self) -> Result<(), Self::Error> {
        self.epd.update_frame(&mut self.spi, &self.buffer).await?;
        self.epd.display_frame(&mut self.spi).await?;
        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::Error> {
        self.epd.update_partial_frame(&mut self.spi, &self.buffer).await?;
        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::Error> {
        self.epd.sleep(&mut self.spi).await?;
        Ok(())
    }
}

// Simulator implementation
use embedded_graphics_simulator::{SimulatorDisplay, Window};

pub struct SimDisplay {
    display: SimulatorDisplay<Gray4>,
    window: Option<Window>,
}

impl DisplayDriver for SimDisplay {
    type Error = core::convert::Infallible;

    async fn refresh_full(&mut self) -> Result<(), Self::Error> {
        println!("Simulator: Full refresh");
        if let Some(window) = &mut self.window {
            window.update(&self.display);
        }
        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::Error> {
        println!("Simulator: Partial refresh");
        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::Error> {
        println!("Simulator: Sleep mode");
        Ok(())
    }
}
```

## Display Abstraction

### Core Trait

```rust
use embedded_graphics::prelude::*;

pub trait DisplayDriver: DrawTarget {
    type Error: core::fmt::Debug;

    /// Perform full display refresh (clears ghosting, slow)
    async fn refresh_full(&mut self) -> Result<(), Self::Error>;

    /// Perform partial refresh (fast, may have ghosting)
    async fn refresh_partial(&mut self) -> Result<(), Self::Error>;

    /// Enter deep sleep mode
    async fn sleep(&mut self) -> Result<(), Self::Error>;

    /// Wake from sleep
    async fn wake(&mut self) -> Result<(), Self::Error>;

    /// Get display dimensions
    fn dimensions(&self) -> Size {
        self.bounding_box().size
    }
}
```

### E-Ink Specific Extensions

```rust
pub trait EinkDisplay: DisplayDriver {
    /// Set refresh mode (affects ghosting vs speed)
    fn set_refresh_mode(&mut self, mode: RefreshMode);

    /// Get current temperature (affects refresh quality)
    fn temperature(&self) -> Option<i8>;
}

#[derive(Copy, Clone, Debug)]
pub enum RefreshMode {
    Full,      // Best quality, slow
    Partial,   // Fast, some ghosting
    Fast,      // Fastest, most ghosting
}
```

### Usage Example

```rust
pub async fn update_ui<D>(display: &mut D, state: &AppState) -> Result<()>
where
    D: DisplayDriver,
{
    // Clear with background color
    display.clear(BinaryColor::Off)?;

    // Draw UI elements
    Text::new("Now Playing", Point::new(10, 10), MonoTextStyle::new(&FONT, BinaryColor::On))
        .draw(display)?;

    Circle::new(Point::new(100, 100), 50)
        .into_styled(PrimitiveStyle::with_stroke(BinaryColor::On, 2))
        .draw(display)?;

    // Refresh display
    if state.needs_full_refresh {
        display.refresh_full().await?;
    } else {
        display.refresh_partial().await?;
    }

    Ok(())
}
```

## Input Abstraction

### Core Trait

```rust
pub trait InputDevice {
    /// Wait for next input event (async, power-efficient)
    async fn wait_for_event(&mut self) -> InputEvent;

    /// Poll for event (non-blocking, returns None if no event)
    fn poll_event(&mut self) -> Option<InputEvent>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    ButtonPress(Button),
    ButtonRelease(Button),
    ButtonLongPress(Button),
    RotaryIncrement(i32),  // Positive = clockwise, negative = counter-clockwise
    Touch(Point),
    Swipe(SwipeDirection),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Button {
    Play,
    Pause,
    Next,
    Previous,
    VolumeUp,
    VolumeDown,
    Menu,
    Back,
    Select,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwipeDirection {
    Up,
    Down,
    Left,
    Right,
}
```

### Hardware Implementation (GPIO + Rotary Encoder)

```rust
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{Input, Pull};
use rotary_encoder_embedded::{RotaryEncoder, Direction};

pub struct HardwareInput {
    play_button: ExtiInput<'static>,
    next_button: ExtiInput<'static>,
    encoder: RotaryEncoder<Input<'static>, Input<'static>>,
}

impl InputDevice for HardwareInput {
    async fn wait_for_event(&mut self) -> InputEvent {
        use embassy_futures::select::{select3, Either3};

        match select3(
            self.play_button.wait_for_falling_edge(),
            self.next_button.wait_for_falling_edge(),
            self.poll_encoder(),
        ).await {
            Either3::First(_) => InputEvent::ButtonPress(Button::Play),
            Either3::Second(_) => InputEvent::ButtonPress(Button::Next),
            Either3::Third(increment) => InputEvent::RotaryIncrement(increment),
        }
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        if self.play_button.is_low() {
            Some(InputEvent::ButtonPress(Button::Play))
        } else if let Some(dir) = self.encoder.update() {
            let increment = match dir {
                Direction::Clockwise => 1,
                Direction::CounterClockwise => -1,
            };
            Some(InputEvent::RotaryIncrement(increment))
        } else {
            None
        }
    }
}

impl HardwareInput {
    async fn poll_encoder(&mut self) -> i32 {
        // Wait for encoder change via interrupt
        // Return accumulated increments
        todo!()
    }
}
```

### Simulator Implementation (Keyboard Mapping)

```rust
use winit::event::{KeyCode, VirtualKeyCode};

pub struct SimulatorInput {
    event_queue: VecDeque<InputEvent>,
}

impl InputDevice for SimulatorInput {
    async fn wait_for_event(&mut self) -> InputEvent {
        // In simulator, check keyboard every 10ms
        loop {
            if let Some(event) = self.event_queue.pop_front() {
                return event;
            }
            embassy_time::Timer::after_millis(10).await;
        }
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        self.event_queue.pop_front()
    }
}

impl SimulatorInput {
    pub fn handle_keyboard(&mut self, key: VirtualKeyCode, pressed: bool) {
        let event = match (key, pressed) {
            (VirtualKeyCode::Space, true) => InputEvent::ButtonPress(Button::Play),
            (VirtualKeyCode::Right, true) => InputEvent::ButtonPress(Button::Next),
            (VirtualKeyCode::Left, true) => InputEvent::ButtonPress(Button::Previous),
            (VirtualKeyCode::Up, true) => InputEvent::RotaryIncrement(1),
            (VirtualKeyCode::Down, true) => InputEvent::RotaryIncrement(-1),
            _ => return,
        };
        self.event_queue.push_back(event);
    }
}
```

## Audio Codec Abstraction

### Core Trait

```rust
pub trait AudioCodec {
    type Error: core::fmt::Debug;

    /// Initialize codec with sample rate and format
    async fn init(&mut self, config: AudioConfig) -> Result<(), Self::Error>;

    /// Start playback
    async fn start(&mut self) -> Result<(), Self::Error>;

    /// Stop playback
    async fn stop(&mut self) -> Result<(), Self::Error>;

    /// Set volume (0-100)
    async fn set_volume(&mut self, volume: u8) -> Result<(), Self::Error>;

    /// Write audio samples (DMA-based, returns when buffer is queued)
    async fn write_samples(&mut self, samples: &[i16]) -> Result<(), Self::Error>;
}

#[derive(Debug, Clone, Copy)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u8,
    pub bit_depth: u8,
}
```

### Hardware Implementation (WM8960 via I2S)

```rust
use embassy_stm32::sai::Sai;
use embassy_stm32::i2c::I2c;

pub struct Wm8960Codec<'d> {
    i2c: I2c<'d>,      // Control interface
    sai: Sai<'d>,      // Audio data interface
    volume: u8,
}

impl AudioCodec for Wm8960Codec<'_> {
    type Error = CodecError;

    async fn init(&mut self, config: AudioConfig) -> Result<(), Self::Error> {
        // Configure WM8960 registers via I2C
        self.write_register(Register::Reset, 0x0000).await?;
        self.write_register(Register::Power1, 0x00FF).await?;

        // Configure sample rate
        let rate_config = self.sample_rate_config(config.sample_rate)?;
        self.write_register(Register::Iface2, rate_config).await?;

        // Configure SAI for I2S
        // self.sai.set_config(...)

        Ok(())
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        self.write_register(Register::Power3, 0x00FF).await?;
        self.sai.start().await?;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.sai.stop().await?;
        self.write_register(Register::Power3, 0x0000).await?;
        Ok(())
    }

    async fn set_volume(&mut self, volume: u8) -> Result<(), Self::Error> {
        let volume = volume.min(100);
        self.volume = volume;

        // Map 0-100 to WM8960's volume range
        let reg_value = (volume as u16 * 127) / 100;
        self.write_register(Register::LeftOut1Volume, reg_value).await?;
        self.write_register(Register::RightOut1Volume, reg_value).await?;

        Ok(())
    }

    async fn write_samples(&mut self, samples: &[i16]) -> Result<(), Self::Error> {
        // Convert to bytes and write via SAI DMA
        let bytes = unsafe {
            core::slice::from_raw_parts(
                samples.as_ptr() as *const u8,
                samples.len() * 2,
            )
        };
        self.sai.write(bytes).await?;
        Ok(())
    }
}
```

### Simulator Implementation (Host Audio)

```rust
pub struct SimulatorCodec {
    // Use cpal or similar for host audio output
    volume: u8,
}

impl AudioCodec for SimulatorCodec {
    type Error = core::convert::Infallible;

    async fn init(&mut self, config: AudioConfig) -> Result<(), Self::Error> {
        println!("Sim Audio: Init {}Hz, {} channels", config.sample_rate, config.channels);
        Ok(())
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        println!("Sim Audio: Start playback");
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        println!("Sim Audio: Stop playback");
        Ok(())
    }

    async fn set_volume(&mut self, volume: u8) -> Result<(), Self::Error> {
        self.volume = volume;
        println!("Sim Audio: Volume = {}", volume);
        Ok(())
    }

    async fn write_samples(&mut self, samples: &[i16]) -> Result<(), Self::Error> {
        // In real simulator, send to host audio output
        println!("Sim Audio: Write {} samples", samples.len());
        Ok(())
    }
}
```

## Storage Abstraction

### Core Trait

```rust
pub trait Storage {
    type Error: core::fmt::Debug;
    type File: File;
    type Dir: Directory;

    /// Open file for reading
    async fn open_file(&mut self, path: &str) -> Result<Self::File, Self::Error>;

    /// Open directory for listing
    async fn open_dir(&mut self, path: &str) -> Result<Self::Dir, Self::Error>;

    /// Check if path exists
    async fn exists(&mut self, path: &str) -> Result<bool, Self::Error>;
}

pub trait File {
    type Error: core::fmt::Debug;

    /// Read from current position
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;

    /// Seek to position
    async fn seek(&mut self, pos: u64) -> Result<u64, Self::Error>;

    /// Get file size
    fn size(&self) -> u64;
}

pub trait Directory {
    type Error: core::fmt::Debug;
    type Entry: DirEntry;

    /// Read next directory entry
    async fn read_entry(&mut self) -> Result<Option<Self::Entry>, Self::Error>;
}

pub trait DirEntry {
    fn name(&self) -> &str;
    fn is_file(&self) -> bool;
    fn size(&self) -> u64;
}
```

### Hardware Implementation (SD Card via embedded-sdmmc)

```rust
use embedded_sdmmc::{VolumeManager, Volume, File as SdFile};

pub struct SdCardStorage<'a> {
    volume_mgr: VolumeManager<SdMmcDevice, Clock>,
    volume: Volume,
}

impl Storage for SdCardStorage<'_> {
    type Error = StorageError;
    type File = SdCardFile;
    type Dir = SdCardDir;

    async fn open_file(&mut self, path: &str) -> Result<Self::File, Self::Error> {
        let root = self.volume.open_root_dir()?;
        let file = self.volume.open_file_in_dir(root, path, Mode::ReadOnly)?;
        Ok(SdCardFile { file })
    }

    async fn open_dir(&mut self, path: &str) -> Result<Self::Dir, Self::Error> {
        let root = self.volume.open_root_dir()?;
        let dir = if path == "/" {
            root
        } else {
            self.volume.open_dir(root, path)?
        };
        Ok(SdCardDir { dir, volume: &mut self.volume })
    }

    async fn exists(&mut self, path: &str) -> Result<bool, Self::Error> {
        match self.open_file(path).await {
            Ok(_) => Ok(true),
            Err(StorageError::NotFound) => Ok(false),
            Err(e) => Err(e),
        }
    }
}
```

### Simulator Implementation (Host Filesystem)

```rust
use std::fs;
use std::io::Read;

pub struct HostStorage {
    root_path: PathBuf,
}

impl Storage for HostStorage {
    type Error = std::io::Error;
    type File = HostFile;
    type Dir = HostDir;

    async fn open_file(&mut self, path: &str) -> Result<Self::File, Self::Error> {
        let full_path = self.root_path.join(path.trim_start_matches('/'));
        let file = fs::File::open(full_path)?;
        Ok(HostFile { file })
    }

    async fn open_dir(&mut self, path: &str) -> Result<Self::Dir, Self::Error> {
        let full_path = self.root_path.join(path.trim_start_matches('/'));
        let entries = fs::read_dir(full_path)?;
        Ok(HostDir { entries })
    }

    async fn exists(&mut self, path: &str) -> Result<bool, Self::Error> {
        let full_path = self.root_path.join(path.trim_start_matches('/'));
        Ok(full_path.exists())
    }
}
```

## Bluetooth Abstraction

### Core Trait

```rust
pub trait BluetoothAdapter {
    type Error: core::fmt::Debug;

    /// Initialize Bluetooth adapter
    async fn init(&mut self) -> Result<(), Self::Error>;

    /// Start advertising
    async fn start_advertising(&mut self, name: &str) -> Result<(), Self::Error>;

    /// Stop advertising
    async fn stop_advertising(&mut self) -> Result<(), Self::Error>;

    /// Wait for connection
    async fn wait_for_connection(&mut self) -> Result<Connection, Self::Error>;

    /// Check if connected
    fn is_connected(&self) -> bool;
}

pub struct Connection {
    // Connection handle
}

impl Connection {
    pub async fn send(&mut self, data: &[u8]) -> Result<(), BtError> {
        // Send data over connection
        todo!()
    }

    pub async fn receive(&mut self, buf: &mut [u8]) -> Result<usize, BtError> {
        // Receive data
        todo!()
    }
}
```

### Hardware Implementation (HCI via UART)

```rust
use embassy_stm32::usart::Uart;
use trouble_host::Host;

pub struct HciBluetoothAdapter<'d> {
    uart: Uart<'d>,
    host: Host,
    connected: bool,
}

impl BluetoothAdapter for HciBluetoothAdapter<'_> {
    type Error = BtError;

    async fn init(&mut self) -> Result<(), Self::Error> {
        // Initialize HCI host
        self.host.init().await?;
        Ok(())
    }

    async fn start_advertising(&mut self, name: &str) -> Result<(), Self::Error> {
        self.host.advertise(name).await?;
        Ok(())
    }

    async fn stop_advertising(&mut self) -> Result<(), Self::Error> {
        self.host.stop_advertising().await?;
        Ok(())
    }

    async fn wait_for_connection(&mut self) -> Result<Connection, Self::Error> {
        let conn = self.host.wait_connection().await?;
        self.connected = true;
        Ok(Connection { /* ... */ })
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}
```

### Simulator Implementation (Mock)

```rust
pub struct MockBluetoothAdapter {
    connected: bool,
}

impl BluetoothAdapter for MockBluetoothAdapter {
    type Error = core::convert::Infallible;

    async fn init(&mut self) -> Result<(), Self::Error> {
        println!("BT: Init");
        Ok(())
    }

    async fn start_advertising(&mut self, name: &str) -> Result<(), Self::Error> {
        println!("BT: Advertising as '{}'", name);
        Ok(())
    }

    async fn stop_advertising(&mut self) -> Result<(), Self::Error> {
        println!("BT: Stop advertising");
        Ok(())
    }

    async fn wait_for_connection(&mut self) -> Result<Connection, Self::Error> {
        println!("BT: Simulating connection in 2s...");
        embassy_time::Timer::after_secs(2).await;
        self.connected = true;
        Ok(Connection {})
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}
```

## Best Practices

### 1. Prefer Async Traits

```rust
// ✓ Good: Async for I/O operations
pub trait Storage {
    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}

// ✗ Bad: Blocking in embedded context
pub trait Storage {
    fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error>;
}
```

### 2. Use Associated Types for Errors

```rust
// ✓ Good: Associated error type
pub trait DisplayDriver {
    type Error: core::fmt::Debug;
    async fn refresh(&mut self) -> Result<(), Self::Error>;
}

// ✗ Bad: Generic error constrains implementations
pub trait DisplayDriver {
    async fn refresh(&mut self) -> Result<(), DisplayError>;
}
```

### 3. Implement embedded-hal Traits

```rust
// Implement standard traits where applicable
impl embedded_hal::spi::SpiDevice for MyDevice {
    // Standard interface enables using with ecosystem drivers
}
```

### 4. Document Safety Invariants

```rust
impl DisplayDriver for HardwareDisplay {
    /// # Safety
    /// Must not be called while DMA transfer is in progress
    async unsafe fn refresh_unchecked(&mut self) -> Result<(), Self::Error> {
        // ...
    }
}
```

### 5. Provide Builder Pattern for Initialization

```rust
pub struct DisplayBuilder<SPI, DC, BUSY> {
    spi: Option<SPI>,
    dc: Option<DC>,
    busy: Option<BUSY>,
}

impl<SPI, DC, BUSY> DisplayBuilder<SPI, DC, BUSY> {
    pub fn new() -> Self { /* ... */ }

    pub fn with_spi(mut self, spi: SPI) -> Self {
        self.spi = Some(spi);
        self
    }

    pub fn build(self) -> Result<HardwareDisplay<SPI, DC, BUSY>, BuildError> {
        // Validate all required fields set
        Ok(HardwareDisplay { /* ... */ })
    }
}
```

## Testing Abstractions

See `docs/TESTING.md` for detailed testing strategies using these abstractions.

**Quick Example:**

```rust
#[cfg(test)]
mod tests {
    use super::*;

    struct MockDisplay {
        refresh_count: usize,
    }

    impl DisplayDriver for MockDisplay {
        type Error = ();

        async fn refresh_full(&mut self) -> Result<(), ()> {
            self.refresh_count += 1;
            Ok(())
        }

        // ... other methods
    }

    #[test]
    fn test_ui_updates() {
        let mut display = MockDisplay { refresh_count: 0 };

        // Test code using DisplayDriver trait
        update_ui(&mut display, &state).await.unwrap();

        assert_eq!(display.refresh_count, 1);
    }
}
```

## Troubleshooting

### "Trait not implemented" errors

Ensure all required trait bounds are specified:
```rust
impl<D> MyStruct<D>
where
    D: DisplayDriver,
    D::Error: core::fmt::Debug,  // Don't forget associated type bounds
{
    // ...
}
```

### Lifetime issues with async traits

Use `'static` lifetime for types used in async contexts:
```rust
pub struct MyDriver<SPI: 'static> {  // Note 'static bound
    spi: SPI,
}
```

### Generic type explosion

Use type aliases to simplify signatures:
```rust
type DefaultDisplay = HardwareDisplay<Spi1, Pin5, Pin6>;

pub struct App {
    display: DefaultDisplay,  // Instead of full generic signature
}
```

## References

- [embedded-hal Documentation](https://docs.rs/embedded-hal)
- [embedded-hal-async Documentation](https://docs.rs/embedded-hal-async)
- [Rust Async Book](https://rust-lang.github.io/async-book/)
- [4 Steps for Platform-Agnostic Drivers](https://blog.theembeddedrustacean.com/4-simple-steps-for-creating-a-platform-agnostic-driver-in-rust)
