//! `storage-fat32` — the real [`dap_core::ports::Storage`] adapter: open audio
//! files from a FAT32 SD card and hand each one back as the no_std
//! `BufSource + SeekSource` ladder soul-audio-core decodes.
//!
//! # Ownership design (why this is safe, lifetime-free, and alloc-free)
//!
//! `dap_core::ports::Storage::Source` has NO lifetime parameter, yet reading an
//! SD file needs the `embedded_sdmmc::VolumeManager`. The reconciliation: the
//! VolumeManager lives in a `StaticCell` (the board already uses that pattern
//! everywhere), and because embedded-sdmmc 0.10's VolumeManager methods are all
//! `&self` (interior-mutable) with `Copy` [`RawFile`] handles, both [`SdStorage`]
//! and the returned [`SdSource`] hold a shared `&'static VolumeManager`. The
//! `'static` is baked into the concrete types, so no lifetime leaks into the
//! port, and there is **no `unsafe` and no self-reference**. [`SdSource`] closes
//! its [`RawFile`] on `Drop`, so sequential track opens don't leak handles.
//!
//! The block device is generic: the board supplies the STM32 SDMMC peripheral
//! (Phase E); the tests supply a RAM-backed one over a FAT32 image.
#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use dap_core::ports::Storage;
use embedded_io::{BufRead, ErrorKind, ErrorType, Read, Seek, SeekFrom};
use embedded_sdmmc::{
    BlockDevice, Mode, RawDirectory, RawFile, TimeSource, VolumeIdx, VolumeManager,
};

/// Errors from the FAT32 storage adapter. Deliberately non-generic (the specific
/// block-device error is collapsed) so it satisfies both [`embedded_io::Error`]
/// (for the `Source` ladder) and [`Storage::Error`] without a type parameter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdError {
    /// `track` was outside the configured name list.
    OutOfRange,
    /// A FAT/SD filesystem or block-device error (open/read/seek/length).
    Fs,
    /// A seek resolved to a position outside `[0, file_len]`.
    SeekOutOfBounds,
}

impl core::fmt::Display for SdError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            SdError::OutOfRange => write!(f, "track index out of range"),
            SdError::Fs => write!(f, "FAT32/SD filesystem error"),
            SdError::SeekOutOfBounds => write!(f, "seek out of bounds"),
        }
    }
}

impl embedded_io::Error for SdError {
    fn kind(&self) -> ErrorKind {
        match self {
            SdError::OutOfRange | SdError::SeekOutOfBounds => ErrorKind::InvalidInput,
            SdError::Fs => ErrorKind::Other,
        }
    }
}

/// Internal read buffer size — one FAT sector. `fill_buf`/`read` refill a sector
/// at a time; the decoders do their own larger-grained buffering on top.
const CAP: usize = 512;

/// A seekable, bufferable byte source over one open FAT32 file. Reads go through
/// the shared `&'static VolumeManager` via the file's [`RawFile`] handle; a small
/// internal buffer backs [`BufRead`]. Closes the file on `Drop`.
pub struct SdSource<D: BlockDevice + 'static, T: TimeSource + 'static> {
    vm: &'static VolumeManager<D, T>,
    file: RawFile,
    buf: [u8; CAP],
    /// File offset of `buf[0]`.
    buf_start: u32,
    /// Valid bytes in `buf`.
    buf_len: usize,
    /// Consumed bytes within `buf` (`buf_cur <= buf_len`).
    buf_cur: usize,
}

impl<D: BlockDevice + 'static, T: TimeSource + 'static> SdSource<D, T> {
    /// Logical read position = start of the buffer + bytes consumed from it.
    fn pos(&self) -> u32 {
        self.buf_start + self.buf_cur as u32
    }

    /// Refill `buf` from the underlying file. Precondition: the buffer is fully
    /// consumed (`buf_cur == buf_len`), so the underlying position is already at
    /// `buf_start + buf_len`. Sets `buf_len = 0` on EOF.
    fn refill(&mut self) -> Result<(), SdError> {
        self.buf_start += self.buf_len as u32;
        self.buf_cur = 0;
        self.buf_len = 0;
        let n = self
            .vm
            .read(self.file, &mut self.buf)
            .map_err(|_| SdError::Fs)?;
        self.buf_len = n;
        Ok(())
    }
}

