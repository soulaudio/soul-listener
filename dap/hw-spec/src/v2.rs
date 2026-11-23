//! **V2** — the firmware's target board. Soft-locked.
//!
//! STM32H7A3 + CS43131 (integrated amp, I2S slave) + external dual MCLK
//! oscillators + no external RAM. Per
//! `docs/plans/2026-11-15-v2-migration-design.md`; `hardware/elec/src` is still
//! [`crate::v1`] and has not migrated.

use crate::select::Constraint;
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
            value: 22_579_200.0,
            rel_tol: 1e-6,
        },
        why: "22.5792 MHz is simultaneously a nominal CS43131 MCLK_INT (so it feeds XTI with no \
              divider) and 512 x 44100, which divides by an integer to an exact 64 x Fs bit clock \
              at 44.1, 88.2 and 176.4 kHz. 24.576 MHz satisfies the DAC equally well and cannot \
              produce this family exactly, which is why the frequency is pinned rather than left \
              as 'either nominal MCLK'",
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
            value: 24_576_000.0,
            rel_tol: 1e-6,
        },
        why: "the other nominal CS43131 MCLK_INT, and 512 x 48000 — exact for 48, 96 and 192 kHz",
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
        pred: crate::select::Predicate::AtLeast(24_576_000.0),
        why: "it has to pass the faster of the two oscillators",
    },
    Constraint {
        attr: "supply_mv_max",
        pred: crate::select::Predicate::AtMost(3300.0),
        why: "the board has no rail above 3.3 V",
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
        attr: "integrated_pcm",
        pred: crate::select::Predicate::IsTrue,
        why: "the cell must arrive as a finished, pre-certified assembly with its own protection \
              circuit. Buying bare cells and adding our own PCM makes US the battery \
              manufacturer, inheriting UN 38.3 and IEC 62133-2 testing — a five-figure \
              certification programme acquired by a sourcing decision",
    },
    Constraint {
        attr: "un38_3_report",
        pred: crate::select::Predicate::IsTrue,
        why: "UN 38.3 transport testing must exist FOR THIS MODEL and be obtainable from the \
              vendor. Certification is per model, so a vendor's general compliance claim is not \
              the document that ships with the product",
    },
    Constraint {
        attr: "iec62133_2_report",
        pred: crate::select::Predicate::IsTrue,
        why: "IEC 62133-2 cell safety report, per model, from the vendor — the other half of the \
              archive that lets certification sit with them rather than us",
    },
    Constraint {
        attr: "termination",
        pred: crate::select::Predicate::OneOf(USER_SWAPPABLE_TERMINATIONS),
        why: "a connectorised cell is what makes a spare battery usable: the user unplugs one and \
              plugs in another. Solder tabs would satisfy nothing — not Art. 11 \
              user-replaceability, and certainly not carrying a charged spare",
    },
    Constraint {
        attr: "cells_series",
        pred: crate::select::Predicate::NumEq {
            value: 1.0,
            rel_tol: 0.0,
        },
        why: "1S. The charger, the fuel gauge and every rail in the power model assume a single \
              3.7 V nominal cell; a 2S pack is a different power tree, not a bigger battery",
    },
    Constraint {
        attr: "capacity_mah",
        pred: crate::select::Predicate::AtLeast(1000.0),
        why: "below ~1000 mAh the runtime drops out of the compact-DAP class the product is \
              measured against (Shanling M0 Pro ~14.5 h, FiiO M6 ~12 h)",
    },
    Constraint {
        attr: "watt_hours",
        pred: crate::select::Predicate::AtMost(100.0),
        why: "above 100 Wh a lithium battery installed in equipment leaves UN3481 PI967 Section \
              II and needs Class 9 dangerous-goods shipping. The design is ~4 Wh, so this is a \
              guard against a future cell choice quietly changing how the product can be posted",
    },
    Constraint {
        attr: "thickness_mm",
        pred: crate::select::Predicate::AtMost(7.1),
        why: "the battery bay is 35 x 59 x 7.1 mm",
    },
    Constraint {
        attr: "width_mm",
        pred: crate::select::Predicate::AtMost(35.0),
        why: "battery bay width",
    },
    Constraint {
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
        attr: "is_module",
        pred: crate::select::Predicate::IsTrue,
        why: "a module, never a bare chip. The module vendor has already run the radio through \
              qualification and certification; starting from a chip means running both ourselves",
    },
    Constraint {
        attr: "bt_sig_qualified",
        pred: crate::select::Predicate::IsTrue,
        why: "the module must carry its own Bluetooth SIG qualification (a QDID we can reference \
              in an end-product Declaration). Without one, using Bluetooth means a full \
              qualification programme rather than a ~$5k Declaration — this is exactly how \
              Tangara inherits the ESP32's QDID",
    },
    Constraint {
        attr: "radio_certified_eu",
        pred: crate::select::Predicate::IsTrue,
        why: "pre-certified for RED/CE, so populating the radio does not require a radiated \
              emissions programme on our own board. Note this is not a full exemption: the end \
              product still needs its own EMC test (Tangara needed one despite a pre-certified \
              module)",
    },
    Constraint {
        attr: "requires_vendor_nda",
        pred: crate::select::Predicate::IsFalse,
        why: "an NDA-gated SDK is incompatible with shipping open-source firmware: the Qualcomm \
              QCC route needs an NDA and ADK access. A pre-made module encapsulates the \
              NDA-bound firmware behind a documented host interface, which is why this is a hard \
              filter rather than an inconvenience",
    },
    Constraint {
        attr: "codec_sbc",
        pred: crate::select::Predicate::IsTrue,
        why: "SBC is the mandatory royalty-free codec and the only one the product commits to. \
              aptX is licensed through the module vendor and LDAC source has no off-the-shelf \
              module, so neither is required — a module that offers ONLY licensed codecs would \
              make the audio path depend on a fee",
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
        what: "22.5792 MHz fixed oscillator, <=3.3 V, low phase noise, SMD",
        constraints: OSC_44K1_CONSTRAINTS,
        candidates: &[Candidate {
            mpn: "ASDMB-22.5792MHZ-LY-T",
            preference: Preference::Evaluating,
            note: "placeholder: the frequency is the requirement, the part is not chosen. \
                       Needs a phase-noise figure against DS1155F2 Fig. 4-14 and an EU stock \
                       check. Note this is a far more common frequency than the 45.1584 MHz the \
                       spec previously asked for, so the candidate set should be wide.",
        }],
    },
    PartReq {
        id: "mclk-osc-48k",
        what: "24.576 MHz fixed oscillator, <=3.3 V, low phase noise, SMD",
        constraints: OSC_48K_CONSTRAINTS,
        candidates: &[Candidate {
            mpn: "ASDMB-24.576MHZ-LY-T",
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
