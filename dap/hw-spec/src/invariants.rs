//! The constraints — what makes the spec *enforced* rather than merely written
//! down.
//!
//! Each test states a property the design must satisfy, and the ones that can
//! fail are written to **enumerate the resolutions** rather than just report a
//! mismatch. A gate that says "mismatch" is a nag; a gate that says "resolve by
//! (a), (b) or (c)" forces a decision.
//!
//! These run on the host (`cargo test -p dap-hw-spec`), so they cost nothing and
//! gate every build.

#![cfg(test)]

use crate::{
    v1::V1, v2::V2, LockStatus, MclkContract, Preference, Provenance, Revision, VolumeMechanism,
};

/// Every rate the spec claims must be reachable with the declared clock tree.
///
/// Two things must hold, and they are *different* checks:
///
/// 1. **The SAI arithmetic closes exactly** — `osc_hz / mckdiv == frame_bits *
///    hz`, no remainder. This is what makes LRCK exactly Fs, and it is the
///    constraint that forces one oscillator per sample-rate family.
/// 2. **The DAC's own MCLK contract is satisfied** — which depends on which
///    contract it declares. See [`MclkContract`]: an Fs-multiple part must land
///    on an accepted ratio, a fixed-system-clock part must be fed one of its
///    nominal frequencies.
///
/// Keeping these separate is the lesson from getting it wrong. An earlier version
/// of this test checked only (2), in the Fs-multiple form, for every DAC — and
/// so "proved" that 176.4 kHz was unreachable on a 22.5792 MHz oscillator
/// because it lands on 128fs. The arithmetic was right and the premise was
/// false: the CS43131 does not constrain that ratio. The test passed, the
/// conclusion was wrong, and the cost was a 1024xFs oscillator pair chosen to
/// clear a limit that does not exist.
#[test]
fn rate_matrix_is_reachable_with_the_declared_oscillators() {
    for spec in [&V2, &V1] {
        if spec.rates.is_empty() {
            // V1 is SAI-master with resampling; there is no clock table to check
            // and inventing one would be fiction.
            continue;
        }
        for r in spec.rates {
            assert!(
                spec.oscillators.iter().any(|o| o.hz == r.osc_hz),
                "{:?}: {} Hz names oscillator {} Hz, which the spec does not declare",
                spec.revision,
                r.hz,
                r.osc_hz
            );
            // (1) The SAI side: the bit clock must come out exact.
            assert!(
                r.is_exact(),
                "{:?}: {} Hz does not close exactly — {} Hz / {} = {} Hz, but {} x {} = {} Hz. \
                 LRCK is derived from that bit clock, so a tree that does not divide exactly \
                 plays at the wrong rate.\n\
                 Resolve by one of:\n  \
                 (a) pick an mckdiv that divides exactly;\n  \
                 (b) add an oscillator for this rate's family;\n  \
                 (c) drop {} Hz from the supported matrix.",
                spec.revision,
                r.hz,
                r.osc_hz,
                r.mckdiv,
                r.sck_hz(),
                r.frame_bits,
                r.hz,
                u32::from(r.frame_bits) * r.hz,
                r.hz
            );

            // (2) The DAC side, by whichever contract it declares.
            match spec.dac.mclk {
                MclkContract::FixedSystemClock(nominal) => {
                    // The oscillator feeds XTI/MCLK directly, so the oscillator
                    // itself — not the divided bit clock — must be nominal.
                    assert!(
                        nominal.contains(&r.osc_hz),
                        "{:?}: {} Hz is clocked from a {} Hz oscillator, but {} requires its MCLK \
                         to be one of {:?} Hz exactly (it is a fixed system clock, not an Fs \
                         multiple). The oscillator is wired to XTI/MCLK with no divider, so this \
                         is a hard mismatch.\n\
                         Resolve by one of:\n  \
                         (a) use a nominal frequency as the oscillator;\n  \
                         (b) put a divider or PLL between oscillator and DAC — which reintroduces \
                         the jitter the fixed-oscillator design exists to avoid;\n  \
                         (c) select a DAC whose contract this tree satisfies.",
                        spec.revision,
                        r.hz,
                        r.osc_hz,
                        spec.dac.mpn,
                        nominal
                    );
                }
                MclkContract::FsMultiple(ratios) => {
                    // Here the MCLK the DAC sees scales with Fs, so the ratio is
                    // the quantity under test.
                    let ratio = r.sck_hz() / r.hz;
                    let ratio = u16::try_from(ratio).unwrap_or(u16::MAX);
                    assert!(
                        ratios.contains(&ratio),
                        "{:?}: {} Hz lands on MCLK/LRCK = {}fs, which {} does not accept (accepts \
                         {:?}).\n\
                         Resolve by one of:\n  \
                         (a) change the oscillator so the divider reaches an accepted ratio;\n  \
                         (b) drop {} Hz from the supported matrix;\n  \
                         (c) if a datasheet read shows {} does accept {}fs, add it — but check \
                         FIRST that this part really constrains the ratio at all, because the \
                         CS43131 does not and assuming it did cost a wrong oscillator pair.",
                        spec.revision,
                        r.hz,
                        ratio,
                        spec.dac.mpn,
                        ratios,
                        r.hz,
                        spec.dac.mpn,
                        ratio
                    );
                }
            }
        }
    }
}

