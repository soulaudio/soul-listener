//! Power consumption simulation for e-ink displays
//!
//! Models realistic power usage based on hardware datasheets and measurements.
//! Enables battery life optimization without physical hardware.

use std::time::Instant;

/// Power consumption profile for a display
///
/// All current values in microamps (µA) for precision.
/// Based on real hardware measurements from datasheets.
#[derive(Debug, Clone)]
pub struct PowerProfile {
    /// Idle current when display is static (no refresh)
    pub idle_current_ua: u32,

    /// Deep sleep current (display off, SRAM retained)
    pub sleep_current_ua: u32,

    /// Base refresh current (constant during any refresh)
    pub refresh_current_ua: u32,

    /// Additional current per flash during full refresh
    pub refresh_boost_ua: u32,

    /// Current during SRAM buffer transfer via SPI
    pub sram_transfer_current_ua: u32,

    /// Current during initialization sequence
    pub init_current_ua: u32,
}

impl PowerProfile {
    /// Waveshare 2.13" V4 power profile
    ///
    /// Based on SSD1680 controller + 250×122 panel (~6.5cm²)
    /// - Full refresh: 7.5mJ/cm² × 6.5cm² = 48.75mJ
    /// - At 3.3V, 980ms duration → ~54mA average during refresh
    pub const WAVESHARE_2_13_V4: Self = Self {
        idle_current_ua: 150,           // 150µA idle (typical e-ink idle)
        sleep_current_ua: 2,            // 2µA deep sleep (static image)
        refresh_current_ua: 54_000,     // 54mA calculated from energy (base + flashes)
        refresh_boost_ua: 15_000,       // +15mA per additional flash
        sram_transfer_current_ua: 8_000, // 8mA SPI transfer overhead
        init_current_ua: 70_000,        // 70mA init (multiple refreshes)
    };

    /// Waveshare 2.9" V2 power profile
    ///
    /// Based on IL0373 controller + 296×128 panel (~9.5cm²)
    pub const WAVESHARE_2_9_V2: Self = Self {
        idle_current_ua: 200,
        sleep_current_ua: 2,
        refresh_current_ua: 60_000,
        refresh_boost_ua: 18_000,
        sram_transfer_current_ua: 9_000,
        init_current_ua: 75_000,
    };

    /// Waveshare 4.2" V2 power profile
    ///
    /// Based on SSD1619 controller + 400×300 panel (~30cm²)
    pub const WAVESHARE_4_2_V2: Self = Self {
        idle_current_ua: 300,
        sleep_current_ua: 3,
        refresh_current_ua: 100_000,
        refresh_boost_ua: 35_000,
        sram_transfer_current_ua: 15_000,
        init_current_ua: 120_000,
    };

    /// Waveshare 7.5" V2 power profile
    ///
    /// Based on ED075TC1 controller + 800×480 panel (~75cm²)
    /// - Full refresh: 7.5mJ/cm² × 75cm² = 562.5mJ
    /// - At 3.3V, 1400ms duration → ~173mA average during refresh
    pub const WAVESHARE_7_5_V2: Self = Self {
        idle_current_ua: 400,
        sleep_current_ua: 3,
        refresh_current_ua: 173_000,   // 173mA calculated from energy
        refresh_boost_ua: 40_000,      // +40mA per flash
        sram_transfer_current_ua: 30_000,
        init_current_ua: 200_000,
    };

    /// Good Display GDEM0397T81P power profile (3.97" 800×480)
    ///
    /// Based on SSD1677 controller + 800×480 panel (~45cm² active area)
    /// Datasheet specifications:
    /// - Typical power: 36mW (datasheet spec)
    /// - Deep sleep: 0.003mW → 0.9µA (confirmed by datasheet)
    /// - Active area: 86.40 × 51.84mm = 44.7cm²
    /// - Full refresh: 7.5mJ/cm² × 45cm² = 337.5mJ
    /// - At 3.3V, 3000ms duration → 337.5mJ/3s/3.3V = 34mA average
    pub const GDEM0397T81P: Self = Self {
        idle_current_ua: 350,          // 350µA idle (SSD1677 + large SRAM)
        sleep_current_ua: 1,           // 1µA deep sleep (datasheet: 0.003mW)
        refresh_current_ua: 25_000,    // 25mA base refresh current
        refresh_boost_ua: 9_000,       // +9mA per flash (3 flashes → 34mA avg)
        sram_transfer_current_ua: 10_000, // 10mA SPI transfer (large 384KB framebuffer)
        init_current_ua: 35_000,       // 35mA initialization
    };
}

/// Real-time power statistics
#[derive(Debug, Clone, Default)]
pub struct PowerStats {
    /// Total energy consumed in microwatt-hours (µWh)
    pub total_energy_uwh: u64,

    /// Average current draw since start (µA)
    pub average_current_ua: u32,

