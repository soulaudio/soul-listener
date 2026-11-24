//! **V2** — the firmware's target board. Soft-locked.
//!
//! STM32H7A3 + CS43131 (integrated amp, I2S slave) + external dual MCLK
//! oscillators + no external RAM. Per
//! `docs/plans/2026-11-15-v2-migration-design.md`; `hardware/elec/src` is still
//! [`crate::v1`] and has not migrated.

use crate::select::{Constraint, Evidence};
use crate::{
    BoardSpec, Candidate, Dac, DacRole, LockStatus, MclkContract, Mcu, MemoryRegion, Oscillator,
    PartReq, PowerBudget, Preference, Provenance, RateEntry, Revision, VolumeMechanism,
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
    // This part does NOT constrain MCLK as a multiple of Fs. It wants a fixed
    // system clock of 22.5792 or 24.576 MHz and carries the sample rate in the
    // ASP_SPRATE register instead.
    //
    // DS1155F2 §4.6.1: "When the MCLK is supplied to the device through the
    // XTI/MCLK pin ... its frequency must be one of the nominal MCLK_INT
    // frequencies (22.5792 or 24.576 MHz), and its duty cycle must be between
    // 45% to 55%." Register 0x1000B adds that only 384 kHz in master mode pins
    // the choice: "For all other rates, MCLK_INT can be either 22.5792 MHz or
    // 24.576 MHz."
    //
    // An earlier draft of this spec recorded `accepted_mclk_ratios: [512, 384,
    // 256]` and concluded that 176.4 kHz "falls off the bottom at 128fs", which
    // drove a 1024xFs oscillator pair. That was a category error, not a bad
    // reading: ratios are how the ES9038Q2M (V1) works, not this part.
    mclk: MclkContract::FixedSystemClock(&[22_579_200, 24_576_000]),
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

/// The oscillator pair — the two nominal CS43131 system clocks.
///
/// 22.5792 MHz and 24.576 MHz, which are simultaneously:
///
/// - the only two frequencies the CS43131 accepts on XTI/MCLK, so each
///   oscillator can feed the DAC *directly* with no divider in the path; and
/// - exact integer sources for the SAI's bit clock across the whole rate matrix
///   (MCKDIV 8/4/2 for the 1x/2x/4x rates of each family).
///
/// **Why two, now that the DAC would accept either for every supported rate:**
/// the SAI, not the DAC. SCK must be exactly `64 x Fs` so that LRCK is exactly
/// Fs, and 24.576 MHz does not divide into the 44.1 kHz family by an integer
/// (24.576e6 / 2.8224e6 = 8.707...). One oscillator per family is what makes
/// native-rate playback bit-exact; a single oscillator would force either
/// resampling or a fractional PLL, which is the jitter the design exists to
/// avoid.
///
/// An earlier draft specified 45.1584/49.152 MHz (1024xFs) to clear a DAC ratio
/// limit that does not exist. The 512x pair is strictly better on every axis
/// that matters — lower power, far better distributor stock, relaxed layout —
/// and is what the datasheet actually asks for.
///
/// Still to verify before a hard lock: the H7A3's SAI clock-input requirements at
/// 24.576 MHz (headroom is ample, but it is unmeasured), and the phase-noise mask
/// in DS1155F2 Fig. 4-14, which applies to a directly-supplied MCLK and is what
/// the oscillator's phase-noise constraint should eventually be derived from
/// rather than the round -130 dBc/Hz placeholder used today.
const OSCILLATORS: &[Oscillator] = &[
    Oscillator {
        hz: 22_579_200,
        family_hz: 44_100,
        note: "512 x 44.1k; a nominal CS43131 MCLK_INT — serves 44.1/88.2/176.4 kHz",
    },
    Oscillator {
        hz: 24_576_000,
        family_hz: 48_000,
        note: "512 x 48k; a nominal CS43131 MCLK_INT — serves 48/96/192 kHz",
    },
];

/// Every rate the product claims, with the exact clock tree that reaches it.
///
/// `osc_hz / mckdiv == frame_bits * hz` must hold exactly, and each oscillator
/// must itself be a frequency the DAC accepts on MCLK (it is wired straight to
/// XTI). Both are gated.
///
/// 32-bit stereo frames throughout, so `frame_bits` is 64 and MCKDIV walks 8/4/2
/// as the rate doubles within a family. Every division is an integer power of
/// two — nothing here is approximate.
const RATES: &[RateEntry] = &[
    RateEntry {
        hz: 44_100,
        osc_hz: 22_579_200,
        mckdiv: 8,
        frame_bits: 64,
    },
    RateEntry {
        hz: 88_200,
        osc_hz: 22_579_200,
        mckdiv: 4,
        frame_bits: 64,
    },
    RateEntry {
        hz: 176_400,
        osc_hz: 22_579_200,
        mckdiv: 2,
        frame_bits: 64,
    },
    RateEntry {
        hz: 48_000,
        osc_hz: 24_576_000,
        mckdiv: 8,
        frame_bits: 64,
    },
    RateEntry {
        hz: 96_000,
        osc_hz: 24_576_000,
        mckdiv: 4,
        frame_bits: 64,
    },
    RateEntry {
        hz: 192_000,
        osc_hz: 24_576_000,
        mckdiv: 2,
        frame_bits: 64,
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

/// What TME calls an *active* clock source, in both spellings it uses.
///
/// The "Resonators and Generators" category mixes passive parts (`crystal`,
/// `ceramic`, `resonator` — a quartz element with no output driver) with active
/// ones (`oscillator`, `XO`). Only an active part can drive the CS43131's
/// XTI/MCLK pin *and* the SAI's clock input from one output, which is the whole
/// topology; a crystal would need an oscillator circuit built around it and could
/// not present a clock to two consumers.
///
/// Both spellings are listed because the vendor uses both for the same thing —
/// in one query, 9 parts labelled `oscillator` and 4 labelled `XO`.
const ACTIVE_CLOCK_TYPES: &[&str] = &["oscillator", "XO"];

/// The 44.1 kHz-family MCLK oscillator, machine-checkable.
const OSC_44K1_CONSTRAINTS: &[Constraint<'static>] = &[
    // Without this the filter happily returns passive crystals, which cannot
    // drive two consumers from one output.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "part_type",
        pred: crate::select::Predicate::OneOf(ACTIVE_CLOCK_TYPES),
        why: "an ACTIVE clock source. The vendor category mixes crystals, ceramics and \
              resonators (passive quartz, no output driver) with oscillators and XOs; only an \
              active part can feed the DAC's XTI pin and the SAI's clock input from one output",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "frequency_hz",
        pred: crate::select::Predicate::NumEq {
            value: 22_579_200.0,
            // 1e-4, not 1e-6. TME publishes this part's frequency as
            // "22.579MHz" — TRUNCATED to five significant figures — which is
            // 8.86e-6 away from the real 22.5792 MHz and would have been
            // rejected under the old tolerance. The doc on NumEq anticipated
            // representation noise (22579200 vs 22.5792e6) but not a vendor
            // publishing fewer digits than the part has.
            //
            // 1e-4 is still 200x tighter than the nearest frequency anyone would
            // confuse this with: 22.1184 MHz (the common UART crystal) is 2.0e-2
            // away, and the other MCLK family is 8.8e-2 away. So this buys
            // tolerance of vendor formatting without losing the ability to
            // reject a genuinely different part.
            rel_tol: 1e-4,
        },
        why: "22.5792 MHz is simultaneously a nominal CS43131 MCLK_INT (so it feeds XTI with no \
              divider) and 512 x 44100, which divides by an integer to an exact 64 x Fs bit clock \
              at 44.1, 88.2 and 176.4 kHz. 24.576 MHz satisfies the DAC equally well and cannot \
              produce this family exactly, which is why the frequency is pinned rather than left \
              as 'either nominal MCLK'",
    },
    // Range CONTAINMENT, not a ceiling — see MUX_CONSTRAINTS for the error this
    // replaces. An oscillator rated 1.8-3.6 V runs fine on a 3.3 V rail.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_min",
        pred: crate::select::Predicate::AtMost(3300.0),
        why: "must operate at the board's 3.3 V rail",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_max",
        pred: crate::select::Predicate::AtLeast(3300.0),
        why: "and must tolerate 3.3 V — a higher maximum rating is better, not worse",
    },
    // `mounting`, not a package-name list. TME publishes "Body dimensions"
    // ("3.2x2.5x0.8mm") and sometimes "Case", never the SMD-3225-4 style codes
    // the spec used to enumerate — so filtering on package NAMES rejected every
    // real part. Hand-assembly cares about through-hole vs surface mount and
    // about size; the first is published, so it is the one filtered on.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "mounting",
        pred: crate::select::Predicate::OneOf(&["SMD"]),
        why: "hand-assembled small batch, so surface mount rather than through-hole. The \
              previous constraint enumerated package codes (SMD-3225-4 and friends) that no \
              distributor publishes for this category, which rejected every real oscillator",
    },
    Constraint {
        // Marked Datasheet on EVIDENCE rather than assumption: a survey of all
        // 121 parts TME returns for these two frequencies found no phase-noise
        // or jitter parameter of any kind. Jitter lives in the datasheet. Left
        // as a catalogue constraint it rejects every real oscillator for a field
        // no distributor publishes — the trap break_before_make fell into.
        evidence: Evidence::Datasheet,
        attr: "phase_noise_dbc_1khz",
        pred: crate::select::Predicate::AtMost(-130.0),
        why: "jitter in the MCLK path is the single thing the dual-oscillator design exists to \
              avoid, so an oscillator whose phase noise is unknown is NOT adequate. Datasheet-\
              only, and on evidence: a survey of all 121 parts TME returns for the two MCLK \
              frequencies found no phase-noise or jitter parameter in any form. Confirm it per \
              candidate against the DS1155F2 Fig. 4-14 mask and record it in the candidate note",
    },
];

