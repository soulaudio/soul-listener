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

// NO_CONSTRAINTS used to live here, as the placeholder for a prose-only
// requirement. It is gone because every requirement in this revision now has a
// constraint set — the compiler noticing it was unused is the milestone.

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

/// An *active* clock source, in the pipeline's canonical vocabulary.
///
/// The oscillator category mixes passive parts (`crystal`, `ceramic`,
/// `resonator` — a quartz element with no output driver) with active ones. Only
/// an active part can drive the CS43131's XTI/MCLK pin *and* the SAI's clock
/// input from one output, which is the whole topology; a crystal would need an
/// oscillator circuit built around it and could not present a clock to two
/// consumers.
///
/// One value, not four. Vendors disagree on spelling — TME says `oscillator` and
/// `XO`, Digi-Key says `XO (Standard)` — and this list briefly carried all of
/// them. That was the wrong place to absorb it: `tools/sourcing/canon.py` now
/// translates vendor vocabulary at export, so the spec names the concept once and
/// adding a third distributor means adding a synonym rather than editing a
/// requirement.
const ACTIVE_CLOCK_TYPES: &[&str] = &["oscillator"];

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

/// The battery cell, machine-checkable.
///
/// **Re-evidenced 2026-11-24 against TME's rechargeable-battery category.** The
/// first version of this set was written from the licensing research and assumed
/// the certification documents were part attributes; reading 70 real Li-Po cells
/// showed which of these a distributor answers and which only a human can.
///
/// Several of these are *licensing* constraints wearing electrical clothing: the
/// certification for a lithium cell belongs to whoever manufactures the battery,
/// and buying bare cells to protect ourselves would make that us. See
/// [`crate::licensing::BATTERY_CERT_CONDITIONS`].
/// # Deferred: catalogue-level SIZE filtering
///
/// Requested, and deliberately not enabled yet. The attribute exists and is
/// already mapped — Digi-Key publishes `Size / Dimension` as
/// `'2.44" L x 1.50" W x 0.24" H (62.0mm x 38.1mm x 6.0mm)'` — so the gap is not
/// the data model. It is three things, in order:
///
/// 1. **Coverage.** 5 of 627 candidates publish it, all of them Jauch cells from
///    Digi-Key. TME publishes no dimensions for cells at all. Switching
///    `thickness_mm` from `Evidence::Datasheet` to `Catalogue` today would
///    reject 622 cells for a vendor data gap and leave the AKYGA parts — the
///    only ones that actually qualify — rejected too, since TME is where they
///    come from.
/// 2. **A parse rule.** `parametric::derive` has no rule for a dual-unit,
///    three-axis string. It needs to extract the millimetre triple specifically,
///    because the inch figures are rounded (`0.24"` is 6.096 mm against a stated
///    6.0 mm) and a 7.1 mm bay has no room for that error.
/// 3. **Three attributes, not one.** A cell that fits 35 x 59 x 7.1 mm in one
///    orientation may not in another, so `body_length_mm`, `body_width_mm` and
///    `body_thickness_mm` are separate constraints — and whether the cell may be
///    rotated is a mechanical decision, not an arithmetic one.
///
/// So the enabling order is: teach `derive` the triple, add the three
/// dimensions, wait for (or add) a distributor that publishes cell dimensions,
/// then flip the evidence. Until then `thickness_mm` stays datasheet-only and
/// the bay is checked by a person against a drawing, which is what the
/// constraint's own `why` already says.
///
/// # Swappable, with the device off
///
/// Scope, stated because a connectorised cell invites the stronger reading:
/// the owner powers the device down, opens the bay, unplugs one cell and plugs
/// in another. That is what the constraints below deliver — a JST-connectorised
/// finished cell with its own protection circuit, obtainable as a spare.
///
/// **Hot swap — replacing the cell while the device runs — is explicitly not a
/// requirement.** It would be a property of the power tree rather than of the
/// cell (a single-cell device loses its only supply the moment the connector
/// parts, so it needs holdup across the gap), and nothing here is designed for
/// it. Recorded so that "swappable" is not quietly read as the stronger claim
/// later, and so no one adds a supercapacitor to satisfy a requirement that was
/// never made.
const BATTERY_CONSTRAINTS: &[Constraint<'static>] = &[
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "battery_chemistry",
        pred: crate::select::Predicate::OneOf(&["Li-Po", "Li-Pol", "LiPo"]),
        why: "a lithium-polymer pouch cell. The category also holds NiMH, NiCd and lead-acid, \
              none of which suit a 7 mm-thick portable device or the charger in the power model",
    },
    // The condition the whole certification argument rests on, and it IS
    // published: TME lists 'PCB protection' under the features parameter.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "battery_features",
        pred: crate::select::Predicate::Contains("PCB protection"),
        why: "the cell must arrive as a finished assembly with its own protection circuit. \
              Buying bare cells and adding our own PCM makes US the battery manufacturer, \
              inheriting UN 38.3 and IEC 62133-2 testing — a five-figure certification programme \
              acquired by a sourcing decision. A substring match because the vendor publishes a \
              multi-valued feature list",
    },
    // What makes a spare cell usable, and also published — as part of a
    // multi-valued 'Leads' list, which is why the predicate is Contains and why
    // the adapter joins multiple values instead of keeping the first.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "termination",
        pred: crate::select::Predicate::Contains("JST"),
        why: "a connectorised cell is what makes a spare battery usable: the user unplugs one and \
              plugs in another. JST is the de-facto standard for single-cell Li-Po, so a spare is \
              obtainable from more than one maker. Soldered tabs would satisfy neither that nor \
              EU Battery Regulation Art. 11 user-replaceability",
    },
    // 3.7 V nominal IS the 1S statement. A cells-in-series count is not
    // published anywhere in this category; the rated voltage says it exactly,
    // and a 2S pack reads 7.4 V.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "rated_voltage_mv",
        pred: crate::select::Predicate::NumEq {
            value: 3700.0,
            rel_tol: 0.05,
        },
        why: "1S. The charger, the fuel gauge and every rail in the power model assume a single \
              3.7 V nominal cell; a 2S pack reads 7.4 V and is a different power tree rather than \
              a bigger battery. The tolerance absorbs vendors quoting 3.6 V or 3.8 V for the same \
              chemistry",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "capacity_mah",
        pred: crate::select::Predicate::AtLeast(1000.0),
        why: "below ~1000 mAh the runtime drops out of the compact-DAP class the product is \
              measured against (Shanling M0 Pro ~14.5 h, FiiO M6 ~12 h)",
    },
    // --- Datasheet-only, on evidence ------------------------------------
    //
    // None of the following appear in the category's parametric data. They are
    // vendor documents and mechanical drawings, so leaving them as catalogue
    // constraints would reject all 70 real cells for fields no distributor
    // publishes — the trap break_before_make and phase noise both fell into.
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "un38_3_report",
        pred: crate::select::Predicate::IsTrue,
        why: "UN 38.3 transport testing must exist FOR THIS MODEL and be obtainable from the \
              vendor. Datasheet-only: certification is a document, not a parametric field, and no \
              distributor publishes it — confirm per candidate and archive it",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "iec62133_2_report",
        pred: crate::select::Predicate::IsTrue,
        why: "IEC 62133-2 cell safety report, per model, from the vendor. Datasheet-only for the \
              same reason as UN 38.3",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "watt_hours",
        pred: crate::select::Predicate::AtMost(100.0),
        why: "above 100 Wh a lithium battery installed in equipment leaves UN3481 PI967 Section \
              II and needs Class 9 dangerous-goods shipping. Datasheet-only, though it is \
              arithmetic on two published figures (capacity x rated voltage) — at ~4 Wh this \
              design is nowhere near the threshold, so the guard is against a future cell choice \
              quietly changing how the product can be posted",
    },
    // Dimensions are not published in this category. The MPN encodes them
    // (AKY-LP623255 is 6.2 x 32 x 55 mm) but parsing physical specs out of a
    // part number is exactly the fragile inference this pipeline avoids.
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "thickness_mm",
        pred: crate::select::Predicate::AtMost(7.1),
        why: "the battery bay is 35 x 59 x 7.1 mm. Datasheet-only: TME publishes no dimensions \
              for cells, and although the Akyga part number encodes them, reading physical specs \
              out of an MPN is the kind of inference that is right until it is silently wrong",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "width_mm",
        pred: crate::select::Predicate::AtMost(35.0),
        why: "battery bay width; datasheet-only, as above",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "length_mm",
        pred: crate::select::Predicate::AtMost(59.0),
        why: "battery bay length; datasheet-only, as above",
    },
];

