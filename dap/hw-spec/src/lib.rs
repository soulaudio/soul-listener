//! **The** machine-readable hardware spec for the DAP — one source of truth that
//! the firmware *consumes* and the hardware design is *generated from*.
//!
//! # Why a crate and not a TOML file
//!
//! Because the firmware can `use` it. A checked spec still permits two sources
//! that have to be reconciled; a consumed spec eliminates the drift by
//! construction. `dap-core` and `board-h7a3` read these constants directly, so
//! there is nothing to keep in sync — and the compiler is the first enforcement
//! layer, for free.
//!
//! Checks are only needed where the consumer cannot read Rust: the atopile
//! design and the BOM. For those, the pattern is **generate, commit, and gate
//! that the committed artefact matches a fresh regeneration** — the same idiom
//! soul-audio-core already uses for its generated DSP tables.
//!
//! # SOFT LOCK — read this before treating anything here as decided
//!
//! [`BoardSpec::lock`] is [`LockStatus::Soft`] for every revision in this crate
//! today. That is deliberate and load-bearing:
//!
//! * A part is not a decision, it is a **candidate with a preference**. Every
//!   [`PartReq`] carries several [`Candidate`]s, each tagged
//!   [`Preference::Preferred`], [`Alternate`](Preference::Alternate),
//!   [`Evaluating`](Preference::Evaluating) or
//!   [`Rejected`](Preference::Rejected) with a reason. The invariants assert
//!   that a requirement has a preference *and* a fallback — never that it has
//!   exactly one answer.
//! * Numbers that came from a datasheet are marked as such. Numbers that came
//!   from a measurement cite the measurement. Numbers that are estimates say so,
//!   so nobody mistakes an estimate for a commitment (the mistake
//!   `HOST_TO_TARGET_DERATE` made — see the placement doc's S15/S16).
//! * Hard-locking is a separate, deliberate act: flip [`LockStatus`] when the
//!   board is frozen, and the invariants tighten accordingly.
//!
//! # Two revisions, both first class
//!
//! [`V1`] is the as-built atopile design (STM32H743 + ES9038Q2M + TPA6120A2 +
//! 32 MB SDRAM). [`V2`] is the firmware's target (STM32H7A3, CS43131, no
//! external RAM). They are both here on purpose: the firmware migrated and
//! `hardware/elec/src` has not, so the gap is a typed diff between two values of
//! one type rather than tribal knowledge. See
//! `docs/plans/2026-11-15-v2-migration-design.md`.

#![no_std]
#![forbid(unsafe_code)]

pub mod invariants;
pub mod v1;
pub mod v2;

/// Where a number came from. Attached to anything that could be mistaken for a
/// commitment, because the difference between "the datasheet says" and "we
/// guessed" is the difference between a budget and a wish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Provenance {
    /// A datasheet figure, with the document that states it.
    Datasheet(&'static str),
    /// Measured by a named gate or probe in this repo.
    Measured(&'static str),
    /// An engineering estimate. Must be replaced before a hard lock.
    Estimate(&'static str),
    /// Decided by us (a design choice, not a fact about the world).
    Choice(&'static str),
}

impl Provenance {
    /// Whether this figure is still a guess. A hard lock must have none.
    #[must_use]
    pub const fn is_estimate(&self) -> bool {
        matches!(self, Self::Estimate(_))
    }
}

/// How firmly this revision is committed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockStatus {
    /// Alternatives are still open; estimates are tolerated. Invariants check
    /// *viability*, not finality.
    Soft {
        /// What has to happen before this can harden.
        until: &'static str,
    },
    /// The board is frozen. Invariants tighten: no estimates, one preferred part
    /// per requirement, every part sourced.
    Hard,
}

/// Board revision identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Revision {
    /// The as-built atopile design.
    V1,
    /// The firmware's target.
    V2,
}

/// The MCU, and the facts about it the firmware depends on.
#[derive(Debug, Clone, Copy)]
pub struct Mcu {
    /// Orderable part number.
    pub mpn: &'static str,
    /// The `embassy-stm32` chip feature the firmware must enable. Pinned here so
    /// the board crate cannot select a different chip than the spec describes.
    pub embassy_feature: &'static str,
    /// Maximum core clock at the top voltage scale, MHz.
    pub max_core_mhz: u32,
    pub provenance: Provenance,
}

/// One addressable RAM region, as the linker must see it.
#[derive(Debug, Clone, Copy)]
pub struct MemoryRegion {
    /// Linker-script region name (`RAM`, `AHB_SRAM`, …).
    pub name: &'static str,
    pub origin: u64,
    pub bytes: usize,
    /// Reachable by DMA. A buffer a peripheral writes must live in one of these.
    pub dma_capable: bool,
    /// Linked today. A region that exists on the die but is not in `memory.x` is
    /// invisible to the firmware — which is exactly how 128 KB went unused.
    pub linked: bool,
    /// Why this region is or is not linked.
    pub note: &'static str,
}

/// Who drives the I2S bit/frame clocks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DacRole {
    /// The DAC receives SCLK/LRCK; the MCU's SAI generates them. Needed for
    /// native-rate playback off an external oscillator.
    I2sSlave,
    /// The DAC generates SCLK/LRCK from its own clock.
    I2sMaster,
}

