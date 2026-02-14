# SoulAudio DAP Firmware

Professional-grade Digital Audio Player firmware for STM32H7 with e-ink display support.

## Overview

This crate contains the complete firmware for the SoulAudio DAP, including:

- **SSD1677 Hardware Driver** - Full async driver for 800×480 e-ink display
- **Emulator Support** - Desktop development with realistic e-ink simulation
- **HAL Abstraction** - Unified interface for hardware and emulator
- **UI Components** - Splash screen, test patterns, and UI framework foundation

## Hardware

### Target Platform

- **MCU**: STM32H743ZI (480 MHz Cortex-M7)
- **Display**: GDEM0397T81P (Good Display 3.97" 800×480)
- **Controller**: SSD1677 e-ink controller
- **Interface**: SPI (4 MHz)

### Pin Configuration

```
PA5  (SPI1_SCK)  → SCK
PA7  (SPI1_MOSI) → DIN
PB0  (GPIO)      → DC (Data/Command)
PB1  (GPIO)      → CS (Chip Select)
PB2  (GPIO)      → RST (Reset)
PB3  (GPIO)      → BUSY
```

### Memory Layout

```
FLASH:   2 MB   (0x0800_0000)
RAM:     128 KB (0x2000_0000) - DTCM - Stack
AXISRAM: 512 KB (0x2400_0000) - Framebuffer (96 KB)
SRAM1:   128 KB (0x3000_0000) - DMA buffers
```

## Building

### For Hardware (STM32H7)

```bash
# Build firmware
cargo build --release --target thumbv7em-none-eabihf --features hardware

# Flash and debug
cargo embed --release

# Run hardware test
cargo embed --release --example display_hardware_test
```

### For Emulator (Desktop)

```bash
# Build and run main firmware
cargo run --features emulator

# Run emulator test
cargo run --example display_emulator_test --features emulator
```

## Features

### Feature Flags

- `hardware` - Build for STM32H7 (default for `cargo embed`)
- `emulator` - Build for desktop with eink-emulator
- `std` - Enable standard library (implied by emulator)
- `defmt-logging` - Enable defmt logging for hardware

### Conditional Compilation

The firmware uses feature-based conditional compilation to support both targets:

```rust
#[cfg(feature = "hardware")]
pub use display::Ssd1677Display;

#[cfg(feature = "emulator")]
pub use display::EmulatorDisplay;
```

## Architecture

### Layer Structure

```
┌─────────────────────────────────────┐
│   Application (main.rs, ui)         │
├─────────────────────────────────────┤
│   HAL Abstraction (DapDisplay)      │
├─────────────────────────────────────┤
│   Hardware Driver │  Emulator       │
│   (SSD1677)       │  (eink-emulator)│
├─────────────────────────────────────┤
│   Platform HAL (Embassy/Tokio)      │
└─────────────────────────────────────┘
```

### Trait-Based Abstraction

All display operations go through the `DapDisplay` trait:

```rust
pub trait DapDisplay: platform::DisplayDriver {
    async fn init(&mut self) -> Result<(), Self::DriverError>;
    async fn update_buffer(&mut self, framebuffer: &[u8]) -> Result<(), Self::DriverError>;
    fn framebuffer_size(&self) -> usize;
    async fn clear(&mut self, color: Color) -> Result<(), Self::DriverError>;
}
```

Both hardware and emulator implement this trait identically.

## Display Specifications

### GDEM0397T81P

- **Resolution**: 800×480 pixels
- **Size**: 3.97 inches diagonal
- **Colors**: 4-level grayscale (2 bits per pixel)
- **Refresh Times**:
  - Full: 2000 ms (3 flashes, clears ghosting)
  - Partial: 300 ms (fast updates, some ghosting)
  - Fast: 260 ms (fastest, most ghosting)

### Framebuffer

- **Size**: 96,000 bytes (800×480 ÷ 4)
- **Format**: 2 bits per pixel (packed)
- **Location**: AXI SRAM (`#[link_section = ".axisram"]`)
- **Values**: 0x00 (black), 0xFF (white)

## SSD1677 Controller

### Initialization Sequence

1. Hardware reset (10 ms pulse)
2. Software reset (wait for BUSY)
3. Driver output control (gate count = 480)
4. Data entry mode (X/Y increment)
5. RAM window setup (0-799, 0-479)
6. Border waveform (follow LUT)
7. Temperature sensor (internal)
8. LUT loading (full refresh waveform)

### LUT (Look-Up Table)

The driver includes two LUTs:

- **Full Refresh LUT** - 3-flash waveform for complete update
- **Partial Refresh LUT** - 1-flash waveform for fast updates

LUTs control voltage sequencing for pixel transitions.

### Ghosting Management

After 5 partial refreshes, the driver automatically triggers a full refresh to clear accumulated ghosting.

## Examples

### Hardware Test

Comprehensive hardware validation:

```bash
cargo embed --release --example display_hardware_test
```

Tests:
1. Initialization
2. Clear to white
3. Rectangle drawing
4. Circle drawing
5. Text rendering
6. Partial refresh
7. Sleep mode
8. Wake from sleep

### Emulator Test

Desktop development workflow:

```bash
cargo run --example display_emulator_test --features emulator
```

Opens a window showing realistic e-ink behavior with same tests as hardware.

## Development Workflow

### 1. Desktop Development

Start with emulator for rapid iteration:

```bash
# Terminal 1: Run emulator
cargo run --features emulator

# Terminal 2: Run tests
cargo test --features emulator

# Terminal 3: Watch and rebuild
cargo watch -x "test --features emulator"
```

### 2. Hardware Validation

Flash to device for final testing:

```bash
# Build and flash
cargo embed --release

# View defmt logs
# (automatically shown by probe-rs)
```

### 3. Debugging

```bash
# Hardware debugging with probe-rs
probe-rs debug --chip STM32H743ZITx

# Emulator debugging (standard Rust tools)
rust-gdb target/debug/examples/display_emulator_test
```

## API Examples

### Basic Usage

```rust
use firmware::{DapDisplay, EmulatorDisplay};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut display = EmulatorDisplay::new();

    display.init().await?;
    display.clear(firmware::Color::White).await?;

    // Draw using embedded-graphics
    // ...

    display.refresh_full().await?;

    Ok(())
}
```

### Drawing with embedded-graphics

```rust
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Rectangle, PrimitiveStyle};
use embedded_graphics::pixelcolor::BinaryColor;

Rectangle::new(Point::new(10, 10), Size::new(100, 50))
    .into_styled(PrimitiveStyle::with_fill(BinaryColor::Off))
    .draw(&mut display)?;

display.refresh_full().await?;
```

### Text Rendering

```rust
use embedded_graphics::text::Text;
use embedded_graphics::mono_font::{ascii::FONT_9X18, MonoTextStyle};

let text_style = MonoTextStyle::new(&FONT_9X18, BinaryColor::Off);

Text::new("Hello, World!", Point::new(50, 100), text_style)
    .draw(&mut display)?;
```

## Performance

### Refresh Timing (Measured)

- **Full Refresh**: ~2000 ms (3 flashes)
- **Partial Refresh**: ~300 ms (1 flash)
- **SPI Transfer**: ~24 ms (96 KB @ 4 MHz)

### Memory Usage

- **Flash**: ~45 KB (optimized build)
- **RAM**: ~2 KB (stack + static)
- **Framebuffer**: 96 KB (AXI SRAM)

## Error Handling

All operations return `Result` types:

```rust
pub enum DisplayError {
    Communication,  // SPI error
    Busy,           // Display not ready
    Timeout,        // Operation timeout (5s)
    InvalidState,   // Wrong state for operation
    InvalidBuffer,  // Wrong buffer size
    Unsupported,    // Operation not supported
}
```

Logging via `defmt` on hardware, `println!` on emulator.

## Testing

```bash
# Unit tests (library code)
cargo test --lib

# Integration tests (emulator required)
cargo test --features emulator

# Hardware-in-loop tests (requires hardware)
cargo test --features hardware --target thumbv7em-none-eabihf
```

## Future Enhancements

- [ ] Temperature compensation from internal sensor
- [ ] Custom LUT loading from flash
- [ ] DMA optimization for SPI transfers
- [ ] 4-level grayscale support (currently binary)
- [ ] Partial window updates (not full screen)
- [ ] Fast mode (A2 waveform)
- [ ] Power consumption optimization

## License

MIT OR Apache-2.0

## References

- [SSD1677 Datasheet](https://www.good-display.com/companyfile/197.html)
- [Embassy Documentation](https://embassy.dev/)
- [embedded-graphics](https://docs.rs/embedded-graphics/)
- [STM32H7 Reference Manual](https://www.st.com/resource/en/reference_manual/rm0433-stm32h742-stm32h743753-and-stm32h750-value-line-advanced-armbased-32bit-mcus-stmicroelectronics.pdf)
