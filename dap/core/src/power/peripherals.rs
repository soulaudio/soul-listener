//! Peripheral **clock-gating policy** — which RCC enable bits are set in which
//! power state, as a complete, host-testable partition of the chip's gateable
//! surface rather than an aspiration in a doc.
//!
//! # Why this module exists
//! The memory/power enforcement roster
//! (`docs/plans/2026-11-20-memory-power-best-practices-and-enforcement.md`)
//! asks for two things under "Peripheral clock gating": *"gate unused APB/AHB
//! clocks explicitly (not NOPs)"*, enforced by *"enable-register discipline; an
//! audit of unused peripherals"*. Neither needs silicon. Both are decidable
//! here:
//!
//! - **Enable-register discipline** is a *type* property. Every [`Peripheral`]
//!   in [`ROSTER`] must name a concrete [`Clock`] — an `(RCC enable register,
//!   bit)` coordinate. There is no way to express "gated" as a software flag or
//!   a spin loop, so the anti-pattern the roster names ("software-NOP clock
//!   gating — use the controller") is unrepresentable rather than merely
//!   discouraged.
//! - **The audit** is a *completeness* property, and the reason this module
//!   works in masks instead of enumerating ~120 peripherals: the gated set is
//!   *derived*, `gated = gateable & !enabled`. A peripheral cannot be
//!   accidentally left unclassified, because not being on the roster **is** its
//!   classification — off. Adding a peripheral to the roster is therefore the
//!   only way to turn a clock on, and it forces a per-state decision at the
//!   same time.
//!
//! What still needs the board is writing the masks to RCC (and the GPIO pin
//! states — see [`ROSTER`]'s note on bank assignment, the one input here that
//! is a placeholder pending the schematic).
//!
//! # The three power states
//! [`PowerState`] is deliberately coarser than [`PowerProfile`](super::PowerProfile)
//! — a *profile* picks a clock (see [`dvfs`](super::dvfs)); a *state* picks which
//! peripherals exist at all:
//!
//! | State | What's running |
//! |---|---|
//! | [`Playback`](PowerState::Playback) | Audio path live: SAI ring, SD reads, plus everything Idle has. |
//! | [`Idle`](PowerState::Idle) | UI awake, nothing playing: display + control I²C, no audio path. |
//! | [`Paused`](PowerState::Paused) | The pre-Stop configuration — only wake sources remain (see `model::STOP_MODE_CURRENT_MA`: 32 µA, ~4 years on the real pack). |
//!
//! The states nest strictly — `Paused ⊂ Idle ⊂ Playback` — which
//! `each_lower_power_state_gates_strictly_more` pins. That is the property that
//! makes "lower power state" mean something concrete instead of being a label.
//!
//! What the policy comes out to, of the part's **119** gateable peripheral
//! clocks: **12 on in Playback, 8 in Idle, 4 in Paused**. So ~90 % of the chip is
//! unclocked even at full tilt — which is the expected shape for a DAP on a
//! general-purpose MCU (no Ethernet, no LTDC/DSI, no crypto, no USB, 8 of 11 GPIO
//! banks, and all but one of ~30 timers), and is exactly the "audit unused
//! peripherals" result the roster asked for, in a form that stays true.
//!
//! # Scope and provenance
//! The enable-bit coordinates and each bus's [`gateable_mask`](Bus::gateable_mask)
//! come from the `rcc_h7ab` RCC block in `stm32-metapac` v21 — the same
//! register definitions `embassy-stm32` drives this part through, generated from
//! ST's own SVD/`stm32-data`. The mask is "the bits the firmware's RCC writes can
//! touch", which is the right surface for a gating policy: a bit belonging to a
//! peripheral absent from this package is reserved, and writing 0 to it is the
//! same no-op as gating one that exists.
//!
//! Pure, `no_std`, no hardware: no register writes, no reset sequencing, no
//! kernel-clock mux selection (`RCC_D*CCIP*R`) — those are `board-h7a3`'s.

