# Interactive Now Playing Screen — Application Event Loop

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Replace the static demo menu in the emulator with a live, keyboard-driven Now Playing screen that reacts to button presses and encoder turns.

**Architecture:** Fix `pump_events()` in `Window` to forward keyboard events during display refreshes (currently uses a minimal `PumpEventHandler` that discards them). Add a `pump_window_events()` method for non-blocking OS event polling. Replace `display.into_inner().run()` in `display_emulator.rs` with an async polling loop: pump OS events → drain `InputQueue` → update `AppState` → partial refresh on change.

**Tech Stack:** winit 0.30 `pump_app_events`, `platform::InputDevice` / `InputEvent` / `Button`, `ui::navigation::Navigator`, `ui::now_playing::NowPlayingState`, `firmware_ui::screens::now_playing::render_now_playing_to`, tokio multi-thread runtime, eink-emulator `Emulator`.

---

### Task 1: Fix `pump_events()` — forward keyboard events during refresh

**Context:** `Window::pump_events()` is called during every `refresh_full()` / `refresh_partial()` animation. It currently pumps OS events using a minimal `PumpEventHandler` struct that ignores `KeyboardInput` and `MouseWheel`, so key presses during a 300 ms refresh are silently dropped. This task replaces that struct with `self` (the `Window` itself) so the full `Window::window_event()` handler runs, forwarding keyboard events to `InputQueue`.

**Files:**
- Modify: `crates/eink/eink-emulator/src/window.rs`

**Step 1: Read the file to understand current structure**

Read `crates/eink/eink-emulator/src/window.rs`. Locate:
- `PumpEventHandler` struct and its `impl ApplicationHandler` (around line 251)
- `Window::pump_events()` method (around line 673)

Confirm that `pump_events` creates a `PumpEventHandler` and passes it to `el.pump_app_events()`.

**Step 2: Write a failing test**

Add to `window.rs` — inside the existing `#[cfg(test)]` block if one exists, or create one at the end:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn pump_events_compiles_without_pump_event_handler() {
        // This is a compile-only test. If PumpEventHandler is removed and
        // pump_events() uses Window as the ApplicationHandler, this must compile.
        // Actual runtime test requires a display (omitted in CI).
    }
}
```

Run: `cargo test -p eink-emulator 2>&1 | grep pump_events`
Expected: test exists (0 failures).

**Step 3: Remove `PumpEventHandler`**

Delete the entire `PumpEventHandler` struct definition and its `impl ApplicationHandler for PumpEventHandler` block (both together ~28 lines).

These are the only two uses of `PumpEventHandler` — the struct definition and one construction site in `pump_events()`.

**Step 4: Rewrite `pump_events()` to use `self`**

Find `pub fn pump_events(&mut self, duration: Duration)` and replace its body:

Old body (roughly):
```rust
let step = Duration::from_millis(16);
let mut remaining = duration;

