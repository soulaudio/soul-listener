# DAP (Digital Audio Player) Foundation Implementation Plan

## Overview

Transition from emulator development to building the actual Digital Audio Player firmware, starting with display integration for the Good Display e-paper panel.

**Target Hardware:**
- **MCU:** STM32H7 (Cortex-M7, 480 MHz)
- **Display:** Good Display 2.13" e-paper (to be determined)
- **Framework:** Embassy async runtime
- **Language:** Embedded Rust (no_std)

---

## Phase 5: DAP Foundation & Display Integration

### Goal
Create the firmware foundation with working e-ink display on real STM32H7 hardware, while maintaining emulator support for rapid development.

---

## Step 1: Choose the Right Display

### Option A: GDEW0213I5F (Already Supported) ✅
**Specs:**
- Size: 2.13" (212×104 pixels)
- Controller: UC8151
- Panel: Pearl (older technology)
- Refresh: 2s full, 500ms partial
- Quirks: Rotation glitch, SPI hang issues (we emulate these!)

**Pros:**
- Already have full emulator support
- Known quirks documented
- Good size for DAP interface
- Cost-effective

**Cons:**
- Older Pearl panel (lower contrast than Carta)
- Lower resolution

### Option B: Find Specific Good Display Model
We should verify which exact model you want to use. Common options:

**2.13" Options:**
- GDEY0213B74 - 212×104, SSD1680, newer
- GDEH0213B73 - 250×122, UC8151D, flexible
- GDEM0213B74 - 212×104, SSD1680, Carta

**Recommendation:** Let's use **GDEW0213I5F** since we already have it fully specified and emulated. We can upgrade to a newer model later if needed.

---

## Step 2: Project Structure

### Workspace Layout
```
eink-emulator/  (root)
├── crates/
│   ├── eink/
│   │   ├── eink-specs/       ✅ Already done
│   │   └── eink-emulator/    ✅ Already done
│   ├── platform/             ✅ Already done (HAL traits)
│   └── firmware/             ⭐ NEW - DAP firmware
│       ├── Cargo.toml
│       ├── src/
│       │   ├── main.rs       # Entry point
│       │   ├── display/      # Display driver integration
│       │   ├── ui/           # UI screens
│       │   ├── audio/        # Audio playback (future)
│       │   └── tasks/        # Embassy tasks
│       └── examples/
│           └── display_test.rs
├── memory.x                   ✅ Already present
└── .cargo/config.toml         ✅ Already configured
```

### New Crate: `firmware`

**Purpose:** Main DAP application binary

**Cargo.toml:**
```toml
[package]
name = "firmware"
version = "0.1.0"
edition = "2021"

[dependencies]
# Embassy async runtime
embassy-executor = { version = "0.6", features = ["arch-cortex-m", "executor-thread", "integrated-timers"] }
embassy-time = { version = "0.3", features = ["tick-hz-32_768"] }
embassy-sync = "0.6"
embassy-stm32 = { version = "0.1", features = ["stm32h743zi", "time-driver-any", "exti"] }

# Hardware abstraction
embedded-hal = "1.0"
embedded-hal-async = "1.0"
embedded-graphics = "0.8"

# Our crates
eink-specs = { path = "../eink/eink-specs" }
platform = { path = "../platform" }

# E-ink driver (Good Display)
epd-waveshare = "0.7"  # Or good-display-epd if available

# Utilities
heapless = "0.8"
defmt = "0.3"
defmt-rtt = "0.4"
panic-probe = { version = "0.3", features = ["print-defmt"] }
cortex-m = { version = "0.7", features = ["inline-asm"] }
cortex-m-rt = "0.7"

[features]
default = []

[[bin]]
name = "display_test"
test = false
bench = false
```

---

## Step 3: STM32H7 Pin Mapping

### SPI Configuration for E-Paper Display

**Typical SPI connections:**
```
STM32H7         E-Paper Display
-------         ---------------
PA5  (SPI1_SCK) → SCK   (Clock)
PA7  (SPI1_MOSI)→ DIN   (Data In)
PB0  (GPIO)     → DC    (Data/Command)
PB1  (GPIO)     → CS    (Chip Select)
PB2  (GPIO)     → RST   (Reset)
PB3  (GPIO)     → BUSY  (Busy signal)
```

