//! Storage abstraction for file systems

/// Storage trait for file system access
pub trait Storage {
    /// Error type
    type Error: core::fmt::Debug;
    /// File type
    type File: File;

    /// Open file for reading
    fn open_file(
        &mut self,
        path: &str,
    ) -> impl core::future::Future<Output = Result<Self::File, Self::Error>>;

    /// Check if path exists
    fn exists(
        &mut self,
        path: &str,
    ) -> impl core::future::Future<Output = Result<bool, Self::Error>>;
}

/// File trait for reading files
pub trait File {
    /// Error type
    type Error: core::fmt::Debug;

    /// Read from current position
    fn read(
        &mut self,
        buf: &mut [u8],
    ) -> impl core::future::Future<Output = Result<usize, Self::Error>>;

    /// Seek to position
    fn seek(&mut self, pos: u64) -> impl core::future::Future<Output = Result<u64, Self::Error>>;

    /// Get file size
    fn size(&self) -> u64;
}