/// How the DAC attenuates. This is the fact the bit-perfect gates reference, and
/// the distinction matters more than it looks — see each variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeMechanism {
    /// Digital attenuation inside the DAC, applied at the modulator's internal
    /// precision. The I2S stream stays bit-exact, which is the meaningful claim,
    /// but the attenuation itself is still digital — NOT lossless.
    DacDigital {
        /// Step size in dB x100 (50 == 0.5 dB).
        step_db_centi: u32,
        /// Minimum attenuation in -dB.
        min_db: u32,
        /// Internal precision the attenuation is applied at, bits.
        internal_bits: u32,
    },
    /// True analog attenuation after conversion. The only strictly bit-perfect
    /// option; no DAC in the candidate set offers it as a separate stage.
    Analog,
    /// Host-side multiply. Lossy: discards resolution before the DAC ever sees
    /// the samples (~5 bits at -30 dB on a 24-bit path). Present as a variant so
    /// a spec that chose it would be *visibly* wrong rather than silently so.
    HostDigital,
}

/// The DAC/amp, and the facts the audio path depends on.
#[derive(Debug, Clone, Copy)]
pub struct Dac {
    pub mpn: &'static str,
    pub role: DacRole,
    /// Accepted MCLK/LRCK ratios. A rate the spec claims to support must land on
    /// one of these — that is `rate_matrix_is_reachable`.
    pub accepted_mclk_ratios: &'static [u16],
    pub volume: VolumeMechanism,
    /// Typical playback current **referred to the battery rail**, mA.
    ///
    /// The rail is in the name because a bare "mA" is meaningless for a part
    /// with internal rails, and getting it wrong is silent. The CS43131's
    /// ~23 mW of playback power is 12.8 mA at its 1.8 V core and ~6-7 mA drawn
    /// from a 3.7 V battery — the same power, a factor of ~1.8 apart. The power
    /// model works battery-referred (`DAC_AMP_PLAYBACK_MA`'s doc: "playback
    /// current at the battery rail"), so this field must too. The first draft of
    /// this spec used the rail-referred figure and the bridge gate in
    /// `dap-arch-tests` caught it.
    pub playback_ma_at_battery: f32,
    /// Integrated headphone amp, or a separate amp part is required.
    pub integrated_amp: bool,
    pub provenance: Provenance,
}

/// A fixed oscillator feeding the SAI's clock input.
#[derive(Debug, Clone, Copy)]
pub struct Oscillator {
    pub hz: u32,
    /// Which sample-rate family it serves (44100 or 48000).
    pub family_hz: u32,
    pub note: &'static str,
}

/// One supported output rate, and exactly how the clock tree reaches it.
///
/// This is the table that makes "native-rate first" checkable instead of
/// aspirational: `osc_hz / mckdiv == ratio * hz`, with `ratio` accepted by the
/// DAC. A rate that cannot satisfy that is not supported, and
/// `rate_matrix_is_reachable` says so with the resolutions spelled out.
#[derive(Debug, Clone, Copy)]
pub struct RateEntry {
    /// Output sample rate, Hz.
    pub hz: u32,
    /// Oscillator driving it.
    pub osc_hz: u32,
    /// SAI master-clock divider applied to the oscillator.
    pub mckdiv: u32,
    /// Resulting MCLK/LRCK ratio.
    pub ratio: u16,
}

impl RateEntry {
    /// The MCLK this entry produces.
    #[must_use]
    pub const fn mclk_hz(&self) -> u32 {
        self.osc_hz / self.mckdiv
    }

