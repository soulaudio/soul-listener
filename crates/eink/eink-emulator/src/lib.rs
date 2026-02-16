//! E-Ink Display Emulator
//!
//! Desktop emulator for e-ink displays with realistic behavior simulation.
//!
//! # Phase 0: Foundation
//!
//! - Window management (winit + softbuffer)
//! - Framebuffer (Gray4 pixels)
//! - embedded-graphics integration
//! - Headless mode for CI
//!
//! # Example
//!
//! ```no_run
//! use eink_emulator::{Emulator, DisplayDriver};
//! use embedded_graphics::prelude::*;
//! use embedded_graphics::primitives::{Rectangle, PrimitiveStyle};
//! use embedded_graphics::pixelcolor::Gray4;
//!
//! # async fn example() {
//! let mut emulator = Emulator::new(250, 122);
//!
//! Rectangle::new(Point::new(10, 10), Size::new(50, 30))
//!     .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
//!     .draw(&mut emulator)
//!     .unwrap();
//!
//! emulator.refresh_full().await.unwrap();
//! emulator.run();
//! # }
//! ```

pub mod alignment;
pub mod config;
mod display_driver;
mod framebuffer;
mod initialization;
pub mod lut;
pub mod partial_window;
pub mod pixel_color;
mod pixel_state;
pub mod power;
mod refresh_mode;
mod waveform_mode;

#[cfg(not(feature = "headless"))]
mod window;

#[cfg(feature = "debug")]
pub mod debug;

pub use config::{EmulatorConfig, Rotation};
pub use display_driver::{DisplayDriver, EinkDisplay};
pub use framebuffer::{ColorMode, Framebuffer};
pub use initialization::{InitSequence, InitStep, InitializationState};
pub use lut::{LutError, LutPhase, WaveformLut, WaveformLutSet};
pub use partial_window::PartialWindow;
pub use pixel_color::{EinkColor, SpectraColor};
pub use pixel_state::{PixelState, PixelStateBuffer};
pub use power::{PowerProfile, PowerState, PowerStats, PowerTracker, StatePercentages};
pub use refresh_mode::{RefreshMode, RefreshStrategy};
pub use waveform_mode::WaveformMode;

use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;

/// Convert EinkColor framebuffer to RGBA buffer for rendering
fn framebuffer_to_rgba(framebuffer: &[EinkColor]) -> Vec<u32> {
    framebuffer.iter().map(|pixel| pixel.to_rgba()).collect()
}

/// Display statistics tracking
#[derive(Debug, Clone, Copy, Default)]
pub struct DisplayStats {
    pub full_refresh_count: u64,
    pub partial_refresh_count: u64,
    pub fast_refresh_count: u64,
    pub total_refresh_time_ms: u64,
    pub dc_warnings: u32,
}

impl DisplayStats {
    fn record_refresh(&mut self, mode: WaveformMode, duration_ms: u32) {
        match mode {
            WaveformMode::GC16 | WaveformMode::GL16 | WaveformMode::GCC16 => {
                self.full_refresh_count += 1
            }
            WaveformMode::DU4 => self.partial_refresh_count += 1,
            WaveformMode::DU | WaveformMode::A2 | WaveformMode::GCU => self.fast_refresh_count += 1,
        }
        self.total_refresh_time_ms += duration_ms as u64;
    }
}

/// E-Ink display emulator with realistic behavior simulation
pub struct Emulator {
    pub framebuffer: Framebuffer,
    staged_buffer: Vec<EinkColor>,
    spec: &'static eink_specs::DisplaySpec,
    pixel_states: PixelStateBuffer,
    waveform_mode: WaveformMode,
    current_temp: i8,
    refresh_mode: RefreshMode,
    stats: DisplayStats,

    // Dirty region tracking
    dirty_regions: Vec<embedded_graphics::primitives::Rectangle>,
    auto_track_dirty: bool,

    // Initialization tracking
    init_sequence: InitSequence,
    requires_init: bool,

    // Power tracking
    power_tracker: PowerTracker,

    // Debug system
    #[cfg(feature = "debug")]
    debug_manager: Option<debug::DebugManager>,

    // Hardware quirks simulation
    pub quirks_enabled: bool,
    pub active_quirk: Option<String>,

    // Presentation configuration (rotation, scaling)
    config: config::EmulatorConfig,

    #[cfg(not(feature = "headless"))]
    window: Option<window::Window>,
}

impl Emulator {
    /// Create new emulator with window using default Waveshare 2.13" V4 spec
    pub fn new(_width: u32, _height: u32) -> Self {
        Self::with_spec(&eink_specs::displays::WAVESHARE_2_13_V4)
    }

    /// Create emulator with custom configuration
    pub fn with_config(config: config::EmulatorConfig) -> Self {
        Self::with_spec_and_config(&eink_specs::displays::WAVESHARE_2_13_V4, config)
    }

    /// Create emulator with specific display specification
    pub fn with_spec(spec: &'static eink_specs::DisplaySpec) -> Self {
        Self::with_spec_and_config(spec, config::EmulatorConfig::default())
    }

    /// Create emulator with specific display specification and configuration
    pub fn with_spec_and_config(
        spec: &'static eink_specs::DisplaySpec,
        config: config::EmulatorConfig,
    ) -> Self {
        let buffer_size = (spec.width * spec.height) as usize;

        // Apply rotation to get logical dimensions for drawing
        let (logical_width, logical_height) =
            config.rotation.apply_to_dimensions(spec.width, spec.height);

        // Select power profile based on display spec
        let power_profile = Self::select_power_profile(spec);

        // Create window config with no rotation (framebuffer is pre-rotated)
        let window_config = config::EmulatorConfig {
            rotation: config::Rotation::Degrees0,
            scale: config.scale,
        };

        #[cfg(feature = "debug")]
        let debug_manager = Some(debug::DebugManager::new());

        Self {
            framebuffer: Framebuffer::new(logical_width, logical_height),
            staged_buffer: vec![EinkColor::default(); buffer_size],
            spec,
            pixel_states: PixelStateBuffer::new(logical_width, logical_height),
            waveform_mode: WaveformMode::default(),
            current_temp: 25, // Default to room temperature
            refresh_mode: RefreshMode::default(),
            stats: DisplayStats::default(),
            dirty_regions: Vec::new(),
            auto_track_dirty: false,
            init_sequence: InitSequence::new(),
            requires_init: false, // Disabled by default for backward compatibility
            power_tracker: PowerTracker::new(power_profile),
            #[cfg(feature = "debug")]
            debug_manager,
            quirks_enabled: true, // Enabled by default for realistic simulation
            active_quirk: None,
            config: config.clone(),

            #[cfg(not(feature = "headless"))]
            window: Some(window::Window::new(
                logical_width,
                logical_height,
                &window_config,
            )),
        }
    }

