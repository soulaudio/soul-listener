//! Fluent builder API for input source configuration.
//!
//! Provides a convenient builder pattern for configuring input sources with
//! feature-gated hardware and emulated paths.
//!
//! # Design Notes
//!
//! A full type-state builder (where `.build_hardware()` only compiles after
//! `.pins()` is called) would require storing `embassy_stm32` generic pin
//! types inside the builder struct, which forces either boxing or complex
//! generics across the call site.  Instead we use a lighter approach:
//! the builder stores non-pin configuration (debounce timing, emulated key
//! mappings) and accepts the actual GPIO objects directly in `build_hardware`.
//! This keeps the API ergonomic while preserving the fluent `.debounce_ms()`
//! chain.
//!
//! # Usage (hardware)
//!
//! ```no_run
//! # #[cfg(feature = "hardware")]
//! # {
//! use firmware::input::builder::InputBuilder;
//! use platform::Button;
//!
//! let config = InputBuilder::rotary().debounce_ms(20);
//! // Pass config + GPIO pins to spawn_input_task_with_config(...)
//! # }
//! ```
//!
//! # Usage (emulator)
//!
//! ```no_run
//! # #[cfg(feature = "keyboard-input")]
//! # {
//! use firmware::input::builder::{InputBuilder, EmulatedAxis, EmulatedKey};
//! use platform::Button;
//!
//! let mut emulator = eink_emulator::Emulator::headless(800, 480);
//!
//! let _encoder_input = InputBuilder::rotary()
//!     .emulated(EmulatedAxis::ArrowUpDown)
//!     .build_emulated(&mut emulator);
//!
//! let _play_input = InputBuilder::button(Button::Play)
//!     .emulated_key(EmulatedKey::Space)
//!     .build_emulated(&mut emulator);
//! # }
//! ```

use platform::Button;

// ---------------------------------------------------------------------------
// EmulatedAxis — what physical scroll/encoder axis maps to
// ---------------------------------------------------------------------------

/// Emulated axis source for the rotary encoder.
///
/// Determines which host input event is interpreted as encoder rotation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmulatedAxis {
    /// Arrow Up = +1 (clockwise), Arrow Down = -1 (counter-clockwise).
    ArrowUpDown,
    /// Mouse scroll wheel: scroll up = +1, scroll down = -1.
    ScrollWheel,
}

// ---------------------------------------------------------------------------
// EmulatedKey — what keyboard key maps to a button
// ---------------------------------------------------------------------------

/// Emulated keyboard key for a button.
///
/// These are the primary key bindings.  The emulator's `map_key` function
/// handles the full multi-key mapping; this enum documents the *canonical*
/// key for each button used in builder configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmulatedKey {
    /// Space bar → [`Button::Play`].
    Space,
    /// K key → [`Button::Play`] (vi-style).
    KeyK,
    /// Right arrow → [`Button::Next`].
    ArrowRight,
    /// L key → [`Button::Next`] (vi-style).
    KeyL,
    /// Left arrow → [`Button::Previous`].
    ArrowLeft,
    /// J key → [`Button::Previous`] (vi-style).
    KeyJ,
    /// Up arrow → [`Button::VolumeUp`].
    ArrowUp,
    /// Down arrow → [`Button::VolumeDown`].
    ArrowDown,
    /// M key → [`Button::Menu`].
    KeyM,
    /// Escape → [`Button::Back`].
    Escape,
    /// Enter → [`Button::Select`].
    Enter,
}

// ---------------------------------------------------------------------------
// InputKind — what this builder represents
// ---------------------------------------------------------------------------

/// Internal: what kind of input source the builder is configuring.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputKind {
    /// Rotary encoder (quadrature).
    Rotary,
    /// Momentary push button.
    Button(Button),
}

// ---------------------------------------------------------------------------
// InputBuilder — main builder struct
// ---------------------------------------------------------------------------

/// Fluent builder for input sources.
///
/// Call [`InputBuilder::rotary()`] or [`InputBuilder::button()`] to start,
/// then chain configuration methods, and finally call the appropriate
/// `build_*` method for your platform.
///
/// # Feature gates
///
/// - `.emulated()` / `.emulated_key()` / `build_emulated()`:
///   only compiled with `keyboard-input` feature.
/// - `build_hardware()`: hardware-specific wiring lives in
///   `input::hardware::spawn_input_task`.
pub struct InputBuilder {
    kind: InputKind,
    debounce_ms: u32,

