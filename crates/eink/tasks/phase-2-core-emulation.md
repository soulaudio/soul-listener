# Phase 4: Emulation (eink-emulator)

**Duration:** Week 4-5 (60 hours)
**Status:** ⏳ Pending
**Dependencies:** Phase 1 (Tasks 1.1-1.4), Phase 2 (Tasks 2.1-2.10)

## Overview

Build desktop emulator with realistic e-ink simulation (ghosting, refresh timing, grayscale), hot reload support, and headless mode for testing.

---

## Tasks

### 4.1: Window Setup - winit Integration
**Effort:** 4 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Set up window creation and event loop with winit.

**Acceptance Criteria:**
- [ ] Create window with configurable size
- [ ] Event loop handling
- [ ] Window title shows display spec name
- [ ] Close button works
- [ ] Keyboard input (for later)

**Implementation:**
```rust
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;

pub struct Window {
    window: winit::window::Window,
    event_loop: EventLoop<()>,
}

impl Window {
    pub fn new(width: u32, height: u32, title: &str) -> Self {
        let event_loop = EventLoop::new().unwrap();
        let window = WindowBuilder::new()
            .with_title(title)
            .with_inner_size(winit::dpi::PhysicalSize::new(width, height))
            .with_resizable(false)
            .build(&event_loop)
            .unwrap();

        Self { window, event_loop }
    }
}
```

**Verification:**
- [ ] Window opens on Windows
- [ ] Window opens on Linux (X11 and Wayland)
- [ ] Window opens on macOS

---

### 4.2: Pixel Buffer - softbuffer Integration
**Effort:** 3 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Set up pixel buffer rendering with softbuffer.

**Acceptance Criteria:**
- [ ] Create softbuffer context
- [ ] Create surface from window
- [ ] Write pixels to buffer
- [ ] Present buffer to screen
- [ ] Handle window resize

**Implementation:**
```rust
use softbuffer::{Context, Surface};

pub struct PixelBuffer {
    context: Context,
    surface: Surface,
}

impl PixelBuffer {
    pub fn new(window: &winit::window::Window) -> Self {
        let context = Context::new(window).unwrap();
        let surface = Surface::new(&context, window).unwrap();
        Self { context, surface }
    }

    pub fn present(&mut self, pixels: &[u32]) {
        let mut buffer = self.surface.buffer_mut().unwrap();
        buffer.copy_from_slice(pixels);
        buffer.present().unwrap();
    }
}
```

---

### 4.3: Framebuffer - Gray4 to RGBA Conversion
**Effort:** 3 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Convert Gray4 pixels to RGBA for softbuffer.

**Acceptance Criteria:**
- [ ] `Framebuffer` struct storing Gray4 pixels
- [ ] Conversion to u32 RGBA
- [ ] Grayscale mapping (0-3 → 0-255)
- [ ] Efficient conversion (simd optional)

**Implementation:**
```rust
use embedded_graphics::pixelcolor::Gray4;

pub struct Framebuffer {
    pixels: Vec<Gray4>,
    width: u32,
    height: u32,
}

impl Framebuffer {
    pub fn to_rgba(&self) -> Vec<u32> {
        self.pixels
            .iter()
            .map(|gray| {
                let level = gray.luma() * 85; // 0, 85, 170, 255
                let rgb = (level as u32) | ((level as u32) << 8) | ((level as u32) << 16);
                rgb | 0xFF000000 // Alpha = 255
            })
            .collect()
    }
}
```

---

### 4.4: Ghosting Simulation - State Tracking
**Effort:** 4 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Track ghosting state for each pixel.

**Acceptance Criteria:**
- [ ] `GhostingState` struct
- [ ] Per-pixel ghost level (0.0 to 1.0)
- [ ] Accumulation on partial refresh
- [ ] Clear on full refresh
- [ ] Exponential decay over time

**Implementation:**
```rust
pub struct GhostingState {
    ghost_levels: Vec<f32>,  // 0.0 = no ghost, 1.0 = fully ghosted
    width: u32,
    height: u32,
}

impl GhostingState {
    pub fn accumulate(&mut self, rate: f32) {
        for level in &mut self.ghost_levels {
            *level = (*level + rate).min(1.0);
        }
    }

    pub fn clear(&mut self) {
        self.ghost_levels.fill(0.0);
    }

    pub fn decay(&mut self, delta_time: f32, decay_rate: f32) {
        for level in &mut self.ghost_levels {
            *level = (*level - decay_rate * delta_time).max(0.0);
        }
    }
}
```

---

### 4.5: Ghosting Visualization
**Effort:** 4 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Render ghosting effect visually.

**Acceptance Criteria:**
- [ ] Blend previous frame with current frame
- [ ] Ghost level affects blend alpha
- [ ] Realistic ghosting appearance
- [ ] Toggle ghosting visualization on/off

