#![no_std]
#![no_main]

use defmt::info;
use defmt_rtt as _;
use panic_probe as _;

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};

// Phase-1 de-risking spike (migration/phase-1): a minimal but real use of
// soul-audio-core, to force its decode code into the ELF and prove it links
// for thumbv7em-none-eabihf. Not meaningful audio -- `open()` is expected to
// fail on this empty/bogus buffer; the point is that the call compiles,
// links, and runs.
fn probe_soul_audio_core() {
    use soul_audio_core::{SliceSource, open};

    let bogus: &[u8] = &[0u8; 16];
    let src = SliceSource::new(bogus);
    match open(src) {
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
