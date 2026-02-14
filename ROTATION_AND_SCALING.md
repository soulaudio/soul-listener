# Rotation and Scaling Configuration

The e-ink emulator supports configurable display rotation and scaling for flexible development.

## Key Concept: Presentation-Only Transform

**Important:** Rotation and scaling are **presentation-only** transformations. They affect how the window is displayed, but **do not change the HAL coordinate system**.

- DrawTarget coordinates remain **logical** (e.g., 800×480 for GDEM0397T81P)
- Rotation is applied **after** physics simulation and **before** window rendering
- Your drawing code doesn't need to know about rotation

## Configuration

### EmulatorConfig Structure

```rust
pub struct EmulatorConfig {
    /// Display rotation (affects window orientation only)
    pub rotation: Rotation,
    /// Upscaling factor (1 = no scaling, 2 = 2x, etc.)
    pub scale: u32,
}
```

### Rotation Modes

```rust
pub enum Rotation {
    Degrees0,      // No rotation (landscape: 800×480)
    Degrees90,     // Rotate 90° clockwise (portrait: 480×800)
    Degrees180,    // Upside down (landscape: 800×480)
    Degrees270,    // Rotate 90° counter-clockwise (portrait: 480×800)
}
```

### Predefined Configurations

```rust
// Default: 2x upscaling, no rotation
EmulatorConfig::DEFAULT

// 1:1 pixel mapping, no rotation
EmulatorConfig::NATIVE

// Portrait mode, no upscaling
EmulatorConfig::PORTRAIT

// Portrait mode, 2x upscaling
EmulatorConfig::PORTRAIT_2X
```

## Usage Examples

### Portrait Mode (90° rotation, no upscaling)

```rust
use eink_emulator::{Emulator, EmulatorConfig, Rotation};

let config = EmulatorConfig {
    rotation: Rotation::Degrees90,
    scale: 1,
};

let mut emulator = Emulator::with_config(config);
```

### Landscape 2x (default)

```rust
let emulator = Emulator::with_spec(&DISPLAY_SPEC); // Uses DEFAULT config
```

### Custom Configuration

```rust
let config = EmulatorConfig {
    rotation: Rotation::Degrees180,  // Upside-down
    scale: 3,                         // 3x upscaling for very high visibility
};

let mut emulator = Emulator::with_spec_and_config(&DISPLAY_SPEC, config);
```

## Window Size Calculation

The window size is calculated based on rotation and scale:

```
Logical display: 800×480

Rotation::Degrees0:
  Window: 800×480 → Scaled: (800 * scale) × (480 * scale)

Rotation::Degrees90:
  Window: 480×800 → Scaled: (480 * scale) × (800 * scale)

Rotation::Degrees180:
  Window: 800×480 → Scaled: (800 * scale) × (480 * scale)

Rotation::Degrees270:
  Window: 480×800 → Scaled: (480 * scale) × (800 * scale)
```

## Coordinate System

**Always use logical coordinates** when drawing, regardless of rotation:

```rust
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Rectangle, PrimitiveStyle};
use embedded_graphics::pixelcolor::Gray4;

// Logical display is 800×480
let size = Size::new(800, 480);

// Draw at (10, 10) in LOGICAL coordinates
Rectangle::new(Point::new(10, 10), Size::new(100, 50))
    .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
    .draw(&mut emulator)?;

// Rotation is applied automatically during presentation
emulator.refresh_full().await?;
```

## Implementation Details

### Rotation Transform

The rotation is applied in `window.rs::present()`:

1. **Apply e-ink appearance** - Add paper texture, reduce contrast
2. **Apply rotation transform** - Pixel buffer rotation (90°/180°/270°)
3. **Apply upscaling** - Nearest-neighbor scaling with pixel grid effect
4. **Write to window buffer** - Present to screen

### Rotation Math

```rust
// 90° clockwise
(x, y) → (height - 1 - y, x)

// 180°
(x, y) → (width - 1 - x, height - 1 - y)

// 270° clockwise (90° counter-clockwise)
(x, y) → (y, width - 1 - x)
```

### Performance

- Rotation is O(n) where n = pixel count
- For 800×480 @ 60fps: ~23M pixel copies/sec (negligible on modern CPUs)
- No SIMD optimization yet (can be added if needed)
- Rotation buffer is allocated per-frame (could be optimized with reuse)

## Testing

Run the rotation demo to see all configurations:

```bash
cargo run --example rotation_demo
```

The demo shows:
- Portrait mode (90° rotation) with no upscaling
- Orientation markers (TOP, LEFT, RIGHT, BOTTOM)
- Corner markers to verify correct rotation
- Instructions for other configurations

## Use Cases

### Mobile DAP (portrait orientation)

```rust
let config = EmulatorConfig::PORTRAIT;  // 90° rotation, 1x scale
let emulator = Emulator::with_config(config);
```

### Desktop Development (landscape, high visibility)

```rust
let config = EmulatorConfig::DEFAULT;  // No rotation, 2x scale
let emulator = Emulator::with_spec(&SPEC);
```

### Kiosk/Dashboard (specific rotation)

```rust
let config = EmulatorConfig {
    rotation: Rotation::Degrees180,  // Mounted upside-down
    scale: 1,
};
```

### Testing (native resolution)

```rust
let config = EmulatorConfig::NATIVE;  // No rotation, 1x scale
let emulator = Emulator::with_config(config);
```

## Migration

Existing code continues to work without changes:

```rust
// Old API (still works, uses DEFAULT config)
let emulator = Emulator::new(250, 122);
let emulator = Emulator::with_spec(&SPEC);

// New API (explicit configuration)
let emulator = Emulator::with_config(config);
let emulator = Emulator::with_spec_and_config(&SPEC, config);
```

## API Reference

### Constructors

- `Emulator::new(width, height)` - Default config (2x scale, no rotation)
- `Emulator::with_spec(spec)` - Default config with custom spec
- `Emulator::with_config(config)` - Custom config, default spec
- `Emulator::with_spec_and_config(spec, config)` - Full customization

### Configuration Methods

- `EmulatorConfig::default()` - 2x scale, no rotation
- `Rotation::swaps_dimensions()` - Check if width/height swap
- `Rotation::apply_to_dimensions(w, h)` - Calculate window size

## Future Enhancements

Potential future features:

- **Runtime rotation change** - Allow changing rotation after creation
- **SIMD rotation** - Optimize rotation with vector instructions
- **Buffer reuse** - Avoid per-frame allocation
- **Non-integer scaling** - 1.5x, 2.5x scaling factors
- **Rotation interpolation** - Smoother rotation with bilinear filtering
