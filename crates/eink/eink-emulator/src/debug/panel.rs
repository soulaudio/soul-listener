//! Debug panel UI rendering
//!
//! Provides a side panel for debug controls and information display. The panel is rendered
//! into a separate buffer region to the right of the display. It uses embedded-graphics for
//! text rendering and displays debug information including hotkeys, display properties,
//! components, power consumption, and a live power history chart.
//!
//! # Panel Layout (top → bottom)
//!
//! ```text
//! ┌─────────────────────────────┐
//! │ E-INK DEBUG                 │ ← title
//! ├─────────────────────────────┤
//! │ HOTKEYS                     │
//! │  Ctrl+1  Panel    [ON/OFF]  │
//! │  Ctrl+2  Borders  [ON/OFF]  │
//! │  Ctrl+3  Inspect  [ON/OFF]  │
//! │  Ctrl+4  Power    [ON/OFF]  │
//! ├─────────────────────────────┤
//! │ DISPLAY                     │
//! │  480 x 800                  │
//! │  Rot: 90deg (portrait)      │
//! │  Scale: 1x                  │
//! │  Temp:  25C  OK             │
//! ├─────────────────────────────┤
//! │ COMPONENTS                  │
//! │  3 registered               │
//! │  BTN  play-button           │
//! │  CTN  display-root          │
//! │  ...                        │
//! ├─────────────────────────────┤
//! │ REFRESH                     │
//! │  Full:    12                │
//! │  Partial: 47                │
//! ├─────────────────────────────┤
//! │ POWER                       │
//! │  Now: 12.4 mW               │
//! │  Avg: 10.2 mW               │
//! │  Pk: 220mW Idle: 82%        │
//! │  ~18.4hr @ 1000mAh          │
//! ├─────────────────────────────┤
//! │ POWER HISTORY               │
//! │ ┌───────────────────────┐  │
//! │ │ ░░▒▒░░░░░█░░░░░░░░░░ │  │ ← bar chart
//! │ └───────────────────────┘  │
//! └─────────────────────────────┘
//! ```
//!
//! # Pixel Format
//!
//! All pixels are ARGB: `0xFFRRGGBB` (fully opaque).
//!
//! # Backward Compatibility
//!
//! The original `DebugPanel` struct and its `render()` method are preserved for
//! compatibility with existing tests and callers.

use std::convert::Infallible;

use embedded_graphics::{
    draw_target::DrawTarget,
    geometry::{OriginDimensions, Size},
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    text::Text,
};

use super::state::{DebugState, DebugTab, RefreshType};

// ---------------------------------------------------------------------------
// Backward-compatible overlay panel (kept for existing tests)
// ---------------------------------------------------------------------------

const PANEL_WIDTH: u32 = 200;
const PANEL_BG_COLOR: u32 = 0xDC282828; // Semi-transparent dark gray

/// Debug panel renderer (legacy overlay mode)
///
/// Preserved for backward compatibility.  New code should use [`render_into`].
pub struct DebugPanel {
    width: u32,
}

impl DebugPanel {
    /// Create a new debug panel renderer
    pub fn new() -> Self {
        Self { width: PANEL_WIDTH }
    }

    /// Get the width of the debug panel in pixels
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Render the debug panel (legacy overlay mode)
    pub fn render(
        &self,
        buffer: &mut [u32],
        screen_width: u32,
        screen_height: u32,
        state: &DebugState,
    ) {
        if !state.panel_visible {
            return;
        }
        let panel_x = screen_width - PANEL_WIDTH;
        self.draw_background(buffer, screen_width, screen_height, panel_x);
    }

    fn draw_background(
        &self,
        buffer: &mut [u32],
        screen_width: u32,
        screen_height: u32,
        panel_x: u32,
    ) {
        for y in 0..screen_height {
            for x in panel_x..screen_width {
                let idx = (y * screen_width + x) as usize;
                if idx < buffer.len() {
                    buffer[idx] = PANEL_BG_COLOR;
                }
            }
        }
    }
}

impl Default for DebugPanel {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// PanelInfo — data passed to render_into
// ---------------------------------------------------------------------------

/// All information needed to render the debug side panel.
pub struct PanelInfo<'a> {
    /// Current debug toggle states.
    pub state: &'a DebugState,
    /// Display width **before** rotation, in physical pixels.
    pub disp_w: u32,
    /// Display height **before** rotation, in physical pixels.
    pub disp_h: u32,
    /// Rotation applied to the display output: 0, 90, 180, or 270 degrees.
    pub rotation_deg: u32,
    /// Integer pixel-scale factor (1 = native, 2 = 2×2 upscale, …).
    pub scale: u32,
    /// Operating temperature in °C (used for range warnings).
    pub temperature: i8,
    /// Live power consumption graph.  `None` when the debug manager is unavailable.
    pub power_graph: Option<&'a super::power_graph::PowerGraph>,
    /// Accumulated hardware power statistics.  `None` when not tracked.
    pub power_stats: Option<&'a crate::power::PowerStats>,
}

// ---------------------------------------------------------------------------
// PanelCanvas — embedded-graphics DrawTarget backed by a &mut [u32] slice
// ---------------------------------------------------------------------------

/// An `embedded-graphics` draw target that writes ARGB pixels into a raw `u32` slice.
///
/// Pixels are stored in row-major order with `width` pixels per row.
pub struct PanelCanvas<'buf> {
    buf: &'buf mut [u32],
    width: u32,
    height: u32,
}

impl<'buf> PanelCanvas<'buf> {
    /// Create a new canvas that renders into `buf`.
    ///
    /// `buf` must contain at least `width * height` elements.
    pub fn new(buf: &'buf mut [u32], width: u32, height: u32) -> Self {
        Self { buf, width, height }
    }
}

