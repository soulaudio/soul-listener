//! Memory budget lines and conditional risks — the part of the design that is
//! *decided* rather than measured.
//!
//! # Why this exists
//!
//! "255 KB spare" was a true number and a misleading one. It was spare because
//! the framebuffer, the DMA rings and the listing cache are not built yet, so
//! the headroom was really a forecast overrun wearing a surplus costume. Saying
//! that in prose (placement doc S2, S13) did not stop it being quoted as
//! headroom, because prose is not subtractable.
//!
//! So the lines become data, each with:
//!
//! * a **size** and the **region** it must live in, so a total can be computed
//!   against something real rather than against `USER_SRAM`;
//! * a **status** — [`Built`](LineStatus::Built) and measured, [`Planned`] and
//!   committed, or [`Discretionary`] and droppable with its tradeoff recorded;
//! * **provenance**, so a measured 164,752 B is never confused with a forecast
//!   96,000 B.
//!
//! That makes two questions answerable by arithmetic instead of argument: does
//! the committed design fit, and if not, is there enough discretionary weight to
//! drop to make it fit.
//!
//! # Conditional risks
//!
//! The other half of the same problem. "Caches are off, so there is no MPU or
//! DMA-alignment discipline" is not a bug today and becomes a
//! silent-corruption class of bug the moment someone enables caches for speed —
//! which the RTF work made *tempting*, since the cache question is worth ~2x.
//! A risk like that has a trigger, a consequence and a mitigation, and nothing
//! enforces it while it sits in a document.
//!
//! [`ConditionalRisk`] records the trigger; `dap-arch-tests`' risk gate scans the
//! firmware for the trigger and demands the mitigation when it fires. The risk
//! arms itself.

use crate::Provenance;

/// How committed a budget line is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineStatus {
    /// Implemented, and the size is a measurement of real code.
    Built,
    /// Committed to but not implemented. Counts against the budget in full,
    /// because a plan that does not fit is not a plan.
    Planned,
    /// Optional. Counts only while `enabled`, and carries what is given up by
    /// dropping it — the relief valve, with its price written down.
    Discretionary {
        /// What is lost if this line is dropped.
        tradeoff: &'static str,
        /// Whether the design currently intends to include it.
        enabled: bool,
    },
}

impl LineStatus {
    /// Whether this line consumes bytes as the design stands.
    #[must_use]
    pub const fn counts(&self) -> bool {
        match self {
            Self::Built | Self::Planned => true,
            Self::Discretionary { enabled, .. } => *enabled,
        }
    }

    /// Whether this line could be dropped to recover its bytes.
    #[must_use]
    pub const fn is_droppable(&self) -> bool {
        matches!(self, Self::Discretionary { enabled: true, .. })
    }
}

/// One claim on a memory region.
#[derive(Debug, Clone, Copy)]
pub struct BudgetLine {
    pub id: &'static str,
    pub what: &'static str,
    pub bytes: usize,
    /// The [`crate::MemoryRegion`] name this must live in. A line with nowhere
    /// to live is the failure the AHB-SRAM linking existed to prevent.
    pub region: &'static str,
    pub status: LineStatus,
    pub provenance: Provenance,
}

/// A risk that is dormant until something in the design turns it on.
///
/// The fields are deliberately about *detection*: `trigger_symbols` is what a
/// gate greps for, so arming is observed rather than remembered.
#[derive(Debug, Clone, Copy)]
pub struct ConditionalRisk {
    pub id: &'static str,
    /// The condition in words.
    pub condition: &'static str,
    /// What goes wrong when the condition holds and the mitigation is absent.
    pub consequence: &'static str,
    /// Source symbols whose presence means the condition now holds.
    pub trigger_symbols: &'static [&'static str],
    /// Source symbols whose presence means the mitigation is in place.
    pub mitigation_symbols: &'static [&'static str],
    /// What to do, for the failure message.
    pub mitigation: &'static str,
}