    /// Peak current draw observed (µA)
    pub peak_current_ua: u32,

    /// Time spent in idle state (milliseconds)
    pub idle_time_ms: u64,

    /// Time spent actively refreshing (milliseconds)
    pub active_time_ms: u64,

    /// Time spent in sleep state (milliseconds)
    pub sleep_time_ms: u64,
}

impl PowerStats {
    /// Estimate battery life in hours for a given battery capacity
    ///
    /// # Arguments
    /// * `battery_mah` - Battery capacity in milliamp-hours
    ///
    /// # Returns
    /// Estimated battery life in hours, or 0.0 if no power consumed yet
    pub fn estimated_battery_life_hours(&self, battery_mah: u32) -> f32 {
        if self.average_current_ua == 0 {
            return 0.0;
        }
        let avg_current_ma = self.average_current_ua as f32 / 1000.0;
        battery_mah as f32 / avg_current_ma
    }

    /// Get total runtime in milliseconds
    pub fn total_runtime_ms(&self) -> u64 {
        self.idle_time_ms + self.active_time_ms + self.sleep_time_ms
    }

    /// Get percentage of time in each state
    pub fn state_percentages(&self) -> StatePercentages {
        let total = self.total_runtime_ms() as f32;
        if total == 0.0 {
            return StatePercentages::default();
        }

        StatePercentages {
            idle: (self.idle_time_ms as f32 / total) * 100.0,
            active: (self.active_time_ms as f32 / total) * 100.0,
            sleep: (self.sleep_time_ms as f32 / total) * 100.0,
        }
    }
}

/// Percentage of time spent in each power state
#[derive(Debug, Clone, Default)]
pub struct StatePercentages {
    pub idle: f32,
    pub active: f32,
    pub sleep: f32,
}

/// Current power state of the display
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum PowerState {
    /// Display is idle (showing static image, no refresh)
    Idle,

    /// Display is refreshing with specified number of flashes
    Refreshing { flash_count: u8 },

    /// Display in deep sleep mode
    Sleeping,

    /// Display is initializing
    Initializing,

    /// Transferring buffer to display SRAM
    TransferringBuffer,
}

impl Default for PowerState {
    fn default() -> Self {
        Self::Idle
    }
}

/// Power consumption tracker
///
/// Tracks power consumption over time and calculates statistics.
#[derive(Debug)]
pub struct PowerTracker {
    /// Current power state
    state: PowerState,

    /// Power statistics
    stats: PowerStats,

    /// Power profile for the display
    profile: &'static PowerProfile,

    /// Last time power was updated
    last_update: Instant,

    /// Whether power tracking is enabled
    enabled: bool,
}

impl PowerTracker {
    /// Create new power tracker with specified profile
    pub fn new(profile: &'static PowerProfile) -> Self {
        Self {
            state: PowerState::Idle,
            stats: PowerStats::default(),
            profile,
            last_update: Instant::now(),
            enabled: true,
        }
    }

    /// Update to new power state and record energy consumption
    pub fn transition_to(&mut self, new_state: PowerState) {
        if !self.enabled {
            return;
        }

        let elapsed_ms = self.last_update.elapsed().as_millis() as u64;

        // Calculate energy for previous state
        let current_ua = self.current_draw_ua();

        // Energy = Current × Voltage × Time
        // Assuming 3.3V: E (µWh) = I (µA) × 3.3V × t (h)
        // E (µWh) = I (µA) × 3.3V × (t_ms / 3_600_000)
        // E (µWh) = I (µA) × 3.3 × t_ms / 3_600_000
        // E (µWh) = I (µA) × 33 × t_ms / 36_000_000
        let energy_uwh = (current_ua as u64) * 33 * elapsed_ms / 36_000_000;

        self.stats.total_energy_uwh += energy_uwh;
        self.stats.peak_current_ua = self.stats.peak_current_ua.max(current_ua);

        // Update time tracking
        match self.state {
            PowerState::Idle => self.stats.idle_time_ms += elapsed_ms,
            PowerState::Refreshing { .. } => self.stats.active_time_ms += elapsed_ms,
            PowerState::Sleeping => self.stats.sleep_time_ms += elapsed_ms,
            PowerState::Initializing => self.stats.active_time_ms += elapsed_ms,
            PowerState::TransferringBuffer => self.stats.active_time_ms += elapsed_ms,
        }

        // Update state
        self.state = new_state;
        self.last_update = Instant::now();

        // Recalculate average current
        let total_time_ms = self.stats.total_runtime_ms();
        if total_time_ms > 0 {
            let total_time_h = total_time_ms as f64 / 3_600_000.0;
            let avg_power_uw = self.stats.total_energy_uwh as f64 / total_time_h;
            self.stats.average_current_ua = (avg_power_uw / 3.3) as u32;
        }
    }

