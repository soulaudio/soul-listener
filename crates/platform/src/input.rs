//! Input device abstraction

/// Input device trait for buttons and encoders
pub trait InputDevice {
    /// Wait for next input event (async, power-efficient)
    fn wait_for_event(&mut self) -> impl core::future::Future<Output = InputEvent>;

    /// Poll for event (non-blocking)
    fn poll_event(&mut self) -> Option<InputEvent>;
}

/// Input events from buttons and encoders
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum InputEvent {
    /// Button pressed
    ButtonPress(Button),
    /// Button released
    ButtonRelease(Button),
    /// Button held for extended period
    ButtonLongPress(Button),
    /// Rotary encoder increment (positive = clockwise)
    RotaryIncrement(i32),
}

/// Physical buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Button {
    /// Play/Pause button
    Play,
    /// Next track
    Next,
    /// Previous track
    Previous,
    /// Volume up
    VolumeUp,
    /// Volume down
    VolumeDown,
    /// Menu button
    Menu,
    /// Back button
    Back,
    /// Select/OK button
    Select,
}
