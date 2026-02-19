//! Custom LUT (Lookup Table) Waveform Support
//!
//! Enables loading and using real waveform data from e-ink display controllers
//! for maximum accuracy in simulation. Supports both JSON (human-readable) and
//! binary (hardware-compatible) formats.
//!
//! # Overview
//!
//! E-ink displays use waveform lookup tables to control voltage sequences applied
//! to pixels during refresh operations. These LUTs define:
//! - Voltage level to apply (-15V to +15V typically)
//! - Duration of each voltage phase
//! - Sequence of phases for a complete transition
//!
//! # Example
//!
//! ```no_run
//! use eink_emulator::lut::{WaveformLut, LutPhase, WaveformLutSet};
//! use eink_emulator::WaveformMode;
//!
//! // Create custom waveform for GC16 mode
//! let phases = vec![
//!     LutPhase { voltage: -15, duration_us: 10000 },
//!     LutPhase { voltage: 15, duration_us: 10000 },
//!     LutPhase { voltage: -10, duration_us: 8000 },
//!     LutPhase { voltage: 10, duration_us: 8000 },
//! ];
//!
//! let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
//! println!("Total duration: {}ms", lut.total_duration_ms);
//! println!("Ghosting contribution: {:.2}%", lut.ghosting_contribution() * 100.0);
//! ```

use crate::waveform_mode::WaveformMode;
use std::collections::HashMap;

/// Single voltage phase in a waveform
///
/// Represents one step in the voltage sequence applied during a refresh.
#[derive(Debug, Clone, PartialEq)]
pub struct LutPhase {
    /// Voltage to apply in volts (-15 to +15 typical range)
    pub voltage: i8,

    /// Duration of this phase in microseconds
    pub duration_us: u16,
}

/// Complete waveform for one mode (e.g., GC16)
///
/// Defines the complete voltage sequence for a refresh operation.
#[derive(Debug, Clone)]
pub struct WaveformLut {
    /// Waveform mode this LUT applies to
    pub mode: WaveformMode,

    /// Sequence of voltage phases
    pub phases: Vec<LutPhase>,

    /// Total duration in milliseconds (calculated from phases)
    pub total_duration_ms: u32,

    /// Valid temperature range for this waveform (min, max in °C)
    pub temperature_range: (i8, i8),
}

impl WaveformLut {
    /// Create new waveform LUT
    ///
    /// Calculates total duration from phase durations.
    pub fn new(mode: WaveformMode, phases: Vec<LutPhase>, temp_range: (i8, i8)) -> Self {
        let total_duration_ms = phases.iter().map(|p| p.duration_us as u32).sum::<u32>() / 1000;

        Self {
            mode,
            phases,
            total_duration_ms,
            temperature_range: temp_range,
        }
    }

    /// Calculate ghosting contribution from voltage phases
    ///
    /// Analyzes the voltage sequence to estimate how much ghosting this
    /// waveform will introduce. Returns 0.0-1.0 scale.
    ///
    /// Higher voltage magnitudes and longer durations contribute more ghosting.
    pub fn ghosting_contribution(&self) -> f32 {
        let mut ghosting = 0.0;

        for phase in &self.phases {
            // Voltage effect: higher voltages move particles more
            let voltage_effect = (phase.voltage as f32).abs() / 15.0;

            // Duration factor: longer phases allow more incomplete transitions
            let duration_factor = (phase.duration_us as f32) / 1000.0;

            // Accumulate ghosting (scaled down to keep in 0-1 range)
            ghosting += voltage_effect * duration_factor * 0.01;
        }

        ghosting.min(1.0)
    }

    /// Calculate DC balance from voltage phases
    ///
    /// Measures the net voltage applied over the complete waveform.
    /// Ideally should be near zero to prevent panel damage.
    ///
    /// Returns signed value: positive = net positive voltage applied.
    pub fn dc_balance(&self) -> f32 {
        let mut dc = 0.0;

        for phase in &self.phases {
            // Voltage applied (signed)
            let voltage_effect = (phase.voltage as f32) / 15.0;

            // Duration weight
            let duration_factor = (phase.duration_us as f32) / 1000.0;

            // Accumulate signed voltage
            dc += voltage_effect * duration_factor;
        }

        dc
    }