/// Under a fixed-system-clock contract, every declared oscillator must be one the
/// DAC accepts.
///
/// [`rate_matrix_is_reachable_with_the_declared_oscillators`] only covers
/// oscillators some rate names. This covers the rest: an oscillator in the BOM
/// that no rate uses is either dead weight on the board or a rate matrix someone
/// forgot to extend, and either way it should not be a frequency the DAC cannot
/// take.
#[test]
fn every_declared_oscillator_is_one_the_dac_can_be_fed() {
    for spec in [&V2, &V1] {
        let MclkContract::FixedSystemClock(nominal) = spec.dac.mclk else {
            // Under an Fs-multiple contract the oscillator is divided before it
            // reaches the DAC, so there is nothing to check here.
            continue;
        };
        for o in spec.oscillators {
            assert!(
                nominal.contains(&o.hz),
                "{:?}: oscillator {} Hz ({}) is not one of {}'s nominal MCLK frequencies {:?}. \
                 Every oscillator on this board feeds XTI/MCLK through the mux, so an \
                 unacceptable one cannot be on the board at all — not even for a rate family \
                 that is currently unused.",
                spec.revision,
                o.hz,
                o.note,
                spec.dac.mpn,
                nominal
            );
            assert!(
                spec.rates.iter().any(|r| r.osc_hz == o.hz),
                "{:?}: oscillator {} Hz is declared but no rate uses it. Either add the rates it \
                 serves or remove the part.",
                spec.revision,
                o.hz
            );
            // Two oscillators at the same frequency are two parts doing one
            // part's job: extra cost, extra board area, a second phase-noise
            // contributor and a mux input wasted. Caught here because the
            // "declared but unused" check above cannot see it — a duplicate of a
            // used frequency is, trivially, used.
            assert_eq!(
                spec.oscillators.iter().filter(|x| x.hz == o.hz).count(),
                1,
                "{:?}: {} Hz is declared more than once. One oscillator per frequency; if two \
                 rate families need the same clock they share the same part.",
                spec.revision,
                o.hz
            );
        }
    }
}

/// Native-rate playback must cover both sample-rate families, or the dual
/// oscillator earns nothing.
#[test]
fn both_rate_families_are_served() {
    let spec = &V2;
    for family in [44_100u32, 48_000] {
        assert!(
            spec.rates.iter().any(|r| r.hz % family == 0),
            "no rate in the matrix belongs to the {family} Hz family — a dual-oscillator design \
             that cannot play one family is paying for hardware it does not use"
        );
        assert!(
            spec.oscillators.iter().any(|o| o.family_hz == family),
            "no oscillator declared for the {family} Hz family"
        );
    }
}

/// Memory regions must not overlap, and the linked DMA pool is the only figure a
/// budget may be measured against.
///
/// Generalises `dap-arch-tests`' `linker_script_regions_match_the_rust_memory_map`:
/// that gate compares `memory.x` to Rust constants, this one makes the spec the
/// thing both are derived from.
#[test]
fn memory_regions_are_disjoint_and_the_linked_pool_is_known() {
    for spec in [&V2, &V1] {
        for (i, a) in spec.memory.iter().enumerate() {
            assert!(
                a.bytes > 0,
                "{:?}: region {} has zero size",
                spec.revision,
                a.name
            );
            for b in spec.memory.iter().skip(i + 1) {
                let a_end = a.origin + a.bytes as u64;
                let b_end = b.origin + b.bytes as u64;
                assert!(
                    a_end <= b.origin || b_end <= a.origin,
                    "{:?}: regions {} [{:#x}..{:#x}) and {} [{:#x}..{:#x}) overlap — a static \
                     placed in one would alias the other",
                    spec.revision,
                    a.name,
                    a.origin,
                    a_end,
                    b.name,
                    b.origin,
                    b_end
                );
            }
        }
        assert!(
            spec.linked_dma_bytes() > 0,
            "{:?}: no linked DMA-capable region — the firmware would have nowhere to put a \
             buffer a peripheral writes",
            spec.revision
        );
        assert!(
            spec.linked_dma_bytes() <= spec.total_dma_bytes(),
            "{:?}: linked DMA RAM exceeds what the die has",
            spec.revision
        );
    }
}

