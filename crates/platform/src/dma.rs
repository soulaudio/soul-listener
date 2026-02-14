//! DMA abstraction layer
//!
//! Provides safe, type-checked DMA transfers with ownership semantics.

use core::marker::PhantomData;

/// DMA channel abstraction
pub trait DmaChannel {
    /// Error type
    type Error: core::fmt::Debug;

    /// Start a transfer
    fn start(&mut self) -> Result<(), Self::Error>;

    /// Stop a transfer
    fn stop(&mut self) -> Result<(), Self::Error>;

    /// Check if transfer is complete
    fn is_complete(&self) -> bool;

    /// Get transfer count
    fn transfer_count(&self) -> usize;
}

/// DMA transfer that owns its buffer
pub struct DmaTransfer<B, C> {
    buffer: B,
    channel: C,
    _phantom: PhantomData<B>,
}

impl<B, C> DmaTransfer<B, C>
where
    C: DmaChannel,
{
    /// Create a new DMA transfer
    ///
    /// # Safety
    ///
    /// Buffer must remain valid for the duration of the transfer.
    /// No other references to the buffer may exist.
    pub unsafe fn new(buffer: B, channel: C) -> Self {
        Self {
            buffer,
            channel,
            _phantom: PhantomData,
        }
    }

    /// Start the transfer
    pub fn start(mut self) -> Result<DmaTransferActive<B, C>, C::Error> {
        self.channel.start()?;
        Ok(DmaTransferActive {
            buffer: self.buffer,
            channel: self.channel,
        })
    }
}

/// Active DMA transfer
pub struct DmaTransferActive<B, C> {
    buffer: B,
    channel: C,
}

impl<B, C> DmaTransferActive<B, C>
where
    C: DmaChannel,
{
    /// Wait for transfer to complete (blocking)
    pub fn wait(mut self) -> Result<(B, C), C::Error> {
        while !self.channel.is_complete() {
            // Yield or sleep
        }
        self.channel.stop()?;
        Ok((self.buffer, self.channel))
    }

    /// Wait for transfer to complete (async)
    pub async fn wait_async(mut self) -> Result<(B, C), C::Error> {
        while !self.channel.is_complete() {
            // Await interrupt or timer
            embassy_time::Timer::after_millis(1).await;
        }
        self.channel.stop()?;
        Ok((self.buffer, self.channel))
    }

    /// Check if complete without blocking
    pub fn is_complete(&self) -> bool {
        self.channel.is_complete()
    }

    /// Get current transfer count
    pub fn transfer_count(&self) -> usize {
        self.channel.transfer_count()
    }
}

/// DMA buffer trait
pub trait DmaBuffer {
    /// Get buffer pointer
    fn as_ptr(&self) -> *const u8;

    /// Get mutable buffer pointer
    fn as_mut_ptr(&mut self) -> *mut u8;

    /// Get buffer length
    fn len(&self) -> usize;

    /// Check if buffer is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl DmaBuffer for &[u8] {
    fn as_ptr(&self) -> *const u8 {
        (*self).as_ptr()
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        panic!("Cannot get mutable pointer from immutable slice")
    }

    fn len(&self) -> usize {
        (*self).len()
    }
}

impl DmaBuffer for &mut [u8] {
    fn as_ptr(&self) -> *const u8 {
        (**self).as_ptr()
    }

    fn as_mut_ptr(&mut self) -> *mut u8 {
        (**self).as_mut_ptr()
    }

    fn len(&self) -> usize {
        (**self).len()
    }
}

/// Circular DMA buffer
pub struct CircularBuffer<const N: usize> {
    buffer: [u8; N],
    write_pos: usize,
    read_pos: usize,
}

impl<const N: usize> CircularBuffer<N> {
    /// Create new circular buffer
    pub const fn new() -> Self {
        Self {
            buffer: [0; N],
            write_pos: 0,
            read_pos: 0,
        }
    }

    /// Get available data length
    pub fn available(&self) -> usize {
        if self.write_pos >= self.read_pos {
            self.write_pos - self.read_pos
        } else {
            N - self.read_pos + self.write_pos
        }
    }

    /// Get free space
    pub fn free_space(&self) -> usize {
        N - self.available() - 1
    }

    /// Write data
    pub fn write(&mut self, data: &[u8]) -> usize {
        let free = self.free_space();
        let to_write = data.len().min(free);

        for &byte in &data[..to_write] {
            self.buffer[self.write_pos] = byte;
            self.write_pos = (self.write_pos + 1) % N;
        }

        to_write
    }

    /// Read data
    pub fn read(&mut self, buffer: &mut [u8]) -> usize {
        let available = self.available();
        let to_read = buffer.len().min(available);

        for i in 0..to_read {
            buffer[i] = self.buffer[self.read_pos];
            self.read_pos = (self.read_pos + 1) % N;
        }

        to_read
    }
}