/// The 48 kHz-family MCLK oscillator. Same constraints, different frequency.
const OSC_48K_CONSTRAINTS: &[Constraint<'static>] = &[
    // Without this the filter happily returns passive crystals, which cannot
    // drive two consumers from one output.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "part_type",
        pred: crate::select::Predicate::OneOf(ACTIVE_CLOCK_TYPES),
        why: "an ACTIVE clock source. The vendor category mixes crystals, ceramics and \
              resonators (passive quartz, no output driver) with oscillators and XOs; only an \
              active part can feed the DAC's XTI pin and the SAI's clock input from one output",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "frequency_hz",
        pred: crate::select::Predicate::NumEq {
            value: 24_576_000.0,
            // 1e-4, not 1e-6. TME publishes this part's frequency as
            // "22.579MHz" — TRUNCATED to five significant figures — which is
            // 8.86e-6 away from the real 22.5792 MHz and would have been
            // rejected under the old tolerance. The doc on NumEq anticipated
            // representation noise (22579200 vs 22.5792e6) but not a vendor
            // publishing fewer digits than the part has.
            //
            // 1e-4 is still 200x tighter than the nearest frequency anyone would
            // confuse this with: 22.1184 MHz (the common UART crystal) is 2.0e-2
            // away, and the other MCLK family is 8.8e-2 away. So this buys
            // tolerance of vendor formatting without losing the ability to
            // reject a genuinely different part.
            rel_tol: 1e-4,
        },
        why: "the other nominal CS43131 MCLK_INT, and 512 x 48000 — exact for 48, 96 and 192 kHz",
    },
    // Range CONTAINMENT, not a ceiling — see MUX_CONSTRAINTS for the error this
    // replaces. An oscillator rated 1.8-3.6 V runs fine on a 3.3 V rail.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_min",
        pred: crate::select::Predicate::AtMost(3300.0),
        why: "must operate at the board's 3.3 V rail",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_max",
        pred: crate::select::Predicate::AtLeast(3300.0),
        why: "and must tolerate 3.3 V — a higher maximum rating is better, not worse",
    },
    // `mounting`, not a package-name list. TME publishes "Body dimensions"
    // ("3.2x2.5x0.8mm") and sometimes "Case", never the SMD-3225-4 style codes
    // the spec used to enumerate — so filtering on package NAMES rejected every
    // real part. Hand-assembly cares about through-hole vs surface mount and
    // about size; the first is published, so it is the one filtered on.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "mounting",
        pred: crate::select::Predicate::OneOf(&["SMD"]),
        why: "hand-assembled small batch, so surface mount rather than through-hole. The \
              previous constraint enumerated package codes (SMD-3225-4 and friends) that no \
              distributor publishes for this category, which rejected every real oscillator",
    },
    Constraint {
        // Marked Datasheet on EVIDENCE rather than assumption: a survey of all
        // 121 parts TME returns for these two frequencies found no phase-noise
        // or jitter parameter of any kind. Jitter lives in the datasheet. Left
        // as a catalogue constraint it rejects every real oscillator for a field
        // no distributor publishes — the trap break_before_make fell into.
        evidence: Evidence::Datasheet,
        attr: "phase_noise_dbc_1khz",
        pred: crate::select::Predicate::AtMost(-130.0),
        why: "jitter in the MCLK path, as for the 44.1 kHz family. Datasheet-only: no \
              distributor publishes phase noise for this category",
    },
];

