//! GPIO and pin abstraction layer
//!
//! Provides safe, type-state based pin control with interrupt support.

use core::marker::PhantomData;

/// GPIO pin with type-state encoding
pub struct Pin<MODE> {
    pin_number: u8,
    _mode: PhantomData<MODE>,
}

/// Input mode marker
pub struct Input<PULL = Floating> {
    _pull: PhantomData<PULL>,
}

/// Output mode marker
pub struct Output<MODE = PushPull> {
    _mode: PhantomData<MODE>,
}

/// Analog mode marker
pub struct Analog;

/// Pull-up configuration
pub struct PullUp;

/// Pull-down configuration
pub struct PullDown;

/// Floating (no pull resistor)
pub struct Floating;

/// Push-pull output
pub struct PushPull;

/// Open-drain output
pub struct OpenDrain;

/// Pin mode trait
pub trait PinMode {}

impl<PULL> PinMode for Input<PULL> {}
impl<MODE> PinMode for Output<MODE> {}
impl PinMode for Analog {}

/// Input pin operations
pub trait InputPin {
    /// Error type
    type Error;

    /// Read pin state
    fn is_high(&self) -> Result<bool, Self::Error>;

    /// Read pin state (inverted)
    fn is_low(&self) -> Result<bool, Self::Error> {
        self.is_high().map(|v| !v)
    }
}

/// Output pin operations
pub trait OutputPin {
    /// Error type
    type Error;

    /// Set pin high
    fn set_high(&mut self) -> Result<(), Self::Error>;

    /// Set pin low
    fn set_low(&mut self) -> Result<(), Self::Error>;

    /// Set pin state
    fn set_state(&mut self, state: PinState) -> Result<(), Self::Error> {
        match state {
            PinState::High => self.set_high(),
            PinState::Low => self.set_low(),
        }
    }

    /// Toggle pin state
    fn toggle(&mut self) -> Result<(), Self::Error>;
}

/// Pin state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum PinState {
    /// High (logic 1)
    High,
    /// Low (logic 0)
    Low,
}

impl From<bool> for PinState {
    fn from(value: bool) -> Self {
        if value {
            Self::High
        } else {
            Self::Low
        }
    }
}

impl From<PinState> for bool {
    fn from(value: PinState) -> Self {
        matches!(value, PinState::High)
    }
}

/// External interrupt configuration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum InterruptMode {
    /// Trigger on rising edge
    RisingEdge,
    /// Trigger on falling edge
    FallingEdge,
    /// Trigger on both edges
    BothEdges,
}

/// Pin with interrupt capability
pub trait InterruptPin: InputPin {
    /// Enable interrupt
    fn enable_interrupt(&mut self, mode: InterruptMode) -> Result<(), Self::Error>;

    /// Disable interrupt
    fn disable_interrupt(&mut self) -> Result<(), Self::Error>;

    /// Wait for interrupt (async)
    fn wait_for_interrupt(&mut self) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Clear interrupt flag
    fn clear_interrupt(&mut self) -> Result<(), Self::Error>;
}

/// Typestate transitions
impl<MODE> Pin<MODE> {
    /// Convert to input mode
    pub fn into_input<PULL>(self) -> Pin<Input<PULL>> {
        Pin {
            pin_number: self.pin_number,
            _mode: PhantomData,
        }
    }

    /// Convert to output mode
    pub fn into_output<MODE2>(self) -> Pin<Output<MODE2>> {
        Pin {
            pin_number: self.pin_number,
            _mode: PhantomData,
        }
    }

    /// Convert to analog mode
    pub fn into_analog(self) -> Pin<Analog> {
        Pin {
            pin_number: self.pin_number,
            _mode: PhantomData,
        }
    }
}

/// Pin group for efficient multi-pin operations
pub trait PinGroup {
    /// Error type
    type Error;

    /// Read all pins at once
    fn read(&self) -> Result<u32, Self::Error>;

    /// Write all pins at once
    fn write(&mut self, value: u32) -> Result<(), Self::Error>;

    /// Set specific pins high
    fn set_high(&mut self, mask: u32) -> Result<(), Self::Error>;

    /// Set specific pins low
    fn set_low(&mut self, mask: u32) -> Result<(), Self::Error>;
}
