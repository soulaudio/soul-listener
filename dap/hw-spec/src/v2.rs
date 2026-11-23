//! **V2** — the firmware's target board. Soft-locked.
//!
//! STM32H7A3 + CS43131 (integrated amp, I2S slave) + external dual MCLK
//! oscillators + no external RAM. Per
//! `docs/plans/2026-11-15-v2-migration-design.md`; `hardware/elec/src` is still
//! [`crate::v1`] and has not migrated.

use crate::select::Constraint;
use crate::{
    BoardSpec, Candidate, Dac, DacRole, LockStatus, Mcu, MemoryRegion, Oscillator, PartReq,
    PowerBudget, Preference, Provenance, RateEntry, Revision, VolumeMechanism,
};

/// No machine-checkable constraints yet — the requirement is prose-only.
const NO_CONSTRAINTS: &[Constraint<'static>] = &[];

/// Soft lock. Everything below may still change; this says what has to happen
/// before it should not.
const LOCK: LockStatus = LockStatus::Soft {
    until: "the v2 board is routed and a prototype measured: the oscillator pair is still a \
            proposal (see OSCILLATORS), SLEEP_CURRENT_MA is an estimate, and CPI/cache behaviour \
            is unmeasured (placement doc S15/S16)",
};

const MCU: Mcu = Mcu {
    mpn: "STM32H7A3ZIT6",
    // Pinned so board-h7a3 cannot select a chip the spec does not describe.
    embassy_feature: "stm32h7a3zi",
    max_core_mhz: 280,
    provenance: Provenance::Datasheet("ST DS13677"),
};

/// The memory map, as `memory.x` must declare it.
///
/// AXI is one contiguous 1024 KB region even though the die splits it 256+384+384;
/// AHB-SRAM1/2 are address-contiguous (0x3000_0000 + 0x2_0000) so they are one
/// 128 KB region, which is what lets a 96 KB framebuffer span them.
///
/// SRD-SRAM is deliberately absent: DMA-reachable but in a different power domain
/// with its own retention behaviour, so counting it as general-purpose RAM is how
/// a buffer ends up un-retained across a pause.
const MEMORY: &[MemoryRegion] = &[
    MemoryRegion {
        name: "RAM",
        origin: 0x2400_0000,
        bytes: 1024 * 1024,
        dma_capable: true,
        linked: true,
        note: "AXI-SRAM, CD domain. Holds every static the firmware owns.",
    },
    MemoryRegion {
        name: "AHB_SRAM",
        origin: 0x3000_0000,
        bytes: 128 * 1024,
        dma_capable: true,
        linked: true,
        note: "AHB-SRAM1+2, contiguous. Linked 2026-11-23; nothing placed there yet — \
               RCC SRAM1/2EN are still gated and `.ahbram` is not zeroed.",
    },
    MemoryRegion {
        name: "SRD_SRAM",
        origin: 0x3804_0000,
        bytes: 32 * 1024,
        dma_capable: true,
        linked: false,
        note: "Separate power domain; excluded from the general pool on purpose.",
    },
    MemoryRegion {
        name: "DTCM",
        origin: 0x2000_0000,
        bytes: 128 * 1024,
        dma_capable: false,
        linked: false,
        note: "CPU-only, so never a DMA buffer. Too small for the 352 KiB stack reserve.",
    },
];

const DAC: Dac = Dac {
    mpn: "CS43131",
    // Slave is what allows native-rate playback: the SAI generates SCLK/LRCK
    // from the external oscillator, so changing rate changes the clock tree, not
    // a resampler.
    role: DacRole::I2sSlave,
    // Per the DAC-selection research: MCLK/LRCK of 512, 256 or 384.
    //
    // NOTE, and it is the reason `rate_matrix_is_reachable` exists: 128 is NOT
    // in this list, and 128fs is what a 22.5792 MHz oscillator yields at
    // 176.4 kHz. Other DACs do accept 128fs (ADAU1966, WM8742), so if a
    // datasheet read shows the CS43131 does too, add it here and the oscillator
    // pair below can drop back to 22.5792/24.576.
    accepted_mclk_ratios: &[512, 384, 256],
    volume: VolumeMechanism::DacDigital {
        step_db_centi: 50,
        min_db: 127,
        internal_bits: 32,
    },
    // ~23 mW of playback power. At the battery rail that is ~6 mA at moderate
    // listening and up to ~11 mA near full 32 ohm output; the model takes 7 mA as
    // the realistic average. NOT 12.8 mA — that is the same power referred to the
    // chip's 1.8 V core, which is the error the bridge gate caught.
    playback_ma_at_battery: 7.0,
    integrated_amp: true,
    provenance: Provenance::Datasheet("Cirrus CS43131 DS1155F2"),
};

/// The oscillator pair — **a proposal, and the open hardware decision.**
///
/// 45.1584 MHz = 1024 x 44.1k and 49.152 MHz = 1024 x 48k. Starting an octave
/// above the usual 22.5792/24.576 pair is what makes the whole rate matrix
/// reachable: the SAI can only *divide* its clock input, so a 22.5792 MHz
/// oscillator caps MCLK at 512fs and 176.4 kHz falls off the bottom at 128fs —
/// a ratio the CS43131 does not list. Doubling the oscillator gives MCKDIV room
/// in both directions while staying integer-divided off a fixed oscillator, so
/// the no-PLL jitter argument is untouched.
///
/// Verify before a hard lock: that the H7A3's SAI clock input tolerates
/// 49.152 MHz, and that the CS43131 really excludes 128fs.
const OSCILLATORS: &[Oscillator] = &[
    Oscillator {
        hz: 45_158_400,
        family_hz: 44_100,
        note: "1024 x 44.1k — serves 44.1/88.2/176.4 kHz via MCKDIV",
    },
    Oscillator {
        hz: 49_152_000,
        family_hz: 48_000,
        note: "1024 x 48k — serves 48/96/192 kHz via MCKDIV",
    },
];

/// Every rate the product claims, with the exact clock tree that reaches it.
///
/// `osc_hz / mckdiv == ratio * hz` must hold exactly, and `ratio` must be one of
/// [`DAC`]'s accepted ratios. Both are gated.
const RATES: &[RateEntry] = &[
    RateEntry {
        hz: 44_100,
        osc_hz: 45_158_400,
        mckdiv: 2,
        ratio: 512,
    },
    RateEntry {
        hz: 88_200,
        osc_hz: 45_158_400,
        mckdiv: 1,
        ratio: 512,
    },
    RateEntry {
        hz: 176_400,
        osc_hz: 45_158_400,
        mckdiv: 1,
        ratio: 256,
    },
    RateEntry {
        hz: 48_000,
        osc_hz: 49_152_000,
        mckdiv: 2,
        ratio: 512,
    },
    RateEntry {
        hz: 96_000,
        osc_hz: 49_152_000,
        mckdiv: 1,
        ratio: 512,
    },
    RateEntry {
        hz: 192_000,
        osc_hz: 49_152_000,
        mckdiv: 1,
        ratio: 256,
    },
];

const POWER: PowerBudget = PowerBudget {
    battery_mah: 1100,
    target_hours: 20.0,
    // DAC+amp 7.0 + microSD 2.0 + e-ink 2.0, all battery-referred, matching
    // `power::model::PERIPHERAL_PLAYBACK_MA`. The display figure is the one that
    // was 0.5 mA against code drawing ~34 mA until the refresh policy landed,
    // which is why these are gated rather than asserted in prose.
    peripheral_playback_ma: 11.0,
    provenance: Provenance::Datasheet("Cirrus CS43131 DS1155F2 + ST DS13677 + GDEM0397T81P"),
};

/// Packages a hand-assembled EU small-batch build can place and inspect.
///
/// Not an aesthetic preference: a BGA or a 1.6x1.2 mm package cannot be hand-
/// soldered or inspected without equipment this build does not have, so it is a
/// hard constraint rather than a nice-to-have.
const HAND_PLACEABLE_OSC_PACKAGES: &[&str] =
    &["SMD-3225-4", "SMD-2520-4", "SMD-5032-4", "SMD-7050-4"];

/// The 44.1 kHz-family MCLK oscillator, machine-checkable.
const OSC_44K1_CONSTRAINTS: &[Constraint<'static>] = &[
    Constraint {
        attr: "frequency_hz",
        pred: crate::select::Predicate::NumEq {
            value: 45_158_400.0,
            rel_tol: 1e-6,
        },
        why: "1024 x 44100. The SAI can only DIVIDE its clock input, so a lower oscillator caps \
              MCLK at 512fs and 176.4 kHz falls off the bottom at an unaccepted 128fs — which is \
              the whole reason this is 45.1584 and not 22.5792 MHz",
    },
    Constraint {
        attr: "supply_mv_max",
        pred: crate::select::Predicate::AtMost(3300.0),
        why: "the board has no rail above 3.3 V",
    },
    Constraint {
        attr: "package",
        pred: crate::select::Predicate::OneOf(HAND_PLACEABLE_OSC_PACKAGES),
        why: "hand-assembled small batch: the package must be placeable and inspectable without \
              equipment this build does not have",
    },
    Constraint {
        attr: "phase_noise_dbc_1khz",
        pred: crate::select::Predicate::AtMost(-130.0),
        why: "jitter in the MCLK path is the single thing the dual-oscillator design exists to \
              avoid; an oscillator with no published phase-noise figure is rejected rather than \
              assumed adequate",
    },
];

/// The 48 kHz-family MCLK oscillator. Same constraints, different frequency.
const OSC_48K_CONSTRAINTS: &[Constraint<'static>] = &[
    Constraint {
        attr: "frequency_hz",
        pred: crate::select::Predicate::NumEq {
            value: 49_152_000.0,
            rel_tol: 1e-6,
        },
        why: "1024 x 48000, for the same MCKDIV-headroom reason as the 44.1 kHz family",
    },
    Constraint {
        attr: "supply_mv_max",
        pred: crate::select::Predicate::AtMost(3300.0),
        why: "the board has no rail above 3.3 V",
    },
    Constraint {
        attr: "package",
        pred: crate::select::Predicate::OneOf(HAND_PLACEABLE_OSC_PACKAGES),
        why: "hand-assembled small batch",
    },
    Constraint {
        attr: "phase_noise_dbc_1khz",
        pred: crate::select::Predicate::AtMost(-130.0),
        why: "jitter in the MCLK path",
    },
];

/// The oscillator mux.
const MUX_CONSTRAINTS: &[Constraint<'static>] = &[
    Constraint {
        attr: "channels",
        pred: crate::select::Predicate::NumEq {
            value: 2.0,
            rel_tol: 0.0,
        },
        why: "two oscillators, one SAI clock input",
    },
    Constraint {
        attr: "break_before_make",
        pred: crate::select::Predicate::IsTrue,
        why: "without it the two oscillators are briefly shorted together during a switch. This \
              is why a PLL-based clock multiplier (NB3N511) is the wrong class of part rather \
              than merely a worse one",
    },
    Constraint {
        attr: "max_freq_hz",
        pred: crate::select::Predicate::AtLeast(49_152_000.0),
        why: "it has to pass the faster of the two oscillators",
    },
    Constraint {
        attr: "supply_mv_max",
        pred: crate::select::Predicate::AtMost(3300.0),
        why: "the board has no rail above 3.3 V",
    },
];

/// Requirements with candidates — the soft lock in practice.
///
/// Stock figures are from the committed sourcing snapshot
/// (`hardware/bom/snapshot.json`), not from here; these notes record *why* a
/// candidate is where it is.
const PARTS: &[PartReq] = &[
    PartReq {
        id: "dac",
        what: "stereo DAC + headphone amp, I2S slave, both MCLK families, >=120 dB(A) DR",
        constraints: NO_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "CS43131-CNZ",
                preference: Preference::Preferred,
                note: "the -CNZ suffix is deliberate, and the reason is DEPTH rather than \
                       availability: -CNZ is stocked at both Mouser EU and Digi-Key EU, while \
                       -CNZR (tape and reel, same die) is stocked only at Digi-Key. Either can \
                       be built; only one has a second source",
            },
            Candidate {
                mpn: "CS43198-CNZ",
                preference: Preference::Alternate,
                note: "same family, no integrated amp — would need a separate amp stage, so \
                       this is a fallback with a board change, not a drop-in",
            },
            Candidate {
                mpn: "ES9038Q2M",
                preference: Preference::Rejected(
                    "v1 part: no integrated amp (needs TPA6120A2), far higher playback current, \
                     and SAI-master clocking — the whole reason v2 exists",
                ),
                note: "kept so the choice is not re-litigated from scratch",
            },
        ],
    },
    PartReq {
        id: "mclk-osc-44k1",
        what: "45.1584 MHz fixed oscillator, <=3.3 V, low phase noise, SMD",
        constraints: OSC_44K1_CONSTRAINTS,
        candidates: &[Candidate {
            mpn: "ASDMB-45.1584MHZ-LY-T",
            preference: Preference::Evaluating,
            note: "placeholder: the frequency is the requirement, the part is not chosen. \
                       Needs a phase-noise figure and an EU stock check.",
        }],
    },
    PartReq {
        id: "mclk-osc-48k",
        what: "49.152 MHz fixed oscillator, <=3.3 V, low phase noise, SMD",
        constraints: OSC_48K_CONSTRAINTS,
        candidates: &[Candidate {
            mpn: "ASDMB-49.152MHZ-LY-T",
            preference: Preference::Evaluating,
            note: "placeholder, as above",
        }],
    },
    PartReq {
        id: "mclk-mux",
        what: "2:1 clock mux, break-before-make, <=50 MHz, single digital select",
        constraints: MUX_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "74LVC1G3157GW",
                preference: Preference::Preferred,
                note: "passive analog mux, break-before-make guarantees no contention between \
                       the two oscillators during a switch; 6-7.5 ohm Ron",
            },
            Candidate {
                mpn: "SN74LVC1G3157DCKR",
                preference: Preference::Alternate,
                note: "TI equivalent of the same function",
            },
            Candidate {
                mpn: "NB3N511",
                preference: Preference::Rejected(
                    "wrong class of part: a PLL-based clock multiplier, which reintroduces the \
                     jitter the dual-oscillator design exists to avoid",
                ),
                note: "",
            },
        ],
    },
    PartReq {
        id: "mcu",
        what: "Cortex-M7 >=250 MHz, >=1 MB contiguous DMA-capable SRAM, SAI, SDMMC, 2 MB flash",
        constraints: NO_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "STM32H7A3ZIT6",
                preference: Preference::Preferred,
                note: "1.4 MB SRAM total, 280 MHz, no external RAM needed",
            },
            Candidate {
                mpn: "STM32H7B3ZIT6",
                preference: Preference::Alternate,
                note: "same family and map, higher flash — a pin-compatible fallback if H7A3 \
                       supply tightens",
            },
            Candidate {
                mpn: "STM32H743ZIT6",
                preference: Preference::Rejected(
                    "v1 part: needs external SDRAM for the same job, which costs board area, \
                     power and a refresh-contention problem in the audio path",
                ),
                note: "",
            },
        ],
    },
];

/// The V2 board spec.
pub const V2: BoardSpec = BoardSpec {
    revision: Revision::V2,
    lock: LOCK,
    mcu: MCU,
    memory: MEMORY,
    dac: DAC,
    oscillators: OSCILLATORS,
    rates: RATES,
    power: POWER,
    parts: PARTS,
};
