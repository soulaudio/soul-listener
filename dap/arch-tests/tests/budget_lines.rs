//! Budget and risk gates: does the committed design fit, and are the latent
//! risks still latent?
//!
//! # The question this answers
//!
//! "255 KB spare" was true and misleading. It was spare because the framebuffer,
//! the DMA rings and the listing cache are not built, so the headroom was a
//! forecast overrun wearing a surplus costume. Prose could not stop that being
//! quoted as headroom, because prose is not subtractable. These gates subtract.
//!
//! Three separate questions, deliberately not merged:
//!
//! 1. does what is BUILT fit? (it does, with room)
//! 2. does built + PLANNED fit? (the real question)
//! 3. if not, is there enough DISCRETIONARY weight to drop to make it fit?
//!    A design that overflows with no relief valve is over-committed; one that
//!    overflows but can drop a 96 KB shadow buffer has a decision to make.

use dap_hw_spec::budget::{LineStatus, V2_LINES, V2_RISKS};
use dap_hw_spec::ACTIVE;

/// Bytes claimed in `region` by lines matching `pred`.
fn claimed(region: &str, pred: impl Fn(&LineStatus) -> bool) -> usize {
    V2_LINES
        .iter()
        .filter(|l| l.region == region && pred(&l.status))
        .map(|l| l.bytes)
        .sum()
}

/// Every budget line must name a region the spec declares AND links.
///
/// A line with nowhere to live is the failure the AHB-SRAM linking existed to
/// prevent: 128 KB of real silicon that the linker could not see, so a
/// framebuffer "planned for AHB" had no address to go to.
#[test]
fn every_budget_line_lives_in_a_linked_region() {
    for line in V2_LINES {
        let region = ACTIVE
            .memory
            .iter()
            .find(|r| r.name == line.region)
            .unwrap_or_else(|| {
                panic!(
                    "budget line '{}' claims region '{}', which the spec does not declare",
                    line.id, line.region
                )
            });
        assert!(
            region.linked,
            "budget line '{}' ({} B) claims region '{}', which exists on the die but is NOT \
             linked — so there is no address for it. {}",
            line.id, line.bytes, line.region, region.note
        );
    }
}

/// What is built today must fit, with the stack reserve honoured.
///
/// This is the only one of the three that is a statement about reality rather
/// than about a plan, so it is the one that must never be yellow.
#[test]
fn what_is_built_fits_today() {
    for region in ACTIVE.memory.iter().filter(|r| r.linked) {
        let built = claimed(region.name, |s| *s == LineStatus::Built);
        assert!(
            built <= region.bytes,
            "{}: built lines claim {} B of {} B — the firmware as it stands does not fit",
            region.name,
            built,
            region.bytes
        );
    }
}

/// The real headroom question: built + planned + enabled-discretionary.
///
/// Reported per region with the arithmetic shown, because the useful output is
/// not pass/fail but *which region is tight and by how much*.
#[test]
fn the_committed_design_fits_or_names_its_relief_valve() {
    let mut overflow_report = String::new();

    for region in ACTIVE.memory.iter().filter(|r| r.linked) {
        let built = claimed(region.name, |s| *s == LineStatus::Built);
        let planned = claimed(region.name, |s| *s == LineStatus::Planned);
        let discretionary = claimed(region.name, LineStatus::is_droppable);
        let committed = built + planned + discretionary;

        println!(
            "{:>9}: {:>7} B capacity | built {:>7} | planned {:>7} | discretionary {:>6} | \
             committed {:>7} | {}",
            region.name,
            region.bytes,
            built,
            planned,
            discretionary,
            committed,
            if committed <= region.bytes {
                format!("{} B spare", region.bytes - committed)
            } else {
                format!("OVER by {} B", committed - region.bytes)
            }
        );

        if committed > region.bytes {
            let over = committed - region.bytes;
            // Over-committed is tolerable ONLY if dropping discretionary lines
            // can recover it. Otherwise the design has no escape and the
            // overflow is a hard problem, not a decision.
            if discretionary < over {
                overflow_report.push_str(&format!(
                    "\n  {}: over by {} B, and only {} B is discretionary. Dropping every \
                     optional line still does not fit — this needs a real reduction, not a \
                     decision.",
                    region.name, over, discretionary
                ));
            } else {
                // Enumerate what could be dropped, so the failure is actionable.
                let mut options = String::new();
                for l in V2_LINES
                    .iter()
                    .filter(|l| l.region == region.name && l.status.is_droppable())
                {
                    if let LineStatus::Discretionary { tradeoff, .. } = l.status {
                        options.push_str(&format!(
                            "\n      - drop '{}' ({} B): {}",
                            l.id, l.bytes, tradeoff
                        ));
                    }
                }
                overflow_report.push_str(&format!(
                    "\n  {}: over by {} B, recoverable from {} B of discretionary lines:{}",
                    region.name, over, discretionary, options
                ));
            }
        }
    }

    assert!(
        overflow_report.is_empty(),
        "the committed design does not fit:{overflow_report}\n\nThis is the \"255 KB spare\" \
         number made subtractable: spare space that exists only because a committed line is not \
         built yet was never headroom."
    );
}

