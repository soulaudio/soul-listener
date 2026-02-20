# Speaker System Design

**Date:** 2026-02-20
**Status:** Approved
**Scope:** Add stereo internal speakers + 3.5mm headphone jack to DAP hardware design

---

## Overview

Add two small full-range speaker drivers (above and below the e-ink panel) driven by a TI TAS5805M
stereo Class-D amplifier. A switched 3.5mm TRRS headphone jack provides auto-detection: when
headphones are inserted, firmware mutes the speakers via I2C. The existing ES9038Q2M → TPA6120A2
headphone amp chain is unchanged.

---

## Hardware Components

### Speaker Amplifier — TI TAS5805M
- Stereo Class-D, I2S input, I2C control
- Package: VQFN-24 (5×5 mm)
- I2C address: 0x2C (ADR pin pulled to GND via 100kΩ)
- PVDD supply: 5V from BQ25895 PMIC VSYS rail
- Control GPIOs: `PDN` (hard power-down), `FAULT` (open-drain fault, pull-up required)
- Onboard DSP with 15-band parametric EQ — used for 32mm driver frequency compensation

### Speaker Drivers
- 2× 32mm full-range, 4Ω, 1–2W rated
- One above, one below the e-ink panel
- Reference: PUI Audio AS04008MS-R or equivalent
- Sealed back cavity (chassis forms the enclosure)

### Headphone Jack
- Switched TRRS 3.5mm, 5-pin
- Pin 5: normally-closed detect switch, opens when plug inserted
- Detect pin → STM32H743 GPIO with EXTI interrupt

---

## Signal Chain

```
STM32H743 SAI1 (I2S Master)
    BCLK ──────────────┬──────────────────────────
    LRCK ──────────────┼──────────────────────────
    SD   ──────────────┤                          │
                       ▼                          ▼
               ES9038Q2M (I2S slave)      TAS5805M (I2S slave)
               I2C: 0x48                  I2C: 0x2C
                       │                          │
               TPA6120A2 headphone amp    Speaker L / Speaker R
                       │
               3.5mm switched TRRS jack
               Pin 5 detect ──→ STM32 GPIO EXTI
```

**Switching mechanism:** Both chips receive the same I2S stream at all times. Firmware mutes the
inactive output via I2C — no analog mux IC required. TAS5805M soft-mute ramp prevents pops/clicks.

**Power:** TAS5805M PVDD minimum is 4.5V. LiPo (3.7–4.2V) is insufficient directly; BQ25895 VSYS
5V output feeds PVDD.

---

## Firmware Changes

### 1. TAS5805M Driver
**File:** `crates/platform/audio/tas5805m.rs`

New `SpeakerAmp` trait:
```rust
pub trait SpeakerAmp {
    async fn init(&mut self) -> Result<()>;
    async fn set_mute(&mut self, muted: bool) -> Result<()>;
    async fn set_volume(&mut self, db: i8) -> Result<()>;
}
```

Init sequence:
1. Boot delay (5ms after PDN high)
2. Write device configuration registers
3. Load 5-band PEQ biquad coefficients (const table in flash, ~120 bytes):
   - High-pass: 150Hz (protect driver from over-excursion)
   - Bass shelf boost: +4dB @ 250Hz
   - Mid-bass notch: −2dB @ 600Hz
   - Presence boost: +2dB @ 4kHz
4. Transition to play state

### 2. Jack Detection
**File:** `crates/platform/input/jack_detect.rs`

GPIO EXTI interrupt, 5ms debounce. Posts to an Embassy channel:
```rust
static JACK_EVENTS: Channel<CriticalSectionRawMutex, JackEvent, 2> = Channel::new();

pub enum JackEvent {
    Inserted,
    Removed,
}
```

### 3. Audio Output Task
**File:** `crates/firmware/src/audio_output_task.rs`

New Embassy task owning both the TAS5805M driver and jack detect channel:
```rust
#[embassy_executor::task]
async fn audio_output_task(mut speaker_amp: impl SpeakerAmp, mut jack: JackDetect) {
    speaker_amp.init().await.unwrap();
    loop {
        match jack.wait_for_event().await {
            JackEvent::Inserted => speaker_amp.set_mute(true).await.unwrap(),
            JackEvent::Removed  => speaker_amp.set_mute(false).await.unwrap(),
        }
    }
}
```

**No changes to:** playback engine, DMA buffers, SAI configuration, ES9038Q2M driver, UI tasks.

---

## BOM Delta

| # | Component | Part | Qty | Est. Unit Cost | Notes |
|---|-----------|------|-----|----------------|-------|
| 1 | Speaker amp | TI TAS5805M | 1 | ~$3.00 | VQFN-24 5×5mm |
| 2 | Speaker drivers | 32mm full-range, 4Ω, 1–2W | 2 | ~$3.00–5.00 | PUI Audio AS04008MS-R or equiv. |
| 3 | Headphone jack | Switched TRRS 3.5mm, 5-pin | 1 | ~$0.50 | Replaces any plain TRS jack |
| 4 | Output inductors | 10µH, ~2A, shielded | 2 | ~$0.30 | One per channel, near speaker connector |
| 5 | Output caps | 0.68µF, 25V, X7R | 2 | ~$0.10 | Output filter with inductors |
| 6 | PVDD bulk caps | 10µF, 10V, X7R | 4 | ~$0.10 | Per PVDD pin |
| 7 | PVDD decoupling | 100nF, 10V, X7R | 4 | ~$0.05 | Per PVDD pin |
| 8 | Bootstrap caps | 220nF, 10V, X7R | 4 | ~$0.05 | TAS5805M BST pins |
| 9 | I2C pull-ups | 10kΩ | 2 | ~$0.01 | SCL/SDA (if not already present) |
| 10 | ADR resistor | 100kΩ pull-down | 1 | ~$0.01 | Sets I2C address 0x2C |

**Approximate BOM delta: ~$12–16** depending on speaker driver choice.

---

## Open Decisions

- [ ] **I2S tap:** Share SAI1 with ES9038Q2M vs. dedicate SAI2 to TAS5805M independently
- [ ] **Speaker driver final selection:** Confirm mechanical fit above/below e-ink panel; confirm 4Ω
- [ ] **VSYS load budget:** Confirm BQ25895 VSYS can sustain ~1A peak under speaker load
- [ ] **Output filter:** Verify 10µH + 0.68µF adequate for chosen drivers; may need EMI testing

---

## References

- [TI TAS5805M Datasheet](https://www.ti.com/lit/ds/symlink/tas5805m.pdf)
- [TI E2E — I2S multi-slave configurations](https://e2e.ti.com/support/audio-group/audio/f/audio-forum/426477/i2s-multi-slave-multi-master-configurations)
- FX-Audio D01 (ES9038Q2M + TPA6120A2 reference implementation)
- Topping DX3 Pro+ (ES9038Q2M + TPA6120A2 reference implementation)