    /// Select appropriate power profile based on display specification
    fn select_power_profile(spec: &eink_specs::DisplaySpec) -> &'static PowerProfile {
        // Match by display name first (most accurate)
        match spec.name {
            "Waveshare 2.13\" V4" => &PowerProfile::WAVESHARE_2_13_V4,
            "Waveshare 2.9\" V2" => &PowerProfile::WAVESHARE_2_9_V2,
            "Waveshare 4.2\" V2" => &PowerProfile::WAVESHARE_4_2_V2,
            "Waveshare 7.5\" V2" => &PowerProfile::WAVESHARE_7_5_V2,
            // Fallback: guess by size
            _ => {
                let pixels = spec.width * spec.height;
                if pixels < 40_000 {
                    &PowerProfile::WAVESHARE_2_13_V4
                } else if pixels < 100_000 {
                    &PowerProfile::WAVESHARE_2_9_V2
                } else if pixels < 200_000 {
                    &PowerProfile::WAVESHARE_4_2_V2
                } else {
                    &PowerProfile::WAVESHARE_7_5_V2
                }
            }
        }
    }

    /// Create headless emulator (for testing/CI)
    pub fn headless(_width: u32, _height: u32) -> Self {
        Self::headless_with_spec(&eink_specs::displays::WAVESHARE_2_13_V4)
    }

    /// Create headless emulator with specific display specification
    pub fn headless_with_spec(spec: &'static eink_specs::DisplaySpec) -> Self {
        let buffer_size = (spec.width * spec.height) as usize;
        let power_profile = Self::select_power_profile(spec);

        Self {
            framebuffer: Framebuffer::new(spec.width, spec.height),
            staged_buffer: vec![EinkColor::default(); buffer_size],
            spec,
            pixel_states: PixelStateBuffer::new(spec.width, spec.height),
            waveform_mode: WaveformMode::default(),
            current_temp: 25,
            refresh_mode: RefreshMode::default(),
            stats: DisplayStats::default(),
            dirty_regions: Vec::new(),
            auto_track_dirty: false,
            init_sequence: InitSequence::new(),
            requires_init: false, // Disabled by default for backward compatibility
            power_tracker: PowerTracker::new(power_profile),
            #[cfg(feature = "debug")]
            debug_manager: None, // Debug not supported in headless mode
            quirks_enabled: true, // Enabled by default for realistic simulation
            active_quirk: None,
            config: config::EmulatorConfig::default(), // Config not used in headless mode

            #[cfg(not(feature = "headless"))]
            window: None,
        }
    }

    /// Set current temperature (for testing temperature compensation)
    pub fn set_temperature(&mut self, temp: i8) {
        self.current_temp = temp;

        // Update window title if in graphical mode
        #[cfg(not(feature = "headless"))]
        if let Some(window) = &mut self.window {
            window.set_temperature(temp);
            window.set_power_stats(self.power_tracker.stats());
        }
    }

    /// Get power statistics
    pub fn power_stats(&self) -> &PowerStats {
        self.power_tracker.stats()
    }

    /// Reset power statistics
    pub fn reset_power_stats(&mut self) {
        self.power_tracker.reset();
    }

    /// Enable or disable power tracking
    pub fn enable_power_tracking(&mut self, enabled: bool) {
        self.power_tracker.set_enabled(enabled);
    }

    /// Check if power tracking is enabled
    pub fn is_power_tracking_enabled(&self) -> bool {
        self.power_tracker.is_enabled()
    }

    /// Get current ghosting level (average across all pixels)
    pub fn ghosting_level(&self) -> f32 {
        self.pixel_states.average_ghosting()
    }

    /// Get current waveform mode
    pub fn waveform_mode(&self) -> WaveformMode {
        self.waveform_mode
    }

    /// Set waveform mode for subsequent refreshes
    pub fn set_waveform_mode(&mut self, mode: WaveformMode) {
        self.waveform_mode = mode;
    }

    /// Get initialization state
    pub fn init_state(&self) -> &InitializationState {
        self.init_sequence.state()
    }

    /// Set whether initialization is required before refresh
    ///
    /// When enabled, calling refresh methods without initialization will return an error.
    /// Disabled by default for backward compatibility with existing code.
    pub fn set_requires_initialization(&mut self, requires: bool) {
        self.requires_init = requires;
    }

    /// Set initialization timeout in milliseconds (default: 5000ms)
    pub fn set_init_timeout(&mut self, timeout_ms: u32) {
        self.init_sequence.set_timeout(timeout_ms);
    }

    /// Initialize the display with 7-step sequence
    ///
    /// Performs realistic initialization sequence similar to real e-ink displays:
    /// 1. Power settling (100ms)
    /// 2. Panel detection (50ms)
    /// 3. Temperature sensor (20ms)
    /// 4. Waveform tables (200ms)
    /// 5. VCOM calibration (100ms)
    /// 6. Checkerboard pattern (500ms) - Visual
    /// 7. Clear to white (1000ms) - Visual
    ///
    /// Total time: ~1970ms (varies by temperature)
    ///
    /// # Example
    /// ```no_run
    /// # use eink_emulator::{Emulator, DisplayDriver};
    /// # async fn example() {
    /// let mut emulator = Emulator::new(250, 122);
    /// emulator.set_requires_initialization(true);
    /// emulator.initialize().await.unwrap();
    /// emulator.refresh_full().await.unwrap();
    /// # }
    /// ```
    pub async fn initialize(&mut self) -> Result<(), std::io::Error> {
        use tokio::time::{sleep, Duration};

        // Transition to initializing state
        self.power_tracker.transition_to(PowerState::Initializing);

        // Start initialization sequence
        self.init_sequence
            .start()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        let steps = InitStep::all_steps();

        for step in steps {
            // Execute step timing
            sleep(Duration::from_millis(step.duration_ms as u64)).await;

            // Visual steps
            if step.has_visual {
                match step.number {
                    6 => {
                        // Checkerboard pattern (8x8 squares)
                        self.display_checkerboard().await;
                    }
                    7 => {
                        // Clear to white
                        self.framebuffer.clear();
                        let rgba = framebuffer_to_rgba(&self.framebuffer.pixels);
                        #[cfg(not(feature = "headless"))]
                        self.present_frame(&rgba).await;
                    }
                    _ => {}
                }
            }

            // Advance to next step
            self.init_sequence
                .next_step()
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        }

        // Return to idle after initialization
        self.power_tracker.transition_to(PowerState::Idle);

        // Update window title with power stats
        #[cfg(not(feature = "headless"))]
        if let Some(window) = &mut self.window {
            window.set_power_stats(self.power_tracker.stats());
        }

        Ok(())
    }

    /// Display checkerboard pattern (used during initialization)
    async fn display_checkerboard(&mut self) {
        const SQUARE_SIZE: u32 = 8;

        for y in 0..self.framebuffer.height {
            for x in 0..self.framebuffer.width {
                let square_x = x / SQUARE_SIZE;
                let square_y = y / SQUARE_SIZE;
                let is_black = (square_x + square_y) % 2 == 0;
                let color = if is_black { Gray4::BLACK } else { Gray4::WHITE };
                self.framebuffer.set_pixel(x, y, EinkColor::Gray(color));
            }
        }

        // Present the checkerboard
        let rgba = framebuffer_to_rgba(&self.framebuffer.pixels);
        #[cfg(not(feature = "headless"))]
        self.present_frame(&rgba).await;
    }

    /// Get display statistics
    pub fn stats(&self) -> &DisplayStats {
        &self.stats
    }

    /// Get pixel states buffer (for advanced usage)
    pub fn pixel_states(&self) -> &PixelStateBuffer {
        &self.pixel_states
    }

    /// Quantize a buffer based on waveform mode
    ///
    /// Converts EinkColor buffer to waveform-quantized values
    fn quantize_buffer(&self, buffer: &[EinkColor], mode: WaveformMode) -> Vec<Gray4> {
        buffer
            .iter()
            .map(|pixel| {
                // Extract grayscale value from EinkColor
                let gray4 = match pixel {
                    EinkColor::Gray(g) => *g,
                    EinkColor::Spectra6 { bw, .. } => *bw,
                    EinkColor::Kaleido3 { r, g, b } => {
                        // Convert RGB to grayscale (luma calculation)
                        let luma = ((*r as u16) + (*g as u16) + (*b as u16)) / 3;
                        Gray4::new((luma / 4).min(3) as u8)
                    }
                };

                // Gray4 luma is 0-3, convert to 0-15 range for quantization
                let value_15 = gray4.luma() * 5; // 0,1,2,3 → 0,5,10,15

                // Apply waveform-specific quantization in 0-15 range
                let quantized_15 = mode.quantize_gray4(value_15);

                // Convert back to 0-3 range for Gray4
                // Note: quantized_15 is always a multiple of 5 (0, 5, 10, 15)
                // so division by 5 is exact
                Gray4::new(quantized_15 / 5)
            })
            .collect()
    }

    /// Present solid color frame (for flashing)
    #[cfg(not(feature = "headless"))]
    async fn present_solid_color(&mut self, color: u32) {
        if let Some(window) = &mut self.window {
            let frame = vec![color; (self.spec.width * self.spec.height) as usize];
            window.present(&frame);
        }
    }

    /// Present frame with RGBA data
    #[cfg(not(feature = "headless"))]
    async fn present_frame(&mut self, rgba: &[u32]) {
        if let Some(window) = &mut self.window {
            window.present(rgba);
        }
    }

    /// Render with flash animations based on waveform mode
    async fn render_with_flashes(
        &mut self,
        mode: WaveformMode,
        framebuffer: &[EinkColor],
    ) -> Result<(), std::io::Error> {
        use tokio::time::{sleep, Duration};

        let base_duration = mode.base_duration_ms();
        let adjusted = self
            .spec
            .adjusted_refresh_ms(base_duration, self.current_temp);
        let flash_count = mode.flash_count();

        if flash_count > 0 {
            let flash_duration = adjusted / (flash_count as u32 * 3);

            for _ in 0..flash_count {
                // Flash black
                #[cfg(not(feature = "headless"))]
                self.present_solid_color(0xFF000000).await;

                sleep(Duration::from_millis(flash_duration as u64)).await;

                // Flash white
                #[cfg(not(feature = "headless"))]
                self.present_solid_color(0xFFFFFFFF).await;

                sleep(Duration::from_millis(flash_duration as u64)).await;
            }
        }

        // Present final image
        let mut rgba = framebuffer_to_rgba(framebuffer);

        // Render debug overlays (feature-gated)
        #[cfg(feature = "debug")]
        if let Some(ref debug_manager) = self.debug_manager {
            self.render_debug_overlays(&mut rgba, debug_manager);
        }

        #[cfg(not(feature = "headless"))]
        self.present_frame(&rgba).await;

        sleep(Duration::from_millis((adjusted / 3) as u64)).await;

        Ok(())
    }

    /// Run window event loop (blocks until window closed)
    #[cfg(not(feature = "headless"))]
    pub fn run(mut self) {
        if let Some(mut window) = self.window {
            // Transfer debug_manager to window for keyboard event handling
            #[cfg(feature = "debug")]
            if let Some(debug_manager) = self.debug_manager.take() {
                window.set_debug_manager(debug_manager);
            }

            window.run();
        }
    }

    #[cfg(feature = "headless")]
    pub fn run(self) {
        // No-op in headless mode
    }

    /// Save screenshot to PNG (for testing)
    ///
    /// Per embedded-graphics-simulator pattern: use for
    /// automated testing and visual regression.
    pub fn screenshot(
        &self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use image::{GrayImage, Luma};

        let mut img = GrayImage::new(self.framebuffer.width, self.framebuffer.height);

        for (i, pixel) in self.framebuffer.pixels.iter().enumerate() {
            let x = (i as u32) % self.framebuffer.width;
            let y = (i as u32) / self.framebuffer.width;
            // Convert EinkColor to grayscale for screenshot
            let gray = match pixel {
                EinkColor::Gray(g) => (g.luma() as u32) * 85, // 0-3 → 0, 85, 170, 255
                EinkColor::Spectra6 { bw, .. } => (bw.luma() as u32) * 85,
                EinkColor::Kaleido3 { r, g, b } => {
                    // RGB to grayscale luma
                    let luma = ((*r as u32) + (*g as u32) + (*b as u32)) / 3;
                    luma * 17 // 0-15 → 0-255
                }
            };
            img.put_pixel(x, y, Luma([gray as u8]));
        }

        img.save(path)?;
        Ok(())
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
                self.framebuffer
                    .set_pixel(point.x as u32, point.y as u32, EinkColor::Gray(color));
            }
        }
        Ok(())
    }
}