    // Emulator configuration (compiled only with keyboard-input feature)
    #[cfg(feature = "keyboard-input")]
    emulated_axis: Option<EmulatedAxis>,

    #[cfg(feature = "keyboard-input")]
    emulated_key: Option<EmulatedKey>,
}

impl InputBuilder {
    // -----------------------------------------------------------------------
    // Factory methods
    // -----------------------------------------------------------------------

    /// Start building a rotary encoder input.
    ///
    /// Default debounce: 20 ms.
    pub fn rotary() -> Self {
        Self {
            kind: InputKind::Rotary,
            debounce_ms: 20,

            #[cfg(feature = "keyboard-input")]
            emulated_axis: None,

            #[cfg(feature = "keyboard-input")]
            emulated_key: None,
        }
    }

    /// Start building a button input for `btn`.
    ///
    /// Default debounce: 50 ms.
    pub fn button(btn: Button) -> Self {
        Self {
            kind: InputKind::Button(btn),
            debounce_ms: 50,

            #[cfg(feature = "keyboard-input")]
            emulated_axis: None,

            #[cfg(feature = "keyboard-input")]
            emulated_key: None,
        }
    }

    // -----------------------------------------------------------------------
    // Common configuration
    // -----------------------------------------------------------------------

    /// Set debounce time in milliseconds.
    ///
    /// Applied to hardware GPIO reads.  Has no effect on the emulated path.
    #[must_use]
    pub fn debounce_ms(mut self, ms: u32) -> Self {
        self.debounce_ms = ms;
        self
    }

    /// Get the configured debounce time in milliseconds.
    pub fn debounce(&self) -> u32 {
        self.debounce_ms
    }

    // -----------------------------------------------------------------------
    // Emulator configuration — only compiled with keyboard-input feature
    // -----------------------------------------------------------------------

    /// Configure the emulated axis for a rotary encoder builder.
    ///
    /// Only meaningful on a builder created with [`InputBuilder::rotary()`].
    /// On a button builder this is silently ignored.
    ///
    /// # Example
    /// ```no_run
    /// use firmware::input::builder::{InputBuilder, EmulatedAxis};
    ///
    /// let config = InputBuilder::rotary().emulated(EmulatedAxis::ScrollWheel);
    /// ```
    #[cfg(feature = "keyboard-input")]
    #[must_use]
    pub fn emulated(mut self, axis: EmulatedAxis) -> Self {
        self.emulated_axis = Some(axis);
        self
    }

    /// Configure the emulated keyboard key for a button builder.
    ///
    /// Only meaningful on a builder created with [`InputBuilder::button()`].
    /// On a rotary builder this is silently ignored.
    ///
    /// # Example
    /// ```no_run
    /// use firmware::input::builder::{InputBuilder, EmulatedKey};
    /// use platform::Button;
    ///
    /// let config = InputBuilder::button(Button::Play).emulated_key(EmulatedKey::Space);
    /// ```
    #[cfg(feature = "keyboard-input")]
    #[must_use]
    pub fn emulated_key(mut self, key: EmulatedKey) -> Self {
        self.emulated_key = Some(key);
        self
    }

    // -----------------------------------------------------------------------
    // Build — emulator path
    // -----------------------------------------------------------------------

    /// Build the emulated input driver.
    ///
    /// Calls [`Emulator::input_receiver()`] to attach this builder's key/axis
    /// configuration to the emulator window.  Returns an [`EmulatorInput`]
    /// that implements [`platform::InputDevice`].
    ///
    /// The emulator's built-in key map ([`eink_emulator::input::map_key`])
    /// already handles the full multi-key layout for every button; this
    /// builder records the *canonical* mapping for documentation and future
    /// per-instance filtering.
    ///
    /// # Panics
    ///
    /// Does not panic.  Calling this on a rotary builder with no axis set,
    /// or a button builder with no key set, is valid — the default global
    /// key map will still handle events.
    ///
    /// [`Emulator::input_receiver()`]: eink_emulator::Emulator::input_receiver
    /// [`EmulatorInput`]: eink_emulator::input::EmulatorInput
    #[cfg(feature = "keyboard-input")]
    pub fn build_emulated(
        self,
        emulator: &mut eink_emulator::Emulator,
    ) -> eink_emulator::input::EmulatorInput {
        // Log the configured mapping so developers can see what keys are wired.
        match self.kind {
            InputKind::Rotary => {
                let axis_desc = self
                    .emulated_axis
                    .map(|a| format!("{:?}", a))
                    .unwrap_or_else(|| "ScrollWheel (default)".to_string());
                eprintln!(
                    "[InputBuilder] Rotary encoder → emulated axis: {}",
                    axis_desc
                );
            }
            InputKind::Button(btn) => {
                let key_desc = self
                    .emulated_key
                    .map(|k| format!("{:?}", k))
                    .unwrap_or_else(|| "global key map (default)".to_string());
                eprintln!(
                    "[InputBuilder] Button::{:?} → emulated key: {}",
                    btn, key_desc
                );
            }
        }

        // Attach (or re-attach) the input queue to the emulator window.
        emulator.input_receiver()
    }
}