/// The Bluetooth module — **not populated on the shipped board**, and specified
/// anyway.
///
/// Specified because the constraints are the expensive part. A bare Bluetooth
/// *chip* costs a SIG qualification programme and, for Qualcomm parts, an NDA
/// and a gated SDK; a pre-qualified *module* costs neither because its QDID and
/// RED/FCC approvals are inheritable. The difference between those two choices
/// is roughly the entire licensing budget of the product.
///
/// **Re-evidenced 2026-11-24** against 100 real modules swept from Digi-Key —
/// the first time this requirement had any candidates at all. The result is the
/// same correction phase noise and break-before-make went through: almost
/// nothing here is a parametric field. A distributor publishes the radio's
/// frequency, supply, temperature and package; it does not publish a QDID, a
/// certification number, or whether an SDK is NDA-gated. Left as catalogue
/// constraints these rejected all 100 modules for fields nobody carries.
/// The microSD socket.
///
/// # Why this requirement exists at all
///
/// `interfaces.ato` has carried an `SDMMCBus` and `mcu.ato` an `mcu.sdcard`
/// port since V1, and the firmware's whole storage path assumes a card — but no
/// socket was ever sourced, so the one component the user physically touches
/// most often was the only one with no part behind it. A bus with no connector
/// is not a storage subsystem.
///
/// # microSD, not SD
///
/// Both vendors publish the distinction and they publish it in different
/// fields: Digi-Key as `Card Type` ('microSD™', or 'Secure Digital - microSD™'),
/// TME as `Kind of card` ('microSD'). A full-size SD socket is not a worse
/// option here, it is the wrong part — the card cage alone is wider than the
/// 35 mm bay the battery already shares.
///
/// The trademark glyph is the reason this is `Contains` rather than an exact
/// match: `'microSD™' != 'microSD'`, and `canon.py` folds both spellings plus
/// Digi-Key's `'Secure Digital - microSD™'` onto one token. `Contains` would
/// also match `'microSDHC'`, which is correct — the socket is identical and the
/// capacity class is a property of the card, not the connector.
const MICROSD_CONSTRAINTS: &[Constraint<'static>] = &[
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "card_type",
        pred: crate::select::Predicate::Contains("microSD"),
        why: "microSD, not full-size SD. A full-size cage does not fit beside a 35 x 59 mm \
              battery bay in a 7.1 mm-thick device, and the firmware's SDMMC path is 4-bit \
              either way. Both distributors publish this — Digi-Key as 'Card Type', TME as \
              'Kind of card' — so it is the one genuinely catalogue-checkable half of the \
              requirement",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "insertion_removal",
        pred: crate::select::Predicate::Contains("push-"),
        why: "the card must be removable by hand, repeatedly, without a tool. Matches both \
              'push-push' (spring-ejected) and 'push-pull' (gripped), which are the two \
              mechanisms that qualify; a bare 'with hinged cover' cage does not and is \
              excluded. \
              \
              Contains rather than OneOf, and the first draft got this wrong: TME publishes \
              this field MULTI-VALUED and joined, as 'push-push; top board mount', \
              'bottom board mount; push-push' and 'push-pull; top board mount; with hinged \
              cover'. An exact match against either token rejected all 398 sockets — the same \
              mistake the battery requirement's 'Leads' and 'features' constraints were \
              already written as Contains to avoid",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "mounting",
        pred: crate::select::Predicate::OneOf(&["SMD"]),
        why: "hand-assembled small batch, so surface mount. TME publishes connector mounting as \
              'SMT' and Digi-Key as 'Surface Mount, Right Angle'; both reach this as 'SMD' \
              through canon.py, because the orientation is a layout choice and not a different \
              mounting technology",
    },
    // --- Datasheet-only ---------------------------------------------------
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "card_detect_switch",
        pred: crate::select::Predicate::IsTrue,
        why: "a card-detect contact, so firmware learns a card has arrived instead of polling \
              the bus for one. \
              \
              Datasheet rather than Catalogue, which the first draft got wrong: Digi-Key does \
              publish 'Features' = 'Switch' for some sockets, but TME publishes no features \
              list for card connectors AT ALL, so as a catalogue constraint it rejected all 400 \
              TME parts for a vendor data gap rather than a design fault. Presence of a \
              card-detect pin is on every socket's drawing, and the pin count hints at it \
              ('9 (8 + 1)' is eight signal contacts plus one switch)",
    },
    // --- Datasheet-only ---------------------------------------------------
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "insertion_cycles",
        pred: crate::select::Predicate::AtLeast(5000.0),
        why: "the user-swappable storage promise is worthless if the socket wears out. 5000 \
              cycles is the commodity figure for this class; distributors publish contact \
              plating and current rating but never a cycle count, so this is read off the \
              datasheet of whichever socket is chosen",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "sd_logo_not_required",
        pred: crate::select::Predicate::IsTrue,
        why: "the SD Association licenses the LOGO and the full specification, not the \
              electrical interface. This product uses SDMMC in SPI/4-bit mode with no SD mark \
              on the artwork, which is the position licensing::EXPOSURES already records — this \
              constraint exists so the socket choice cannot quietly reintroduce a branded part \
              that implies membership we do not hold",
    },
];

