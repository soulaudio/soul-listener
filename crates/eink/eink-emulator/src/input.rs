//! Keyboard and scroll-wheel input for the desktop emulator.
//!
//! This module is compiled only when the `keyboard-input` feature is active.
//! It provides:
//!
//! - [`InputQueue`] — producer, owned by the winit [`Window`](crate::window::Window).
//! - [`EmulatorInput`] — consumer, returned by [`Emulator::input_receiver()`](crate::Emulator::input_receiver).
//!   Implements [`platform::InputDevice`] so application code is identical for
//!   hardware and emulator targets.
//!
//! # Key mapping
//!
//! | Key(s)              | Action                        |
//! |---------------------|-------------------------------|
//! | Space, K            | [`Button::Play`]              |
//! | →, L, .             | [`Button::Next`]              |
//! | ←, J, ,             | [`Button::Previous`]          |
//! | ↑, =                | [`Button::VolumeUp`]          |
//! | ↓, -                | [`Button::VolumeDown`]        |
//! | M                   | [`Button::Menu`]              |
//! | Backspace, Esc      | [`Button::Back`]              |
//! | Enter               | [`Button::Select`]            |
//! | Scroll up           | `RotaryIncrement(+1)`         |
//! | Scroll down         | `RotaryIncrement(-1)`         |

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use platform::{Button, InputDevice, InputEvent};
use winit::keyboard::KeyCode;

/// Maximum number of unread events buffered in the queue.
///
/// Oldest events are silently dropped when the queue is full.
// In headless mode window.rs is excluded, so the push/map helpers appear unused
// to the compiler even though tests exercise them.  Suppress the noise.
#[cfg_attr(feature = "headless", allow(dead_code))]
const QUEUE_CAP: usize = 64;

// ---------------------------------------------------------------------------
// InputQueue — producer (owned by the winit event loop inside Window)
// ---------------------------------------------------------------------------

/// Producer half of the keyboard-input pipe.
///
/// Lives on the [`Window`](crate::window::Window) and is populated by
/// `WindowEvent::KeyboardInput` and `WindowEvent::MouseWheel` handlers.
#[cfg_attr(feature = "headless", allow(dead_code))]
pub(crate) struct InputQueue {
    queue: Arc<Mutex<VecDeque<InputEvent>>>,
}

impl InputQueue {
    /// Create a linked (producer, consumer) pair.
    pub fn new() -> (Self, EmulatorInput) {
        let q = Arc::new(Mutex::new(VecDeque::new()));
        (InputQueue { queue: q.clone() }, EmulatorInput { queue: q })
    }

