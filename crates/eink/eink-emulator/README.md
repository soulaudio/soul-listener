# eink-emulator

Desktop emulator for e-ink displays with realistic behavior simulation.

## Phase 0: Foundation ✅ COMPLETE

Foundation for e-ink emulation built on research-backed best practices.

### Features

- ✅ **Window Management** - Cross-platform via winit + softbuffer
- ✅ **CPU Rendering** - No GPU dependency (more portable)
- ✅ **Gray4 Framebuffer** - 4-level grayscale matching real e-ink
- ✅ **embedded-graphics Integration** - DrawTarget trait
- ✅ **Headless Mode** - For CI/CD testing
- ✅ **Screenshot Capture** - Visual regression testing
- ✅ **2x Scaling** - Better desktop visibility

## Debug Features

Enable comprehensive debug tooling:

```bash
cargo run --features debug
```

See [docs/DEBUG.md](docs/DEBUG.md) for full documentation.

### Installation

Add to `Cargo.toml`:

```toml
[dependencies]
eink-emulator = { path = "path/to/eink-emulator" }
embedded-graphics = "0.8"
```

### Quick Start

```rust
use eink_emulator::{Emulator, DisplayDriver};
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Rectangle, PrimitiveStyle};
use embedded_graphics::pixelcolor::Gray4;

#[tokio::main]
async fn main() {
    // Create emulator (250×122 = Waveshare 2.13")
    let mut emulator = Emulator::new(250, 122);

    // Draw something
    Rectangle::new(Point::new(10, 10), Size::new(50, 30))
        .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
        .draw(&mut emulator)
        .unwrap();

    // Refresh display
    emulator.refresh().await.unwrap();

    // Show window
    emulator.run();
}
```

### Examples

Run the hello window example:

```bash
cargo run --target x86_64-pc-windows-msvc --example hello_window
```

**Note:** Must specify target since workspace defaults to embedded target.

### Headless Mode (CI)

```bash
cargo test --features headless
```

### Screenshot Testing

```rust
let emulator = Emulator::headless(250, 122);
// ... draw UI ...
emulator.screenshot("output.png").unwrap();
```

## Research-Informed Design

Based on analysis of:
- [embedded-graphics-simulator](https://github.com/embedded-graphics/simulator) - Testing patterns
- [softbuffer](https://github.com/rust-windowing/softbuffer) - CPU rendering
- [KDAB Best Practices](https://www.kdab.com/best-practices-embedded-development/) - Hardware isolation
- [Glider](https://github.com/Modos-Labs/Glider) - Real e-ink reference

### Design Decisions

✅ **CPU over GPU** - More portable, sufficient for e-ink
✅ **Isolated layers** - Window/framebuffer separation
✅ **Headless support** - Essential for CI/CD
✅ **2x scaling** - Better desktop UX

## Architecture

```
Application Code
    ↓
DrawTarget trait (embedded-graphics)
    ↓
Emulator
    ├─ Framebuffer (isolated)
    └─ Window (isolated)
```

## Next Phase

**Phase 1:** Display Specs
- Define DisplaySpec struct
- Waveshare & GoodDisplay templates
- Timing and characteristics

## License

MIT OR Apache-2.0