/// The peripheral playback budget must leave room for the CPU.
///
/// This is the constraint that would have caught the V1/V2 power mismatch:
/// the model is calibrated for a CS43131 audio section, and V1's
/// ES9038Q2M + TPA6120A2 draws roughly ten times as much.
#[test]
fn the_peripheral_budget_leaves_room_for_the_cpu() {
    let spec = &V2;
    let budget = spec.power.budget_ma();
    assert!(
        spec.power.peripheral_playback_ma < budget,
        "{:?}: peripherals alone draw {:.1} mA against a {:.1} mA budget ({} mAh / {} h) — the \
         CPU has not even been counted yet",
        spec.revision,
        spec.power.peripheral_playback_ma,
        budget,
        spec.power.battery_mah,
        spec.power.target_hours
    );
    // And leave a real share for the CPU, not a rounding error.
    let cpu_share = budget - spec.power.peripheral_playback_ma;
    assert!(
        cpu_share > spec.power.peripheral_playback_ma * 0.5,
        "{:?}: only {cpu_share:.1} mA left for the CPU after {:.1} mA of peripherals — either \
         the target hours are optimistic or a peripheral figure is wrong",
        spec.revision,
        spec.power.peripheral_playback_ma
    );
}

/// Volume must not be applied in the host's digital path.
///
/// Attenuating then truncating on the host discards resolution (~5 bits at
/// -30 dB on a 24-bit path). Keeping the mechanism in the spec means the
/// bit-perfect gates reference data instead of a comment, and a spec that chose
/// the lossy option would be visibly wrong.
#[test]
fn volume_is_not_in_the_host_digital_path() {
    for spec in [&V2, &V1] {
        assert!(
            !matches!(spec.dac.volume, VolumeMechanism::HostDigital),
            "{:?}: volume is declared HostDigital, which throws away resolution before the DAC \
             sees the samples. Route it to the DAC's own registers, or to an analog stage.",
            spec.revision
        );
        if let VolumeMechanism::DacDigital { internal_bits, .. } = spec.dac.volume {
            assert!(
                internal_bits >= 24,
                "{:?}: in-DAC attenuation at {internal_bits} bits is not enough headroom above a \
                 24-bit path",
                spec.revision
            );
        }
    }
}

/// Native-rate playback needs the DAC to be the I2S slave.
///
/// If the DAC drives the clocks, changing rate means reconfiguring the DAC
/// rather than the clock tree, and the resampler stops being avoidable — which
/// is 22x the decode cost (placement doc S16).
#[test]
fn the_active_revision_can_do_native_rate() {
    assert_eq!(
        crate::ACTIVE.dac.role,
        crate::DacRole::I2sSlave,
        "the active revision's DAC must be the I2S slave for native-rate playback; as master, \
         the resampler becomes structural and costs ~19.1M instructions per second of audio"
    );
}

/// Every requirement must have a preference and a fallback — the AVL rule.
///
/// Industry practice is 2-3 second sources; the constraint here is deliberately
/// weaker while the lock is soft (a requirement may still be `Evaluating` with
/// no preferred part), and tightens when the revision hardens.
#[test]
fn requirements_have_a_preference_and_a_second_source() {
    let spec = crate::ACTIVE;
    for req in spec.parts {
        assert!(
            !req.candidates.is_empty(),
            "requirement '{}' has no candidates at all",
            req.id
        );
        let viable = req
            .candidates
            .iter()
            .filter(|c| {
                matches!(
                    c.preference,
                    Preference::Preferred | Preference::Alternate | Preference::Evaluating
                )
            })
            .count();
        assert!(
            viable > 0,
            "requirement '{}' has only rejected candidates — the design cannot be built",
            req.id
        );

        match spec.lock {
            LockStatus::Soft { .. } => {
                // Soft: a requirement may still be unresolved, but if it HAS a
                // preferred part it must also have a fallback, otherwise a single
                // EOL notice stops the build.
                if req.preferred().is_some() {
                    assert!(
                        req.alternates() > 0,
                        "requirement '{}' has a preferred part ({}) but no alternate. A single \
                         source is a single point of failure; industry practice is 2-3 second \
                         sources. Add an Alternate, or mark the preferred one Evaluating while \
                         the second source is found.",
                        req.id,
                        req.preferred().map_or("?", |c| c.mpn)
                    );
                }
            }
            LockStatus::Hard => {
                assert!(
                    req.preferred().is_some(),
                    "requirement '{}' has no preferred part, but the revision is HARD locked",
                    req.id
                );
                assert!(
                    req.alternates() > 0,
                    "requirement '{}' has no alternate, but the revision is HARD locked",
                    req.id
                );
            }
        }
    }
}

