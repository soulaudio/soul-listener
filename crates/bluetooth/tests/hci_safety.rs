//! HCI serialization safety tests.
//!
//! These tests enforce that HCI serialization returns explicit errors on buffer
//! overflow rather than silently truncating packets.
//!
//! GAP-A7: heapless::Vec push().ok() in from_command() silently drops bytes
//! when a LE Audio HCI command's params exceed the 64-byte buffer capacity
//! (4 header bytes consumed, leaving only 60 bytes for params). The STM32WB55
//! then receives a malformed — truncated — HCI command and enters a broken BLE
//! state with no error logged.

// Test files legitimately use expect() for readable assertions.
#![allow(clippy::expect_used)]

/// Test that HciPacket::from_command returns an error when params would overflow
/// the 64-byte heapless Vec (4 header bytes + params must fit in 64 bytes total,
/// so max params length is 60 bytes).
#[test]
fn hci_from_command_returns_err_on_overflow() {
    use bluetooth::hci::{HciError, HciPacket, HciRawCommand};
    // 61 params + 4 header bytes = 65 bytes, overflows the 64-byte Vec
    let oversized_params = [0u8; 61];
    let result = HciPacket::from_raw_command(HciRawCommand {
        opcode: 0x2037, // LE Set Extended Advertising Data
        params: &oversized_params,
    });
    assert!(
        result.is_err(),
        "HCI serialization must return Err on buffer overflow, not silently truncate. \
         Silent truncation sends malformed commands to STM32WB55 BLE stack."
    );
    assert_eq!(
        result.unwrap_err(),
        HciError::CommandTooLong,
        "Overflow must produce HciError::CommandTooLong"
    );
}

/// Test that standard BLE advertising commands (31-byte params) succeed.
/// 4 header bytes + 31 params = 35 bytes — well within 64-byte buffer.
#[test]
fn hci_from_command_standard_advertising_succeeds() {
    use bluetooth::hci::{HciPacket, HciRawCommand};
    // Standard LE Set Advertising Data: 4 header + 31 params = 35 bytes (fits in 64)
    let params = [0u8; 31];
    let result = HciPacket::from_raw_command(HciRawCommand {
        opcode: 0x2008, // LE Set Advertising Data
        params: &params,
    });
    assert!(
        result.is_ok(),
        "Standard BLE advertising command (31-byte params) must fit in HCI buffer. Got: {:?}",
        result.err()
    );
}

/// Test that exact capacity boundary (60-byte params) succeeds.
/// 4 header bytes + 60 params = 64 bytes — exactly fills the buffer.
#[test]
fn hci_from_command_max_params_succeeds() {
    use bluetooth::hci::{HciPacket, HciRawCommand};
    let max_params = [0u8; 60];
    let result = HciPacket::from_raw_command(HciRawCommand {
        opcode: 0x2037,
        params: &max_params,
    });
    assert!(
        result.is_ok(),
        "HCI command with exactly 60-byte params (fills 64-byte buffer) must succeed. Got: {:?}",
        result.err()
    );
}

/// Test that the HciError::CommandTooLong variant exists and is usable.
#[test]
fn hci_error_command_too_long_variant_exists() {
    use bluetooth::hci::HciError;
    let err = HciError::CommandTooLong;
    // Verify Debug formatting works (requires #[derive(Debug)])
    let debug_str = format!("{:?}", err);
    assert!(
        debug_str.contains("CommandTooLong"),
        "HciError::CommandTooLong must implement Debug"
    );
}
