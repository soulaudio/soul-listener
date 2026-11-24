//! What the spec's own constraints make of the scraped catalogue.
//!
//! Run with:
//!
//! ```text
//! cargo run -p dap-arch-tests --example sourcing-report
//! ```
//!
//! # Why this exists next to the gates
//!
//! The arch tests answer "is anything broken". They are pass/fail by design, so
//! a healthy run tells you nothing about *how* the filter is doing — and the
//! filter is the part that is supposed to earn its keep. Two failure modes are
//! invisible to a green test suite:
//!
//! 1. **A constraint that rejects everything.** A hard `moq <= 100` once
//!    eliminated the entire oscillator category. That was caught in a minute
//!    only because another gate happened to be over-strict; nothing was
//!    *reporting* it.
//! 2. **A constraint that rejects nothing.** A predicate that passes every part
//!    is decoration, and it looks identical to a well-chosen one in a test that
//!    only asserts the preferred part survives.
//!
//! So this prints, per requirement: how many candidates were swept, how many
//! qualify, which constraint did the rejecting, and whether the spec's OWN
//! preferred part is among the survivors. The rejection histogram is the useful
//! half — it is the only view in which a constraint's selectivity is visible.
//!
//! It shares `dap_arch_tests::candidates` with the gates deliberately, so the
//! report cannot claim a part qualifies while a gate rejects it.

use std::collections::BTreeMap;

use dap_arch_tests::candidates::{load, part, typed};
use dap_hw_spec::select::{evaluate, rank, AttrValue, Rejection, DEFAULT_RANKING};
use dap_hw_spec::{Preference, ACTIVE};

/// Candidate exports, included at compile time so the report cannot be run
/// against a file that has since changed under it.
const EXPORTS: &[(&str, &str)] = &[
    (
        "dac",
        include_str!("../../../hardware/bom/candidates/dac.json"),
    ),
    (
        "mclk-osc-44k1",
        include_str!("../../../hardware/bom/candidates/mclk-osc-44k1.json"),
    ),
    (
        "mclk-osc-48k",
        include_str!("../../../hardware/bom/candidates/mclk-osc-48k.json"),
    ),
    (
        "mclk-mux",
        include_str!("../../../hardware/bom/candidates/mclk-mux.json"),
    ),
    (
        "mcu",
        include_str!("../../../hardware/bom/candidates/mcu.json"),
    ),
    (
        "microsd",
        include_str!("../../../hardware/bom/candidates/microsd.json"),
    ),
    (
        "battery",
        include_str!("../../../hardware/bom/candidates/battery.json"),
    ),
    (
        "bluetooth",
        include_str!("../../../hardware/bom/candidates/bluetooth.json"),
    ),
];

/// One rejection, as a histogram key.
///
/// The constraint's own `why` is included because it is the reason the
/// constraint exists, and a histogram row saying `supply_mv_min: not published`
/// is far less useful than one that also states what the bound was for. The
/// three variants are kept distinct on purpose: "the vendor does not publish
/// this" is a DATA gap and "the part is out of range" is a real rejection, and
/// conflating them would make a catalogue coverage problem look like a market
/// problem.
fn why(r: &Rejection<'_>) -> String {
    match r {
        Rejection::MissingAttribute { attr, why } => {
            format!("{attr}: not published by any vendor ({why})")
        }
        Rejection::WrongType { attr } => format!("{attr}: published but unparseable"),
        Rejection::Failed { attr, why, .. } => format!("{attr}: out of range ({why})"),
    }
}

