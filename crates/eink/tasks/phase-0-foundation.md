# Phase 0: Foundation for Emulation (Research-Informed)

**Duration:** Day 1-2 (8 hours)
**Status:** 📋 Planning
**Dependencies:** None

## Overview

Build minimal emulation foundation based on research of existing implementations and best practices. Focus on proven patterns from embedded-graphics-simulator, softbuffer architecture, and hot reload techniques.

---

## Research Findings Summary

### Existing Implementations Studied

1. **[embedded-graphics-simulator](https://github.com/embedded-graphics/simulator)** - Official simulator for embedded-graphics
   - Uses SDL2 for window management
   - Supports headless mode for CI
   - Screenshot generation via environment variables
   - **Key Lesson:** Headless testing is critical for CI/CD

2. **[Glider](https://github.com/Modos-Labs/Glider)** - Open-source E-ink monitor
   - Real e-ink hardware driver
   - **Key Lesson:** Study actual refresh patterns from hardware

3. **[softbuffer](https://github.com/rust-windowing/softbuffer)** - Pixel buffer rendering
   - No GPU dependency (more portable)
   - Cross-platform via raw-window-handle
   - **Key Lesson:** CPU rendering is sufficient for e-ink simulation

### Best Practices Identified

From [KDAB Embedded Best Practices](https://www.kdab.com/best-practices-embedded-development/):
- **"Isolate the madness"** - Hardware-dependent code in separate modules
- **Regular hardware testing** - Nightly builds on target hardware
- **Simulator-first development** - Faster iteration cycles

From [fasterthanli.me hot reload article](https://fasterthanli.me/articles/so-you-want-to-live-reload-rust):
- **Library copying** - Required on Windows for hot reload
- **Avoid TLS** - Thread-local storage breaks hot reload
- **State serialization** - Preserve state across reloads

From [Blur Busters display testing](https://testufo.com/):
- **Refresh cycle simulation** - Expose multiple refresh cycles
- **Ghosting modeling** - Track pixel persistence over time

---

## Tasks

### 0.1: Project Scaffolding & Dependencies
**Effort:** 1 hour | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Create minimal crate structure following best practice of isolated hardware abstraction.

**Structure:**
```
crates/eink/eink-emulator/
├─ src/
│  ├─ lib.rs              # Public API
│  ├─ window.rs           # Window management (isolated)
│  ├─ framebuffer.rs      # Pixel buffer (isolated)
│  ├─ display_driver.rs   # DisplayDriver trait
│  └─ examples/
│     └─ hello_window.rs
├─ Cargo.toml
└─ README.md
```

**Cargo.toml** (based on softbuffer research):
```toml
[package]
name = "eink-emulator"
version = "0.1.0"
edition = "2021"

[dependencies]
# Window & rendering (no GPU - more portable per research)
winit = "0.30"
softbuffer = "0.4"

# Graphics primitives
embedded-graphics = "0.8"

[dev-dependencies]
tokio = { version = "1.0", features = ["full"] }

[features]
default = []
headless = []  # For CI/CD (per embedded-graphics-simulator pattern)
```

**Acceptance Criteria:**
- [ ] Crate compiles
- [ ] No GPU dependencies (follows softbuffer recommendation)
- [ ] Headless feature flag prepared

---

### 0.2: Window Management (Isolated Layer)
**Effort:** 2 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Window creation following "isolate the madness" principle - all platform-specific code isolated here.

**Implementation:**
```rust
// src/window.rs
use winit::event_loop::{EventLoop, ControlFlow};
use winit::window::WindowBuilder;
use winit::event::{Event, WindowEvent};
use softbuffer::{Context, Surface};

/// Isolated window management layer
///
/// Based on softbuffer pattern: https://github.com/rust-windowing/softbuffer
pub struct Window {
    #[cfg(not(feature = "headless"))]
    event_loop: EventLoop<()>,
    #[cfg(not(feature = "headless"))]
    window: winit::window::Window,
    #[cfg(not(feature = "headless"))]
    surface: Surface<winit::window::Window, winit::window::Window>,
}

impl Window {
    /// Create window with 2x scaling for visibility
    ///
    /// E-ink displays are typically 200-300 DPI, so we scale 2x
    /// for comfortable desktop viewing
    pub fn new(width: u32, height: u32) -> Self {
        #[cfg(not(feature = "headless"))]
        {
            let event_loop = EventLoop::new().unwrap();
            let window = WindowBuilder::new()
                .with_title("E-Ink Emulator")
                .with_inner_size(winit::dpi::PhysicalSize::new(width * 2, height * 2))
                .with_resizable(false)
                .build(&event_loop)
                .unwrap();

            let context = Context::new(&window).unwrap();
            let surface = Surface::new(&context, &window).unwrap();

            Self { event_loop, window, surface }
        }

        #[cfg(feature = "headless")]
        {
            // Headless mode for CI (per embedded-graphics-simulator pattern)
            Self {}
        }
    }

    pub fn present(&mut self, rgba_pixels: &[u32], width: u32, height: u32) {
        #[cfg(not(feature = "headless"))]
        {
            let mut buffer = self.surface.buffer_mut().unwrap();

            // 2x upscaling for visibility
            for y in 0..height {
                for x in 0..width {
                    let pixel = rgba_pixels[(y * width + x) as usize];
                    let sx = x * 2;
                    let sy = y * 2;
                    let scaled_width = width * 2;

                    // Write 2x2 block
                    buffer[(sy * scaled_width + sx) as usize] = pixel;
                    buffer[(sy * scaled_width + sx + 1) as usize] = pixel;
                    buffer[((sy + 1) * scaled_width + sx) as usize] = pixel;
                    buffer[((sy + 1) * scaled_width + sx + 1) as usize] = pixel;
                }
            }

            buffer.present().unwrap();
        }
    }

    pub fn run(mut self, mut update_fn: impl FnMut() + 'static) {
        #[cfg(not(feature = "headless"))]
        {
            self.event_loop.run(move |event, elwt| {
                elwt.set_control_flow(ControlFlow::Wait);

                match event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => elwt.exit(),

                    Event::WindowEvent {
                        event: WindowEvent::RedrawRequested,
                        ..
                    } => {
                        update_fn();
                    }
                    _ => {}
                }
            }).unwrap();
        }
    }
}
```

**Acceptance Criteria:**
- [ ] Window opens (non-headless)
- [ ] Headless mode compiles
- [ ] Cross-platform verified (Windows, Linux, macOS)
- [ ] 2x scaling for readability

---

### 0.3: Framebuffer with Gray4 Support
**Effort:** 2 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
CPU-based framebuffer (no GPU) optimized for grayscale e-ink.

**Implementation:**
```rust
// src/framebuffer.rs
use embedded_graphics::pixelcolor::Gray4;

/// CPU-based framebuffer for e-ink simulation
///
/// Uses Gray4 (4-level grayscale) matching real e-ink displays.
/// Per research: CPU rendering is sufficient and more portable than GPU.
pub struct Framebuffer {
    pub pixels: Vec<Gray4>,
    pub width: u32,
    pub height: u32,
}

impl Framebuffer {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            pixels: vec![Gray4::WHITE; (width * height) as usize],
            width,
            height,
        }
    }

    pub fn set_pixel(&mut self, x: u32, y: u32, color: Gray4) {
        if x < self.width && y < self.height {
            let idx = (y * self.width + x) as usize;
            self.pixels[idx] = color;
        }
    }

    pub fn get_pixel(&self, x: u32, y: u32) -> Option<Gray4> {
        if x < self.width && y < self.height {
            Some(self.pixels[(y * self.width + x) as usize])
        } else {
            None
        }
    }

    /// Convert Gray4 to RGBA for display
    ///
    /// Gray4 levels (0-3) map to (0, 85, 170, 255)
    /// This gives proper visual spacing for 4-level grayscale
    pub fn to_rgba(&self) -> Vec<u32> {
        self.pixels
            .iter()
            .map(|gray| {
                let level = gray.luma() * 85; // 0-3 → 0, 85, 170, 255
                let rgb = (level as u32) | ((level as u32) << 8) | ((level as u32) << 16);
                rgb | 0xFF000000 // Alpha = 255
            })
            .collect()
    }

    pub fn fill(&mut self, color: Gray4) {
        self.pixels.fill(color);
    }

    pub fn clear(&mut self) {
        self.fill(Gray4::WHITE);
    }
}
```

**Acceptance Criteria:**
- [ ] Efficient pixel access
- [ ] Proper Gray4 → RGBA conversion
- [ ] Bounds checking

---

### 0.4: DisplayDriver Trait (Minimal)
**Effort:** 1.5 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Minimal DisplayDriver trait - will expand in Phase 2.

**Implementation:**
```rust
// src/display_driver.rs

/// Display driver abstraction
///
/// Follows "isolate the madness" principle - application code
/// should never directly interact with hardware/window.
pub trait DisplayDriver {
    type Error;

    /// Refresh display (basic for now, will add modes in Phase 2)
    fn refresh(&mut self) -> impl std::future::Future<Output = Result<(), Self::Error>>;
}
```

---

### 0.5: embedded-graphics DrawTarget
**Effort:** 1.5 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Implement DrawTarget to use embedded-graphics primitives.

**Implementation:**
```rust
// src/lib.rs
use embedded_graphics::prelude::*;
use embedded_graphics::pixelcolor::Gray4;

pub struct Emulator {
    framebuffer: Framebuffer,
    window: Option<Window>,  // None in headless mode
}

impl Emulator {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            framebuffer: Framebuffer::new(width, height),
            #[cfg(not(feature = "headless"))]
            window: Some(Window::new(width, height)),
            #[cfg(feature = "headless")]
            window: None,
        }
    }

    pub fn headless(width: u32, height: u32) -> Self {
        Self {
            framebuffer: Framebuffer::new(width, height),
            window: None,
        }
    }
}

impl DrawTarget for Emulator {
    type Color = Gray4;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(point, color) in pixels {
            if point.x >= 0 && point.y >= 0 {
                self.framebuffer.set_pixel(
                    point.x as u32,
                    point.y as u32,
                    color,
                );
            }
        }
        Ok(())
    }
}

impl OriginDimensions for Emulator {
    fn size(&self) -> Size {
        Size::new(self.framebuffer.width, self.framebuffer.height)
    }
}

impl DisplayDriver for Emulator {
    type Error = std::io::Error;

    async fn refresh(&mut self) -> Result<(), Self::Error> {
        if let Some(window) = &mut self.window {
            let rgba = self.framebuffer.to_rgba();
            window.present(&rgba, self.framebuffer.width, self.framebuffer.height);
        }
        Ok(())
    }
}
```

---

### 0.6: Example Program
**Effort:** 1 hour | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Working example demonstrating the foundation.

**Example:**
```rust
// examples/hello_window.rs
use eink_emulator::Emulator;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Rectangle, PrimitiveStyle, RoundedRectangle, CornerRadii};
use embedded_graphics::mono_font::{MonoTextStyle, ascii::FONT_9X18_BOLD};
use embedded_graphics::text::Text;
use embedded_graphics::pixelcolor::Gray4;

#[tokio::main]
async fn main() {
    // Create emulator for Waveshare 2.13" size
    let mut emulator = Emulator::new(250, 122);

    // Draw background
    emulator.framebuffer.clear();

    // Draw border
    RoundedRectangle::new(
        Rectangle::new(Point::new(5, 5), Size::new(240, 112)),
        CornerRadii::new(Size::new(8, 8)),
    )
    .into_styled(PrimitiveStyle::with_stroke(Gray4::BLACK, 2))
    .draw(&mut emulator)
    .unwrap();

    // Draw title
    Text::new(
        "E-Ink Emulator",
        Point::new(40, 50),
        MonoTextStyle::new(&FONT_9X18_BOLD, Gray4::BLACK),
    )
    .draw(&mut emulator)
    .unwrap();

    // Draw subtitle
    Text::new(
        "Phase 0 Complete!",
        Point::new(60, 80),
        MonoTextStyle::new(&FONT_6X10, Gray4::new(2)),
    )
    .draw(&mut emulator)
    .unwrap();

    // Refresh and show
    emulator.refresh().await.unwrap();

    println!("Window opened! Close to exit.");
    emulator.run();
}
```

**Run with:**
```bash
cargo run --example hello_window
cargo run --example hello_window --features headless  # CI mode
```

---

### 0.7: Screenshot Support (For CI)
**Effort:** 1 hour | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Screenshot generation following embedded-graphics-simulator pattern.

**Implementation:**
```rust
// src/lib.rs
impl Emulator {
    /// Save screenshot to PNG (for testing)
    ///
    /// Per embedded-graphics-simulator pattern: use environment
    /// variables for automated testing.
    pub fn screenshot(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        use image::{GrayImage, Luma};

        let mut img = GrayImage::new(self.framebuffer.width, self.framebuffer.height);

        for (i, pixel) in self.framebuffer.pixels.iter().enumerate() {
            let x = (i as u32) % self.framebuffer.width;
            let y = (i as u32) / self.framebuffer.width;
            let gray = pixel.luma() * 85;  // 0-3 → 0-255
            img.put_pixel(x, y, Luma([gray]));
        }

        img.save(path)?;
        Ok(())
    }
}
```

---

## Phase Completion Checklist

- [ ] Window opens and displays correctly
- [ ] Headless mode works
- [ ] Can draw using embedded-graphics
- [ ] Screenshot capture works
- [ ] Example program runs
- [ ] Cross-platform verified
- [ ] Code follows "isolate the madness" principle
- [ ] No GPU dependencies

---

## Research-Informed Decisions

✅ **softbuffer over GPU** - More portable, simpler, sufficient for e-ink
✅ **Headless mode** - Essential for CI/CD
✅ **2x scaling** - Better desktop visibility
✅ **Isolated layers** - Window/framebuffer separation
✅ **Screenshot testing** - Following embedded-graphics-simulator pattern

---

## References

- [embedded-graphics-simulator](https://github.com/embedded-graphics/simulator) - Testing patterns
- [softbuffer](https://github.com/rust-windowing/softbuffer) - CPU rendering approach
- [KDAB Best Practices](https://www.kdab.com/best-practices-embedded-development/) - Hardware isolation
- [fasterthanli.me hot reload](https://fasterthanli.me/articles/so-you-want-to-live-reload-rust) - Hot reload patterns
- [Glider E-ink Monitor](https://github.com/Modos-Labs/Glider) - Real hardware reference

---

**Estimated Completion:** Day 2
**Next Phase:** Phase 1 (Display Specs)