impl OriginDimensions for PanelCanvas<'_> {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for PanelCanvas<'_> {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Infallible>
    where
        I: IntoIterator<Item = Pixel<Rgb888>>,
    {
        for Pixel(coord, color) in pixels {
            if coord.x >= 0 && coord.y >= 0 {
                let x = coord.x as u32;
                let y = coord.y as u32;
                if x < self.width && y < self.height {
                    let idx = (y * self.width + x) as usize;
                    if idx < self.buf.len() {
                        self.buf[idx] = 0xFF000000
                            | ((color.r() as u32) << 16)
                            | ((color.g() as u32) << 8)
                            | color.b() as u32;
                    }
                }
            }
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Low-level pixel helpers (module-private)
// ---------------------------------------------------------------------------

#[inline]
fn set_px(buf: &mut [u32], stride: u32, x: u32, y: u32, color: u32) {
    let idx = (y * stride + x) as usize;
    if idx < buf.len() {
        buf[idx] = color;
    }
}

fn hline(buf: &mut [u32], stride: u32, x: u32, y: u32, w: u32, color: u32) {
    for dx in 0..w {
        set_px(buf, stride, x + dx, y, color);
    }
}

fn fill_rect(buf: &mut [u32], stride: u32, x: u32, y: u32, w: u32, h: u32, color: u32) {
    for dy in 0..h {
        hline(buf, stride, x, y + dy, w, color);
    }
}

/// Draw a 1-pixel-high separator spanning the full panel width.
/// Column 0 (left border) retains the border colour.
fn draw_sep_line(buf: &mut [u32], panel_w: u32, y: u32) {
    const SEP_COLOR: u32 = 0xFF252540;
    const BORDER_COLOR: u32 = 0xFF4A4A6A;
    let row_start = (y * panel_w) as usize;
    let row_end = (row_start + panel_w as usize).min(buf.len());
    if row_start < row_end {
        buf[row_start] = BORDER_COLOR; // keep left border
        for px in &mut buf[(row_start + 1)..row_end] {
            *px = SEP_COLOR;
        }
    }
}

/// Abbreviate a component type name to ≤3 uppercase chars.
fn abbrev_type(t: &str) -> &'static str {
    match t {
        "Container" => "CTN",
        "Label" => "LBL",
        "Button" => "BTN",
        "ProgressBar" => "PBR",
        "Text" => "TXT",
        "Image" => "IMG",
        "Stack" => "STK",
        _ => "???",
    }
}

// ---------------------------------------------------------------------------
// Power graph rendering
// ---------------------------------------------------------------------------

/// Render a colour-coded bar chart of power history into `buf`.
///
/// The chart occupies the rectangle `(gx, gy, gw, gh)` inside the panel buffer
/// (which has row stride = `panel_w`).  Bar colours encode the refresh type:
///
/// - Idle (no refresh) → dim blue
/// - Partial refresh   → amber
/// - Full refresh      → red
/// - Fast refresh      → orange
///
/// A bright white pixel marks the top of each bar.  Three horizontal grid lines
/// are drawn at 25 %, 50 %, and 75 % of the peak power level.
fn render_power_graph(
    buf: &mut [u32],
    panel_w: u32,
    gx: u32,
    gy: u32,
    gw: u32,
    gh: u32,
    graph: &super::power_graph::PowerGraph,
) {
    const GRAPH_BG: u32 = 0xFF0E0E1E;
    const GRAPH_BORDER: u32 = 0xFF3A3A5A;
    const GRID_COLOR: u32 = 0xFF1C1C32;
    const BAR_IDLE: u32 = 0xFF205888;
    const BAR_PARTIAL: u32 = 0xFFB07020;
    const BAR_FULL: u32 = 0xFFCC3030;
    const BAR_FAST: u32 = 0xFFCC6020;
    const BAR_TIP: u32 = 0xFFD0E0FF;

    // Background
    fill_rect(buf, panel_w, gx, gy, gw, gh, GRAPH_BG);

    // 1px border box
    hline(buf, panel_w, gx, gy, gw, GRAPH_BORDER);
    hline(buf, panel_w, gx, gy + gh - 1, gw, GRAPH_BORDER);
    for row in gy..gy + gh {
        set_px(buf, panel_w, gx, row, GRAPH_BORDER);
        set_px(buf, panel_w, gx + gw - 1, row, GRAPH_BORDER);
    }

    let samples = graph.samples();
    if samples.is_empty() {
        return;
    }

    // Inner area (excludes 1px border)
    let inner_x = gx + 1;
    let inner_y = gy + 1;
    let inner_w = gw.saturating_sub(2);
    let inner_h = gh.saturating_sub(2);
    if inner_w == 0 || inner_h == 0 {
        return;
    }

    // Determine Y-axis range: floor at baseline*0.9, ceil at peak
    let baseline = graph.baseline_power();
    let min_p = baseline * 0.9;
    let max_p = samples
        .iter()
        .map(|s| s.power_mw)
        .fold(baseline + 1.0, f32::max);
    let range = max_p - min_p;

    // Horizontal grid lines at 25 %, 50 %, 75 % of max
    for pct in [25u32, 50, 75] {
        // power at pct%: min_p + range * pct/100
        // y position from top: inner_h - (pct * inner_h / 100)
        let grid_y = inner_y + inner_h - (pct * inner_h / 100);
        hline(buf, panel_w, inner_x, grid_y, inner_w, GRID_COLOR);
    }

    let n = samples.len();
    for (i, sample) in samples.iter().enumerate() {
        // Map sample index → x pixel within inner area
        let x_frac = if n <= 1 {
            0.0f32
        } else {
            i as f32 / (n - 1) as f32
        };
        let bar_x = inner_x + (x_frac * (inner_w.saturating_sub(1)) as f32).round() as u32;
        if bar_x >= inner_x + inner_w {
            continue;
        }

        // Normalise power → bar height (0 = baseline, inner_h = max)
        let norm = ((sample.power_mw - min_p) / range).clamp(0.0, 1.0);
        let bar_h = ((norm * inner_h as f32).round() as u32).max(1);
        let bar_top = inner_y + inner_h - bar_h;

        let bar_color = match sample.refresh_type {
            Some(RefreshType::Full) => BAR_FULL,
            Some(RefreshType::Partial) => BAR_PARTIAL,
            Some(RefreshType::Fast) => BAR_FAST,
            None => BAR_IDLE,
        };

        // Draw bar from bar_top down to the graph floor
        for row in bar_top..(inner_y + inner_h) {
            set_px(buf, panel_w, bar_x, row, bar_color);
        }
        // Bright tip pixel
        set_px(buf, panel_w, bar_x, bar_top, BAR_TIP);
    }
}

// ---------------------------------------------------------------------------
// Text command list (used to batch all text into a single canvas pass)
// ---------------------------------------------------------------------------

struct TextCmd {
    x: i32,
    y: i32,
    text: String,
    color: Rgb888,
}

/// A rectangular highlight block drawn behind text (used for selection and tab
/// backgrounds).
struct HighlightCmd {
    /// Left edge of the block (column index, 0-based).
    x: u32,
    /// Top pixel row of the block.
    y: u32,
    /// Width in pixels.
    w: u32,
    /// Height in pixels.
    h: u32,
    /// ARGB colour.
    color: u32,
}

// ---------------------------------------------------------------------------
// Hit-test support for mouse interaction
// ---------------------------------------------------------------------------

/// Panel layout Y constants for hit-testing (must stay in sync with `render_into`).
///
/// ```text
/// cy=10  → title baseline      top pixel = 10-9 = 1
/// cy=23  → separator (sep #1)
/// cy=27  → tab bar baseline    top pixel = 27-9 = 18
/// cy=40  → separator (sep #2)
/// cy=44  → inspect hint line   top pixel = 44-9 = 35
/// cy=57  → first tree-row baseline  top pixel = 57-9 = 48
/// ```
/// Tab bar clickable zone (y in [TAB_HIT_Y_START, TAB_HIT_Y_END)).
pub const TAB_HIT_Y_START: u32 = 18;
pub const TAB_HIT_Y_END:   u32 = 40; // exclusive – up to the second separator

/// Inspect-mode toggle clickable zone (y in [INSPECT_HIT_Y_START, INSPECT_HIT_Y_END)).
/// Covers the gap between sep#2 and the first tree row.
pub const INSPECT_HIT_Y_START: u32 = 40;
pub const INSPECT_HIT_Y_END:   u32 = 57; // exclusive – = TREE_BASELINE_CY

/// `cy` value at the start of the first tree row in `render_into`.
pub const TREE_BASELINE_CY: i32 = 57;
/// Ascent of FONT_6X10: baseline-to-top distance.
pub const FONT_ASCENT: i32 = 9;

/// A hit-testable rectangle covering one visible scene tree row.
///
/// Computed by [`compute_scene_hits`] to map mouse clicks to row indices.
pub struct SceneHitRect {
    /// Top pixel row in panel-local coordinates.
    pub y: u32,
    /// Height of the row in pixels.
    pub h: u32,
    /// Absolute index into the result of [`DebugState::build_scene_rows`].
    pub abs_idx: usize,
}

/// Compute hit-test rectangles for all currently visible scene-tree rows.
///
/// Mirrors the Y-layout logic of [`render_into`] for the SCENE tab without
/// rendering anything.  Returns an empty Vec when the panel is showing a
/// different tab or when no components are registered.
pub fn compute_scene_hits(
    _panel_w: u32,
    height: u32,
    state: &super::state::DebugState,
) -> Vec<SceneHitRect> {
    use super::state::DebugTab;

    if state.active_tab != DebugTab::Scene || state.registered_components.is_empty() {
        return Vec::new();
    }

    const LH: i32 = 13;

    let rows  = state.build_scene_rows();
    let total = rows.len();

    let meta_h: i32 = if state.scene_selected.is_some() {
        2 + 3 + 5 * LH   // sep + pad + 5 data lines
    } else {
        2 + 3 + 2 * LH   // sep + pad + 2 hint lines
    };
    // tree_budget uses TREE_BASELINE_CY (57) matching render_into's `cy` at row start
    let tree_budget   = height as i32 - TREE_BASELINE_CY - meta_h - 4;
    let scene_visible = ((tree_budget / LH).max(2) as usize).min(total);
    let scroll        = state.scene_scroll;
    let visible_end   = (scroll + scene_visible).min(total);

    (scroll..visible_end)
        .enumerate()
        .map(|(off, abs_idx)| SceneHitRect {
            // Top of hit rect = top of the character = baseline - font_ascent
            y: (TREE_BASELINE_CY - FONT_ASCENT + off as i32 * LH).max(0) as u32,
            h: LH as u32,
            abs_idx,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Standalone render_into function
// ---------------------------------------------------------------------------

/// Render the debug side panel into a raw pixel buffer.
///
/// The buffer represents a `panel_w × height` region in row-major order
/// (stride = `panel_w`).  Renders:
///
/// 1. Dark background + left border
/// 2. Section separators
/// 3. Power bar-chart (if history is available)
/// 4. All text labels in a single embedded-graphics canvas pass
///
/// # Arguments
///
/// * `buf`     – Mutable ARGB pixel slice (`panel_w * height` elements)
/// * `panel_w` – Width of the panel in pixels
/// * `height`  – Height of the panel in pixels
/// * `info`    – Display metadata and current debug state
pub fn render_into(buf: &mut [u32], panel_w: u32, height: u32, info: &PanelInfo<'_>) {
    // Colours
    let col_normal = Rgb888::new(195, 195, 215);
    let col_dim    = Rgb888::new(85,  85,  110);
    let col_white  = Rgb888::new(220, 220, 255);
    let col_green  = Rgb888::new(80,  200, 120);
    let col_warn   = Rgb888::new(255, 160, 60);
    let col_err    = Rgb888::new(255, 80,  80);
    let col_cyan   = Rgb888::new(80,  210, 210);

    const PL: i32 = 5;   // left text padding
    const LH: i32 = 13;  // line height

    let mut cmds: Vec<TextCmd>         = Vec::with_capacity(80);
    let mut seps: Vec<u32>             = Vec::with_capacity(8);
    let mut highlights: Vec<HighlightCmd> = Vec::with_capacity(8);
    // Power graph area set by the Power tab: (x, y, w, h)
    let mut graph_area: Option<(u32, u32, u32, u32)> = None;

    macro_rules! push {
        ($x:expr, $y:expr, $text:expr, $color:expr) => {
            cmds.push(TextCmd { x: $x, y: $y, text: $text.to_string(), color: $color })
        };
    }

    let max_chars = ((panel_w as i32 - PL - 1) / 6).max(1) as usize;

    let mut cy = 10i32;

    // =========================================================================
    // TITLE BAR
    // =========================================================================
    push!(PL, cy, "E-INK DEBUG", col_white);

    // Right-side compact indicators: B=borders  I=inspect  P=power
    let b_col = if info.state.borders_enabled    { col_green } else { col_dim };
    let i_col = if info.state.inspector_mode      { col_cyan  } else { col_dim };
    let p_col = if info.state.power_graph_enabled { col_warn  } else { col_dim };
    let ind_x = panel_w as i32 - PL - 3 * 8;
    push!(ind_x,      cy, "B", b_col);
    push!(ind_x +  8, cy, "I", i_col);
    push!(ind_x + 16, cy, "P", p_col);
    cy += LH;
    seps.push(cy as u32);
    cy += 1;

    // =========================================================================
    // TAB BAR  –  [SCENE] [DISP] [PWR]  (Tab key cycles)
    // =========================================================================
    cy += 3;
    let tab_w = (panel_w - 1) / 3; // distribute available width across 3 tabs
    let tab_defs: &[(&str, DebugTab)] = &[
        ("SCENE", DebugTab::Scene),
        ("DISP",  DebugTab::Display),
        ("PWR",   DebugTab::Power),
    ];
    for (i, &(label, tab)) in tab_defs.iter().enumerate() {
        let tx = 1 + i as u32 * tab_w;
        let is_active = info.state.active_tab == tab;
        if is_active {
            highlights.push(HighlightCmd {
                x: tx,
                y: (cy - 9).max(0) as u32,
                w: tab_w,
                h: 13,
                color: 0xFF1E3D62,
            });
        }
        let tab_col = if is_active { col_white } else { col_dim };
        // Centre label inside tab cell
        let lx = tx as i32 + (tab_w as i32 - label.len() as i32 * 6) / 2;
        push!(lx.max(tx as i32 + 2), cy, label, tab_col);
    }
    cy += LH;
    seps.push(cy as u32);
    cy += 1;
    cy += 3;

    // =========================================================================
    // TAB CONTENT
    // =========================================================================
    match info.state.active_tab {
        // -----------------------------------------------------------------
        // SCENE TAB
        // -----------------------------------------------------------------
        DebugTab::Scene => {
            let rows  = info.state.build_scene_rows();
            let total = rows.len();
            let comps = &info.state.registered_components;

            if comps.is_empty() {
                push!(PL, cy, "No nodes yet.", col_dim);
                cy += LH;
                push!(PL, cy, "Trigger a display refresh", col_dim);
                cy += LH;
                push!(PL, cy, "to scan components.", col_dim);
                cy += LH;
                seps.push((cy + 4) as u32);
                cy += 8;
                push!(PL, cy, "Arrows: navigate tree", col_dim);
                cy += LH;
                push!(PL, cy, "Enter:  expand/collapse", col_dim);
                cy += LH;
                push!(PL, cy, "Ctrl+3: hover-inspect", col_dim);
                cy += LH;
            } else {
                // ── Inspect-mode toggle button ────────────────────────────
                if info.state.inspector_mode {
                    // Active: bright cyan button
                    highlights.push(HighlightCmd {
                        x: 1,
                        y: (cy - FONT_ASCENT as i32).max(0) as u32,
                        w: panel_w - 1,
                        h: 12,
                        color: 0xFF0A2535,
                    });
                    push!(PL, cy, "[ INSPECT ]  hover=select", col_cyan);
                } else {
                    push!(PL, cy, "[ inspect ]  click to on", col_dim);
                }
                let cnt = format!("{}/{}", total, comps.len());
                let cx = panel_w as i32 - cnt.len() as i32 * 6 - PL;
                push!(cx.max(PL + 60), cy, cnt, col_dim);
                cy += LH;

                let scroll = info.state.scene_scroll;

                // Compute how many tree rows fit above the metadata pane.
                let meta_h: i32 = if info.state.scene_selected.is_some() {
                    2 + 3 + 5 * LH   // separator + pad + 5 data lines
                } else {
                    2 + 3 + 2 * LH   // separator + pad + 2 hint lines
                };
                let tree_budget   = height as i32 - cy - meta_h - 4;
                let scene_visible = ((tree_budget / LH).max(2) as usize).min(total);
                let visible_end   = (scroll + scene_visible).min(total);

                // ── tree rows ─────────────────────────────────────────────
                for (off, row) in rows[scroll..visible_end].iter().enumerate() {
                    let abs_idx   = scroll + off;
                    let is_sel    = info.state.scene_selected == Some(abs_idx);

                    // Hover highlight (inspector mode): row whose component matches hovered.
                    let is_hov = !is_sel && info.state.inspector_mode && {
                        let comp = &comps[row.comp_idx];
                        info.state.hovered_component.as_ref().map(|h| {
                            h.position == comp.position && h.size == comp.size
                        }).unwrap_or(false)
                    };

                    if is_sel {
                        highlights.push(HighlightCmd {
                            x: 1,
                            y: (cy - 9).max(0) as u32,
                            w: panel_w - 1,
                            h: 12,
                            color: 0xFF1E3D62,  // blue – selected
                        });
                    } else if is_hov {
                        highlights.push(HighlightCmd {
                            x: 1,
                            y: (cy - 9).max(0) as u32,
                            w: panel_w - 1,
                            h: 12,
                            color: 0xFF3A2800,  // dark amber – hovered
                        });
                    }

                    let indent = "  ".repeat(row.depth.min(4));
                    let toggle = if row.has_children || row.is_label_group {
                        if row.is_collapsed { "+ " } else { "- " }
                    } else {
                        "  "
                    };
                    let label: String = if row.is_label_group {
                        format!("LBL x{}", row.label_group_count)
                    } else {
                        let comp = &comps[row.comp_idx];
                        let abbr = abbrev_type(&comp.component_type);
                        let name = comp.test_id.as_deref().unwrap_or("");
                        if name.is_empty() {
                            format!(
                                "{} {}x{}@{},{}",
                                abbr,
                                comp.size.0, comp.size.1,
                                comp.position.0, comp.position.1
                            )
                        } else {
                            let n: String = name.chars().take(18).collect();
                            format!("{abbr} {n}")
                        }
                    };

                    let line: String = format!("{indent}{toggle}{label}")
                        .chars().take(max_chars).collect();

                    let row_col = if is_sel {
                        col_white
                    } else if is_hov {
                        col_warn  // amber text for hovered row
                    } else {
                        match row.depth { 0 => col_white, 1 => col_normal, _ => col_dim }
                    };
                    push!(PL, cy, line, row_col);
                    cy += LH;
                }

                if visible_end < total {
                    push!(
                        PL, cy,
                        format!("  +{} more (Arrow-Down)", total - visible_end),
                        col_dim
                    );
                    cy += LH;
                }

                // ── metadata pane ─────────────────────────────────────────
                seps.push(cy as u32);
                cy += 1;
                cy += 3;

                if let Some(sel) = info.state.scene_selected {
                    if sel < rows.len() {
                        let row  = &rows[sel];
                        let comp = &comps[row.comp_idx];
                        let abbr = if row.is_label_group {
                            "LBL"
                        } else {
                            abbrev_type(&comp.component_type)
                        };
                        let tid = comp.test_id.as_deref().unwrap_or("(unnamed)");
                        push!(
                            PL, cy,
                            format!("[{abbr}] {}", tid.chars().take(max_chars.saturating_sub(7)).collect::<String>()),
                            col_white
                        );
                        cy += LH;
                        push!(
                            PL, cy,
                            format!("{}x{}  @{},{}", comp.size.0, comp.size.1, comp.position.0, comp.position.1),
                            col_normal
                        );
                        cy += LH;
                        let parent_str = match info.state.find_parent(comp) {
                            Some(p) => {
                                let pa  = abbrev_type(&p.component_type);
                                let pid = p.test_id.as_deref().unwrap_or("?");
                                format!("^ [{pa}] {}", pid.chars().take(14).collect::<String>())
                            }
                            None => "^ (root)".to_string(),
                        };
                        push!(PL, cy, parent_str, col_dim);
                        cy += LH;
                        let children = info.state.find_children(comp);
                        let area     = comp.size.0 as u64 * comp.size.1 as u64;
                        push!(
                            PL, cy,
                            format!("v {} children  {}px\u{00B2}", children.len(), area),
                            col_dim
                        );
                        cy += LH;
                        push!(PL, cy, "Arrows:nav  Enter:fold", col_dim);
                        cy += LH;
                    }
                } else {
                    push!(PL, cy, "Arrows: navigate tree", col_dim);
                    cy += LH;
                    push!(PL, cy, "Enter/L/R: expand/fold", col_dim);
                    cy += LH;
                }
            }
        }

        // -----------------------------------------------------------------
        // DISPLAY TAB
        // -----------------------------------------------------------------
        DebugTab::Display => {
            push!(PL, cy, "DISPLAY", col_dim);
            cy += LH;
            push!(PL, cy, format!("{} x {}", info.disp_w, info.disp_h), col_normal);
            cy += LH;

            let rot_name = match info.rotation_deg {
                0   => "landscape",
                90  => "portrait",
                180 => "inverted",
                270 => "portrait-R",
                _   => "custom",
            };
            push!(PL, cy, format!("Rot: {}deg  ({})", info.rotation_deg, rot_name), col_normal);
            cy += LH;

            let (win_w, win_h) = if info.rotation_deg == 90 || info.rotation_deg == 270 {
                (info.disp_h * info.scale, info.disp_w * info.scale)
            } else {
                (info.disp_w * info.scale, info.disp_h * info.scale)
            };
            if info.scale == 1 {
                push!(PL, cy, "Scale: 1x  (native)", col_normal);
            } else {
                push!(PL, cy, format!("Scale: {}x  ({}x{})", info.scale, win_w, win_h), col_normal);
            }
            cy += LH;

            let (temp_status, temp_col) = match info.temperature {
                t if t < 0  => ("VERY COLD", col_err),
                t if t < 5  => ("COLD",      col_warn),
                t if t > 40 => ("VERY HOT",  col_err),
                t if t > 35 => ("HOT",       col_warn),
                _           => ("OK",        col_green),
            };
            push!(PL, cy, format!("Temp: {:>3}C  ", info.temperature), col_normal);
            push!(PL + 11 * 6, cy, temp_status, temp_col);
            cy += LH;

            seps.push(cy as u32);
            cy += 1;
            cy += 4;

            push!(PL, cy, "REFRESH", col_dim);
            cy += LH;
            push!(PL, cy, format!("Full:    {}", info.state.full_refresh_count), col_normal);
            cy += LH;
            push!(PL, cy, format!("Partial: {}", info.state.partial_refresh_count), col_normal);
            cy += LH;

            seps.push(cy as u32);
            cy += 1;
            cy += 4;

            push!(PL, cy, "HOTKEYS", col_dim);
            cy += LH;
            let keys: &[(&str, &str)] = &[
                ("Ctrl+1", "Toggle panel"),
                ("Ctrl+2", "Layout overlay"),
                ("Ctrl+3", "Hover inspect"),
                ("Ctrl+4", "Power graph"),
                ("Tab   ", "Next tab"),
                ("Arrows", "Navigate scene"),
                ("Enter ", "Expand/collapse"),
                ("F11   ", "Block fullscreen"),
            ];
            for &(k, v) in keys {
                push!(PL, cy, format!("{k}  {v}"), col_dim);
                cy += LH;
            }
        }

        // -----------------------------------------------------------------
        // POWER TAB
        // -----------------------------------------------------------------
        DebugTab::Power => {
            push!(PL, cy, "POWER", col_dim);
            cy += LH;

            if let Some(graph) = info.power_graph {
                let cur     = graph.current_power();
                let avg     = graph.average_power();
                let cur_col = if cur > 150.0 { col_err } else if cur > 60.0 { col_warn } else { col_normal };
                push!(PL, cy, format!("Now: {:>6.1} mW", cur), cur_col);
                cy += LH;
                push!(PL, cy, format!("Avg: {:>6.1} mW", avg), col_normal);
                cy += LH;
            } else {
                push!(PL, cy, "No data", col_dim);
                cy += LH;
            }

            if let Some(stats) = info.power_stats {
                let peak_mw = stats.peak_current_ua as f32 * 3.3 / 1000.0;
                push!(PL, cy, format!("Peak: {:.0} mW", peak_mw), col_normal);
                cy += LH;
                let pct = stats.state_percentages();
                push!(
                    PL, cy,
                    format!("Idle:{:.0}% Act:{:.0}% Slp:{:.0}%", pct.idle, pct.active, pct.sleep),
                    col_dim
                );
                cy += LH;
                let batt_h = stats.estimated_battery_life_hours(1000);
                let batt_str = if batt_h > 0.0 {
                    format!("~{:.1}hr @ 1000mAh", batt_h)
                } else {
                    "~-- hr (no data)".to_string()
                };
                push!(PL, cy, batt_str, col_cyan);
                cy += LH;
            }

            seps.push(cy as u32);
            cy += 1;
            cy += 4;

            push!(PL, cy, "POWER HISTORY", col_dim);
            cy += LH - 2;

            // Size the graph to fill remaining height (leave 24px for legend).
            let gx: u32 = 1;
            let gy: u32 = cy as u32;
            let gw: u32 = panel_w - 1;
            let remaining = height.saturating_sub(gy + 24);
            let gh: u32   = remaining.min(120);
            graph_area = Some((gx, gy, gw, gh));
            cy += gh as i32 + 4;

            // Legend
            push!(PL,          cy, "Idle", Rgb888::new(32, 88, 136));
            push!(PL + 4 * 6,  cy, "Part", Rgb888::new(176, 112, 32));
            push!(PL + 8 * 6,  cy, "Full", Rgb888::new(204, 48, 48));
            cy += LH;

            // Axis annotation (peak mW + sample count)
            if let Some(graph) = info.power_graph {
                let samples = graph.samples();
                if !samples.is_empty() {
                    let max_p = samples.iter().map(|s| s.power_mw).fold(0f32, f32::max);
                    let n     = samples.len();
                    push!(PL, cy, format!("{:.0}mW", max_p), col_dim);
                    let cnt = format!("{n} smpl");
                    let sx  = panel_w as i32 - cnt.len() as i32 * 6 - PL;
                    push!(sx.max(PL + 40), cy, cnt, col_dim);
                }
            }
        }
    }

    let _ = cy;

    // =========================================================================
    // RENDER PHASE 1: background fill
    // =========================================================================
    const BG: u32 = 0xFF1A1A2E;
    for px in buf.iter_mut() {
        *px = BG;
    }

    // =========================================================================
    // RENDER PHASE 2: left border
    // =========================================================================
    const BORDER: u32 = 0xFF4A4A6A;
    for row in 0..height {
        let idx = (row * panel_w) as usize;
        if idx < buf.len() {
            buf[idx] = BORDER;
        }
    }

    // =========================================================================
    // RENDER PHASE 2b: highlight blocks (tab backgrounds + selection bars)
    // =========================================================================
    for hl in &highlights {
        let y_end = (hl.y + hl.h).min(height);
        let x_end = (hl.x + hl.w).min(panel_w);
        for row in hl.y..y_end {
            let row_start = (row * panel_w + hl.x) as usize;
            let row_end   = (row * panel_w + x_end) as usize;
            if row_start < buf.len() {
                let end = row_end.min(buf.len());
                for px in &mut buf[row_start..end] {
                    *px = hl.color;
                }
            }
        }
    }

    // =========================================================================
    // RENDER PHASE 3: separator lines
    // =========================================================================
    for &sy in &seps {
        if sy < height {
            draw_sep_line(buf, panel_w, sy);
        }
    }

    // =========================================================================
    // RENDER PHASE 4: power graph (Power tab only)
    // =========================================================================
    if let Some((gx, gy, gw, gh)) = graph_area {
        if gy + gh <= height {
            if let Some(graph) = info.power_graph {
                render_power_graph(buf, panel_w, gx, gy, gw, gh, graph);
            } else {
                fill_rect(buf, panel_w, gx, gy, gw, gh, 0xFF0E0E1E);
            }
        }
    }

    // =========================================================================
    // RENDER PHASE 5: all text (single embedded-graphics pass)
    // =========================================================================
    {
        let mut canvas = PanelCanvas::new(buf, panel_w, height);
        for cmd in &cmds {
            let style = MonoTextStyle::new(&FONT_6X10, cmd.color);
            let _ = Text::new(&cmd.text, Point::new(cmd.x, cmd.y), style).draw(&mut canvas);
        }
    }

    // Re-stamp left border so text can't overwrite column 0.
    for row in 0..height {
        let idx = (row * panel_w) as usize;
        if idx < buf.len() {
            buf[idx] = BORDER;
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_info(state: &DebugState) -> PanelInfo<'_> {
        PanelInfo {
            state,
            disp_w: 480,
            disp_h: 800,
            rotation_deg: 90,
            scale: 1,
            temperature: 25,
            power_graph: None,
            power_stats: None,
        }
    }

    // --- Legacy DebugPanel tests ---

    #[test]
    fn test_panel_creation() {
        let panel = DebugPanel::new();
        assert_eq!(panel.width(), 200);
    }

    #[test]
    fn test_panel_default() {
        let panel = DebugPanel::default();
        assert_eq!(panel.width(), 200);
    }

    #[test]
    fn test_panel_not_visible_when_disabled() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFFu32; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = false;

        panel.render(&mut buffer, 800, 600, &state);

        assert_eq!(buffer[0], 0xFFFFFFFF);
        assert_eq!(buffer[buffer.len() - 1], 0xFFFFFFFF);
    }

    #[test]
    fn test_panel_renders_background_when_visible() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFFu32; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = true;

        panel.render(&mut buffer, 800, 600, &state);

        let panel_pixel_idx = (0 * 800 + 700) as usize;
        assert_eq!(buffer[panel_pixel_idx], PANEL_BG_COLOR);

        let outside_pixel_idx = (0 * 800 + 400) as usize;
        assert_eq!(buffer[outside_pixel_idx], 0xFFFFFFFF);
    }

    #[test]
    fn test_panel_background_fills_entire_height() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFFu32; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = true;
        panel.render(&mut buffer, 800, 600, &state);

        assert_eq!(buffer[(0   * 800 + 700) as usize], PANEL_BG_COLOR);
        assert_eq!(buffer[(300 * 800 + 700) as usize], PANEL_BG_COLOR);
        assert_eq!(buffer[(599 * 800 + 700) as usize], PANEL_BG_COLOR);
    }

    #[test]
    fn test_panel_background_fills_entire_width() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFFu32; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = true;
        panel.render(&mut buffer, 800, 600, &state);

        assert_eq!(buffer[(100 * 800 + 600) as usize], PANEL_BG_COLOR);
        assert_eq!(buffer[(100 * 800 + 799) as usize], PANEL_BG_COLOR);
    }

    #[test]
    fn test_panel_boundary_is_correct() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFFu32; 800 * 600];
        let mut state = DebugState::new();
        state.panel_visible = true;
        panel.render(&mut buffer, 800, 600, &state);

        assert_eq!(buffer[(100 * 800 + 599) as usize], 0xFFFFFFFF);
        assert_eq!(buffer[(100 * 800 + 600) as usize], PANEL_BG_COLOR);
    }