const BLUETOOTH_CONSTRAINTS: &[Constraint<'static>] = &[
    // The one certification-adjacent fact a catalogue DOES answer: Digi-Key
    // publishes `Package / Case: 'Module'`, which is precisely the
    // module-versus-bare-chip distinction the licensing argument turns on.
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "package",
        pred: crate::select::Predicate::Contains("Module"),
        why: "a module, never a bare chip. The module vendor has already run the radio through \
              qualification and certification; starting from a chip means running both ourselves. \
              Catalogue-checkable because distributors classify the package as 'Module'. \
              \
              Known coverage gap, investigated 2026-11-24: unpublished for 338 of 806 \
              candidates (42%), and that is a REAL vendor gap rather than a mapping one. TME \
              publishes no package field for its IoT module category at all — it carries \
              'Dimensions' (18x25.5x3.1mm) and 'Type of communications module' (IoT) instead — \
              so only 62 of its 444 parts can answer this, against 410 of Digi-Key's 448. The \
              rejections are correct: a part whose form factor nobody states is not a part \
              known to be a module",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_min",
        pred: crate::select::Predicate::AtMost(3300.0),
        why: "must operate from the board's 3.3 V rail",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_max",
        pred: crate::select::Predicate::AtLeast(3300.0),
        why: "and must tolerate 3.3 V",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "mounting",
        pred: crate::select::Predicate::OneOf(&["SMD"]),
        why: "hand-assembled small batch, so surface mount",
    },
    // --- Datasheet / certification-only --------------------------------
    //
    // None of these is a parametric field at any distributor. They are the
    // reason the requirement exists, and they can only be confirmed by reading a
    // qualification record.
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "bt_sig_qualified",
        pred: crate::select::Predicate::IsTrue,
        why: "the module must carry its own Bluetooth SIG qualification — a QDID we can reference \
              in an end-product Declaration. Without one, using Bluetooth means a full \
              qualification programme rather than a ~EUR 5k Declaration, which is exactly how \
              Tangara inherits the ESP32's QDID. Datasheet-only: a QDID is a certificate, not a \
              parametric field",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "radio_certified_eu",
        pred: crate::select::Predicate::IsTrue,
        why: "pre-certified for RED/CE, so populating the radio needs no radiated-emissions \
              programme on our own board. Not a full exemption — the end product still needs its \
              own EMC test, as Tangara found with a pre-certified ESP32. Datasheet-only",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "requires_vendor_nda",
        pred: crate::select::Predicate::IsFalse,
        why: "an NDA-gated SDK is incompatible with shipping open-source firmware: the Qualcomm \
              QCC route needs an NDA and ADK access. A pre-made module encapsulates the \
              NDA-bound firmware behind a documented host interface. Datasheet-only, and a hard \
              filter rather than an inconvenience",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "codec_sbc",
        pred: crate::select::Predicate::IsTrue,
        why: "SBC is the mandatory royalty-free codec and the only one the product commits to. \
              aptX is licensed through the module vendor and LDAC source has no off-the-shelf \
              module, so a module offering ONLY licensed codecs would make the audio path depend \
              on a fee. Datasheet-only",
    },
];

/// The MCU, machine-checkable.
///
/// Added 2026-11-24, after a sweep of TME's ST-microcontroller category showed
/// how much of this requirement a distributor actually answers: `core_clock_hz`
/// for 101 of 102 parts, `cpu_core` for 97, `supply_mv` for 99. The requirement
/// had been prose-only since the spec was written.
///
/// SRAM and flash stay datasheet-only, and not for want of trying: TME publishes
/// them in a single multi-valued `Memory` parameter (`['1.4MB SRAM',
/// '2MB FLASH']`), which cannot be reduced to one number without guessing which
/// entry is which. They are also the figures `dap-arch-tests`' budget gates
/// already enforce against the firmware's real memory map, which is a stronger
/// check than a catalogue string.
const MCU_CONSTRAINTS: &[Constraint<'static>] = &[
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "cpu_core",
        pred: crate::select::Predicate::Contains("Cortex-M7"),
        why: "the firmware is built for Cortex-M7: it depends on the double-precision FPU and the \
              AXI/AHB memory layout the placement doc reasons about. An M4 or M33 part is not a \
              slower option, it is a different target",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "core_clock_hz",
        pred: crate::select::Predicate::AtLeast(250_000_000.0),
        why: "the measured decode+resample budget needs >=250 MHz at VOS0; below that the \
              resampled pipeline misses real time (see power::model's \
              native_rate_reaches_vos3_while_resampling_needs_vos2)",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_min",
        pred: crate::select::Predicate::AtMost(3300.0),
        why: "must operate from the board's 3.3 V rail",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "supply_mv_max",
        pred: crate::select::Predicate::AtLeast(3300.0),
        why: "and must tolerate 3.3 V — a higher maximum rating is better, not worse",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "mounting",
        pred: crate::select::Predicate::OneOf(&["SMD"]),
        why: "hand-assembled small batch, so surface mount",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "sram_bytes",
        pred: crate::select::Predicate::AtLeast(1_048_576.0),
        why: ">=1 MB of CONTIGUOUS DMA-capable SRAM, which is the whole reason V2 drops the \
              external SDRAM. Datasheet-only: TME reports SRAM and flash in one multi-valued \
              `Memory` parameter that cannot be split without guessing, and contiguity is not a \
              catalogue concept at all. The firmware's real memory map is enforced far more \
              strictly by dap-arch-tests' budget gates",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "flash_bytes",
        pred: crate::select::Predicate::AtLeast(2_097_152.0),
        why: "2 MB flash. Datasheet-only for the same reason as SRAM",
    },
];

