//! Isolated window management layer
//!
//! Based on softbuffer pattern: https://github.com/rust-windowing/softbuffer
//! Follows "isolate the madness" principle - all platform-specific code here.

use crate::config::Rotation;
use crate::power::PowerStats;
use softbuffer::{Context, Surface};
use std::time::Duration;
use winit::application::ApplicationHandler;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::platform::pump_events::EventLoopExtPumpEvents;
use winit::window::{Window as WinitWindow, WindowAttributes, WindowId};

/// Apply e-ink visual characteristics to a pixel
///
/// Adds paper-like texture and subtle noise to simulate real e-ink appearance
fn apply_eink_appearance(pixel: u32, x: u32, y: u32) -> u32 {
    // Extract RGB channels (stored as 0xAARRGGBB)
    let r = ((pixel >> 16) & 0xFF) as u8;
    let g = ((pixel >> 8) & 0xFF) as u8;
    let b = (pixel & 0xFF) as u8;

    // Add subtle paper-like texture using deterministic pseudo-random noise
    // This creates the characteristic "dotty" appearance of e-ink
    let noise = pseudo_random_noise(x, y);

    // Apply noise (±3 levels for subtle texture)
    let r = (r as i16 + noise).clamp(0, 255) as u8;
    let g = (g as i16 + noise).clamp(0, 255) as u8;
    let b = (b as i16 + noise).clamp(0, 255) as u8;

    // Reduce contrast slightly to simulate matte paper (not glossy LCD)
    // E-ink black is ~15% reflectance, white is ~50%, not pure 0-100%
    let r = adjust_contrast(r, 0.9);
    let g = adjust_contrast(g, 0.9);
    let b = adjust_contrast(b, 0.9);

    0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Generate deterministic pseudo-random noise for paper texture
///
/// Uses simple hash function for consistent per-pixel noise
fn pseudo_random_noise(x: u32, y: u32) -> i16 {
    // Simple hash combining x and y
    let hash = ((x.wrapping_mul(374761393))
        .wrapping_add(y.wrapping_mul(668265263))
        .wrapping_add(x.wrapping_mul(y))) as i16;

    // Map to ±3 range for subtle texture
    (hash & 0x7) as i16 - 3
}

/// Adjust contrast to simulate matte paper surface
///
/// factor < 1.0 reduces contrast (matte), factor > 1.0 increases (glossy)
fn adjust_contrast(value: u8, factor: f32) -> u8 {
    let centered = (value as f32 - 128.0) * factor + 128.0;
    centered.clamp(0.0, 255.0) as u8
}

/// Darken a pixel by specified amount (for pixel grid effect)
fn darken(pixel: u32, amount: u8) -> u32 {
    let r = (((pixel >> 16) & 0xFF) as u8).saturating_sub(amount);
    let g = (((pixel >> 8) & 0xFF) as u8).saturating_sub(amount);
    let b = ((pixel & 0xFF) as u8).saturating_sub(amount);

    0xFF000000 | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Rotate pixel buffer 90 degrees clockwise
///
/// Transforms (x, y) → (height - 1 - y, x)
/// Output dimensions: height × width
fn rotate_90_cw(src: &[u32], width: u32, height: u32) -> Vec<u32> {
    let mut dst = vec![0u32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let src_idx = (y * width + x) as usize;
            let dst_x = height - 1 - y;
            let dst_y = x;
            let dst_idx = (dst_y * height + dst_x) as usize;
            dst[dst_idx] = src[src_idx];
        }
    }
    dst
}

/// Rotate pixel buffer 180 degrees
///
/// Transforms (x, y) → (width - 1 - x, height - 1 - y)
/// Output dimensions: width × height
fn rotate_180(src: &[u32], width: u32, height: u32) -> Vec<u32> {
    let mut dst = vec![0u32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let src_idx = (y * width + x) as usize;
            let dst_x = width - 1 - x;
            let dst_y = height - 1 - y;
            let dst_idx = (dst_y * width + dst_x) as usize;
            dst[dst_idx] = src[src_idx];
        }
    }
    dst
}

/// Rotate pixel buffer 270 degrees clockwise (90 degrees counter-clockwise)
///
/// Transforms (x, y) → (y, width - 1 - x)
/// Output dimensions: height × width
fn rotate_270_cw(src: &[u32], width: u32, height: u32) -> Vec<u32> {
    let mut dst = vec![0u32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let src_idx = (y * width + x) as usize;
            let dst_x = y;
            let dst_y = width - 1 - x;
            let dst_idx = (dst_y * height + dst_x) as usize;
            dst[dst_idx] = src[src_idx];
        }
    }
    dst
}

