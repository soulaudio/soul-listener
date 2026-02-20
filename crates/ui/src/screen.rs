//! Screen identifier enum — every top-level screen and overlay the UI can display.

/// Every top-level screen or overlay the navigator can push onto its stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    /// Main playback view.
    NowPlaying,
    /// Music library browser.
    LibraryBrowse,
    /// Application settings.
    Settings,
    /// Transient volume-adjustment overlay (pushed on top of any screen).
    VolumeOverlay,
}

#[cfg(test)]
mod tests {
    use super::Screen;

    #[test]
    fn test_screen_enum_has_now_playing() {
        let s = Screen::NowPlaying;
        assert_eq!(s, Screen::NowPlaying);
    }

    #[test]
    fn test_screen_enum_has_library_browse() {
        let s = Screen::LibraryBrowse;
        assert_eq!(s, Screen::LibraryBrowse);
    }

    #[test]
    fn test_screen_enum_has_settings() {
        let s = Screen::Settings;
        assert_eq!(s, Screen::Settings);
    }

    #[test]
    fn test_screen_enum_has_volume_overlay() {
        let s = Screen::VolumeOverlay;
        assert_eq!(s, Screen::VolumeOverlay);
    }

    #[test]
    fn test_screen_is_copy() {
        let a = Screen::NowPlaying;
        let b = a; // copy
        let c = b; // copy (Screen implements Copy, no need to clone)
        assert_eq!(a, b);
        assert_eq!(b, c);
    }
}