/// An RCC peripheral-clock enable register. Each variant is one `RCC_*ENR` whose
/// bits gate a bus's peripherals.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Bus {
    /// `RCC_AHB1ENR` — DMA1/2, ADC12, USB OTG, Ethernet.
    Ahb1,
    /// `RCC_AHB2ENR` — crypto/hash/RNG, SDMMC2, SRAM1-3, DCMI.
    Ahb2,
    /// `RCC_AHB3ENR` — MDMA, SDMMC1, FMC, OCTOSPI, AXI SRAM, DTCM/ITCM.
    Ahb3,
    /// `RCC_AHB4ENR` — GPIO banks, BDMA, CRC, backup SRAM, HSEM.
    Ahb4,
    /// `RCC_APB1LENR` — TIM2-14, SPI2/3, I²C1-3, USARTs, DAC1.
    Apb1L,
    /// `RCC_APB1HENR` — CRS, SWPMI, OPAMP, MDIOS, FDCAN, TIM23/24.
    Apb1H,
    /// `RCC_APB2ENR` — TIM1/8/15-17, SPI1/4/5, SAI1-3, DFSDM1, HRTIM.
    Apb2,
    /// `RCC_APB3ENR` — LTDC, DSI, WWDG1.
    Apb3,
    /// `RCC_APB4ENR` — SYSCFG, LPUART1, SPI6, I²C4, LPTIM2-5, VREF, RTC APB.
    Apb4,
}

/// Number of [`Bus`] variants — the length of [`Bus::ALL`].
pub const BUS_COUNT: usize = 9;

impl Bus {
    /// Every enable register, in [`Bus`] declaration order.
    pub const ALL: [Self; BUS_COUNT] = [
        Self::Ahb1,
        Self::Ahb2,
        Self::Ahb3,
        Self::Ahb4,
        Self::Apb1L,
        Self::Apb1H,
        Self::Apb2,
        Self::Apb3,
        Self::Apb4,
    ];

    /// The set of bits in this register that actually gate a peripheral clock —
    /// i.e. the gating surface. Transcribed from `stm32-metapac` v21's
    /// `rcc_h7ab` block (see the module doc); every bit outside the mask is
    /// reserved.
    ///
    /// This is what makes the audit complete: [`gated_mask`] is
    /// `gateable_mask & !enabled`, so there is no third category for a
    /// peripheral to hide in.
    #[must_use]
    pub const fn gateable_mask(self) -> u32 {
        match self {
            Self::Ahb1 => 0x1E03_C023,
            Self::Ahb2 => 0xE003_0A71,
            Self::Ahb3 => 0xF0E9_5031,
            Self::Ahb4 => 0x1328_07FF,
            Self::Apb1L => 0xEAFF_CBFF,
            Self::Apb1H => 0x0300_0136,
            Self::Apb2 => 0x31D7_30F3,
            Self::Apb3 => 0x0000_0058,
            Self::Apb4 => 0x0421_FEAA,
        }
    }

    /// The register's name, for assertion messages and `defmt` logging.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ahb1 => "RCC_AHB1ENR",
            Self::Ahb2 => "RCC_AHB2ENR",
            Self::Ahb3 => "RCC_AHB3ENR",
            Self::Ahb4 => "RCC_AHB4ENR",
            Self::Apb1L => "RCC_APB1LENR",
            Self::Apb1H => "RCC_APB1HENR",
            Self::Apb2 => "RCC_APB2ENR",
            Self::Apb3 => "RCC_APB3ENR",
            Self::Apb4 => "RCC_APB4ENR",
        }
    }
}

/// One peripheral-clock enable bit: the `(register, bit)` coordinate the board
/// writes. Requiring this on every [`Peripheral`] is what makes gating a
/// *controller* operation by construction — there is no variant of this type
/// that means "handled in software".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clock {
    /// The enable register holding this peripheral's bit.
    pub bus: Bus,
    /// The bit position within [`bus`](Self::bus) (`0..=31`).
    pub bit: u8,
}

impl Clock {
    /// This clock as a single-bit mask.
    #[must_use]
    pub const fn mask(self) -> u32 {
        1u32 << self.bit
    }

    /// Whether this coordinate names a real gateable bit (i.e. is inside its
    /// bus's [`Bus::gateable_mask`]). A typo'd bit would otherwise silently gate
    /// a reserved bit instead of the intended peripheral — see
    /// `every_roster_clock_is_a_real_gateable_bit`.
    #[must_use]
    pub const fn is_gateable(self) -> bool {
        self.mask() & self.bus.gateable_mask() != 0
    }
}