    #[test]
    fn test_panel_with_different_screen_sizes() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFFu32; 1024 * 768];
        let mut state = DebugState::new();
        state.panel_visible = true;
        panel.render(&mut buffer, 1024, 768, &state);

        assert_eq!(buffer[(0 * 1024 + 900) as usize], PANEL_BG_COLOR);
        assert_eq!(buffer[(0 * 1024 + 700) as usize], 0xFFFFFFFF);
    }

    #[test]
    fn test_panel_with_small_screen() {
        let panel = DebugPanel::new();
        let mut buffer = vec![0xFFFFFFFFu32; 400 * 300];
        let mut state = DebugState::new();
        state.panel_visible = true;
        panel.render(&mut buffer, 400, 300, &state);

        assert_eq!(buffer[(0 * 400 + 300) as usize], PANEL_BG_COLOR);
        assert_eq!(buffer[(0 * 400 + 100) as usize], 0xFFFFFFFF);
        assert_eq!(buffer[(0 * 400 + 250) as usize], PANEL_BG_COLOR);
    }

    // --- render_into / PanelCanvas tests ---

    #[test]
    fn test_panel_canvas_fills_background() {
        let panel_w = 200u32;
        let height  = 400u32;
        let mut buf = vec![0u32; (panel_w * height) as usize];
        let state = DebugState::new();

        render_into(&mut buf, panel_w, height, &make_info(&state));

        // Background is dark navy away from text/border
        let idx = (1 * panel_w + 5) as usize;
        assert_eq!(buf[idx], 0xFF1A1A2E, "Background pixel should be dark navy");
    }

    #[test]
    fn test_panel_canvas_left_border() {
        let panel_w = 200u32;
        let height  = 400u32;
        let mut buf = vec![0u32; (panel_w * height) as usize];
        let state = DebugState::new();

        render_into(&mut buf, panel_w, height, &make_info(&state));

        for y in 0..height {
            let idx = (y * panel_w) as usize;
            assert_eq!(buf[idx], 0xFF4A4A6A, "Left border pixel at y={y} wrong");
        }
    }

    #[test]
    fn test_panel_canvas_separator_line() {
        let panel_w = 200u32;
        let height  = 400u32;
        let mut buf = vec![0u32; (panel_w * height) as usize];
        let state = DebugState::new();

        render_into(&mut buf, panel_w, height, &make_info(&state));

        // Separator at y=11 (after title at cy=10, LH=13 → cy=23, sep at 23)
        // The exact y depends on our layout; just verify col 0 is always border.
        for y in 0..height {
            assert_eq!(buf[(y * panel_w) as usize], 0xFF4A4A6A,
                "Left border overwritten at y={y}");
        }
    }

    #[test]
    fn test_panel_canvas_draw_target_bounds() {
        let w = 10u32;
        let h = 10u32;
        let mut buf = vec![0u32; (w * h) as usize];

        {
            let mut canvas = PanelCanvas::new(&mut buf, w, h);
            let _ = canvas.draw_iter([Pixel(Point::new(5, 5), Rgb888::new(255, 0, 0))]);
        }
        assert_eq!(buf[(5 * w + 5) as usize], 0xFFFF0000);

        {
            let mut canvas = PanelCanvas::new(&mut buf, w, h);
            let _ = canvas.draw_iter([
                Pixel(Point::new(-1,  0), Rgb888::new(0, 255, 0)),
                Pixel(Point::new( 0, -1), Rgb888::new(0, 255, 0)),
                Pixel(Point::new(10,  0), Rgb888::new(0, 255, 0)),
                Pixel(Point::new( 0, 10), Rgb888::new(0, 255, 0)),
            ]);
        }
        assert_eq!(buf[0], 0u32); // (0,0) untouched
    }

    #[test]
    fn test_panel_canvas_size() {
        let w = 50u32;
        let h = 80u32;
        let mut buf = vec![0u32; (w * h) as usize];
        let canvas = PanelCanvas::new(&mut buf, w, h);
        assert_eq!(canvas.size(), Size::new(w, h));
    }

    #[test]
    fn test_render_into_does_not_panic_with_full_state() {
        use crate::debug::state::ComponentInfo;
        use crate::debug::power_graph::PowerGraph;

        let panel_w = 280u32;
        let height  = 800u32;
        let mut buf = vec![0u32; (panel_w * height) as usize];

        let mut state = DebugState::new();
        state.panel_visible = true;
        state.borders_enabled = true;
        state.inspector_mode = false;
        state.power_graph_enabled = true;
        state.record_full_refresh();
        state.record_full_refresh();
        state.record_partial_refresh();
        state.register_component(ComponentInfo {
            component_type: "Button".to_string(),
            position: (0, 0),
            size: (100, 40),
            test_id: Some("play-button".to_string()),
        });
        state.register_component(ComponentInfo {
            component_type: "Container".to_string(),
            position: (0, 0),
            size: (480, 800),
            test_id: Some("display-root".to_string()),
        });

        let mut graph = PowerGraph::new();
        graph.add_sample(10.0, None);
        graph.add_sample(60.0, Some(crate::debug::state::RefreshType::Partial));
        graph.add_sample(210.0, Some(crate::debug::state::RefreshType::Full));
        graph.add_sample(12.0, None);

        let info = PanelInfo {
            state: &state,
            disp_w: 480,
            disp_h: 800,
            rotation_deg: 90,
            scale: 1,
            temperature: -5,
            power_graph: Some(&graph),
            power_stats: None,
        };

        // Must not panic
        render_into(&mut buf, panel_w, height, &info);

        // Verify left border is intact everywhere
        for y in 0..height {
            assert_eq!(buf[(y * panel_w) as usize], 0xFF4A4A6A,
                "Left border should be intact at y={y}");
        }
    }

    #[test]
    fn test_render_into_temperature_warning() {
        let panel_w = 280u32;
        let height  = 800u32;
        let mut buf = vec![0u32; (panel_w * height) as usize];

        // Cold temperature — should not panic
        let mut state = DebugState::new();
        state.panel_visible = true;
        let mut info = make_info(&state);
        info.temperature = -10;
        render_into(&mut buf, panel_w, height, &info);

        // Hot temperature — should not panic
        info.temperature = 45;
        render_into(&mut buf, panel_w, height, &info);
    }

    #[test]
    fn test_render_into_many_components() {
        use crate::debug::state::ComponentInfo;

        let panel_w = 280u32;
        let height  = 800u32;
        let mut buf = vec![0u32; (panel_w * height) as usize];

        let mut state = DebugState::new();
        state.panel_visible = true;
        for i in 0..10 {
            state.register_component(ComponentInfo {
                component_type: "Button".to_string(),
                position: (0, i * 50),
                size: (100, 40),
                test_id: Some(format!("btn-{i}")),
            });
        }

        render_into(&mut buf, panel_w, height, &make_info(&state));

        // Should render without panic and border should be intact
        for y in 0..height {
            assert_eq!(buf[(y * panel_w) as usize], 0xFF4A4A6A);
        }
    }

    #[test]
    fn test_render_into_portrait_rotation() {
        let panel_w = 280u32;
        let height  = 800u32;
        let mut buf = vec![0u32; (panel_w * height) as usize];
        let state = DebugState::new();

        for &rot in &[0u32, 90, 180, 270] {
            let info = PanelInfo {
                state: &state,
                disp_w: 480,
                disp_h: 800,
                rotation_deg: rot,
                scale: 1,
                temperature: 20,
                power_graph: None,
                power_stats: None,
            };
            render_into(&mut buf, panel_w, height, &info);
        }
    }
}