    /// Validate LUT data
    ///
    /// Checks for invalid voltage ranges and duration values.
    pub fn validate(&self) -> Result<(), LutError> {
        for phase in &self.phases {
            // Check voltage range (typical e-ink range)
            if phase.voltage < -20 || phase.voltage > 20 {
                return Err(LutError::InvalidVoltage(phase.voltage));
            }

            // Check duration (must be non-zero)
            if phase.duration_us == 0 {
                return Err(LutError::InvalidDuration(phase.duration_us));
            }
        }

        Ok(())
    }
}

/// Set of waveforms for a display
///
/// Contains LUTs for different refresh modes. Not all modes need to be defined.
#[derive(Debug, Clone)]
pub struct WaveformLutSet {
    pub gc16: Option<WaveformLut>,
    pub gl16: Option<WaveformLut>,
    pub du4: Option<WaveformLut>,
    pub du: Option<WaveformLut>,
    pub a2: Option<WaveformLut>,
    pub gcc16: Option<WaveformLut>,
    pub gcu: Option<WaveformLut>,
}

impl WaveformLutSet {
    /// Create empty LUT set
    pub fn new() -> Self {
        Self {
            gc16: None,
            gl16: None,
            du4: None,
            du: None,
            a2: None,
            gcc16: None,
            gcu: None,
        }
    }

    /// Get LUT for specific mode
    pub fn get_lut(&self, mode: WaveformMode) -> Option<&WaveformLut> {
        match mode {
            WaveformMode::GC16 => self.gc16.as_ref(),
            WaveformMode::GL16 => self.gl16.as_ref(),
            WaveformMode::DU4 => self.du4.as_ref(),
            WaveformMode::DU => self.du.as_ref(),
            WaveformMode::A2 => self.a2.as_ref(),
            WaveformMode::GCC16 => self.gcc16.as_ref(),
            WaveformMode::GCU => self.gcu.as_ref(),
        }
    }

    /// Set LUT for specific mode
    pub fn set_lut(&mut self, lut: WaveformLut) {
        match lut.mode {
            WaveformMode::GC16 => self.gc16 = Some(lut),
            WaveformMode::GL16 => self.gl16 = Some(lut),
            WaveformMode::DU4 => self.du4 = Some(lut),
            WaveformMode::DU => self.du = Some(lut),
            WaveformMode::A2 => self.a2 = Some(lut),
            WaveformMode::GCC16 => self.gcc16 = Some(lut),
            WaveformMode::GCU => self.gcu = Some(lut),
        }
    }
}

impl Default for WaveformLutSet {
    fn default() -> Self {
        Self::new()
    }
}

/// LUT loading/parsing errors
#[derive(Debug)]
pub enum LutError {
    InvalidFormat(String),
    UnsupportedVersion(u8),
    InvalidVoltage(i8),
    InvalidDuration(u16),
    IoError(String),
    ParseError(String),
}

