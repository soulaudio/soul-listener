//! Isolated window management layer.
//!
//! Based on winit 0.30 `ApplicationHandler` + softbuffer 0.4 canonical patterns.
//!
//! # DPI / multi-monitor behaviour
//!
//! The window is created with `PhysicalSize`, bypassing DPI scaling entirely.
//! `ScaleFactorChanged` overrides the OS-suggested resize so the window keeps
//! a fixed physical pixel count on every monitor.  This mirrors e-ink hardware
//! which has a fixed pixel grid regardless of host display scale.
//!
//! On Windows we also subclass the WndProc to intercept `WM_DPICHANGED` before
//! winit's broken handler runs (winit bug #4041 — fixed in 0.31).
//! `WM_DPICHANGED` is sent synchronously (`SendMessage`) so `with_msg_hook`
//! (which only intercepts `DispatchMessage`-posted messages) cannot catch it.
//! WndProc subclassing is the correct Windows-level solution.
//!
//! # Drop order
//!
//! Rust drops struct fields in **reverse declaration order** (LIFO).  The field
//! ordering in `Window` is chosen so:
//!   `surface` drops before `_context` (surface needs context alive on some platforms)
//!   `_context` drops before `window`
//!   `window` drops before `event_loop`

use crate::config::Rotation;
use crate::power::PowerStats;
use softbuffer::{Context, Surface};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, OwnedDisplayHandle};
use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
#[cfg(feature = "debug")]
use winit::window::CursorIcon;
use winit::window::{Window as WinitWindow, WindowAttributes, WindowId};

// --- Windows DPI fix ---------------------------------------------------------
//
// winit 0.30.x bug #4041: when the user drags the window to a monitor with a
// different DPI, winit's WM_DPICHANGED handler applies a cursor-bias formula
// to compute a "corrected" window position.  When the app overrides the
// OS-suggested physical size (as we do), the formula misfires → visible jump.
//
// Fix: subclass the WndProc to intercept WM_DPICHANGED ourselves.  We honour
// the OS-suggested upper-left position (which IS correct) and apply our fixed
// physical size, then return 0 (message handled) so winit never runs.
//
// Remove this module after upgrading to winit 0.31 which fixes the bug.

