//! DAP Display Emulator
//!
//! Blank canvas for developing the DAP UI.
//! Run with: cargo run --example display_emulator --features emulator
//!
//! # Hot-Reload Mode
//!
//! For true in-process hot-reload (window stays open on code changes):
//!
//! Step 1: cargo build --package firmware-ui --features hot-reload
//! Step 2: cargo run --example display_emulator --features emulator,hot-reload
//!
//! Rendering code lives in: crates/firmware-ui/src/render.rs
//! Edit that file, save, and the emulator reloads without restart.
//!
//! # Kill-and-Restart Mode (default: xtask dev)
//!
//! Without the hot-reload feature, xtask dev uses kill-and-restart.
//! That mode works for all code changes (not just render.rs).

use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{PrimitiveStyle, Rectangle};
use tracing_subscriber::EnvFilter;

use firmware::EmulatorDisplay;
use platform::config;

#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use firmware::input::{Button, InputEvent};
#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use firmware_ui::screens::now_playing::render_now_playing_to;
#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use ui::navigation::Navigator;
#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use ui::now_playing::NowPlayingState;
#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use ui::screen::Screen;

// The #[hot_module] attribute macro (hot-lib-reloader 0.8) generates
// a mod with hot-reloadable wrappers for the dylib functions.
// Build the dylib FIRST: cargo build --package firmware-ui --features hot-reload
//
// C ABI boundary uses eink_emulator::Emulator (not EmulatorDisplay)
// to avoid circular dep: firmware -> firmware-ui -> firmware.
// We pass display.emulator_mut() as raw pointer to the dylib.
#[cfg(feature = "hot-reload")]
#[hot_lib_reloader::hot_module(dylib = "firmware_ui")]
mod hot_ui {
    hot_functions_from_file!("crates/firmware-ui/src/lib.rs");

    #[lib_change_subscription]
    pub fn subscribe() -> hot_lib_reloader::LibReloadObserver {}
}

#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
struct AppState {
    now_playing: NowPlayingState,
    nav: Navigator,
    needs_redraw: bool,
}

#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
impl Default for AppState {
    fn default() -> Self {
        let mut now_playing = NowPlayingState::default();
        now_playing.title.push_str("Sample Track").ok();
        now_playing.artist.push_str("Sample Artist").ok();
        now_playing.set_duration_ms(180_000); // 3 minutes demo
        AppState {
            now_playing,
            nav: Navigator::new(),
            needs_redraw: true,
        }
    }
}

