//! HCI (Host Controller Interface) packet framing for the STM32WB55 co-processor.
//!
//! Implements the UART transport layer defined in Bluetooth Core Spec v5.x
//! Part H, Section 4 (H4 framing). Each packet is prefixed with a 1-byte
//! packet-type indicator.
//!
//! Packet type codes:
//! - `0x01` — HCI Command packet (host → controller)
//! - `0x04` — HCI Event packet (controller → host)

#[cfg(test)]
#[allow(clippy::expect_used)] // Tests use expect() for readable assertions
mod tests {
    use super::{HciCommand, HciError, HciEvent, HciEventCode, HciPacket};

    // ---- Command tests -------------------------------------------------------

    #[test]
    fn test_hci_command_packet_opcode() {
        // BT Core Spec: Reset is OGF=0x03, OCF=0x003 → opcode = (OGF << 10) | OCF = 0x0C03
        assert_eq!(HciCommand::Reset.opcode(), 0x0C03_u16);
    }

    #[test]
    fn test_hci_command_packet_serialise_reset() {
        let pkt = HciPacket::from_command(HciCommand::Reset)
            .expect("Reset command has zero params and must always fit in HCI buffer");
        // H4 format: [packet_type=0x01, opcode_lo, opcode_hi, param_len, ...params]
        // Reset opcode 0x0C03 little-endian → [0x03, 0x0C]; param_len = 0
        assert_eq!(&pkt[..], &[0x01_u8, 0x03, 0x0C, 0x00]);
    }

    // ---- Event-code test -----------------------------------------------------

    #[test]
    fn test_hci_event_code_command_complete() {
        assert_eq!(HciEventCode::CommandComplete as u8, 0x0E);
    }

    // ---- Event parsing tests --------------------------------------------------

    #[test]
    fn test_hci_event_parse_command_complete() {
        // HCI Event packet (H4):
        //   byte 0: packet type = 0x04 (event)
        //   byte 1: event code  = 0x0E (CommandComplete)
        //   byte 2: parameter total length = 0x04
        //   byte 3: num_hci_command_packets = 0x01
        //   byte 4-5: command opcode (Reset = 0x03, 0x0C)
        //   byte 6: return_parameters[0] = status = 0x00 (success)
        let raw = [0x04_u8, 0x0E, 0x04, 0x01, 0x03, 0x0C, 0x00];
        let event = HciPacket::parse(&raw).expect("should parse successfully");
        assert_eq!(event, HciEvent::CommandComplete { status: 0x00 });
    }

    #[test]
    fn test_hci_packet_too_short_returns_err() {
        let raw = [0x04_u8]; // event type byte only — no event code
        let result = HciPacket::parse(&raw);
        assert_eq!(result, Err(HciError::PacketTooShort));
    }

    #[test]
    fn test_hci_invalid_type_returns_err() {
        let raw = [0xFF_u8, 0x00];
        let result = HciPacket::parse(&raw);
        assert_eq!(result, Err(HciError::UnknownPacketType(0xFF)));
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public types
// ─────────────────────────────────────────────────────────────────────────────

/// HCI commands the host can send to the controller.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HciCommand {
    /// HCI_Reset — OGF=0x03 (Controller & Baseband), OCF=0x0003.
    Reset,
}

impl HciCommand {
    /// Return the 16-bit opcode: `(OGF << 10) | OCF`.
    #[must_use]
    pub const fn opcode(self) -> u16 {
        match self {
            HciCommand::Reset => 0x0C03, // OGF=3, OCF=0x003
        }
    }

    /// Return the parameter bytes for this command (empty for Reset).
    #[must_use]
    pub fn params(self) -> heapless::Vec<u8, 64> {
        match self {
            HciCommand::Reset => heapless::Vec::new(),
        }
    }
}

/// HCI event codes (controller → host).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HciEventCode {
    /// CommandComplete event (0x0E): reports the result of a command.
    CommandComplete = 0x0E,
}

/// Decoded HCI events.
#[derive(Debug, PartialEq, Eq)]
pub enum HciEvent {
    /// A previously issued command completed.
    CommandComplete {
        /// Return status; `0x00` means success.
        status: u8,
    },
}

/// Errors that can occur when parsing or serializing an HCI packet.
#[derive(Debug, PartialEq, Eq)]
pub enum HciError {
    /// The byte slice is shorter than the minimum valid packet length.
    PacketTooShort,
    /// The first byte is not a recognised H4 packet-type indicator.
    UnknownPacketType(u8),
    /// The command parameters are too long to fit in the HCI buffer.
    ///
    /// heapless::Vec<u8, 64> holds 4 header bytes + max 60 param bytes.
    /// LE Audio commands can have up to 251 bytes of params — these must
    /// be rejected explicitly rather than silently truncated.
    CommandTooLong,
}

/// Zero-size marker struct that owns the HCI framing logic.
pub struct HciPacket;

