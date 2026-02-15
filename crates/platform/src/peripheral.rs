//! Peripheral abstraction layer
//!
//! Provides trait-based abstractions for common peripherals (SPI, I2C, UART).
//! Wraps embedded-hal traits with additional functionality.

/// SPI peripheral abstraction
pub trait SpiPeripheral {
    /// Error type
    type Error: core::fmt::Debug;

    /// Transfer data (full duplex)
    fn transfer(
        &mut self,
        read: &mut [u8],
        write: &[u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Write data (half duplex)
    fn write(&mut self, data: &[u8])
        -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Read data (half duplex)
    fn read(
        &mut self,
        buffer: &mut [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Configure SPI mode and frequency
    fn configure(&mut self, config: SpiConfig) -> Result<(), Self::Error>;
}

/// SPI configuration
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct SpiConfig {
    /// Clock frequency in Hz
    pub frequency: u32,
    /// SPI mode (CPOL, CPHA)
    pub mode: SpiMode,
    /// Bit order
    pub bit_order: BitOrder,
}

/// SPI modes (CPOL, CPHA)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum SpiMode {
    /// Mode 0: CPOL=0, CPHA=0
    Mode0,
    /// Mode 1: CPOL=0, CPHA=1
    Mode1,
    /// Mode 2: CPOL=1, CPHA=0
    Mode2,
    /// Mode 3: CPOL=1, CPHA=1
    Mode3,
}

/// Bit order
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum BitOrder {
    /// Most significant bit first
    MsbFirst,
    /// Least significant bit first
    LsbFirst,
}

/// I2C peripheral abstraction
pub trait I2cPeripheral {
    /// Error type
    type Error: core::fmt::Debug;

    /// Write to device
    fn write(
        &mut self,
        address: u8,
        data: &[u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Read from device
    fn read(
        &mut self,
        address: u8,
        buffer: &mut [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Write then read (repeated start)
    fn write_read(
        &mut self,
        address: u8,
        write: &[u8],
        read: &mut [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Configure I2C speed
    fn configure(&mut self, config: I2cConfig) -> Result<(), Self::Error>;
}

/// I2C configuration
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct I2cConfig {
    /// Clock frequency in Hz
    pub frequency: u32,
    /// Addressing mode
    pub address_mode: AddressMode,
}

/// I2C addressing mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum AddressMode {
    /// 7-bit addressing
    SevenBit,
    /// 10-bit addressing
    TenBit,
}

/// UART peripheral abstraction
pub trait UartPeripheral {
    /// Error type
    type Error: core::fmt::Debug;

    /// Write data
    fn write(&mut self, data: &[u8])
        -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Read data
    fn read(
        &mut self,
        buffer: &mut [u8],
    ) -> impl core::future::Future<Output = Result<(), Self::Error>>;

    /// Write single byte
    fn write_byte(
        &mut self,
        byte: u8,
    ) -> impl core::future::Future<Output = Result<(), Self::Error>> {
        async move { self.write(&[byte]).await }
    }

    /// Read single byte
    fn read_byte(&mut self) -> impl core::future::Future<Output = Result<u8, Self::Error>> {
        async move {
            let mut buf = [0u8];
            self.read(&mut buf).await?;
            Ok(buf[0])
        }
    }

    /// Configure UART
    fn configure(&mut self, config: UartConfig) -> Result<(), Self::Error>;
}

/// UART configuration
#[derive(Debug, Clone, Copy)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub struct UartConfig {
    /// Baud rate
    pub baud_rate: u32,
    /// Data bits
    pub data_bits: DataBits,
    /// Parity
    pub parity: Parity,
    /// Stop bits
    pub stop_bits: StopBits,
}

/// Data bits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum DataBits {
    /// 5 data bits
    Five,
    /// 6 data bits
    Six,
    /// 7 data bits
    Seven,
    /// 8 data bits
    Eight,
    /// 9 data bits
    Nine,
}

/// Parity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum Parity {
    /// No parity
    None,
    /// Even parity
    Even,
    /// Odd parity
    Odd,
}

/// Stop bits
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum StopBits {
    /// 1 stop bit
    One,
    /// 1.5 stop bits
    OnePointFive,
    /// 2 stop bits
    Two,
}
