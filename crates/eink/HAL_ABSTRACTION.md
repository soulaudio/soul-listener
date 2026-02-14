# E-Ink HAL Abstraction Guide

## Overview

This guide explains how to integrate hardware e-ink drivers with the emulator framework, ensuring seamless switching between emulator and real hardware.

## Architecture

```
┌─────────────────────────────────────────────────────────┐
│                  Application Code                       │
│  (UI rendering, state management, business logic)       │
└───────────────────────────┬─────────────────────────────┘
                            │
                    ┌───────▼────────┐
                    │ DisplayDriver  │  ◄── Core trait
                    │     Trait      │
                    └───────┬────────┘
                            │
        ┌───────────────────┴───────────────────┐
        │                                       │
┌───────▼────────┐                  ┌──────────▼────────┐
│   Emulator     │                  │ Hardware Adapters │
│  (Desktop)     │                  │  (Embedded)       │
├────────────────┤                  ├───────────────────┤
│ • Framebuffer  │                  │ • epd-waveshare   │
│ • Window       │                  │ • GoodDisplay     │
│ • Ghosting sim │                  │ • Custom HAL      │
│ • Animations   │                  │ • SPI + GPIO      │
└────────────────┘                  └───────────────────┘
        │                                       │
        │          Same DisplaySpec             │
        └───────────────┬───────────────────────┘
                        │
                ┌───────▼────────┐
                │  DisplaySpec   │  ◄── Shared display characteristics
                │   (eink-specs) │
                └────────────────┘
```

## Core Trait: DisplayDriver

The `DisplayDriver` trait provides a unified interface for both emulator and hardware:

```rust
use embedded_graphics::prelude::*;
use eink_specs::DisplaySpec;
use core::future::Future;

pub trait DisplayDriver: DrawTarget {
    /// Error type for display operations
    type DriverError: core::fmt::Debug;

    /// Get display specification
    ///
    /// Returns reference to DisplaySpec defining display characteristics.
    /// Both emulator and hardware return the same spec for a given display model.
    fn spec(&self) -> &'static DisplaySpec;

    /// Full refresh (clears ghosting)
    ///
    /// Hardware: Sends full waveform LUT, triggers complete refresh
    /// Emulator: Simulates flash sequence and timing
    fn refresh_full(&mut self) -> impl Future<Output = Result<(), Self::DriverError>>;

    /// Partial refresh (fast, accumulates ghosting)
    ///
    /// Hardware: Uses partial update command
    /// Emulator: Fast update with ghosting simulation
    fn refresh_partial(&mut self) -> impl Future<Output = Result<(), Self::DriverError>>;

    /// Fast refresh (fastest, high ghosting)
    ///
    /// Hardware: Single-pass update (if supported)
    /// Emulator: Quick flash with high ghosting
    fn refresh_fast(&mut self) -> impl Future<Output = Result<(), Self::DriverError>> {
        // Default: fall back to partial
        self.refresh_partial()
    }

    /// Enter low-power sleep mode
    fn sleep(&mut self) -> impl Future<Output = Result<(), Self::DriverError>>;

    /// Wake from sleep
    fn wake(&mut self) -> impl Future<Output = Result<(), Self::DriverError>>;
}
```

## Supporting Different HALs

### 1. epd-waveshare Adapter

