# Phase 7: SoulAudio Integration

**Duration:** Week 7-8 (20 hours)
**Status:** ⏳ Pending
**Dependencies:** Phase 1-6

## Overview

Integrate the complete e-ink ecosystem into the SoulAudio DAP project, build the product-specific UI layer (soul-ui), and deploy to hardware.

---

## Tasks

### 7.1: soul-ui Crate Setup
**Effort:** 2 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Create soul-ui crate with SoulAudio-specific theme and components.

**Acceptance Criteria:**
- [ ] Crate created at `crates/soul-ui/`
- [ ] Dependencies: eink-system, eink-components
- [ ] Workspace integration
- [ ] Hot reload compatible (cdylib)

**Structure:**
```
crates/soul-ui/
├─ src/
│  ├─ lib.rs
│  ├─ theme.rs          # SoulAudio theme
│  ├─ components/       # Custom components
│  │  ├─ mod.rs
│  │  ├─ waveform.rs    # Audio waveform widget
│  │  ├─ album_art.rs   # Album art display
│  │  └─ track_info.rs  # Track metadata
│  └─ screens/          # DAP screens
│     ├─ mod.rs
│     ├─ now_playing.rs
│     ├─ library.rs
│     ├─ playlists.rs
│     └─ settings.rs
├─ Cargo.toml
└─ README.md
```

---

### 7.2: SoulAudio Theme Definition
**Effort:** 2 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Define SoulAudio brand theme with colors, typography, and spacing.

**Acceptance Criteria:**
- [ ] `soul_theme()` function returns Theme
- [ ] Color palette optimized for e-ink
- [ ] Typography scale (3 levels: heading, body, caption)
- [ ] Spacing scale (xs, sm, md, lg, xl)
- [ ] High contrast for readability

**Implementation:**
```rust
// src/theme.rs
use eink_system::*;
use embedded_graphics::pixelcolor::Gray4;

pub fn soul_theme() -> Theme {
    Theme {
        colors: ColorPalette {
            background: Gray4::WHITE,
            foreground: Gray4::BLACK,
            primary: Gray4::new(3),      // Dark gray
            secondary: Gray4::new(1),     // Light gray
            accent: Gray4::new(2),        // Mid gray
        },
        typography: Typography {
            heading1: TextStyle {
                size: 24,
                weight: FontWeight::Bold,
                color: Gray4::BLACK,
                line_height: 1.2,
            },
            heading2: TextStyle {
                size: 18,
                weight: FontWeight::Bold,
                color: Gray4::new(3),
                line_height: 1.2,
            },
            body: TextStyle {
                size: 14,
                weight: FontWeight::Normal,
                color: Gray4::BLACK,
                line_height: 1.4,
            },
            caption: TextStyle {
                size: 12,
                weight: FontWeight::Normal,
                color: Gray4::new(2),
                line_height: 1.3,
            },
        },
        spacing: Spacing {
            xs: 4,
            sm: 8,
            md: 16,
            lg: 24,
            xl: 32,
        },
    }
}
```

---

### 7.3: Waveform Component
**Effort:** 3 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Custom audio waveform visualization component.

**Acceptance Criteria:**
- [ ] `WaveformView` struct
- [ ] Accepts &[i16] audio samples
- [ ] Downsamples for display
- [ ] Renders as line graph
- [ ] Test ID support

**Implementation:**
```rust
pub struct WaveformView<'a> {
    samples: &'a [i16],
    height: u32,
    color: Gray4,
    test_id: Option<&'a str>,
}

impl<'a> View for WaveformView<'a> {
    fn draw<D>(&self, target: &mut D, bounds: Rectangle) -> Result<(), D::Error>
    where
        D: DrawTarget<Color = Gray4>,
    {
        let width = bounds.size().width;
        let step = self.samples.len() / width as usize;

        for x in 0..width {
            let sample_idx = (x as usize) * step;
            let sample = self.samples[sample_idx];
            let y = (sample as f32 / i16::MAX as f32) * (self.height as f32 / 2.0);
            let y = (self.height as i32 / 2) - y as i32;

            // Draw point
            Pixel(Point::new(x as i32, y), self.color).draw(target)?;
        }

        Ok(())
    }
}
```

---

### 7.4: Now Playing Screen
**Effort:** 3 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Main playback screen with track info, progress, controls.

**Acceptance Criteria:**
- [ ] Track title, artist, album
- [ ] Album art placeholder
- [ ] Waveform visualization
- [ ] Progress bar
- [ ] Playback controls (prev, play/pause, next)
- [ ] All elements have test IDs