    /// Enqueue an event. Silently drops the event if the queue is full.
    #[cfg_attr(feature = "headless", allow(dead_code))]
    pub fn push(&self, event: InputEvent) {
        if let Ok(mut q) = self.queue.lock() {
            if q.len() < QUEUE_CAP {
                q.push_back(event);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// EmulatorInput — consumer (returned to application code)
// ---------------------------------------------------------------------------

/// Consumer half of the keyboard-input pipe.
///
/// Returned by [`Emulator::input_receiver()`](crate::Emulator::input_receiver).
/// Implements [`platform::InputDevice`], so it is a drop-in replacement for
/// the hardware [`HardwareInput`](firmware::input::hardware::HardwareInput)
/// driver.
pub struct EmulatorInput {
    queue: Arc<Mutex<VecDeque<InputEvent>>>,
}

impl InputDevice for EmulatorInput {
    /// Async wait: polls the queue every 5 ms until an event is available.
    async fn wait_for_event(&mut self) -> InputEvent {
        loop {
            if let Some(e) = self.poll_event() {
                return e;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        self.queue.lock().ok()?.pop_front()
    }
}

// ---------------------------------------------------------------------------
// Key / scroll mapping helpers
// ---------------------------------------------------------------------------

/// Map a physical key code and press/release state to an [`InputEvent`].
///
/// Returns `None` for keys that have no mapping (they are silently ignored,
/// except for debug hotkeys which are consumed upstream by the debug manager).
#[cfg_attr(feature = "headless", allow(dead_code))]
pub(crate) fn map_key(code: KeyCode, pressed: bool) -> Option<InputEvent> {
    let btn = match code {
        KeyCode::Space | KeyCode::KeyK => Button::Play,
        KeyCode::ArrowRight | KeyCode::KeyL | KeyCode::Period => Button::Next,
        KeyCode::ArrowLeft | KeyCode::KeyJ | KeyCode::Comma => Button::Previous,
        KeyCode::ArrowUp | KeyCode::Equal => Button::VolumeUp,
        KeyCode::ArrowDown | KeyCode::Minus => Button::VolumeDown,
        KeyCode::KeyM => Button::Menu,
        KeyCode::Backspace | KeyCode::Escape => Button::Back,
        KeyCode::Enter => Button::Select,
        _ => return None,
    };
    Some(if pressed {
        InputEvent::ButtonPress(btn)
    } else {
        InputEvent::ButtonRelease(btn)
    })
}

/// Accumulate a scroll delta and emit a [`RotaryIncrement`](InputEvent::RotaryIncrement)
/// per whole step.
///
/// `delta` positive = scroll up / away from the user (clockwise on a physical encoder).
/// The fractional remainder is preserved in `acc` across calls.
#[cfg_attr(feature = "headless", allow(dead_code))]
pub(crate) fn map_scroll(acc: &mut f64, delta: f64) -> Option<InputEvent> {
    *acc += delta;
    let steps = acc.trunc() as i32;
    if steps != 0 {
        *acc -= steps as f64;
        Some(InputEvent::RotaryIncrement(steps))
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_key_play_buttons() {
        assert_eq!(
            map_key(KeyCode::Space, true),
            Some(InputEvent::ButtonPress(Button::Play))
        );
        assert_eq!(
            map_key(KeyCode::KeyK, false),
            Some(InputEvent::ButtonRelease(Button::Play))
        );
    }

    #[test]
    fn map_key_unmapped_returns_none() {
        assert_eq!(map_key(KeyCode::F1, true), None);
        assert_eq!(map_key(KeyCode::F11, true), None);
        assert_eq!(map_key(KeyCode::Tab, true), None);
    }

    #[test]
    fn map_scroll_accumulates() {
        let mut acc = 0.0_f64;
        assert_eq!(map_scroll(&mut acc, 0.3), None);
        assert_eq!(map_scroll(&mut acc, 0.3), None);
        assert_eq!(
            map_scroll(&mut acc, 0.5),
            Some(InputEvent::RotaryIncrement(1))
        );
        // Remainder ~0.1 is preserved
        assert!((acc - 0.1).abs() < 1e-9);
    }

    #[test]
    fn map_scroll_negative() {
        let mut acc = 0.0_f64;
        let ev = map_scroll(&mut acc, -1.5);
        assert_eq!(ev, Some(InputEvent::RotaryIncrement(-1)));
        assert!((acc - (-0.5)).abs() < 1e-9);
    }

    #[test]
    fn input_queue_push_and_poll() {
        let (producer, mut consumer) = InputQueue::new();
        producer.push(InputEvent::ButtonPress(Button::Menu));
        assert_eq!(
            consumer.poll_event(),
            Some(InputEvent::ButtonPress(Button::Menu))
        );
        assert_eq!(consumer.poll_event(), None);
    }

    #[test]
    fn input_queue_capacity_limit() {
        let (producer, mut consumer) = InputQueue::new();
        // Fill beyond capacity
        for _ in 0..QUEUE_CAP + 10 {
            producer.push(InputEvent::ButtonPress(Button::Select));
        }
        let mut count = 0;
        while consumer.poll_event().is_some() {
            count += 1;
        }
        assert_eq!(count, QUEUE_CAP);
    }
}