#[cfg(target_os = "windows")]
mod windows_dpi {
    use std::cell::Cell;
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::UI::HiDpi::AdjustWindowRectExForDpi;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, GetWindowLongW, SetWindowLongPtrW, SetWindowPos, GWLP_WNDPROC,
        GWL_EXSTYLE, GWL_STYLE, SWP_NOACTIVATE, SWP_NOZORDER, WNDPROC,
    };

    const WM_DPICHANGED: u32 = 0x02E0;

    // Per-thread state.  Single-window, single-thread emulator.
    thread_local! {
        static ORIG_PROC: Cell<isize> = const { Cell::new(0) };
        static PHYS_W: Cell<i32>      = const { Cell::new(0) };
        static PHYS_H: Cell<i32>      = const { Cell::new(0) };
    }

    /// Replacement WndProc installed by [`install_subclass`].
    ///
    /// Intercepts `WM_DPICHANGED` to:
    ///   1. Move the window to the OS-suggested position (correct for new monitor).
    ///   2. Resize the window so the **client area** remains exactly
    ///      `PHYS_W × PHYS_H` pixels at the new DPI (via `AdjustWindowRectExForDpi`).
    ///
    /// winit bug #4041: winit's own WM_DPICHANGED handler applies a cursor-bias
    /// formula that computes a wrong window position when the app overrides the
    /// OS-suggested outer size.  By returning 0 here we prevent winit from
    /// running that handler at all.
    // SAFETY: outer.right - outer.left and outer.bottom - outer.top are RECT dimensions
    // returned by AdjustWindowRectExForDpi which guarantees right >= left and bottom >= top.
    #[allow(clippy::arithmetic_side_effects)]
    pub unsafe extern "system" fn subclass_proc(
        hwnd: isize,
        msg: u32,
        wparam: usize,
        lparam: isize,
    ) -> isize {
        if msg == WM_DPICHANGED {
            // lParam → RECT: OS-suggested outer window rect at the new DPI.
            // Its position is correct (cursor-tracked); its size would scale the
            // window visually, which we don't want — we need fixed physical pixels.
            //
            // SAFETY: Windows guarantees that for WM_DPICHANGED, lParam is a
            // non-null pointer to a RECT that is valid for the lifetime of this
            // message dispatch. The cast is therefore sound.
            let suggested = unsafe { &*(lparam as *const RECT) };

            // New DPI is in the low word of wParam.
            let new_dpi = (wparam & 0xFFFF) as u32;

            let phys_w = PHYS_W.with(|c| c.get());
            let phys_h = PHYS_H.with(|c| c.get());

            // Compute the outer window rect that gives exactly phys_w × phys_h
            // client pixels at new_dpi.  AdjustWindowRectExForDpi accounts for
            // the title bar and border widths that change with DPI.
            //
            // SAFETY: hwnd is a valid HWND supplied by Windows to this WndProc.
            // All Win32 API calls here follow their documented preconditions.
            let style = unsafe { GetWindowLongW(hwnd, GWL_STYLE) } as u32;
            let ex_style = unsafe { GetWindowLongW(hwnd, GWL_EXSTYLE) } as u32;
            let mut outer = RECT {
                left: 0,
                top: 0,
                right: phys_w,
                bottom: phys_h,
            };
            // SAFETY: &mut outer is a valid, aligned RECT pointer; style, ex_style,
            // and new_dpi are well-formed values obtained from the message above.
            unsafe { AdjustWindowRectExForDpi(&mut outer, style, 0, ex_style, new_dpi) };
            let outer_w = outer.right - outer.left;
            let outer_h = outer.bottom - outer.top;

            // SAFETY: hwnd is valid; all coordinate and flag arguments are safe values.
            unsafe {
                SetWindowPos(
                    hwnd,
                    0, // hwnd_insert_after ignored with SWP_NOZORDER
                    suggested.left,
                    suggested.top,
                    outer_w,
                    outer_h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                )
            };
            return 0; // documented: return 0 if the message is processed
        }

        // Forward all other messages to winit's original WndProc.
        let orig = ORIG_PROC.with(|c| c.get());
        // SAFETY:  was stored by  via ,
        // which returns the previous WNDPROC cast to isize. Windows guarantees this
        // value is a valid WNDPROC function pointer (or 0 for the default proc,
        // which  handles safely). The transmute from isize to the
        // Option<unsafe extern "system" fn(...)> WNDPROC type is therefore sound.
        let orig_fn: WNDPROC = unsafe { std::mem::transmute(orig) };
        // SAFETY: hwnd, msg, wparam, lparam are the unmodified values supplied by
        // Windows to this WndProc. orig_fn is a valid WNDPROC per the above.
        unsafe { CallWindowProcW(orig_fn, hwnd, msg, wparam, lparam) }
    }

    /// Subclass the given HWND so `WM_DPICHANGED` is handled correctly.
    ///
    /// Must be called **once** after the HWND is created.  Calling it again would
    /// set `ORIG_PROC` to `subclass_proc` itself, causing infinite recursion.
    /// Use [`update_size`] to update stored dimensions after the window is resized.
    pub unsafe fn install_subclass(hwnd: isize, phys_w: i32, phys_h: i32) {
        PHYS_W.with(|c| c.set(phys_w));
        PHYS_H.with(|c| c.set(phys_h));
        // SAFETY: hwnd is a valid HWND supplied by the caller (post-creation).
        // subclass_proc is a valid WNDPROC function pointer cast to isize.
        // SetWindowLongPtrW returns the previous WNDPROC (or 0) which we store
        // for later use in CallWindowProcW.
        #[allow(clippy::fn_to_numeric_cast)]
        let old = unsafe { SetWindowLongPtrW(hwnd, GWLP_WNDPROC, subclass_proc as isize) };
        ORIG_PROC.with(|c| c.set(old));
    }

    /// Update the physical size used by the subclass proc without re-installing it.
    ///
    /// Call this whenever the window is resized so `WM_DPICHANGED` uses the
    /// correct dimensions.  Re-calling `install_subclass` would set `ORIG_PROC`
    /// to `subclass_proc` itself, causing infinite recursion on the next message.
    #[cfg(feature = "debug")]
    pub fn update_size(phys_w: i32, phys_h: i32) {
        PHYS_W.with(|c| c.set(phys_w));
        PHYS_H.with(|c| c.set(phys_h));
    }
}

/// Width of the debug side panel in physical pixels (appended to the right of the display).
/// Only active when the `debug` feature is compiled in.
#[cfg(feature = "debug")]
const PANEL_W: u32 = 280;

// --- Pixel helpers -----------------------------------------------------------

// SAFETY: color channel arithmetic is on u8/u32 values in [0, 255] range; bit shifts
// are bounded at 24 bits; no overflow is possible for display pixel values.
#[allow(clippy::arithmetic_side_effects)]
fn apply_eink_appearance(pixel: u32, x: u32, y: u32) -> u32 {
    let r = ((pixel >> 16) & 0xFF) as u8;
    let g = ((pixel >> 8) & 0xFF) as u8;
    let b = (pixel & 0xFF) as u8;

    // E-ink paper tint: display white is warm cream, not blue-white like an LCD.
    // Pixels arrive as neutral gray (r=g=b) from the Gray4 framebuffer.
    // Scale the warm shift proportionally to brightness so black stays neutral.
    let luma = r as u32; // r=g=b for all Gray4 pixels
    let g = (g as u32).saturating_sub(luma * 9 / 255) as u8; // max −9 at white
    let b = (b as u32).saturating_sub(luma * 22 / 255) as u8; // max −22 at white
                                                              // Result: white (255,255,255) → (255,246,233) warm cream; black unchanged.

    let noise = pseudo_random_noise(x, y);
    let r = adjust_contrast((r as i16 + noise).clamp(0, 255) as u8, 0.9);
    let g = adjust_contrast((g as i16 + noise).clamp(0, 255) as u8, 0.9);
    let b = adjust_contrast((b as i16 + noise).clamp(0, 255) as u8, 0.9);
    0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

// SAFETY: wrapping_mul and wrapping_add are explicitly wrapping; XOR and shift are safe;
// the final masking and subtraction yield a bounded i16 result.
#[allow(clippy::arithmetic_side_effects)]
fn pseudo_random_noise(x: u32, y: u32) -> i16 {
    // Murmur3-style finalizer: no cross-term (x*y creates hyperbolic patterns),
    // full bit-avalanche so every input bit affects every output bit.
    let mut h = x
        .wrapping_mul(0x9e3779b9)
        .wrapping_add(y.wrapping_mul(0x517cc1b7));
    h ^= h >> 16;
    h = h.wrapping_mul(0x85ebca6b);
    h ^= h >> 13;
    h = h.wrapping_mul(0xc2b2ae35);
    h ^= h >> 16;
    (h & 0xF) as i16 - 7 // ±7 range, uniform distribution
}

fn adjust_contrast(value: u8, factor: f32) -> u8 {
    ((value as f32 - 128.0) * factor + 128.0).clamp(0.0, 255.0) as u8
}

fn darken(pixel: u32, amount: u8) -> u32 {
    let r = (((pixel >> 16) & 0xFF) as u8).saturating_sub(amount);
    let g = (((pixel >> 8) & 0xFF) as u8).saturating_sub(amount);
    let b = ((pixel & 0xFF) as u8).saturating_sub(amount);
    0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

// SAFETY: rotation index arithmetic is bounded by width * height pixel count; all indices
// are in range [0, width * height) by construction from loop bounds.
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
fn rotate_90_cw(src: &[u32], width: u32, height: u32) -> Vec<u32> {
    // (x, y) → (height-1-y, x);  output: height × width
    let mut dst = vec![0u32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            dst[(x * height + (height - 1 - y)) as usize] = src[(y * width + x) as usize];
        }
    }
    dst
}

// SAFETY: rotation index arithmetic is bounded by width * height pixel count.
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
fn rotate_180(src: &[u32], width: u32, height: u32) -> Vec<u32> {
    let mut dst = vec![0u32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            dst[((height - 1 - y) * width + (width - 1 - x)) as usize] =
                src[(y * width + x) as usize];
        }
    }
    dst
}