impl OriginDimensions for Emulator {
    fn size(&self) -> Size {
        // Framebuffer is already in the correct orientation
        Size::new(self.framebuffer.width, self.framebuffer.height)
    }
}

impl DisplayDriver for Emulator {
    type DriverError = std::io::Error;

    fn spec(&self) -> &'static eink_specs::DisplaySpec {
        self.spec
    }

    async fn update_buffer(&mut self) -> Result<(), Self::DriverError> {
        // Transition to buffer transfer state
        self.power_tracker
            .transition_to(PowerState::TransferringBuffer);

        // Copy framebuffer to staged buffer (simulates SPI transfer to controller SRAM)
        self.staged_buffer.copy_from_slice(&self.framebuffer.pixels);

        // Return to idle after transfer
        self.power_tracker.transition_to(PowerState::Idle);

        // Update window title with power stats
        #[cfg(not(feature = "headless"))]
        if let Some(window) = &mut self.window {
            window.set_power_stats(self.power_tracker.stats());
        }

        Ok(())
    }

    async fn display(&mut self) -> Result<(), Self::DriverError> {
        // Display using the staged buffer with the current waveform mode
        self.display_with_staged_buffer(self.waveform_mode).await
    }

    async fn display_with_mode(&mut self, mode: WaveformMode) -> Result<(), Self::DriverError> {
        // Display using the staged buffer with explicit waveform mode
        self.display_with_staged_buffer(mode).await
    }

    async fn refresh_full(&mut self) -> Result<(), Self::DriverError> {
        // Convenience method: update_buffer + display with GC16
        self.update_buffer().await?;
        self.display_with_mode(WaveformMode::GC16).await
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::DriverError> {
        // Convenience method: update_buffer + display with DU4
        self.update_buffer().await?;
        self.display_with_mode(WaveformMode::DU4).await
    }

    async fn refresh_fast(&mut self) -> Result<(), Self::DriverError> {
        // Convenience method: update_buffer + display with DU
        self.update_buffer().await?;
        self.display_with_mode(WaveformMode::DU).await
    }

    async fn sleep(&mut self) -> Result<(), Self::DriverError> {
        // Transition to sleep state
        self.power_tracker.transition_to(PowerState::Sleeping);

        // Update window title with power stats
        #[cfg(not(feature = "headless"))]
        if let Some(window) = &mut self.window {
            window.set_power_stats(self.power_tracker.stats());
        }

        Ok(())
    }

    async fn wake(&mut self) -> Result<(), Self::DriverError> {
        // Transition to idle state
        self.power_tracker.transition_to(PowerState::Idle);

        // Update window title with power stats
        #[cfg(not(feature = "headless"))]
        if let Some(window) = &mut self.window {
            window.set_power_stats(self.power_tracker.stats());
        }

        Ok(())
    }
}

