//! Display abstraction layer

use embedded_graphics::prelude::*;

/// Display driver trait for E-ink displays
pub trait DisplayDriver: DrawTarget {
    /// Error type for display operations
    type DriverError: core::fmt::Debug;

    /// Perform full display refresh (clears ghosting, slow)
    fn refresh_full(&mut self)
        -> impl core::future::Future<Output = Result<(), Self::DriverError>>;

    /// Perform partial refresh (fast, may have ghosting)
    fn refresh_partial(
        &mut self,
    ) -> impl core::future::Future<Output = Result<(), Self::DriverError>>;

    /// Enter deep sleep mode
    fn sleep(&mut self) -> impl core::future::Future<Output = Result<(), Self::DriverError>>;

    /// Wake from sleep mode
    fn wake(&mut self) -> impl core::future::Future<Output = Result<(), Self::DriverError>>;

    /// Get display dimensions
    fn dimensions(&self) -> Size {
        self.bounding_box().size
    }
}

/// E-ink specific extensions
pub trait EinkDisplay: DisplayDriver {
    /// Set refresh mode
    fn set_refresh_mode(&mut self, mode: RefreshMode);

    /// Get current temperature (affects refresh quality)
    fn temperature(&self) -> Option<i8>;
}

/// E-ink refresh modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshMode {
    /// Full refresh - best quality, clears ghosting
    Full,
    /// Partial refresh - fast, some ghosting
    Partial,
    /// Fast refresh - fastest, most ghosting
    Fast,
}

/// Display errors
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DisplayError {
    /// Communication error
    Communication,
    /// Display busy
    Busy,
    /// Invalid state
    InvalidState,
    /// Timeout
    Timeout,
}

#[cfg(feature = "std")]
impl std::error::Error for DisplayError {}

impl core::fmt::Display for DisplayError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Communication => write!(f, "Display communication error"),
            Self::Busy => write!(f, "Display is busy"),
            Self::InvalidState => write!(f, "Display in invalid state"),
            Self::Timeout => write!(f, "Display operation timeout"),
        }
    }
}
