# Audio Architecture

## Overview

The audio system consists of two independent output paths sharing a single I2S master clock:

- **Headphone output** — ES9038Q2M DAC → TPA6120A2 amp → 3.5mm switched TRRS jack
- **Speaker output** — ES9038Q2M DAC (shared I2S) → TAS5805M Class-D amp → 2× 32mm drivers

Only one output is active at a time. Jack insertion is detected via hardware switch and handled in firmware — no analog multiplexer is needed.

---

## Signal Chain

```
                        STM32H743 SAI1
                        (I2S Master)
                        BCLK / LRCK / SD
                              │
              ┌───────────────┴───────────────┐
              ▼                               ▼
       ES9038Q2M                         TAS5805M
       I2S slave                         I2S slave
       I²C: 0x48                         I²C: 0x2C
       (DAC + vol/filter)                (Class-D + DSP/PEQ)
              │                               │
       TPA6120A2                       Speaker L + Speaker R
       headphone amp                   (2× 32mm, 4Ω, 1–2W)
       class-AB, 250 mA                above + below e-ink
              │
       3.5mm switched TRRS jack
       pin 5 detect ──→ STM32 GPIO EXTI
```

Both chips receive the same I2S stream continuously. The inactive output is muted via I2C — TAS5805M uses its built-in soft-mute ramp to prevent pops.

---

## Hardware Components

### DAC — ES9038Q2M (ESS Technology)
- 32-bit / 768 kHz PCM, DSD512 (native + DoP)
- 128 dB DNR, −120 dB THD+N
- I²C programmable: volume, digital filter, oversampling mode
- I2S slave on SAI1; also feeds TAS5805M on the same bus

### Headphone Amplifier — TPA6120A2 (TI)
- Class-AB, 250 mA output current
- Driven by ES9038Q2M analog output
- Always powered; ES9038Q2M handles mute when speakers are active

### Headphone Jack — Switched TRRS 3.5mm (5-pin)
- Pin 5: normally-closed detect switch, opens on plug insertion
- Detect → STM32H743 GPIO with EXTI interrupt
- Debounced 5 ms in firmware

### Speaker Amplifier — TAS5805M (TI)
- Stereo Class-D, filterless output
- I2S slave on SAI1 (shared with ES9038Q2M)
- I²C control: volume, mute, fault reporting, DSP coefficient loading
- I²C address: 0x2C (ADR pin → 100kΩ pull-down to GND)
- PVDD: 5V from BQ25895 PMIC VSYS rail
- Control GPIOs: `PDN` (hard power-down), `FAULT` (open-drain, pull-up required)
- Onboard DSP runs parametric EQ at init — see PEQ section below

### Speakers
- 2× 32mm full-range driver, 4Ω, 1–2W rated
- One above, one below the e-ink panel
- Sealed back cavity (chassis forms enclosure)
- Reference: PUI Audio AS04008MS-R or equivalent

---

## I2S / SAI Configuration

**SAI1 as I2S master:**
- Standard I2S protocol, 32-bit frames, stereo
- BCLK, LRCK, and SD lines run in parallel to both ES9038Q2M and TAS5805M
- Both chips are I2S slaves; STM32 is the sole master

**DMA:**
- SAI1 TX DMA in circular ping-pong mode
- Buffers in AXI SRAM (DMA-accessible, D1 domain)
- Playback engine writes decoded audio directly into DMA buffer (zero-copy)

```rust
// AXI SRAM — must be DMA-accessible
const AUDIO_BUFFER_SIZE: usize = 8192;  // 2048 samples × 2ch × 2B
static AUDIO_BUFFER: StaticCell<[u8; AUDIO_BUFFER_SIZE]> = StaticCell::new();
```