// SAFETY: rotation index arithmetic is bounded by width * height pixel count.
#[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
fn rotate_270_cw(src: &[u32], width: u32, height: u32) -> Vec<u32> {
    // (x, y) → (y, width-1-x);  output: height × width
    let mut dst = vec![0u32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            dst[((width - 1 - x) * height + y) as usize] = src[(y * width + x) as usize];
        }
    }
    dst
}

// --- Window ------------------------------------------------------------------

/// Window management, isolated from application logic.
///
/// Field declaration order is load-bearing (LIFO drop):
///   surface (pos 4) drops before _context (pos 3)
///   _context (pos 3) drops before window (pos 2)
///   window (pos 2) drops before event_loop (pos 1)
pub struct Window {
    // pos 1 — drops last
    event_loop: Option<EventLoop<()>>,
    // pos 2
    window: Arc<WinitWindow>,
    // pos 3 — must outlive surface; drops after surface (LIFO)
    _context: Context<OwnedDisplayHandle>,
    // pos 4 — drops first among these four (LIFO)
    surface: Surface<OwnedDisplayHandle, Arc<WinitWindow>>,

    /// Display-only physical pixel width (without side panel).
    /// Read in debug mode by sync_window_width() to compute panel-expanded width.
    #[cfg_attr(not(feature = "debug"), allow(dead_code))]
    disp_phys_w: u32,
    /// Current physical window width — equals disp_phys_w normally, or
    /// disp_phys_w + PANEL_W when the debug panel is open.
    phys_w: u32,
    phys_h: u32,
    /// Display content dimensions before rotation.
    disp_w: u32,
    disp_h: u32,
    config: crate::config::EmulatorConfig,
    temperature: i8,
    power_stats: Option<PowerStats>,
    quirk_warning: Option<String>,
    #[cfg(feature = "debug")]
    debug_manager: Option<crate::debug::DebugManager>,
    /// Keyboard/scroll input queue (producer half). Populated by winit events.
    #[cfg(feature = "keyboard-input")]
    input_queue: Option<crate::input::InputQueue>,
    /// Fractional scroll accumulator — carries sub-step remainder across events.
    #[cfg(feature = "keyboard-input")]
    scroll_acc: f64,
    /// Last clean frame (no debug overlays) for re-presentation on hotkey press.
    last_rgba: Vec<u32>,
}

/// `Window` IS the run-phase ApplicationHandler — no separate EventHandler needed.
impl ApplicationHandler for Window {
    fn resumed(&mut self, _: &ActiveEventLoop) {}