**Power:**
- 3.3V → VCC
- GND → GND

---

## Step 4: Display Driver Implementation

### File: `crates/firmware/src/display/mod.rs`

```rust
use embassy_stm32::{
    spi::{Spi, Config as SpiConfig},
    gpio::{Output, Level, Speed},
    peripherals,
};
use embedded_hal_async::spi::SpiDevice;
use platform::display::DisplayDriver;
use eink_specs::DisplaySpec;

pub struct GoodDisplayDriver {
    // SPI + control pins
    spi: Spi<'static, peripherals::SPI1>,
    dc: Output<'static, peripherals::PB0>,
    cs: Output<'static, peripherals::PB1>,
    rst: Output<'static, peripherals::PB2>,
    busy: Input<'static, peripherals::PB3>,

    // Display spec
    spec: &'static DisplaySpec,
}

impl GoodDisplayDriver {
    pub fn new(
        spi: Spi<'static, peripherals::SPI1>,
        dc: Output<'static, peripherals::PB0>,
        cs: Output<'static, peripherals::PB1>,
        rst: Output<'static, peripherals::PB2>,
        busy: Input<'static, peripherals::PB3>,
        spec: &'static DisplaySpec,
    ) -> Self {
        Self { spi, dc, cs, rst, busy, spec }
    }

    pub async fn init(&mut self) -> Result<(), DisplayError> {
        // Hardware reset sequence
        self.rst.set_low();
        embassy_time::Timer::after_millis(10).await;
        self.rst.set_high();
        embassy_time::Timer::after_millis(10).await;

        // Wait for BUSY to go low
        while self.busy.is_high() {
            embassy_time::Timer::after_millis(10).await;
        }

        // Send initialization commands (UC8151 specific)
        self.send_command(0x12).await?; // Software reset
        self.wait_busy().await;

        // ... (rest of init sequence based on datasheet)

        Ok(())
    }

    async fn send_command(&mut self, command: u8) -> Result<(), DisplayError> {
        self.dc.set_low();  // Command mode
        self.cs.set_low();  // Select
        self.spi.write(&[command]).await?;
        self.cs.set_high(); // Deselect
        Ok(())
    }

    async fn send_data(&mut self, data: &[u8]) -> Result<(), DisplayError> {
        self.dc.set_high(); // Data mode
        self.cs.set_low();  // Select
        self.spi.write(data).await?;
        self.cs.set_high(); // Deselect
        Ok(())
    }

    async fn wait_busy(&mut self) {
        while self.busy.is_high() {
            embassy_time::Timer::after_millis(10).await;
        }
    }
}

impl DisplayDriver for GoodDisplayDriver {
    type DriverError = DisplayError;

    async fn refresh_full(&mut self) -> Result<(), Self::DriverError> {
        // Send display update command
        self.send_command(0x22).await?; // Display Update Control 2
        self.send_data(&[0xF7]).await?; // Full refresh sequence
        self.send_command(0x20).await?; // Master Activation
        self.wait_busy().await;
        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::DriverError> {
        self.send_command(0x22).await?; // Display Update Control 2
        self.send_data(&[0xFF]).await?; // Partial refresh sequence
        self.send_command(0x20).await?; // Master Activation
        self.wait_busy().await;
        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::DriverError> {
        self.send_command(0x10).await?; // Deep sleep mode
        self.send_data(&[0x01]).await?;
        Ok(())
    }

    async fn wake(&mut self) -> Result<(), Self::DriverError> {
        self.init().await
    }
}

// Implement DrawTarget for embedded-graphics
impl DrawTarget for GoodDisplayDriver {
    type Color = Gray4;
    type Error = DisplayError;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        // Buffer pixels and send to display
        // This would typically write to an internal framebuffer
        todo!("Implement pixel buffering")
    }
}

impl OriginDimensions for GoodDisplayDriver {
    fn size(&self) -> Size {
        Size::new(self.spec.width, self.spec.height)
    }
}

#[derive(Debug)]
pub enum DisplayError {
    Spi(embassy_stm32::spi::Error),
    Timeout,
}
```

