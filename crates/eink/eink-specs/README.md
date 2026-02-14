# eink-specs

E-ink display specifications for emulation and hardware abstraction.

## Features

- **no_std compatible** - Works on embedded systems with no heap allocation
- **Display templates** - Pre-configured specs for Waveshare and Good Display panels
- **Optional serde** - Serialization/deserialization for TOML/JSON configs
- **Temperature compensation** - Adjust refresh timing based on ambient temperature
- **Grayscale tracking** - Track capabilities of different panel types (Pearl, Carta 1000/1200/1300)

## Installation

Add to `Cargo.toml`:

```toml
[dependencies]
eink-specs = { path = "../eink-specs" }

# Optional: Enable serde support
eink-specs = { path = "../eink-specs", features = ["serde"] }
```

## Quick Start

### Using Pre-configured Displays

```rust
use eink_specs::displays::WAVESHARE_2_13_V4;

let spec = WAVESHARE_2_13_V4;
println!("Display: {}", spec.name);
println!("Resolution: {}×{}", spec.width, spec.height);
println!("Diagonal: {:.2}\"", spec.diagonal_inches());
println!("Full refresh: {}ms", spec.full_refresh_ms);
```

### Temperature Compensation

```rust
use eink_specs::displays::WAVESHARE_2_13_V4;

let spec = WAVESHARE_2_13_V4;

// Normal temperature
let normal = spec.adjusted_refresh_ms(spec.full_refresh_ms, 25);
println!("At 25°C: {}ms", normal);  // 2000ms

// Cold temperature (50% slower)
let cold = spec.adjusted_refresh_ms(spec.full_refresh_ms, -5);
println!("At -5°C: {}ms", cold);    // 3000ms

// Hot temperature (20% slower)
let hot = spec.adjusted_refresh_ms(spec.full_refresh_ms, 45);
println!("At 45°C: {}ms", hot);     // 2400ms
```

### Custom Display Spec

```rust
use eink_specs::{DisplaySpec, Controller, PanelType};

const MY_DISPLAY: DisplaySpec = DisplaySpec {
    name: "Custom Display",
    width: 320,
    height: 240,
    controller: Controller::SSD1680,
    panel_type: PanelType::Carta1300,
    grayscale_levels: 4,
    full_refresh_ms: 2000,
    partial_refresh_ms: 300,
    fast_refresh_ms: 260,
    ghosting_rate_partial: 0.15,
    ghosting_rate_fast: 0.25,
    flash_count_full: 3,
    temp_optimal_min: 15,
    temp_optimal_max: 35,
    temp_operating_min: 0,
    temp_operating_max: 50,
};
```

## Available Display Templates

### Waveshare Displays

| Constant | Resolution | Controller | Panel Type | Diagonal |
|----------|-----------|-----------|------------|----------|
| `WAVESHARE_2_13_V4` | 250×122 | SSD1680 | Carta 1000 | 2.13" |
| `WAVESHARE_2_9_V2` | 296×128 | IL0373 | Carta 1000 | 2.9" |
| `WAVESHARE_4_2_V2` | 400×300 | SSD1619 | Carta 1200 | 4.2" |
| `WAVESHARE_7_5_V2` | 800×480 | ED075TC1 | Carta 1200 | 7.5" |

### Good Display Panels

| Constant | Resolution | Controller | Panel Type | Diagonal |
|----------|-----------|-----------|------------|----------|
| `GDEW0213I5F` | 212×104 | UC8151 | Pearl | 2.13" |
| `GDEW029T5` | 296×128 | GDEW | Carta 1000 | 2.9" |
| `GDEW042T2` | 400×300 | SSD1619 | Carta 1200 | 4.2" |
| `GDEW075T7` | 800×480 | GDEW | Carta 1200 | 7.5" |

## Supported Controllers

- `SSD1680` - Solomon Systech (Waveshare 2.13" V4)
- `IL0373` - ImagEInk (Waveshare 2.9" V2)
- `UC8151` - UltraChip (Good Display panels)
- `SSD1619` - Solomon Systech (4.2" displays)
- `ED075TC1` - E Ink (7.5" displays)
- `GDEW` - Good Display series
- `Generic` - Unknown/custom controllers

## Panel Types

- `Pearl` - Older generation e-ink (higher ghosting)
- `Carta1000` - Improved contrast
- `Carta1200` - Faster response time
- `Carta1300` - Latest black & white technology
- `Kaleido3` - Color with filter layer
- `Spectra6` - Multi-pigment color

## Refresh Modes

### Full Refresh
- Duration: ~2000ms (varies by display size)
- Grayscale: 16 levels
- Flashes: 3-4 times
- Ghosting: Completely cleared
- Use for: Page transitions, periodic cleaning

### Partial Refresh
- Duration: ~300ms
- Grayscale: 4 levels
- Flashes: None
- Ghosting: 15% accumulation per refresh
- Use for: Progress bars, live updates

### Fast Refresh
- Duration: ~260ms
- Grayscale: 2 levels (black/white only)
- Flashes: Single flash
- Ghosting: 25% accumulation per refresh
- Use for: Page turns, scrolling

## Examples

Run the display info example:

```bash
cargo run --example display_info
```

This will print detailed information about all pre-configured displays.

## Features

### Default
No features enabled by default - pure `no_std` with no dependencies.

### `serde`
Enable serialization/deserialization support:

```toml
[dependencies]
eink-specs = { path = "../eink-specs", features = ["serde"] }
```

### `std`
Enable standard library support (enables `serde/std` if serde is also enabled):

```toml
[dependencies]
eink-specs = { path = "../eink-specs", features = ["std", "serde"] }
```

## Architecture

```
DisplaySpec
├─ Physical properties (width, height, controller, panel)
├─ Refresh timing (full, partial, fast)
├─ Ghosting rates (partial, fast)
└─ Temperature characteristics (operating, optimal)

Helper Methods
├─ aspect_ratio() - Compute width/height ratio
├─ diagonal_inches() - Estimate screen size
├─ full_refresh_duration() - Get as Duration
├─ adjusted_refresh_ms() - Temperature compensation
├─ is_optimal_temp() - Check temperature range
└─ is_operating_temp() - Check operational range
```

## Usage in Emulator

The emulator uses these specs to configure realistic behavior:

```rust
use eink_specs::displays::WAVESHARE_2_13_V4;
use eink_emulator::Emulator;

let mut emulator = Emulator::with_spec(WAVESHARE_2_13_V4);

// Emulator will automatically:
// - Set resolution to 250×122
// - Simulate 2000ms full refresh with 3 flashes
// - Simulate 300ms partial refresh
// - Accumulate ghosting at 15% per partial refresh
// - Adjust timing based on temperature
```

## Testing

Run tests:

```bash
cargo test
```

All tests pass for both Waveshare and Good Display templates.

## License

MIT OR Apache-2.0
