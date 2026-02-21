//! SDMMC-backed Storage stub for the hardware target.
//!
//! This is a placeholder that compiles but always returns `NotImplemented`.
//! The full implementation requires Embassy SDMMC init (blocked on PLL1Q config).
//!
//! # TODO
//! Replace the stub body in `open_file` and `exists` with Embassy SDMMC calls
//! once `firmware::boot::build_embassy_config()` configures the SDMMC clock.

use crate::storage::{File, Storage};

/// Error type for SDMMC storage operations.
#[derive(Debug)]
pub enum SdmmcError {
    /// This stub operation is not yet implemented.
    NotImplemented,
    /// Underlying SDMMC I/O error — will wrap `embassy_stm32::sdmmc::Error` once implemented.
    Io,
}

impl core::fmt::Display for SdmmcError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotImplemented => f.write_str("SDMMC not yet implemented"),
            Self::Io => f.write_str("SDMMC I/O error"),
        }
    }
}

/// Placeholder file for SDMMC (stub — always returns `NotImplemented`).
pub struct SdmmcFile;

impl File for SdmmcFile {
    type Error = SdmmcError;

    async fn read(&mut self, _buf: &mut [u8]) -> Result<usize, Self::Error> {
        Err(SdmmcError::NotImplemented)
    }

    async fn seek(&mut self, _pos: u64) -> Result<u64, Self::Error> {
        Err(SdmmcError::NotImplemented)
    }

    fn size(&self) -> u64 {
        0
    }
}

/// SDMMC-backed Storage — stub implementation.
///
/// Construct with `SdmmcStorage::new(sdmmc_peripheral)` once Embassy SDMMC
/// is wired.  For now, all operations return `SdmmcError::NotImplemented`.
pub struct SdmmcStorage;

impl SdmmcStorage {
    /// Create a new (stub) SDMMC storage instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl Default for SdmmcStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl Storage for SdmmcStorage {
    type Error = SdmmcError;
    type File = SdmmcFile;

    async fn open_file(&mut self, _path: &str) -> Result<Self::File, Self::Error> {
        Err(SdmmcError::NotImplemented)
    }

    async fn exists(&mut self, _path: &str) -> Result<bool, Self::Error> {
        Err(SdmmcError::NotImplemented)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn sdmmc_error_is_debug() {
        let e = SdmmcError::NotImplemented;
        // use the Debug format via alloc::format to verify the derive
        assert!(format!("{e:?}").contains("NotImplemented"));
    }

    #[test]
    fn sdmmc_storage_default_is_new() {
        // Type check: Default is implemented
        let _s: SdmmcStorage = SdmmcStorage::default();
    }
}