---

## Step 5: Main Firmware Entry Point

### File: `crates/firmware/src/main.rs`

```rust
#![no_std]
#![no_main]

use defmt::*;
use embassy_executor::Spawner;
use embassy_stm32::{
    Config,
    gpio::{Level, Output, Speed, Input, Pull},
    spi::{Spi, Config as SpiConfig, MODE_0},
    peripherals,
    time::Hertz,
};
use embassy_time::Timer;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray4,
    prelude::*,
    text::Text,
};
use eink_specs::displays::gooddisplay::GDEW0213I5F;
use {defmt_rtt as _, panic_probe as _};

mod display;
use display::GoodDisplayDriver;

#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // Configure STM32H7
    let mut config = Config::default();
    {
        use embassy_stm32::rcc::*;
        config.rcc.hse = Some(Hse {
            freq: Hertz(25_000_000), // 25 MHz external crystal
            mode: HseMode::Oscillator,
        });
        config.rcc.sys = Sysclk::PLL1_P;
        config.rcc.pll1 = Some(Pll {
            source: PllSource::HSE,
            prediv: PllPreDiv::DIV5,
            mul: PllMul::MUL96,
            divp: Some(PllDiv::DIV2),  // 480 MHz
            divq: None,
            divr: None,
        });
    }

    let p = embassy_stm32::init(config);

    info!("🎵 SoulAudio DAP - Digital Audio Player");
    info!("Initializing e-ink display...");

    // Configure SPI
    let mut spi_config = SpiConfig::default();
    spi_config.frequency = Hertz(4_000_000); // 4 MHz (conservative)
    spi_config.mode = MODE_0;

    let spi = Spi::new(
        p.SPI1,
        p.PA5,  // SCK
        p.PA7,  // MOSI
        p.PA6,  // MISO (unused for e-paper)
        p.DMA1_CH0,
        p.DMA1_CH1,
        spi_config,
    );

    // Control pins
    let dc = Output::new(p.PB0, Level::Low, Speed::Low);
    let cs = Output::new(p.PB1, Level::High, Speed::Low);
    let rst = Output::new(p.PB2, Level::High, Speed::Low);
    let busy = Input::new(p.PB3, Pull::Down);

    // Create display driver
    let mut display = GoodDisplayDriver::new(spi, dc, cs, rst, busy, &GDEW0213I5F);

    // Initialize display
    display.init().await.unwrap();
    info!("Display initialized!");

    // Draw "Hello, World!"
    let text_style = MonoTextStyle::new(&FONT_6X10, Gray4::BLACK);
    Text::new("SoulAudio DAP", Point::new(10, 20), text_style)
        .draw(&mut display)
        .unwrap();

    display.refresh_full().await.unwrap();
    info!("Display updated!");

    // Spawn tasks
    spawner.spawn(display_task(display)).unwrap();
    spawner.spawn(heartbeat_task(p.PA0)).unwrap();
}

#[embassy_executor::task]
async fn display_task(mut display: GoodDisplayDriver) {
    loop {
        Timer::after_secs(10).await;
        info!("Display task running");
    }
}

#[embassy_executor::task]
async fn heartbeat_task(led_pin: peripherals::PA0) {
    let mut led = Output::new(led_pin, Level::Low, Speed::Low);
    loop {
        led.set_high();
        Timer::after_millis(100).await;
        led.set_low();
        Timer::after_millis(900).await;
    }
}
```

---

## Step 6: Parallel Development with Emulator

**Key advantage:** Develop UI in emulator while hardware is being set up!

### File: `crates/firmware/examples/emulator_test.rs`

```rust
use eink_emulator::{Emulator, DisplayDriver};
use eink_specs::displays::gooddisplay::GDEW0213I5F;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Gray4,
    prelude::*,
    text::Text,
};

#[tokio::main]
async fn main() {
    // Use the SAME display spec as hardware
    let mut emulator = Emulator::with_spec(&GDEW0213I5F);

    // Initialize (simulates hardware init timing)
    emulator.initialize().await.unwrap();

    // Draw the same content as firmware
    let text_style = MonoTextStyle::new(&FONT_6X10, Gray4::BLACK);
    Text::new("SoulAudio DAP", Point::new(10, 20), text_style)
        .draw(&mut emulator)
        .unwrap();

    emulator.refresh_full().await.unwrap();

    // Keep window open
    emulator.run_event_loop().await;
}
```