/// The oscillator mux.
///
/// **Rewritten 2026-11-24 against real distributor data**, which rejected the
/// correct part three times over. The TI `SN74LVC1G3157DCKR` — 6916 in stock at
/// TME, and the part the ranking policy prefers — failed on supply voltage, on
/// channel count, and on two fields no catalogue publishes. Every change here is
/// traceable to a string in `hardware/bom/responses/tme/parameters-0.json`.
const MUX_CONSTRAINTS: &[Constraint<'static>] = &[
    // TME publishes "Output configuration" = SPDT. This is the unambiguous way to
    // say "2:1 mux": one single-pole double-throw switch selecting between two
    // inputs. It is what `channels` was trying and failing to express.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "switch_configuration",
        pred: crate::select::Predicate::OneOf(&["SPDT"]),
        why: "one single-pole double-throw switch IS a 2:1 mux: two oscillator inputs, one SAI \
              clock output. This is the term distributors publish, so it is checkable where a \
              raw count is not",
    },
    // `channels: 2` was WRONG. TME reports "Number of channels" = 1 for an SPDT
    // part, counting switch elements, while the spec was counting inputs. Same
    // part, incompatible conventions, and the constraint rejected the right one.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "channels",
        pred: crate::select::Predicate::NumEq {
            value: 1.0,
            rel_tol: 0.0,
        },
        why: "ONE switch element. Distributors count channels as switches rather than inputs, so \
              a 2:1 mux is 'channels = 1'. The spec previously demanded 2 here, meaning inputs, \
              and thereby rejected every correct part — which is why the convention is now \
              spelled out instead of assumed",
    },
    // Supply voltage was the worst of the three: `supply_mv_max <= 3300` read the
    // part's maximum RATING as though it were a requirement, so a part rated
    // 1.65-5.5 V was rejected for being too capable. The real requirement is that
    // the operating range CONTAINS the 3.3 V rail, which takes two bounds.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_min",
        pred: crate::select::Predicate::AtMost(3300.0),
        why: "the part must operate at the board's 3.3 V rail, so its minimum supply cannot \
              exceed it",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_max",
        pred: crate::select::Predicate::AtLeast(3300.0),
        why: "and its maximum must reach 3.3 V. Note the DIRECTION: a higher maximum rating is \
              better, not worse. The earlier AtMost(3300) rejected the TI mux (rated 1.65-5.5 V) \
              for tolerating more than the board supplies, which is the opposite of a defect",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "mounting",
        pred: crate::select::Predicate::OneOf(&["SMD"]),
        why: "hand-assembled small batch, so no through-hole. `mounting` is what distributors \
              publish; specific package names vary too much between them to filter on",
    },
    // TME returns "Clock frequency" EMPTY for analog switches, so as a catalogue
    // constraint this rejected the correct mux for a field the vendor never fills.
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "max_freq_hz",
        pred: crate::select::Predicate::AtLeast(24_576_000.0),
        why: "it has to pass the faster of the two oscillators. Datasheet-only: TME returns an \
              empty 'Clock frequency' for analog switches, so no catalogue can answer this and \
              treating it as a catalogue question disqualifies every candidate",
    },
    // The defining property of the part, published by nobody.
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "break_before_make",
        pred: crate::select::Predicate::IsTrue,
        why: "without it the two oscillators are briefly shorted together during a switch, which \
              is why a PLL-based clock multiplier (NB3N511) is the wrong CLASS of part and not \
              merely a worse one. Datasheet-only: no distributor publishes it parametrically, so \
              it must be confirmed per candidate from the datasheet and recorded in the note",
    },
];

