//! Whole-firmware-tick stack-measurement harness (feature `tick`).
//!
//! The sibling `mcu-stackprobe` bin measures only the decode path
//! (`open_seekable → Deck::load → Deck::pump`). This one measures the WHOLE
//! firmware tick the board actually runs: `dap_core::firmware::step`, which does
//! `App::tick` (drain controls, poll power, pump the deck — including the deep
//! track-OPEN on the first tick) **and then** `App::render` (the e-ink
//! now-playing view: `eink-ui` flex layout + widget drawing through the
//! `Display` port). Because `tick` and `render` run sequentially (render after
//! tick returns), the tick's worst-case stack is `max(open_transient,
//! render_transient)`, not their sum — this harness measures that max directly
//! on real thumbv7em (Cortex-M7) codegen under QEMU, closing the "but what about
//! the rest of the tick, not just decode?" gap.
//!
//! It wires a REAL `App` exactly as `board-h7a3/src/main.rs` does — the same
//! `Stub*` ports — except `Storage` is a [`FixtureStorage`] that hands back an
//! in-memory Matroska-FLAC fixture (the decode worst case) instead of
//! `StubStorage` (whose `open` always fails, so the board build never actually
//! decodes). That makes the first `step` run the real `open_seekable` decoder
//! construction *through* `App`, so the measured transient includes it.
//!
//! Like `mcu-stackprobe`, this is a throwaway measurement rig, not product
//! firmware; `check-tick-qemu.sh` builds + runs it and gates the number.
#![no_std]
#![no_main]

use cortex_m_rt::entry;
use panic_halt as _;
use static_cell::StaticCell;

use dap_core::app::{App, AppConfig};
use dap_core::firmware;
use dap_core::ports::Storage;
use dap_core::stub::{StubControls, StubDisplay, StubPower, StubSink};
use soul_audio_core::{decoder_scratch_len, BufSource, SeekSource, SliceSource};

// --- stack painting -------------------------------------------------------
// A minimal copy of `mcu-stackprobe`'s `stackmeasure` (the two bins are
// deliberately independent so the proven decode gate stays untouched). Paint
// the free stack `[__sheap .. sp)` with a sentinel, run the workload, then scan
// from the bottom for the deepest overwritten word. The board-relevant number
// is the TRANSIENT = growth BELOW the paint point (`painted_hi - deepest`); see
// `mcu-stackprobe/src/main.rs`'s `measure` doc for why depth-from-RAM-top is the
// wrong (probe-specific) metric.
mod stackmeasure {
    extern "C" {
        static _stack_start: u8;
        static __sheap: u8;
    }
    const SENTINEL: u32 = 0xAAAA_AAAA;

    fn sheap() -> usize {
        (core::ptr::addr_of!(__sheap) as usize + 3) & !3
    }
    fn stack_start() -> usize {
        core::ptr::addr_of!(_stack_start) as usize
    }
    #[inline(always)]
    fn sp_now() -> usize {
        let sp: usize;
        unsafe { core::arch::asm!("mov {}, sp", out(reg) sp, options(nomem, nostack)) };
        sp
    }

    /// Fill `[__sheap .. sp)` with SENTINEL; returns the sp it painted up to.
    pub fn paint() -> usize {
        let lo = sheap();
        let hi = sp_now() - 64;
        let mut p = lo;
        while p + 4 <= hi {
            unsafe { core::ptr::write_volatile(p as *mut u32, SENTINEL) };
            p += 4;
        }
        hi
    }

    /// `(transient_below_paint, total_from_ram_top)` bytes — see the module doc.
    pub fn measure(painted_hi: usize) -> (usize, usize) {
        let lo = sheap();
        let top = stack_start();
        let mut deepest = painted_hi;
        let mut p = lo;
        while p + 4 <= painted_hi {
            if unsafe { core::ptr::read_volatile(p as *const u32) } != SENTINEL {
                deepest = p;
                break;
            }
            p += 4;
        }
        (painted_hi - deepest, top - deepest)
    }
}

/// A short stereo Matroska/WebM-wrapped FLAC track — the decode worst case (see
/// `mcu-stackprobe/src/main.rs`'s episode 2). Reused byte-for-byte from
/// soul-audio-core's corpus. Path is relative to THIS file (`src/bin/`), one
/// level deeper than `src/main.rs`, hence five `../`.
const MKA_FLAC_FIXTURE: &[u8] =
    include_bytes!("../../../../../soul-audio-core/testdata/tone-flac.mka");