/// Window management (isolated from application logic)
pub struct Window {
    event_loop: Option<EventLoop<()>>,
    window: std::sync::Arc<WinitWindow>,
    surface: Surface<std::sync::Arc<WinitWindow>, std::sync::Arc<WinitWindow>>,
    width: u32,      // Logical display width
    height: u32,     // Logical display height
    config: crate::config::EmulatorConfig,
    temperature: i8,
    power_stats: Option<PowerStats>,
    quirk_warning: Option<String>,
}

/// Internal handler for the event loop
struct EventHandler {
    should_exit: bool,
}

impl ApplicationHandler for EventHandler {
    fn resumed(&mut self, _event_loop: &ActiveEventLoop) {
        // Window is already created before event loop starts
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _window_id: WindowId,
        event: WindowEvent,
    ) {
        match event {
            WindowEvent::CloseRequested => {
                self.should_exit = true;
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                // Redraw happens via present() calls
            }
            _ => {}
        }
    }
}

impl Window {
    /// Create window with configurable rotation and scaling
    ///
    /// # Arguments
    /// * `width` - Logical display width (before rotation)
    /// * `height` - Logical display height (before rotation)
    /// * `config` - Emulator configuration (rotation, scale)
    pub fn new(width: u32, height: u32, config: &crate::config::EmulatorConfig) -> Self {
        let mut event_loop = EventLoop::new().unwrap();

        // Calculate window size based on rotation and scale
        let (window_w, window_h) = config.rotation.apply_to_dimensions(width, height);
        let scaled_w = window_w * config.scale;
        let scaled_h = window_h * config.scale;

        // Create window attributes
        let window_attributes = WindowAttributes::default()
            .with_title("E-Ink Emulator")
            .with_inner_size(winit::dpi::PhysicalSize::new(scaled_w, scaled_h))
            .with_resizable(false);

        // Use a single-shot approach: create window in resumed() using pump_app_events
        struct WindowCreator {
            window_attributes: Option<WindowAttributes>,
            window: Option<std::sync::Arc<WinitWindow>>,
            surface: Option<Surface<std::sync::Arc<WinitWindow>, std::sync::Arc<WinitWindow>>>,
        }

        impl ApplicationHandler for WindowCreator {
            fn resumed(&mut self, event_loop: &ActiveEventLoop) {
                if self.window.is_none() {
                    if let Some(window_attributes) = self.window_attributes.take() {
                        match event_loop.create_window(window_attributes) {
                            Ok(window) => {
                                let window = std::sync::Arc::new(window);

                                match Context::new(window.clone()) {
                                    Ok(context) => {
                                        match Surface::new(&context, window.clone()) {
                                            Ok(surface) => {
                                                self.window = Some(window);
                                                self.surface = Some(surface);
                                            }
                                            Err(e) => eprintln!("Failed to create surface: {}", e),
                                        }
                                    }
                                    Err(e) => eprintln!("Failed to create context: {}", e),
                                }
                            }
                            Err(e) => eprintln!("Failed to create window: {}", e),
                        }
                    }
                }
            }

            fn window_event(&mut self, _: &ActiveEventLoop, _: WindowId, _: WindowEvent) {}
        }

        let mut creator = WindowCreator {
            window_attributes: Some(window_attributes),
            window: None,
            surface: None,
        };

        // Pump the event loop once to create the window
        // Use a short timeout to allow window creation without blocking
        let _ = event_loop.pump_app_events(Some(Duration::from_millis(1)), &mut creator);

        let window = creator.window.expect("Failed to create window");
        let surface = creator.surface.expect("Failed to create surface");

        let mut window_obj = Self {
            event_loop: Some(event_loop),
            window,
            surface,
            width,
            height,
            config: config.clone(),
            temperature: 25, // Default to room temperature
            power_stats: None,
            quirk_warning: None,
        };

        // Resize surface once during initialization (never resize again)
        let (window_w, window_h) = config.rotation.apply_to_dimensions(width, height);
        let final_width = window_w * config.scale;
        let final_height = window_h * config.scale;
        window_obj.surface
            .resize(
                std::num::NonZeroU32::new(final_width).unwrap(),
                std::num::NonZeroU32::new(final_height).unwrap(),
            )
            .unwrap();

        window_obj.update_title();
        window_obj
    }

    /// Update power statistics display in window title
    pub fn set_power_stats(&mut self, stats: &PowerStats) {
        self.power_stats = Some(stats.clone());
        self.update_title();
    }

