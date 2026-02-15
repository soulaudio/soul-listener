# Debug System

Comprehensive debug tooling for e-ink emulator development.

## Features

- **Visual Debug Overlays**: Colored borders showing component boundaries
- **Interactive Inspector**: Click components to see details (layout, props, stats)
- **Power Monitoring**: Real-time graph of power consumption
- **Hotkey Controls**: Quick access to debug features

## Enabling Debug Mode

```bash
cargo run --example debug_demo --features debug
```

## Hotkeys

| Key    | Action |
|--------|--------|
| Ctrl+1 | Toggle debug panel |
| Ctrl+2 | Toggle layout borders |
| Ctrl+3 | Toggle inspector mode |
| Ctrl+4 | Toggle power graph |

## Border Colors

- **Blue** - Containers (VStack, HStack)
- **Green** - Buttons
- **Red** - Labels/Text
- **Purple** - Progress bars
- **Yellow** - Other components

## Inspector Tabs

- **Layout**: Position, size, constraints, padding
- **Component**: Type, properties, state
- **Stats**: Refresh count, last refresh time

## Architecture

The debug system is modular:

```
debug/
├── mod.rs       - Module exports
├── state.rs     - DebugState
├── manager.rs   - DebugManager (coordinator)
├── overlay.rs   - Border rendering
├── panel.rs     - Side panel UI
├── inspector.rs - Component inspector
└── power_graph.rs - Power graph
```

## Adding Debug Info to Components

```rust
#[cfg(feature = "debug")]
impl Button {
    fn debug_info(&self) -> DebugInfo {
        DebugInfo {
            component_type: "Button",
            color: DebugColor::Button,
            custom_data: Some(self.label.to_string()),
        }
    }
}
```

## Performance

- Debug overlay adds <5ms per frame when enabled
- Zero overhead when compiled without `debug` feature
- Ring buffer uses ~2.4KB for power history