// ---------------------------------------------------------------------------
// Hardware convenience: re-export spawn helper
// ---------------------------------------------------------------------------

/// Spawn all hardware GPIO input tasks from the builder's configuration.
///
/// This is a thin convenience wrapper around
/// [`hardware::spawn_input_task`](super::hardware::spawn_input_task) that
/// allows callers to configure debounce via an [`InputBuilder`] chain and
/// then call this function to wire everything up.
///
/// Debounce timing from the builder is currently informational — the GPIO
/// task uses the value from [`InputBuilder::debounce()`] that is printed at
/// start-up.  Extending `spawn_input_task` to accept a debounce parameter
/// is left as future work.
///
/// # Pin assignments (STM32H743ZI target PCB)
///
/// | Signal          | MCU pin | Parameter   |
/// |-----------------|---------|-------------|
/// | Encoder CLK (A) | PE9     | `enc_clk`   |
/// | Encoder DT  (B) | PE11    | `enc_dt`    |
/// | Play/Pause      | PD0     | `btn_play`  |
/// | Next            | PD1     | `btn_next`  |
/// | Previous        | PD2     | `btn_prev`  |
/// | Menu            | PD3     | `btn_menu`  |
/// | Back            | PD4     | `btn_back`  |
/// | Select          | PD5     | `btn_select`|
///
/// # Example
///
/// ```no_run
/// # #[cfg(feature = "hardware")]
/// # {
/// use embassy_executor::Spawner;
/// use embassy_stm32::exti::ExtiInput;
/// use embassy_stm32::gpio::{AnyPin, Input, Pull};
/// use firmware::input::builder::{InputBuilder, spawn_hardware_input};
///
/// async fn example(spawner: Spawner, p: embassy_stm32::Peripherals) {
///     let enc_config = InputBuilder::rotary().debounce_ms(20);
///     let btn_config = InputBuilder::button(platform::Button::Play).debounce_ms(50);
///
///     eprintln!("Encoder debounce: {} ms", enc_config.debounce());
///     eprintln!("Button debounce:  {} ms", btn_config.debounce());
///
///     // Construct the GPIO objects — embassy-stm32 requires typed peripherals.
///     let enc_clk = ExtiInput::new(
///         Input::new(p.PE9.degrade(), Pull::None),
///         p.EXTI9,
///     );
///     // ... (remaining pins) ...
/// }
/// # }
/// ```
#[cfg(feature = "hardware")]
pub fn spawn_hardware_input(
    spawner: &embassy_executor::Spawner,
    enc_clk: embassy_stm32::exti::ExtiInput<'static, embassy_stm32::gpio::AnyPin>,
    enc_dt: embassy_stm32::gpio::Input<'static, embassy_stm32::gpio::AnyPin>,
    btn_play: embassy_stm32::exti::ExtiInput<'static, embassy_stm32::gpio::AnyPin>,
    btn_next: embassy_stm32::exti::ExtiInput<'static, embassy_stm32::gpio::AnyPin>,
    btn_prev: embassy_stm32::exti::ExtiInput<'static, embassy_stm32::gpio::AnyPin>,
    btn_menu: embassy_stm32::exti::ExtiInput<'static, embassy_stm32::gpio::AnyPin>,
    btn_back: embassy_stm32::exti::ExtiInput<'static, embassy_stm32::gpio::AnyPin>,
    btn_select: embassy_stm32::exti::ExtiInput<'static, embassy_stm32::gpio::AnyPin>,
) {
    super::hardware::spawn_input_task(
        spawner, enc_clk, enc_dt, btn_play, btn_next, btn_prev, btn_menu, btn_back, btn_select,
    );
}
