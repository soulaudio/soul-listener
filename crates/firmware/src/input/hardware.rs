//! Hardware GPIO input driver — rotary encoder + debounced buttons.
//!
//! # Pin assignments
//!
//! These constants document the target PCB assignment; change them to match
//! your board before flashing.
//!
//! | Signal          | MCU pin | Notes                          |
//! |-----------------|---------|--------------------------------|
//! | Encoder CLK (A) | PA8     | EXTI8 rising-edge interrupt    |
//! | Encoder DT  (B) | PA3     | GPIO input only (no EXTI)      |
//! | Play/Pause      | PA0     | Active-low, internal pull-up   |
//! | Next            | PA1     | Active-low, internal pull-up   |
//! | Previous        | PA2     | Active-low, internal pull-up   |
//! | Menu            | PD3     | Active-low, internal pull-up   |
//! | Back            | PD4     | Active-low, internal pull-up   |
//! | Select          | PD5     | Active-low, internal pull-up   |
//!
//! # Architecture
//!
//! A single static [`Channel`] carries events from the GPIO task to the
//! application.  [`HardwareInput`] wraps the channel receiver and implements
//! [`platform::InputDevice`].
//!
//! Call [`spawn_input_task`] once at startup to own all GPIO peripherals and
//! start the concurrent encoder + button tasks.

use embassy_executor::Spawner;
use embassy_futures::join::{join, join5};
use embassy_stm32::exti::ExtiInput;
use embassy_stm32::gpio::{AnyPin, Input};
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_sync::channel::{Channel, Receiver, Sender};
use embassy_time::Timer;

use platform::{Button, InputDevice, InputEvent};

// ---------------------------------------------------------------------------
// Channel capacity
// ---------------------------------------------------------------------------

/// Depth of the static event channel.
const CHANNEL_DEPTH: usize = 16;

// ---------------------------------------------------------------------------
// Static channel (one sender per GPIO task, one receiver for HardwareInput)
// ---------------------------------------------------------------------------

/// Global event channel shared between the GPIO task and the application.
pub static INPUT_CHANNEL: Channel<CriticalSectionRawMutex, InputEvent, CHANNEL_DEPTH> =
    Channel::new();

// ---------------------------------------------------------------------------
// HardwareInput — consumer
// ---------------------------------------------------------------------------

/// Hardware input driver.  Owns the [`Channel`] receiver and implements
/// [`platform::InputDevice`].
///
/// # Usage
/// ```no_run
/// use firmware::input::hardware::{HardwareInput, spawn_input_task};
///
/// // In your main / init:
/// spawn_input_task(spawner, encoder_clk, encoder_dt, btn_play, btn_next,
///                  btn_prev, btn_menu, btn_back, btn_select);
///
/// let mut input = HardwareInput::new();
/// // Then pass `input` to your application task.
/// ```
pub struct HardwareInput {
    rx: Receiver<'static, CriticalSectionRawMutex, InputEvent, CHANNEL_DEPTH>,
}

impl HardwareInput {
    /// Create a new hardware input driver backed by the static channel.
    pub fn new() -> Self {
        Self {
            rx: INPUT_CHANNEL.receiver(),
        }
    }
}

impl Default for HardwareInput {
    fn default() -> Self {
        Self::new()
    }
}

impl InputDevice for HardwareInput {
    async fn wait_for_event(&mut self) -> InputEvent {
        self.rx.receive().await
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        self.rx.try_receive().ok()
    }
}

// ---------------------------------------------------------------------------
// GPIO task
// ---------------------------------------------------------------------------

