//! Emulator configuration

/// Configuration for emulator display presentation
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmulatorConfig {
    /// Display rotation (affects window orientation only, not HAL coordinates)
    pub rotation: Rotation,
    /// Upscaling factor (1 = no scaling, 2 = 2x for visibility, etc.)
    pub scale: u32,
}

impl EmulatorConfig {
    /// Default configuration: no rotation, 2x scaling for visibility
    pub const DEFAULT: Self = Self {
        rotation: Rotation::Degrees0,
        scale: 2,
    };

    /// No rotation, no upscaling (1:1 pixel mapping)
    pub const NATIVE: Self = Self {
        rotation: Rotation::Degrees0,
        scale: 1,
    };

    /// Portrait mode (90° rotation), no upscaling
    pub const PORTRAIT: Self = Self {
        rotation: Rotation::Degrees90,
        scale: 1,
    };

    /// Portrait mode (90° rotation), 2x upscaling
    pub const PORTRAIT_2X: Self = Self {
        rotation: Rotation::Degrees90,
        scale: 2,
    };
}

impl Default for EmulatorConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

/// Display rotation modes
///
/// Rotation is applied to the **window presentation only**, not to the HAL
/// coordinate system. DrawTarget coordinates remain logical (e.g., 800×480),
/// and rotation is applied when rendering to the window buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rotation {
    /// No rotation (landscape: width × height)
    Degrees0,
    /// Rotate 90° clockwise (portrait: height × width)
    Degrees90,
    /// Rotate 180° (upside-down landscape: width × height)
    Degrees180,
    /// Rotate 270° clockwise / 90° counter-clockwise (portrait: height × width)
    Degrees270,
}

impl Rotation {
    /// Check if rotation swaps width and height
    pub fn swaps_dimensions(&self) -> bool {
        matches!(self, Rotation::Degrees90 | Rotation::Degrees270)
    }

    /// Calculate window dimensions after rotation
    pub fn apply_to_dimensions(&self, width: u32, height: u32) -> (u32, u32) {
        if self.swaps_dimensions() {
            (height, width)
        } else {
            (width, height)
        }
    }
}