/// How much of the system is awake. See the module doc's table — this is about
/// *which peripherals exist*, orthogonal to
/// [`PowerProfile`](super::PowerProfile)'s choice of *how fast the CPU runs*.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PowerState {
    /// Audio path live: decode → SAI ring → DAC, SD refills, UI updating.
    Playback,
    /// Awake but silent: UI and control buses up, audio path gated.
    Idle,
    /// The configuration the firmware enters Stop mode from: wake sources only.
    Paused,
}

/// Number of [`PowerState`] variants — the length of [`PowerState::ALL`].
pub const POWER_STATE_COUNT: usize = 3;

impl PowerState {
    /// Every state, most awake first — which is also strictly-decreasing
    /// enabled-set order (see `each_lower_power_state_gates_strictly_more`).
    pub const ALL: [Self; POWER_STATE_COUNT] = [Self::Playback, Self::Idle, Self::Paused];

    /// This state as a one-bit flag, for [`Peripheral::states`]' bitset.
    #[must_use]
    pub const fn flag(self) -> u8 {
        match self {
            Self::Playback => 1 << 0,
            Self::Idle => 1 << 1,
            Self::Paused => 1 << 2,
        }
    }

    /// The state's name, for assertion messages and `defmt` logging.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Playback => "Playback",
            Self::Idle => "Idle",
            Self::Paused => "Paused",
        }
    }
}

/// Enabled in every state — for a peripheral the core cannot run without.
pub const ALWAYS: u8 =
    PowerState::Playback.flag() | PowerState::Idle.flag() | PowerState::Paused.flag();
/// Enabled whenever the device is awake, gated entering Stop.
pub const AWAKE: u8 = PowerState::Playback.flag() | PowerState::Idle.flag();
/// Enabled only while audio is actually flowing.
pub const PLAYING: u8 = PowerState::Playback.flag();

/// One clock the firmware deliberately turns **on**, and the states it's on in.
/// Everything not listed in [`ROSTER`] is gated off in every state — see the
/// module doc on why the gated set is derived rather than declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Peripheral {
    /// The peripheral's register name (e.g. `"SAI1"`), for messages.
    pub name: &'static str,
    /// Its RCC enable coordinate.
    pub clock: Clock,
    /// Bitset of [`PowerState::flag`]s this clock is enabled in — one of
    /// [`ALWAYS`] / [`AWAKE`] / [`PLAYING`], or an explicit combination.
    pub states: u8,
    /// Why the firmware needs it. Not decoration: this is the justification the
    /// audit is an audit *of*, and the thing a reviewer checks when a new entry
    /// turns a clock on.
    pub why: &'static str,
}

impl Peripheral {
    /// Whether this clock is enabled in `state`.
    #[must_use]
    pub const fn enabled_in(&self, state: PowerState) -> bool {
        self.states & state.flag() != 0
    }
}