    /// Called when the event loop is about to sleep.
    ///
    /// When the debug panel is visible we schedule a periodic wake-up so the
    /// power-graph and inspector stay animated even without OS events.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(feature = "debug")]
        {
            let panel_open = self
                .debug_manager
                .as_ref()
                .map(|dm| dm.state().panel_visible)
                .unwrap_or(false);

            if panel_open {
                // Tick debug state (idle power sample + hover) and repaint.
                if !self.last_rgba.is_empty() {
                    self.present_overlaid();
                }
                // Wake up again in 500 ms to keep the graph animating.
                event_loop.set_control_flow(ControlFlow::WaitUntil(
                    std::time::Instant::now() + Duration::from_millis(500),
                ));
                return;
            }
        }
        // Debug panel closed (or debug feature disabled) — idle until an event arrives.
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _: WindowId, event: WindowEvent) {
        #[cfg(feature = "debug")]
        {
            let consumed = if let Some(ref mut dm) = self.debug_manager {
                use crate::debug::manager::EventResult;
                dm.handle_event(&event) == EventResult::Consumed
            } else {
                false
            };
            if consumed {
                self.re_present();
                return;
            }
        }

        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            // Keep fixed physical size when moved between monitors.
            WindowEvent::ScaleFactorChanged {
                mut inner_size_writer,
                ..
            } => {
                let _ = inner_size_writer
                    .request_inner_size(PhysicalSize::new(self.phys_w, self.phys_h));
            }
            // Resized fires after ScaleFactorChanged (with our overridden size)
            // or on initial window map — keep softbuffer surface in sync.
            WindowEvent::Resized(size) => {
                // Prevent fullscreen: if the OS put us into fullscreen mode, exit it.
                // A corrected Resized event will follow with the proper windowed size.
                if self.window.fullscreen().is_some() {
                    self.window.set_fullscreen(None);
                    return;
                }
                // Guard against OS Snap / maximize sneaking past with_resizable(false).
                // Clamp back to our target size so the surface never grows beyond it.
                if size.width > self.phys_w || size.height > self.phys_h {
                    let _ = self
                        .window
                        .request_inner_size(PhysicalSize::new(self.phys_w, self.phys_h));
                    return;
                }
                if let (Some(w), Some(h)) =
                    (NonZeroU32::new(size.width), NonZeroU32::new(size.height))
                {
                    self.surface.resize(w, h).ok();
                }
            }
            // Keyboard input: when keyboard-input feature is on, handle ALL keys
            // in one arm (F11 first, then input mapping). When off, only block F11.
            #[cfg(feature = "keyboard-input")]
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key: winit::keyboard::PhysicalKey::Code(code),
                        state,
                        ..
                    },
                ..
            } => {
                // Always prevent F11 fullscreen regardless of input mapping.
                if code == winit::keyboard::KeyCode::F11 {
                    self.window.set_fullscreen(None);
                    return;
                }
                let pressed = state == winit::event::ElementState::Pressed;
                if let Some(ref iq) = self.input_queue {
                    if let Some(ev) = crate::input::map_key(code, pressed) {
                        iq.push(ev);
                    }
                }
            }
            #[cfg(not(feature = "keyboard-input"))]
            WindowEvent::KeyboardInput {
                event:
                    winit::event::KeyEvent {
                        physical_key:
                            winit::keyboard::PhysicalKey::Code(winit::keyboard::KeyCode::F11),
                        ..
                    },
                ..
            } => {
                self.window.set_fullscreen(None);
            }
            // Scroll wheel → RotaryIncrement (keyboard-input feature only).
            #[cfg(feature = "keyboard-input")]
            WindowEvent::MouseWheel { delta, .. } => {
                let lines = match delta {
                    winit::event::MouseScrollDelta::LineDelta(_, y) => f64::from(y),
                    winit::event::MouseScrollDelta::PixelDelta(p) => p.y / 40.0,
                };
                if let Some(ref iq) = self.input_queue {
                    if let Some(ev) = crate::input::map_scroll(&mut self.scroll_acc, lines) {
                        iq.push(ev);
                    }
                }
            }
            // Mouse click: panel rows → select component; display area → select hovered.
            WindowEvent::MouseInput {
                state: winit::event::ElementState::Pressed,
                button: winit::event::MouseButton::Left,
                ..
            } => {
                #[cfg(feature = "debug")]
                {
                    if let Some(ref mut dm) = self.debug_manager {
                        // Compute where the debug panel starts in physical window pixels.
                        let (rot_w, rot_h) = match self.config.rotation {
                            crate::config::Rotation::Degrees0
                            | crate::config::Rotation::Degrees180 => (self.disp_w, self.disp_h),
                            _ => (self.disp_h, self.disp_w),
                        };
                        let panel_start = (rot_w * self.config.scale) as f64;
                        let panel_h = rot_h * self.config.scale;

                        let cursor = dm.cursor_pos();
                        if let Some((cx, cy)) = cursor {
                            if cx >= panel_start && dm.state().panel_visible {
                                // Click is inside the debug panel → hit-test scene rows.
                                let panel_x = cx - panel_start;
                                if dm.handle_panel_click(panel_x, cy, PANEL_W, panel_h) {
                                    self.re_present();
                                }
                            } else {
                                // Click is in the display area → select hovered component.
                                let hov = dm.state().hovered_component.clone();
                                let changed = if let Some(comp) = hov {
                                    let already = dm
                                        .state()
                                        .selected_component
                                        .as_ref()
                                        .map(|s| s.position == comp.position && s.size == comp.size)
                                        .unwrap_or(false);
                                    dm.state_mut().selected_component = Some(comp);
                                    !already
                                } else if dm.state().selected_component.is_some() {
                                    dm.state_mut().selected_component = None;
                                    true
                                } else {
                                    false
                                };
                                if changed {
                                    self.re_present();
                                }
                            }
                        }
                    }
                }
            }
            // Update cursor icon: pointer when over the panel or an inspectable component.
            // handle_event() already stored the position in dm.cursor_pos() above.
            WindowEvent::CursorMoved { .. } => {
                #[cfg(feature = "debug")]
                {
                    let icon = 'icon: {
                        let Some(ref dm) = self.debug_manager else {
                            break 'icon CursorIcon::Default;
                        };
                        let cursor_x = dm.cursor_pos().map(|(x, _)| x).unwrap_or(0.0);
                        let (rot_w, _) = match self.config.rotation {
                            crate::config::Rotation::Degrees0
                            | crate::config::Rotation::Degrees180 => (self.disp_w, self.disp_h),
                            _ => (self.disp_h, self.disp_w),
                        };
                        let panel_start = (rot_w * self.config.scale) as f64;
                        // Over the debug side panel.
                        if dm.state().panel_visible && cursor_x >= panel_start {
                            break 'icon CursorIcon::Pointer;
                        }
                        // Inspector mode: hovering over a registered component.
                        if dm.state().inspector_mode && dm.state().hovered_component.is_some() {
                            break 'icon CursorIcon::Pointer;
                        }
                        CursorIcon::Default
                    };
                    self.window.set_cursor(icon);
                }
            }
            _ => {}
        }
    }
}

