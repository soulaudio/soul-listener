#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use static_cell::StaticCell;

// Phase-1 de-risking spike (migration/phase-1): a minimal but real use of
// soul-audio-core, to force its decode code into the ELF and prove it links
// for thumbv7em-none-eabihf. Not meaningful audio -- `open()` is expected to
// fail on this empty/bogus buffer; the point is that the call compiles,
// links, and runs.
//
// `open()` takes the externalized decoder scratch (see soul-audio-core's
// `decoder_scratch_len`/`AnyDecoder<S, P>` -- the board-blocking ~1.44 MB
// open-path transient this was built to fix) as a caller-provided `P: AsRef
// <[i32]> + AsMut<[i32]>`. This is only a no-op smoke test -- no real pool is
// wired here (that's PCB-blocked future work) -- so a single static,
// `StaticCell`-backed `[i32; decoder_scratch_len()]` is the simplest sound
// (no `unsafe`) way to hand `open()` a real `&'static mut [i32]` without an
// allocator: the buffer lives in static storage, not on this task's stack
// (a plain local array here would just reintroduce the oversized transient
// on the stack instead of inline in `AnyDecoder` -- no improvement).
static SCRATCH: StaticCell<[i32; soul_audio_core::decoder_scratch_len()]> = StaticCell::new();

fn probe_soul_audio_core() {
    use soul_audio_core::{decoder_scratch_len, open, SliceSource};

    let bogus: &[u8] = &[0u8; 16];
    let src = SliceSource::new(bogus);
    let scratch: &'static mut [i32] = SCRATCH.init([0; decoder_scratch_len()]);
    match open(src, scratch) {
        Ok(_decoder) => info!("soul-audio-core: open() unexpectedly succeeded"),
        Err(_e) => info!("soul-audio-core: open() returned an error, as expected for bogus data"),
    }
}

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let _p = embassy_stm32::init(Default::default());
    info!("board-h7a3: boot");
    probe_soul_audio_core();
    loop {
        info!("heartbeat");
        Timer::after(Duration::from_secs(1)).await;
    }
}