/// A discretionary line must record what dropping it costs.
///
/// Otherwise "optional" degrades into "nobody knows why this is here", and the
/// relief valve gets pulled without anyone understanding the price.
#[test]
fn discretionary_lines_record_their_tradeoff() {
    for line in V2_LINES {
        if let LineStatus::Discretionary { tradeoff, .. } = line.status {
            assert!(
                tradeoff.len() > 60,
                "discretionary line '{}' does not explain what dropping it costs. An optional \
                 line without a recorded tradeoff is just an unexplained line.",
                line.id
            );
        }
    }
}

/// Planned lines must not masquerade as measurements.
///
/// The provenance split is the whole point: 164,752 B is a measurement of real
/// code, 96,000 B is arithmetic about a panel, and 32 KB of eink scratch is a
/// guess. Totalling them is fine; forgetting which is which is how a forecast
/// becomes a budget.
#[test]
fn built_lines_are_measured_and_planned_lines_are_not() {
    use dap_hw_spec::Provenance;
    for line in V2_LINES {
        match line.status {
            LineStatus::Built => assert!(
                matches!(line.provenance, Provenance::Measured(_)),
                "line '{}' is marked Built but its size is not a measurement ({:?}). A built \
                 line's size is observable, so observe it.",
                line.id,
                line.provenance
            ),
            LineStatus::Planned | LineStatus::Discretionary { .. } => assert!(
                !matches!(line.provenance, Provenance::Measured(_)),
                "line '{}' is not built, yet claims a measured size ({:?}) — nothing exists to \
                 measure",
                line.id,
                line.provenance
            ),
        }
    }
}

/// A conditional risk must arm itself by inspection, not by memory.
///
/// The cache/DMA risk is the motivating case: harmless while caches are off,
/// and a silent-corruption class of bug the moment someone enables them for
/// speed — which the RTF work made tempting, since the cache question is worth
/// ~2x. So the gate greps the firmware for the trigger. If the trigger appears
/// and the mitigation does not, the build fails with the consequence spelled
/// out. Nobody has to remember.
#[test]
fn no_armed_risk_is_left_unmitigated() {
    // The firmware sources a risk can hide in. Deliberately the whole audio and
    // board surface rather than one file: an `enable_dcache` in a HAL init
    // helper is just as dangerous as one in main.
    const SOURCES: &[(&str, &str)] = &[
        (
            "board-h7a3/src/main.rs",
            include_str!("../../board-h7a3/src/main.rs"),
        ),
        (
            "core/src/firmware.rs",
            include_str!("../../core/src/firmware.rs"),
        ),
        ("core/src/app.rs", include_str!("../../core/src/app.rs")),
        ("core/src/audio.rs", include_str!("../../core/src/audio.rs")),
        ("core/src/power.rs", include_str!("../../core/src/power.rs")),
    ];

    for risk in V2_RISKS {
        let mut armed_in: Vec<&str> = Vec::new();
        let mut mitigated = false;

        for (name, src) in SOURCES {
            // Ignore comments: this file and the sources discuss `enable_dcache`
            // at length, and a gate that fires on its own documentation is
            // useless. Only non-comment lines count as a trigger.
            for line in src.lines() {
                let t = line.trim_start();
                if t.starts_with("//") || t.starts_with("/*") || t.starts_with('*') {
                    continue;
                }
                if risk.trigger_symbols.iter().any(|s| t.contains(s)) {
                    armed_in.push(name);
                }
                if risk.mitigation_symbols.iter().any(|s| t.contains(s)) {
                    mitigated = true;
                }
            }
        }

        if !armed_in.is_empty() {
            assert!(
                mitigated,
                "RISK '{}' IS NOW ARMED and unmitigated.\n\nCondition: {}\nFound in: {:?}\n\n\
                 Consequence: {}\n\nMitigation: {}",
                risk.id, risk.condition, armed_in, risk.consequence, risk.mitigation
            );
        } else {
            println!(
                "risk '{}' is dormant: no trigger ({:?}) in the firmware sources",
                risk.id, risk.trigger_symbols
            );
        }
    }
}