/// A [`Storage`] that hands back the in-memory [`MKA_FLAC_FIXTURE`] for any
/// track — unlike `dap_core::stub::StubStorage` (whose `open` always fails), so
/// the App's first `pump` actually runs the real `open_seekable` decoder
/// construction. Its `Source` is a `SliceSource` over the fixture bytes
/// (`BufSource + SeekSource`, as `Storage::Source` requires).
struct FixtureStorage;

impl Storage for FixtureStorage {
    type Source = SliceSource<'static>;
    type Error = ();

    fn open(&mut self, _track: usize) -> Result<Self::Source, Self::Error> {
        let src = SliceSource::new(MKA_FLAC_FIXTURE);
        // Assert the bounds `Storage::Source` promises actually hold for this
        // concrete type (compile-time no-op; documents intent).
        fn _assert_bounds<S: BufSource + SeekSource>(_: &S) {}
        _assert_bounds(&src);
        Ok(src)
    }
}

/// The board's real `App`, but over [`FixtureStorage`] so decode actually runs.
/// `PRELOAD = 0`, matching the board.
type TickApp =
    App<StubSink, FixtureStorage, &'static mut [i32], StubPower, StubControls, StubDisplay, 0>;

/// `App` is large (~175 KiB — it embeds a full decoder by value); build it once
/// into static storage via `StaticCell`, off the measured stack, exactly as
/// `board-h7a3/src/main.rs` does.
static APP: StaticCell<TickApp> = StaticCell::new();

/// The externalized decoder scratch the App takes per track-open (256 KiB),
/// `StaticCell`-backed so it lives in `.bss`, not on the stack.
static SCRATCH: StaticCell<[i32; decoder_scratch_len()]> = StaticCell::new();

fn tick_scratch() -> &'static mut [i32] {
    SCRATCH.init([0; decoder_scratch_len()])
}

#[entry]
fn main() -> ! {
    // Paint BEFORE any deep work; `main`'s own frame is already reserved.
    let painted_hi = stackmeasure::paint();
    let after_paint = stackmeasure::measure(painted_hi).0;

    // Wire the real App over the fixture storage (every other port a stub),
    // constructed in place in APP's storage (not on this stack) via init_with.
    let app: &'static mut TickApp = APP.init_with(|| {
        App::new(
            StubSink,
            FixtureStorage,
            StubPower,
            StubControls,
            StubDisplay,
            AppConfig::default(),
            tick_scratch,
        )
    });
    // Arm one track so the first tick's pump opens + decodes it (FixtureStorage
    // succeeds, so poll_open → open_seekable → the deep decoder construction
    // runs, unlike a StubStorage board boot).
    app.set_queue(1, 0);

    // Drive several real firmware ticks. The FIRST does the track-OPEN (the deep
    // decoder construction) inside pump AND a render (last_render_ms == None ⇒
    // due); later ticks do steady-state decode + periodic render. The sentinel
    // is only ever overwritten deeper and never restored, so one measure() after
    // the loop yields the overall peak = max(open, render, steady-state).
    let mut last_render_ms: Option<u64> = None;
    let mut now_ms: u64 = 0;
    // Snapshot right after the first tick (open + first render) for context.
    let _ = firmware::step(app, now_ms, &mut last_render_ms);
    let after_first_tick = stackmeasure::measure(painted_hi).0;
    now_ms += 20;
    for _ in 0..8 {
        let _ = firmware::step(app, now_ms, &mut last_render_ms);
        now_ms += 20;
    }

    let (tick_transient, tick_total) = stackmeasure::measure(painted_hi);

    let _ = cortex_m_semihosting::hprintln!(
        "mcu-tickprobe-qemu: VALIDATE after_paint={} B (expect 0); whole-tick transient = {} KiB ({} B); first-tick(open+render) = {} KiB; probe-total(ctx) = {} KiB",
        after_paint,
        tick_transient / 1024,
        tick_transient,
        after_first_tick / 1024,
        tick_total / 1024,
    );
    cortex_m_semihosting::debug::exit(cortex_m_semihosting::debug::EXIT_SUCCESS);

    loop {}
}