impl Emulator {
    /// Display using staged buffer with specific waveform mode (robust physics simulation)
    ///
    /// This is the internal method that performs the actual refresh using the staged buffer.
    /// It uses the staged buffer (not the framebuffer) for the refresh, matching real hardware behavior.
    async fn display_with_staged_buffer(
        &mut self,
        mode: WaveformMode,
    ) -> Result<(), std::io::Error> {
        // 0. Check initialization requirement
        if self.requires_init && !self.init_state().is_ready() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Display not initialized. Call initialize() first.",
            ));
        }

        // Transition to refreshing state with appropriate flash count
        let flash_count = mode.flash_count();
        self.power_tracker
            .transition_to(PowerState::Refreshing { flash_count });

        // 1. Quantize staged buffer based on waveform mode
        let quantized = self.quantize_buffer(&self.staged_buffer, mode);

        // 2. Update pixel states with physics (including temperature effects)
        match mode {
            WaveformMode::GC16 | WaveformMode::GL16 | WaveformMode::GCC16 => {
                self.pixel_states.full_refresh_all(&quantized);
            }
            WaveformMode::DU4 => {
                let rate = mode.ghosting_rate();
                self.pixel_states
                    .partial_refresh_all(&quantized, rate, self.current_temp);
            }
            WaveformMode::DU | WaveformMode::A2 | WaveformMode::GCU => {
                let rate = mode.ghosting_rate();
                self.pixel_states
                    .fast_refresh_all(&quantized, rate, self.current_temp);
            }
        }

        // 3. Check DC balance and warn
        let max_dc = self.pixel_states.max_dc_balance();
        if max_dc > 50.0 {
            eprintln!(
                "⚠️  DC balance critical ({:.1})! Full refresh required.",
                max_dc
            );
            self.stats.dc_warnings += 1;
        } else if max_dc > 30.0 {
            eprintln!(
                "⚠️  DC balance warning ({:.1}). Consider full refresh soon.",
                max_dc
            );
            self.stats.dc_warnings += 1;
        }

        // 4. Get effective framebuffer with ghosting
        let effective_fb = self.pixel_states.effective_framebuffer();

        // Convert Gray4 to EinkColor for rendering
        let effective_fb_eink: Vec<EinkColor> =
            effective_fb.iter().map(|g| EinkColor::Gray(*g)).collect();

        // 5. Render with flash animation
        let base_duration = mode.base_duration_ms();
        self.render_with_flashes(mode, &effective_fb_eink).await?;

        // 6. Update statistics
        self.stats.record_refresh(mode, base_duration);

        // Return to idle after refresh
        self.power_tracker.transition_to(PowerState::Idle);

        // Update window title with power stats
        #[cfg(not(feature = "headless"))]
        if let Some(window) = &mut self.window {
            window.set_power_stats(self.power_tracker.stats());
        }

        Ok(())
    }

    /// Refresh with specific waveform mode (robust physics simulation)
    ///
    /// Convenience method for backward compatibility.
    /// This combines update_buffer() + display_with_staged_buffer().
    pub async fn refresh_with_waveform(
        &mut self,
        mode: WaveformMode,
    ) -> Result<(), std::io::Error> {
        // Update buffer then display (bypasses trait to avoid recursion)
        self.update_buffer().await?;
        self.display_with_staged_buffer(mode).await
    }

    /// Mark a rectangular region as dirty for partial refresh
    pub fn mark_dirty(&mut self, rect: embedded_graphics::primitives::Rectangle) {
        self.dirty_regions.push(rect);
    }

    /// Clear all dirty regions
    pub fn clear_dirty(&mut self) {
        self.dirty_regions.clear();
    }

    /// Get the bounding box of all dirty regions
    pub fn get_dirty_bounds(&self) -> Option<embedded_graphics::primitives::Rectangle> {
        partial_window::merge_rectangles(&self.dirty_regions)
    }

    /// Enable or disable auto-dirty tracking
    pub fn enable_auto_dirty_tracking(&mut self, enable: bool) {
        self.auto_track_dirty = enable;
    }

    /// Refresh a specific partial window
    pub async fn refresh_partial_window(
        &mut self,
        window: embedded_graphics::primitives::Rectangle,
    ) -> Result<(), std::io::Error> {
        let partial_window = PartialWindow::new(window);
        self.refresh_partial().await?;
        if partial_window.was_aligned {
            eprintln!(
                "Partial window aligned: {:?} -> {:?}",
                window, partial_window.aligned_rect
            );
        }
        Ok(())
    }

    /// Get all dirty regions
    pub fn dirty_regions(&self) -> &[embedded_graphics::primitives::Rectangle] {
        &self.dirty_regions
    }

    /// Check if auto-dirty tracking is enabled
    pub fn is_auto_dirty_tracking_enabled(&self) -> bool {
        self.auto_track_dirty
    }

    /// Check if a specific operation would trigger any hardware quirks
    ///
    /// Returns Ok(()) if no quirks triggered, Err(message) if a quirk is triggered.
    /// Only checks if quirks_enabled is true.
    pub fn check_quirks(&mut self, operation: &str) -> Result<(), String> {
        if !self.quirks_enabled {
            return Ok(());
        }

        // Get quirks from display spec
        let quirks = match self.spec.quirks {
            Some(quirks) => quirks,
            None => return Ok(()), // No quirks defined
        };

        // Check each quirk for triggering conditions
        for quirk in quirks {
            use eink_specs::Quirk;

            match quirk {
                Quirk::RotationGlitch { description } if operation.contains("rotation") => {
                    self.active_quirk = Some(description.to_string());

                    // Update window title if available
                    #[cfg(not(feature = "headless"))]
                    if let Some(window) = &mut self.window {
                        window.set_quirk_warning(Some(description));
                    }

                    return Err(format!("⚠️  QUIRK TRIGGERED: {}", description));
                }
                Quirk::SpiWriteHang { description }
                    if operation.contains("spi_write") || operation.contains("init") =>
                {
                    self.active_quirk = Some(description.to_string());

                    #[cfg(not(feature = "headless"))]
                    if let Some(window) = &mut self.window {
                        window.set_quirk_warning(Some(description));
                    }

                    return Err(format!("⚠️  QUIRK TRIGGERED: {}", description));
                }
                Quirk::UncontrollableRefreshRate { description }
                    if operation.contains("refresh") =>
                {
                    // This quirk is a warning, not an error - just log it
                    eprintln!("⚠️  Hardware Quirk: {}", description);
                    self.active_quirk = Some(description.to_string());

                    #[cfg(not(feature = "headless"))]
                    if let Some(window) = &mut self.window {
                        window.set_quirk_warning(Some(description));
                    }
                    // Don't return error, just warn
                }
                Quirk::PanelSpecific { description }
                    if operation.contains("init") || operation.contains("vcom") =>
                {
                    eprintln!("⚠️  Hardware Quirk: {}", description);
                    self.active_quirk = Some(description.to_string());

                    #[cfg(not(feature = "headless"))]
                    if let Some(window) = &mut self.window {
                        window.set_quirk_warning(Some(description));
                    }
                }
                Quirk::LimitedLibrarySupport { description } if operation.contains("init") => {
                    eprintln!("ℹ️  Note: {}", description);
                    self.active_quirk = Some(description.to_string());
                }
                _ => {}
            }
        }

        Ok(())
    }

    /// Disable hardware quirks simulation
    ///
    /// Useful for testing idealized behavior without controller-specific bugs.
    pub fn disable_quirks(&mut self) {
        self.quirks_enabled = false;
        self.active_quirk = None;

        #[cfg(not(feature = "headless"))]
        if let Some(window) = &mut self.window {
            window.set_quirk_warning(None);
        }
    }

    /// Enable hardware quirks simulation (enabled by default)
    pub fn enable_quirks(&mut self) {
        self.quirks_enabled = true;
    }

    /// Get currently active quirk (if any)
    pub fn active_quirk(&self) -> Option<&str> {
        self.active_quirk.as_deref()
    }

    /// Check if quirks are enabled
    pub fn quirks_enabled(&self) -> bool {
        self.quirks_enabled
    }

    /// Get debug manager reference
    ///
    /// Returns `None` in headless mode or when debug feature is disabled.
    ///
    /// # Example
    /// ```no_run
    /// # use eink_emulator::Emulator;
    /// let mut emulator = Emulator::new(250, 122);
    /// #[cfg(feature = "debug")]
    /// if let Some(debug_manager) = emulator.debug_manager() {
    ///     println!("Panel visible: {}", debug_manager.state().panel_visible);
    /// }
    /// ```
    #[cfg(feature = "debug")]
    pub fn debug_manager(&self) -> Option<&debug::DebugManager> {
        self.debug_manager.as_ref()
    }

    /// Get mutable debug manager reference
    ///
    /// Returns `None` in headless mode or when debug feature is disabled.
    ///
    /// # Example
    /// ```no_run
    /// # use eink_emulator::Emulator;
    /// let mut emulator = Emulator::new(250, 122);
    /// #[cfg(feature = "debug")]
    /// if let Some(debug_manager) = emulator.debug_manager_mut() {
    ///     debug_manager.state_mut().toggle_panel();
    /// }
    /// ```
    #[cfg(feature = "debug")]
    pub fn debug_manager_mut(&mut self) -> Option<&mut debug::DebugManager> {
        self.debug_manager.as_mut()
    }

    /// Render debug overlays onto the RGBA buffer
    ///
    /// This renders borders, panel, and power graph based on debug state.
    #[cfg(feature = "debug")]
    fn render_debug_overlays(&self, rgba: &mut [u32], debug_manager: &debug::DebugManager) {
        let width = self.framebuffer.width;
        let height = self.framebuffer.height;
        let state = debug_manager.state();

        // Render component borders if enabled
        if state.borders_enabled {
            // Collect component info from debug metadata
            // For now, we'll just render the overlay if there are any registered components
            let components = vec![]; // TODO: Get actual component list from metadata
            debug::OverlayRenderer::new().render_borders(rgba, width, height, &components);
        }

        // Render power graph if enabled
        if state.power_graph_enabled {
            debug_manager.power_graph().render(rgba, width, 10, 400);
        }

        // Render debug panel last (on top of everything)
        if state.panel_visible {
            debug::DebugPanel::new().render(rgba, width, height, state);
        }
    }
}