**Implementation:**
```rust
// src/screens/now_playing.rs
use eink_components::*;
use eink_system::*;
use crate::components::*;

pub struct NowPlayingScreen;

impl NowPlayingScreen {
    pub fn build<'a>(
        state: &'a PlaybackState,
        theme: &'a Theme,
    ) -> impl View + 'a {
        VStack::new(&[
            // Title
            &Text::new(&state.track.title)
                .style(theme.typography.heading1)
                .test_id("track-title"),

            // Artist
            &Text::new(&state.track.artist)
                .style(theme.typography.body)
                .test_id("track-artist"),

            &Spacer::new(theme.spacing.md),

            // Waveform
            &WaveformView::new(&state.waveform_samples)
                .height(80)
                .test_id("waveform"),

            &Spacer::new(theme.spacing.sm),

            // Progress bar
            &ProgressBar::new(state.position / state.duration)
                .height(8)
                .test_id("progress"),

            &Spacer::new(theme.spacing.md),

            // Controls
            &HStack::new(&[
                &IconButton::new(Icon::Prev).test_id("btn-prev"),
                &IconButton::new(Icon::PlayPause).test_id("btn-play"),
                &IconButton::new(Icon::Next).test_id("btn-next"),
            ])
            .spacing(theme.spacing.md)
            .test_id("playback-controls"),
        ])
        .spacing(theme.spacing.sm)
        .style(Style {
            padding: EdgeInsets::all(theme.spacing.md),
            ..Default::default()
        })
    }
}
```

---

### 7.5: Library Screen
**Effort:** 3 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Music library browser with scrollable list of tracks.

**Acceptance Criteria:**
- [ ] ListView with tracks
- [ ] Search/filter capability
- [ ] Virtualized rendering
- [ ] Test IDs

---

### 7.6: Settings Screen
**Effort:** 2 hours | **Priority:** Medium | **Status:** ⏳ Pending

**Description:**
Settings screen with options.

**Settings:**
- Volume
- Brightness
- Sleep timeout
- About

**Acceptance Criteria:**
- [ ] Settings list
- [ ] Toggle components
- [ ] Value display
- [ ] Test IDs

---

### 7.7: Simulator Integration
**Effort:** 2 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Update simulator to use soul-ui with hot reload.

**Acceptance Criteria:**
- [ ] Simulator uses soul-ui screens
- [ ] Hot reload works
- [ ] Keyboard → button mapping
- [ ] Mock playback state

**Updated simulator/src/main.rs:**
```rust
use eink_emulator::{Emulator, HotReloadEmulator};
use eink_specs::displays;
use soul_ui::{screens::*, theme::soul_theme};

fn main() {
    let mut emulator = HotReloadEmulator::new(displays::WAVESHARE_2_13_V4);

    let state = AppState {
        current_screen: Screen::NowPlaying,
        playback: mock_playback_state(),
        theme: soul_theme(),
    };

    emulator.run(state);
}
```

---

### 7.8: Firmware Integration
**Effort:** 3 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Integrate soul-ui into firmware crate for hardware deployment.

**Acceptance Criteria:**
- [ ] Firmware uses soul-ui screens
- [ ] Hardware DisplayDriver connected
- [ ] Input handling from physical buttons
- [ ] Compiles for thumbv7em-none-eabihf

**Updated firmware/src/main.rs:**
```rust
#![no_std]
#![no_main]

use embassy_executor::Spawner;
use platform::DisplayDriver;
use soul_ui::{screens::*, theme::soul_theme};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let mut display = EinkHardware::new(/* SPI, GPIO pins */);
    let theme = soul_theme();

    loop {
        let state = get_playback_state();

        NowPlayingScreen::build(&state, &theme)
            .draw(&mut display, display.bounding_box())
            .unwrap();

        display.refresh_partial().await.unwrap();

        // Event loop...
    }
}
```

---

### 7.9: Visual Regression Tests for DAP
**Effort:** 2 hours | **Priority:** High | **Status:** ⏳ Pending

**Description:**
Visual regression tests for all DAP screens.

**Acceptance Criteria:**
- [ ] Test for Now Playing screen
- [ ] Test for Library screen
- [ ] Test for Settings screen
- [ ] Test for all states (playing, paused, stopped)

**Tests:**
```rust
#[test]
fn test_now_playing_screen_visual() {
    let mut emulator = Emulator::headless(displays::WAVESHARE_2_13_V4);
    let state = mock_playback_state();

    NowPlayingScreen::build(&state, &soul_theme())
        .draw(&mut emulator, emulator.bounding_box())
        .unwrap();

    assert_ui_snapshot!("now_playing", emulator);
}
```

---

### 7.10: Hardware Testing
**Effort:** 4 hours | **Priority:** Critical | **Status:** ⏳ Pending

**Description:**
Test on actual STM32H7 + e-ink hardware.

**Test Plan:**
1. Flash firmware
2. Verify display renders correctly
3. Test all screens
4. Test button inputs
5. Test refresh modes
6. Verify ghosting management
7. Performance check (< 300ms UI updates)

**Acceptance Criteria:**
- [ ] All screens render correctly on hardware
- [ ] No visual artifacts
- [ ] Refresh timing as expected
- [ ] Input responsive
- [ ] No crashes
- [ ] Memory usage within limits

---

## Phase Completion Checklist

- [ ] soul-ui crate complete
- [ ] All DAP screens implemented
- [ ] Simulator integration working
- [ ] Firmware integration working
- [ ] Visual regression tests passing
- [ ] Hardware testing successful
- [ ] Performance targets met
- [ ] Ready for production

---

**Final Deliverables:**
- ✅ 5 publishable crates (eink-*)
- ✅ SoulAudio DAP UI (soul-ui)
- ✅ Working desktop simulator with hot reload
- ✅ Deployable firmware
- ✅ Comprehensive test suite
- ✅ Complete documentation

**Estimated Completion:** End of Week 8
