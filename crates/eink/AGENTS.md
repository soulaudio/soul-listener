# E-Ink Ecosystem - Vertical Slice

**Context for AI Assistant**

This directory contains a complete, publishable ecosystem for building e-ink display applications in Rust with embedded-graphics. These crates are designed to be reusable beyond the SoulAudio DAP project.

## Vision

Create a comprehensive, production-ready e-ink UI framework for embedded Rust that:
- **Abstracts display hardware** - Works with any e-ink controller (Waveshare, GoodDisplay, etc.)
- **Desktop emulation** - Develop and test without hardware using realistic e-ink simulation
- **Declarative UI** - React-like component model that compiles to embedded-graphics primitives
- **CSS-like styling** - Familiar layout system (flexbox) and theming
- **Hot reload** - Instant feedback during development
- **Testable** - Playwright-like testing with test IDs and screenshot comparison
- **no_std compatible** - Runs on embedded devices with minimal resources

## Architecture

### Vertical Slice Structure

```
crates/eink/
├─ eink-specs/          # Display specifications (pure data)
├─ eink-system/         # Layout + styling engine (core framework)
├─ eink-components/     # Standard reusable components
├─ eink-emulator/       # Desktop emulator with hot reload
└─ eink-testing/        # Testing utilities and assertions
```

### Dependency Flow

```
                    Application Code
                          ↓
    ┌─────────────────────┴─────────────────────┐
    ↓                                           ↓
eink-components                              soul-ui
    ↓                                    (DAP-specific)
eink-system
    ↓
embedded-graphics
    ↓
┌───────────────────┐               ┌────────────────┐
│ Hardware Driver   │    OR         │ eink-emulator  │
│ (epd-waveshare)   │               │ (desktop sim)  │
└───────────────────┘               └────────────────┘
         ↓                                   ↓
    Platform HAL                        Testing Suite
   (DisplayDriver)                     (eink-testing)
```

### Abstraction Layers

**Layer 1: eink-specs** (Data)
- Pure data structures describing e-ink display characteristics
- No dependencies, no_std, serde optional
- Predefined constants for popular displays
- TOML loading for custom displays

**Layer 2: eink-system** (Engine)
- Layout engine (flexbox-like)
- Styling system (CSS-inspired)
- Theme definitions
- Text measurement and flow
- **no_std compatible** - Core allocation-free algorithms

**Layer 3: eink-components** (Components)
- Standard UI building blocks (Button, Text, ProgressBar, List, etc.)
- Implements View trait
- Compiles to embedded-graphics primitives
- **no_std compatible** with `heapless` collections

**Layer 4: eink-emulator** (Simulation)
- Desktop window using winit + softbuffer
- Realistic e-ink behavior simulation (ghosting, refresh timing, grayscale)
- Hot reload support with `hot-lib-reloader`
- Screenshot capture for testing
- **std only** - Desktop development tool

**Layer 5: eink-testing** (Testing)
- Test ID query system (`query_by_test_id`)
- Screenshot comparison (golden testing)
- Accessibility tree inspection
- Headless mode support
- **std only** - Testing infrastructure

## Key Design Principles

### 1. Compile to Primitives
All components ultimately compile down to `embedded-graphics` primitives:
```rust
// Declarative
VStack::new(&[
    &Text::new("Title"),
    &Button::new("Click"),
])

// Compiles to
Rectangle::draw(&mut display)?;
Text::draw(&mut display)?;
```