/// A hard lock may not rest on estimates.
///
/// While soft, estimates are expected and allowed — that is what soft means.
/// This test exists so hardening cannot happen while a guessed number is still
/// load-bearing, which is the mistake `HOST_TO_TARGET_DERATE` made.
#[test]
fn a_hard_lock_has_no_estimates_left() {
    for spec in [&V2, &V1] {
        if spec.is_soft() {
            continue;
        }
        for (what, p) in [
            ("mcu", spec.mcu.provenance),
            ("dac", spec.dac.provenance),
            ("power", spec.power.provenance),
        ] {
            assert!(
                !p.is_estimate(),
                "{:?} is HARD locked but its {what} figure is still an estimate: {p:?}",
                spec.revision
            );
        }
    }
}

/// The V1 -> V2 migration gap, enumerated.
///
/// The firmware targets V2; `hardware/elec/src` is still V1. That is a planned
/// migration, not a mistake — but it must be *tracked*, because several firmware
/// conclusions are conditional on V2 hardware that does not exist yet. This test
/// fails if a delta appears that is not listed, so the gap cannot grow quietly.
#[test]
fn migration_deltas_are_tracked() {
    // Fixed buffer, not a `Vec`: this crate is `no_std`, so the test harness
    // does not bring `std` into scope for it.
    let mut deltas: [&str; 8] = [""; 8];
    let mut n = 0usize;

    if V1.mcu.mpn != V2.mcu.mpn {
        deltas[n] = "mcu";
        n += 1;
    }
    if V1.dac.mpn != V2.dac.mpn {
        deltas[n] = "dac";
        n += 1;
    }
    if V1.dac.role != V2.dac.role {
        deltas[n] = "dac-role";
        n += 1;
    }
    if V1.dac.integrated_amp != V2.dac.integrated_amp {
        deltas[n] = "amp";
        n += 1;
    }
    if V1.memory.iter().any(|r| r.name == "SDRAM") {
        deltas[n] = "sdram";
        n += 1;
    }
    if V1.oscillators.len() != V2.oscillators.len() {
        deltas[n] = "oscillators";
        n += 1;
    }

    // The deltas the migration design document accounts for. A NEW entry here
    // means the two revisions diverged further without anyone deciding to.
    const TRACKED: &[&str] = &["mcu", "dac", "dac-role", "amp", "sdram", "oscillators"];

    for d in &deltas[..n] {
        assert!(
            TRACKED.contains(d),
            "untracked V1->V2 delta '{d}'. Either record it in docs/plans/\
             2026-11-15-v2-migration-design.md and add it to TRACKED, or reconcile the two \
             revisions."
        );
    }
    for t in TRACKED {
        assert!(
            deltas[..n].contains(t),
            "tracked delta '{t}' no longer exists — if the migration completed for it, remove it \
             from TRACKED so the list stays an honest account of what still differs"
        );
    }

    // The consequence worth stating loudly, since it conditions several firmware
    // conclusions: V1 has external SDRAM and V2 does not.
    assert!(
        V1.total_dma_bytes() > V2.total_dma_bytes() * 4,
        "sanity: V1 was expected to have far more DMA RAM than V2 (32 MB SDRAM vs 1.15 MB \
         on-chip). If that is no longer true, every memory conclusion in the placement doc \
         needs re-reading."
    );
}

/// The active revision is V2, and soft.
#[test]
fn the_active_revision_is_v2_and_still_soft() {
    assert_eq!(crate::ACTIVE.revision, Revision::V2);
    assert!(
        crate::ACTIVE.is_soft(),
        "V2 is soft-locked on purpose: the oscillator pair is a proposal, the sleep current is \
         an estimate, and CPI is unmeasured. Hardening is a deliberate edit, not a default."
    );
    // And the soft lock must say what would harden it, or it is just a shrug.
    if let LockStatus::Soft { until } = crate::ACTIVE.lock {
        assert!(
            until.len() > 40,
            "the soft lock must record what has to happen before it can harden"
        );
    }
}

/// Provenance must be attached to the figures a budget rests on.
#[test]
fn load_bearing_figures_cite_their_source() {
    let spec = crate::ACTIVE;
    for (what, p) in [
        ("mcu", spec.mcu.provenance),
        ("dac", spec.dac.provenance),
        ("power", spec.power.provenance),
    ] {
        let cite = match p {
            Provenance::Datasheet(s)
            | Provenance::Measured(s)
            | Provenance::Estimate(s)
            | Provenance::Choice(s) => s,
        };
        assert!(
            cite.len() > 5,
            "the {what} figure's provenance is too vague to check: {cite:?}"
        );
    }
}