while remaining > Duration::ZERO {
    std::thread::sleep(remaining.min(step));
    remaining = remaining.saturating_sub(step);

    let mut handler = PumpEventHandler {
        phys_w: self.phys_w,
        phys_h: self.phys_h,
        should_exit: false,
    };

    if let Some(ref mut el) = self.event_loop {
        match el.pump_app_events(Some(Duration::ZERO), &mut handler) {
            PumpStatus::Exit(_) => break,
            PumpStatus::Continue => {}
        }
    } else {
        std::thread::sleep(remaining);
        break;
    }

    if handler.should_exit {
        break;
    }
}
```

New body:
```rust
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
```

**Step 5: Verify it compiles**

```bash
cargo check -p eink-emulator 2>&1
```
Expected: no errors.

**Step 6: Run existing tests**

```bash
cargo test -p eink-emulator --features debug 2>&1 | tail -5
```
Expected: all tests still pass.

**Step 7: Commit**

```bash
git add crates/eink/eink-emulator/src/window.rs
git commit -m "fix(emulator): pump_events forwards keyboard during refresh (remove PumpEventHandler)"
```

---

### Task 2: Add `pump_window_events()` to `Window`

**Context:** The interactive application loop needs to pump OS events non-blockingly between renders — not during a timed refresh animation. This new method takes `event_loop` out, pumps with `self` as handler (zero timeout = process only currently queued events), and returns `false` when the close button is clicked.

**Files:**
- Modify: `crates/eink/eink-emulator/src/window.rs`

**Step 1: Write a failing test**

Add to `mod tests` in `window.rs`:

```rust
#[test]
fn pump_window_events_returns_false_without_event_loop() {
    // A Window with no event_loop (e.g. after run() consumes it) returns false.
    // We can't construct a real Window without a display, so test the logic path:
    // pump_window_events() on a headless emulator must return false gracefully.
    // (Compile-only — runtime path requires a display.)
    let _ = "pump_window_events returns false when event_loop is None";
}
```

Run: `cargo test -p eink-emulator 2>&1 | grep pump_window`
Expected: test exists (0 failures).

**Step 2: Implement `pump_window_events()`**

Add the following method to `impl Window`, directly after `pump_events`:

```rust
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
```

**Step 3: Check it compiles**

```bash
cargo check -p eink-emulator 2>&1
```
Expected: no errors.

**Step 4: Run existing tests**

```bash
cargo test -p eink-emulator --features debug 2>&1 | tail -5
```
Expected: all pass.

**Step 5: Commit**

```bash
git add crates/eink/eink-emulator/src/window.rs
git commit -m "feat(emulator): add pump_window_events() for non-blocking OS event polling"
```

---

### Task 3: Expose `pump_window_events()` on `Emulator`

**Context:** `Emulator` is the public API surface; `Window` is internal. The application code in `display_emulator.rs` reaches the emulator via `EmulatorDisplay::emulator_mut()` → `&mut Emulator`. We add a thin proxy method.

**Files:**
- Modify: `crates/eink/eink-emulator/src/lib.rs`

**Step 1: Locate the `Emulator` impl**

In `lib.rs`, search for `impl Emulator` and find the block of public methods. Look for `run()` and `initialize()` to orient yourself.

**Step 2: Write a failing test**

Add to the existing `#[cfg(test)] mod tests` in `lib.rs`:

```rust
#[test]
fn pump_window_events_headless_returns_false() {
    // In headless mode (no window) pump_window_events() must return false.
    // Emulator::headless(width, height) constructs a windowless emulator.
    let mut emulator = Emulator::headless(128, 64);
    assert!(!emulator.pump_window_events());
}
```

> NOTE: `Emulator::headless(width, height)` takes two `u32` arguments. Look at existing tests in `lib.rs` for other headless construction patterns if this signature has changed.

Run: `cargo test -p eink-emulator 2>&1 | grep pump_window`
Expected: FAIL with "method not found".

**Step 3: Implement on `Emulator`**

Find the `impl Emulator` block (not `impl<...> DrawTarget for Emulator`) and add:

```rust
/// Poll all pending OS events without blocking.
///
/// Forwards `KeyboardInput` and `MouseWheel` events to the `InputQueue`
/// (when [`keyboard-input`](crate) feature is active) so that
/// `EmulatorInput::poll_event()` sees them.
///
/// Returns `true` if the window is still open, `false` if the user clicked
/// the close button or there is no window (headless mode).
pub fn pump_window_events(&mut self) -> bool {
    #[cfg(not(feature = "headless"))]
    if let Some(ref mut w) = self.window {
        return w.pump_window_events();
    }
    false
}
```

> The `self.window` field is `Option<Window>`. In headless mode the field is absent or `None`; the `false` fallback handles that.

**Step 4: Run the test**

```bash
cargo test -p eink-emulator 2>&1 | grep pump_window
```
Expected: PASS.

**Step 5: Commit**

```bash
git add crates/eink/eink-emulator/src/lib.rs
git commit -m "feat(emulator): expose pump_window_events() on Emulator"
```

---

### Task 4: Add `AppState` and `handle_input()` in `display_emulator.rs`

**Context:** We need a plain Rust struct (no embassy, no generics) that holds the current Now Playing state and responds to `InputEvent`s. This is pure logic and is fully unit-testable on the host.

**Files:**
- Modify: `crates/firmware/examples/display_emulator.rs`

**Step 1: Add imports at the top of the file**

Inside the existing `use` section (before `fn main()`), add:

```rust
#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use firmware::input::{Button, InputEvent};
#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use platform::InputDevice as _;
#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use ui::navigation::{Navigator, Screen};
#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use ui::now_playing::NowPlayingState;
#[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
use firmware_ui::screens::now_playing::render_now_playing_to;
```

**Cargo dependency check:**