### 2. Test IDs for Robust Testing
Every component supports test IDs (like React's `data-testid`):
```rust
let button = Button::new("Play")
    .test_id("play-button");

// Test
let element = emulator.query_by_test_id("play-button")?;
assert_eq!(element.text(), "Play");
```

### 3. Theme-Driven Styling
Consistent theming across the application:
```rust
let theme = Theme {
    colors: ColorPalette { /* ... */ },
    typography: Typography { /* ... */ },
    spacing: Spacing { xs: 4, sm: 8, md: 16, lg: 24, xl: 32 },
};

Text::new("Hello").style(theme.typography.heading1)
```

### 4. Smart Refresh Management
Components track dirty regions and decide optimal refresh strategy:
```rust
// Component library decides when to refresh
ui.update_progress(0.5);  // Marks dirty region
ui.render()?;             // Calls display.refresh_partial()

// After 5 partial refreshes, triggers full refresh to clear ghosting
```

### 5. Seamless Hardware/Emulator Swap
Both implement the same `DisplayDriver` trait:
```rust
async fn run<D: DisplayDriver>(mut display: D) {
    NowPlayingScreen::render(&mut display)?;
}

// Works with EITHER:
run(EinkHardware::new(/* SPI */));  // Real hardware
run(Emulator::new(spec));            // Desktop emulator
```

## Development Workflow

### 1. Desktop Development (Hot Reload)
```bash
# Terminal 1: Run emulator
cargo run --bin simulator

# Terminal 2: Watch for UI changes
cd crates/soul-ui
cargo watch -x "build --lib"

# Edit soul-ui code → Save → See changes instantly!
```

### 2. Testing
```bash
# Unit tests
cargo test -p eink-components

# Visual regression tests
cargo test -p soul-ui --test visual_tests

# Integration tests with emulator
cargo test -p simulator --test integration_tests
```

### 3. Hardware Deployment
```bash
# Build for embedded target
cargo build --release --target thumbv7em-none-eabihf -p firmware

# Flash to device
probe-rs run --chip STM32H743ZITx target/thumbv7em-none-eabihf/release/firmware
```

## Hot Reload Architecture

### Library Setup
```toml
# soul-ui/Cargo.toml
[lib]
crate-type = ["rlib", "cdylib"]  # Static AND dynamic
```

### Emulator Integration
```rust
use hot_lib_reloader::{LibReloader, ReloadHandler};

pub struct HotReloadEmulator {
    emulator: Emulator,
    ui_lib: LibReloader<UiLibrary>,
}

impl HotReloadEmulator {
    pub fn run(&mut self, state: AppState) {
        loop {
            // Check for recompiled library
            if self.ui_lib.reload() {
                println!("🔥 Hot reloaded UI!");
            }

            // Render with hot-reloadable code
            self.ui_lib.render(&mut self.emulator, &state)?;
        }
    }
}
```

## Testing Strategy

### 1. Unit Tests (Components)
Test individual components in isolation:
```rust
#[test]
fn test_button_layout() {
    let button = Button::new("Click");
    let layout = button.layout(Constraints::tight(Size::new(100, 40)), &theme);
    assert_eq!(layout.size, Size::new(100, 40));
}
```

### 2. Visual Regression Tests
Screenshot comparison for UI consistency:
```rust
#[test]
fn test_now_playing_screen() {
    let mut emulator = Emulator::headless(displays::WAVESHARE_2_13_V4);

    NowPlayingScreen::render(&mut emulator, &mock_state())?;
    emulator.screenshot("tests/screenshots/now_playing.png")?;

    assert_snapshot!("now_playing", emulator);
}
```

### 3. Integration Tests
Test user interactions and state changes:
```rust
#[tokio::test]
async fn test_play_pause_interaction() {
    let mut emulator = Emulator::new(displays::WAVESHARE_2_13_V4);

    // Find play button
    let play_button = emulator.query_by_test_id("play-button")?;

    // Simulate tap
    emulator.tap(play_button.center())?;

    // Verify state change
    assert_eq!(playback_state.playing, true);
}
```

### 4. E-Ink Behavior Tests
Verify ghosting, refresh timing, grayscale:
```rust
#[tokio::test]
async fn test_ghosting_accumulation() {
    let mut emulator = Emulator::headless(displays::WAVESHARE_2_13_V4);

    // 5 partial refreshes should accumulate ghosting
    for i in 0..5 {
        update_progress(&mut emulator, i * 20)?;
        emulator.refresh_partial().await?;
    }

    assert!(emulator.ghosting_level() > 0.6);

    // Full refresh should clear ghosting
    emulator.refresh_full().await?;
    assert_eq!(emulator.ghosting_level(), 0.0);
}
```

## Memory Constraints

### Embedded Target (no_std)
- **Stack-based allocation** - Use `heapless::Vec` for bounded collections
- **Static strings** - `&'static str` for all text content
- **Const generics** - Size known at compile time
- **No dynamic dispatch** - Monomorphization preferred over trait objects

Example:
```rust
// Good - stack allocated
let children: heapless::Vec<&dyn View, 8> = heapless::Vec::new();

// Bad - heap allocation
let children: Vec<Box<dyn View>> = Vec::new();  // Won't compile in no_std
```

### Desktop Target (std)
- Full standard library available
- Can use `Vec`, `String`, `Box`, etc.
- Hot reload requires dynamic library loading

## E-Ink Display Characteristics

### Refresh Modes
1. **Full Refresh** (~2000ms)
   - Flashes 3 times (black → white → black → target)
   - 16-level grayscale
   - Clears ghosting completely
   - Use for: Page transitions, every 5-10 partial refreshes

2. **Partial Refresh** (~300ms)
   - Updates only changed pixels
   - 4-level grayscale
   - Accumulates ghosting (15% per refresh)
   - Use for: Progress bars, animations, live updates

3. **Fast Refresh** (~260ms)
   - Single flash
   - 2-level (black/white only)
   - High ghosting accumulation (25% per refresh)
   - Use for: Page turns, scrolling

### Ghosting Management
```rust
pub struct RefreshStrategy {
    partial_count: usize,
    full_refresh_threshold: usize,  // Default: 5
}

impl RefreshStrategy {
    pub async fn refresh<D: DisplayDriver>(&mut self, display: &mut D, mode: RefreshMode) {
        match mode {
            RefreshMode::Partial => {
                display.refresh_partial().await?;
                self.partial_count += 1;

                if self.partial_count >= self.full_refresh_threshold {
                    display.refresh_full().await?;
                    self.partial_count = 0;
                }
            }
            RefreshMode::Full => {
                display.refresh_full().await?;
                self.partial_count = 0;
            }
        }
    }
}
```

## Dependencies

### Core (no_std)
```toml
embedded-graphics = "0.8"
heapless = "0.8"
```

### Desktop (std)
```toml
winit = "0.30"
softbuffer = "0.4"
hot-lib-reloader = "0.8"
image = "0.25"  # Screenshot support
```

### Optional
```toml
serde = { version = "1.0", optional = true }  # TOML loading
defmt = { version = "0.3", optional = true }  # Embedded logging
```

## Publishing Strategy

All `eink-*` crates are designed to be published to crates.io:
- **eink-specs** - v0.1.0 - Display specifications
- **eink-system** - v0.1.0 - Layout and styling engine
- **eink-components** - v0.1.0 - Standard components
- **eink-emulator** - v0.1.0 - Desktop emulator
- **eink-testing** - v0.1.0 - Testing utilities

`soul-ui` remains private to SoulAudio DAP project.

## Documentation Standards

Each crate must have:
- ✅ Comprehensive README.md
- ✅ Rustdoc on all public APIs
- ✅ Code examples in doc comments
- ✅ CHANGELOG.md
- ✅ Example programs in `examples/`

## File Structure Template

```
eink-<name>/
├─ src/
│  ├─ lib.rs
│  └─ ...
├─ examples/
│  └─ basic.rs
├─ tests/
│  ├─ unit_tests.rs
│  └─ integration_tests.rs
├─ benches/           # Optional
│  └─ benchmarks.rs
├─ Cargo.toml
├─ README.md
└─ CHANGELOG.md
```

## Next Steps

See `ROADMAP.md` for implementation phases and `TASKS.md` for detailed task breakdown.

---

**Last Updated:** 2026-02-14
**Status:** Planning Phase - Documentation Complete, Implementation Pending
