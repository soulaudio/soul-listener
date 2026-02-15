# Debug System Design

**Date:** 2026-02-15
**Status:** Approved
**Author:** Claude (with user collaboration)

## Overview

Add comprehensive debug tooling to the e-ink emulator to support layout development, performance analysis, and component inspection. The debug system provides visual overlays, interactive inspection, and power consumption monitoring.

## Requirements

### Primary Use Cases
1. **Layout Development** - Visualize component boundaries and hierarchy while building UI
2. **Performance Analysis** - Monitor power consumption and refresh statistics
3. **Testing & Debugging** - Inspect component properties, verify layouts, check styling

### User Experience
- Side panel (200px) with toggle controls and live stats
- Hotkey activation (F12/Ctrl+D to show/hide)
- Adaptive detail: simple borders by default, detailed info on hover/inspect
- Historical power consumption graph (5-minute window)
- Complete inspector with Layout/Component/Stats tabs

## Architecture

### Module Structure

```
eink-emulator/src/
├── debug/
│   ├── mod.rs              # DebugManager - central coordinator
│   ├── state.rs            # DebugState - feature toggles & data
│   ├── overlay.rs          # OverlayRenderer - borders/hitboxes
│   ├── panel.rs            # DebugPanel - side panel UI
│   ├── inspector.rs        # Inspector - component details
│   └── power_graph.rs      # PowerGraph - consumption tracking
└── window.rs               # Integration point
```

### Debug Manager

Central coordinator that owns all debug state and orchestrates rendering.

**Responsibilities:**
- Handle hotkey toggles (F12, F1-F3)
- Coordinate render order: main UI → debug overlay → debug panel
- Track mouse position for hover effects
- Manage inspector selection
- Collect power consumption samples

**Integration Points:**
```rust
// In emulator window event loop
debug_manager.handle_event(event);  // Process input first
app.render(framebuffer);            // Render main UI
debug_manager.render_overlay(framebuffer, layout_tree);  // Add debug overlays
debug_manager.render_panel(framebuffer);  // Draw side panel
```

### Debug State

```rust
struct DebugState {
    panel_visible: bool,
    borders_enabled: bool,
    inspector_mode: bool,
    power_graph_enabled: bool,
    hovered_component: Option<ComponentInfo>,
    selected_component: Option<ComponentInfo>,
    power_history: RingBuffer<PowerSample, 300>, // 5 min at 1 sample/sec
}
```

### Component Metadata

Debug metadata flows from components through the layout system to the emulator.

**In eink-components:**
```rust
#[cfg(feature = "debug")]
pub struct DebugInfo {
    pub component_type: &'static str,  // "Button", "Label", etc.
    pub color: DebugColor,              // Border color
    pub custom_data: Option<&'static str>,
}

#[cfg(feature = "debug")]
pub enum DebugColor {
    Container,   // Blue (#0080FF)
    Button,      // Green (#00FF80)
    Label,       // Red (#FF4040)
    ProgressBar, // Purple (#C040FF)
    Other,       // Yellow (#FFCC00)
}
```

**In eink-system:**
```rust
// Extend LayoutResult
pub struct LayoutResult {
    pub position: Point,
    pub size: Size,
    pub children: Vec<ChildLayout>,

    #[cfg(feature = "debug")]
    pub debug_info: Option<DebugInfo>,
}
```

## Features

### 1. Debug Overlay (Borders/Hitboxes)

**Rendering Modes:**
- **Simple Mode**: No overlay (borders disabled)
- **Borders Mode**: 1px colored outlines around all components
- **Hover Mode**: 2px border on hovered component + dimension label
- **Selected Mode**: Persistent highlight + details in panel