/// Every clock the v2 DAP turns on, and when.
///
/// Grounded in the v2 hardware set (`docs/plans/2026-11-15-v2-migration-design.md`
/// §: CS43131 DAC on SAI, GDEY0397T81P/SSD1677 e-ink on SPI, MT6701 scroll-wheel,
/// BQ21080 charger + MAX17048 gauge on I²C, microSD on SDMMC1) and the memory map
/// (`dap/board-h7a3/memory.x`).
///
/// # SRAM1/SRAM2 are now LINKED but still gated — read this before placing a static
/// `memory.x` gained an `AHB_SRAM` region (128K at `0x3000_0000`, AHB-SRAM1 +
/// AHB-SRAM2, contiguous) and an `.ahbram` output section. Nothing is placed
/// there yet, so their clocks stay in [`MUST_STAY_GATED`] — an unused SRAM with
/// its clock off is exactly right.
///
/// The first static that lands in `.ahbram` must move `SRAM1`/`SRAM2` OUT of
/// [`MUST_STAY_GATED`] and into [`ROSTER`], because a read or write with the
/// clock gated bus-faults. Two further prerequisites, both verified the hard way
/// (see the placement doc S17):
///
/// * **Zero-init.** `cortex-m-rt`'s startup zeroes `.bss` in `RAM` only.
///   `.ahbram` is NOLOAD and never zeroed, so anything depending on a zeroed
///   start — a `StaticCell`'s `used` flag, an `AtomicBool`, an `Option`
///   discriminant — is unsound there until a `pre_init` clears
///   `__sahbram..__eahbram`.
/// * **`board-h7a3` forbids `unsafe_code`**, and `#[link_section]`/`#[used]` are
///   unsafe attributes, so that crate **cannot** place a static there as it
///   stands. Placement needs either a narrowly-scoped `expect(unsafe_code)` with
///   justification, or a small sibling crate owning the placement behind a safe
///   API (the shape `eink-ssd1677` already uses).
///
/// DTCM/ITCM remain genuinely unlinked, hence still gated.
///
/// `SRAM3` is listed in [`MUST_STAY_GATED`] for register completeness; the H7A3
/// has only two AHB SRAMs (the 1208K/"SRAM1-3" figure corrected in S1 came from
/// the H743's map), so that bit is reserved on this part and gating it is a
/// no-op rather than a decision.
///
/// # Two placeholders, both flagged
/// 1. **GPIO bank assignment.** No schematic exists yet, so which banks carry the
///    wheel/button/jack-detect inputs and the SAI/SPI/SDMMC alternate functions is
///    not yet a fact. The three entries below are a *proposal*; the gates are
///    written to verify the **policy** (unused banks gated; only wake-carrying
///    banks survive into `Paused`), so they stay meaningful after the schematic
///    renames the banks. Swapping a bank is a one-line change here.
/// 2. **Peripheral instance choice** (SPI4 for the display, I²C1 for control) —
///    likewise a proposal, pinned down by routing. Nothing in the policy or the
///    gates depends on *which* instance; they depend on *one* SPI and *one* I²C
///    being on, and on both being gated entering Stop.
pub const ROSTER: [Peripheral; 12] = [
    // --- always on -------------------------------------------------------
    Peripheral {
        name: "AXISRAM",
        clock: Clock {
            bus: Bus::Ahb3,
            bit: 31,
        },
        states: ALWAYS,
        why: "the RAM region holding every static the firmware owns (1 MB, D1, \
              DMA-capable) — gating it would unmap them. memory.x also links \
              AHB_SRAM now, but nothing is placed there yet, so SRAM1/2 stay \
              gated; see this module's doc before changing that",
    },
    Peripheral {
        name: "SYSCFG",
        clock: Clock {
            bus: Bus::Apb4,
            bit: 1,
        },
        states: ALWAYS,
        why: "EXTI line-to-port routing lives behind it; the wheel/button wake \
              path is configured through it, and it is cheap enough that keeping \
              it on avoids a reconfigure-on-every-wake",
    },
    Peripheral {
        name: "RTCAPB",
        clock: Clock {
            bus: Bus::Apb4,
            bit: 16,
        },
        states: ALWAYS,
        why: "LSE-backed RTC: wall-clock time across Stop, and the periodic wake \
              source that does not depend on user input",
    },
    Peripheral {
        name: "GPIOA",
        clock: Clock {
            bus: Bus::Ahb4,
            bit: 0,
        },
        states: ALWAYS,
        why: "wake-carrying bank (proposal — see ROSTER's placeholder note): \
              scroll-wheel press + jack detect EXTI, which must stay live in Stop",
    },
    // --- awake (gated entering Stop) -------------------------------------
    Peripheral {
        name: "GPIOB",
        clock: Clock {
            bus: Bus::Ahb4,
            bit: 1,
        },
        states: AWAKE,
        why: "alternate-function bank for the display SPI + control I²C \
              (proposal); no wake line, so it gates entering Stop",
    },
    Peripheral {
        name: "SPI4",
        clock: Clock {
            bus: Bus::Apb2,
            bit: 13,
        },
        states: AWAKE,
        why: "SSD1677 e-ink panel (GDEY0397T81P). Needed in Playback too — the \
              now-playing progress redraw — but the panel is bistable, so nothing \
              is driven while paused",
    },
    Peripheral {
        name: "DMA2",
        clock: Clock {
            bus: Bus::Ahb1,
            bit: 1,
        },
        states: AWAKE,
        why: "display SPI transfer: a full 800x480 Gray4 frame is far too large to \
              clock out byte-by-byte from the CPU",
    },
    Peripheral {
        name: "I2C1",
        clock: Clock {
            bus: Bus::Apb1L,
            bit: 21,
        },
        states: AWAKE,
        why: "the control bus: CS43131 (volume/filter), BQ21080 charger, MAX17048 \
              fuel gauge, MT6701 wheel. Polled on wake, not during Stop",
    },
    // --- playback only ---------------------------------------------------
    Peripheral {
        name: "GPIOC",
        clock: Clock {
            bus: Bus::Ahb4,
            bit: 2,
        },
        states: PLAYING,
        why: "alternate-function bank for the SAI (I2S) + SDMMC pins (proposal) — \
              gated whenever audio is not flowing",
    },
    Peripheral {
        name: "SAI1",
        clock: Clock {
            bus: Bus::Apb2,
            bit: 22,
        },
        states: PLAYING,
        why: "I2S to the CS43131, clocked at the source's native rate off the dual \
              MCLK oscillator (see model's native_rate_clocking test)",
    },
    Peripheral {
        name: "DMA1",
        clock: Clock {
            bus: Bus::Ahb1,
            bit: 0,
        },
        states: PLAYING,
        why: "the SAI1 TX circular ring — the whole reason the CPU can sleep \
              between decode bursts (model::wakeups_per_sec)",
    },
    Peripheral {
        name: "SDMMC1",
        clock: Clock {
            bus: Bus::Ahb3,
            bit: 16,
        },
        states: PLAYING,
        why: "microSD, 4-bit. No DMA1/2 channel is listed for it on purpose: SDMMC \
              has its own internal IDMA",
    },
];

