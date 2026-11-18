#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;

use dap_core::app::{App, AppConfig};
use dap_core::firmware;
use dap_core::stub::{StubControls, StubDisplay, StubPower, StubSink, StubStorage};
use embedded_alloc::LlffHeap as Heap;

// `dap-core`'s `display` port re-exports `eink-system`'s container/layout
// toolkit (unused by the current Now-Playing screen, but always compiled in
// as part of `eink-system` itself), which unconditionally needs `extern
// crate alloc` (`Box`/a boxed-child `heapless::Vec`) in its non-`std` build
// -- see `Cargo.toml`'s comment on the `embedded-alloc` dependency. This
// tiny static arena is ONLY here to satisfy that link-time requirement; no
// code reachable from this binary today actually allocates from it (the
// Now-Playing screen draws with raw embedded-graphics primitives). Sized
// generously above zero, far below any real SRAM budget.
#[global_allocator]
static HEAP: Heap = Heap::empty();
const HEAP_SIZE_BYTES: usize = 4 * 1024;
static mut HEAP_MEM: [core::mem::MaybeUninit<u8>; HEAP_SIZE_BYTES] =
    [core::mem::MaybeUninit::uninit(); HEAP_SIZE_BYTES];

// Phase B (see docs/plans/2026-11-17-firmware-selfimproving-loop-roadmap.md):
// this board now constructs the REAL `App` -- audio + power + controls +
// display, all five ports wired -- and drives it through
// `dap_core::firmware::step`, the exact same run-loop function
// `dap-sim/tests/firmware_step.rs` drives on the host. No real hardware
// exists yet (SAI/I2S DMA, the SD card, the e-ink SPI panel, the
// wheel/button GPIOs, the fuel-gauge I2C -- all Phase E), so every port is
// the `dap_core::stub` placeholder adapter for now; each one is replaced by
// a real driver, independently, as hardware comes online. This supersedes
// the old smoke test (a bare `soul_audio_core::open()` call on bogus bytes):
// `AudioPlayer::pump`'s real call chain -- `Engine::poll_open` ->
// `soul_audio_core::open_seekable` -> every format decoder behind
// `AnyDecoder` -- is reachable from `main` below (even though `StubStorage`
// always fails before a decode ever actually runs), so it forces strictly
// MORE real code into the linked ELF than the old probe did, not less.

/// Gapless preload depth -- 0 = single resident decoder; gapless comes from
/// the output ring at a track reopen (the Rockbox model), not a second
/// preloaded decoder. Matches every `dap-sim` fixture's real value (see
/// e.g. `dap/sim/tests/device_headless.rs`).
const PRELOAD: usize = 0;

/// The externalized decoder scratch `AudioPlayer`/`App` take per track open
/// (see `soul_audio_core::decoder_scratch_len`) -- a `StaticCell`-backed
/// static, the sound (no `unsafe`) way to hand out a real `&'static mut
/// [i32]` with no allocator (same pattern the old smoke test's `SCRATCH`
/// used, and `dap-sim`'s `tests/static_placement.rs`'s `PLAYER_CELL`).
///
/// `StubStorage::open` always fails (see its doc comment), and
/// `AudioPlayer::pump` only ever calls `scratch_fn` *after* `Storage::open`
/// succeeds -- so in THIS build, `board_scratch` below is never actually
/// called at runtime. It still has to exist and type-check (`AppConfig`'s
/// `scratch_fn: fn() -> Pcm` is unconditional), and a real board swapping in
/// working `Storage` later needs exactly this shape, so it's wired for real
/// here rather than stubbed with an `unreachable!()`.
static SCRATCH: StaticCell<[i32; soul_audio_core::decoder_scratch_len()]> = StaticCell::new();

fn board_scratch() -> &'static mut [i32] {
    SCRATCH.init([0; soul_audio_core::decoder_scratch_len()])
}

/// The real `App`, generic over the board's five ports -- all
/// `dap_core::stub` placeholders until Phase E's real drivers replace them
/// one at a time.
type Dap =
    App<StubSink, StubStorage, &'static mut [i32], StubPower, StubControls, StubDisplay, PRELOAD>;

