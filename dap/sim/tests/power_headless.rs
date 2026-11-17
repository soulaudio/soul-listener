//! Headless, hardware-free proof that `dap_core::power::PowerService`'s
//! state machine drives real cross-cutting effects -- pausing/stopping an
//! actual `AudioPlayer`, throttling charge current, switching clock
//! profiles, sleeping/waking -- when driven against a scripted
//! `dap_sim::SimBattery` instead of real fuel-gauge/charger hardware.
//!
//! Each scenario is a driver loop that repeatedly calls `PowerService::poll`
//! (advancing `now_ms` in small steps) and acts on the events it returns,
//! exactly as a real firmware driver loop would. `PowerService` smooths SoC
//! with an EMA (see `dap_core::power`'s module docs), so every
//! threshold-crossing scenario here drives many small poll steps across a
//! long enough timeline for the *smoothed* SoC to actually cross the
//! threshold -- asserting after a single poll would only prove the raw
//! reading crossed it, not that the service's own logic did.

use std::path::{Path, PathBuf};

use dap_core::audio::AudioPlayer;
use dap_core::power::{PowerConfig, PowerEvent, PowerProfile, PowerService};
use dap_sim::{sac_scratch, BatteryKeyframe, SimBattery, SimSink, SimStorage};
use soul_audio_core::{DeckState, Progress, Quality};

// 0 = single resident decoder; gapless via the output ring at track reopen
// (Rockbox model) — the board's SAI DMA ring must be deep enough to cover
// the reopen's wall-clock latency.
const PRELOAD: usize = 0;

fn tone_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/tone.wav")
}

fn antiphase_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../soul-audio-core/testdata/antiphase.wav")
}

type Player = AudioPlayer<SimSink, SimStorage, Box<[i32]>, PRELOAD>;

fn build_player() -> Player {
    let sink = SimSink::new();
    let storage = SimStorage::new([tone_path(), antiphase_path()]);
    let mut player = AudioPlayer::new(sink, storage, Quality::Fast, sac_scratch);
    player.set_queue(2, 0);
    player.play();
    player
}

/// Keep the queue looping forever without depending on
/// `soul_audio_engine::RepeatMode` (not exposed through `dap-sim`'s
/// dependency set): on `Ended`, rewind to the start and resume. The fixture
/// queue is only ~4s of audio; the discharge scenario's battery timeline
/// runs much longer than that, so the queue must keep going on its own.
fn pump_keep_playing(player: &mut Player) {
    if let Progress::Ended = player.pump().expect("pump") {
        player.goto(0).expect("goto 0");
        player.play();
    }
}

#[test]
fn discharge_to_critical_while_playing_pauses_then_stops() {
    // Generous worker stack -- same reason as `tests/audio_headless.rs`:
    // `Deck`/`Engine` embed `AnyDecoder` by value across several unoptimized
    // (debug) call frames.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(discharge_to_critical_while_playing_pauses_then_stops_inner)
        .expect("spawn worker thread")
        .join()
        .expect("worker thread panicked");
}

fn discharge_to_critical_while_playing_pauses_then_stops_inner() {
    let mut player = build_player();
    // Unplugged, discharging 100% -> 0% over 200s of simulated time, safe
    // temperature throughout -- crosses both the low-battery (15%) and
    // critical (3%) thresholds on the way down.
    let mut battery = SimBattery::new([
        BatteryKeyframe::new(0, 1000, 2500, false),
        BatteryKeyframe::new(200_000, 0, 2500, false),
    ]);
    let mut svc = PowerService::default();

    let mut paused_at: Option<u64> = None;
    let mut frames_at_pause: Option<u64> = None;
    let mut stopped_at: Option<u64> = None;

    let step_ms = 250u64;
    let mut t = 0u64;
    while t <= 200_000 {
        battery.set_now_ms(t);

        // Once stopped, the driver stops driving playback (matches a real
        // power-off: nothing pumps the deck anymore).
        if stopped_at.is_none() {
            pump_keep_playing(&mut player);
        }

        for ev in svc.poll(&mut battery, t, true).iter() {
            match ev {
                PowerEvent::LowBattery => {
                    player.pause();
                    if paused_at.is_none() {
                        paused_at = Some(t);
                        frames_at_pause = Some(player.position_frames());
                    }
                }
                PowerEvent::CriticalShutdown => {
                    if stopped_at.is_none() {
                        // Prove the pause actually held the deck still
                        // between the warn and critical points, not just
                        // that `pause()` was called once.
                        assert_eq!(
                            player.position_frames(),
                            frames_at_pause.expect("LowBattery must fire before CriticalShutdown"),
                            "player must not have advanced while paused"
                        );
                    }
                    player.stop();
                    if stopped_at.is_none() {
                        stopped_at = Some(t);
                    }
                }
                _ => {}
            }
        }

        t += step_ms;
    }

    let paused_at = paused_at.expect("LowBattery never fired across the discharge");
    let stopped_at = stopped_at.expect("CriticalShutdown never fired across the discharge");
    assert!(paused_at < stopped_at, "warn must precede critical");
    assert_eq!(
        player.state(),
        DeckState::Stopped,
        "critical must stop the deck"
    );
    assert_eq!(
        battery.power_off_calls(),
        1,
        "request_power_off must be recorded exactly once"
    );
}