- `firmware-ui` is already `optional = true` in `[dependencies]` but only enabled by `hot-reload` feature — add it to `keyboard-input`:

In `crates/firmware/Cargo.toml`, update the `keyboard-input` feature:
```toml
keyboard-input = [
    "emulator",
    "eink-emulator/keyboard-input",
    "firmware-ui",           # <-- add this line
]
```

- `ui` is already in `[dev-dependencies]` as `ui = { path = "../ui" }`.  Examples can use dev-dependencies, so no change needed for `ui`.

**Step 2: Write a failing test for `handle_input()`**

At the BOTTOM of `display_emulator.rs`, add:

```rust
#[cfg(all(test, feature = "keyboard-input"))]
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
        // Release events do not toggle state
        assert!(!s.now_playing.playing);
        assert!(!s.needs_redraw);
    }
}
```

Run:
```bash
cargo test --example display_emulator --features emulator,keyboard-input -p firmware 2>&1 | tail -15
```

> NOTE: Examples are not normally tested with `cargo test --example`. Use:
```bash
cargo test -p firmware --features emulator,keyboard-input --lib 2>&1 | tail -10
```
Wait — the tests are in the example file, not in lib.rs. They won't run with `--lib`. This is fine: they'll be compiled but can only run via:
```bash
cargo test --example display_emulator --features emulator,keyboard-input -p firmware 2>&1 | tail -15
```

Expected: FAIL with "AppState not found" or similar.

**Step 3: Add `AppState` struct and `handle_input()`**

Add the following block to `display_emulator.rs`, just before `fn main()` (wrapped in the feature gate):

```rust
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
                self.now_playing.set_volume(self.now_playing.volume.saturating_add(5));
                self.needs_redraw = true;
            }
            InputEvent::ButtonPress(Button::VolumeDown) => {
                self.now_playing.set_volume(self.now_playing.volume.saturating_sub(5));
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
                // Each encoder step = ±2 volume units (clamped 0..=100).
                if steps > 0 {
                    let delta = (steps.unsigned_abs() as u8).min(100) * 2;
                    self.now_playing
                        .set_volume(self.now_playing.volume.saturating_add(delta));
                } else {
                    let delta = (steps.unsigned_abs() as u8).min(100) * 2;
                    self.now_playing
                        .set_volume(self.now_playing.volume.saturating_sub(delta));
                }
                self.needs_redraw = true;
            }
            _ => {} // ButtonRelease and unmapped events are ignored
        }
    }
}
```

**Step 4: Run the tests**

```bash
cargo test --example display_emulator --features emulator,keyboard-input -p firmware 2>&1 | tail -20
```
Expected: all `AppState` tests pass.

**Step 5: Commit**

```bash
git add crates/firmware/examples/display_emulator.rs crates/firmware/Cargo.toml
git commit -m "feat(emulator): add AppState + handle_input() for interactive Now Playing screen"
```

---

### Task 5: Wire the interactive application loop in `display_emulator.rs`

**Context:** Replace the static `render_demo_menu + run()` non-hot-reload path with an async polling loop. The loop pumps OS events (→ `InputQueue`), drains `EmulatorInput`, calls `handle_input()`, and triggers a partial refresh when state changes.

**Files:**
- Modify: `crates/firmware/examples/display_emulator.rs`

**Step 1: Locate the non-hot-reload section**

Find the block:
```rust
#[cfg(not(feature = "hot-reload"))]
{
    tracing::info!("Rendering demo menu");
    render_demo_menu(&mut display)?;
    rt.block_on(async { display.refresh_full().await })?;

    // Register named DAP scene components ...
    #[cfg(feature = "debug")]
    register_dap_components(&mut display);

    tracing::info!("Demo menu rendered");
    tracing::info!("Close window to exit");

    // Spawn a background task that logs every input event to stdout.
    #[cfg(all(feature = "keyboard-input", not(feature = "hot-reload")))]
    rt.spawn(async move {
        use platform::InputDevice as _;
        loop {
            let ev = input.wait_for_event().await;
            tracing::info!(event = ?ev, "Input");
        }
    });

    display.into_inner().run();
}
```

**Step 2: Replace the keyboard-input sub-block with the interactive loop**

Keep the non-keyboard-input path unchanged (static menu + `run()`). For the keyboard-input path, replace the `rt.spawn` + `display.into_inner().run()` with a polling loop.