    /// Whether the arithmetic closes: dividing the oscillator by `mckdiv` must
    /// give exactly `ratio * hz`, with no remainder anywhere.
    #[must_use]
    pub const fn is_exact(&self) -> bool {
        self.mckdiv != 0
            && self.osc_hz.is_multiple_of(self.mckdiv)
            && self.mclk_hz() == self.ratio as u32 * self.hz
    }
}

/// The battery/power envelope the model must fit.
#[derive(Debug, Clone, Copy)]
pub struct PowerBudget {
    pub battery_mah: u32,
    /// Playback hours the product promises.
    pub target_hours: f32,
    /// Everything except the CPU: DAC+amp, storage, display.
    pub peripheral_playback_ma: f32,
    pub provenance: Provenance,
}

impl PowerBudget {
    /// Total current the target allows.
    #[must_use]
    pub fn budget_ma(&self) -> f32 {
        self.battery_mah as f32 / self.target_hours
    }
}

/// How firmly a candidate part is chosen. The soft lock lives here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preference {
    /// What the design uses today.
    Preferred,
    /// A viable second source. The AVL rule wants at least one.
    Alternate,
    /// Under consideration; not yet viable.
    Evaluating,
    /// Ruled out, with the reason kept so it is not re-litigated.
    Rejected(&'static str),
}

/// One orderable candidate for a requirement.
#[derive(Debug, Clone, Copy)]
pub struct Candidate {
    /// Manufacturer part number, exactly as orderable. The suffix matters:
    /// CS43131-CNZ is stocked in EU, CS43131-CNZR (tape and reel) is not stocked
    /// at Mouser — a distinction invisible in prose and fatal in a BOM.
    pub mpn: &'static str,
    pub preference: Preference,
    pub note: &'static str,
}

/// A functional requirement, with candidates rather than a winner.
///
/// Requirements are what the spec commits to; parts are what satisfies them.
/// Keeping them separate is what makes the lock soft: the rate matrix depends on
/// "a 45.1584 MHz oscillator", not on one manufacturer's part.
#[derive(Debug, Clone, Copy)]
pub struct PartReq {
    /// Stable id, used to join against the sourcing snapshot.
    pub id: &'static str,
    /// What the design needs, in words.
    pub what: &'static str,
    pub candidates: &'static [Candidate],
}

impl PartReq {
    /// The preferred candidate, if one is chosen.
    #[must_use]
    pub fn preferred(&self) -> Option<&Candidate> {
        self.candidates
            .iter()
            .find(|c| c.preference == Preference::Preferred)
    }

    /// Candidates that could be ordered instead — the second-source depth.
    #[must_use]
    pub fn alternates(&self) -> usize {
        self.candidates
            .iter()
            .filter(|c| c.preference == Preference::Alternate)
            .count()
    }
}

/// Everything about one board revision.
#[derive(Debug, Clone, Copy)]
pub struct BoardSpec {
    pub revision: Revision,
    pub lock: LockStatus,
    pub mcu: Mcu,
    pub memory: &'static [MemoryRegion],
    pub dac: Dac,
    pub oscillators: &'static [Oscillator],
    pub rates: &'static [RateEntry],
    pub power: PowerBudget,
    pub parts: &'static [PartReq],
}

impl BoardSpec {
    /// Bytes of DMA-capable RAM the linker can actually address today. The
    /// figure every memory budget must be measured against — and the one that
    /// was wrong by 56 KB until a human read the datasheet.
    #[must_use]
    pub fn linked_dma_bytes(&self) -> usize {
        self.memory
            .iter()
            .filter(|r| r.linked && r.dma_capable)
            .map(|r| r.bytes)
            .sum()
    }

    /// Bytes present on the die and DMA-capable, linked or not.
    #[must_use]
    pub fn total_dma_bytes(&self) -> usize {
        self.memory
            .iter()
            .filter(|r| r.dma_capable)
            .map(|r| r.bytes)
            .sum()
    }

    /// Whether this revision is still open to change.
    #[must_use]
    pub const fn is_soft(&self) -> bool {
        matches!(self.lock, LockStatus::Soft { .. })
    }
}

/// The revision the firmware builds against. **Soft-locked to V2.**
///
/// Soft means: the firmware targets V2 and the invariants hold for V2, but V1
/// remains a first-class value and nothing here forbids revisiting a part, a
/// rate or an oscillator. Hardening is a deliberate edit of
/// [`v2::V2`]'s [`LockStatus`].
pub const ACTIVE: &BoardSpec = &v2::V2;
