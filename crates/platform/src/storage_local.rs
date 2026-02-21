//! Local filesystem Storage implementation for the desktop emulator.
//!
//! `LocalFileStorage` implements `platform::Storage` using `std::fs`.
//! Used when the `std` feature is enabled (emulator builds only).
//! All paths are resolved relative to the `soul_root` provided at construction.

use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use crate::storage::{File, Storage};

/// Error type for local filesystem operations.
#[derive(Debug)]
pub struct LocalStorageError(pub std::io::Error);

impl core::fmt::Display for LocalStorageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "local storage error: {}", self.0)
    }
}

impl std::error::Error for LocalStorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.0)
    }
}

/// An open file on the local filesystem.
pub struct LocalFile {
    inner: fs::File,
    size: u64,
}

impl File for LocalFile {
    type Error = LocalStorageError;

    async fn read(&mut self, buf: &mut [u8]) -> Result<usize, Self::Error> {
        Read::read(&mut self.inner, buf).map_err(LocalStorageError)
    }

    async fn seek(&mut self, pos: u64) -> Result<u64, Self::Error> {
        Seek::seek(&mut self.inner, SeekFrom::Start(pos)).map_err(LocalStorageError)
    }

    fn size(&self) -> u64 {
        self.size
    }
}

/// A `platform::Storage` implementation backed by `std::fs`.
///
/// Paths passed to [`LocalFileStorage::open_file`] and [`LocalFileStorage::exists`]
/// are resolved relative to the `soul_root` provided at construction.
///
/// # Example
/// ```no_run
/// # async fn example() {
/// use platform::storage_local::LocalFileStorage;
/// use platform::Storage;
/// let mut storage = LocalFileStorage::new("/home/user/soul");
/// let file = storage.open_file("manifest.bin").await.unwrap();
/// # }
/// ```
pub struct LocalFileStorage {
    root: PathBuf,
}

impl LocalFileStorage {
    /// Create a new storage rooted at `soul_root`.
    #[must_use]
    pub fn new(soul_root: &str) -> Self {
        Self { root: PathBuf::from(soul_root) }
    }

    /// Create from the `MUSIC_PATH` environment variable.
    ///
    /// Returns `None` if `MUSIC_PATH` is not set or is not valid UTF-8.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        std::env::var("MUSIC_PATH").ok().map(|p| Self::new(&p))
    }

    fn resolve(&self, path: &str) -> PathBuf {
        self.root.join(path)
    }
}

impl Storage for LocalFileStorage {
    type Error = LocalStorageError;
    type File = LocalFile;

    async fn open_file(&mut self, path: &str) -> Result<Self::File, Self::Error> {
        let full = self.resolve(path);
        let file = fs::File::open(&full).map_err(LocalStorageError)?;
        let meta = file.metadata().map_err(LocalStorageError)?;
        Ok(LocalFile { inner: file, size: meta.len() })
    }

    async fn exists(&mut self, path: &str) -> Result<bool, Self::Error> {
        Ok(self.resolve(path).exists())
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::storage::{File, Storage};
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn local_storage_read_full_file() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("test.bin"), b"hello world").unwrap();
        let mut storage = LocalFileStorage::new(tmp.path().to_str().unwrap());
        let mut file = storage.open_file("test.bin").await.unwrap();
        let mut buf = [0u8; 11];
        let n = file.read(&mut buf).await.unwrap();
        assert_eq!(n, 11);
        assert_eq!(&buf, b"hello world");
    }

    #[tokio::test]
    async fn local_storage_size_matches() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("size.bin"), &[0u8; 64]).unwrap();
        let mut storage = LocalFileStorage::new(tmp.path().to_str().unwrap());
        let file = storage.open_file("size.bin").await.unwrap();
        assert_eq!(file.size(), 64);
    }

    #[tokio::test]
    async fn local_storage_seek_and_read() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("seek.bin"), b"ABCDEFGH").unwrap();
        let mut storage = LocalFileStorage::new(tmp.path().to_str().unwrap());
        let mut file = storage.open_file("seek.bin").await.unwrap();
        file.seek(4).await.unwrap();
        let mut buf = [0u8; 4];
        file.read(&mut buf).await.unwrap();
        assert_eq!(&buf, b"EFGH");
    }

    #[tokio::test]
    async fn local_storage_exists_true() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("exists.bin"), b"x").unwrap();
        let mut storage = LocalFileStorage::new(tmp.path().to_str().unwrap());
        assert!(storage.exists("exists.bin").await.unwrap());
    }

    #[tokio::test]
    async fn local_storage_exists_false() {
        let tmp = TempDir::new().unwrap();
        let mut storage = LocalFileStorage::new(tmp.path().to_str().unwrap());
        assert!(!storage.exists("missing.bin").await.unwrap());
    }
}