/// `App` is large -- it holds an `AudioPlayer` holding a full `AnyDecoder`
/// by value (`dap-arch-tests`' `component_footprint_roster` test measures
/// the exact current byte count) -- so it is built once at boot and moved
/// into static storage via `StaticCell`, the same pattern as `SCRATCH`
/// above and `dap-sim`'s `tests/static_placement.rs`'s `PLAYER_CELL`.
///
/// Read that test's doc comment before assuming this is free: `App::new`'s
/// return value is constructed on `main`'s own stack FIRST, and only then
/// moved into `APP_CELL`'s backing storage by `StaticCell::init` (there is
/// no stable placement-new in Rust). The optimizer usually elides that move
/// via RVO/NRVO, but the language gives no guarantee of it. This board's
/// real on-target stack headroom for that construction-time transient is
/// NOT verified by this change -- it needs QEMU stack-painting (deferred,
/// no admin rights on this dev machine -- see the roadmap's Phase A1) or a
/// flashed board. See this crate's own task report for the concrete known
/// risk: `App`'s measured size is well beyond the H7A3's 128 KB DTCM (where
/// the main stack lives), so this transient likely needs a non-stack
/// construction path (e.g. piecewise field-init directly into AXI SRAM)
/// before it's safe to run for real, even though it links and is expected
/// to behave correctly once past this one construction call.
static APP_CELL: StaticCell<Dap> = StaticCell::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let _p = embassy_stm32::init(Default::default());
    info!("board-h7a3: boot");

    // SAFETY: `HEAP_MEM`'s address/length are handed to `Heap::init` exactly
    // once, here, before any allocation can occur (nothing above this line
    // allocates) -- the one-time-init precondition `embedded_alloc::LlffHeap::init`
    // documents.
    #[allow(static_mut_refs)]
    unsafe {
        HEAP.init(HEAP_MEM.as_ptr() as usize, HEAP_SIZE_BYTES);
    }

    let app = App::new(
        StubSink,
        StubStorage,
        StubPower,
        StubControls,
        StubDisplay,
        AppConfig::default(),
        board_scratch,
    );
    let app: &'static mut Dap = APP_CELL.init(app);
    info!("board-h7a3: App constructed (audio + power + controls + display wired)");

    // One track "queued" against storage that always reports NoMedia -- see
    // `StubStorage`'s doc comment. This still exercises the engine's real
    // poll_open/open/provide call chain every tick (storage.open fails,
    // AudioPlayer::pump returns an error, App::tick swallows it into
    // `TickOutcome::pump`), it just never actually decodes anything, same
    // as any board boot before its SD card Storage adapter exists.
    app.set_queue(1, 0);
    info!("board-h7a3: queue armed");

    let mut last_render_ms: Option<u64> = None;
    let mut now_ms: u64 = 0;
    const PERIOD_MS: u64 = 20;

    loop {
        // The one run-loop step this board and `dap-sim`'s
        // `tests/firmware_step.rs` both call -- see `dap_core::firmware`'s
        // module docs ("test what you ship").
        let outcome = firmware::step(app, now_ms, &mut last_render_ms);

        if outcome.tick.play_pause_toggled {
            info!("board-h7a3: play/pause toggled");
        }
        if outcome.tick.track_advanced {
            info!("board-h7a3: track advanced");
        }
        if outcome.tick.low_battery {
            info!("board-h7a3: low battery");
        }
        if outcome.tick.critical_shutdown {
            info!("board-h7a3: critical shutdown");
        }
        match outcome.render {
            Some(Ok(())) => info!("board-h7a3: display refreshed"),
            Some(Err(_)) => info!("board-h7a3: display refresh failed"),
            None => {}
        }

        if now_ms.is_multiple_of(5_000) {
            info!(
                "board-h7a3: heartbeat t={=u64}ms volume={=f32} soc_permille={=u16}",
                now_ms,
                app.volume(),
                app.power_soc()
            );
        }

        now_ms = now_ms.saturating_add(PERIOD_MS);
        Timer::after(Duration::from_millis(PERIOD_MS)).await;
    }
}