impl EinkDisplay for Emulator {
    fn refresh_mode(&self) -> RefreshMode {
        self.refresh_mode
    }

    fn set_refresh_mode(&mut self, mode: RefreshMode) {
        self.refresh_mode = mode;
    }

    fn temperature(&self) -> Option<i8> {
        Some(self.current_temp)
    }

    fn ghosting_level(&self) -> Option<f32> {
        Some(self.pixel_states.average_ghosting())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_emulator_creation() {
        let emulator = Emulator::headless(250, 122);
        assert_eq!(emulator.framebuffer.width, 250);
        assert_eq!(emulator.framebuffer.height, 122);
    }

    #[test]
    fn test_draw_target() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let mut emulator = Emulator::headless(100, 100);

        Rectangle::new(Point::new(10, 10), Size::new(20, 20))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();

        // Check that pixels were drawn
        assert_eq!(
            emulator.framebuffer.get_pixel(15, 15),
            Some(EinkColor::Gray(Gray4::BLACK))
        );
        assert_eq!(
            emulator.framebuffer.get_pixel(0, 0),
            Some(EinkColor::Gray(Gray4::WHITE))
        );
    }

    #[tokio::test]
    async fn test_refresh_full() {
        let mut emulator = Emulator::headless(100, 100);
        emulator.refresh_full().await.unwrap();
    }

    #[tokio::test]
    async fn test_refresh_partial() {
        let mut emulator = Emulator::headless(100, 100);
        emulator.refresh_partial().await.unwrap();
    }

    #[tokio::test]
    async fn test_ghosting_accumulation() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let mut emulator = Emulator::headless(100, 100);

        // Partial refreshes with content changes should accumulate ghosting
        for i in 0..3 {
            // Alternate between drawing and clearing to create transitions
            let color = if i % 2 == 0 {
                Gray4::BLACK
            } else {
                Gray4::WHITE
            };
            Rectangle::new(Point::new(10, 10), Size::new(50, 50))
                .into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut emulator)
                .unwrap();