/// # Known gap: Bluetooth output (v2)
/// v2 adds Bluetooth as a **secondary** output path — an I²S-input A2DP source
/// module sharing the SAI (or, preferably, on a dedicated SAI; see
/// `docs/plans/2026-11-21-memory-architecture-and-placement.md`). Two things here
/// will need to change, and neither is a simple addition:
///
/// 1. **A fourth state, as a *sibling*.** `PlaybackJack` and `PlaybackBt` are
///    mutually exclusive, not nested: the DAC's control I²C is clocked in one,
///    the BT module's control bus in the other, and neither set contains the
///    other. `each_lower_power_state_gates_strictly_more` asserts a strict chain,
///    so it must become a chain along `Paused ⊂ Idle ⊂ {either Playback}` with
///    the two Playback states as leaves. Forcing one to "nest" inside the other
///    to keep the current gate green would be fiction.
/// 2. **`every_billed_output_path_device_has_its_bus_clock_enabled_in_playback`
///    stops holding as written.** With two output paths the DAC is no longer
///    clocked in *every* Playback state, so the gate becomes "every device the
///    model bills for **in state X** is clocked in state X".
///
/// Nothing is added speculatively here: the module isn't chosen, so its control
/// bus (a UART or I²C) has no `(RCC_*ENR, bit)` coordinate yet, and inventing one
/// would defeat the point of requiring a real coordinate.
///
/// The clocks that must be gated **in every state**, named explicitly rather
/// than merely omitted from [`ROSTER`].
///
/// Omission already gates them (see the module doc), so this list adds nothing
/// to the *policy* — it adds a **tripwire**. Each entry is hardware a reasonable
/// contributor might switch on without realizing it contradicts a design
/// decision: USB and Bluetooth are explicitly deferred past v2
/// (`2026-11-15-v2-migration-design.md`), and the external FMC SDRAM / OCTOSPI
/// NOR of the v1 design are gone — `memory.x` links internal AXI SRAM only.
/// Enabling one of these should mean revisiting that decision, so it has to be
/// a deliberate edit here, not a quiet addition to `ROSTER`.
pub const MUST_STAY_GATED: [(&str, Clock); 11] = [
    (
        "USB_OTG_HS",
        Clock {
            bus: Bus::Ahb1,
            bit: 25,
        },
    ),
    (
        "USB_OTG_FS",
        Clock {
            bus: Bus::Ahb1,
            bit: 27,
        },
    ),
    (
        "FMC",
        Clock {
            bus: Bus::Ahb3,
            bit: 12,
        },
    ),
    (
        "OCTOSPI1",
        Clock {
            bus: Bus::Ahb3,
            bit: 14,
        },
    ),
    (
        "OCTOSPI2",
        Clock {
            bus: Bus::Ahb3,
            bit: 19,
        },
    ),
    (
        "SRAM1",
        Clock {
            bus: Bus::Ahb2,
            bit: 29,
        },
    ),
    (
        "SRAM2",
        Clock {
            bus: Bus::Ahb2,
            bit: 30,
        },
    ),
    (
        "SRAM3",
        Clock {
            bus: Bus::Ahb2,
            bit: 31,
        },
    ),
    (
        "DTCM1",
        Clock {
            bus: Bus::Ahb3,
            bit: 28,
        },
    ),
    (
        "DTCM2",
        Clock {
            bus: Bus::Ahb3,
            bit: 29,
        },
    ),
    (
        "ITCM",
        Clock {
            bus: Bus::Ahb3,
            bit: 30,
        },
    ),
];

