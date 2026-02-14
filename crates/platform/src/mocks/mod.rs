//! Mock implementations for testing
//!
//! This module provides mock implementations of all platform traits
//! for use in unit and integration tests.

#![cfg(any(test, feature = "std"))]

use crate::*;
use embedded_graphics::{pixelcolor::Gray4, prelude::*};

/// Mock display implementation
pub struct MockDisplay {
    width: u32,
    height: u32,
    refresh_count: usize,
    pixels: heapless::Vec<Pixel<Gray4>, 10000>,
}

impl MockDisplay {
    /// Create new mock display
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            refresh_count: 0,
            pixels: heapless::Vec::new(),
        }
    }

    /// Get refresh count
    pub fn refresh_count(&self) -> usize {
        self.refresh_count
    }

    /// Get pixels
    pub fn pixels(&self) -> &[Pixel<Gray4>] {
        &self.pixels
    }
}

impl DrawTarget for MockDisplay {
    type Color = Gray4;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for pixel in pixels {
            // Store pixels for verification
            if self.pixels.len() < self.pixels.capacity() {
                let _ = self.pixels.push(pixel);
            }
        }
        Ok(())
    }
}

impl OriginDimensions for MockDisplay {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DisplayDriver for MockDisplay {
    type Error = core::convert::Infallible;

    async fn refresh_full(&mut self) -> Result<(), Self::Error> {
        self.refresh_count += 1;
        Ok(())
    }

    async fn refresh_partial(&mut self) -> Result<(), Self::Error> {
        self.refresh_count += 1;
        Ok(())
    }

    async fn sleep(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }

    async fn wake(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}

/// Mock input device
pub struct MockInput {
    events: heapless::Deque<InputEvent, 16>,
}

impl MockInput {
    /// Create new mock input
    pub fn new() -> Self {
        Self {
            events: heapless::Deque::new(),
        }
    }

    /// Add event to queue
    pub fn add_event(&mut self, event: InputEvent) -> Result<(), InputEvent> {
        self.events.push_back(event)
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

impl Default for MockInput {
    fn default() -> Self {
        Self::new()
    }
}

impl InputDevice for MockInput {
    async fn wait_for_event(&mut self) -> InputEvent {
        loop {
            if let Some(event) = self.events.pop_front() {
                return event;
            }
            embassy_time::Timer::after_millis(10).await;
        }
    }

    fn poll_event(&mut self) -> Option<InputEvent> {
        self.events.pop_front()
    }
}

/// Mock audio codec
pub struct MockAudio {
    volume: u8,
    sample_rate: u32,
    playing: bool,
    samples_written: usize,
}

impl MockAudio {
    /// Create new mock audio codec
    pub fn new() -> Self {
        Self {
            volume: 50,
            sample_rate: 44100,
            playing: false,
            samples_written: 0,
        }
    }

    /// Get current volume
    pub fn volume(&self) -> u8 {
        self.volume
    }

    /// Check if playing
    pub fn is_playing(&self) -> bool {
        self.playing
    }

    /// Get total samples written
    pub fn samples_written(&self) -> usize {
        self.samples_written
    }
}

impl Default for MockAudio {
    fn default() -> Self {
        Self::new()
    }
}

impl AudioCodec for MockAudio {
    type Error = core::convert::Infallible;

    async fn init(&mut self, config: AudioConfig) -> Result<(), Self::Error> {
        self.sample_rate = config.sample_rate;
        Ok(())
    }

    async fn start(&mut self) -> Result<(), Self::Error> {
        self.playing = true;
        Ok(())
    }

    async fn stop(&mut self) -> Result<(), Self::Error> {
        self.playing = false;
        Ok(())
    }

    async fn set_volume(&mut self, volume: u8) -> Result<(), Self::Error> {
        self.volume = volume.min(100);
        Ok(())
    }

    async fn write_samples(&mut self, samples: &[i16]) -> Result<(), Self::Error> {
        self.samples_written += samples.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_display() {
        let mut display = MockDisplay::new(400, 300);

        display.refresh_full().await.unwrap();
        assert_eq!(display.refresh_count(), 1);

        display.refresh_partial().await.unwrap();
        assert_eq!(display.refresh_count(), 2);
    }

    #[tokio::test]
    async fn test_mock_input() {
        let mut input = MockInput::new();

        input.add_event(InputEvent::ButtonPress(Button::Play)).unwrap();
        input.add_event(InputEvent::ButtonPress(Button::Next)).unwrap();

        assert_eq!(input.poll_event(), Some(InputEvent::ButtonPress(Button::Play)));
        assert_eq!(input.poll_event(), Some(InputEvent::ButtonPress(Button::Next)));
        assert_eq!(input.poll_event(), None);
    }

    #[tokio::test]
    async fn test_mock_audio() {
        let mut audio = MockAudio::new();

        audio.init(AudioConfig::default()).await.unwrap();
        audio.set_volume(75).await.unwrap();
        assert_eq!(audio.volume(), 75);

        audio.start().await.unwrap();
        assert!(audio.is_playing());

        let samples = [0i16; 1024];
        audio.write_samples(&samples).await.unwrap();
        assert_eq!(audio.samples_written(), 1024);

        audio.stop().await.unwrap();
        assert!(!audio.is_playing());
    }
}