            emulator.refresh_partial().await.unwrap();
        }

        // With content-dependent ghosting, transitions should accumulate some ghosting
        assert!(emulator.ghosting_level() > 0.0);

        // Full refresh should clear ghosting
        emulator.refresh_full().await.unwrap();
        assert_eq!(emulator.ghosting_level(), 0.0);
    }

    #[tokio::test]
    async fn test_temperature_adjustment() {
        let mut emulator = Emulator::headless(100, 100);

        emulator.set_temperature(-5);
        assert_eq!(emulator.temperature(), Some(-5));

        // Temperature affects timing (tested through spec)
        // -5°C: factor = 1.5 + (0 - (-5)) * 0.05 = 1.5 + 0.25 = 1.75
        assert_eq!(
            emulator.spec().adjusted_refresh_ms(2000, -5),
            3500 // 1.75x slower at -5°C
        );
    }

    #[test]
    fn test_eink_display_trait() {
        let mut emulator = Emulator::headless(100, 100);

        assert_eq!(emulator.refresh_mode(), RefreshMode::Full);

        emulator.set_refresh_mode(RefreshMode::Partial);
        assert_eq!(emulator.refresh_mode(), RefreshMode::Partial);

        assert_eq!(emulator.temperature(), Some(25));

        // ghosting_level() is a method on Emulator, not from EinkDisplay trait
        assert_eq!(emulator.ghosting_level(), 0.0);

        // EinkDisplay::ghosting_level() returns Option
        use crate::EinkDisplay;
        assert_eq!(EinkDisplay::ghosting_level(&emulator), Some(0.0));
    }

    // ============================================================================
    // ROBUST PHYSICS E2E TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_content_dependent_ghosting() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        // Test using direct pixel state updates for clarity
        let mut pixel_small = PixelState::new();
        let mut pixel_large = PixelState::new();

        // Small transition: 0 → 5 (in 0-15 range)
        pixel_small.partial_refresh(5, 0.15, 25);
        let ghosting_small = pixel_small.ghosting;

        // Large transition: 0 → 15 (in 0-15 range)
        pixel_large.partial_refresh(15, 0.15, 25);
        let ghosting_large = pixel_large.ghosting;

        println!("Small transition (0→5) ghosting: {}", ghosting_small);
        println!("Large transition (0→15) ghosting: {}", ghosting_large);

        // Larger transitions should produce more ghosting
        assert!(
            ghosting_large > ghosting_small * 1.5,
            "Large transition ({}) should have >50% more ghosting than small transition ({})",
            ghosting_large,
            ghosting_small
        );
    }

    #[tokio::test]
    async fn test_waveform_quantization() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let mut emulator = Emulator::headless(16, 16);

        // Draw all 4 grayscale levels
        for i in 0..4 {
            Rectangle::new(Point::new(i as i32 * 4, 0), Size::new(4, 16))
                .into_styled(PrimitiveStyle::with_fill(Gray4::new(i)))
                .draw(&mut emulator)
                .unwrap();
        }

        // Test GC16: No quantization (supports full 4 levels in Gray4)
        emulator
            .refresh_with_waveform(WaveformMode::GC16)
            .await
            .unwrap();
        let effective = emulator.pixel_states.effective_framebuffer();
        // Verify we have different levels
        let unique_levels: std::collections::HashSet<_> =
            effective.iter().map(|c| c.luma()).collect();
        assert!(unique_levels.len() >= 2); // At least 2 distinct levels

        // Test DU: Binary quantization (only 0 and 3 in Gray4)
        emulator
            .refresh_with_waveform(WaveformMode::DU)
            .await
            .unwrap();
        let effective = emulator.pixel_states.effective_framebuffer();
        // After DU quantization, should only have extremes
        let unique_levels: std::collections::HashSet<_> =
            effective.iter().map(|c| c.luma()).collect();
        assert!(unique_levels.len() <= 2); // At most 2 levels (black and white)
    }

    #[tokio::test]
    async fn test_dc_balance_warnings() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let mut emulator = Emulator::headless(50, 50);

        // Many partial refreshes with large transitions
        for i in 0..15 {
            let color = if i % 2 == 0 {
                Gray4::BLACK
            } else {
                Gray4::WHITE
            };
            Rectangle::new(Point::new(0, 0), Size::new(50, 50))
                .into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut emulator)
                .unwrap();

            emulator
                .refresh_with_waveform(WaveformMode::DU4)
                .await
                .unwrap();
        }

        // DC balance should have accumulated
        let dc_balance = emulator.pixel_states.max_dc_balance();
        println!("DC balance after 15 partial refreshes: {}", dc_balance);
        println!("DC warnings issued: {}", emulator.stats().dc_warnings);
        assert!(
            dc_balance > 10.0,
            "DC balance ({}) should accumulate after many partial refreshes",
            dc_balance
        );

        // With 15 refreshes and DC balance of ~21, we may or may not have warnings
        // (warnings trigger at >30). The important thing is DC accumulates.
        // To guarantee warnings, do more refreshes
        for i in 0..10 {
            let color = if i % 2 == 0 {
                Gray4::BLACK
            } else {
                Gray4::WHITE
            };
            Rectangle::new(Point::new(0, 0), Size::new(50, 50))
                .into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut emulator)
                .unwrap();

            emulator
                .refresh_with_waveform(WaveformMode::DU4)
                .await
                .unwrap();
        }

        // Now warnings should definitely have been issued
        assert!(
            emulator.stats().dc_warnings > 0,
            "DC warnings should be issued after many refreshes"
        );

        // Full refresh should reduce DC balance
        emulator
            .refresh_with_waveform(WaveformMode::GC16)
            .await
            .unwrap();
        let dc_after = emulator.pixel_states.max_dc_balance();
        assert!(
            dc_after < dc_balance * 0.2,
            "Full refresh should reduce DC balance by >80%"
        );
    }

    #[tokio::test]
    async fn test_realistic_ereader_usage() {
        use embedded_graphics::mono_font::{ascii::FONT_6X10, MonoTextStyle};
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
        use embedded_graphics::text::{Alignment, Text};

        let mut emulator = Emulator::headless(250, 122);

        // Page 1: Initial render with GC16 (high quality)
        Text::new(
            "Page 1",
            Point::new(10, 20),
            MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
        )
        .draw(&mut emulator)
        .unwrap();
        emulator
            .refresh_with_waveform(WaveformMode::GC16)
            .await
            .unwrap();
        assert_eq!(emulator.ghosting_level(), 0.0);

        // Pages 2-5: Fast page turns with DU
        // Draw alternating patterns to ensure visible transitions
        for page in 2..=5 {
            // Alternate between black and white blocks to create transitions
            let fill_color = if page % 2 == 0 {
                Gray4::BLACK
            } else {
                Gray4::WHITE
            };
            Rectangle::new(Point::new(50, 30), Size::new(150, 60))
                .into_styled(PrimitiveStyle::with_fill(fill_color))
                .draw(&mut emulator)
                .unwrap();

            // Add page number text
            let text = format!("Page {}", page);
            Text::new(
                &text,
                Point::new(10, 20),
                MonoTextStyle::new(&FONT_6X10, Gray4::BLACK),
            )
            .draw(&mut emulator)
            .unwrap();

            emulator
                .refresh_with_waveform(WaveformMode::DU)
                .await
                .unwrap();
        }

        // Ghosting should have accumulated (reduced threshold since average includes unchanged pixels)
        let ghosting = emulator.ghosting_level();
        let max_ghosting = emulator.pixel_states.max_ghosting();
        println!("Average ghosting after 4 DU page turns: {}", ghosting);
        println!("Max ghosting: {}", max_ghosting);
        assert!(
            ghosting > 0.01 && ghosting < 1.0,
            "Ghosting ({}) should accumulate but not max out",
            ghosting
        );

        // Periodic cleanup with GC16
        emulator
            .refresh_with_waveform(WaveformMode::GC16)
            .await
            .unwrap();
        assert_eq!(
            emulator.ghosting_level(),
            0.0,
            "GC16 should clear all ghosting"
        );

        // Check statistics
        let stats = emulator.stats();
        assert!(stats.full_refresh_count >= 2); // Initial + cleanup
        assert!(stats.fast_refresh_count >= 4); // Page turns
        assert!(stats.total_refresh_time_ms > 0);
    }

    #[tokio::test]
    async fn test_waveform_mode_switching() {
        let mut emulator = Emulator::headless(100, 100);

        // Start with GC16
        emulator.set_waveform_mode(WaveformMode::GC16);
        assert_eq!(emulator.waveform_mode(), WaveformMode::GC16);

        // Switch to DU4
        emulator.set_waveform_mode(WaveformMode::DU4);
        assert_eq!(emulator.waveform_mode(), WaveformMode::DU4);

        // Refresh with different modes
        emulator
            .refresh_with_waveform(WaveformMode::GC16)
            .await
            .unwrap();
        emulator
            .refresh_with_waveform(WaveformMode::DU4)
            .await
            .unwrap();
        emulator
            .refresh_with_waveform(WaveformMode::A2)
            .await
            .unwrap();

        // All should succeed without errors
    }

    #[test]
    fn test_display_stats() {
        let emulator = Emulator::headless(100, 100);
        let stats = emulator.stats();

        assert_eq!(stats.full_refresh_count, 0);
        assert_eq!(stats.partial_refresh_count, 0);
        assert_eq!(stats.fast_refresh_count, 0);
        assert_eq!(stats.total_refresh_time_ms, 0);
        assert_eq!(stats.dc_warnings, 0);
    }

    // ============================================================================
    // INITIALIZATION SEQUENCE TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_successful_initialization() {
        let mut emulator = Emulator::headless(100, 100);

        // Initially uninitialized
        assert_eq!(emulator.init_state(), &InitializationState::Uninitialized);

        // Perform initialization
        emulator.initialize().await.unwrap();

        // Should be initialized
        assert_eq!(emulator.init_state(), &InitializationState::Initialized);
        assert!(emulator.init_state().is_ready());
    }

    #[tokio::test]
    async fn test_initialization_timeout() {
        let mut emulator = Emulator::headless(100, 100);

        // Set very short timeout (1ms - impossible to complete)
        emulator.set_init_timeout(1);

        // Initialization should fail with timeout
        let result = emulator.initialize().await;
        assert!(result.is_err());

        // State should be Failed
        assert!(emulator.init_state().is_failed());
    }

    #[tokio::test]
    async fn test_cannot_refresh_before_initialize() {
        let mut emulator = Emulator::headless(100, 100);

        // Enable initialization requirement
        emulator.set_requires_initialization(true);

        // Attempt to refresh without initialization
        let result = emulator.refresh_full().await;
        assert!(result.is_err());

        // After initialization, refresh should work
        emulator.initialize().await.unwrap();
        let result = emulator.refresh_full().await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_can_reinitialize_after_failure() {
        let mut emulator = Emulator::headless(100, 100);

        // Set short timeout to cause failure
        emulator.set_init_timeout(1);
        let result = emulator.initialize().await;
        assert!(result.is_err());
        assert!(emulator.init_state().is_failed());

        // Reset timeout and reinitialize
        emulator.set_init_timeout(5000);
        emulator.initialize().await.unwrap();
        assert!(emulator.init_state().is_ready());
    }

    #[tokio::test]
    async fn test_initialization_visual_checkerboard() {
        let mut emulator = Emulator::headless(100, 100);

        // Initialize (includes checkerboard step)
        emulator.initialize().await.unwrap();

        // After initialization, framebuffer should be white (step 7 clears)
        // Check a few pixels
        assert_eq!(
            emulator.framebuffer.get_pixel(0, 0),
            Some(EinkColor::Gray(Gray4::WHITE))
        );
        assert_eq!(
            emulator.framebuffer.get_pixel(50, 50),
            Some(EinkColor::Gray(Gray4::WHITE))
        );
    }

    #[tokio::test]
    async fn test_initialization_state_transitions() {
        let mut emulator = Emulator::headless(100, 100);

        // Start: Uninitialized
        assert_eq!(emulator.init_state(), &InitializationState::Uninitialized);
        assert!(!emulator.init_state().is_ready());
        assert!(!emulator.init_state().is_initializing());

        // During initialization, state should be Initializing
        // (We can't easily test this without mocking, but we can verify the end state)

        // After initialization: Initialized
        emulator.initialize().await.unwrap();
        assert_eq!(emulator.init_state(), &InitializationState::Initialized);
        assert!(emulator.init_state().is_ready());
        assert!(!emulator.init_state().is_initializing());
    }

    #[tokio::test]
    async fn test_backward_compatibility_no_init_required() {
        let mut emulator = Emulator::headless(100, 100);

        // By default, initialization is NOT required (backward compatibility)
        assert!(!emulator.requires_init);

        // Should be able to refresh without initialization
        let result = emulator.refresh_full().await;
        assert!(result.is_ok());

        // State should still be Uninitialized
        assert_eq!(emulator.init_state(), &InitializationState::Uninitialized);
    }

    #[tokio::test]
    async fn test_initialization_timing() {
        use tokio::time::Instant;

        let mut emulator = Emulator::headless(100, 100);

        let start = Instant::now();
        emulator.initialize().await.unwrap();
        let elapsed = start.elapsed();

        // Total expected time: 100+50+20+200+100+500+1000 = 1970ms
        // Allow some tolerance for timing
        assert!(
            elapsed.as_millis() >= 1900,
            "Initialization took {}ms, expected ~1970ms",
            elapsed.as_millis()
        );
        assert!(
            elapsed.as_millis() < 2500,
            "Initialization took {}ms, expected ~1970ms",
            elapsed.as_millis()
        );
    }

    #[tokio::test]
    async fn test_initialization_with_visual_window() {
        // This test verifies that visual initialization works in non-headless mode
        // In headless mode, the visual steps are no-ops, so this mainly tests the flow
        let mut emulator = Emulator::headless(100, 100);

        emulator.initialize().await.unwrap();

        // After initialization, display should be cleared to white
        for y in 0..emulator.framebuffer.height {
            for x in 0..emulator.framebuffer.width {
                assert_eq!(
                    emulator.framebuffer.get_pixel(x, y),
                    Some(EinkColor::Gray(Gray4::WHITE)),
                    "Pixel at ({}, {}) should be white after initialization",
                    x,
                    y
                );
            }
        }
    }

    // ============================================================================
    // BUFFER SEPARATION TESTS (Priority 1)
    // ============================================================================

    #[tokio::test]
    async fn test_update_buffer_copies_framebuffer() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let mut emulator = Emulator::headless(100, 100);

        // Draw a pattern to framebuffer
        Rectangle::new(Point::new(10, 10), Size::new(20, 20))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();

        // Staged buffer should still be white
        assert_eq!(emulator.staged_buffer[0], EinkColor::Gray(Gray4::WHITE));

        // Update buffer
        emulator.update_buffer().await.unwrap();

        // Now staged buffer should match framebuffer
        assert_eq!(
            emulator.staged_buffer.len(),
            emulator.framebuffer.pixels.len()
        );
        for (i, pixel) in emulator.framebuffer.pixels.iter().enumerate() {
            assert_eq!(
                emulator.staged_buffer[i], *pixel,
                "Staged buffer should match framebuffer after update_buffer()"
            );
        }
    }

    #[tokio::test]
    async fn test_display_uses_staged_buffer() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let mut emulator = Emulator::headless(100, 100);

        // Draw to framebuffer and stage it
        Rectangle::new(Point::new(10, 10), Size::new(20, 20))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
        emulator.update_buffer().await.unwrap();

        // Now modify framebuffer (clear it)
        emulator.framebuffer.clear();

        // Display should use staged buffer (black rectangle), not current framebuffer (white)
        let initial_ghosting = emulator.ghosting_level();
        emulator.display_with_mode(WaveformMode::DU4).await.unwrap();

        // Verify that pixels were updated based on staged buffer having black pixels
        // (ghosting would be different if it used the white framebuffer)
        let final_ghosting = emulator.ghosting_level();
        assert!(
            final_ghosting > initial_ghosting,
            "Display should use staged buffer (with black), causing ghosting accumulation"
        );
    }

    #[tokio::test]
    async fn test_multiple_update_buffer_calls() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let mut emulator = Emulator::headless(100, 100);
        let width = emulator.framebuffer.width as usize;

        // First update
        Rectangle::new(Point::new(10, 10), Size::new(20, 20))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();
        emulator.update_buffer().await.unwrap();

        // Second update (overwrite with different pattern)
        emulator.framebuffer.clear();
        Rectangle::new(Point::new(30, 30), Size::new(20, 20))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(2)))
            .draw(&mut emulator)
            .unwrap();
        emulator.update_buffer().await.unwrap();

        // Staged buffer should have the second pattern
        // Check a pixel from the second rectangle
        let idx = 35 * width + 35; // x=35, y=35
        assert_eq!(
            emulator.staged_buffer[idx],
            EinkColor::Gray(Gray4::new(2)),
            "Last update_buffer() should win"
        );

        // Check a pixel from the first rectangle (should be white now)
        let idx = 15 * width + 15; // x=15, y=15
        assert_eq!(
            emulator.staged_buffer[idx],
            EinkColor::Gray(Gray4::WHITE),
            "First pattern should be overwritten"
        );
    }

    #[tokio::test]
    async fn test_buffer_independence() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let mut emulator = Emulator::headless(100, 100);
        let width = emulator.framebuffer.width as usize;

        // Draw and stage a black rectangle
        Rectangle::new(Point::new(10, 10), Size::new(20, 20))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();

        let idx = 15 * width + 15;

        emulator.update_buffer().await.unwrap();

        // Modify framebuffer (shouldn't affect staged buffer)
        emulator.framebuffer.clear();
        Rectangle::new(Point::new(10, 10), Size::new(20, 20))
            .into_styled(PrimitiveStyle::with_fill(Gray4::new(3)))
            .draw(&mut emulator)
            .unwrap();

        // Staged buffer should still have black rectangle (reuse the idx from above)
        assert_eq!(
            emulator.staged_buffer[idx],
            EinkColor::Gray(Gray4::BLACK),
            "Staged buffer should be independent from framebuffer changes"
        );

        // Framebuffer should have the new value
        assert_eq!(
            emulator.framebuffer.get_pixel(15, 15),
            Some(EinkColor::Gray(Gray4::new(3))),
            "Framebuffer should have new value"
        );
    }

    #[tokio::test]
    async fn test_explicit_workflow_update_then_display() {
        use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};

        let mut emulator = Emulator::headless(100, 100);

        // Explicit workflow: draw → update_buffer → display_with_mode
        Rectangle::new(Point::new(10, 10), Size::new(50, 50))
            .into_styled(PrimitiveStyle::with_fill(Gray4::BLACK))
            .draw(&mut emulator)
            .unwrap();

        // Step 1: Update buffer
        emulator.update_buffer().await.unwrap();

        // Step 2: Display with explicit waveform
        emulator.display_with_mode(WaveformMode::DU4).await.unwrap();

        // Verify stats were updated
        assert_eq!(emulator.stats().partial_refresh_count, 1);

        // Verify pixel states were updated
        assert!(
            emulator.ghosting_level() > 0.0,
            "Partial refresh should accumulate some ghosting"
        );
    }

    #[tokio::test]
    async fn test_refresh_methods_still_work() {
        // Verify backward compatibility: refresh_full/partial/fast still work
        let mut emulator = Emulator::headless(100, 100);

        emulator.refresh_full().await.unwrap();
        assert_eq!(emulator.stats().full_refresh_count, 1);
        assert_eq!(emulator.ghosting_level(), 0.0);

        emulator.refresh_partial().await.unwrap();
        assert_eq!(emulator.stats().partial_refresh_count, 1);

        emulator.refresh_fast().await.unwrap();
        assert_eq!(emulator.stats().fast_refresh_count, 1);
    }

    #[tokio::test]
    async fn test_display_method_uses_current_waveform_mode() {
        let mut emulator = Emulator::headless(100, 100);

        // Set waveform mode
        emulator.set_waveform_mode(WaveformMode::GC16);

        // Update buffer and display (should use GC16)
        emulator.update_buffer().await.unwrap();
        emulator.display().await.unwrap();

        // Should have triggered full refresh
        assert_eq!(emulator.stats().full_refresh_count, 1);
        assert_eq!(emulator.ghosting_level(), 0.0);
    }

    // ============================================================================
    // DEBUG MODULE TESTS
    // ============================================================================

    #[cfg(feature = "debug")]
    #[test]
    fn test_debug_state_creation() {
        use crate::debug::DebugState;

        let state = DebugState::new();
        assert!(!state.panel_visible);
        assert!(!state.borders_enabled);
        assert!(!state.inspector_mode);
        assert!(!state.power_graph_enabled);
        assert!(state.hovered_component.is_none());
        assert!(state.selected_component.is_none());
        assert_eq!(state.power_history.len(), 0);
    }

    #[cfg(feature = "debug")]
    #[test]
    fn test_debug_state_toggles() {
        use crate::debug::DebugState;

        let mut state = DebugState::new();

        // Test panel toggle
        assert!(!state.panel_visible);
        state.toggle_panel();
        assert!(state.panel_visible);
        state.toggle_panel();
        assert!(!state.panel_visible);

        // Test borders toggle
        assert!(!state.borders_enabled);
        state.toggle_borders();
        assert!(state.borders_enabled);

        // Test inspector toggle
        assert!(!state.inspector_mode);
        state.toggle_inspector();
        assert!(state.inspector_mode);

        // Test power graph toggle
        assert!(!state.power_graph_enabled);
        state.toggle_power_graph();
        assert!(state.power_graph_enabled);
    }

    #[cfg(feature = "debug")]
    #[test]
    fn test_component_info_creation() {
        use crate::debug::ComponentInfo;

        let info = ComponentInfo {
            component_type: "Button".to_string(),
            position: (10, 20),
            size: (100, 40),
            test_id: Some("play-button".to_string()),
        };

        assert_eq!(info.component_type, "Button");
        assert_eq!(info.position, (10, 20));
        assert_eq!(info.size, (100, 40));
        assert_eq!(info.test_id, Some("play-button".to_string()));
    }

    #[cfg(feature = "debug")]
    #[test]
    fn test_refresh_type() {
        use crate::debug::RefreshType;

        assert_ne!(RefreshType::Full, RefreshType::Partial);
        assert_ne!(RefreshType::Full, RefreshType::Fast);
        assert_eq!(RefreshType::Full, RefreshType::Full);
    }
}