    /// Update temperature display in window title
    pub fn set_temperature(&mut self, temp: i8) {
        self.temperature = temp;
        self.update_title();
    }

    /// Set hardware quirk warning in window title
    pub fn set_quirk_warning(&mut self, warning: Option<&str>) {
        self.quirk_warning = warning.map(|s| s.to_string());
        self.update_title();
    }

    /// Update window title with temperature, power info, and quirk warnings
    fn update_title(&self) {
        let temp_warning = if self.temperature < 5 || self.temperature > 35 {
            " ⚠ OUTSIDE OPTIMAL RANGE"
        } else {
            ""
        };

        let quirk_str = if let Some(quirk) = &self.quirk_warning {
            format!(" | ⚠ QUIRK: {}", quirk.chars().take(40).collect::<String>())
        } else {
            String::new()
        };

        let title = if let Some(stats) = &self.power_stats {
            let avg_ma = stats.average_current_ua as f32 / 1000.0;
            let peak_ma = stats.peak_current_ua as f32 / 1000.0;
            let total_mwh = stats.total_energy_uwh as f32 / 1000.0;

            format!(
                "E-Ink Emulator | {}°C{} | Avg: {:.1}mA | Peak: {:.1}mA | Energy: {:.2}mWh{}",
                self.temperature, temp_warning, avg_ma, peak_ma, total_mwh, quirk_str
            )
        } else {
            format!("E-Ink Emulator ({}°C){}{}",self.temperature, temp_warning, quirk_str)
        };

        self.window.set_title(&title);
    }

    /// Present framebuffer to window with rotation, upscaling, and e-ink visual effects
    pub fn present(&mut self, rgba_pixels: &[u32]) {
        let logical_width = self.width;
        let logical_height = self.height;

        // Step 1: Apply e-ink visual appearance to all pixels
        let mut eink_pixels = Vec::with_capacity(rgba_pixels.len());
        for y in 0..logical_height {
            for x in 0..logical_width {
                let pixel = rgba_pixels[(y * logical_width + x) as usize];
                eink_pixels.push(apply_eink_appearance(pixel, x, y));
            }
        }

        // Step 2: Apply rotation transformation
        let (rotated_pixels, rotated_width, rotated_height) = match self.config.rotation {
            Rotation::Degrees0 => (eink_pixels, logical_width, logical_height),
            Rotation::Degrees90 => {
                let rotated = rotate_90_cw(&eink_pixels, logical_width, logical_height);
                (rotated, logical_height, logical_width) // Width and height swap
            }
            Rotation::Degrees180 => {
                let rotated = rotate_180(&eink_pixels, logical_width, logical_height);
                (rotated, logical_width, logical_height)
            }
            Rotation::Degrees270 => {
                let rotated = rotate_270_cw(&eink_pixels, logical_width, logical_height);
                (rotated, logical_height, logical_width) // Width and height swap
            }
        };

        // Step 3: Calculate final window dimensions
        let scale = self.config.scale;
        let window_width = rotated_width * scale;
        let window_height = rotated_height * scale;

        // Surface was resized once during initialization, no need to resize every frame
        let mut buffer = self.surface.buffer_mut().unwrap();

        // Step 4: Apply upscaling with pixel grid effect
        if scale == 1 {
            // No upscaling: direct copy
            buffer.copy_from_slice(&rotated_pixels);
        } else {
            // Upscaling with pixel grid effect
            for y in 0..rotated_height {
                for x in 0..rotated_width {
                    let pixel = rotated_pixels[(y * rotated_width + x) as usize];

                    // Write scale×scale block
                    for dy in 0..scale {
                        for dx in 0..scale {
                            let sx = x * scale + dx;
                            let sy = y * scale + dy;
                            let idx = (sy * window_width + sx) as usize;

                            // Apply subtle pixel grid darkening at edges
                            let is_edge_x = dx == scale - 1 && x % 2 == 1;
                            let is_edge_y = dy == scale - 1 && y % 2 == 1;

                            buffer[idx] = if is_edge_x && is_edge_y {
                                darken(pixel, 12)
                            } else if is_edge_x || is_edge_y {
                                darken(pixel, 8)
                            } else {
                                pixel
                            };
                        }
                    }
                }
            }
        }

        buffer.present().unwrap();

        // Request redraw after presenting
        self.window.request_redraw();
    }

    /// Run event loop (blocks until window closed)
    pub fn run(mut self) {
        if let Some(event_loop) = self.event_loop.take() {
            let mut handler = EventHandler { should_exit: false };
            let _ = event_loop.run_app(&mut handler);
        }
    }
}