/// The DAC, machine-checkable.
///
/// Added 2026-11-24, once Digi-Key was fetched: it publishes 15 parametric fields
/// for the CS43131 where TME carries the part at all. This was the last
/// requirement still filtering nothing.
///
/// The audio figures that actually decide the part — dynamic range, THD+N, the
/// integrated Class-H amp, and which MCLK frequencies it accepts — are datasheet
/// characteristics that no distributor publishes, so they are marked as such
/// rather than silently dropped. What a catalogue *can* answer is whether the
/// part is the right shape: stereo, I2S, high-resolution, surface mount.
const DAC_CONSTRAINTS: &[Constraint<'static>] = &[
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "channels",
        pred: crate::select::Predicate::NumEq {
            value: 2.0,
            rel_tol: 0.0,
        },
        why: "stereo. Digi-Key publishes this as 'Number of D/A Converters', and a mono or \
              4-channel part is a different product rather than a worse one",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "data_interface",
        pred: crate::select::Predicate::Contains("I2S"),
        why: "I2S, because the SAI drives it and the whole native-rate design depends on the DAC \
              being an I2S slave. A substring match since vendors publish compound values like \
              'I2S/TDM'",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "resolution_bits",
        pred: crate::select::Predicate::AtLeast(24.0),
        why: "24-bit minimum: the decode path is 24/32-bit throughout and a 16-bit converter \
              would throw away resolution the rest of the chain preserves",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "mounting",
        pred: crate::select::Predicate::OneOf(&["SMD"]),
        why: "hand-assembled small batch, so surface mount",
    },
    Constraint {
        evidence: Evidence::Catalogue,
        attr: "package",
        pred: crate::select::Predicate::ExcludesAll(&["WLCSP", "BGA"]),
        why: "hand-solderable, which 'SMD' does not imply. Found by scaling the sweep to 1860               converters: CS43131-CWZR is the SAME DIE as the preferred part in a '42-UFBGA,               WLCSP' with 3873 in stock and Active status, and it passed every catalogue               constraint this requirement had — because a wafer-level chip-scale package is               indeed surface mount. A 0.4 mm-pitch WLCSP needs a stencil, paste and reflow with               no way to inspect or rework the joints, which is exactly what 'hand-assembled               small batch' rules out. Stated as an exclusion rather than an allowlist of               packages because vendors spell package names dozens of ways, and the one time               this spec enumerated allowed package codes it rejected every real oscillator",
    },
    // --- Datasheet-only ---------------------------------------------------
    //
    // None of the following is a parametric field at any distributor fetched so
    // far. Left as catalogue constraints they would reject every real DAC for
    // fields nobody publishes — the trap break_before_make and phase noise both
    // fell into.
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "dynamic_range_db",
        pred: crate::select::Predicate::AtLeast(120.0),
        why: ">=120 dB(A) dynamic range is the headline audio requirement, and the reason this \
              part was chosen over cheaper codecs. Datasheet-only: distributors publish bit depth \
              and interface, never measured DR",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "integrated_headphone_amp",
        pred: crate::select::Predicate::IsTrue,
        why: "an integrated ground-centred Class-H amp is what removes the separate amp stage, \
              the output DC-blocking capacitors and most of the playback current — the single \
              biggest reason V2 exists. Datasheet-only",
    },
    Constraint {
        evidence: Evidence::Datasheet,
        attr: "accepts_both_mclk_families",
        pred: crate::select::Predicate::IsTrue,
        why: "the part must accept both nominal MCLK frequencies (22.5792 and 24.576 MHz), or the \
              dual-oscillator clock tree cannot work. Datasheet-only, and the fact a careless \
              reading of it already cost one wrong oscillator pair — see MclkContract",
    },
];

/// Requirements with candidates — the soft lock in practice.
///
/// Stock figures are from the committed sourcing snapshot
/// (`hardware/bom/snapshot.json`), not from here; these notes record *why* a
/// candidate is where it is.
/// Preferred parts whose single EU-distributor stocking is accepted, and why.
///
/// # Why a table and not a weakened gate
///
/// `preferred_parts_have_more_than_one_eu_source` offers three resolutions and
/// the third is "accept single-sourcing explicitly, and record why" — which had
/// no mechanism, so the only way to take it was to make the gate quieter. An
/// exception that lives in the gate is invisible; an exception that lives here
/// answers "which parts are we exposed on, and on whose word?" by being read.
///
/// # This list cannot rot
///
/// `accepted_single_sourcing_is_still_warranted` asserts that every entry is
/// still a Preferred candidate AND still has fewer than two stocking EU
/// distributors. So an entry that becomes untrue fails the build and must be
/// deleted — the list shrinks on its own as sourcing improves, rather than
/// quietly excusing a part that no longer needs excusing.
///
/// A reason is required, and is the audit record a reviewer reads.
/// Requirements whose part the USER is expected to buy and replace themselves.
///
/// # Why this is a list and not a property of the part
///
/// "Widely available" means something different for a cell than for a DAC. A
/// converter needs two distributors so WE can build the board; a replaceable
/// battery needs more than that — the owner has to be able to buy a spare, in
/// three years, without this project's distributor accounts. That makes
/// MANUFACTURER diversity the thing that matters, not distributor count: if one
/// maker stops producing the cell, a second distributor stocking the same dead
/// part helps nobody.
///
/// So the two properties are enforced separately rather than conflated:
///
/// - `preferred_parts_have_more_than_one_eu_source` — can we build it.
/// - `user_swappable_requirements_span_two_manufacturers` — can the owner
///   replace it.
///
/// The microSD socket is deliberately NOT here. The user swaps the *card*, which
/// is not on this BOM at all; the socket is soldered down and is ours.
pub const USER_SWAPPABLE_REQUIREMENTS: &[&str] = &["battery"];

/// User-swappable requirements that do NOT yet span two manufacturers, and why.
///
/// Same discipline as [`SINGLE_SOURCE_ACCEPTED`]: the reason is the audit
/// record, and `accepted_single_manufacturer_is_still_warranted` deletes an
/// entry by failing the build once a second manufacturer qualifies.
pub const SINGLE_MANUFACTURER_ACCEPTED: &[(&str, &str)] = &[(
    "battery",
    "AKYGA is the only manufacturer whose cells pass this requirement, and the sweep behind \
     that statement is now large enough to mean something: 627 candidates across TME's whole \
     446-part battery category and Digi-Key's Li-Po keyword sweep, of which 31 qualify and \
     every one is AKYGA. Jauch Quartz is a genuine second maker and Digi-Key stocks it, but \
     its closest cell (LP503759JU) is 62.0 x 38.1 mm against a 35 x 59 x 7.1 mm bay, so it \
     fails on mechanics rather than on sourcing. \
     \
     This is accepted rather than resolved because the realistic fixes are both outside the \
     sourcing pipeline: widen the bay, or buy direct from a pouch-cell maker (Farnell and RS \
     stock more bare cells than either distributor fetched here). What is NOT acceptable is \
     shipping a user-replaceable-battery promise whose spare has one maker — that is the \
     exposure this entry exists to keep visible.",
)];

