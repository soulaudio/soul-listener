//! Tracking marker for the Rule 1 (no heap allocation — MISRA Dir 4.12)
//! **display-path** gap.
//!
//! `tests/no_alloc.rs` already link/runtime-proves the AUDIO path
//! (`soul-audio-core`'s decode/pump loop) is alloc-free. The DISPLAY path is
//! not: `eink-system`'s `VStack`/`HStack` containers store children as
//! `heapless::Vec<Box<dyn Layout>, N>` (`crates/eink/eink-system/src/
//! containers.rs`), and in its no_std build that `Box` comes from `extern
//! crate alloc` — a real heap allocation every time a child is boxed into a
//! stack. `dap-core::display` re-exports that same toolkit
//! (`dap/core/src/display.rs`), so this gap is reachable from the DAP's real
//! display port, not just an eink-system-internal detail.
//!
//! This is WHY `board-h7a3/src/main.rs` currently carries a 4 KB
//! `embedded_alloc::LlffHeap` static arena purely to satisfy that link
//! requirement (see its own doc comment, and `tools/mcu-stackprobe/
//! README.md`'s "Display-path scope note"). It is also why there is no
//! full-stack (audio AND display) no-alloc link gate yet — only the
//! `mcu-stackprobe` AUDIO-only one.
//!
//! # What this test does
//! Counts heap allocations (same counting-global-allocator technique as
//! `no_alloc.rs`) while building one `VStack<1>` with a single boxed
//! `Spacer` child — the minimal operation that exercises the `Box<dyn
//! Layout>` allocation above — and asserts it currently allocates **at
//! least once**.
//!
//! # When this test starts FAILING
//! That is good news, not a regression: it means `eink-system` (or whatever
//! replaces it — see the eink-ui no-alloc redesign, item C in
//! `docs/plans/2026-11-17-architecture-enforcement-fitness-functions.md`)
//! stopped allocating on this path. At that point:
//!   1. Flip this test's assertion from `allocs > 0` to `allocs == 0` (or
//!      delete this file and fold the check into `no_alloc.rs` as a second,
//!      now-legitimately-parallel no-alloc assertion).
//!   2. Remove `board-h7a3/src/main.rs`'s 4 KB `embedded_alloc` heap shim —
//!      it exists ONLY to satisfy this now-gone link requirement.
//!   3. Wire up the full-stack (audio + display) no-alloc link gate this
//!      doc has been deferring (`mcu-stackprobe` only proves the audio path
//!      today).
//!
//! Until then, this test's PASSING state is itself the tracking marker: it
//! stays green, visible, and greppable (`cargo test -p dap-sim
//! alloc_policy`) for exactly as long as the gap is real.

use std::alloc::{GlobalAlloc, Layout as AllocLayout, System};
use std::sync::atomic::{AtomicUsize, Ordering};

use eink_system::prelude::*;
use embedded_graphics::prelude::*;

/// Count of `alloc`/`realloc` calls since the last reset. `SeqCst`
/// throughout — this is a one-shot tracking check, not a hot path; see
/// `no_alloc.rs`'s identical reasoning.
static ALLOC_COUNT: AtomicUsize = AtomicUsize::new(0);

/// Wraps [`System`], counting every `alloc`/`realloc` call unconditionally
/// (unlike `no_alloc.rs`'s `ARMED`-gated version: this file has exactly one
/// test, no warmup/steady-state split needed — every allocation during the
/// test body is one this test cares about).
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

/// See module doc for the full tracking-marker rationale. This file carries
/// exactly one `#[test]` for the same cross-thread-sharing reason
/// `no_alloc.rs` documents: `ALLOC_COUNT` is a process-wide static, and
/// `cargo test` runs a file's tests on parallel threads of one process by
/// default. Add a new file, not a second `#[test]` here, if another
/// allocation-tracking check is ever needed.
#[test]
fn eink_system_display_path_still_allocates_tracked_gap() {
    ALLOC_COUNT.store(0, Ordering::SeqCst);

    // The minimal operation that exercises containers.rs's `Box<dyn
    // Layout>` child slot: box one leaf component and push it into a
    // one-child VStack.
    let spacer: Box<dyn Layout> = Box::new(Spacer::new(Size::new(4, 4)));
    let mut stack = VStack::<1>::new();
    // `add_child`'s `Err` carries the rejected `Box<dyn Layout>` back (no
    // `Debug` impl on a trait object), so match rather than `.expect(...)`.
    if stack.add_child(spacer).is_err() {
        panic!("push one child into a VStack<1> should never fail (capacity 1, one push)");
    }

    let allocs = ALLOC_COUNT.load(Ordering::SeqCst);
    println!(
        "display-path alloc-policy tracking marker: {allocs} heap allocation(s) observed \
         boxing one eink-system Layout child"
    );
    assert!(
        allocs > 0,
        "eink-system's VStack/Box<dyn Layout> display path allocated ZERO heap bytes boxing \
         one child -- this means the tracked display-path alloc gap this test exists to mark \
         has CLOSED. See this file's module doc comment for exactly what to do next (flip this \
         assertion, remove board-h7a3's 4 KB embedded_alloc heap shim, wire up the full-stack \
         no-alloc link gate) -- do not just delete this test."
    );
}