/// Spawn the GPIO input task.
///
/// Call this once from your Embassy `main` function.  The task owns all GPIO
/// peripherals for the lifetime of the program.
///
/// # Parameters
/// - `spawner` — Embassy task spawner.
/// - `enc_clk` — Encoder CLK (A) pin with EXTI capability (PA8).
/// - `enc_dt` — Encoder DT (B) pin (PA3, input only).
/// - `btn_play` through `btn_select` — Button pins with EXTI capability (PA0–PA2, PD3–PD5).
pub fn spawn_input_task(
    spawner: &Spawner,
    enc_clk: ExtiInput<'static, AnyPin>,
    enc_dt: Input<'static, AnyPin>,
    btn_play: ExtiInput<'static, AnyPin>,
    btn_next: ExtiInput<'static, AnyPin>,
    btn_prev: ExtiInput<'static, AnyPin>,
    btn_menu: ExtiInput<'static, AnyPin>,
    btn_back: ExtiInput<'static, AnyPin>,
    btn_select: ExtiInput<'static, AnyPin>,
) {
    spawner
        .spawn(input_task(
            enc_clk, enc_dt, btn_play, btn_next, btn_prev, btn_menu, btn_back, btn_select,
        ))
        .expect("failed to spawn input_task");
}

/// Embassy task that owns all input GPIO and forwards events to [`INPUT_CHANNEL`].
#[embassy_executor::task]
async fn input_task(
    mut enc_clk: ExtiInput<'static, AnyPin>,
    enc_dt: Input<'static, AnyPin>,
    mut btn_play: ExtiInput<'static, AnyPin>,
    mut btn_next: ExtiInput<'static, AnyPin>,
    mut btn_prev: ExtiInput<'static, AnyPin>,
    mut btn_menu: ExtiInput<'static, AnyPin>,
    mut btn_back: ExtiInput<'static, AnyPin>,
    mut btn_select: ExtiInput<'static, AnyPin>,
) {
    let tx = INPUT_CHANNEL.sender();

    // embassy-futures 0.1 tops out at join5; nest two join5 + join for 7 tasks.
    join(
        join5(
            encoder_loop(&mut enc_clk, &enc_dt, tx),
            button_loop(&mut btn_play, Button::Play, tx),
            button_loop(&mut btn_next, Button::Next, tx),
            button_loop(&mut btn_prev, Button::Previous, tx),
            button_loop(&mut btn_menu, Button::Menu, tx),
        ),
        join(
            button_loop(&mut btn_back, Button::Back, tx),
            button_loop(&mut btn_select, Button::Select, tx),
        ),
    )
    .await;
}

// ---------------------------------------------------------------------------
// Encoder loop
// ---------------------------------------------------------------------------

/// Quadrature encoder loop.
///
/// Waits for a rising edge on CLK (A), then samples DT (B) to determine
/// direction.  One step per rising edge (quarter-period resolution).
async fn encoder_loop(
    clk: &mut ExtiInput<'static, AnyPin>,
    dt: &Input<'static, AnyPin>,
    tx: Sender<'static, CriticalSectionRawMutex, InputEvent, CHANNEL_DEPTH>,
) {
    loop {
        clk.wait_for_rising_edge().await;
        // DT low when CLK rises → clockwise (+1); DT high → counter-clockwise (−1).
        let increment = if dt.is_low() { 1_i32 } else { -1_i32 };
        defmt::trace!("Encoder step: delta={=i32}", increment);
        tx.send(InputEvent::RotaryIncrement(increment)).await;
    }
}

// ---------------------------------------------------------------------------
// Button loop
// ---------------------------------------------------------------------------

/// Debounced button loop (active-low, internal pull-up).
///
/// Waits for a falling edge (press), debounces 20 ms, confirms the level is
/// still low, sends `ButtonPress`, then waits for a rising edge (release) and
/// sends `ButtonRelease`.
async fn button_loop(
    pin: &mut ExtiInput<'static, AnyPin>,
    btn: Button,
    tx: Sender<'static, CriticalSectionRawMutex, InputEvent, CHANNEL_DEPTH>,
) {
    loop {
        pin.wait_for_falling_edge().await;
        Timer::after_millis(20).await; // debounce
        if pin.is_low() {
            defmt::debug!("Button press: {}", btn);
            tx.send(InputEvent::ButtonPress(btn)).await;
            pin.wait_for_rising_edge().await;
            Timer::after_millis(20).await; // debounce release
            defmt::debug!("Button release: {}", btn);
            tx.send(InputEvent::ButtonRelease(btn)).await;
        }
    }
}
