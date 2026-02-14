//! Power management abstraction
//!
//! Provides interfaces for sleep modes, clock gating, and voltage scaling.

/// Power management interface
pub trait PowerManager {
    /// Error type
    type Error: core::fmt::Debug;

    /// Enter sleep mode
    fn enter_sleep(&mut self, mode: SleepMode) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Wake from sleep
    fn wake(&mut self) -> Result<(), Self::Error>;

    /// Set voltage scaling
    fn set_voltage_scale(&mut self, scale: VoltageScale) -> Result<(), Self::Error>;

    /// Enable peripheral clock
    fn enable_peripheral_clock(&mut self, peripheral: Peripheral) -> Result<(), Self::Error>;

    /// Disable peripheral clock
    fn disable_peripheral_clock(&mut self, peripheral: Peripheral) -> Result<(), Self::Error>;

    /// Get current power consumption (if available)
    fn current_consumption(&self) -> Option<u32> {
        None
    }
}

/// Sleep modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SleepMode {
    /// Idle mode (CPU stopped, peripherals running)
    Idle,
    /// Sleep mode (CPU and some peripherals stopped)
    Sleep,
    /// Stop mode (most peripherals stopped, RAM retained)
    Stop,
    /// Standby mode (deep sleep, minimal power, RAM lost)
    Standby,
}

/// Voltage scaling modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum VoltageScale {
    /// Scale 1 (highest performance, highest power)
    Scale1,
    /// Scale 2 (medium performance)
    Scale2,
    /// Scale 3 (low performance, low power)
    Scale3,
}

/// Peripheral identifiers for clock gating
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Peripheral {
    /// SPI1
    Spi1,
    /// SPI2
    Spi2,
    /// I2C1
    I2c1,
    /// I2C2
    I2c2,
    /// UART1
    Uart1,
    /// UART2
    Uart2,
    /// SAI (audio)
    Sai,
    /// SDMMC
    Sdmmc,
    /// DMA1
    Dma1,
    /// DMA2
    Dma2,
}

/// Wake-up source
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum WakeSource {
    /// GPIO pin interrupt
    Gpio(u8),
    /// RTC alarm
    RtcAlarm,
    /// Timer
    Timer,
    /// UART activity
    Uart,
}

/// Power state monitor
pub trait PowerMonitor {
    /// Get battery voltage (mV)
    fn battery_voltage(&self) -> Option<u16>;

    /// Get battery percentage (0-100)
    fn battery_percentage(&self) -> Option<u8>;

    /// Check if charging
    fn is_charging(&self) -> bool;

    /// Check if USB power connected
    fn is_usb_connected(&self) -> bool;
}
