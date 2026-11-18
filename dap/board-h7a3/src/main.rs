//! `board-h7a3` — the STM32H7A3 firmware entry point.
//!
//! Now `#![forbid(unsafe_code)]`. The one `unsafe` block this file used to carry
//! (`HEAP.init`, for a 4 KB `embedded_alloc` arena) is gone: that heap existed
//! ONLY to satisfy a link-time `alloc` requirement leaked by the v1 `eink-system`
//! display toolkit. ui-4 moved the DAP onto the no-alloc `eink-ui` toolkit
//! (proven by `dap-sim`'s `alloc_policy` test), so the whole embedded path is
//! alloc-free and this crate needs neither an allocator nor `unsafe`.
#![no_std]
#![no_main]
#![forbid(unsafe_code)]

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;

use dap_core::app::{App, AppConfig};
use dap_core::firmware;
use dap_core::stub::{StubControls, StubDisplay, StubPower, StubSink, StubStorage};

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
/// by value -- so it is built once at boot and moved into static storage via
/// `StaticCell`, the same pattern as `SCRATCH` above and `dap-sim`'s
/// `tests/static_placement.rs`'s `PLAYER_CELL`.
///
/// Read that test's doc comment before assuming the move is free: `App::new`'s
/// return value is constructed on `main`'s own stack FIRST, and only then moved
/// into `APP_CELL`'s backing storage by `StaticCell::init` (Rust has no stable
/// placement-new). The optimizer usually elides that move via RVO/NRVO, but the
/// language gives no guarantee of it.
///
/// Where the stack lives, MEASURED (rust-lld, `--features hardware`, `llvm-nm`
/// on the linked ELF -- not the 128 KB DTCM an earlier note here assumed):
/// `memory.x` maps a single RAM region, the 1 MB AXI SRAM, and cortex-m-rt puts
/// the stack at the top of it. So the "put the main stack in AXI SRAM, not DTCM"
/// recommendation (`dap-sim/tests/stack_highwater.rs`) is already in force here.
/// This board's real `App` -- `App<stubs, &'static mut [i32], PRELOAD = 0>` --
/// is 174.7 KiB (`dap-arch-tests`' `component_footprint_roster` measures the
/// same from the same instantiation; a stale comment there still cites 332 KiB
/// from the old PRELOAD=1 / pre-externalization era), `SCRATCH` is 256 KiB, and
/// total static storage ~432 KiB leaves ~592 KiB of stack headroom. So even a
/// non-elided ~175 KiB App-by-value transient fits with ~3.4x margin -- this
/// construction is NOT the overflow risk an earlier revision of this note feared.
///
/// Two honest caveats remain. (1) On-target runtime stack high-water is still
/// unverified -- that wants QEMU stack-painting (deferred, no admin rights on
/// this dev machine -- see the roadmap's Phase A1) or a flashed board. (2) As
/// Phase E swaps the zero-size stub ports for real drivers (an e-ink
/// framebuffer, DMA audio rings), their statics eat into that 592 KiB headroom.
/// `dap-arch-tests`' `board_static_footprint_leaves_stack_room_in_linked_axi_sram`
/// guards that the board's dominant statics (this `App` + the decoder scratch)
/// plus a 256 KiB stack reserve still fit the 1 MB AXI SRAM `memory.x` actually
/// links (NOT the wider 1208 KB `USER_SRAM` the region budgets use) -- add any
/// new large AXI-SRAM static to that accounting, or place it in the as-yet-
/// unlinked AHB SRAM1/2/3 instead.
static APP_CELL: StaticCell<Dap> = StaticCell::new();

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let _p = embassy_stm32::init(Default::default());
    info!("board-h7a3: boot");

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
