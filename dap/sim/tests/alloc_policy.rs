//! Rule 1 (no heap allocation — MISRA Dir 4.12) proof for the **display path**.
//!
//! `tests/no_alloc.rs` link/runtime-proves the AUDIO path (soul-audio-core's
//! decode/pump loop) is alloc-free. This file proves the DISPLAY path is too:
//! the DAP's real Now-Playing render (`dap_core::screens::now_playing::render`,
//! which draws an `eink-ui` widget tree) performs **zero** heap allocations.
//!
//! This used to be a *tracking marker* that asserted the OLD display toolkit
//! (`eink-system`'s `VStack`/`HStack`, which boxed children as
//! `heapless::Vec<Box<dyn Layout>, N>`) allocated at least once — the gap that
//! forced `board-h7a3` to carry an `embedded_alloc` heap. ui-4 closed that gap
//! by moving the DAP onto the no-alloc `eink-ui` toolkit (tuple-composed
//! children, no `Box`/`dyn`), so the assertion is now the inverse: `allocs == 0`.
//! board-h7a3 consequently dropped its heap allocator and its last `unsafe`.

use std::alloc::{GlobalAlloc, Layout as AllocLayout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use dap_core::display::{DisplayError, EinkRefresh, Rectangle};
use dap_core::screens::now_playing::{render, NowPlayingView};
use embedded_graphics::pixelcolor::Gray4;
use embedded_graphics::prelude::*;

/// Count of `alloc`/`realloc` calls since the last reset.
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Wraps [`System`], counting every `alloc`/`realloc` call.
struct CountingAllocator;

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: AllocLayout) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        System.alloc(layout)
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: AllocLayout) {
        System.dealloc(ptr, layout);
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: AllocLayout, new_size: usize) -> *mut u8 {
        ALLOC_COUNT.fetch_add(1, Ordering::SeqCst);
        System.realloc(ptr, layout, new_size)
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

/// A zero-storage [`dap_core::display::Display`]: it accepts and discards every
/// pixel (no framebuffer => no `Vec`), reports a panel-sized bounding box so the
/// screen lays out realistically, and no-ops its e-ink refresh. Drawing into it
/// exercises all of `eink-ui`'s layout + draw code with nothing of its own that
/// could allocate.
struct NullDisplay {
    width: u32,
    height: u32,
}

impl DrawTarget for NullDisplay {
    type Color = Gray4;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = embedded_graphics::Pixel<Self::Color>>,
    {
        // Consume (and drop) every pixel without storing it.
        for _ in pixels {}
        Ok(())
    }
}

impl OriginDimensions for NullDisplay {
    fn size(&self) -> embedded_graphics::geometry::Size {
        embedded_graphics::geometry::Size::new(self.width, self.height)
    }
}

impl EinkRefresh for NullDisplay {
    fn full_refresh(&mut self) -> Result<(), DisplayError> {
        Ok(())
    }
    fn partial_refresh(&mut self, _area: Rectangle) -> Result<(), DisplayError> {
        Ok(())
    }
}

/// One `#[test]` only: `ALLOC_COUNT` is a process-wide static and `cargo test`
/// runs a file's tests on parallel threads. Add a new file, not a second
/// `#[test]`, for any other allocation-tracking check.
#[test]
fn now_playing_display_path_is_alloc_free() {
    let mut d = NullDisplay {
        width: 800,
        height: 480,
    };
    let view = NowPlayingView {
        title: "A Representative Track Title",
        elapsed_s: 65,
        total_s: 200,
        volume: 0.75,
        battery_permille: 823,
        playing: true,
    };

    // Warm the display up once outside the measured window (nothing here should
    // allocate either, but this keeps the window tight around the render).
    render(&mut d, &view).expect("warm-up render");

    ALLOC_COUNT.store(0, Ordering::SeqCst);
    render(&mut d, &view).expect("render must succeed");
    let allocs = ALLOC_COUNT.load(Ordering::SeqCst);

    assert_eq!(
        allocs, 0,
        "the DAP Now-Playing render (eink-ui widget tree) must allocate zero heap bytes -- \
         the display-path no-alloc gap is closed (ui-4). If this ever regresses, the toolkit \
         grew a Box/Vec/String on the draw path."
    );
}
