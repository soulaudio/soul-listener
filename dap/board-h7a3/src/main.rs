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
use dap_core::display::refresh::RefreshAction;
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
/// `main` builds it with `StaticCell::init_with` (below), which constructs `App`
/// in-place inside `APP_CELL`'s storage — the documented guidance for a big `T`
/// (plain `init` would construct on `main`'s stack and move, relying on RVO/NRVO
/// the language does not guarantee; `init_with`'s closure removes that dependence).
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
/// total static storage ~432 KiB leaves ~592 KiB of stack headroom.
///
/// On-target runtime stack high-water is now MEASURED, not just reasoned about:
/// `tools/mcu-stackprobe` runs the real open->decode->pump path under QEMU
/// (`mps2-an500`, Cortex-M7) with stack-painting. The worst-case track-open
/// transient (Matroska-FLAC) is ~315 KiB of stack growth below the call site --
/// it fits the 592 KiB headroom with ~277 KiB to spare. (An earlier revision of
/// this note cited a ~1 MB figure from that harness; that was a measurement bug --
/// it counted the probe's own `main` frame by measuring depth from the top of RAM
/// instead of growth below the call site -- fixed 2026-11-19, see
/// `docs/plans/2026-11-19-flac-stack-externalization.md`.) The remaining caveat:
/// as Phase E swaps the zero-size stub ports for real drivers (an e-ink
/// framebuffer, DMA audio rings), their statics eat into that 592 KiB headroom.
/// `dap-arch-tests`' `board_static_footprint_leaves_stack_room_in_linked_axi_sram`
/// guards that the board's dominant statics (this `App` + the decoder scratch)
/// plus a 256 KiB stack reserve still fit the 1 MB AXI SRAM `memory.x` actually
/// links (NOT the wider 1208 KB `USER_SRAM` the region budgets use) -- add any
/// new large AXI-SRAM static to that accounting, or place it in the as-yet-
/// unlinked AHB SRAM1/2/3 instead.
static APP_CELL: StaticCell<Dap> = StaticCell::new();

/// **Compile-time** memory-budget guard — best-practice enforcement (fail the
/// *build*, not just a host test): the board's dominant AXI-SRAM statics (`App`
/// by value + the decoder scratch) plus a 256 KiB stack reserve must fit the
/// single linked 1 MB AXI-SRAM region. This is the compile-time twin of
/// `dap-arch-tests`' runtime `board_static_footprint_leaves_stack_room_in_linked_axi_sram`
/// guard: with it, a footprint regression that would leave too little stack
/// can't even *link* the firmware. Add any new large AXI-SRAM static to this sum
/// (or place it in the as-yet-unlinked AHB SRAM1/2/3).
const AXI_SRAM_BYTES: usize = 1024 * 1024;
/// Stack headroom kept clear above the board's statics, in bytes.
///
/// **480 KiB — measured, not assumed**: `tools/mcu-stackprobe/check-tick-qemu.sh`
/// enforces this budget against a whole-firmware-tick worst case of 413 KiB
/// (decode-open plus `App::render` in one tick). This was 256 KiB, which was
/// *below* both the 297 KiB isolated decode transient and the 413 KiB whole-tick
/// figure — so the compile-time guard below would have permitted statics that the
/// real stack runs into. Corrected 2026-11-21; keep in lockstep with
/// `check-tick-qemu.sh` and `dap-arch-tests`' `BOARD_STACK_RESERVE`.
/// See `docs/plans/2026-11-21-memory-architecture-and-placement.md`.
const STACK_RESERVE_BYTES: usize = 480 * 1024;
const _: () = assert!(
    core::mem::size_of::<Dap>()
        + soul_audio_core::decoder_scratch_len() * core::mem::size_of::<i32>()
        + STACK_RESERVE_BYTES
        <= AXI_SRAM_BYTES,
    "board static footprint (App + decoder scratch) + 256 KiB stack reserve exceeds the 1 MB \
     AXI-SRAM — shrink a buffer or move a static to AHB SRAM1/2/3",
);

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let _p = embassy_stm32::init(Default::default());
    info!("board-h7a3: boot");

    // `init_with` (not `init`) so the large `App` is constructed in-place inside
    // `APP_CELL`'s storage rather than built on `main`'s stack and moved — the
    // documented `StaticCell` guidance for big `T` ("If T is big, [init] is likely
    // to cause stack overflows. Consider init_with instead"). This makes the
    // construction robust instead of depending on the optimizer's RVO/NRVO to
    // elide the move (Rust has no stable placement-new). See the `APP_CELL` note.
    let app: &'static mut Dap = APP_CELL.init_with(|| {
        App::new(
            StubSink,
            StubStorage,
            StubPower,
            StubControls,
            StubDisplay,
            AppConfig::default(),
            board_scratch,
        )
    });
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
        // The refresh policy (dap_core::display::refresh) decides whether a
        // redrawn frame is worth committing to the panel at all, so log which of
        // the three outcomes happened rather than just "refreshed" — on real
        // hardware the distinction is a 3 s full flash vs a 300 ms partial vs
        // nothing.
        match outcome.render {
            Some(Ok(RefreshAction::Full)) => info!("board-h7a3: display full refresh"),
            Some(Ok(RefreshAction::Partial(_))) => info!("board-h7a3: display partial refresh"),
            Some(Ok(RefreshAction::None)) => info!("board-h7a3: frame redrawn, panel unchanged"),
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