#[test]
fn overheat_throttles_then_restores_charge_current() {
    let mut svc = PowerService::default();
    let cfg = PowerConfig::default();
    // Charging at mid-SoC, safe temp -> jumps out of the safe band -> back in.
    let mut battery = SimBattery::new([
        BatteryKeyframe::new(0, 500, 2500, true),
        BatteryKeyframe::new(1_000, 500, 2500, true),
        BatteryKeyframe::new(1_001, 500, 5500, true),
        BatteryKeyframe::new(3_000, 500, 5500, true),
        BatteryKeyframe::new(3_001, 500, 2500, true),
        BatteryKeyframe::new(5_000, 500, 2500, true),
    ]);

    battery.set_now_ms(0);
    let ev = svc.poll(&mut battery, 0, true);
    assert!(
        !ev.iter()
            .any(|e| matches!(e, PowerEvent::ThermalThrottle { .. })),
        "must not throttle while in the safe band"
    );
    assert!(battery.charge_current_calls().is_empty());

    battery.set_now_ms(1_001);
    let ev = svc.poll(&mut battery, 1_001, true);
    assert!(
        ev.contains(&PowerEvent::ThermalThrottle {
            charge_ma: cfg.throttled_charge_current_ma
        }),
        "pack above the safe temp band must throttle"
    );
    assert_eq!(
        battery.charge_current_calls().last().copied(),
        Some(cfg.throttled_charge_current_ma),
        "a reduced (throttled) charge current must be recorded"
    );

    battery.set_now_ms(3_001);
    let ev = svc.poll(&mut battery, 3_001, true);
    assert!(
        !ev.iter()
            .any(|e| matches!(e, PowerEvent::ThermalThrottle { .. })),
        "cooling back into the safe band must not re-fire the event"
    );
    assert_eq!(
        battery.charge_current_calls().last().copied(),
        Some(cfg.nominal_charge_current_ma),
        "cooling back into the safe band must restore nominal charge current"
    );
}

#[test]
fn low_soc_engages_battery_saver_profile() {
    let mut svc = PowerService::default();
    let mut battery = SimBattery::new([
        BatteryKeyframe::new(0, 1000, 2500, false),
        BatteryKeyframe::new(100_000, 0, 2500, false),
    ]);

    let mut saw_saver = false;
    let step_ms = 250u64;
    let mut t = 0u64;
    while t <= 100_000 {
        battery.set_now_ms(t);
        let ev = svc.poll(&mut battery, t, true);
        if ev.contains(&PowerEvent::ProfileChanged(PowerProfile::BatterySaver)) {
            saw_saver = true;
        }
        t += step_ms;
    }

    assert!(
        saw_saver,
        "BatterySaver must auto-engage once smoothed SoC crosses the saver threshold"
    );
    assert_eq!(svc.profile(), PowerProfile::BatterySaver);
    assert_eq!(
        battery.profile_calls().last().copied(),
        Some(PowerProfile::BatterySaver),
        "set_clock_profile(BatterySaver) must be recorded"
    );
}

#[test]
fn idle_past_timeout_sleeps_then_activity_wakes() {
    let mut svc = PowerService::default();
    let mut battery = SimBattery::new([BatteryKeyframe::new(0, 800, 2500, false)]);
    let idle_timeout = PowerConfig::default().idle_timeout_ms;

    battery.set_now_ms(0);
    let ev = svc.poll(&mut battery, 0, false);
    assert!(ev.is_empty());
    assert!(!svc.is_asleep());

    battery.set_now_ms(idle_timeout - 1);
    let ev = svc.poll(&mut battery, idle_timeout - 1, false);
    assert!(ev.is_empty(), "must not sleep before the timeout elapses");
    assert!(!svc.is_asleep());

    battery.set_now_ms(idle_timeout);
    let ev = svc.poll(&mut battery, idle_timeout, false);
    assert!(ev.contains(&PowerEvent::EnterSleep));
    assert!(svc.is_asleep());

    battery.set_now_ms(idle_timeout + 1_000);
    let ev = svc.poll(&mut battery, idle_timeout + 1_000, true);
    assert!(ev.contains(&PowerEvent::Wake));
    assert!(!svc.is_asleep());
}

#[test]
fn time_to_empty_is_finite_and_decreases_as_soc_drops() {
    let mut svc = PowerService::default();
    let mut battery = SimBattery::new([
        BatteryKeyframe::new(0, 1_000, 2500, false),
        BatteryKeyframe::new(60_000, 300, 2500, false),
    ]);

    battery.set_now_ms(0);
    svc.poll(&mut battery, 0, true);
    let first = svc
        .time_to_empty_s()
        .expect("discharging with current must estimate a runtime");
    assert!(
        first > 0,
        "sanity: runtime estimate must be positive while discharging"
    );

    let step_ms = 250u64;
    let mut t = step_ms;
    while t <= 60_000 {
        battery.set_now_ms(t);
        svc.poll(&mut battery, t, true);
        t += step_ms;
    }
    let later = svc.time_to_empty_s().expect("still discharging");

    assert!(later < first, "runtime estimate must shrink as SoC drops");
}