**Color Coding:**
- Containers (VStack, HStack): Blue (#0080FF)
- Buttons: Green (#00FF80)
- Labels/Text: Red (#FF4040)
- ProgressBar: Purple (#C040FF)
- Other: Yellow (#FFCC00)

**Hit Testing:**
- Walk layout tree depth-first (deepest to shallowest)
- First component containing mouse position wins
- Update `hovered_component` on mouse move

### 2. Debug Panel (Side Panel)

**Layout:**
- Fixed 200px width on right side
- Semi-transparent background: RGBA(40, 40, 40, 220)
- Sections: Toggle Controls, Inspector Details, Power Graph

**Toggle Controls:**
```
┌─ Debug Controls ──────┐
│ [✓] Layout Borders    │
│ [✓] Inspector Mode    │
│ [✓] Power Graph       │
│ [ ] Ghosting Visual   │
└───────────────────────┘
```

**Interaction:**
- Click checkboxes to toggle features
- Active toggle highlighted
- Hotkeys work even when panel hidden

### 3. Inspector System

**Activation:**
- Enable "Inspector Mode" toggle
- Visual cursor change or hover indicator
- Click component to inspect

**Details Display (Tabbed):**
```
┌─ Inspector ────────────┐
│ Button @ 0x1a4f        │
│ [Layout][Component][Stats]
│                        │
│ Layout Tab:            │
│  Position: (120, 80)   │
│  Size: 100×40          │
│  Constraints: min 80×32│
│  Padding: 8            │
│                        │
│ Component Tab:         │
│  Type: Button          │
│  Label: "Play"         │
│  State: Enabled        │
│                        │
│ Stats Tab:             │
│  Refreshes: 3          │
│  Last: 0.5s ago        │
│  Type: Partial         │
└────────────────────────┘
```

**Data Sources:**
- Layout info: From `LayoutResult`
- Component info: From `DebugInfo` metadata
- Stats: Emulator tracks refreshes per component region

### 4. Power Consumption Graph

**Graph Specifications:**
- Dimensions: 190×100px
- Time window: 5 minutes (300 samples at 1/sec)
- Y-axis: Power in mW, auto-scaled
- X-axis: Time (0-5 min)

**Data Collection:**
```rust
struct PowerSample {
    timestamp: Instant,
    power_mw: f32,
    refresh_type: Option<RefreshType>,
}
```

**Power Estimation:**
- Idle: 10mW baseline
- Partial refresh: +50mW spike for 0.2s
- Full refresh: +200mW spike for 1.0s
- Active UI updates: +5mW per component change

**Visual Elements:**
- Line graph of power over time
- Vertical markers for refresh events (color-coded: partial/full)
- Current power value displayed
- Average power over window
- Estimated battery life

## Input Handling

### Event Flow

```
Window Event → DebugManager.handle_event() →
  - Hotkey? Update state, consume event
  - Panel click? Update toggles, consume event
  - Inspector click? Select component, consume event
  - Else? Pass through to main app
```

### Hotkeys

- **F12** or **Ctrl+D**: Toggle debug panel visibility
- **F1**: Toggle layout borders
- **F2**: Toggle inspector mode
- **F3**: Toggle power graph
- **Esc**: Clear inspector selection

### Priority

Debug system gets first chance at events. If handled, event is consumed and not passed to main app. This prevents debug interactions from affecting UI state.

## Implementation Strategy

### Phase 1: Foundation
1. Create debug module structure
2. Implement DebugManager and DebugState
3. Add feature flag `debug` to workspace
4. Integrate with emulator window event loop

### Phase 2: Component Metadata
1. Add DebugInfo struct to eink-components
2. Extend LayoutResult with debug_info field
3. Implement debug_info() for all components (Button, Label, etc.)
4. Thread metadata through layout computation

### Phase 3: Debug Overlay
1. Implement OverlayRenderer
2. Add border rendering with color coding
3. Implement hit testing for hover detection
4. Add dimension labels on hover

### Phase 4: Debug Panel
1. Implement DebugPanel UI rendering
2. Add toggle controls with click detection
3. Implement panel show/hide with hotkey
4. Add semi-transparent background

### Phase 5: Inspector
1. Implement Inspector component selection
2. Add tabbed detail view (Layout/Component/Stats)
3. Integrate with panel rendering
4. Track component refresh statistics

### Phase 6: Power Graph
1. Implement PowerGraph with ring buffer
2. Add power estimation logic
3. Render line graph with markers
4. Display current/average power stats

### Phase 7: Polish & Testing
1. Add all hotkey handlers
2. Comprehensive testing (unit + integration)
3. Performance validation (<5ms overhead)
4. Documentation and examples

## Testing Strategy

### Unit Tests
- `DebugState`: Toggle logic, state transitions
- `PowerGraph`: Sample collection, ring buffer, calculations
- `OverlayRenderer`: Color selection, hit testing
- `Inspector`: Component selection, tab switching

### Integration Tests
- Create test UI with known layout
- Verify borders at correct positions
- Test inspector returns correct data
- Validate power graph sample accumulation

### Manual Testing
```bash
cargo run --example display_emulator --features debug

# Test scenarios:
# 1. Toggle panel with F12
# 2. Enable borders, verify colors
# 3. Enable inspector, click components
# 4. Trigger refreshes, watch power graph
# 5. Verify all hotkeys
# 6. Check panel toggle clicks
```

### Performance Testing
- Debug overlay adds <5ms to frame time
- Memory usage acceptable (ring buffer ~2.4KB)
- Zero overhead when debug disabled

## Success Criteria

1. **Layout Debugging**: Developers can visually see component boundaries and hierarchy
2. **Inspector**: Click any component to see full details (layout, properties, stats)
3. **Power Monitoring**: Real-time graph shows consumption patterns and refresh events
4. **Usability**: Hotkeys and panel toggles work intuitively
5. **Performance**: Negligible impact (<5ms) when enabled, zero when disabled
6. **Quality**: All components render correct debug metadata with proper colors

## Future Enhancements (Out of Scope)

- Export debug traces/logs
- Record and replay interaction sessions
- Performance profiler with flame graphs
- Component hierarchy tree view
- Diff view for layout changes
- Remote debugging for firmware on hardware
- Screenshot comparison tool

## References

- Browser DevTools (Chrome, Firefox) - Inspiration for inspector
- React DevTools - Component tree inspection
- Xcode View Debugger - 3D layer visualization (simplified to 2D here)
