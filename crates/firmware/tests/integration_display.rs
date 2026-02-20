//! Display integration tests — verify the display driver stack.
//!
//! Verifies public constants and driver behaviour accessible from outside
//! the firmware crate.
//!
//! Note: `Ssd1677::y_to_ram` is `pub(crate)` and therefore not accessible
//! from integration tests. Y-reversal correctness is covered by unit tests in
//! `crates/firmware/src/display/driver.rs`. The constants verified here are
//! the public surface used by application code.
//!
//! Run with: cargo test -p firmware --test integration_display

use embedded_hal_mock::eh1::delay::NoopDelay;
use embedded_hal_mock::eh1::digital::{
    Mock as PinMock, State as PinState, Transaction as PinTransaction,
};
use embedded_hal_mock::eh1::spi::{Mock as SpiMock, Transaction as SpiTransaction};
use firmware::{DisplayError, Ssd1677, DISPLAY_HEIGHT, DISPLAY_WIDTH, FRAMEBUFFER_SIZE};
use platform::DisplayDriver;

// ---------------------------------------------------------------------------
// Test: display constants correct for GDEM0397T81P
// ---------------------------------------------------------------------------

/// Verify display constants are correct for GDEM0397T81P
///
/// 800×480, 1bpp: 800*480/8 = 48000 bytes
#[test]
fn test_display_constants() {
    assert_eq!(DISPLAY_WIDTH, 800, "GDEM0397T81P is 800 pixels wide");
    assert_eq!(DISPLAY_HEIGHT, 480, "GDEM0397T81P is 480 pixels tall");
    // 1bpp: 800*480/8 = 48000 bytes
    assert_eq!(
        FRAMEBUFFER_SIZE, 48_000,
        "1bpp framebuffer must be 48000 bytes"
    );
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build the three SPI expectations that correspond to one `spi.write(&data)` call
/// via the `SpiDevice` trait.
fn spi_device_write(data: &[u8]) -> [SpiTransaction<u8>; 3] {
    [
        SpiTransaction::transaction_start(),
        SpiTransaction::write_vec(data.to_vec()),
        SpiTransaction::transaction_end(),
    ]
}

/// Create an idle pin mock that expects no transactions.
fn idle_pin() -> PinMock {
    PinMock::new(&[])
}

// ---------------------------------------------------------------------------
// Test: update_buffer rejects wrong-size framebuffer
// ---------------------------------------------------------------------------

/// Verify that `update_buffer` returns `InvalidBuffer` when given the wrong size.
///
/// This exercises the public API surface of `Ssd1677` from outside the crate.
#[tokio::test]
async fn test_update_buffer_rejects_wrong_size() {
    let mut spi_h = SpiMock::new(&[]);
    let mut dc_h = idle_pin();
    let mut rst_h = idle_pin();
    let mut busy_h = idle_pin();

    let mut drv = Ssd1677::new(
        spi_h.clone(),
        dc_h.clone(),
        rst_h.clone(),
        busy_h.clone(),
        NoopDelay,
    );

    // A buffer that is too short (100 bytes instead of 48000)
    let short_buf = [0u8; 100];
    let result = drv.update_buffer(&short_buf).await;
    assert_eq!(
        result,
        Err(DisplayError::InvalidBuffer),
        "wrong-size buffer must return InvalidBuffer"
    );

    spi_h.done();
    dc_h.done();
    rst_h.done();
    busy_h.done();
}

// ---------------------------------------------------------------------------
// Test: deep sleep command issues correct SPI bytes
// ---------------------------------------------------------------------------

/// Verify that `sleep()` emits the DeepSleep command byte (0x10) with
/// data byte 0x01 (preserve RAM), observable from outside the crate.
#[tokio::test]
async fn test_sleep_emits_deep_sleep_command() {
    // DeepSleep = 0x10, data = 0x01
    let spi_expectations: Vec<SpiTransaction<u8>> = [
        &spi_device_write(&[0x10_u8]) as &[_], // DeepSleep command byte
        &spi_device_write(&[0x01]),            // data: preserve RAM
    ]
    .iter()
    .flat_map(|s| s.iter().cloned())
    .collect();

    let dc_expectations = [
        PinTransaction::set(PinState::Low),  // DC low for command
        PinTransaction::set(PinState::High), // DC high for data
    ];

    let mut spi = SpiMock::new(&spi_expectations);
    let mut dc = PinMock::new(&dc_expectations);
    let mut rst = idle_pin();
    let mut busy = idle_pin();

    let mut drv = Ssd1677::new(
        spi.clone(),
        dc.clone(),
        rst.clone(),
        busy.clone(),
        NoopDelay,
    );
    drv.sleep().await.expect("sleep() must succeed");

    spi.done();
    dc.done();
    rst.done();
    busy.done();
}

// ---------------------------------------------------------------------------
// Test: wait_busy timeout via public wait_ready
// ---------------------------------------------------------------------------

/// Verify that `wait_ready()` eventually returns `Err(Timeout)` when the
/// BUSY pin never goes low.  Uses the public `DisplayDriver::wait_ready`
/// method which delegates to the internal `wait_busy`.
#[tokio::test]
async fn test_wait_ready_times_out() {
    // 200 × HIGH, no trailing LOW — pin never deasserts
    let busy_txns: Vec<PinTransaction> = (0..200)
        .map(|_| PinTransaction::get(PinState::High))
        .collect();

    let mut spi_h = SpiMock::new(&[]);
    let mut dc_h = idle_pin();
    let mut rst_h = idle_pin();
    let mut busy_h = PinMock::new(&busy_txns);

    let mut drv = Ssd1677::new(
        spi_h.clone(),
        dc_h.clone(),
        rst_h.clone(),
        busy_h.clone(),
        NoopDelay,
    );

    let result = drv.wait_ready().await;
    assert_eq!(
        result,
        Err(DisplayError::Timeout),
        "wait_ready must return Timeout when BUSY never deasserts"
    );

    spi_h.done();
    dc_h.done();
    rst_h.done();
    busy_h.done();
}

// ---------------------------------------------------------------------------
// Test: DisplayError formatting (std only)
// ---------------------------------------------------------------------------

/// Verify all error variants have non-empty Display strings.
#[test]
fn test_display_error_variants_have_descriptions() {
    use std::string::ToString;

    let variants = [
        DisplayError::Communication,
        DisplayError::Gpio,
        DisplayError::Busy,
        DisplayError::Timeout,
        DisplayError::InvalidState,
        DisplayError::InvalidBuffer,
        DisplayError::InvalidCoordinate,
        DisplayError::Unsupported,
    ];

    for variant in variants {
        let s = variant.to_string();
        assert!(
            !s.is_empty(),
            "DisplayError::{variant:?} must have a non-empty Display string"
        );
    }
}