**Implementation:**
```rust
pub fn render_with_ghosting(
    current: &Framebuffer,
    previous: &Framebuffer,
    ghosting: &GhostingState,
) -> Vec<u32> {
    current
        .pixels
        .iter()
        .zip(&previous.pixels)
        .zip(&ghosting.ghost_levels)
        .map(|((cur, prev), ghost)| {
            let cur_luma = cur.luma() * 85;
            let prev_luma = prev.luma() * 85;
            let blended = (cur_luma as f32 * (1.0 - ghost) + prev_luma as f32 * ghost) as u8;
            let rgb = (blended as u32) | ((blended as u32) << 8) | ((blended as u32) << 16);
            rgb | 0xFF000000
        })
        .collect()
}
```

---

### 4.6: Refresh Animation - Full Refresh
**Effort:** 5 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Animate full refresh with 3 flashes (black → white → black → target).

**Acceptance Criteria:**
- [ ] Flash sequence implementation
- [ ] Timing based on display spec (e.g., 2000ms total)
- [ ] Visual flashing effect
- [ ] Async/await support

**Implementation:**
```rust
impl Emulator {
    pub async fn animate_full_refresh(&mut self) -> Result<()> {
        let spec = &self.spec;
        let flash_duration = spec.full_refresh_duration_ms() / 4;

        // Flash to black
        self.framebuffer.fill(Gray4::BLACK);
        self.present();
        tokio::time::sleep(Duration::from_millis(flash_duration as u64)).await;

        // Flash to white
        self.framebuffer.fill(Gray4::WHITE);
        self.present();
        tokio::time::sleep(Duration::from_millis(flash_duration as u64)).await;

        // Flash to black
        self.framebuffer.fill(Gray4::BLACK);
        self.present();
        tokio::time::sleep(Duration::from_millis(flash_duration as u64)).await;

        // Final target state
        self.present();
        tokio::time::sleep(Duration::from_millis(flash_duration as u64)).await;

        Ok(())
    }
}
```

---

### 4.7: Refresh Animation - Partial Refresh
**Effort:** 3 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Subtle partial refresh animation (no flashing).

**Acceptance Criteria:**
- [ ] Timing delay (300ms typical)
- [ ] No flashing
- [ ] Accumulate ghosting
- [ ] Async/await support

---

### 4.8: Temperature Simulation
**Effort:** 3 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Simulate temperature effects on refresh timing and quality.

**Acceptance Criteria:**
- [ ] Temperature property (-15°C to 65°C)
- [ ] Refresh timing scales with temperature (slower at low temp)
- [ ] Visual artifacts at extreme temps
- [ ] UI control to adjust temperature

**Effects:**
- Below 0°C: 1.5x slower refresh
- Above 40°C: 1.2x slower, slight ghosting increase

---

### 4.9: DisplayDriver Trait Implementation
**Effort:** 4 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Implement platform::DisplayDriver trait for emulator.

**Acceptance Criteria:**
- [ ] Implements `DisplayDriver` trait
- [ ] `refresh_full()` triggers full refresh animation
- [ ] `refresh_partial()` triggers partial refresh + ghosting
- [ ] `sleep()` and `wake()` methods
- [ ] Async methods work correctly

**Implementation:**
```rust
use platform::DisplayDriver;

impl DisplayDriver for Emulator {
    type Error = EmulatorError;

    async fn refresh_full(&mut self) -> Result<(), Self::Error> {
        self.ghosting_state.clear();
        self.animate_full_refresh().await?;
        self.stats.full_refresh_count += 1;
        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::Error> {
        let rate = self.spec.partial_ghosting_rate();
        self.ghosting_state.accumulate(rate);
        self.animate_partial_refresh().await?;
        self.stats.partial_refresh_count += 1;
        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::Error> {
        self.sleeping = true;
        Ok(())
    }

    async fn wake(&mut self) -> Result<(), Self::Error> {
        self.sleeping = false;
        Ok(())
    }
}
```

---

### 4.10: DrawTarget Trait Implementation
**Effort:** 3 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Implement embedded-graphics DrawTarget for emulator.

**Acceptance Criteria:**
- [ ] Implements `DrawTarget<Color = Gray4>`
- [ ] `draw_iter()` writes pixels to framebuffer
- [ ] Bounds checking
- [ ] Efficient pixel writes

---

### 4.11: Headless Mode
**Effort:** 4 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Run emulator without window for testing.

**Acceptance Criteria:**
- [ ] `headless` flag in builder
- [ ] No window created in headless mode
- [ ] All rendering still works
- [ ] CI-compatible (no display server required)

**Implementation:**
```rust
pub struct EmulatorBuilder {
    spec: DisplaySpec,
    headless: bool,
}

impl EmulatorBuilder {
    pub fn headless(mut self, headless: bool) -> Self {
        self.headless = headless;
        self
    }

    pub fn build(self) -> Emulator {
        let window = if self.headless {
            None
        } else {
            Some(Window::new(self.spec.width, self.spec.height, self.spec.name))
        };

        Emulator { window, /* ... */ }
    }
}
```

