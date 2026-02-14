//! Bluetooth adapter abstraction

/// Bluetooth adapter trait
pub trait BluetoothAdapter {
    /// Error type
    type Error: core::fmt::Debug;

    /// Initialize adapter
    async fn init(&mut self) -> Result<(), Self::Error>;

    /// Start advertising
    async fn start_advertising(&mut self, name: &str) -> Result<(), Self::Error>;

    /// Stop advertising
    async fn stop_advertising(&mut self) -> Result<(), Self::Error>;

    /// Check if connected
    fn is_connected(&self) -> bool;
}