/// Terminations that let a user swap the cell with their hands.
///
/// Hot-swap and user-replaceability both come down to this: a connectorised cell
/// can be unplugged, a solder-tab cell cannot. EU Battery Regulation Art. 11
/// requires user-replaceability as a floor; being able to carry a charged spare
/// is the stronger property the product wants, and it is the connector that
/// delivers both.
const USER_SWAPPABLE_TERMINATIONS: &[&str] =
    &["JST-PH-2.0", "JST-PH", "connector", "molex-picoblade"];

/// The battery cell, machine-checkable.
///
/// Several of these are *licensing* constraints wearing electrical clothing. The
/// certification for a lithium cell belongs to whoever manufactures the battery,
/// and buying bare cells to protect ourselves would make that us — which is why
/// `integrated_pcm` is a hard filter and not a preference. See
/// [`crate::licensing::BATTERY_CERT_CONDITIONS`].
const BATTERY_CONSTRAINTS: &[Constraint<'static>] = &[
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "integrated_pcm",
        pred: crate::select::Predicate::IsTrue,
        why: "the cell must arrive as a finished, pre-certified assembly with its own protection \
              circuit. Buying bare cells and adding our own PCM makes US the battery \
              manufacturer, inheriting UN 38.3 and IEC 62133-2 testing — a five-figure \
              certification programme acquired by a sourcing decision",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "un38_3_report",
        pred: crate::select::Predicate::IsTrue,
        why: "UN 38.3 transport testing must exist FOR THIS MODEL and be obtainable from the \
              vendor. Certification is per model, so a vendor's general compliance claim is not \
              the document that ships with the product",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "iec62133_2_report",
        pred: crate::select::Predicate::IsTrue,
        why: "IEC 62133-2 cell safety report, per model, from the vendor — the other half of the \
              archive that lets certification sit with them rather than us",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "termination",
        pred: crate::select::Predicate::OneOf(USER_SWAPPABLE_TERMINATIONS),
        why: "a connectorised cell is what makes a spare battery usable: the user unplugs one and \
              plugs in another. Solder tabs would satisfy nothing — not Art. 11 \
              user-replaceability, and certainly not carrying a charged spare",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "cells_series",
        pred: crate::select::Predicate::NumEq {
            value: 1.0,
            rel_tol: 0.0,
        },
        why: "1S. The charger, the fuel gauge and every rail in the power model assume a single \
              3.7 V nominal cell; a 2S pack is a different power tree, not a bigger battery",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "capacity_mah",
        pred: crate::select::Predicate::AtLeast(1000.0),
        why: "below ~1000 mAh the runtime drops out of the compact-DAP class the product is \
              measured against (Shanling M0 Pro ~14.5 h, FiiO M6 ~12 h)",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "watt_hours",
        pred: crate::select::Predicate::AtMost(100.0),
        why: "above 100 Wh a lithium battery installed in equipment leaves UN3481 PI967 Section \
              II and needs Class 9 dangerous-goods shipping. The design is ~4 Wh, so this is a \
              guard against a future cell choice quietly changing how the product can be posted",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "thickness_mm",
        pred: crate::select::Predicate::AtMost(7.1),
        why: "the battery bay is 35 x 59 x 7.1 mm",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "width_mm",
        pred: crate::select::Predicate::AtMost(35.0),
        why: "battery bay width",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "length_mm",
        pred: crate::select::Predicate::AtMost(59.0),
        why: "battery bay length",
    },
];