impl std::fmt::Display for LutError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LutError::InvalidFormat(msg) => write!(f, "Invalid LUT format: {}", msg),
            LutError::UnsupportedVersion(v) => write!(f, "Unsupported LUT version: {}", v),
            LutError::InvalidVoltage(v) => {
                write!(f, "Invalid voltage: {}V (must be -20 to +20)", v)
            }
            LutError::InvalidDuration(d) => write!(f, "Invalid duration: {}µs", d),
            LutError::IoError(msg) => write!(f, "IO error: {}", msg),
            LutError::ParseError(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl std::error::Error for LutError {}

// JSON format support
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize)]
struct JsonLutSet {
    version: u8,
    temperature: i8,
    waveforms: HashMap<String, JsonWaveform>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonWaveform {
    phases: Vec<JsonPhase>,
}

#[derive(Debug, Deserialize, Serialize)]
struct JsonPhase {
    voltage: i8,
    duration_us: u16,
}

impl WaveformLutSet {
    /// Load from JSON (human-readable format)
    ///
    /// # Example JSON
    /// ```json
    /// {
    ///   "version": 1,
    ///   "temperature": 25,
    ///   "waveforms": {
    ///     "GC16": {
    ///       "phases": [
    ///         {"voltage": -15, "duration_us": 10000},
    ///         {"voltage": 15, "duration_us": 10000}
    ///       ]
    ///     }
    ///   }
    /// }
    /// ```
    pub fn from_json(json: &str) -> Result<Self, LutError> {
        let parsed: JsonLutSet =
            serde_json::from_str(json).map_err(|e| LutError::ParseError(e.to_string()))?;

        if parsed.version != 1 {
            return Err(LutError::UnsupportedVersion(parsed.version));
        }

        let mut lut_set = WaveformLutSet::new();

        for (mode_name, waveform) in parsed.waveforms {
            let mode = match mode_name.as_str() {
                "GC16" => WaveformMode::GC16,
                "GL16" => WaveformMode::GL16,
                "DU4" => WaveformMode::DU4,
                "DU" => WaveformMode::DU,
                "A2" => WaveformMode::A2,
                "GCC16" => WaveformMode::GCC16,
                "GCU" => WaveformMode::GCU,
                _ => continue, // Skip unknown modes
            };

            let phases: Vec<LutPhase> = waveform
                .phases
                .into_iter()
                .map(|p| LutPhase {
                    voltage: p.voltage,
                    duration_us: p.duration_us,
                })
                .collect();

            let temp_range = (parsed.temperature - 5, parsed.temperature + 5);
            let lut = WaveformLut::new(mode, phases, temp_range);

            // Validate LUT
            lut.validate()?;

            lut_set.set_lut(lut);
        }

        Ok(lut_set)
    }

    /// Convert to JSON
    pub fn to_json(&self) -> Result<String, LutError> {
        let mut waveforms = HashMap::new();

        if let Some(ref lut) = self.gc16 {
            waveforms.insert("GC16".to_string(), lut_to_json(lut));
        }
        if let Some(ref lut) = self.gl16 {
            waveforms.insert("GL16".to_string(), lut_to_json(lut));
        }
        if let Some(ref lut) = self.du4 {
            waveforms.insert("DU4".to_string(), lut_to_json(lut));
        }
        if let Some(ref lut) = self.du {
            waveforms.insert("DU".to_string(), lut_to_json(lut));
        }
        if let Some(ref lut) = self.a2 {
            waveforms.insert("A2".to_string(), lut_to_json(lut));
        }
        if let Some(ref lut) = self.gcc16 {
            waveforms.insert("GCC16".to_string(), lut_to_json(lut));
        }
        if let Some(ref lut) = self.gcu {
            waveforms.insert("GCU".to_string(), lut_to_json(lut));
        }

        let json_lut = JsonLutSet {
            version: 1,
            temperature: 25, // Default temperature
            waveforms,
        };

        serde_json::to_string_pretty(&json_lut).map_err(|e| LutError::ParseError(e.to_string()))
    }
}

fn lut_to_json(lut: &WaveformLut) -> JsonWaveform {
    JsonWaveform {
        phases: lut
            .phases
            .iter()
            .map(|p| JsonPhase {
                voltage: p.voltage,
                duration_us: p.duration_us,
            })
            .collect(),
    }
}

// Binary LUT format support
impl WaveformLutSet {
    /// Load from binary LUT file (IT8951/UC8151 compatible format)
    ///
    /// # Binary Format
    /// ```text
    /// [0-3]   Magic: "WFM\0"
    /// [4]     Version: 1
    /// [5]     Temperature (°C, signed)
    /// [6]     Mode count
    /// [7+]    Mode data blocks:
    ///         [0]     Mode ID (0=GC16, 1=GL16, 2=DU4, 3=DU, 4=A2)
    ///         [1-2]   Phase count (u16 LE)
    ///         [3+]    Phase data (3 bytes each):
    ///                 [0]     Voltage (i8)
    ///                 [1-2]   Duration (u16 LE)
    /// ```
    pub fn from_bytes(data: &[u8]) -> Result<Self, LutError> {
        // Check magic number
        if data.len() < 4 || &data[0..4] != b"WFM\0" {
            return Err(LutError::InvalidFormat("Invalid magic number".into()));
        }

        let version = data[4];
        if version != 1 {
            return Err(LutError::UnsupportedVersion(version));
        }

        if data.len() < 7 {
            return Err(LutError::InvalidFormat("File too short for header".into()));
        }

        let temperature = data[5] as i8;
        let mode_count = data[6];

        let mut offset = 7;
        let mut lut_set = WaveformLutSet::new();

        for _ in 0..mode_count {
            if offset + 3 > data.len() {
                return Err(LutError::InvalidFormat("Truncated file".into()));
            }

            let mode_id = data[offset];
            offset += 1;

            let phase_count = u16::from_le_bytes([data[offset], data[offset + 1]]);
            offset += 2;

            let mut phases = Vec::new();
            for _ in 0..phase_count {
                if offset + 3 > data.len() {
                    return Err(LutError::InvalidFormat("Truncated phase data".into()));
                }

                let voltage = data[offset] as i8;
                let duration = u16::from_le_bytes([data[offset + 1], data[offset + 2]]);
                offset += 3;

                phases.push(LutPhase {
                    voltage,
                    duration_us: duration,
                });
            }

            let mode = match mode_id {
                0 => WaveformMode::GC16,
                1 => WaveformMode::GL16,
                2 => WaveformMode::DU4,
                3 => WaveformMode::DU,
                4 => WaveformMode::A2,
                5 => WaveformMode::GCC16,
                6 => WaveformMode::GCU,
                _ => continue, // Skip unknown modes
            };

            let temp_range = (temperature - 5, temperature + 5);
            let lut = WaveformLut::new(mode, phases, temp_range);

            // Validate LUT
            lut.validate()?;

            lut_set.set_lut(lut);
        }

        Ok(lut_set)
    }

