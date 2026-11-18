//! Style tokens — the "Tailwind" layer.
//!
//! Widgets never hand-pick raw pixel values, fonts, or gray levels. Instead
//! they refer to semantic tokens ([`Space`], [`TextScale`], [`Role`]) and a
//! [`Theme`] resolves each token to a concrete value. Swapping the `Theme`
//! (e.g. for an inverted/high-contrast selection state) restyles every
//! widget that reads from it, without touching widget code.

use embedded_graphics::mono_font::ascii::{FONT_10X20, FONT_5X8, FONT_6X10, FONT_9X15_BOLD};
use embedded_graphics::mono_font::MonoFont;
use embedded_graphics::pixelcolor::Gray4;

/// The spacing scale, used for padding/gap/margin.
///
/// Each step maps to a pixel value via [`Theme::space`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Space {
    /// Extra-small spacing (tightest).
    Xs,
    /// Small spacing.
    Sm,
    /// Medium spacing (the default for most containers).
    Md,
    /// Large spacing.
    Lg,
    /// Extra-large spacing (loosest).
    Xl,
}

/// The typography scale, used by text-rendering widgets such as
/// [`crate::widgets::Text`].
///
/// Each step maps to a real `embedded-graphics` [`MonoFont`] via
/// [`Theme::font`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextScale {
    /// Small print: timestamps, helper text, list secondary lines.
    Caption,
    /// The default body copy size.
    Body,
    /// Section headings / screen titles.
    Title,
    /// The largest scale: hero numbers (e.g. the current track time).
    Display,
}

/// Semantic color roles, resolved to a [`Gray4`] gray level by [`Theme::color`].
///
/// Roles are named for *purpose*, not for the raw gray value, so a `Theme`
/// can remap them (e.g. an inverted selection swaps `Ink` and `Paper`)
/// without widgets knowing anything changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Role {
    /// Primary foreground content (default text color). Darkest by default.
    Ink,
    /// Secondary/de-emphasized foreground content.
    Muted,
    /// Tertiary foreground content (hints, disabled state, dividers).
    Faint,
    /// The page/background color. Lightest by default.
    Paper,
}

/// A swappable bundle of token -> value mappings, threaded through every
/// [`crate::widget::Widget::measure`] and [`crate::widget::Widget::draw`]
/// call.
///
/// `Theme::default()` provides the canonical mapping described in the
/// design spec; construct a custom `Theme` (or mutate a cloned default) to
/// restyle an entire screen.
#[derive(Debug, Clone, Copy)]
pub struct Theme {
    /// Gray level for [`Role::Ink`].
    pub ink: Gray4,
    /// Gray level for [`Role::Muted`].
    pub muted: Gray4,
    /// Gray level for [`Role::Faint`].
    pub faint: Gray4,
    /// Gray level for [`Role::Paper`].
    pub paper: Gray4,
    /// Pixel value for [`Space::Xs`].
    pub space_xs: u32,
    /// Pixel value for [`Space::Sm`].
    pub space_sm: u32,
    /// Pixel value for [`Space::Md`].
    pub space_md: u32,
    /// Pixel value for [`Space::Lg`].
    pub space_lg: u32,
    /// Pixel value for [`Space::Xl`].
    pub space_xl: u32,
    /// Font for [`TextScale::Caption`].
    pub font_caption: MonoFont<'static>,
    /// Font for [`TextScale::Body`].
    pub font_body: MonoFont<'static>,
    /// Font for [`TextScale::Title`].
    pub font_title: MonoFont<'static>,
    /// Font for [`TextScale::Display`].
    pub font_display: MonoFont<'static>,
}

impl Theme {
    /// Resolve a [`Role`] to a concrete [`Gray4`] gray level.
    #[must_use]
    pub const fn color(&self, role: Role) -> Gray4 {
        match role {
            Role::Ink => self.ink,
            Role::Muted => self.muted,
            Role::Faint => self.faint,
            Role::Paper => self.paper,
        }
    }

    /// Resolve a [`Space`] token to a concrete pixel value.
    #[must_use]
    pub const fn space(&self, space: Space) -> u32 {
        match space {
            Space::Xs => self.space_xs,
            Space::Sm => self.space_sm,
            Space::Md => self.space_md,
            Space::Lg => self.space_lg,
            Space::Xl => self.space_xl,
        }
    }

    /// Resolve a [`TextScale`] token to a concrete `embedded-graphics`
    /// [`MonoFont`].
    #[must_use]
    pub const fn font(&self, scale: TextScale) -> MonoFont<'static> {
        match scale {
            TextScale::Caption => self.font_caption,
            TextScale::Body => self.font_body,
            TextScale::Title => self.font_title,
            TextScale::Display => self.font_display,
        }
    }

    /// A copy of this theme with foreground/background roles swapped —
    /// [`Role::Ink`]↔[`Role::Paper`] and [`Role::Muted`]↔[`Role::Faint`] — for
    /// drawing a selected/highlighted element against an inverted fill (e.g. a
    /// [`List`](crate::widgets::List)'s selected row, or inverted [`Button`]
    /// text). Spacing and fonts are unchanged.
    #[must_use]
    pub fn inverted(&self) -> Theme {
        Theme {
            ink: self.paper,
            muted: self.faint,
            faint: self.muted,
            paper: self.ink,
            ..*self
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme {
            // Gray4 is a 4-bit greyscale: 0x0 = black (darkest), 0xF = white
            // (lightest). Values per the design brief.
            ink: Gray4::new(0x0),
            muted: Gray4::new(0x5),
            faint: Gray4::new(0xA),
            paper: Gray4::new(0xF),
            space_xs: 2,
            space_sm: 4,
            space_md: 8,
            space_lg: 16,
            space_xl: 32,
            font_caption: FONT_5X8,
            font_body: FONT_6X10,
            font_title: FONT_9X15_BOLD,
            font_display: FONT_10X20,
        }
    }
}