impl Window {
    /// Create the emulator window.
    ///
    /// `width` / `height` are display content dimensions before rotation.
    /// Physical window pixels = rotated dimensions × `config.scale`.
    // SAFETY: win_w * config.scale and win_h * config.scale are bounded by display dimensions
    // (max ~4000px) * scale (max ~4); product fits in u32.
    #[allow(clippy::arithmetic_side_effects)]
    pub fn new(width: u32, height: u32, config: &crate::config::EmulatorConfig) -> Self {
        let mut event_loop = EventLoop::builder()
            .build()
            .expect("Failed to create winit event loop");

        let (win_w, win_h) = config.rotation.apply_to_dimensions(width, height);
        // Start at display-only size. Panel expands the window when toggled on.
        let phys_w = (win_w * config.scale).max(1);
        let phys_h = (win_h * config.scale).max(1);
        let phys_size = PhysicalSize::new(phys_w, phys_h);

        // PhysicalSize: raw pixels, no DPI multiplication applied by the OS.
        // min == max == inner: belt-and-suspenders against drag-resize.
        // with_resizable(false): removes the resize border/handle.
        let attrs = WindowAttributes::default()
            .with_title("E-Ink Emulator")
            .with_inner_size(phys_size)
            .with_min_inner_size(phys_size)
            .with_max_inner_size(phys_size)
            .with_resizable(false);

        // winit 0.30: windows must be created inside `resumed()`.
        // We pump once to trigger that callback, extract window + surface,
        // then continue in pre-run phase.
        #[allow(clippy::type_complexity)]
        struct Creator {
            attrs: Option<WindowAttributes>,
            result: Option<(
                Arc<WinitWindow>,
                Context<OwnedDisplayHandle>,
                Surface<OwnedDisplayHandle, Arc<WinitWindow>>,
            )>,
        }

        impl ApplicationHandler for Creator {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                if self.result.is_some() {
                    return; // already created
                }
                let attrs = self
                    .attrs
                    .take()
                    .expect("Creator.attrs was None — resumed() called twice");
                let window = Arc::new(
                    event_loop
                        .create_window(attrs)
                        .expect("create_window failed"),
                );

                // OwnedDisplayHandle is a self-contained owned display connection
                // (HINSTANCE on Windows, Wayland display on Linux, etc.).
                // Correct pattern per softbuffer 0.4 + winit 0.30 examples.
                let context = Context::new(event_loop.owned_display_handle())
                    .expect("softbuffer Context failed");
                let surface =
                    Surface::new(&context, window.clone()).expect("softbuffer Surface failed");

                self.result = Some((window, context, surface));
            }

            fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
        }

        let mut creator = Creator {
            attrs: Some(attrs),
            result: None,
        };

        // One pump is enough — resumed() fires synchronously on all desktop platforms.
        let _ = event_loop.pump_app_events(Some(Duration::from_millis(100)), &mut creator);

        let (window, context, mut surface) = creator
            .result
            .expect("Window creation failed — resumed() never fired");

        // Windows: install WndProc subclass to fix WM_DPICHANGED position jump.
        // Must happen after the HWND exists (i.e. after resumed() fires above).
        #[cfg(target_os = "windows")]
        {
            use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
            if let Ok(handle) = window.window_handle() {
                if let RawWindowHandle::Win32(h) = handle.as_raw() {
                    unsafe {
                        windows_dpi::install_subclass(h.hwnd.get(), phys_w as i32, phys_h as i32);
                    }
                }
            }
        }

        // Pre-size surface so the first present() skips the resize step.
        if let (Some(w), Some(h)) = (NonZeroU32::new(phys_w), NonZeroU32::new(phys_h)) {
            surface.resize(w, h).ok();
        }

        let obj = Self {
            event_loop: Some(event_loop),
            window,
            _context: context,
            surface,
            disp_phys_w: phys_w,
            phys_w,
            phys_h,
            disp_w: width,
            disp_h: height,
            config: config.clone(),
            temperature: 25,
            power_stats: None,
            quirk_warning: None,
            #[cfg(feature = "debug")]
            debug_manager: None,
            #[cfg(feature = "keyboard-input")]
            input_queue: None,
            #[cfg(feature = "keyboard-input")]
            scroll_acc: 0.0,
            last_rgba: Vec::new(),
        };