**Open decision:** Share SAI1 vs. dedicate SAI2 to TAS5805M for independent sample rate / volume control — see [Open Hardware Decisions](../CLAUDE.md#open-hardware-decisions).

---

## TAS5805M Parametric EQ

32mm full-range drivers have poor bass response and a pronounced mid-bass peak. The TAS5805M DSP
applies a 5-band biquad PEQ at startup to compensate:

| Band | Type | Freq | Gain | Q | Purpose |
|------|------|------|------|---|---------|
| 1 | High-pass | 150 Hz | — | 0.7 | Protect driver from over-excursion |
| 2 | Low shelf | 250 Hz | +4 dB | — | Restore bass body |
| 3 | Peak/notch | 600 Hz | −2 dB | 2.0 | Reduce mid-bass honk |
| 4 | Peak | 4 kHz | +2 dB | 1.5 | Presence / clarity |
| 5 | High shelf | 10 kHz | +1 dB | — | Air / openness |

Coefficients are a `const` table in flash loaded over I²C during `SpeakerAmp::init()`. Values are
starting points — tune on physical hardware with a measurement mic.

---

## Firmware Architecture

### Traits (`crates/platform/audio/`)

```rust
pub trait SpeakerAmp {
    async fn init(&mut self) -> Result<()>;           // boot sequence + PEQ load
    async fn set_mute(&mut self, muted: bool) -> Result<()>;
    async fn set_volume(&mut self, db: i8) -> Result<()>;
}
```

`Tas5805m` implements `SpeakerAmp` for hardware. A `MockSpeakerAmp` implements it for host tests.

### Jack Detection (`crates/platform/input/jack_detect.rs`)

```rust
static JACK_EVENTS: Channel<CriticalSectionRawMutex, JackEvent, 2> = Channel::new();

pub enum JackEvent { Inserted, Removed }
```

GPIO EXTI interrupt fires on pin 5 edge, debounced 5 ms, posts to channel.

### Audio Output Task (`crates/firmware/src/audio_output_task.rs`)

Owns the TAS5805M driver and jack detect channel. Responds to jack events:

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

The playback pipeline (`audio_playback_task`) is unaware of output routing — it writes to the SAI
DMA buffer regardless of which output is active.

---

## Output Filter (TAS5805M)

Filterless Class-D uses the speaker coil inductance as the output filter. A small LC network on
each output improves EMI compliance:

- Series inductor: 10µH, ~2A rated, shielded, placed near speaker connector
- Shunt capacitor: 0.68µF, 25V, X7R

**Open decision:** Verify filter values suit final speaker driver selection; adjust after EMI test.

---

## Power

| Rail | Source | Consumer | Peak current |
|------|--------|----------|-------------|
| PVDD (5V) | BQ25895 VSYS | TAS5805M | ~1A (both channels, 1W/ch into 4Ω) |
| AVCC (3.3V) | LDO | ES9038Q2M, TPA6120A2 | ~150 mA |

**Open decision:** Confirm BQ25895 VSYS can sustain ~1A speaker load alongside USB, display, and MCU.

---

## Supported Formats

| Format | Decoder | Max spec |
|--------|---------|----------|
| FLAC | libfoxenflac (C FFI) | 32-bit / 192 kHz |
| MP3 | minimp3 (C FFI) | 320 kbps CBR/VBR |
| WAV | Native | 32-bit / 768 kHz PCM |
| DSD | ES9038Q2M native | DSD512 (DoP or native) |

---

## References

- [ES9038Q2M Datasheet](https://www.esstech.com/index.php/en/products/sabre-digital-analog-converters/sabre-hifi-mobile-series/sabre9038q2m/)
- [TPA6120A2 Datasheet](https://www.ti.com/product/TPA6120A2)
- [TAS5805M Datasheet](https://www.ti.com/lit/ds/symlink/tas5805m.pdf)
- [STM32H7 SAI Reference Manual](https://www.st.com/resource/en/reference_manual/rm0433-stm32h742-stm32h743753-and-stm32h750-value-line-advanced-armbased-32bit-mcus-stmicroelectronics.pdf)
- `docs/plans/2026-02-20-speaker-system-design.md` — speaker system design decisions