    /// Get current draw in microamps for current state
    fn current_draw_ua(&self) -> u32 {
        match self.state {
            PowerState::Idle => self.profile.idle_current_ua,
            PowerState::Sleeping => self.profile.sleep_current_ua,
            PowerState::Refreshing { flash_count } => {
                self.profile.refresh_current_ua
                    + (flash_count as u32 * self.profile.refresh_boost_ua)
            }
            PowerState::Initializing => self.profile.init_current_ua,
            PowerState::TransferringBuffer => self.profile.sram_transfer_current_ua,
        }
    }

    /// Get current power statistics
    pub fn stats(&self) -> &PowerStats {
        &self.stats
    }

    /// Reset all statistics
    pub fn reset(&mut self) {
        self.stats = PowerStats::default();
        self.last_update = Instant::now();
        self.state = PowerState::Idle;
    }

    /// Enable or disable power tracking
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    /// Check if power tracking is enabled
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Get current power state
    pub fn state(&self) -> PowerState {
        self.state
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_profile_constants() {
        // Verify profiles are reasonable
        assert!(PowerProfile::WAVESHARE_2_13_V4.idle_current_ua > 0);
        assert!(PowerProfile::WAVESHARE_2_13_V4.refresh_current_ua > PowerProfile::WAVESHARE_2_13_V4.idle_current_ua);
        assert!(PowerProfile::WAVESHARE_7_5_V2.refresh_current_ua > PowerProfile::WAVESHARE_2_13_V4.refresh_current_ua);
    }

    #[test]
    fn test_battery_life_calculation() {
        let mut stats = PowerStats::default();
        stats.average_current_ua = 10_000; // 10mA average (10,000 µA)

        // 3000mAh battery / 10mA = 300 hours
        let life = stats.estimated_battery_life_hours(3000);
        assert_eq!(life, 300.0);
    }

    #[test]
    fn test_state_percentages() {
        let mut stats = PowerStats::default();
        stats.idle_time_ms = 70_000;  // 70 seconds
        stats.active_time_ms = 20_000; // 20 seconds
        stats.sleep_time_ms = 10_000;  // 10 seconds

        let percentages = stats.state_percentages();
        assert!((percentages.idle - 70.0).abs() < 0.1);
        assert!((percentages.active - 20.0).abs() < 0.1);
        assert!((percentages.sleep - 10.0).abs() < 0.1);
    }

    #[test]
    fn test_power_tracker_idle() {
        let mut tracker = PowerTracker::new(&PowerProfile::WAVESHARE_2_13_V4);

        // Need ~8s for 150µA idle current to accumulate 1µWh with integer math
        // Formula: 150µA × 8000ms × 33 / 36_000_000 = 1.1µWh
        std::thread::sleep(std::time::Duration::from_millis(8000));
        tracker.transition_to(PowerState::Idle);

        let stats = tracker.stats();
        assert!(stats.total_energy_uwh > 0, "Idle should consume energy after 8s");
        assert!(stats.idle_time_ms >= 8000);
    }

    #[test]
    fn test_power_tracker_refresh() {
        let mut tracker = PowerTracker::new(&PowerProfile::WAVESHARE_2_13_V4);

        tracker.transition_to(PowerState::Refreshing { flash_count: 3 });
        std::thread::sleep(std::time::Duration::from_millis(100));
        tracker.transition_to(PowerState::Idle);

        let stats = tracker.stats();
        assert!(stats.total_energy_uwh > 0, "Refresh should consume energy");
        assert!(stats.active_time_ms >= 100);
    }

    #[test]
    fn test_power_tracker_disable() {
        let mut tracker = PowerTracker::new(&PowerProfile::WAVESHARE_2_13_V4);
        tracker.set_enabled(false);

        std::thread::sleep(std::time::Duration::from_millis(10));
        tracker.transition_to(PowerState::Idle);

        // Should not accumulate energy when disabled
        let stats = tracker.stats();
        assert_eq!(stats.total_energy_uwh, 0);
    }

    #[test]
    fn test_peak_current_tracking() {
        let mut tracker = PowerTracker::new(&PowerProfile::WAVESHARE_2_13_V4);

        tracker.transition_to(PowerState::Idle);
        std::thread::sleep(std::time::Duration::from_millis(5));

        tracker.transition_to(PowerState::Refreshing { flash_count: 3 });
        std::thread::sleep(std::time::Duration::from_millis(5));

        tracker.transition_to(PowerState::Idle);

        let stats = tracker.stats();
        // Peak should be refresh current + boost
        let expected_peak = PowerProfile::WAVESHARE_2_13_V4.refresh_current_ua
            + 3 * PowerProfile::WAVESHARE_2_13_V4.refresh_boost_ua;
        assert_eq!(stats.peak_current_ua, expected_peak);
    }
}