/// V2's memory claims.
///
/// `Built` sizes are measurements from `dap-arch-tests`' roster; the rest are the
/// placement doc's forecast, and the difference is in the provenance rather than
/// in a footnote.
pub const V2_LINES: &[BudgetLine] = &[
    BudgetLine {
        id: "app",
        what: "App by value: decoder slot + resampler + output ring, PRELOAD=0",
        bytes: 164_752,
        region: "RAM",
        status: LineStatus::Built,
        provenance: Provenance::Measured("dap-arch-tests component_footprint_roster"),
    },
    BudgetLine {
        id: "decoder-scratch",
        what: "the externalized decoder scratch pool the decoders carve from",
        bytes: 262_144,
        region: "RAM",
        status: LineStatus::Built,
        provenance: Provenance::Measured("soul_audio_core::decoder_scratch_len"),
    },
    BudgetLine {
        id: "stack-reserve",
        what: "stack headroom kept clear above the statics",
        bytes: 352 * 1024,
        region: "RAM",
        status: LineStatus::Built,
        provenance: Provenance::Measured(
            "check-tick-qemu.sh, 308 KiB measured under the pinned compiler",
        ),
    },
    BudgetLine {
        id: "framebuffer-live",
        what: "e-ink live framebuffer, 800x480 Gray4 packed 2bpp",
        bytes: 800 * 480 * 2 / 8,
        // Deliberately AHB: it is the one buffer large enough to justify the
        // region, and it is why AHB-SRAM1/2 were linked as one 128 KB span.
        region: "AHB_SRAM",
        status: LineStatus::Planned,
        provenance: Provenance::Choice("800x480 panel at Gray4 is arithmetic, not an estimate"),
    },
    BudgetLine {
        id: "framebuffer-shadow",
        what: "second framebuffer for partial-refresh dirty-diffing",
        bytes: 800 * 480 * 2 / 8,
        region: "AHB_SRAM",
        status: LineStatus::Discretionary {
            tradeoff: "DISABLED BECAUSE IT DOES NOT FIT, which the budget gate established rather \
                       than anyone deciding. A second 96,000 B buffer is short by 60,928 B in \
                       AHB (35,072 left after the live buffer) and by 31,376 B in AXI (64,624 \
                       left after everything else). There is no region for it. What it costs to \
                       be without it: partial refresh cannot diff against the previous frame, so \
                       either every update is a full 3000 ms flash — which the refresh policy \
                       exists to avoid — or the dirty region is tracked another way, e.g. by \
                       widget-level invalidation rather than a pixel diff, which DirtyTracker \
                       already does at a coarser grain. To ENABLE it, 96,000 B has to come from \
                       somewhere, and only two levers are big enough, together and only just: \
                       shrink the decoder scratch 256 -> 192 KiB (frees 65,536; the placement \
                       doc's backlog item 1, and Rockbox's proven floor of ~125 kB says there is \
                       room) AND drop the stack reserve 352 -> 320 KiB (frees 32,768; the \
                       measured tick is 308 KiB, so the margin is 12 KiB rather than 44). That \
                       is 98,304 B for a 96,000 B buffer — a 2,304 B margin, which is not a \
                       plan, it is a dare.",
            enabled: false,
        },
        provenance: Provenance::Choice("same geometry as the live buffer"),
    },
    BudgetLine {
        id: "eink-scratch",
        what: "eink-ui glyph/layout/compose scratch",
        bytes: 32 * 1024,
        region: "RAM",
        status: LineStatus::Planned,
        provenance: Provenance::Estimate("forecast; never measured against the real widget set"),
    },
    BudgetLine {
        id: "library-listing",
        what: "browse page + order table + string arena",
        bytes: 64 * 1024,
        region: "RAM",
        status: LineStatus::Planned,
        provenance: Provenance::Estimate("forecast"),
    },
    BudgetLine {
        id: "catalog-index",
        what: "soulidx IndexReader + scan scratch",
        bytes: 32 * 1024,
        region: "RAM",
        status: LineStatus::Planned,
        provenance: Provenance::Estimate("forecast; the reader itself measures 96 B today"),
    },
    BudgetLine {
        id: "dma-staging",
        what: "SAI/SDMMC DMA descriptors and staging buffers",
        bytes: 64 * 1024,
        // AXI, not AHB — forced by arithmetic, not preference. AHB is 131,072 B
        // and the live framebuffer alone is 96,000, leaving 35,072: a 65,536 B
        // DMA region does not fit beside it. AHB keeping DMA away from the span
        // the stack grows into would have been nicer; it is not available.
        region: "RAM",
        status: LineStatus::Planned,
        provenance: Provenance::Estimate(
            "forecast size; the REGION is arithmetic — see the comment above.",
        ),
    },
];

/// V2's conditional risks.
pub const V2_RISKS: &[ConditionalRisk] = &[ConditionalRisk {
    id: "cache-dma-coherency",
    condition: "the D-cache is enabled",
    consequence:
        "DMA writes and CPU reads see different memory. The audio path is the worst place \
             for it: a stale cache line in an SAI buffer is silent corruption that sounds like a \
             glitch, not like a bug, and it is intermittent. Teensy requires the same 32-byte \
             alignment independently, so this is cross-vendor practice rather than an ST quirk \
             (AN4839).",
    // Enabling either cache is the trigger. The I-cache alone is safe for
    // coherency (instruction fetch is read-only) but is listed because the
    // project's own rule is "caches on WITH the MPU work, or off and
    // documented — never partially".
    trigger_symbols: &["enable_dcache", "enable_icache"],
    mitigation_symbols: &["MPU", "mpu", "non_cacheable", "noncacheable"],
    mitigation:
        "configure an MPU non-cacheable region covering every DMA buffer and 32-byte-align \
             those buffers, then re-measure the stack probes (the cache changes codegen, and the \
             budgets are pinned to a measurement).",
}];