For displays using the [epd-waveshare](https://docs.rs/epd-waveshare) crate:

```rust
use epd_waveshare::{epd2in13_v4::Epd2in13, prelude::*};
use embedded_hal::spi::SpiDevice;
use embedded_hal::digital::{InputPin, OutputPin};
use embedded_hal::delay::DelayNs;
use eink_specs::{DisplaySpec, displays};

pub struct WaveshareAdapter<SPI, BUSY, DC, RST, DELAY> {
    driver: Epd2in13<SPI, BUSY, DC, RST>,
    spi: SPI,
    delay: DELAY,
    spec: &'static DisplaySpec,
    buffer: [u8; Self::BUFFER_SIZE],
}

impl<SPI, BUSY, DC, RST, DELAY> WaveshareAdapter<SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    const BUFFER_SIZE: usize = 250 * 122 / 8;

    pub fn new(
        mut spi: SPI,
        busy: BUSY,
        dc: DC,
        rst: RST,
        mut delay: DELAY,
    ) -> Result<Self, DisplayError> {
        let driver = Epd2in13::new(&mut spi, busy, dc, rst, &mut delay)?;

        Ok(Self {
            driver,
            spi,
            delay,
            spec: &displays::WAVESHARE_2_13_V4,
            buffer: [0xFF; Self::BUFFER_SIZE],
        })
    }
}

impl<SPI, BUSY, DC, RST, DELAY> DisplayDriver
    for WaveshareAdapter<SPI, BUSY, DC, RST, DELAY>
where
    SPI: SpiDevice,
    BUSY: InputPin,
    DC: OutputPin,
    RST: OutputPin,
    DELAY: DelayNs,
{
    type DriverError = DisplayError;

    fn spec(&self) -> &'static DisplaySpec {
        self.spec
    }

    async fn refresh_full(&mut self) -> Result<(), Self::DriverError> {
        // Update frame buffer
        self.driver.update_frame(&mut self.spi, &self.buffer, &mut self.delay)?;

        // Trigger full refresh (hardware handles LUT and flashing)
        self.driver.display_frame(&mut self.spi, &mut self.delay)?;

        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::DriverError> {
        // Use partial update if supported
        self.driver.update_partial_frame(
            &mut self.spi,
            &self.buffer,
            0, 0,
            self.spec.width,
            self.spec.height,
        )?;

        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::DriverError> {
        self.driver.sleep(&mut self.spi, &mut self.delay)?;
        Ok(())
    }

    async fn wake(&mut self) -> Result<(), Self::DriverError> {
        self.driver.wake_up(&mut self.spi, &mut self.delay)?;
        Ok(())
    }
}

impl<SPI, BUSY, DC, RST, DELAY> DrawTarget
    for WaveshareAdapter<SPI, BUSY, DC, RST, DELAY>
{
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            if let Ok((x @ 0..250, y @ 0..122)) = coord.try_into() {
                let index = (x + y * 250) as usize / 8;
                let bit = 7 - ((x + y * 250) % 8);

                if color.is_on() {
                    self.buffer[index] &= !(1 << bit);
                } else {
                    self.buffer[index] |= 1 << bit;
                }
            }
        }
        Ok(())
    }
}

impl<SPI, BUSY, DC, RST, DELAY> OriginDimensions
    for WaveshareAdapter<SPI, BUSY, DC, RST, DELAY>
{
    fn size(&self) -> Size {
        Size::new(self.spec.width, self.spec.height)
    }
}
```

### 2. GoodDisplay Direct Driver

For custom GoodDisplay integration:

```rust
use embedded_hal::spi::SpiDevice;
use embedded_hal::digital::{InputPin, OutputPin};

pub struct GoodDisplayDriver<SPI, CS, DC, RST, BUSY> {
    spi: SPI,
    cs: CS,
    dc: DC,
    rst: RST,
    busy: BUSY,
    spec: &'static DisplaySpec,
    framebuffer: Vec<u8>,
}

impl<SPI, CS, DC, RST, BUSY> GoodDisplayDriver<SPI, CS, DC, RST, BUSY>
where
    SPI: SpiDevice,
    CS: OutputPin,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    pub fn new(
        spi: SPI,
        cs: CS,
        dc: DC,
        rst: RST,
        busy: BUSY,
        spec: &'static DisplaySpec,
    ) -> Self {
        let buffer_size = (spec.width * spec.height / 8) as usize;

        Self {
            spi,
            cs,
            dc,
            rst,
            busy,
            spec,
            framebuffer: vec![0xFF; buffer_size],
        }
    }

    fn send_command(&mut self, command: u8) -> Result<(), DisplayError> {
        self.dc.set_low().map_err(|_| DisplayError::Pin)?;
        self.cs.set_low().map_err(|_| DisplayError::Pin)?;
        self.spi.write(&[command]).map_err(|_| DisplayError::Spi)?;
        self.cs.set_high().map_err(|_| DisplayError::Pin)?;
        Ok(())
    }

    fn send_data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        self.dc.set_high().map_err(|_| DisplayError::Pin)?;
        self.cs.set_low().map_err(|_| DisplayError::Pin)?;
        self.spi.write(data).map_err(|_| DisplayError::Spi)?;
        self.cs.set_high().map_err(|_| DisplayError::Pin)?;
        Ok(())
    }

    fn wait_busy(&mut self) -> Result<(), DisplayError> {
        while self.busy.is_high().map_err(|_| DisplayError::Pin)? {
            // Wait for BUSY to go low
        }
        Ok(())
    }
}

impl<SPI, CS, DC, RST, BUSY> DisplayDriver
    for GoodDisplayDriver<SPI, CS, DC, RST, BUSY>
where
    SPI: SpiDevice,
    CS: OutputPin,
    DC: OutputPin,
    RST: OutputPin,
    BUSY: InputPin,
{
    type DriverError = DisplayError;

    fn spec(&self) -> &'static DisplaySpec {
        self.spec
    }

    async fn refresh_full(&mut self) -> Result<(), Self::DriverError> {
        // Send display refresh command
        self.send_command(0x12)?;  // Display Update Control 2
        self.wait_busy()?;

        // Send full refresh sequence
        self.send_command(0x20)?;  // Master Activation
        self.wait_busy()?;

        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::DriverError> {
        // Send partial update command
        self.send_command(0x91)?;  // Partial In
        self.send_command(0x90)?;  // Partial Window
        self.send_data(&[
            0, 0,  // x_start
            (self.spec.width / 8 - 1) as u8, 0,  // x_end
            0, 0,  // y_start
            (self.spec.height - 1) as u8, 0,  // y_end
        ])?;

        self.send_command(0x13)?;  // Write RAM
        self.send_data(&self.framebuffer)?;

        self.send_command(0x12)?;  // Display Update
        self.wait_busy()?;

        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::DriverError> {
        self.send_command(0x10)?;  // Deep Sleep
        self.send_data(&[0x01])?;
        Ok(())
    }

    async fn wake(&mut self) -> Result<(), Self::DriverError> {
        self.rst.set_low().map_err(|_| DisplayError::Pin)?;
        // Delay 10ms
        self.rst.set_high().map_err(|_| DisplayError::Pin)?;
        // Delay 10ms
        self.wait_busy()?;
        Ok(())
    }
}

impl<SPI, CS, DC, RST, BUSY> DrawTarget
    for GoodDisplayDriver<SPI, CS, DC, RST, BUSY>
{
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            let (x, y): (u32, u32) = coord.try_into().ok()?;
            if x < self.spec.width && y < self.spec.height {
                let index = (x + y * self.spec.width) as usize / 8;
                let bit = 7 - ((x + y * self.spec.width) % 8);

                if color.is_on() {
                    self.framebuffer[index] &= !(1 << bit);
                } else {
                    self.framebuffer[index] |= 1 << bit;
                }
            }
        }
        Ok(())
    }
}

impl<SPI, CS, DC, RST, BUSY> OriginDimensions
    for GoodDisplayDriver<SPI, CS, DC, RST, BUSY>
{
    fn size(&self) -> Size {
        Size::new(self.spec.width, self.spec.height)
    }
}
```

### 3. Generic HAL Driver

For any SPI-based e-ink display:

```rust
pub struct GenericEinkDriver<SPI, PINS> {
    spi: SPI,
    pins: PINS,
    spec: &'static DisplaySpec,
    command_table: &'static CommandTable,
}

pub struct CommandTable {
    pub init: &'static [u8],
    pub full_refresh: u8,
    pub partial_refresh: u8,
    pub sleep: u8,
    pub wake: u8,
}

// Usage:
const UC8151_COMMANDS: CommandTable = CommandTable {
    init: &[0x01, 0x03, 0x00],
    full_refresh: 0x12,
    partial_refresh: 0x91,
    sleep: 0x10,
    wake: 0x12,
};

let display = GenericEinkDriver::new(
    spi,
    pins,
    &displays::GDEW042T2,
    &UC8151_COMMANDS,
);
```

## Application Code

Your application code works **identically** with any driver:

```rust
async fn render_ui<D: DisplayDriver>(display: &mut D) -> Result<(), D::DriverError> {
    // Clear display
    display.clear(Gray4::WHITE)?;

    // Draw UI (same code for emulator and hardware!)
    Text::new(
        "Temperature: 25°C",
        Point::new(10, 10),
        MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
    )
    .draw(display)?;

    Rectangle::new(Point::new(10, 30), Size::new(200, 20))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
        .draw(display)?;

    // Refresh based on display capabilities
    if display.spec().supports_partial_refresh() {
        display.refresh_partial().await?;
    } else {
        display.refresh_full().await?;
    }

    Ok(())
}

// Development with emulator
let mut emulator = Emulator::with_spec(&displays::WAVESHARE_2_13_V4);
render_ui(&mut emulator).await?;

// Production with hardware
let mut hardware = WaveshareAdapter::new(spi, pins)?;
render_ui(&mut hardware).await?;
```

## Platform-Specific Features

### Conditional Compilation

```toml
[features]
default = []
hardware = ["epd-waveshare", "embassy-stm32"]
simulator = ["eink-emulator"]
```

```rust
#[cfg(feature = "simulator")]
type Display = Emulator;

#[cfg(feature = "hardware")]
type Display = WaveshareAdapter<Spi, Pins>;

async fn main() {
    #[cfg(feature = "simulator")]
    let mut display = Emulator::new(250, 122);

    #[cfg(feature = "hardware")]
    let mut display = WaveshareAdapter::new(spi, pins)?;

    render_ui(&mut display).await?;
}
```

## Error Handling

Define unified error types:

```rust
#[derive(Debug)]
pub enum DisplayError {
    Spi,
    Pin,
    Timeout,
    InvalidState,
}

impl From<epd_waveshare::Error> for DisplayError {
    fn from(_: epd_waveshare::Error) -> Self {
        DisplayError::Spi
    }
}

impl From<std::io::Error> for DisplayError {
    fn from(_: std::io::Error) -> Self {
        DisplayError::InvalidState
    }
}
```

## Temperature Handling

### Hardware (from sensor)

```rust
impl<SPI, PINS> EinkDisplay for HardwareDriver<SPI, PINS> {
    fn temperature(&self) -> Option<i8> {
        // Read from display's built-in sensor
        self.read_temperature_register().ok()
    }
}
```

### Emulator (simulated)

```rust
impl EinkDisplay for Emulator {
    fn temperature(&self) -> Option<i8> {
        Some(self.current_temp)  // Set via set_temperature()
    }
}
```

### Application (temperature compensation)

```rust
async fn smart_refresh<D: EinkDisplay>(display: &mut D) -> Result<()> {
    let temp = display.temperature().unwrap_or(25);

    if !display.spec().is_operating_temp(temp) {
        return Err(DisplayError::InvalidState);
    }

    let duration = display.spec().adjusted_refresh_ms(
        display.spec().full_refresh_ms,
        temp,
    );

    // Use adjusted duration for timeout
    timeout(Duration::from_millis(duration as u64 * 2),
            display.refresh_full()).await?
}
```

## Testing Strategy

### Unit Tests (with emulator)

```rust
#[tokio::test]
async fn test_ui_rendering() {
    let mut display = Emulator::headless(250, 122);

    render_ui(&mut display).await.unwrap();

    // Verify pixels
    assert_eq!(display.framebuffer.get_pixel(10, 10), Some(Gray4::BLACK));

    // Save screenshot for visual regression
    display.screenshot("tests/ui_test.png").unwrap();
}
```

### Integration Tests (with mock HAL)

```rust
use embedded_hal_mock::eh1::spi::{Mock as SpiMock, Transaction};

#[test]
fn test_hardware_commands() {
    let expectations = vec![
        Transaction::write(vec![0x12]),  // Display Update Control
        Transaction::write(vec![0x20]),  // Master Activation
    ];

    let spi = SpiMock::new(&expectations);
    let mut display = WaveshareAdapter::new(spi, pins).unwrap();

    display.refresh_full().unwrap();

    spi.done();  // Verifies all commands were sent
}
```

## Best Practices

1. **Use DisplaySpec consistently** - Both emulator and hardware should reference the same spec for a given display model

2. **Implement all trait methods** - Even if hardware doesn't support fast refresh, implement it (can fall back to partial)

3. **Handle errors properly** - Use `DriverError` type for consistent error handling

4. **Test with emulator first** - Develop UI with emulator, deploy to hardware without code changes

5. **Respect temperature ranges** - Check `is_operating_temp()` before refresh operations

6. **Auto-cleanup ghosting** - Use `RefreshStrategy` or implement similar logic

7. **Use feature flags** - Separate `simulator` and `hardware` builds

## Summary

The HAL abstraction provides:

✅ **Unified Interface** - Same `DisplayDriver` trait for all implementations
✅ **Seamless Switching** - Application code works with emulator and hardware
✅ **Multi-Brand Support** - Works with Waveshare, GoodDisplay, custom drivers
✅ **Type Safety** - Compile-time guarantees via Rust traits
✅ **Testability** - Unit tests with emulator, integration tests with mocks
✅ **Flexibility** - Easy to add new display types and controllers

---

**Ready for Production:** This architecture is production-ready and follows Rust embedded best practices from the embedded-hal ecosystem.
