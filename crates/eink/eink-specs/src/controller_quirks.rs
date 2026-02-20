//! Hardware quirks and bugs per controller
//!
//! This module defines known hardware quirks, bugs, and limitations for different
//! e-ink controllers. These quirks are based on real-world issues documented in
//! community forums, GitHub issues, and manufacturer errata.
//!
//! # Purpose
//!
//! - Enable developers to catch controller-specific bugs during development
//! - Simulate realistic hardware behavior including known issues
//! - Provide clear warnings when quirks are triggered
//! - Can be disabled for testing idealized behavior
//!
//! # Sources
//!
//! - IT8951: Panel-specific flash parameters, limited library support
//!   (Source: Waveshare IT8951 documentation, community reports)
//! - SSD1680: Uncontrollable refresh rate with certain libraries
//!   (Source: GitHub issues with epd-waveshare)
//! - UC8151: Display rotation glitch, SPI write hangs
//!   (Source: Community forums, Waveshare wiki)

use crate::Controller;

/// Known hardware quirks/bugs per controller
#[derive(Debug, Clone)]
pub struct ControllerQuirks {
    pub controller: Controller,
    pub quirks: &'static [Quirk],
}

/// Specific hardware quirk/bug/limitation
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Quirk {
    /// IT8951: Panel-specific flash parameters required
    ///
    /// The IT8951 controller requires panel-specific VCOM and flash parameters
    /// that must be configured correctly or the display will have poor contrast
    /// or won't work at all.
    PanelSpecific { description: &'static str },

    /// SSD1680: Uncontrollable refresh rate with certain libraries
    ///
    /// Some SSD1680 implementations have timing issues where the refresh rate
    /// cannot be precisely controlled, leading to inconsistent refresh times.
    UncontrollableRefreshRate { description: &'static str },

    /// UC8151: Display rotation causes garbled output
    ///
    /// The UC8151 controller has a bug where changing rotation settings can
    /// cause garbled or corrupted display output until a full reset.
    RotationGlitch { description: &'static str },

    /// UC8151: SPI write can hang during initial command sequence
    ///
    /// The UC8151 can sometimes hang during the initial SPI command sequence,
    /// requiring a hardware reset to recover.
    SpiWriteHang { description: &'static str },

    /// IT8951: Limited library support
    ///
    /// The IT8951 has limited support in common e-ink libraries, requiring
    /// custom drivers or workarounds.
    LimitedLibrarySupport { description: &'static str },
}

impl Quirk {
    /// Get the human-readable description of this quirk
    pub fn description(&self) -> &str {
        match self {
            Quirk::PanelSpecific { description } => description,
            Quirk::UncontrollableRefreshRate { description } => description,
            Quirk::RotationGlitch { description } => description,
            Quirk::SpiWriteHang { description } => description,
            Quirk::LimitedLibrarySupport { description } => description,
        }
    }

    /// Get the quirk type name
    pub fn quirk_type(&self) -> &'static str {
        match self {
            Quirk::PanelSpecific { .. } => "PanelSpecific",
            Quirk::UncontrollableRefreshRate { .. } => "UncontrollableRefreshRate",
            Quirk::RotationGlitch { .. } => "RotationGlitch",
            Quirk::SpiWriteHang { .. } => "SpiWriteHang",
            Quirk::LimitedLibrarySupport { .. } => "LimitedLibrarySupport",
        }
    }
}

/// Get known quirks for a specific controller
pub const fn quirks_for_controller(controller: Controller) -> &'static [Quirk] {
    match controller {
        Controller::IT8951 => IT8951_QUIRKS,
        Controller::SSD1680 => SSD1680_QUIRKS,
        Controller::UC8151 => UC8151_QUIRKS,
        Controller::IL0373 => IL0373_QUIRKS,
        Controller::ACeP => ACEP_QUIRKS,
        Controller::SSD1619 => &[],  // No known quirks
        Controller::SSD1677 => &[],  // No known quirks (reliable controller)
        Controller::ED075TC1 => &[], // No known quirks
        Controller::GDEW => &[],     // No known quirks
        Controller::Generic => &[],  // Generic controller
    }
}

/// IT8951 controller quirks
const IT8951_QUIRKS: &[Quirk] = &[
    Quirk::PanelSpecific {
        description: "IT8951 requires panel-specific VCOM and flash parameters. \
                     Incorrect settings result in poor contrast or display failure.",
    },
    Quirk::LimitedLibrarySupport {
        description: "IT8951 has limited support in common e-ink libraries. \
                     May require custom drivers or direct register access.",
    },
];

/// SSD1680 controller quirks
const SSD1680_QUIRKS: &[Quirk] = &[Quirk::UncontrollableRefreshRate {
    description: "SSD1680 refresh timing can be inconsistent with certain driver implementations. \
                 Actual refresh time may vary from specified values.",
}];

/// UC8151 controller quirks
const UC8151_QUIRKS: &[Quirk] = &[
    Quirk::RotationGlitch {
        description: "UC8151 rotation changes can cause garbled output. \
                     Full reset required after rotation change.",
    },
    Quirk::SpiWriteHang {
        description: "UC8151 SPI interface can hang during initial command sequence. \
                     Hardware reset may be required to recover.",
    },
];

/// IL0373 controller quirks (minimal - generally reliable)
const IL0373_QUIRKS: &[Quirk] = &[];

/// ACeP (Advanced Color ePaper) controller quirks
const ACEP_QUIRKS: &[Quirk] = &[
    Quirk::PanelSpecific {
        description: "ACeP requires precise temperature and timing control for color accuracy. \
                     Color rendering may vary with temperature changes.",
    },
    Quirk::LimitedLibrarySupport {
        description: "ACeP color displays have limited support in standard e-ink libraries. \
                     Spectra 6 requires specialized color waveforms and dual-plane updates.",
    },
];

// Add IT8951 to Controller enum if not present
impl Controller {
    /// Check if this controller has any known quirks
    pub fn has_quirks(&self) -> bool {
        !quirks_for_controller(*self).is_empty()
    }

    /// Get the quirks for this controller
    pub fn quirks(&self) -> &'static [Quirk] {
        quirks_for_controller(*self)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing)]
    use super::*;

    #[test]
    fn test_quirk_description() {
        let quirk = Quirk::PanelSpecific {
            description: "Test description",
        };
        assert_eq!(quirk.description(), "Test description");
    }

    #[test]
    fn test_quirk_type() {
        let quirk = Quirk::RotationGlitch {
            description: "Test",
        };
        assert_eq!(quirk.quirk_type(), "RotationGlitch");
    }

    #[test]
    fn test_it8951_quirks() {
        let quirks = quirks_for_controller(Controller::IT8951);
        assert_eq!(quirks.len(), 2);
        assert!(quirks
            .iter()
            .any(|q| matches!(q, Quirk::PanelSpecific { .. })));
        assert!(quirks
            .iter()
            .any(|q| matches!(q, Quirk::LimitedLibrarySupport { .. })));
    }

    #[test]
    fn test_ssd1680_quirks() {
        let quirks = quirks_for_controller(Controller::SSD1680);
        assert_eq!(quirks.len(), 1);
        assert!(matches!(quirks[0], Quirk::UncontrollableRefreshRate { .. }));
    }

    #[test]
    fn test_uc8151_quirks() {
        let quirks = quirks_for_controller(Controller::UC8151);
        assert_eq!(quirks.len(), 2);
        assert!(quirks
            .iter()
            .any(|q| matches!(q, Quirk::RotationGlitch { .. })));
        assert!(quirks
            .iter()
            .any(|q| matches!(q, Quirk::SpiWriteHang { .. })));
    }

    #[test]
    fn test_no_quirks_controllers() {
        assert_eq!(quirks_for_controller(Controller::SSD1619).len(), 0);
        assert_eq!(quirks_for_controller(Controller::ED075TC1).len(), 0);
        assert_eq!(quirks_for_controller(Controller::Generic).len(), 0);
    }

    #[test]
    fn test_controller_has_quirks() {
        assert!(Controller::IT8951.has_quirks());
        assert!(Controller::SSD1680.has_quirks());
        assert!(Controller::UC8151.has_quirks());
        assert!(!Controller::SSD1619.has_quirks());
    }

    #[test]
    fn test_controller_quirks_method() {
        let quirks = Controller::IT8951.quirks();
        assert_eq!(quirks.len(), 2);
    }
}