/// A raw HCI command with an opcode and parameter slice.
///
/// Used by [`HciPacket::from_raw_command`] to serialize arbitrary HCI commands,
/// including LE Audio extended commands that exceed the standard 31-byte payload.
pub struct HciRawCommand<'a> {
    /// 16-bit opcode: `(OGF << 10) | OCF`.
    pub opcode: u16,
    /// Raw parameter bytes (max 60 bytes to fit in the 64-byte HCI buffer
    /// alongside the 4-byte H4 header).
    pub params: &'a [u8],
}

// Compile-time assertion: standard BLE advertising fits in HCI buffer.
// 4 header bytes + 31 param bytes = 35 bytes <= 64-byte Vec capacity.
const _: () = assert!(
    4 + 31 <= 64,
    "Standard BLE advertising must fit in HCI buffer"
);

impl HciPacket {
    /// Serialise an HCI command as an H4-framed byte vector.
    ///
    /// Format: `[0x01, opcode_lo, opcode_hi, param_len, ...params]`
    ///
    /// # Errors
    ///
    /// Returns [`HciError::CommandTooLong`] if the params exceed the available
    /// buffer space (60 bytes: 64-byte Vec minus 4 header bytes).
    pub fn from_command(cmd: HciCommand) -> Result<heapless::Vec<u8, 64>, HciError> {
        let opcode = cmd.opcode();
        let params = cmd.params();
        Self::from_raw_command(HciRawCommand {
            opcode,
            params: &params,
        })
    }

    /// Serialise a raw HCI command as an H4-framed byte vector.
    ///
    /// Format: `[0x01, opcode_lo, opcode_hi, param_len, ...params]`
    ///
    /// # Errors
    ///
    /// Returns [`HciError::CommandTooLong`] if `params.len() > 60`. The 64-byte
    /// [`heapless::Vec`] holds 4 header bytes + up to 60 param bytes. Exceeding
    /// this silently truncates the command, causing the STM32WB55 BLE stack to
    /// receive a malformed packet and enter a broken state with no error log.
    pub fn from_raw_command(cmd: HciRawCommand<'_>) -> Result<heapless::Vec<u8, 64>, HciError> {
        // 4 header bytes + params must fit in the 64-byte Vec
        const HEADER_BYTES: usize = 4;
        const MAX_PARAMS: usize = 64 - HEADER_BYTES;

        if cmd.params.len() > MAX_PARAMS {
            return Err(HciError::CommandTooLong);
        }

        let mut out: heapless::Vec<u8, 64> = heapless::Vec::new();

        out.push(0x01).map_err(|_| HciError::CommandTooLong)?; // H4 packet type: command
        out.push((cmd.opcode & 0xFF) as u8)
            .map_err(|_| HciError::CommandTooLong)?; // opcode low byte
        out.push((cmd.opcode >> 8) as u8)
            .map_err(|_| HciError::CommandTooLong)?; // opcode high byte
                                                     // SAFETY: params.len() <= MAX_PARAMS <= 60 <= 255, fits in u8.
        #[allow(clippy::cast_possible_truncation)]
        out.push(cmd.params.len() as u8)
            .map_err(|_| HciError::CommandTooLong)?; // parameter total length

        for b in cmd.params {
            out.push(*b).map_err(|_| HciError::CommandTooLong)?;
        }

        Ok(out)
    }

    /// Parse a raw H4-framed byte slice into an [`HciEvent`].
    ///
    /// # Errors
    ///
    /// Returns [`HciError::PacketTooShort`] when fewer than 2 bytes are
    /// available, or [`HciError::UnknownPacketType`] for unrecognised type
    /// indicators.
    #[allow(clippy::indexing_slicing)] // Safety: len >= 2 checked above
    pub fn parse(bytes: &[u8]) -> Result<HciEvent, HciError> {
        if bytes.len() < 2 {
            return Err(HciError::PacketTooShort);
        }

        match bytes[0] {
            0x04 => Self::parse_event(&bytes[1..]),
            other => Err(HciError::UnknownPacketType(other)),
        }
    }

    /// Parse the payload of an H4 event packet (packet-type byte already consumed).
    #[allow(clippy::indexing_slicing)] // Safety: is_empty + len < 6 guards all indexing
    fn parse_event(bytes: &[u8]) -> Result<HciEvent, HciError> {
        if bytes.is_empty() {
            return Err(HciError::PacketTooShort);
        }

        match bytes[0] {
            0x0E => {
                // CommandComplete:
                //   [event_code=0x0E, param_len, num_hci_cmds, opcode_lo, opcode_hi, status]
                // bytes[0] = 0x0E (already matched)
                // bytes[1] = param_len
                // bytes[2] = num_hci_command_packets
                // bytes[3] = opcode_lo
                // bytes[4] = opcode_hi
                // bytes[5] = status
                if bytes.len() < 6 {
                    return Err(HciError::PacketTooShort);
                }
                let status = bytes[5];
                Ok(HciEvent::CommandComplete { status })
            }
            _ => Err(HciError::PacketTooShort),
        }
    }
}