---

### 4.12: Screenshot Capture
**Effort:** 3 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Capture framebuffer to PNG file.

**Acceptance Criteria:**
- [ ] `screenshot(path)` method
- [ ] Saves as PNG using `image` crate
- [ ] Grayscale PNG (not RGB for efficiency)
- [ ] Works in headless mode

**Implementation:**
```rust
use image::{GrayImage, Luma};

impl Emulator {
    pub fn screenshot(&self, path: impl AsRef<Path>) -> Result<()> {
        let mut img = GrayImage::new(self.spec.width, self.spec.height);

        for (i, pixel) in self.framebuffer.pixels.iter().enumerate() {
            let x = (i as u32) % self.spec.width;
            let y = (i as u32) / self.spec.width;
            let gray = pixel.luma() * 85;  // 0-3 → 0-255
            img.put_pixel(x, y, Luma([gray]));
        }

        img.save(path)?;
        Ok(())
    }
}
```

---

### 4.13: Statistics Tracking
**Effort:** 2 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Track refresh statistics for testing and debugging.

**Acceptance Criteria:**
- [ ] `RefreshStats` struct
- [ ] Count full/partial refreshes
- [ ] Total refresh time
- [ ] Ghosting level queries
- [ ] Getter methods

---

### 4.14: Hot Reload Infrastructure
**Effort:** 8 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Implement hot reload using `hot-lib-reloader`.

**Acceptance Criteria:**
- [ ] `HotReloadEmulator` wrapper
- [ ] `UiLibrary` trait for reloadable code
- [ ] File watching for UI library changes
- [ ] Reload latency < 1 second
- [ ] State preservation across reloads

**Implementation:**
```rust
use hot_lib_reloader::{LibReloader, Symbol};

#[repr(C)]
pub struct UiLibrary {
    pub render: Symbol<unsafe extern "C" fn(&mut dyn DisplayDriver, &AppState)>,
}

pub struct HotReloadEmulator {
    emulator: Emulator,
    ui_lib: LibReloader<UiLibrary>,
    state: AppState,
}

impl HotReloadEmulator {
    pub fn run(&mut self) {
        loop {
            // Check for library changes
            if self.ui_lib.update() {
                println!("🔥 Hot reloaded UI library!");
            }

            // Call into hot-reloadable code
            unsafe {
                (self.ui_lib.render)(&mut self.emulator, &self.state);
            }

            // Event handling
            // ...
        }
    }
}
```

**Challenges:**
- State serialization/deserialization
- Function signature compatibility
- Cross-platform dynamic loading (`.so`, `.dll`, `.dylib`)

---

### 4.15: Input Simulation
**Effort:** 4 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Simulate button inputs from keyboard/mouse.

**Acceptance Criteria:**
- [ ] Mouse click → button press
- [ ] Keyboard shortcuts (space = play/pause, arrow keys = prev/next)
- [ ] Click bounds checking
- [ ] Event queue

---

### 4.16: Control Panel UI (Optional)
**Effort:** 6 hours | **Priority:** Low | **Status:** ⏳ Pending

**Description:**
Add control panel with emulator settings (using egui).

**Features:**
- [ ] Temperature slider
- [ ] Force refresh buttons
- [ ] Ghosting level display
- [ ] Statistics display
- [ ] Display spec selector

**Optional:** Can defer to Phase 6 (Polish)

---

### 4.17: Cross-Platform Testing
**Effort:** 4 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Verify emulator works on all platforms.

**Acceptance Criteria:**
- [ ] Windows 11 tested
- [ ] Linux (Ubuntu, X11) tested
- [ ] Linux (Wayland) tested
- [ ] macOS tested
- [ ] CI runs headless tests on all platforms

---

### 4.18: Documentation & Examples
**Effort:** 5 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Complete documentation and example programs.

**Acceptance Criteria:**
- [ ] Rustdoc on all public APIs
- [ ] Architecture documentation
- [ ] Hot reload guide
- [ ] 3+ example programs

**Examples:**
1. `basic_emulator.rs` - Simple window with text
2. `hot_reload_demo.rs` - Hot reload demonstration
3. `screenshot_test.rs` - Headless screenshot capture

---

## Phase Completion Checklist

- [ ] All tasks completed
- [ ] Emulator visually matches real e-ink
- [ ] Hot reload works reliably
- [ ] Cross-platform compatibility verified
- [ ] Headless mode works in CI
- [ ] Tests pass
- [ ] Documentation complete
- [ ] Code review complete

---

**Dependencies for Next Phase:**
- Phase 5 (Testing) depends on:
  - Task 4.11 (Headless mode)
  - Task 4.12 (Screenshot capture)

**Estimated Completion:** End of Week 5