/// The Bluetooth module — **not populated on the shipped board**, and specified
/// anyway.
///
/// Specified because the constraints are the expensive part. Every one of these
/// is a licensing or certification inheritance: a bare Bluetooth *chip* costs a
/// SIG qualification programme and, for Qualcomm parts, an NDA and a gated SDK. A
/// pre-qualified *module* costs neither, because its QDID and its RED/FCC radio
/// approvals are inheritable. The difference between those two choices is roughly
/// the entire licensing budget of the product.
const BLUETOOTH_CONSTRAINTS: &[Constraint<'static>] = &[
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "is_module",
        pred: crate::select::Predicate::IsTrue,
        why: "a module, never a bare chip. The module vendor has already run the radio through \
              qualification and certification; starting from a chip means running both ourselves",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "bt_sig_qualified",
        pred: crate::select::Predicate::IsTrue,
        why: "the module must carry its own Bluetooth SIG qualification (a QDID we can reference \
              in an end-product Declaration). Without one, using Bluetooth means a full \
              qualification programme rather than a ~$5k Declaration — this is exactly how \
              Tangara inherits the ESP32's QDID",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "radio_certified_eu",
        pred: crate::select::Predicate::IsTrue,
        why: "pre-certified for RED/CE, so populating the radio does not require a radiated \
              emissions programme on our own board. Note this is not a full exemption: the end \
              product still needs its own EMC test (Tangara needed one despite a pre-certified \
              module)",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "requires_vendor_nda",
        pred: crate::select::Predicate::IsFalse,
        why: "an NDA-gated SDK is incompatible with shipping open-source firmware: the Qualcomm \
              QCC route needs an NDA and ADK access. A pre-made module encapsulates the \
              NDA-bound firmware behind a documented host interface, which is why this is a hard \
              filter rather than an inconvenience",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "codec_sbc",
        pred: crate::select::Predicate::IsTrue,
        why: "SBC is the mandatory royalty-free codec and the only one the product commits to. \
              aptX is licensed through the module vendor and LDAC source has no off-the-shelf \
              module, so neither is required — a module that offers ONLY licensed codecs would \
              make the audio path depend on a fee",
    },
    Constraint {
        evidence: Evidence::Catalogue,
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
        what: "22.5792 MHz fixed oscillator, operating range covering 3.3 V, low phase noise, SMD",
        constraints: OSC_44K1_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "ASV-22.5792MHZ-LR-T",
                preference: Preference::Evaluating,
                note: "DATASHEET-CONFIRMED 2026-11-24 (abracon.com/Oscillators/ASV.pdf, see \
                       docs/sourcing/datasheet-confirmations/). An ACTIVE 3.3Vdc HCMOS clock \
                       oscillator — not a crystal — suffixes decoding to 3.3 V +/-10% \
                       (2.97-3.63 V), -40..+85 C, +/-25 ppm, 1000/reel, corroborated \
                       independently by TME's own parameters. 17000 in stock, the ONLY active XO \
                       TME stocks at this frequency. \
                       \
                       Not Preferred, for two measured reasons. Jitter is 3.2 ps RMS period \
                       jitter for the band containing 22.5792 MHz, roughly an order of magnitude \
                       worse than dedicated low-phase-noise audio oscillators — and this design's \
                       stated premise is that MCLK jitter is the one thing to avoid. And it is \
                       reel-only: MOQ 1000 at 0.70 EUR is a ~700 EUR minimum order for a part a \
                       prototype needs one of.",
            },
            Candidate {
                mpn: "KC2016Z22.5792C1KX00",
                preference: Preference::Evaluating,
                note: "CATALOGUE-CONFIRMED: active oscillator, 22.5792 MHz, +/-20 ppm, 3.3 VDC, \
                       -40..+85 C, CMOS, 2.0x1.6x0.8 mm. \
                       \
                       The reason to take this series seriously is that KC2016Z offers a MATCHED \
                       PAIR — KC2016Z24.5760C1KX00 is the same series, package, supply, \
                       temperature range and stability at 24.576 MHz. A dual-oscillator design \
                       wants exactly that: two clocks whose only intended difference is \
                       frequency. No other series TME carries covers both families. \
                       \
                       Against it: zero stock, MOQ 2000, and a 2.0x1.6 mm package that is \
                       markedly harder to hand-place than the ASV's 7.0x5.08 mm. Phase noise \
                       still needs a datasheet read.",
            },
            Candidate {
                mpn: "ASDMB-22.5792MHZ-LY-T",
                preference: Preference::Rejected(
                    "MEMS, and measurably the wrong trade for this design. Datasheet \
                     (abracon.com/Oscillators/ASDMB.pdf): 5-10 ps RMS period jitter against the \
                     ASV quartz part's 3.2 ps; 0..+70 C commercial temperature, which a PORTABLE \
                     device leaves behind outdoors in winter; and a supply range topping out at \
                     exactly 3.3 V, so no margin on the rail it has to run from. Worse on every \
                     axis that matters here",
                ),
                note: "was the spec's placeholder for this requirement until it was actually \
                       read. Kept Rejected so the MEMS-vs-quartz question is not re-opened from \
                       scratch",
            },
        ],
    },
    PartReq {
        id: "mclk-osc-48k",
        what: "24.576 MHz fixed oscillator, operating range covering 3.3 V, low phase noise, SMD",
        constraints: OSC_48K_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "KC2016Z24.5760C1KX00",
                preference: Preference::Evaluating,
                note: "CATALOGUE-CONFIRMED: active oscillator, 24.576 MHz, +/-20 ppm, 3.3 VDC, \
                       -40..+85 C, CMOS, 2.0x1.6x0.8 mm — the 48 kHz half of the KC2016Z matched \
                       pair, identical to KC2016Z22.5792C1KX00 in every respect but frequency. \
                       \
                       Zero stock and MOQ 2000, like every active oscillator TME carries at this \
                       frequency. Phase noise needs a datasheet read.",
            },
            Candidate {
                mpn: "ASDMB-24.576MHZ-LY-T",
                preference: Preference::Rejected(
                    "MEMS — see the 44.1 kHz family for the measured reasons (5-10 ps jitter, \
                 0..+70 C, no supply margin)",
                ),
                note: "NO VALIDATED CANDIDATE REMAINS for this requirement, and that is the \
                   finding. A sweep of 40 parts TME carries at 24.576 MHz on 2026-11-24 found \
                   eight-plus ACTIVE oscillators and every single one at ZERO stock, MOQ \
                   1000-2000; the only stocked parts in the category are passive crystals. \
                   \
                   So TME alone cannot supply this clock tree: 22.5792 MHz has exactly one \
                   reel-only XO in stock and 24.576 MHz has none. The dual-oscillator design \
                   needs BOTH, which makes fetching Mouser and Digi-Key a prerequisite for the \
                   clock tree rather than a nice-to-have. Abracon's ASV series covers 24.576 MHz \
                   per its datasheet, so an ASV part is the obvious thing to look for there — \
                   deliberately not named here, because an unvalidated MPN is what this \
                   requirement just finished removing.",
            },
        ],
    },
    PartReq {
        id: "mclk-mux",
        what: "2:1 clock mux, break-before-make, <=50 MHz, single digital select",
        constraints: MUX_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "74LVC1G3157GW",
                preference: Preference::Preferred,
                note: "DATASHEET-CONFIRMED 2026-11-24 (nexperia.com product page, retrieved \
                       with a headed browser — the site 403s a plain GET): \
                       'Break-before-make switching', Ron 6.5 ohm typical at VCC = 3.3 V and 7.5 \
                       ohm at 2.7 V, so the spec's 6-7.5 ohm claim is accurate. \
                       \
                       NOT carried by TME, which is the live problem with preferring it.",
            },
            Candidate {
                mpn: "SN74LVC1G3157DCKR",
                preference: Preference::Alternate,
                note: "DATASHEET-CONFIRMED 2026-11-24 (TI SCES424O): 'Specified break-before-make \
                       switching' as a front-page feature, with a characterised T_B-M parameter \
                       in four timing tables and a dedicated internal-timing figure — so the \
                       Datasheet-evidence constraint is satisfied with more rigour than a bare \
                       claim. Operating frequency typically 340 MHz (14x the 24.576 MHz needed), \
                       1.65-5.5 V VCC matching TME's catalogue value exactly, SPDT, SC70-6. \
                       \
                       Ron is 7-9 ohm MAXIMUM at VCC = 3 V and 25 C. Note TI publishes maximums \
                       where Nexperia publishes typicals, so the two are the same class and Ron \
                       does not actually separate them — an earlier reading of this comparison \
                       was unfair to Nexperia. \
                       \
                       What does separate them is availability: 6916 in stock at TME, MOQ 1, \
                       0.189 EUR, against a Preferred part TME does not carry at all.",
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
    PartReq {
        id: "battery",
        what: "1S Li-Po, >=1000 mAh, finished cell with its own PCM, connectorised, fits a \
               35 x 59 x 7.1 mm bay, per-model UN 38.3 + IEC 62133-2 docs available",
        constraints: BATTERY_CONSTRAINTS,
        candidates: &[Candidate {
            mpn: "AKY-LP623255",
            preference: Preference::Evaluating,
            // Evaluating rather than Preferred on purpose. The licensing research
            // records this as the documented prototype cell, but a second source
            // has not been researched, and `requirements_have_a_preference_and_a_
            // second_source` would then demand an Alternate — which could only be
            // satisfied by inventing one.
            note: "Akyga AKY-LP623255: 1100 mAh, 6.2 x 32 x 55 mm, PCM + JST, vendor issues the \
                   per-model AKY0646 docs. The documented prototype cell (licensing research §1); \
                   marked Evaluating because no second source is researched yet, and a 1500 mAh \
                   class-leading cell is deferred pending a bay tweak.",
        }],
    },
    PartReq {
        id: "bluetooth",
        what: "pre-qualified BT module (inheritable QDID + EU radio cert), SBC, no vendor NDA — \
               DO NOT POPULATE on the shipped wired-only board",
        constraints: BLUETOOTH_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "FSC-BT1035",
                preference: Preference::Evaluating,
                note: "Feasycom FSC-BT1035 (QCC3056). The documented if/when choice: inherits the \
                       module's RED/FCC/BQB radio certs and hides the Qualcomm NDA behind a host \
                       interface. Evaluating, not Preferred — the shipped board leaves it \
                       unpopulated, and populating it moves the device from the EMC Directive to \
                       the RED (see licensing::regime_for) and adds an antenna window.",
            },
            Candidate {
                mpn: "STM32WB55RGV6TR",
                preference: Preference::Rejected(
                    "v1 part, and the wrong shape of decision: a bare BLE SoC rather than a \
                     pre-qualified module, so Bluetooth qualification and EU radio certification \
                     would both be ours to run instead of inherited",
                ),
                note: "kept so the module-vs-chip choice is not re-litigated from scratch",
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
