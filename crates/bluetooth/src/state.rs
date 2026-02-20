//! Bluetooth connection state tracker.

/// Tracks whether a BLE peer is currently connected and, if so, its address.
pub struct BluetoothState {
    connected: bool,
    peer_address: Option<[u8; 6]>,
}

impl BluetoothState {
    /// Create a new, disconnected state.
    pub fn new() -> Self {
        BluetoothState {
            connected: false,
            peer_address: None,
        }
    }

    /// Record a successful connection from `address`.
    pub fn on_connected(&mut self, address: [u8; 6]) {
        self.connected = true;
        self.peer_address = Some(address);
    }

    /// Record that the peer has disconnected.
    pub fn on_disconnected(&mut self) {
        self.connected = false;
        self.peer_address = None;
    }

    /// Returns `true` if a peer is currently connected.
    #[must_use]
    pub fn connected(&self) -> bool {
        self.connected
    }

    /// Returns the peer's 6-byte Bluetooth address, or `None` when disconnected.
    #[must_use]
    pub fn peer_address(&self) -> Option<[u8; 6]> {
        self.peer_address
    }
}

impl Default for BluetoothState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::BluetoothState;

    #[test]
    fn test_bt_starts_disconnected() {
        let state = BluetoothState::new();
        assert!(!state.connected());
    }

    #[test]
    fn test_bt_connect() {
        let mut state = BluetoothState::new();
        state.on_connected([0x01; 6]);
        assert!(state.connected());
    }

    #[test]
    fn test_bt_disconnect() {
        let mut state = BluetoothState::new();
        state.on_connected([0x01; 6]);
        state.on_disconnected();
        assert!(!state.connected());
    }

    #[test]
    fn test_bt_peer_address_after_connect() {
        let mut state = BluetoothState::new();
        let addr = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
        state.on_connected(addr);
        assert_eq!(state.peer_address(), Some(addr));
    }

    #[test]
    fn test_bt_peer_address_none_when_disconnected() {
        let state = BluetoothState::new();
        assert_eq!(state.peer_address(), None);
    }
}