pub const SINGLE_SOURCE_ACCEPTED: &[(&str, &str)] = &[
    (
        "AKY-LP623255",
        "TME is the only distributor stocking it (1298 at MOQ 1), and this one is accepted on \
         MECHANICS rather than for want of an alternative. AKY0107 is the same maker's cell at \
         1350 mAh with 2873 across Digi-Key AND TME — it satisfies the two-source rule outright \
         — and it is 37 mm wide against a 35 mm bay. Of the 31 cells that pass this \
         requirement, LP623255 (6.2 x 32 x 55 mm) is the only one whose encoded geometry fits. \
         \
         So the exposure is real and the fix is cheap and known: widen the bay 2 mm and promote \
         AKY0107, which is recorded as the Alternate for exactly that reason. Until then a cell \
         that fits with one distributor beats a cell that does not fit with two. Note this is \
         SEPARATE from the single-manufacturer exposure in SINGLE_MANUFACTURER_ACCEPTED: every \
         qualifying cell is AKYGA either way, so promoting AKY0107 would close this entry and \
         not that one.",
    ),
    (
        "STM32H7A3ZIT6",
        "Mouser's 8 units are the only EU stock of this part: Digi-Key and TME both read \
         ZERO, though both list it Active at MOQ 1, so it is a shortage and not an \
         end-of-life. Resolution (a) was checked rather than assumed — a 29-part Digi-Key \
         sweep and a 50-part Mouser sweep of the H7A3/H7B3 range found NO 144-LQFP variant \
         with stock (-ZIT6Q is 0/0 as well), and the alternate STM32H7B3ZIT6 is 0 at all \
         three distributors, so there is no same-footprint answer to switch to. What does \
         exist is STM32H7A3IIT6Q, the identical die and flash in 176-LQFP with 2339 + 1770 \
         across both distributors; it is recorded as an Evaluating candidate because taking \
         it is a footprint change, which is a board decision rather than a sourcing one. \
         Accepted meanwhile: 8 units covers a 5-board prototype run, and the real mitigation \
         is either fetching a fourth distributor or committing to the 176-pin package.",
    ),
    (
        "CS43131-CNZ",
        "Digi-Key is the only distributor STOCKING it (347 at MOQ 1, Active); Mouser carries \
     both -CNZ and -CNZR at zero stock, reel-only (MOQ 490 and 4000), and TME does not \
     carry Cirrus at all. Accepted rather than resolved because every alternative is \
     worse: the one DAC with two stocked sources (CS43198-CNZ, Digi-Key 346 + Mouser 450) \
     omits the integrated ground-centred Class-H amp that this requirement is FOR, so \
     swapping to it is a board change, not a second source. Only three distributors have \
     been fetched — Farnell, Arrow and RS all carry Cirrus and are the realistic second \
     stocking source; adding one is the way this entry gets deleted rather than argued \
     with. Exposure is a build stop if Digi-Key runs dry, mitigated by the 490-piece reel \
     being orderable from Mouser on a 0-day lead.",
    ),
];