impl<D: BlockDevice + 'static, T: TimeSource + 'static> Drop for SdSource<D, T> {
    fn drop(&mut self) {
        let _ = self.vm.close_file(self.file);
    }
}

impl<D: BlockDevice + 'static, T: TimeSource + 'static> ErrorType for SdSource<D, T> {
    type Error = SdError;
}

impl<D: BlockDevice + 'static, T: TimeSource + 'static> Read for SdSource<D, T> {
    fn read(&mut self, out: &mut [u8]) -> Result<usize, SdError> {
        let mut written = 0;
        while written < out.len() {
            if self.buf_cur == self.buf_len {
                self.refill()?;
                if self.buf_len == 0 {
                    break; // EOF
                }
            }
            let n = (self.buf_len - self.buf_cur).min(out.len() - written);
            out[written..written + n].copy_from_slice(&self.buf[self.buf_cur..self.buf_cur + n]);
            self.buf_cur += n;
            written += n;
        }
        Ok(written)
    }
}

impl<D: BlockDevice + 'static, T: TimeSource + 'static> BufRead for SdSource<D, T> {
    fn fill_buf(&mut self) -> Result<&[u8], SdError> {
        if self.buf_cur == self.buf_len {
            self.refill()?;
        }
        Ok(&self.buf[self.buf_cur..self.buf_len])
    }

    fn consume(&mut self, amt: usize) {
        self.buf_cur = (self.buf_cur + amt).min(self.buf_len);
    }
}

impl<D: BlockDevice + 'static, T: TimeSource + 'static> Seek for SdSource<D, T> {
    fn seek(&mut self, pos: SeekFrom) -> Result<u64, SdError> {
        let len = i64::from(self.vm.file_length(self.file).map_err(|_| SdError::Fs)?);
        let target = match pos {
            SeekFrom::Start(n) => n as i64,
            SeekFrom::End(d) => len + d,
            SeekFrom::Current(d) => i64::from(self.pos()) + d,
        };
        if target < 0 || target > len {
            return Err(SdError::SeekOutOfBounds);
        }
        let target = target as u32;
        self.vm
            .file_seek_from_start(self.file, target)
            .map_err(|_| SdError::Fs)?;
        self.buf_start = target;
        self.buf_len = 0;
        self.buf_cur = 0;
        Ok(u64::from(target))
    }
}

/// FAT32 SD-card storage: resolves a queue position to a root-directory file name
/// and opens it as an [`SdSource`]. The name list is the minimal first resolver
/// (mirrors `dap-sim`'s `SimStorage`); a later increment resolves `track` through
/// the on-card `.soulidx` catalog (`soul-audio-index`) instead.
pub struct SdStorage<D: BlockDevice + 'static, T: TimeSource + 'static> {
    vm: &'static VolumeManager<D, T>,
    root: RawDirectory,
    names: &'static [&'static str],
}

impl<D: BlockDevice + 'static, T: TimeSource + 'static> SdStorage<D, T> {
    /// Mount FAT32 volume `volume_idx` from the shared `&'static VolumeManager`
    /// and index files in the root directory by `names` (`open(0)` = `names[0]`).
    ///
    /// # Errors
    /// [`SdError::Fs`] if the volume or root directory can't be opened.
    pub fn new(
        vm: &'static VolumeManager<D, T>,
        volume_idx: VolumeIdx,
        names: &'static [&'static str],
    ) -> Result<Self, SdError> {
        let volume = vm.open_raw_volume(volume_idx).map_err(|_| SdError::Fs)?;
        let root = vm.open_root_dir(volume).map_err(|_| SdError::Fs)?;
        Ok(Self { vm, root, names })
    }
}

impl<D: BlockDevice + 'static, T: TimeSource + 'static> Storage for SdStorage<D, T> {
    type Source = SdSource<D, T>;
    type Error = SdError;

    fn open(&mut self, track: usize) -> Result<Self::Source, SdError> {
        let name = *self.names.get(track).ok_or(SdError::OutOfRange)?;
        let file = self
            .vm
            .open_file_in_dir(self.root, name, Mode::ReadOnly)
            .map_err(|_| SdError::Fs)?;
        Ok(SdSource {
            vm: self.vm,
            file,
            buf: [0; CAP],
            buf_start: 0,
            buf_len: 0,
            buf_cur: 0,
        })
    }
}