fn main() {
    let mut total_swept = 0usize;
    let mut total_pass = 0usize;
    let mut warnings: Vec<String> = Vec::new();

    for req in ACTIVE.parts {
        let Some((_, doc)) = EXPORTS.iter().find(|(id, _)| *id == req.id) else {
            println!("\n## {}\n   no candidate export", req.id);
            continue;
        };
        let raws = load(doc, req.id);

        let catalogue_checkable = req
            .constraints
            .iter()
            .filter(|c| c.evidence == dap_hw_spec::select::Evidence::Catalogue)
            .count();
        let datasheet_only = req.constraints.len() - catalogue_checkable;

        println!("\n## {}  ({} swept)", req.id, raws.len());
        println!("   {}", req.what);
        if req.constraints.is_empty() {
            warnings.push(format!(
                "{}: NO constraints — prose only, so nothing is machine-checked",
                req.id
            ));
            println!("   constraints: NONE (prose only — nothing is machine-checked)");
            continue;
        }
        println!(
            "   constraints: {catalogue_checkable} catalogue-checkable, \
             {datasheet_only} datasheet-only (skipped by the filter)"
        );

        // Typed attributes must outlive the parts that borrow them.
        let typedv: Vec<Vec<(&'static str, AttrValue<'_>)>> =
            raws.iter().map(|r| typed(r, req.constraints)).collect();
        let parts: Vec<_> = raws.iter().zip(&typedv).map(|(r, t)| part(r, t)).collect();

        // `evaluate` writes into a caller-provided slice rather than allocating,
        // because the same engine runs in the firmware where there is no
        // allocator. The host side just provides a big enough buffer.
        let mut slots: Vec<Option<_>> = vec![None; parts.len()];
        let n = evaluate(&parts, req.constraints, &mut slots);
        assert_eq!(n, parts.len(), "the verdict buffer must hold every part");
        let verdicts: Vec<_> = slots.into_iter().flatten().collect();

        let mut hist: BTreeMap<String, usize> = BTreeMap::new();
        // Counted separately from the histogram, keyed by attribute alone: the
        // histogram key carries the constraint's prose so one attribute can
        // appear under several rows.
        let mut missing_attr: BTreeMap<&str, usize> = BTreeMap::new();
        let mut passing = Vec::new();
        for v in &verdicts {
            match &v.rejection {
                None => passing.push(v.part),
                Some(r) => {
                    *hist.entry(why(r)).or_default() += 1;
                    if let Rejection::MissingAttribute { attr, .. } = r {
                        *missing_attr.entry(attr).or_default() += 1;
                    }
                }
            }
        }
        total_swept += raws.len();
        total_pass += passing.len();

        let pct = if raws.is_empty() {
            0.0
        } else {
            100.0 * passing.len() as f64 / raws.len() as f64
        };
        println!(
            "   QUALIFY: {} of {} ({pct:.1}%)",
            passing.len(),
            raws.len()
        );

        // A filter that rejects everything, or nothing, is not filtering.
        if passing.is_empty() && !raws.is_empty() {
            warnings.push(format!(
                "{}: ZERO of {} candidates qualify — a constraint set that admits nothing is \
                 more likely mis-specified than the market being empty",
                req.id,
                raws.len()
            ));
        } else if passing.len() == raws.len() && raws.len() > 10 {
            warnings.push(format!(
                "{}: ALL {} candidates qualify — the constraints are not discriminating, so \
                 they are decoration rather than a filter",
                req.id,
                raws.len()
            ));
        }

        // A "not published" rejection that dominates a category is far more
        // likely a MAPPING gap than a market fact, and the two are
        // indistinguishable without this check.
        //
        // The case that proved it: `switch_configuration` was reported
        // unpublished for 386 of 668 muxes. Digi-Key publishes it on 50 of 50
        // parts as `Switch Circuit` — the ATTR_MAP held `Circuit`, a name
        // transcribed from the web UI that never matched anything, so the field
        // was arriving from TME alone. Fixing three such names took the mux
        // category from 53 qualifying parts to 192 and the MCU from 109 to 210.
        for (attr, missing) in &missing_attr {
            let share = 100.0 * *missing as f64 / raws.len().max(1) as f64;
            if share >= 40.0 {
                warnings.push(format!(
                    "{}: '{attr}' is unpublished for {missing} of {} candidates ({share:.0}%).                      A rate that high usually means a vendor DOES publish it under a name                      ATTR_MAP does not have — check a real response before concluding the                      catalogue is silent",
                    req.id,
                    raws.len()
                ));
            }
        }

        if !hist.is_empty() {
            println!("   rejected by:");
            let mut rows: Vec<_> = hist.iter().collect();
            rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
            for (reason, n) in rows.iter().take(6) {
                println!("      {n:>5}  {reason}");
            }
        }

        // Does the spec's own choice survive its own filter? This is the
        // question a report has to answer first: a requirement whose preferred
        // part fails its own constraints is either a wrong constraint or a wrong
        // part, and both are urgent.
        for cand in req.candidates {
            if !matches!(
                cand.preference,
                Preference::Preferred | Preference::Alternate
            ) {
                continue;
            }
            let found = verdicts.iter().find(|v| v.part.mpn == cand.mpn);
            let label = match cand.preference {
                Preference::Preferred => "preferred",
                _ => "alternate",
            };
            match found {
                None => println!("   {label} {}: not in the swept catalogue", cand.mpn),
                Some(v) if v.qualified() => {
                    println!("   {label} {}: QUALIFIES", cand.mpn)
                }
                Some(v) => {
                    let reason = v.rejection.as_ref().map(why).unwrap_or_default();
                    println!(
                        "   {label} {}: REJECTED by its own filter ({reason})",
                        cand.mpn
                    );
                    warnings.push(format!(
                        "{}: {} is {label} but fails the requirement's own constraints ({reason})",
                        req.id, cand.mpn
                    ));
                }
            }
        }

        // The ranked head, by the spec's stated policy.
        let mut ranked: Vec<_> = passing;
        rank(&mut ranked, DEFAULT_RANKING);
        if !ranked.is_empty() {
            println!("   best by DEFAULT_RANKING (sources, then acquisition cost):");
            for p in ranked.iter().take(5) {
                let price = p
                    .unit_price_cents
                    .map_or("    ?".to_string(), |c| format!("{:5.2}", c as f64 / 100.0));
                println!(
                    "      {:<30} {:<22} src={} stock={:<7} moq={:<5} EUR {price}",
                    p.mpn,
                    p.manufacturer.chars().take(22).collect::<String>(),
                    p.eu_sources,
                    p.eu_stock,
                    p.moq.map_or("?".to_string(), |m| m.to_string()),
                );
            }
        }
    }

    println!("\n## totals");
    println!(
        "   {total_pass} of {total_swept} swept parts qualify across every requirement \
         ({:.1}%)",
        if total_swept == 0 {
            0.0
        } else {
            100.0 * total_pass as f64 / total_swept as f64
        }
    );
    if warnings.is_empty() {
        println!("   no filter anomalies");
    } else {
        println!("\n## anomalies ({})", warnings.len());
        for w in &warnings {
            println!("   - {w}");
        }
    }
}
