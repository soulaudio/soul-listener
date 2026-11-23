//! **V1** — the as-built atopile design in `hardware/elec/src`.
//!
//! Present so the migration gap is a typed diff rather than tribal knowledge.
//! The firmware targets [`crate::v2`]; this revision is what `hardware/` still
//! describes, and `migration_deltas_are_tracked` enumerates the difference.
//!
//! Only the fields the migration actually turns on are filled in with care — the
//! point is the diff, not a complete second design. Where V1 detail is not
//! needed to express a delta, it is recorded loosely and marked as such.

use crate::{
    BoardSpec, Candidate, Dac, DacRole, LockStatus, Mcu, MemoryRegion, Oscillator, PartReq,
    PowerBudget, Preference, Provenance, RateEntry, Revision, VolumeMechanism,
};

const LOCK: LockStatus = LockStatus::Soft {
    until: "superseded — this revision exists to express the migration delta, not to be built",
};

const MCU: Mcu = Mcu {
    mpn: "STM32H743ZIT6",
    embassy_feature: "stm32h743zi",
    max_core_mhz: 480,
    provenance: Provenance::Datasheet("ST DS12930"),
};

/// V1's defining memory difference: 32 MB of external SDRAM on the FMC.
///
/// This is why the revision matters to the firmware rather than only to the
/// board. Every memory conclusion in the placement doc — the 124 KB Phase E
/// overrun, the AHB-SRAM linking, the stack reserve against a 1 MB region —
/// is a statement about V2. On V1 the framebuffer could simply live in SDRAM.
const MEMORY: &[MemoryRegion] = &[
    MemoryRegion {
        name: "RAM",
        origin: 0x2400_0000,
        bytes: 512 * 1024,
        dma_capable: true,
        linked: true,
        note: "H743 AXI-SRAM (smaller than H7A3's)",
    },
    MemoryRegion {
        name: "SDRAM",
        origin: 0xC000_0000,
        bytes: 32 * 1024 * 1024,
        dma_capable: true,
        linked: true,
        note: "W9825G6KH-6 over FMC, 16-bit bus. Removed in V2: board area, power, and \
               refresh contention in the audio path.",
    },
];

const DAC: Dac = Dac {
    mpn: "ES9038Q2M",
    // V1 clocks the DAC from the MCU as SAI master — the arrangement that makes
    // native-rate switching awkward and is the reason V2 inverts it.
    role: DacRole::I2sMaster,
    accepted_mclk_ratios: &[512, 384, 256, 128],
    volume: VolumeMechanism::DacDigital {
        step_db_centi: 50,
        min_db: 127,
        internal_bits: 32,
    },
    // Order-of-magnitude figure: ES9038Q2M has no integrated amp, so the real
    // audio-section draw includes the TPA6120A2, a desktop-class part. Marked an
    // estimate deliberately — the power model was calibrated for V2's CS43131,
    // and that mismatch is one of the tracked deltas.
    // Battery-referred, and an order-of-magnitude guess rather than a figure.
    playback_ma_at_battery: 120.0,
    integrated_amp: false,
    provenance: Provenance::Estimate(
        "ES9038Q2M + TPA6120A2 audio section; never measured, and far above the model's \
         CS43131-calibrated figure",
    ),
};

/// V1 has no oscillator pair — the MCU is the clock master.
const OSCILLATORS: &[Oscillator] = &[];

/// Left empty on purpose: V1's rate handling is SAI-master with resampling, so
/// there is no oscillator/MCKDIV table to express. `rate_matrix_is_reachable`
/// skips revisions with no declared rates rather than inventing a V1 table.
const RATES: &[RateEntry] = &[];

const POWER: PowerBudget = PowerBudget {
    battery_mah: 2000,
    target_hours: 10.0,
    peripheral_playback_ma: 130.0,
    provenance: Provenance::Estimate("v1-era figures; superseded by the v2 power model"),
};

const PARTS: &[PartReq] = &[
    PartReq {
        id: "dac",
        what: "stereo DAC (amp separate)",
        candidates: &[Candidate {
            mpn: "ES9038Q2M",
            preference: Preference::Preferred,
            note: "as-built; Digi-Key EU was chosen as the source for ESS parts",
        }],
    },
    PartReq {
        id: "amp",
        what: "headphone amp — required because the V1 DAC has none",
        candidates: &[Candidate {
            mpn: "TPA6120A2",
            preference: Preference::Preferred,
            note:
                "desktop-class current-feedback amp; disappears in V2 with the integrated CS43131",
        }],
    },
    PartReq {
        id: "mcu",
        what: "Cortex-M7 with FMC for external SDRAM",
        candidates: &[Candidate {
            mpn: "STM32H743ZIT6",
            preference: Preference::Preferred,
            note: "as-built",
        }],
    },
    PartReq {
        id: "sdram",
        what: "32 MB SDRAM on FMC",
        candidates: &[Candidate {
            mpn: "W9825G6KH-6",
            preference: Preference::Preferred,
            note: "deleted in V2",
        }],
    },
];

/// The V1 board spec — as-built, superseded.
pub const V1: BoardSpec = BoardSpec {
    revision: Revision::V1,
    lock: LOCK,
    mcu: MCU,
    memory: MEMORY,
    dac: DAC,
    oscillators: OSCILLATORS,
    rates: RATES,
    power: POWER,
    parts: PARTS,
};
