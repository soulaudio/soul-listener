//! Const-generic, stack-allocated ring buffer for PCM audio samples.
//!
//! `RingBuffer<N>` stores up to `N` `i32` samples without heap allocation.
//! It is a single-producer / single-consumer (SPSC) structure intended for
//! use between the decode task (writer) and the DMA-feed task (reader).
//!
//! # Constraints
//!
//! - `N` must be a power of two for efficient index wrapping (enforced at
//!   runtime by a debug assertion; the API is otherwise correct for any `N`).
//! - `no_std`, no `heapless` — the backing store lives entirely on the stack
//!   or in a `static`.
//! - This implementation is **not** interrupt-safe or `Send`.  Concurrent
//!   access from different Embassy tasks must be protected with a `Mutex`.

/// A fixed-capacity ring buffer for `i32` audio samples.
///
/// Capacity is set at compile time via the const generic `N`.
pub struct RingBuffer<const N: usize> {
    buf: [i32; N],
    /// Index of the next slot to read from.
    read: usize,
    /// Index of the next slot to write to.
    write: usize,
    /// Number of valid samples currently held.
    count: usize,
}

impl<const N: usize> RingBuffer<N> {
    /// Create a new, empty ring buffer.
    ///
    /// This function is `const` so that ring buffers may be stored in
    /// `static` variables without a runtime initialiser.
    pub const fn new() -> Self {
        Self {
            buf: [0i32; N],
            read: 0,
            write: 0,
            count: 0,
        }
    }

    /// Write a slice of samples into the buffer.
    ///
    /// # Errors
    ///
    /// Returns `Err(())` if the slice would not fit in the remaining capacity.
    /// The buffer is left unchanged on error (the write is all-or-nothing).
    #[allow(clippy::result_unit_err)] // overflow is the only error; () is sufficient
    #[allow(clippy::indexing_slicing)] // Safety: write < N invariant; data.len() <= N - count checked above
    #[allow(clippy::arithmetic_side_effects)] // Safety: ring buffer wrap via % N; count += data.len() <= N
    pub fn write_slice(&mut self, data: &[i32]) -> Result<(), ()> {
        if data.len() > N - self.count {
            return Err(());
        }
        for &sample in data {
            self.buf[self.write] = sample;
            self.write = (self.write + 1) % N;
        }
        self.count += data.len();
        Ok(())
    }

    /// Read up to `out.len()` samples from the buffer into `out`.
    ///
    /// Returns the number of samples actually read (may be less than
    /// `out.len()` if the buffer contains fewer samples than requested).
    #[allow(clippy::indexing_slicing)] // Safety: read < N invariant; only reads up to self.count samples
    #[allow(clippy::arithmetic_side_effects)] // Safety: ring buffer wrap via % N; count -= n where n <= count
    pub fn read_slice(&mut self, out: &mut [i32]) -> usize {
        let n = out.len().min(self.count);
        for slot in out.iter_mut().take(n) {
            *slot = self.buf[self.read];
            self.read = (self.read + 1) % N;
        }
        self.count -= n;
        n
    }

    /// Number of samples currently available to read.
    pub fn available(&self) -> usize {
        self.count
    }

    /// Maximum number of samples the buffer can hold.
    pub const fn capacity(&self) -> usize {
        N
    }

    /// `true` when no samples are present.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// `true` when the buffer is completely full.
    pub fn is_full(&self) -> bool {
        self.count == N
    }
}

impl<const N: usize> Default for RingBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}
