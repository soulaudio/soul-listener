# Firmware Crate - Quick Start Guide

## 5-Minute Quick Start

### Desktop Development (Emulator)

```bash
# Run the emulator example
cd crates/firmware
cargo run --example display_emulator_test --features emulator

# A window will open showing the e-ink display
# Tests will run automatically:
# 1. Splash screen
# 2. Test pattern
# 3. Shapes
# 4. Text
# 5. Animation
# 6. Sleep/wake

# Press Ctrl+C to exit
```

### Hardware Deployment (STM32H7)

```bash
# Build for hardware
cargo build --release --target thumbv7em-none-eabihf --features hardware

# Flash to device (requires probe-rs and ST-Link)
cargo embed --release

# Run hardware test
cargo embed --release --example display_hardware_test
```

## API Quick Reference

### Create Display

```rust
// Hardware
use firmware::Ssd1677Display;
let display = Ssd1677Display::new(spi, dc, cs, rst, busy);

// Emulator
use firmware::EmulatorDisplay;
let display = EmulatorDisplay::new();          // With window
let display = EmulatorDisplay::headless();     // Without window
```

### Initialize

```rust
use firmware::DapDisplay;
display.init().await?;
```

### Draw

```rust
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Rectangle, PrimitiveStyle};
use embedded_graphics::pixelcolor::Gray4;  // For emulator

Rectangle::new(Point::new(10, 10), Size::new(100, 50))
    .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
    .draw(&mut display)?;
```

### Refresh

```rust
use platform::DisplayDriver;  // Import trait

// Full refresh (clears ghosting)
display.refresh_full().await?;

// Partial refresh (fast)
display.refresh_partial().await?;
```

### UI Components

```rust
use firmware::ui::{SplashScreen, TestPattern};

SplashScreen::render(&mut display)?;
display.refresh_full().await?;

TestPattern::render(&mut display)?;
display.refresh_full().await?;
```

### Clear

```rust
use firmware::{DapDisplay, Color};

DapDisplay::clear(&mut display, Color::White).await?;
DapDisplay::clear(&mut display, Color::Black).await?;
```

### Power Management

```rust
use platform::DisplayDriver;

display.sleep().await?;   // Deep sleep
display.wake().await?;    // Wake and re-initialize
```

## Pin Configuration (Hardware)

```
STM32H7 → SSD1677
─────────────────
PA5  → SCK
PA7  → MOSI (DIN)
PB0  → DC
PB1  → CS
PB2  → RST
PB3  → BUSY
```

## Feature Flags

```bash
# Emulator (desktop)
--features emulator

# Hardware (STM32H7)
--features hardware --target thumbv7em-none-eabihf

# Both are mutually exclusive
```

## Common Patterns

### Full Application (Emulator)

```rust
use tokio;
use firmware::{EmulatorDisplay, DapDisplay};
use platform::DisplayDriver;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut display = EmulatorDisplay::new();
    display.init().await?;

    // Your drawing code here
    firmware::ui::SplashScreen::render(&mut display)?;
    display.refresh_full().await?;

    // Keep window open
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }
}
```

### Full Application (Hardware)

```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use embassy_stm32::gpio::{Level, Output, Speed, Input, Pull};
use embassy_stm32::spi::{Spi, Config as SpiConfig};
use embassy_stm32::time::Hertz;
use firmware::{Ssd1677Display, DapDisplay};
use platform::DisplayDriver;
use panic_probe as _;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(Default::default());

    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(4_000_000);

    let spi = Spi::new(p.SPI1, p.PA5, p.PA7, p.PA6,
                       p.DMA1_CH0, p.DMA1_CH1, spi_config);

    let dc = Output::new(p.PB0, Level::Low, Speed::VeryHigh);
    let cs = Output::new(p.PB1, Level::High, Speed::VeryHigh);
    let rst = Output::new(p.PB2, Level::High, Speed::VeryHigh);
    let busy = Input::new(p.PB3, Pull::None);

    let mut display = Ssd1677Display::new(spi, dc, cs, rst, busy);

    display.init().await.unwrap();
    firmware::ui::SplashScreen::render(&mut display).unwrap();
    display.refresh_full().await.unwrap();

    // Main loop
    loop {
        embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
    }
}
```

## Troubleshooting

### Build Errors

**"can't find crate for `core`"**
- Remove `target = "thumbv7em-none-eabihf"` from `.cargo/config.toml`
- Use explicit `--target` flag for hardware builds

**"multiple `clear` found"**
- Use qualified path: `DapDisplay::clear(&mut display, Color::White).await?`

**"no method named `refresh_full`"**
- Import trait: `use platform::DisplayDriver;`

### Hardware Issues

**Display doesn't refresh**
- Check BUSY pin connection
- Verify SPI wiring
- Check power supply (3.3V)

**Corrupted display**
- Run full refresh: `display.refresh_full().await?`
- Check framebuffer initialization
- Verify SPI speed (4 MHz max)

## Performance Tips

### Hardware
- Use partial refresh for animations
- Trigger full refresh every 5-10 partial refreshes
- Consider DMA for large SPI transfers

### Emulator
- Use headless mode for automated testing
- Window mode for visual debugging

## Documentation

- **README.md** - Full API documentation
- **FIRMWARE_COMPLETE.md** - Implementation details
- **QUICK_START.md** - This file

## Examples

```bash
# List all examples
ls examples/

# Run emulator test
cargo run --example display_emulator_test --features emulator

# Build hardware test
cargo build --example display_hardware_test --target thumbv7em-none-eabihf --features hardware
```

## Next Steps

1. **Explore examples** - See `examples/` directory
2. **Read README** - Comprehensive documentation
3. **Build UI** - Use embedded-graphics to create screens
4. **Test on hardware** - Flash and validate

---

**Happy coding! 🚀**
