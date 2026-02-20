//! Navigation state machine — a bounded stack of [`Screen`]s.
//!
//! The stack is capped at 8 entries (embedded-safe, no heap). Pushing when
//! the stack is full is a silent no-op (embedded reality: bounded buffer).

use heapless::Vec;

use crate::screen::Screen;

/// Navigation stack bounded at 8 entries.
pub struct Navigator {
    stack: Vec<Screen, 8>,
}

impl Navigator {
    /// Create a new navigator with `NowPlaying` as the root screen.
    pub fn new() -> Self {
        let mut stack = Vec::new();
        // This push always succeeds: the stack starts empty and cap is 8.
        stack.push(Screen::NowPlaying).ok();
        Navigator { stack }
    }

    /// Return the screen currently at the top of the stack.
    #[must_use]
    pub fn current(&self) -> Screen {
        // SAFETY: the stack is never empty (new() seeds it; back() guards depth > 1).
        match self.stack.last() {
            Some(s) => *s,
            None => Screen::NowPlaying, // unreachable by construction
        }
    }

    /// Push a new screen. If the stack is already at capacity the push is a
    /// silent no-op (embedded bounded-buffer contract).
    pub fn push(&mut self, screen: Screen) {
        // heapless::Vec::push returns Err when full; we discard the error.
        self.stack.push(screen).ok();
    }

    /// Pop the top screen. Does nothing if only the root screen remains.
    pub fn back(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
        }
    }

    /// Replace the top screen without growing the stack.
    pub fn replace(&mut self, screen: Screen) {
        if let Some(top) = self.stack.last_mut() {
            *top = screen;
        }
    }

    /// Return the number of entries currently on the stack.
    #[must_use]
    pub fn depth(&self) -> usize {
        self.stack.len()
    }
}

impl Default for Navigator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::Navigator;
    use crate::screen::Screen;

    #[test]
    fn test_nav_starts_at_now_playing() {
        let nav = Navigator::new();
        assert_eq!(nav.current(), Screen::NowPlaying);
    }

    #[test]
    fn test_nav_push_library() {
        let mut nav = Navigator::new();
        nav.push(Screen::LibraryBrowse);
        assert_eq!(nav.current(), Screen::LibraryBrowse);
    }

    #[test]
    fn test_nav_back_from_library() {
        let mut nav = Navigator::new();
        nav.push(Screen::LibraryBrowse);
        nav.back();
        assert_eq!(nav.current(), Screen::NowPlaying);
    }

    #[test]
    fn test_nav_back_at_root_is_noop() {
        let mut nav = Navigator::new();
        nav.back();
        assert_eq!(nav.current(), Screen::NowPlaying);
        assert_eq!(nav.depth(), 1);
    }

    #[test]
    fn test_nav_stack_depth() {
        let mut nav = Navigator::new();
        nav.push(Screen::LibraryBrowse);
        nav.push(Screen::Settings);
        nav.push(Screen::VolumeOverlay);
        assert_eq!(nav.depth(), 4); // root + 3 pushed
    }

    #[test]
    fn test_nav_stack_max_depth() {
        let mut nav = Navigator::new();
        // Push 8 screens. Stack starts with 1 (NowPlaying), cap is 8.
        // So we can push 7 more without panic (total = 8); 8th push is a no-op.
        for _ in 0..8 {
            nav.push(Screen::Settings);
        }
        // Must not panic; depth is capped at 8.
        assert_eq!(nav.depth(), 8);
    }

    #[test]
    fn test_nav_push_volume_overlay() {
        let mut nav = Navigator::new();
        nav.push(Screen::LibraryBrowse);
        nav.push(Screen::VolumeOverlay);
        assert_eq!(nav.current(), Screen::VolumeOverlay);
        nav.back();
        assert_eq!(nav.current(), Screen::LibraryBrowse);
    }

    #[test]
    fn test_nav_replace_screen() {
        let mut nav = Navigator::new();
        nav.push(Screen::LibraryBrowse);
        let depth_before = nav.depth();
        nav.replace(Screen::Settings);
        assert_eq!(nav.current(), Screen::Settings);
        assert_eq!(nav.depth(), depth_before); // stack did not grow
    }
}
