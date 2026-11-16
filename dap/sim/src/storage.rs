//! `SimStorage` -- maps a queue position to a real file on disk, so the
//! headless host test runs the actual decode path. Not a production storage
//! backend (see the future `board-h7a3` SD-card `Storage` for that): just a
//! caller-supplied list of paths.

use std::fs::File;
use std::io::BufReader;
use std::path::{Path, PathBuf};

use dap_core::ports::Storage;
use embedded_io_adapters::std::FromStd;

/// The owned, file-backed, full-ladder (`BufSource` + `SeekSource`) byte
/// source `SimStorage::open` hands back.
///
/// `dap_core::ports::Storage::Source` has no lifetime parameter, so `open`
/// must return something that owns its bytes *and* its read buffer --
/// `soul_audio_core::BufReader` won't fit (it borrows a caller-provided
/// `&'buf mut [u8]`). `std::io::BufReader` owns its scratch buffer
/// internally, so wrapping it in `embedded-io-adapters`' `FromStd` climbs the
/// same `Read`/`BufRead`/`Seek` ladder with no borrow at all.
pub type FileSource = FromStd<BufReader<File>>;

/// [`SimStorage`] failure: an out-of-range track index, or the I/O error from
/// opening the file at that index's path.
#[derive(Debug)]
pub enum SimStorageError {
    /// `track` was outside the configured path list.
    OutOfRange(usize),
    /// Opening the file failed.
    Io(std::io::Error),
}

impl core::fmt::Display for SimStorageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SimStorageError::OutOfRange(track) => write!(f, "track {track} out of range"),
            SimStorageError::Io(e) => write!(f, "opening track file: {e}"),
        }
    }
}

impl std::error::Error for SimStorageError {}

/// Resolves a queue position (`track: usize`) to a path in a fixed,
/// caller-supplied list and opens it as a [`FileSource`].
pub struct SimStorage {
    paths: Vec<PathBuf>,
}

impl SimStorage {
    /// Build storage over `paths`, indexed in the given order: `open(0)` is
    /// `paths[0]`, etc.
    pub fn new<I, P>(paths: I) -> Self
    where
        I: IntoIterator<Item = P>,
        P: AsRef<Path>,
    {
        Self {
            paths: paths.into_iter().map(|p| p.as_ref().to_path_buf()).collect(),
        }
    }
}

impl Storage for SimStorage {
    type Source = FileSource;
    type Error = SimStorageError;

    fn open(&mut self, track: usize) -> Result<Self::Source, Self::Error> {
        let path = self
            .paths
            .get(track)
            .ok_or(SimStorageError::OutOfRange(track))?;
        let file = File::open(path).map_err(SimStorageError::Io)?;
        Ok(FromStd::new(BufReader::new(file)))
    }
}