    /// Convert to binary format
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut data = Vec::new();

        // Magic number
        data.extend_from_slice(b"WFM\0");

        // Version
        data.push(1);

        // Temperature (use 25°C as default)
        data.push(25);

        // Count modes
        let mode_count = [
            &self.gc16,
            &self.gl16,
            &self.du4,
            &self.du,
            &self.a2,
            &self.gcc16,
            &self.gcu,
        ]
        .iter()
        .filter(|m| m.is_some())
        .count() as u8;
        data.push(mode_count);

        // Write each mode
        if let Some(ref lut) = self.gc16 {
            write_lut_binary(&mut data, 0, lut);
        }
        if let Some(ref lut) = self.gl16 {
            write_lut_binary(&mut data, 1, lut);
        }
        if let Some(ref lut) = self.du4 {
            write_lut_binary(&mut data, 2, lut);
        }
        if let Some(ref lut) = self.du {
            write_lut_binary(&mut data, 3, lut);
        }
        if let Some(ref lut) = self.a2 {
            write_lut_binary(&mut data, 4, lut);
        }
        if let Some(ref lut) = self.gcc16 {
            write_lut_binary(&mut data, 5, lut);
        }
        if let Some(ref lut) = self.gcu {
            write_lut_binary(&mut data, 6, lut);
        }

        data
    }
}