**Workflow:**
1. Develop UI in emulator (instant feedback)
2. Test with quirks enabled (catches UC8151 bugs)
3. Deploy to hardware (already know it will work!)

---

## Step 7: Implementation Phases

### Phase 5.1: Hardware Setup (Week 1)
- [ ] Order Good Display GDEW0213I5F panel
- [ ] Wire up SPI connections to STM32H7
- [ ] Verify pin connections with multimeter
- [ ] Test basic SPI communication (LED blink test)

### Phase 5.2: Display Driver (Week 1-2)
- [ ] Create `firmware` crate
- [ ] Implement `GoodDisplayDriver` struct
- [ ] Initialize UC8151 controller
- [ ] Test refresh_full() with solid colors
- [ ] Implement DrawTarget for pixel buffering
- [ ] Test with embedded-graphics primitives

### Phase 5.3: Emulator Parity (Week 2)
- [ ] Create emulator example matching firmware
- [ ] Test UI in emulator first
- [ ] Deploy to hardware and verify identical behavior
- [ ] Document any discrepancies

### Phase 5.4: Basic UI Screens (Week 3)
- [ ] Now Playing screen
- [ ] Track list screen
- [ ] Settings screen
- [ ] Test navigation between screens

### Phase 5.5: Input Handling (Week 4)
- [ ] Rotary encoder integration
- [ ] Button debouncing
- [ ] Touch input (if display supports it)
- [ ] Input simulation in emulator

---

## Step 8: Testing Strategy

### Emulator-First Development
```bash
# 1. Develop in emulator
cargo run --example emulator_test

# 2. Test with quirks enabled
cargo run --example emulator_test -- --quirks

# 3. Build for hardware
cargo build --release --target thumbv7em-none-eabihf

# 4. Flash to device
probe-rs run --chip STM32H743ZITx target/thumbv7em-none-eabihf/release/firmware
```

### Continuous Integration
- Emulator tests run on every commit
- Hardware tests run on release branches
- Visual regression tests for UI changes

---

## Step 9: Dependencies Needed

### Additional Crates
```toml
# For UC8151 controller (if epd-waveshare doesn't support it)
uc8151 = "0.1"  # Check crates.io for Good Display drivers

# Or build our own driver using generic SPI interface
embedded-hal-bus = "0.2"  # SPI bus sharing
```

### Driver Research
Check these crates for Good Display support:
- `epd-waveshare` - May support Good Display panels
- `good-display-epd` - Dedicated Good Display driver
- `eink-driver` - Generic e-ink driver
- Or implement from UC8151 datasheet directly

---

## Success Criteria

**Phase 5 Complete When:**
- ✅ Firmware boots on STM32H7
- ✅ Display initializes successfully
- ✅ Can draw primitives (lines, rectangles, text)
- ✅ Full refresh works (clears ghosting)
- ✅ Partial refresh works (fast updates)
- ✅ Emulator matches hardware behavior
- ✅ Basic UI screens render identically on both
- ✅ No quirks/bugs different from emulator

---

## Next Steps After Phase 5

**Phase 6: Audio Playback**
- I2S/SAI audio output
- FLAC/MP3 decoder integration
- SD card file system

**Phase 7: Input System**
- Rotary encoder driver
- Button matrix
- Touch panel (optional)

**Phase 8: Complete DAP**
- Bluetooth audio
- Library management
- Settings persistence
- Power management

---

## Recommendation

**Start with:** GDEW0213I5F (already have full spec and emulation)

**Parallel track:**
1. **Hardware track:** Order display, wire up STM32H7, implement driver
2. **Software track:** Build UI in emulator, test all screens, refine UX

**Estimated time:** 2-4 weeks to working display on hardware

**Ready to start?** Let me know and I'll help with:
- Creating the `firmware` crate structure
- Finding the right Good Display driver crate
- Implementing the display driver from UC8151 datasheet
- Building the first UI screens in the emulator