const PARTS: &[PartReq] = &[
    PartReq {
        id: "dac",
        what: "stereo DAC + headphone amp, I2S slave, both MCLK families, >=120 dB(A) DR",
        constraints: DAC_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "CS43131-CNZ",
                preference: Preference::Preferred,
                note: "chosen on audio merit: the integrated ground-centred Class-H amp is \
                       the reason this requirement exists, and the only alternate with two \
                       stocked sources (CS43198) does not have it. CORRECTION 2026-11-24: an \
                       earlier note claimed the -CNZ suffix was preferred because it was \
                       'stocked at both Mouser EU and Digi-Key EU' while -CNZR was Digi-Key \
                       only. The Mouser API refutes both halves — Mouser carries -CNZ and \
                       -CNZR at ZERO stock, reel-only at MOQ 490 and 4000 — and at Digi-Key \
                       both variants are MOQ 1 at the same EUR 16.44, so the suffix has no \
                       measured sourcing consequence. The 141-unit Mouser figure the old note \
                       rested on was hand-researched and never verified. Single EU stocking \
                       source, accepted in SINGLE_SOURCE_ACCEPTED",
            },
            Candidate {
                mpn: "CS43198-CNZ",
                preference: Preference::Alternate,
                note: "same family, no integrated amp — would need a separate amp stage, so \
                       this is a fallback with a board change, not a drop-in",
            },
            Candidate {
                mpn: "CS43131-CWZR",
                preference: Preference::Rejected(
                    "same die as the preferred part, but in a 42-UFBGA/WLCSP: a 0.4 mm-pitch \
                     wafer-level package cannot be hand-soldered or inspected, and this board \
                     is hand-assembled in small batches",
                ),
                note: "kept because it is the single best-STOCKED CS43131 anywhere — 3873 at \
                       Digi-Key against 347 for -CNZ — so it will keep surfacing as the \
                       obvious answer to the DAC's single-sourcing problem. It is not: the \
                       package is the blocker, not the supply. Recorded 2026-11-24, when it \
                       passed every catalogue constraint the requirement had and exposed that \
                       'mounting = SMD' never expressed hand-solderability",
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
                mpn: "NSC5083D-22.5792M",
                preference: Preference::Alternate,
                note: "DATASHEET-CONFIRMED 2026-11-24. NDK, NZ2520SDA family — the datasheet is \
                       titled 'Ultra Low Phase Noise Type' and states 'ultra low phase noise \
                       make this product ideal for High quality audio', with typical phase \
                       jitter of 43 FEMTOSECONDS. \
                       \
                       That is the number this requirement has been reaching for. Against the \
                       alternatives actually in stock: the Abracon ASV is 3.2 ps (74x worse) and \
                       the MEMS part the spec once proposed was 5-10 ps (150-230x worse), in a \
                       design whose stated premise is that MCLK jitter is the one thing to avoid. \
                       \
                       And it is a MATCHED PAIR: NSC5083D-24.576M is the same family and package \
                       at the other MCLK frequency, which is what the dual-oscillator topology \
                       wants. 2169 in stock at Digi-Key, MOQ 1, 2.04 EUR; -40..+85 C industrial; \
                       3.3 V; CMOS. \
                       \
                       Open before it can be Preferred: +/-50 ppm stability is looser than the \
                       Kyocera pair's +/-20 ppm (fine for audio playback, worth a deliberate \
                       decision), and the 2.5 x 2.0 x 0.9 mm package is small for hand assembly.",
            },
            Candidate {
                mpn: "CCHD-957-25-22.5792",
                preference: Preference::Evaluating,
                note: "Crystek CCHD-957 'FEMTO Clock', the reference audiophile oscillator: \
                       -100 dBc/Hz at 10 Hz offset, noise floor -169 dBc/Hz. Confirmed from \
                       crystek.com/specification/clock/CCHD-957.pdf. 67 in stock, MOQ 1. \
                       \
                       Three things count against it for THIS design, none of them about \
                       quality: 29.24 EUR is 14x the NDK part; 0..+70 C is commercial where the \
                       NDK is industrial; and NO 24.576 MHz variant appeared in the sweep, so it \
                       cannot form the matched pair the clock tree needs. Recorded because it is \
                       the right answer if the 44.1 kHz family alone ever needs the very best \
                       clock, and because its 14.2 x 9.14 mm package is trivially hand-placed.",
            },
            Candidate {
                mpn: "ASV-22.5792MHZ-LR-T",
                preference: Preference::Preferred,
                note: "DATASHEET-CONFIRMED (abracon.com/Oscillators/ASV.pdf, see \
                       docs/sourcing/datasheet-confirmations/): an active 3.3Vdc HCMOS clock \
                       oscillator, 2.97-3.63 V, -40..+85 C, +/-25 ppm. 18319 at TME and 1319 at \
                       Digi-Key, so it is the only candidate here with TWO EU sources. \
                       \
                       Demoted in practice by measurement rather than by preference: 3.2 ps RMS \
                       period jitter is 74x the NDK part's 43 fs at a comparable price. Kept as a \
                       fallback precisely because of the second source, and because its 7.0 x \
                       5.08 mm package is the easiest of the three to hand-solder.",
            },
            Candidate {
                mpn: "KC2016Z22.5792C1KX00",
                preference: Preference::Evaluating,
                note: "Kyocera AVX, +/-20 ppm — the tightest stability of the candidates — 1871 \
                       in stock at Digi-Key, MOQ 1, 1.09 EUR, and also a matched pair \
                       (KC2016Z24.5760C1KX00). Publishes no phase-noise figure, which is the \
                       blocker: the requirement cannot be satisfied by a part whose jitter is \
                       unknown, and this is a general-purpose XO rather than an audio part.",
            },
            Candidate {
                mpn: "ASDMB-22.5792MHZ-LY-T",
                preference: Preference::Rejected(
                    "MEMS, and measurably the wrong trade. Datasheet \
                     (abracon.com/Oscillators/ASDMB.pdf): 5-10 ps RMS period jitter against the \
                     NDK part's 43 fs and the ASV's 3.2 ps; 0..+70 C; and a supply range topping \
                     out at exactly 3.3 V, so no margin on the rail it runs from",
                ),
                note: "was the spec's placeholder for this requirement until it was read. Kept \
                       Rejected so the MEMS-vs-quartz question is not re-opened from scratch. \
                       \
                       CORRECTION to the original reasoning, 2026-11-24: this was argued partly \
                       as 'a portable device leaves 0..70 C behind outdoors in winter', which \
                       overstated it — the CS43131's own ambient range is -20..+70 C (DS1155F2), \
                       so the DESIGN was never a -40 C product. A 0 C floor is still worse and \
                       narrows the range further, but jitter is the decisive reason.",
            },
        ],
    },
    PartReq {
        id: "mclk-osc-48k",
        what: "24.576 MHz fixed oscillator, operating range covering 3.3 V, low phase noise, SMD",
        constraints: OSC_48K_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "NSC5083D-24.576M",
                preference: Preference::Alternate,
                note: "The 48 kHz half of the NDK NZ2520SDA matched pair: same family, same \
                       2.5 x 2.0 x 0.9 mm package, same -40..+85 C, same 43 fs typical phase \
                       jitter. 2867 in stock at Digi-Key, MOQ 1, 2.61 EUR. \
                       \
                       That both frequencies come from one ultra-low-phase-noise family is the \
                       whole argument: a dual-oscillator clock tree wants two clocks whose only \
                       intended difference is frequency. Crystek's femto-clock has no 24.576 MHz \
                       variant in the sweep, so it cannot do this.",
            },
            Candidate {
                mpn: "KC2016Z24.5760C1KX00",
                preference: Preference::Evaluating,
                note: "Kyocera AVX, +/-20 ppm, 1062 in stock at Digi-Key at MOQ 1, 1.10 EUR — \
                       and the 48 kHz half of its own matched pair. Publishes no phase-noise \
                       figure, which is what keeps it behind the NDK part.",
            },
            Candidate {
                mpn: "ASV-24.576MHZ-EJ-T",
                preference: Preference::Preferred,
                note: "The 48 kHz partner the ASV option never had. Abracon ASV, same family as \
                       the 44.1 kHz ASV-22.5792MHZ-LR-T this spec already carries, +/-20 ppm, \
                       4-SMD No Lead, MOQ 1, 2.00 EUR — and stocked at ALL THREE distributors \
                       (Digi-Key 4113, TME 10000, Mouser 4684), which makes it the \
                       best-sourced clock part in this spec by a wide margin. Its 22.5792 MHz \
                       sibling is also three-sourced, so this is the only PAIR where both \
                       frequencies clear the two-source AVL rule with a spare. \
                       \
                       Found by widening the 24.576 MHz sweep from 162 parts to 639: the ASV \
                       family was previously recorded at 22.5792 MHz only, so it could not form \
                       a pair and the two-source ASV route was closed. It is open now, and it \
                       is the only candidate pair where BOTH frequencies have two stocking EU \
                       distributors. \
                       \
                       The caveat is temperature and it is not a detail: this part is \
                       -20..+70 C commercial where its own 22.5792 MHz sibling is -40..+85 C \
                       industrial, so the pair is NOT grade-matched. Adequate for this design \
                       (the CS43131's own ambient range is -20..+70 C, so the DAC bounds it \
                       first either way) but strictly worse than the NDK pair, which is \
                       industrial at both frequencies and 43 fs. The trade is sourcing depth \
                       against jitter and temperature grade. \
                       \
                       TME publishes this part's type as 'crystal'. That is wrong — ASV is \
                       Abracon's XO family and Digi-Key says 'oscillator' — and it matters: the \
                       part passes `part_type` only because the export's tie-break picks the \
                       alphabetically-first vendor and 'digikey' precedes 'tme'. See \
                       `no_spec_candidate_depends_on_a_conflicted_attribute`.",
            },
            Candidate {
                mpn: "ASDMB-24.576MHZ-LY-T",
                preference: Preference::Rejected(
                    "MEMS — see the 44.1 kHz family for the measured reasons (5-10 ps jitter, \
                     0..+70 C, no supply margin)",
                ),
                note: "979 in stock at Digi-Key, so this one IS buyable — it is rejected on \
                       performance rather than availability, which is the distinction the \
                       pipeline exists to keep straight.",
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
                preference: Preference::Alternate,
                note: "DATASHEET-CONFIRMED 2026-11-24 (nexperia.com product page, retrieved \
                       with a headed browser — the site 403s a plain GET): \
                       'Break-before-make switching', Ron 6.5 ohm typical at VCC = 3.3 V and 7.5 \
                       ohm at 2.7 V, so the spec's 6-7.5 ohm claim is accurate. \
                       \
                       DEMOTED to Alternate 2026-11-24, on the spec's own AVL rule rather than on \
                       a preference. With both distributors fetched it has ONE EU source \
                       (Digi-Key, 40729) where the TI part has TWO (Digi-Key 895727 + TME 6903). \
                       Two sources for one MPN is what cuts shortage delay ~80%, and a single \
                       source is a single point of failure whatever the part costs. \
                       \
                       Nothing is wrong with the part: 6.5 ohm typical Ron at 3.3 V is \
                       marginally better than TI's figure, and break-before-make is confirmed. It \
                       is a sourcing decision, not an engineering one, and reversible the moment \
                       a second distributor stocks it.",
            },
            Candidate {
                mpn: "SN74LVC1G3157DCKR",
                preference: Preference::Preferred,
                note: "DATASHEET-CONFIRMED 2026-11-24 (TI SCES424O): 'Specified break-before-make \
                       switching' as a front-page feature, with a characterised T_B-M parameter \
                       in four timing tables and a dedicated internal-timing figure — so the \
                       Datasheet-evidence constraint is satisfied with more rigour than a bare \
                       claim. Operating frequency typically 340 MHz (14x the 24.576 MHz needed), \
                       1.65-5.5 V VCC matching TME's catalogue value exactly, SPDT, SC70-6. \
                       \
                       RANKED 1st OF 261 PARTS TME carries in the analog-switch category \
                       (2026-11-24 sweep: 56 qualified, 205 rejected with named reasons). So the \
                       Alternate is not merely available, it wins on merit against the whole \
                       category. Runner-up was TS5A3157DBVR (TI, 1437 in stock, 26 cents), which \
                       is the obvious second option if this part tightens. \
                       \
                       Ron is 7-9 ohm MAXIMUM at VCC = 3 V and 25 C. Note TI publishes maximums \
                       where Nexperia publishes typicals, so the two are the same class and Ron \
                       does not actually separate them — an earlier reading of this comparison \
                       was unfair to Nexperia. \
                       \
                       PROMOTED to Preferred 2026-11-24. What separates the two is sourcing \
                       depth, which is now measured rather than assumed: this part has TWO EU \
                       sources (Digi-Key 895727 and TME 6903, both MOQ 1) against the Nexperia \
                       part's one. It also ranked 1st of 261 parts in TME's analog-switch \
                       category. \
                       \
                       I declined this swap once before on the grounds that one distributor is \
                       not the EU market — correctly at the time, since only TME had been \
                       fetched. With both fetched the objection is gone and the spec's own \
                       two-source rule decides it.",
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
        constraints: MCU_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "STM32H7A3ZIT6",
                preference: Preference::Preferred,
                note: "1.4 MB SRAM total, 280 MHz, no external RAM needed",
            },
            Candidate {
                mpn: "STM32H7B3ZIT6",
                preference: Preference::Alternate,
                note: "same family and map, higher flash, pin-compatible. CORRECTION \
                       2026-11-24: this was described as the fallback 'if H7A3 supply \
                       tightens', and it cannot play that role today — it is ZERO stock at \
                       Digi-Key, Mouser and TME, as is -ZIT6Q. A pin-compatible part nobody \
                       stocks is a design fallback, not a supply one",
            },
            Candidate {
                mpn: "STM32H7A3IIT6Q",
                preference: Preference::Evaluating,
                note: "the SAME die and the same 2 MB flash as the preferred part in a \
                       176-LQFP instead of 144, so every memory figure this spec depends on \
                       (1024 KB AXI-SRAM, 128 KB DTCM, 280 MHz) is unchanged. Found by \
                       sweeping for a better-stocked variant when the 144-pin part went \
                       single-sourced: 2339 at Digi-Key and 1770 at Mouser, both MOQ 1, both \
                       Active — the only H7A3 with 2 MB flash that two EU distributors \
                       actually stock. Evaluating and NOT Alternate because 144 -> 176 pins \
                       is a footprint and routing change, and the Q suffix adds an internal \
                       SMPS whose external parts differ; it is a board decision, recorded \
                       here with the data rather than taken by the sourcing pipeline",
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
        id: "microsd",
        what: "microSD socket, push-push or push-pull, card-detect switch, SMD, >=5000 \
               insertion cycles",
        constraints: MICROSD_CONSTRAINTS,
        // All three came out of the sweep rather than a catalogue hunt by hand —
        // 173 of 601 sockets qualify, and these are the two-source ones with
        // MOQ 1. Recorded as Evaluating, not Preferred: choosing between them is
        // a mechanical decision about bezel cutout and board height that the
        // sourcing pipeline has no standing to make.
        //
        // Height is the axis that will probably decide it, and it is the one the
        // filter cannot yet judge: both vendors publish it, Digi-Key dual-unit
        // as '0.104" (2.65mm)', which `parse_quantity` does not read. See
        // MICROSD_CONSTRAINTS.
        candidates: &[
            Candidate {
                mpn: "MEM2080-00-128-00-A",
                preference: Preference::Preferred,
                note: "GCT, push-push, 10 (8 + 2) contacts so it has card-detect, and the \
                       THINNEST qualifying socket found at 1.28 mm above board. In a 7.1 mm \
                       device sharing its thickness budget with a 7.1 mm battery bay, 1.4 mm \
                       against the Amphenol 10067099's 2.65 mm is the difference that matters \
                       more than the 37 cents. 9963 across Digi-Key and TME, MOQ 1, 1.23 EUR.",
            },
            Candidate {
                mpn: "1140084168",
                preference: Preference::Alternate,
                note: "Amphenol, push-pull, 10 (8 + 2), 1.55 mm — nearly as thin as the GCT \
                       part with far deeper stock (75365 across two distributors, MOQ 1, \
                       1.53 EUR). Push-PULL rather than push-push: no ejector spring to fail, \
                       but the card must be gripped, which needs the cutout to expose it.",
            },
            Candidate {
                mpn: "10067099-200LF",
                preference: Preference::Evaluating,
                note: "Amphenol, push-pull, 10 (8 + 2), 41021 across two distributors at MOQ 1 \
                       and 0.86 EUR — the cheapest two-source socket that qualifies. Kept \
                       behind the other two on height alone: 2.65 mm is double the GCT part \
                       and the worst of the shortlist.",
            },
            Candidate {
                mpn: "2908-05WB-MG",
                preference: Preference::Rejected(
                    "8 contacts with no ninth: no card-detect switch, so firmware would have \
                     to poll the bus to notice a card. Also 4.23 EUR against 0.86-1.53 for \
                     the shortlist",
                ),
                note: "3M, push-push, two sources and 53053 in stock — it ranks highly on \
                       sourcing and is wrong on function, which is exactly the case the \
                       datasheet-only card_detect_switch constraint exists to catch. The pin \
                       count is the tell: '8' where every shortlisted part reads '10 (8 + 2)'.",
            },
            Candidate {
                mpn: "105027-0001",
                preference: Preference::Rejected(
                    "MOQ 2000. 150000 in stock at two distributors and unbuyable for a \
                     five-board prototype run — 3020 EUR for the first batch",
                ),
                note: "Molex, push-pull, 1.70 mm, 1.51 EUR per piece in a reel. Recorded \
                       because it ranks SECOND on raw stock and would otherwise keep \
                       resurfacing; Rank::AcquisitionCost is what demotes it, and this note \
                       is why.",
            },
        ],
    },
    PartReq {
        id: "battery",
        what: "1S Li-Po, >=1000 mAh, finished cell with its own PCM, connectorised, fits a \
               35 x 59 x 7.1 mm bay, per-model UN 38.3 + IEC 62133-2 docs available",
        constraints: BATTERY_CONSTRAINTS,
        candidates: &[
            Candidate {
                mpn: "AKY-LP623255",
                preference: Preference::Preferred,
                note: "PICKED 2026-11-24 because it is the only qualifying cell that FITS. The \
                       part number encodes 6.2 x 32 x 55 mm against a 35 x 59 x 7.1 mm bay; \
                       AKY0107 (LP503759) is 37 mm wide and does not, which is why the \
                       better-sourced, higher-capacity cell is the alternate rather than the \
                       pick. Fit is still DATASHEET evidence — neither distributor publishes \
                       cell dimensions, TME's product page states capacity and connector and no \
                       geometry at all — so the drawing has to be read before this is ordered. \
                       \
                       CATALOGUE-CONFIRMED 2026-11-24: 1100 mAh, 3.7 V, 'cables; JST SYP-02T-1 \
                       socket', features 'PCB protection' — so the integrated-PCM condition the \
                       whole certification argument rests on is confirmed from vendor data rather \
                       than assumed. 1298 in stock at TME, MOQ 1. Ranks 3rd of 31 qualifying \
                       cells. \
                       \
                       Identity note worth recording: TME holds this as SYMBOL AKY-LP623255 with \
                       MANUFACTURER PN AKY0646 — the reverse of the assumption, and the licensing \
                       research's reference to 'the per-model AKY0646 docs' is the manufacturer \
                       number. An mpns[]-only lookup reported this part 'not carried' while 1298 \
                       sat in stock, which is why the adapter now falls back to symbols[]. \
                       \
                       Still datasheet-only and genuinely open: the bay fit (TME publishes no \
                       dimensions for cells) and the per-model UN 38.3 / IEC 62133-2 reports.",
            },
            Candidate {
                mpn: "AKY0107",
                preference: Preference::Alternate,
                note: "1350 mAh, 2873 in stock across TWO distributors (Digi-Key + TME), MOQ 1 — \
                       ranks 1st of the 31 qualifying cells, and the only one that satisfies the \
                       two-source AVL rule. It is the ALTERNATE and not the pick for one \
                       mechanical reason: LP503759 encodes 37 mm width against a 35 mm bay. \
                       \
                       That makes this a cheap, concrete trade rather than a dead end — widen \
                       the bay by 2 mm and this becomes the better part on every axis that \
                       remains (two distributors, +250 mAh, 1.2 mm thinner). Recorded so the \
                       mechanical design can price the decision instead of rediscovering it. \
                       Recorded as a model-level fallback, NOT as manufacturer independence: \
                       see the requirement's single-maker finding. \
                       \
                       SINGLE-MANUFACTURER FINDING, measured 2026-11-24: all 70 Li-Po cells TME \
                       carries are AKYGA BATTERY, and so are all 31 that pass the constraints. \
                       TME therefore satisfies 'a spare is buyable' (MOQ 1, thousands in stock) \
                       but NOT 'from more than one manufacturer' — the property that survives a \
                       vendor leaving the market, and the one that matters most for the single \
                       component an END USER re-buys. Closing it needs a second distributor, not \
                       a second Akyga model. \
                       \
                       PARTIAL PROGRESS, 2026-11-24: a Digi-Key sweep found Jauch Quartz cells — \
                       a genuine second manufacturer, with part numbers that state their own \
                       construction (LP503759JU+PCM+2 WIRES 70MM). But neither FITS: that one is \
                       62.0 x 38.1 x 6.0 mm against a 35 x 59 x 7.1 mm bay, and the other Jauch \
                       cell is 330 mAh. So independence remains open, and it is now a MECHANICAL \
                       question rather than a sourcing one — either a cell of the right size \
                       appears, or the bay changes, or the single-manufacturer risk is accepted \
                       explicitly. Digi-Key stocks few bare pouch cells generally; Farnell or \
                       buying direct is the likelier route.",
            },
        ],
    },
    PartReq {
        id: "bluetooth",
        what: "pre-qualified BT module (inheritable QDID + EU radio cert), SBC, no vendor NDA — \
               DO NOT POPULATE on the shipped wired-only board",
        constraints: BLUETOOTH_CONSTRAINTS,
        candidates: &[
            // DELIBERATELY UNPICKED, 2026-11-24, while every other requirement
            // got a Preferred part.
            //
            // The catalogue half of this requirement ranks 324 qualifying
            // modules and the top of that list is the wrong KIND of part, which
            // is the clearest case in this spec of ranking not being deciding:
            //
            //   * The ESP32-WROOM / WROVER / S3-MINI modules that lead on
            //     sourcing are WiFi+BT combos. Populating one changes the radio
            //     regime the whole compliance posture rests on — the shipped
            //     board is an EMC device BECAUSE no radio is populated
            //     (`licensing::regime_for`) — and it is a full application SoC
            //     rather than a codec-carrying audio module.
            //   * `RN4871U-V/RM118` is BLE-only. No A2DP, so no SBC, so it
            //     fails `codec_sbc` — a datasheet fact the filter cannot see.
            //
            // All three deciding properties here (inheritable QDID, SBC, no
            // vendor NDA) are `Evidence::Datasheet`, and the part is DO NOT
            // POPULATE on the shipped board, so there is no cost to waiting and
            // a real cost to reserving a footprint for the wrong module.
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