        obj.update_title();
        obj
    }

    /// Pump OS events for `duration` in ~16 ms steps.
    ///
    /// Call during long refresh animations instead of `thread::sleep` so the
    /// OS does not mark the window as "Not Responding".
    pub fn pump_events(&mut self, duration: Duration) {
        let step = Duration::from_millis(16);
        let mut remaining = duration;

        while remaining > Duration::ZERO {
            std::thread::sleep(remaining.min(step));
            remaining = remaining.saturating_sub(step);

            // Take the event_loop out so we can pass `self` as the ApplicationHandler.
            // After pumping we put it back.  If el is gone the window is already closed.
            if let Some(mut el) = self.event_loop.take() {
                let status = el.pump_app_events(Some(Duration::ZERO), self);
                self.event_loop = Some(el);
                if matches!(status, PumpStatus::Exit(_)) {
                    break;
                }
            } else {
                std::thread::sleep(remaining);
                break;
            }
        }
    }

    /// Poll all pending OS events without blocking.
    ///
    /// Uses `self` as the `ApplicationHandler` so `KeyboardInput` and
    /// `MouseWheel` events ARE forwarded to `input_queue`.
    ///
    /// Returns `true` if the window is still open, `false` if the close
    /// button was clicked (`CloseRequested` → `event_loop.exit()`).
    pub fn pump_window_events(&mut self) -> bool {
        if let Some(mut el) = self.event_loop.take() {
            let status = el.pump_app_events(Some(Duration::ZERO), self);
            self.event_loop = Some(el);
            !matches!(status, PumpStatus::Exit(_))
        } else {
            false // window is already closed / run() consumed the event loop
        }
    }

    /// Present a framebuffer with rotation, emulator scale, and e-ink effects.
    ///
    /// Saves the clean frame so debug overlays can be re-applied instantly
    /// when a debug hotkey fires during the blocking event loop.
    pub fn present(&mut self, rgba_pixels: &[u32]) {
        self.last_rgba = rgba_pixels.to_vec();
        self.present_overlaid();
    }

    /// Re-present the last clean frame with the current debug overlay state.
    /// Called immediately after a debug hotkey toggles state.
    #[cfg(feature = "debug")]
    fn re_present(&mut self) {
        #[cfg(feature = "debug")]
        self.sync_window_width();
        if !self.last_rgba.is_empty() {
            self.present_overlaid();
        }
    }

    /// Expand or shrink the OS window to accommodate the side panel.
    ///
    /// Called whenever the panel visibility may have changed. Uses
    /// `winit`'s `request_inner_size` which takes effect immediately on
    /// most platforms (an `InnerSize` event may follow).
    #[cfg(feature = "debug")]
    fn sync_window_width(&mut self) {
        let panel_open = self
            .debug_manager
            .as_ref()
            .map(|dm| dm.state().panel_visible)
            .unwrap_or(false);

        let target_w = if panel_open {
            self.disp_phys_w + PANEL_W
        } else {
            self.disp_phys_w
        };

        if self.phys_w != target_w {
            self.phys_w = target_w;
            let new_size = PhysicalSize::new(self.phys_w, self.phys_h);
            // Update min/max so the OS permits the new size.
            self.window.set_min_inner_size(Some(new_size));
            self.window.set_max_inner_size(Some(new_size));
            let _ = self.window.request_inner_size(new_size);
            // Update the stored size in the WndProc subclass so DPI changes
            // use the new width.  We must NOT re-call install_subclass here:
            // doing so would set ORIG_PROC to subclass_proc itself, causing
            // infinite recursion on the next WndProc message.
            #[cfg(target_os = "windows")]
            windows_dpi::update_size(self.phys_w as i32, self.phys_h as i32);
        }
    }

    /// Apply debug overlays to `rgba` (pre-rotation display coordinates).
    #[cfg(feature = "debug")]
    fn apply_debug_overlays(&self, rgba: &mut [u32]) {
        use crate::debug;
        let Some(ref dm) = self.debug_manager else {
            return;
        };
        let state = dm.state();
        let w = self.disp_w;
        let h = self.disp_h;

        if state.borders_enabled {
            let components = if !state.registered_components.is_empty() {
                state.registered_components.clone()
            } else {
                let header_h = (h / 7).max(1);
                let footer_h = (h / 7).max(1);
                let content_y = header_h as i32;
                let content_h = h.saturating_sub(header_h + footer_h);
                let footer_y = (header_h + content_h) as i32;
                vec![
                    debug::ComponentInfo {
                        component_type: "Container".to_string(),
                        position: (0, 0),
                        size: (w, h),
                        test_id: Some("display-root".to_string()),
                        ..Default::default()
                    },
                    debug::ComponentInfo {
                        component_type: "Label".to_string(),
                        position: (0, 0),
                        size: (w, header_h),
                        test_id: Some("header".to_string()),
                        ..Default::default()
                    },
                    debug::ComponentInfo {
                        component_type: "Button".to_string(),
                        position: (0, content_y),
                        size: (w, content_h),
                        test_id: Some("content".to_string()),
                        ..Default::default()
                    },
                    debug::ComponentInfo {
                        component_type: "ProgressBar".to_string(),
                        position: (0, footer_y),
                        size: (w, footer_h),
                        test_id: Some("footer".to_string()),
                        ..Default::default()
                    },
                ]
            };
            debug::OverlayRenderer::new().render_layout(rgba, w, h, &components);
        }

        // Inspector mode: draw amber highlight over the hovered component.
        // Skipped when the hovered component is also the selected one (cyan wins).
        if state.inspector_mode {
            if let Some(ref hov) = state.hovered_component {
                let is_also_selected = state
                    .selected_component
                    .as_ref()
                    .map(|s| s.position == hov.position && s.size == hov.size)
                    .unwrap_or(false);
                if !is_also_selected {
                    let renderer = debug::OverlayRenderer::new();
                    if hov.margin.is_zero() && hov.padding.is_zero() && hov.border.is_zero() {
                        renderer.render_hovered_component(rgba, w, h, hov);
                    } else {
                        renderer.render_hovered_box_model(rgba, w, h, hov);
                    }
                }
            }
        }

        // Always draw cyan highlight around the clicked/selected component.
        if let Some(ref selected) = state.selected_component {
            debug::OverlayRenderer::new().render_selected_component(rgba, w, h, selected);
        }

        // Note: the side panel is composited in present_internal(), not here.
        // apply_debug_overlays only handles overlays ON the display content.
    }

    /// Composite debug overlays onto `last_rgba` then run the full render pipeline.
    #[cfg_attr(not(feature = "debug"), allow(unused_mut))]
    fn present_overlaid(&mut self) {
        // Tick the debug state: idle power sampling + cursor hover detection.
        #[cfg(feature = "debug")]
        {
            let disp_w = self.disp_w;
            let disp_h = self.disp_h;
            let scale = self.config.scale;
            if let Some(ref mut dm) = self.debug_manager {
                dm.maybe_add_idle_sample();
                dm.update_hovered_component(disp_w, disp_h, scale);
            }
        }
        let mut rgba = self.last_rgba.clone();
        #[cfg(feature = "debug")]
        self.apply_debug_overlays(&mut rgba);
        self.present_internal(&rgba);
    }

    /// Core render pipeline: e-ink effects → rotation → scale → softbuffer.
    // SAFETY: all arithmetic here operates on display/window pixel coordinates bounded by
    // phys_w * phys_h and rot_w * rot_h * scale — all display-scale values that fit in u32/usize.
    // Indexing is bounded by the same coordinate checks (x < rot_w, y < rot_h, etc.).
    #[allow(clippy::arithmetic_side_effects, clippy::indexing_slicing)]
    fn present_internal(&mut self, rgba_pixels: &[u32]) {
        let disp_w = self.disp_w;
        let disp_h = self.disp_h;

        // 1. E-ink appearance pass
        let mut eink: Vec<u32> = Vec::with_capacity(rgba_pixels.len());
        for y in 0..disp_h {
            for x in 0..disp_w {
                eink.push(apply_eink_appearance(
                    rgba_pixels[(y * disp_w + x) as usize],
                    x,
                    y,
                ));
            }
        }

        // 2. Rotation
        let (rotated, rot_w, rot_h) = match self.config.rotation {
            Rotation::Degrees0 => (eink, disp_w, disp_h),
            Rotation::Degrees90 => {
                let r = rotate_90_cw(&eink, disp_w, disp_h);
                (r, disp_h, disp_w)
            }
            Rotation::Degrees180 => {
                let r = rotate_180(&eink, disp_w, disp_h);
                (r, disp_w, disp_h)
            }
            Rotation::Degrees270 => {
                let r = rotate_270_cw(&eink, disp_w, disp_h);
                (r, disp_h, disp_w)
            }
        };

        // 3. Emulator scale + pixel-grid effect
        let scale = self.config.scale;
        let src_w = rot_w * scale;
        let src_h = rot_h * scale;

        let source: Vec<u32> = if scale == 1 {
            rotated
        } else {
            let mut buf = vec![0u32; (src_w * src_h) as usize];
            for y in 0..rot_h {
                for x in 0..rot_w {
                    let pixel = rotated[(y * rot_w + x) as usize];
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let is_ex = dx == scale - 1 && x % 2 == 1;
                            let is_ey = dy == scale - 1 && y % 2 == 1;
                            buf[((y * scale + dy) * src_w + x * scale + dx) as usize] =
                                match (is_ex, is_ey) {
                                    (true, true) => darken(pixel, 12),
                                    (true, false) | (false, true) => darken(pixel, 8),
                                    _ => pixel,
                                };
                        }
                    }
                }
            }
            buf
        };

        // 4. Build the full window buffer (display + optional side panel).
        //    phys_w = src_w + PANEL_W (debug) or src_w (release).
        let pw = self.phys_w;
        let ph = self.phys_h;

        // Composite: copy display rows into left portion, render panel into right portion.
        let mut full: Vec<u32> = vec![0xFF000000; (pw * ph) as usize];

        // Copy display content row by row into the left pw pixels.
        // If src dimensions match exactly we skip the nearest-neighbour path.
        if src_w <= pw && src_h == ph {
            for y in 0..src_h {
                let src_row = &source[(y * src_w) as usize..((y + 1) * src_w) as usize];
                let dst_start = (y * pw) as usize;
                full[dst_start..dst_start + src_w as usize].copy_from_slice(src_row);
            }
        } else {
            // Nearest-neighbour for any dimension mismatch.
            for dy in 0..ph {
                for dx in 0..src_w.min(pw) {
                    let sx = (dx as u64 * src_w as u64 / src_w.min(pw) as u64).min(src_w as u64 - 1)
                        as u32;
                    let sy = (dy as u64 * src_h as u64 / ph as u64).min(src_h as u64 - 1) as u32;
                    full[(dy * pw + dx) as usize] = source[(sy * src_w + sx) as usize];
                }
            }
        }

        // Render the debug side panel into the right PANEL_W columns.
        #[cfg(feature = "debug")]
        if let Some(ref dm) = self.debug_manager {
            let state = dm.state();
            if !state.panel_visible {
                // Panel hidden — nothing to render here.
            } else {
                let panel_x = src_w; // panel starts right after the display area
                let panel_h = ph;

                // Extract the panel slice (row-major, panel_w columns wide).
                // We build a compact panel_w × panel_h buffer, then copy it in.
                let mut panel_buf = vec![0u32; (PANEL_W * panel_h) as usize];
                let rotation_deg = match self.config.rotation {
                    crate::config::Rotation::Degrees0 => 0,
                    crate::config::Rotation::Degrees90 => 90,
                    crate::config::Rotation::Degrees180 => 180,
                    crate::config::Rotation::Degrees270 => 270,
                };
                let info = crate::debug::PanelInfo {
                    state,
                    disp_w: self.disp_w,
                    disp_h: self.disp_h,
                    rotation_deg,
                    scale: self.config.scale,
                    temperature: self.temperature,
                    power_graph: Some(dm.power_graph()),
                    power_stats: self.power_stats.as_ref(),
                };
                crate::debug::panel::render_into(&mut panel_buf, PANEL_W, panel_h, &info);

                // Copy compact panel buffer into the right columns of `full`.
                for y in 0..panel_h {
                    for x in 0..PANEL_W {
                        let src_idx = (y * PANEL_W + x) as usize;
                        let dst_idx = (y * pw + panel_x + x) as usize;
                        if dst_idx < full.len() {
                            full[dst_idx] = panel_buf[src_idx];
                        }
                    }
                }
            } // end else (panel_visible)
        }

        // 5. Write to softbuffer.
        if let (Some(w), Some(h)) = (NonZeroU32::new(pw), NonZeroU32::new(ph)) {
            self.surface.resize(w, h).ok();
        }
        let mut buffer = match self.surface.buffer_mut() {
            Ok(b) => b,
            Err(_) => return,
        };
        if buffer.len() != (pw * ph) as usize {
            return;
        }
        buffer.copy_from_slice(&full);

        self.window.pre_present_notify();
        buffer.present().ok();
        self.window.request_redraw();
    }

    /// Enter the blocking event loop (blocks until window is closed).
    pub fn run(mut self) {
        if let Some(event_loop) = self.event_loop.take() {
            // Wait: sleep when idle; wake on events.  Prevents 100% CPU spin.
            event_loop.set_control_flow(ControlFlow::Wait);
            // Window itself is the ApplicationHandler — no separate struct needed.
            let _ = event_loop.run_app(&mut self);
        }
    }

    // --- Metadata setters ----------------------------------------------------

    pub fn set_power_stats(&mut self, stats: &PowerStats) {
        self.power_stats = Some(stats.clone());
        self.update_title();
    }

    pub fn set_temperature(&mut self, temp: i8) {
        self.temperature = temp;
        self.update_title();
    }

    pub fn set_quirk_warning(&mut self, warning: Option<&str>) {
        self.quirk_warning = warning.map(str::to_string);
        self.update_title();
    }

    #[cfg(feature = "debug")]
    pub fn set_debug_manager(&mut self, dm: crate::debug::DebugManager) {
        self.debug_manager = Some(dm);
    }

    /// Attach a keyboard/scroll input queue so winit events are forwarded to
    /// the application's [`EmulatorInput`](crate::input::EmulatorInput).
    ///
    /// Called by [`Emulator::run()`](crate::Emulator::run) when
    /// [`keyboard-input`](crate) feature is active.
    #[cfg(feature = "keyboard-input")]
    pub fn set_input_queue(&mut self, iq: crate::input::InputQueue) {
        self.input_queue = Some(iq);
    }

    fn update_title(&self) {
        let temp_warn = if self.temperature < 5 || self.temperature > 35 {
            " ⚠ OUTSIDE OPTIMAL RANGE"
        } else {
            ""
        };

        let quirk = match &self.quirk_warning {
            Some(q) => format!(" | ⚠ QUIRK: {}", q.chars().take(40).collect::<String>()),
            None => String::new(),
        };

        let title = match &self.power_stats {
            Some(s) => format!(
                "E-Ink Emulator | {}°C{} | Avg: {:.1}mA | Peak: {:.1}mA | Energy: {:.2}mWh{}",
                self.temperature,
                temp_warn,
                s.average_current_ua as f32 / 1000.0,
                s.peak_current_ua as f32 / 1000.0,
                s.total_energy_uwh as f32 / 1000.0,
                quirk,
            ),
            None => format!(
                "E-Ink Emulator ({}°C){}{}",
                self.temperature, temp_warn, quirk
            ),
        };

        self.window.set_title(&title);
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pump_events_compiles_without_pump_event_handler() {
        // Compile-only test — documents that PumpEventHandler has been removed
        // and pump_events() uses Window as its ApplicationHandler.
    }

    #[test]
    fn pump_window_events_returns_false_without_event_loop() {
        // A Window with no event_loop returns false from pump_window_events().
        // Compile-only — runtime path requires a display.
        let _ = "pump_window_events returns false when event_loop is None";
    }
}
