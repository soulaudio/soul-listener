//! Debug metadata for components

use heapless::String;

/// Maximum length for custom debug data strings
const MAX_DEBUG_STRING_LENGTH: usize = 64;

/// Debug information provided by components
#[cfg(feature = "debug")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DebugInfo {
    /// Component type name (e.g., "Button", "Label")
    pub component_type: &'static str,

    /// Debug color for borders
    pub color: DebugColor,

    /// Optional custom data (e.g., button label)
    pub custom_data: Option<String<MAX_DEBUG_STRING_LENGTH>>,
}

/// Debug color palette for component types
#[cfg(feature = "debug")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugColor {
    Container,   // Blue
    Button,      // Green
    Label,       // Red
    ProgressBar, // Purple
    Other,       // Yellow
}

#[cfg(feature = "debug")]
impl DebugColor {
    /// Get RGBA color value
    pub fn to_rgba(self) -> u32 {
        match self {
            DebugColor::Container => 0xFF0080FF,   // Blue
            DebugColor::Button => 0xFF00FF80,      // Green
            DebugColor::Label => 0xFFFF4040,       // Red
            DebugColor::ProgressBar => 0xFFC040FF, // Purple
            DebugColor::Other => 0xFFFFCC00,       // Yellow
        }
    }
}

#[cfg(all(test, feature = "debug"))]
mod tests {
    use super::*;

    #[test]
    fn test_debug_color_rgba() {
        assert_eq!(DebugColor::Container.to_rgba(), 0xFF0080FF);
        assert_eq!(DebugColor::Button.to_rgba(), 0xFF00FF80);
        assert_eq!(DebugColor::Label.to_rgba(), 0xFFFF4040);
        assert_eq!(DebugColor::ProgressBar.to_rgba(), 0xFFC040FF);
        assert_eq!(DebugColor::Other.to_rgba(), 0xFFFFCC00);
    }

    #[test]
    fn test_debug_info_creation() {
        use heapless::String;
        let custom_str: String<64> = String::try_from("Play").unwrap();
        let info = DebugInfo {
            component_type: "Button",
            color: DebugColor::Button,
            custom_data: Some(custom_str),
        };
        assert_eq!(info.component_type, "Button");
        assert_eq!(info.color, DebugColor::Button);
        assert!(info.custom_data.is_some());
    }

    #[test]
    fn test_debug_color_equality() {
        assert_eq!(DebugColor::Button, DebugColor::Button);
        assert_ne!(DebugColor::Button, DebugColor::Label);
    }
}