fn write_lut_binary(data: &mut Vec<u8>, mode_id: u8, lut: &WaveformLut) {
    // Mode ID
    data.push(mode_id);

    // Phase count
    let phase_count = lut.phases.len() as u16;
    data.extend_from_slice(&phase_count.to_le_bytes());

    // Phases
    for phase in &lut.phases {
        data.push(phase.voltage as u8);
        data.extend_from_slice(&phase.duration_us.to_le_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lut_phase_creation() {
        let phase = LutPhase {
            voltage: -15,
            duration_us: 10000,
        };
        assert_eq!(phase.voltage, -15);
        assert_eq!(phase.duration_us, 10000);
    }

    #[test]
    fn test_waveform_lut_creation() {
        let phases = vec![
            LutPhase {
                voltage: -15,
                duration_us: 10000,
            },
            LutPhase {
                voltage: 15,
                duration_us: 10000,
            },
        ];

        let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));

        assert_eq!(lut.mode, WaveformMode::GC16);
        assert_eq!(lut.phases.len(), 2);
        assert_eq!(lut.total_duration_ms, 20); // 20000µs = 20ms
        assert_eq!(lut.temperature_range, (20, 30));
    }

    #[test]
    fn test_ghosting_contribution() {
        let phases = vec![
            LutPhase {
                voltage: -15,
                duration_us: 10000,
            },
            LutPhase {
                voltage: 15,
                duration_us: 10000,
            },
        ];

        let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
        let ghosting = lut.ghosting_contribution();

        // Should be non-zero but small
        assert!(ghosting > 0.0);
        assert!(ghosting <= 1.0);
    }

    #[test]
    fn test_dc_balance_symmetric() {
        let phases = vec![
            LutPhase {
                voltage: -15,
                duration_us: 10000,
            },
            LutPhase {
                voltage: 15,
                duration_us: 10000,
            },
        ];

        let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
        let dc = lut.dc_balance();

        // Symmetric waveform should have near-zero DC balance
        assert!(dc.abs() < 0.1);
    }

    #[test]
    fn test_dc_balance_asymmetric() {
        let phases = vec![
            LutPhase {
                voltage: -15,
                duration_us: 10000,
            },
            LutPhase {
                voltage: 15,
                duration_us: 5000,
            }, // Half duration
        ];

        let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
        let dc = lut.dc_balance();

        // Asymmetric waveform should have net negative DC
        assert!(dc < 0.0);
    }

    #[test]
    fn test_lut_validation_valid() {
        let phases = vec![
            LutPhase {
                voltage: -15,
                duration_us: 10000,
            },
            LutPhase {
                voltage: 15,
                duration_us: 10000,
            },
        ];

        let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
        assert!(lut.validate().is_ok());
    }

    #[test]
    fn test_lut_validation_invalid_voltage() {
        let phases = vec![
            LutPhase {
                voltage: -25,
                duration_us: 10000,
            }, // Too negative
        ];

        let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
        assert!(matches!(lut.validate(), Err(LutError::InvalidVoltage(-25))));
    }

    #[test]
    fn test_lut_validation_invalid_duration() {
        let phases = vec![
            LutPhase {
                voltage: 15,
                duration_us: 0,
            }, // Zero duration
        ];

        let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));
        assert!(matches!(lut.validate(), Err(LutError::InvalidDuration(0))));
    }

    #[test]
    fn test_lut_set_get_set() {
        let mut lut_set = WaveformLutSet::new();

        let phases = vec![LutPhase {
            voltage: -15,
            duration_us: 10000,
        }];
        let lut = WaveformLut::new(WaveformMode::GC16, phases, (20, 30));

        lut_set.set_lut(lut.clone());

        let retrieved = lut_set.get_lut(WaveformMode::GC16);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().mode, WaveformMode::GC16);

        // Other modes should be None
        assert!(lut_set.get_lut(WaveformMode::DU4).is_none());
    }

    #[test]
    fn test_json_parsing_valid() {
        let json = r#"{
            "version": 1,
            "temperature": 25,
            "waveforms": {
                "GC16": {
                    "phases": [
                        {"voltage": -15, "duration_us": 10000},
                        {"voltage": 15, "duration_us": 10000}
                    ]
                }
            }
        }"#;

        let lut_set = WaveformLutSet::from_json(json).unwrap();
        assert!(lut_set.gc16.is_some());

        let gc16 = lut_set.gc16.as_ref().unwrap();
        assert_eq!(gc16.phases.len(), 2);
        assert_eq!(gc16.phases[0].voltage, -15);
        assert_eq!(gc16.phases[0].duration_us, 10000);
    }

    #[test]
    fn test_json_parsing_invalid_version() {
        let json = r#"{
            "version": 2,
            "temperature": 25,
            "waveforms": {}
        }"#;

        let result = WaveformLutSet::from_json(json);
        assert!(matches!(result, Err(LutError::UnsupportedVersion(2))));
    }

    #[test]
    fn test_json_roundtrip() {
        let mut lut_set = WaveformLutSet::new();

        let phases = vec![
            LutPhase {
                voltage: -15,
                duration_us: 10000,
            },
            LutPhase {
                voltage: 15,
                duration_us: 10000,
            },
        ];
        lut_set.set_lut(WaveformLut::new(WaveformMode::GC16, phases, (20, 30)));

        // Convert to JSON and back
        let json = lut_set.to_json().unwrap();
        let lut_set2 = WaveformLutSet::from_json(&json).unwrap();

        // Should have same data
        assert!(lut_set2.gc16.is_some());
        let gc16 = lut_set2.gc16.as_ref().unwrap();
        assert_eq!(gc16.phases.len(), 2);
        assert_eq!(gc16.phases[0].voltage, -15);
    }

    #[test]
    fn test_binary_format_valid() {
        let mut data = Vec::new();

        // Magic
        data.extend_from_slice(b"WFM\0");

        // Version
        data.push(1);

        // Temperature
        data.push(25);

        // Mode count
        data.push(1);

        // GC16 mode (ID=0)
        data.push(0);

        // Phase count = 2
        data.extend_from_slice(&2u16.to_le_bytes());

        // Phase 1: -15V, 10000µs
        data.push((-15i8) as u8);
        data.extend_from_slice(&10000u16.to_le_bytes());

        // Phase 2: 15V, 10000µs
        data.push(15u8);
        data.extend_from_slice(&10000u16.to_le_bytes());

        let lut_set = WaveformLutSet::from_bytes(&data).unwrap();
        assert!(lut_set.gc16.is_some());

        let gc16 = lut_set.gc16.as_ref().unwrap();
        assert_eq!(gc16.phases.len(), 2);
        assert_eq!(gc16.phases[0].voltage, -15);
        assert_eq!(gc16.phases[1].voltage, 15);
    }

    #[test]
    fn test_binary_format_invalid_magic() {
        let data = vec![b'B', b'A', b'D', 0];

        let result = WaveformLutSet::from_bytes(&data);
        assert!(matches!(result, Err(LutError::InvalidFormat(_))));
    }

    #[test]
    fn test_binary_format_truncated() {
        let data = vec![b'W', b'F', b'M', 0, 1, 25]; // Missing mode count

        let result = WaveformLutSet::from_bytes(&data);
        // Should succeed but have no modes (mode_count would be uninitialized/0)
        // Actually, we need at least 7 bytes, so this should fail
        assert!(result.is_err());
    }

    #[test]
    fn test_binary_roundtrip() {
        let mut lut_set = WaveformLutSet::new();

        let phases = vec![
            LutPhase {
                voltage: -15,
                duration_us: 10000,
            },
            LutPhase {
                voltage: 15,
                duration_us: 10000,
            },
        ];
        lut_set.set_lut(WaveformLut::new(WaveformMode::GC16, phases, (20, 30)));

        // Convert to binary and back
        let binary = lut_set.to_bytes();
        let lut_set2 = WaveformLutSet::from_bytes(&binary).unwrap();

        // Should have same data
        assert!(lut_set2.gc16.is_some());
        let gc16 = lut_set2.gc16.as_ref().unwrap();
        assert_eq!(gc16.phases.len(), 2);
        assert_eq!(gc16.phases[0].voltage, -15);
    }

    #[test]
    fn test_multiple_modes_in_set() {
        let json = r#"{
            "version": 1,
            "temperature": 25,
            "waveforms": {
                "GC16": {
                    "phases": [
                        {"voltage": -15, "duration_us": 10000}
                    ]
                },
                "DU4": {
                    "phases": [
                        {"voltage": -10, "duration_us": 5000}
                    ]
                },
                "A2": {
                    "phases": [
                        {"voltage": 15, "duration_us": 3000}
                    ]
                }
            }
        }"#;

        let lut_set = WaveformLutSet::from_json(json).unwrap();

        assert!(lut_set.gc16.is_some());
        assert!(lut_set.du4.is_some());
        assert!(lut_set.a2.is_some());
        assert!(lut_set.gl16.is_none());
        assert!(lut_set.du.is_none());
    }
}