#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
impl AppState {
    fn handle_input(&mut self, ev: InputEvent) {
        match ev {
            InputEvent::ButtonPress(Button::Play) => {
                self.now_playing.set_playing(!self.now_playing.playing);
                self.needs_redraw = true;
            }
            InputEvent::ButtonPress(Button::VolumeUp) => {
                self.now_playing
                    .set_volume(self.now_playing.volume.saturating_add(5));
                self.needs_redraw = true;
            }
            InputEvent::ButtonPress(Button::VolumeDown) => {
                self.now_playing
                    .set_volume(self.now_playing.volume.saturating_sub(5));
                self.needs_redraw = true;
            }
            InputEvent::ButtonPress(Button::Menu) => {
                self.nav.push(Screen::LibraryBrowse);
                self.needs_redraw = true;
            }
            InputEvent::ButtonPress(Button::Back) => {
                self.nav.back();
                self.needs_redraw = true;
            }
            InputEvent::RotaryIncrement(steps) => {
                if steps > 0 {
                    let delta = steps.unsigned_abs().min(50) as u8 * 2;
                    self.now_playing
                        .set_volume(self.now_playing.volume.saturating_add(delta));
                } else {
                    let delta = steps.unsigned_abs().min(50) as u8 * 2;
                    self.now_playing
                        .set_volume(self.now_playing.volume.saturating_sub(delta));
                }
                self.needs_redraw = true;
            }
            _ => {} // ButtonRelease and unmapped events are ignored
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing subscriber. Controlled by RUST_LOG env var (default: info).
    // cargo dev sets RUST_LOG=info automatically; override with e.g. RUST_LOG=debug cargo dev.
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .with_timer(tracing_subscriber::fmt::time::uptime())
        .compact()
        .init();

    tracing::info!(
        app = config::APP_NAME,
        version = "0.1.0",
        "Display Emulator starting"
    );
    tracing::info!(
        display = "GDEM0397T81P",
        size = "3.97\"",
        resolution = "800x480",
        "Display Emulator"
    );

    let rt = tokio::runtime::Runtime::new()?;

    let emulator_config = eink_emulator::EmulatorConfig {
        rotation: eink_emulator::Rotation::Degrees90,
        scale: 1,
    };

    let mut display =
        EmulatorDisplay::with_spec_and_config(&firmware::GDEM0397T81P_SPEC, emulator_config);
    tracing::info!(mode = "portrait", resolution = "480x800", "Window opened");

    // Attach keyboard/scroll input before initializing so the queue is wired
    // through to the window event loop when run() is called.
    // Not used in hot-reload mode (which has its own render loop).
    #[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
    let mut input = display.emulator_mut().input_receiver();

    tracing::info!("Initializing display");
    rt.block_on(async { display.emulator_mut().initialize().await })
        .map_err(|e| Box::<dyn std::error::Error>::from(e.to_string()))?;

    use platform::DisplayDriver;
    tracing::info!("Display ready");

    #[cfg(feature = "debug")]
    {
        tracing::debug!(
            "Debug mode enabled — hotkeys: Ctrl+1=panel Ctrl+2=borders Ctrl+3=inspector"
        );
    }

    #[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
    {
        tracing::info!(
            bindings = "Space/K=Play ←/J=Prev →/L=Next ↑/==Vol+ ↓/-=Vol- M=Menu Esc/BS=Back Enter=Select Scroll=Encoder",
            "Keyboard input enabled"
        );
    }

    // True in-process hot-reload path.
    // hot_ui module was generated by #[hot_module] above.
    // wait_for_reload() blocks until the dylib is reloaded.
    #[cfg(feature = "hot-reload")]
    {
        use std::time::{Duration, Instant};

        tracing::info!("Hot-reload mode active");
        tracing::info!(
            path = "crates/firmware-ui/src/render.rs",
            "Edit to see changes instantly"
        );

        let mut last_version = hot_ui::ui_version();

        // ABI version guard: panic early if the loaded dylib was compiled with a
        // different ABI than what this binary expects.
        let dylib_abi = hot_ui::ui_abi_version();
        assert_eq!(
            dylib_abi,
            firmware_ui::ABI_VERSION,
            "hot-reload ABI mismatch: binary expects ABI v{}, dylib reports v{}. \
             Rebuild firmware-ui with: cargo build -p firmware-ui --features hot-reload",
            firmware_ui::ABI_VERSION,
            dylib_abi,
        );
        tracing::info!("Initial render");
        // SAFETY: display.emulator_mut() is valid, non-null, exclusively owned.
        // Unwrapping EmulatorDisplay to its inner Emulator avoids circular dep.
        // Verified: ui_abi_version() == firmware_ui::ABI_VERSION (asserted above).
        unsafe { hot_ui::render_ui(display.emulator_mut() as *mut eink_emulator::Emulator) };
        rt.block_on(async { display.refresh_full().await })?;
        tracing::info!(
            path = "crates/firmware-ui/src/render.rs",
            "Ready — edit to hot-reload"
        );

        // Subscribe to reload events and block until each reload completes.
        let observer = hot_ui::subscribe();
        loop {
            observer.wait_for_reload();
            let new_version = hot_ui::ui_version();
            if new_version != last_version {
                last_version = new_version;
                tracing::info!("Hot-reloaded firmware_ui dylib");

                let size = display.bounding_box().size;
                Rectangle::new(Point::zero(), size)
                    .into_styled(PrimitiveStyle::with_fill(Gray4::WHITE))
                    .draw(&mut display)?;
                // SAFETY: display.emulator_mut() is valid, non-null, exclusively owned.
                unsafe {
                    hot_ui::render_ui(display.emulator_mut() as *mut eink_emulator::Emulator)
                };
                rt.block_on(async { display.refresh_full().await })?;
                tracing::info!(elapsed = ?Instant::now(), "Reload complete");
            }
        }
    }

    // Standard (non-hot-reload) path.
    // xtask dev kill-and-restart mode uses this path.
    #[cfg(not(feature = "hot-reload"))]
    {
        #[cfg(not(feature = "keyboard-input"))]
        {
            // Without keyboard input: static demo menu, block in run()
            tracing::info!("Rendering demo menu");
            render_demo_menu(&mut display)?;
            rt.block_on(async { display.refresh_full().await })?;

            #[cfg(feature = "debug")]
            register_dap_components(&mut display);

            tracing::info!("Demo menu rendered");
            tracing::info!("Close window to exit");
            display.into_inner().run();
        }

        #[cfg(feature = "keyboard-input")]
        {
            tracing::info!("Starting interactive Now Playing screen");

            let mut state = AppState::default();

            rt.block_on(async {
                // Initial full render
                render_now_playing_to(&mut display, &state.now_playing, |_, _, _, _| {})?;
                display.refresh_full().await?;
                state.needs_redraw = false;

                tracing::info!("Now Playing screen ready");
                tracing::info!("Keyboard input: Space/K=Play  </J=Prev  >/L=Next  Up/==Vol+  Down/-=Vol-  M=Menu  Esc/BS=Back  Scroll=Encoder");

                loop {
                    // Pump OS events — forwards keyboard/scroll to InputQueue.
                    // Returns false when the close button is clicked.
                    if !display.emulator_mut().pump_window_events() {
                        tracing::info!("Window closed");
                        break;
                    }

                    // Drain all pending input events (non-blocking).
                    loop {
                        use platform::InputDevice as _;
                        match input.poll_event() {
                            Some(ev) => {
                                tracing::debug!(event = ?ev, "Input");
                                state.handle_input(ev);
                            }
                            None => break,
                        }
                    }

                    // Re-render only when state changed.
                    if state.needs_redraw {
                        render_now_playing_to(
                            &mut display,
                            &state.now_playing,
                            |_, _, _, _| {},
                        )?;
                        // Use partial refresh for responsiveness (~300 ms).
                        display.refresh_partial().await?;
                        state.needs_redraw = false;
                        tracing::debug!(
                            playing = state.now_playing.playing,
                            volume = state.now_playing.volume,
                            "State updated",
                        );
                    }

                    // Yield to tokio so timers / async work can run.
                    tokio::time::sleep(std::time::Duration::from_millis(16)).await;
                }

                Ok::<(), Box<dyn std::error::Error>>(())
            })?;
        }
    }

    Ok(())
}

#[cfg(all(test, feature = "keyboard-input", not(feature = "hot-reload")))]
mod tests {
    use super::*;

    fn make_state() -> AppState {
        AppState {
            now_playing: NowPlayingState::default(),
            nav: Navigator::new(),
            needs_redraw: false,
        }
    }

    #[test]
    fn play_button_toggles_playing() {
        let mut s = make_state();
        assert!(!s.now_playing.playing);
        s.handle_input(InputEvent::ButtonPress(Button::Play));
        assert!(s.now_playing.playing);
        assert!(s.needs_redraw);
    }

    #[test]
    fn play_button_twice_toggles_back() {
        let mut s = make_state();
        s.handle_input(InputEvent::ButtonPress(Button::Play));
        s.handle_input(InputEvent::ButtonPress(Button::Play));
        assert!(!s.now_playing.playing);
    }

    #[test]
    fn volume_up_increments_by_5() {
        let mut s = make_state();
        let before = s.now_playing.volume;
        s.handle_input(InputEvent::ButtonPress(Button::VolumeUp));
        assert_eq!(s.now_playing.volume, before + 5);
        assert!(s.needs_redraw);
    }

    #[test]
    fn volume_down_decrements_by_5() {
        let mut s = make_state();
        s.now_playing.set_volume(50);
        s.handle_input(InputEvent::ButtonPress(Button::VolumeDown));
        assert_eq!(s.now_playing.volume, 45);
    }

    #[test]
    fn volume_does_not_underflow() {
        let mut s = make_state();
        s.now_playing.set_volume(2);
        s.handle_input(InputEvent::ButtonPress(Button::VolumeDown));
        assert_eq!(s.now_playing.volume, 0);
    }

    #[test]
    fn volume_does_not_overflow() {
        let mut s = make_state();
        s.now_playing.set_volume(98);
        s.handle_input(InputEvent::ButtonPress(Button::VolumeUp));
        assert_eq!(s.now_playing.volume, 100);
    }

    #[test]
    fn encoder_up_increases_volume() {
        let mut s = make_state();
        s.now_playing.set_volume(50);
        s.handle_input(InputEvent::RotaryIncrement(1));
        assert!(s.now_playing.volume > 50);
        assert!(s.needs_redraw);
    }

    #[test]
    fn encoder_down_decreases_volume() {
        let mut s = make_state();
        s.now_playing.set_volume(50);
        s.handle_input(InputEvent::RotaryIncrement(-1));
        assert!(s.now_playing.volume < 50);
    }

    #[test]
    fn menu_button_pushes_library_screen() {
        let mut s = make_state();
        assert_eq!(s.nav.current(), Screen::NowPlaying);
        s.handle_input(InputEvent::ButtonPress(Button::Menu));
        assert_eq!(s.nav.current(), Screen::LibraryBrowse);
    }

    #[test]
    fn back_button_pops_nav_stack() {
        let mut s = make_state();
        s.handle_input(InputEvent::ButtonPress(Button::Menu));
        s.handle_input(InputEvent::ButtonPress(Button::Back));
        assert_eq!(s.nav.current(), Screen::NowPlaying);
    }

    #[test]
    fn button_release_events_are_ignored() {
        let mut s = make_state();
        s.handle_input(InputEvent::ButtonRelease(Button::Play));
        assert!(!s.now_playing.playing);
        assert!(!s.needs_redraw);
    }
}

/// Register named DAP scene components with the debug inspector.
///
/// Positions are in the display's native coordinate space (landscape 800×480
/// before the 90° portrait rotation applied at render time). The debug panel's
/// scene tree will show these names instead of raw coordinates.
///
/// Must be called AFTER the initial `refresh_full()` and BEFORE `run()` so
/// the registration is transferred to the window's debug manager.
#[cfg(all(not(feature = "hot-reload"), feature = "debug"))]
fn register_dap_components(display: &mut EmulatorDisplay) {
    use eink_emulator::debug::state::{ComponentInfo, Spacing};

    let Some(dm) = display.emulator_mut().debug_manager_mut() else {
        return;
    };
    let state = dm.state_mut();
    state.clear_registered_components();

    // Root screen container (full display bounds)
    state.register_component(ComponentInfo {
        component_type: "Container".to_string(),
        position: (0, 0),
        size: (800, 480),
        test_id: Some("dap-screen".to_string()),
        ..Default::default()
    });

    // Header bar (top 60px)
    state.register_component(ComponentInfo {
        component_type: "Container".to_string(),
        position: (0, 0),
        size: (800, 60),
        test_id: Some("dap-header".to_string()),
        padding: Spacing::axes(8, 20),
        ..Default::default()
    });

    // Main menu content area
    state.register_component(ComponentInfo {
        component_type: "Container".to_string(),
        position: (0, 60),
        size: (800, 380),
        test_id: Some("dap-menu".to_string()),
        padding: Spacing::all(10),
        ..Default::default()
    });

    // Menu items — match render_demo_menu layout: y = 100 + idx*50, h=45
    let menu_items = [
        "menu-now-playing",
        "menu-library",
        "menu-playlists",
        "menu-settings",
        "menu-about",
    ];
    for (idx, name) in menu_items.iter().enumerate() {
        let y = 95 + (idx as i32 * 50);
        state.register_component(ComponentInfo {
            component_type: "Button".to_string(),
            position: (10, y),
            size: (780, 45),
            test_id: Some((*name).to_string()),
            padding: Spacing::axes(8, 12),
            border: Spacing::all(1),
            attributes: vec![
                ("index".to_string(), idx.to_string()),
                ("enabled".to_string(), "true".to_string()),
            ],
            ..Default::default()
        });
    }

    // Footer label
    state.register_component(ComponentInfo {
        component_type: "Label".to_string(),
        position: (0, 440),
        size: (800, 40),
        test_id: Some("dap-footer".to_string()),
        ..Default::default()
    });
}

/// Render a simple demo menu (non-hot-reload path only).
///
/// When hot-reload is enabled this function is NOT used.
/// Edit crates/firmware-ui/src/render.rs instead.
#[cfg(not(feature = "hot-reload"))]
fn render_demo_menu<D>(display: &mut D) -> Result<(), D::Error>
where
    D: DrawTarget<Color = Gray4>,
{
    use embedded_graphics::mono_font::{ascii::FONT_10X20, MonoTextStyle};
    use embedded_graphics::text::Text;

    let size = display.bounding_box().size;

    Rectangle::new(Point::zero(), Size::new(size.width, 60))
        .into_styled(PrimitiveStyle::with_fill(Gray4::new(0x2)))
        .draw(display)?;

    let header_style = MonoTextStyle::new(&FONT_10X20, Gray4::WHITE);
    Text::new("Main Menu", Point::new(20, 35), header_style).draw(display)?;

    let menu_style = MonoTextStyle::new(&FONT_10X20, Gray4::BLACK);
    let menu_items = [
        "1. Now Playing",
        "2. Library",
        "3. Playlists",
        "4. Settings",
        "5. About",
    ];

    for (idx, item) in menu_items.iter().enumerate() {
        let y = 100 + (idx as i32 * 50);
        if idx % 2 == 0 {
            Rectangle::new(Point::new(10, y - 5), Size::new(size.width - 20, 45))
                .into_styled(PrimitiveStyle::with_fill(Gray4::new(0xE)))
                .draw(display)?;
        }
        Text::new(item, Point::new(30, y + 20), menu_style).draw(display)?;
    }

    let footer_style = MonoTextStyle::new(&FONT_10X20, Gray4::new(0x8));
    Text::new(
        "Replace this with your UI in display_emulator.rs",
        Point::new(30, (size.height - 30) as i32),
        footer_style,
    )
    .draw(display)?;

    Ok(())
}