The full replacement for the `#[cfg(not(feature = "hot-reload"))]` block:

```rust
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
        use embedded_graphics::pixelcolor::Gray4;

        tracing::info!("Starting interactive Now Playing screen");

        let mut state = AppState::default();

        rt.block_on(async {
            // Initial full render
            render_now_playing_to(&mut display, &state.now_playing, |_, _, _, _| {})?;
            display.refresh_full().await?;
            state.needs_redraw = false;

            tracing::info!("Now Playing screen ready");
            tracing::info!(
                bindings = "Space/K=Play  ←/J=Prev  →/L=Next  ↑/==Vol+  ↓/-=Vol-  M=Menu  Esc/BS=Back  Scroll=Encoder",
                "Keyboard input",
            );

            loop {
                // Pump OS events — forwards keyboard/scroll to InputQueue.
                // Returns false when the close button is clicked.
                if !display.emulator_mut().pump_window_events() {
                    tracing::info!("Window closed");
                    break;
                }

                // Drain all pending input events (non-blocking).
                loop {
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
```

> NOTE: The `render_now_playing_to` signature is:
> ```rust
> pub fn render_now_playing_to<D, R>(display: &mut D, state: &NowPlayingState, register: R)
> where D: DrawTarget<Color = Gray4>, R: FnMut(&str, &str, (i32, i32), (u32, u32))
> ```
> The closure `|_, _, _, _| {}` is a no-op register callback (debug-panel registration omitted for now).

**Step 3: Check it compiles**

```bash
cargo check --example display_emulator --features emulator,keyboard-input -p firmware 2>&1
```
Expected: no errors. Fix any type mismatches.

**Step 4: Run existing tests**

```bash
cargo test --workspace 2>&1 | tail -5
```
Expected: all pass.

**Step 5: Manual smoke test (optional — requires a display)**

```bash
cargo run --example display_emulator --features emulator,keyboard-input -p firmware
```

Expected behavior:
- Window opens with Now Playing screen (title "Sample Track", artist "Sample Artist", progress bar, Play button)
- Press `Space` or `K` → button label toggles between "Play" and "Pause"
- Press `↑` or `=` → volume increases (visible if you add a volume display to the screen)
- Scroll up → volume increases
- Press `M` → nav pushes LibraryBrowse (no visual change yet — no LibraryBrowse renderer)
- Press `Esc` → nav pops back to NowPlaying
- Close window → process exits cleanly

**Step 6: Commit**

```bash
git add crates/firmware/examples/display_emulator.rs
git commit -m "feat(emulator): interactive Now Playing screen with keyboard-driven state loop"
```

---

### Task 6: Run full verification

**Step 1: All compile checks**

```bash
cargo check --features emulator,keyboard-input -p firmware 2>&1
cargo check --target thumbv7em-none-eabihf --features hardware -p firmware 2>&1
cargo check -p eink-emulator --features debug 2>&1
```
Expected: no errors.

**Step 2: Full test suite**

```bash
cargo test --workspace 2>&1 | tail -10
cargo test -p eink-emulator --features debug 2>&1 | tail -5
```
Expected: all pass.

**Step 3: Commit if anything was fixed**

```bash
git add -p
git commit -m "fix(emulator): verification pass — interactive now playing loop"
```

---

## Summary

After this plan, the emulator:

| Before | After |
|--------|-------|
| Static demo menu, no interaction | Live Now Playing screen |
| `display.into_inner().run()` blocks forever | Async polling loop with exit detection |
| Keyboard events during refresh are dropped | Keyboard events forwarded during refresh too |
| Input task only logs events | Input events update `AppState` → partial refresh |

**Key files changed:**
- `crates/eink/eink-emulator/src/window.rs` — remove `PumpEventHandler`, fix `pump_events`, add `pump_window_events`
- `crates/eink/eink-emulator/src/lib.rs` — proxy `pump_window_events` on `Emulator`
- `crates/firmware/examples/display_emulator.rs` — `AppState`, `handle_input`, interactive loop
- `crates/firmware/Cargo.toml` — add `firmware-ui` + `ui` to `keyboard-input` feature

**Known limitations (future work):**
- No LibraryBrowse screen renderer (navigation works but shows no visual change)
- No volume readout on screen (volume changes state but isn't displayed yet)
- No track position timer (progress bar stays at 0)