/// The value the board writes to `bus`'s enable register in `state` — the union
/// of every [`ROSTER`] entry enabled there.
#[must_use]
pub fn enabled_mask(state: PowerState, bus: Bus) -> u32 {
    let mut mask = 0;
    let mut i = 0;
    while i < ROSTER.len() {
        let p = &ROSTER[i];
        if p.clock.bus as u8 == bus as u8 && p.enabled_in(state) {
            mask |= p.clock.mask();
        }
        i += 1;
    }
    mask
}

/// The bits the board must leave **clear** in `bus`'s enable register in
/// `state`: every gateable bit this state doesn't need.
///
/// Derived, never declared — `gateable & !enabled`. That is the completeness
/// property the audit rests on (module doc): a peripheral that nobody thought
/// about lands here automatically.
#[must_use]
pub fn gated_mask(state: PowerState, bus: Bus) -> u32 {
    bus.gateable_mask() & !enabled_mask(state, bus)
}

/// How many peripheral clocks are enabled in `state`, across all buses — the
/// one-number summary of "how much of the chip is awake".
#[must_use]
pub fn enabled_count(state: PowerState) -> u32 {
    let mut n = 0;
    for bus in Bus::ALL {
        n += enabled_mask(state, bus).count_ones();
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::power::model::PERIPHERAL_PLAYBACK_MA;

    // --- the roster is well-formed --------------------------------------

    /// Every roster coordinate names a bit that really gates a peripheral on
    /// this part. Without this, a typo'd bit number would set a reserved bit —
    /// leaving the intended peripheral unclocked, which presents as a dead
    /// peripheral on silicon rather than as a compile error.
    #[test]
    fn every_roster_clock_is_a_real_gateable_bit() {
        for p in ROSTER {
            assert!(
                p.clock.is_gateable(),
                "{}: {} bit {} is not a gateable bit (mask {:#010X})",
                p.name,
                p.clock.bus.name(),
                p.clock.bit,
                p.clock.bus.gateable_mask()
            );
            assert!(
                p.clock.bit < 32,
                "{}: bit {} out of range",
                p.name,
                p.clock.bit
            );
            assert!(
                p.states != 0,
                "{}: enabled in no state — then why list it?",
                p.name
            );
            assert!(
                !p.why.is_empty(),
                "{}: every enabled clock needs a reason",
                p.name
            );
        }
        for (name, clock) in MUST_STAY_GATED {
            assert!(
                clock.is_gateable(),
                "{name}: {} bit {} is not a gateable bit — the tripwire would never fire",
                clock.bus.name(),
                clock.bit
            );
        }
    }

    /// No two roster entries claim the same enable bit — two names for one clock
    /// would make the per-state policy ambiguous.
    #[test]
    fn roster_has_no_duplicate_clocks() {
        for (i, a) in ROSTER.iter().enumerate() {
            for b in ROSTER.iter().skip(i + 1) {
                assert!(
                    !(a.clock == b.clock),
                    "{} and {} both claim {} bit {}",
                    a.name,
                    b.name,
                    a.clock.bus.name(),
                    a.clock.bit
                );
            }
        }
    }

    // --- the audit: completeness ----------------------------------------

    /// **The audit.** For every state and every bus, the enabled and gated masks
    /// partition the bus's gateable surface exactly: no overlap, no gap. This is
    /// what "every unused peripheral is explicitly gated" means operationally —
    /// and it holds by construction, so it cannot rot.
    #[test]
    fn enabled_and_gated_partition_the_whole_gateable_surface() {
        for state in PowerState::ALL {
            for bus in Bus::ALL {
                let enabled = enabled_mask(state, bus);
                let gated = gated_mask(state, bus);
                let gateable = bus.gateable_mask();
                assert_eq!(
                    enabled & gated,
                    0,
                    "{}/{}: a bit is both enabled and gated",
                    state.name(),
                    bus.name()
                );
                assert_eq!(
                    enabled | gated,
                    gateable,
                    "{}/{}: enabled|gated ({:#010X}) != gateable ({gateable:#010X}) — \
                     a peripheral is unaccounted for",
                    state.name(),
                    bus.name(),
                    enabled | gated
                );
                assert_eq!(
                    enabled & !gateable,
                    0,
                    "{}/{}: the policy would set a reserved bit",
                    state.name(),
                    bus.name()
                );
            }
        }
    }

    /// The policy must actually gate things. A roster that enabled everything
    /// would satisfy the partition above trivially; this pins that the
    /// overwhelming majority of the chip is off in every state — and that the
    /// enabled count is small enough to be reviewable at a glance.
    #[test]
    fn the_policy_really_gates_most_of_the_chip() {
        let gateable: u32 = Bus::ALL
            .iter()
            .map(|b| b.gateable_mask().count_ones())
            .sum();
        assert!(
            gateable > 100,
            "expected ~120 gateable clocks, found {gateable}"
        );
        for state in PowerState::ALL {
            let on = enabled_count(state);
            assert!(on > 0, "{}: nothing enabled at all", state.name());
            assert!(
                on * 5 < gateable,
                "{}: {on} of {gateable} clocks enabled — a gating policy should \
                 leave well under a fifth of the chip running",
                state.name()
            );
        }
    }

    // --- the audit: per-state policy ------------------------------------

    /// Each lower-power state gates **strictly more** than the one above it:
    /// `Paused ⊂ Idle ⊂ Playback`, proper subsets. Without strictness a state
    /// could be a relabeling of its neighbour and the whole ladder would be
    /// decoration.
    #[test]
    fn each_lower_power_state_gates_strictly_more() {
        for (outer, inner) in [
            (PowerState::Playback, PowerState::Idle),
            (PowerState::Idle, PowerState::Paused),
        ] {
            let mut strictly_smaller = false;
            for bus in Bus::ALL {
                let o = enabled_mask(outer, bus);
                let i = enabled_mask(inner, bus);
                assert_eq!(
                    i & !o,
                    0,
                    "{}/{}: {} enables {:#010X} that {} does not — states must nest",
                    outer.name(),
                    bus.name(),
                    inner.name(),
                    i & !o,
                    outer.name()
                );
                if o != i {
                    strictly_smaller = true;
                }
            }
            assert!(
                strictly_smaller,
                "{} enables exactly what {} does — the states are indistinguishable",
                inner.name(),
                outer.name()
            );
            assert!(enabled_count(inner) < enabled_count(outer));
        }
    }

    /// The audio path exists **only** in `Playback`. This is the single biggest
    /// gating win available (SAI + its DMA + SDMMC + the AF bank), and the one
    /// most likely to be left on by a driver that forgets to tear down.
    #[test]
    fn the_audio_path_is_enabled_only_during_playback() {
        for name in ["SAI1", "DMA1", "SDMMC1"] {
            let p = ROSTER
                .iter()
                .find(|p| p.name == name)
                .unwrap_or_else(|| panic!("{name} missing from the roster"));
            assert!(
                p.enabled_in(PowerState::Playback),
                "{name} must be on to play"
            );
            assert!(
                !p.enabled_in(PowerState::Idle) && !p.enabled_in(PowerState::Paused),
                "{name} must be gated when audio is not flowing"
            );
        }
    }

    /// Nothing survives into `Paused` (the pre-Stop configuration) except clocks
    /// a wake path or the core itself needs. Each one is listed explicitly here,
    /// so adding a clock to the Stop configuration — the state whose current
    /// budget is 32 µA — cannot happen as a side effect of an edit elsewhere.
    #[test]
    fn paused_keeps_only_wake_sources_and_the_linked_sram() {
        const EXPECTED: [&str; 4] = ["AXISRAM", "SYSCFG", "RTCAPB", "GPIOA"];
        let mut found = 0;
        for p in ROSTER {
            if p.enabled_in(PowerState::Paused) {
                assert!(
                    EXPECTED.contains(&p.name),
                    "{} is enabled in Paused but is not a wake source — Stop mode's \
                     32 uA budget assumes only {EXPECTED:?}",
                    p.name
                );
                found += 1;
            }
        }
        assert_eq!(
            found,
            EXPECTED.len(),
            "expected exactly {} clocks in Paused",
            EXPECTED.len()
        );
    }

    /// The deferred/removed hardware tripwire: every [`MUST_STAY_GATED`] clock is
    /// gated in every state. USB and Bluetooth are deferred past v2; the external
    /// FMC SDRAM and OCTOSPI NOR of the v1 design are gone; `memory.x` links AXI
    /// SRAM only, so SRAM1/2/3 and DTCM/ITCM stay unclocked. Enabling any of them
    /// should be a deliberate design change, not a quiet roster addition.
    #[test]
    fn deferred_and_removed_hardware_stays_gated_in_every_state() {
        for (name, clock) in MUST_STAY_GATED {
            for state in PowerState::ALL {
                assert_eq!(
                    enabled_mask(state, clock.bus) & clock.mask(),
                    0,
                    "{name} is enabled in {} — it is deferred or removed hardware; \
                     if that changed, update MUST_STAY_GATED and say why",
                    state.name()
                );
                assert_ne!(
                    gated_mask(state, clock.bus) & clock.mask(),
                    0,
                    "{name} must appear in {}'s gated mask",
                    state.name()
                );
            }
        }
    }

    /// Only banks carrying a wake line survive into `Paused`. Pinned separately
    /// from the roster's own `states` field so the *rule* is visible: a GPIO bank
    /// kept alive in Stop has to justify itself with a wake input, and the
    /// schematic reshuffling which bank that is cannot quietly keep them all on.
    #[test]
    fn only_wake_carrying_gpio_banks_survive_into_stop() {
        let banks: heapless::Vec<&Peripheral, 11> = ROSTER
            .iter()
            .filter(|p| p.name.starts_with("GPIO"))
            .collect();
        assert!(
            banks.len() >= 2,
            "expected several GPIO banks in the roster"
        );
        let in_stop = banks
            .iter()
            .filter(|p| p.enabled_in(PowerState::Paused))
            .count();
        assert_eq!(
            in_stop, 1,
            "exactly one GPIO bank should carry the wake inputs; {in_stop} are on in Paused"
        );
        for p in banks {
            if p.enabled_in(PowerState::Paused) {
                assert!(
                    p.why.contains("wake"),
                    "{} is on in Paused — its `why` must name the wake line that \
                     justifies it",
                    p.name
                );
            }
        }
    }

    // --- seam to the battery model --------------------------------------

    /// The seam between this policy and `power::model`'s current accounting:
    /// every external device the model bills for during playback
    /// (`PERIPHERAL_PLAYBACK_MA` = DAC/amp + SD + display) must have the bus
    /// peripheral that talks to it enabled in `Playback`. Otherwise the model is
    /// charging the battery for a device the gating policy has switched off —
    /// the two modules would be describing different machines.
    #[test]
    fn every_billed_output_path_device_has_its_bus_clock_enabled_in_playback() {
        // (what the model bills for, the clocks that device needs)
        const BILLED: [(&str, &[&str]); 3] = [
            ("DAC_AMP_PLAYBACK_MA (CS43131)", &["SAI1", "I2C1"]),
            ("SD_AVG_MA (microSD)", &["SDMMC1"]),
            ("DISPLAY_AVG_MA (SSD1677)", &["SPI4", "DMA2"]),
        ];
        for (device, clocks) in BILLED {
            for name in clocks {
                let p = ROSTER
                    .iter()
                    .find(|p| &p.name == name)
                    .unwrap_or_else(|| panic!("{device}: {name} missing from the roster"));
                assert!(
                    p.enabled_in(PowerState::Playback),
                    "{device}: the model bills for this device but {name} is gated \
                     during Playback"
                );
            }
        }
        // Sanity: the model really does bill a non-trivial output path (if this
        // ever went to zero the loop above would be vacuous).
        const { assert!(PERIPHERAL_PLAYBACK_MA > 1.0) };
    }

    /// `Paused` is the state `model::STOP_MODE_CURRENT_MA` (32 uA) describes, so
    /// nothing in it may be a device the playback current model bills for — if an
    /// output-path peripheral were still clocked entering Stop, the paused draw
    /// would be milliamps, not microamps, and the "~4 years paused" figure would
    /// be wrong by orders of magnitude.
    #[test]
    fn paused_enables_no_output_path_peripheral() {
        for name in ["SAI1", "SDMMC1", "SPI4", "I2C1", "DMA1", "DMA2"] {
            let p = ROSTER.iter().find(|p| p.name == name).expect("in roster");
            assert!(
                !p.enabled_in(PowerState::Paused),
                "{name} is clocked in Paused — Stop mode's 32 uA budget cannot \
                 absorb an output-path peripheral"
            );
        }
    }
}
