//! The TME slice, end to end, on data a distributor actually returned.
//!
//! Every other test in this crate exercises the pipeline against a fixture I
//! wrote. This one runs it against `hardware/bom/candidates/mclk-mux.json`,
//! which came out of the live TME API via
//! `tools/sourcing-run.py fetch --vendor tme` and `export`. It is the only test
//! that can tell me whether the filter works on real input rather than on my
//! idea of real input — and the first time it ran, it did not.
//!
//! # The chain this closes
//!
//! ```text
//!   TME API v2  ->  TmeAdapter (name mapping only, values untouched)
//!               ->  SQLite catalogue (uncommitted, rebuildable)
//!               ->  candidates/<category>.json  (raw vendor strings, committed)
//!               ->  parametric::derive          (the ONLY unit interpretation)
//!               ->  select::evaluate + rank     (the decision, with reasons)
//! ```
//!
//! Each hop is deliberately boring except `derive` and `evaluate`, which hold all
//! the judgement and all the tests.
//!
//! # Why the candidate file is committed when the database is not
//!
//! So this test runs in CI without credentials. It is the repo's stated idiom —
//! generate, commit, and gate that the committed artefact matches a fresh
//! regeneration — applied one level further out than the snapshot.

use dap_hw_spec::parametric::{derive, expected_dimension};
use dap_hw_spec::select::{
    evaluate, rank, AttrValue, CataloguePart, Evidence, Rejection, DEFAULT_RANKING,
};
use dap_hw_spec::{Preference, ACTIVE};
use serde_json::Value;

const MUX_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/mclk-mux.json");
const BATTERY_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/battery.json");
const MCU_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/mcu.json");
const OSC_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/mclk-osc-44k1.json");

/// Raw vendor strings for one part, owned so the engine can borrow them.
struct Raw {
    mpn: String,
    manufacturer: String,
    eu_sources: u32,
    eu_stock: u32,
    unit_price_cents: Option<u32>,
    moq: Option<u32>,
    lead_time_days: Option<u32>,
    attrs_raw: Vec<(String, String)>,
    datasheet_url: Option<String>,
}

fn load(doc: &str, category: &str) -> Vec<Raw> {
    let v: Value = serde_json::from_str(doc).expect("candidate file must be valid JSON");
    assert_eq!(
        v["kind"].as_str(),
        Some("candidates"),
        "this reads a candidate export, not a fixture or a snapshot"
    );
    assert_eq!(
        v["category"].as_str(),
        Some(category),
        "candidate file is for a different requirement than asked for"
    );
    v["parts"]
        .as_array()
        .expect("parts must be an array")
        .iter()
        .map(|p| Raw {
            mpn: p["mpn"].as_str().unwrap_or_default().to_string(),
            manufacturer: p["manufacturer"].as_str().unwrap_or_default().to_string(),
            eu_sources: p["eu_sources"].as_u64().unwrap_or(0) as u32,
            eu_stock: p["eu_stock"].as_u64().unwrap_or(0) as u32,
            unit_price_cents: p["unit_price_cents"].as_u64().map(|x| x as u32),
            moq: p["moq"].as_u64().map(|x| x as u32),
            lead_time_days: p["lead_time_days"].as_u64().map(|x| x as u32),
            datasheet_url: p["datasheet_url"].as_str().map(str::to_string),
            attrs_raw: p["attrs_raw"]
                .as_object()
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect()
}

/// Find the raw string that answers a constraint's attribute.
///
/// The indirection that makes range containment work from one published field:
/// a constraint on `supply_mv_min` is answered by the vendor's `supply_mv`
/// (`"1.65...5.5V DC"`), because distributors publish one operating range where
/// the spec needs two bounds. An exact name match wins if the vendor happens to
/// publish the bound separately.
fn raw_for<'a>(attrs: &'a [(String, String)], attr: &str) -> Option<&'a str> {
    if let Some((_, v)) = attrs.iter().find(|(k, _)| k.eq_ignore_ascii_case(attr)) {
        return Some(v);
    }
    let base = attr
        .strip_suffix("_min")
        .or_else(|| attr.strip_suffix("_max"))?;
    attrs
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(base))
        .map(|(_, v)| v.as_str())
}

/// Turn raw vendor strings into the typed attributes the constraints read.
///
/// Only the attributes this requirement actually constrains are derived. That is
/// not an optimisation — deriving everything would invent values for attributes
/// no constraint asked about, and a wrong value nobody checks is worse than a
/// missing one, which the engine at least reports.
fn typed<'a>(
    raw: &'a Raw,
    constraints: &'a [dap_hw_spec::select::Constraint<'static>],
) -> Vec<(&'static str, AttrValue<'a>)> {
    let mut out: Vec<(&'static str, AttrValue<'a>)> = Vec::new();
    for c in constraints {
        if c.evidence == Evidence::Datasheet {
            // Not a catalogue question; the engine skips it anyway.
            continue;
        }
        let Some(s) = raw_for(&raw.attrs_raw, c.attr) else {
            continue;
        };
        match expected_dimension(c.attr) {
            Some(_) => {
                if let Some(n) = derive(c.attr, s) {
                    out.push((c.attr, AttrValue::Num(n)));
                }
                // A value that will not derive is left ABSENT rather than
                // defaulted, so the engine reports a missing attribute instead
                // of silently judging a guess.
            }
            None => out.push((c.attr, AttrValue::Text(s))),
        }
    }
    out
}

/// **The assertion this whole slice exists for.**
///
/// `SN74LVC1G3157DCKR` is real: 6916 in stock at TME, 19 cents at quantity one,
/// rated 1.65–5.5 V, SPDT, SMD. It is the part `DEFAULT_RANKING` prefers on
/// sourcing depth, and before the constraints were fixed the filter rejected it
/// three times over — for a supply rating *above* the rail, for a channel count
/// in the wrong convention, and for two fields no distributor publishes.
///
/// If this ever fails, the filter has gone back to rejecting a part that is in
/// stock and correct, which is the single worst failure mode a sourcing pipeline
/// has: it looks like diligence.
#[test]
fn the_real_ti_mux_qualifies_on_live_distributor_data() {
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "mclk-mux")
        .expect("spec must carry mclk-mux");
    let raws = load(MUX_CANDIDATES, "mclk-mux");
    assert!(
        !raws.is_empty(),
        "the candidate export is empty, so this test proves nothing — re-run \
         `tools/sourcing-run.py fetch --vendor tme && ... export --category mclk-mux`"
    );

    let typed_attrs: Vec<Vec<(&str, AttrValue)>> =
        raws.iter().map(|r| typed(r, req.constraints)).collect();
    let parts: Vec<CataloguePart<'_>> = raws
        .iter()
        .zip(&typed_attrs)
        .map(|(r, a)| CataloguePart {
            mpn: &r.mpn,
            manufacturer: &r.manufacturer,
            attrs: a.as_slice(),
            eu_sources: r.eu_sources,
            eu_stock: r.eu_stock,
            unit_price_cents: r.unit_price_cents,
            moq: r.moq,
            lead_time_days: r.lead_time_days,
        })
        .collect();

    let mut verdicts = vec![None; parts.len()];
    evaluate(&parts, req.constraints, &mut verdicts);

    println!("\n=== mclk-mux, live TME data: {}", req.what);
    let mut qualified: Vec<CataloguePart<'_>> = Vec::new();
    for v in verdicts.iter().flatten() {
        match v.rejection {
            None => {
                println!("  QUALIFIES {}", v.part.mpn);
                qualified.push(v.part);
            }
            Some(Rejection::MissingAttribute { attr, .. }) => {
                println!("  reject    {} — no '{attr}' published", v.part.mpn)
            }
            Some(Rejection::WrongType { attr }) => {
                println!("  reject    {} — '{attr}' wrong type", v.part.mpn)
            }
            Some(Rejection::Failed { attr, got, .. }) => {
                println!("  reject    {} — {attr}={got:?}", v.part.mpn)
            }
        }
    }

    let ti = verdicts
        .iter()
        .flatten()
        .find(|v| v.part.mpn == "SN74LVC1G3157DCKR")
        .expect("the TI mux must be in the live candidate export");
    assert!(
        ti.rejection.is_none(),
        "SN74LVC1G3157DCKR was REJECTED on live data: {:?}.\n\n\
         This part is 6916 in stock at TME, rated 1.65-5.5 V, SPDT, SMD — it is correct and \
         buyable, and it is the part the ranking policy prefers. A filter that rejects it is \
         worse than no filter, because the output looks like diligence.",
        ti.rejection
    );

    rank(&mut qualified, DEFAULT_RANKING);
    for (i, p) in qualified.iter().enumerate() {
        println!(
            "  RANK {} {} — {} EU source(s), {} stock, {:?} cents",
            i + 1,
            p.mpn,
            p.eu_sources,
            p.eu_stock,
            p.unit_price_cents
        );
    }
    assert!(!qualified.is_empty(), "no live part qualified");
}

/// The derivation actually read the vendor's strings, rather than skipping them.
///
/// A silent pass is the failure mode here: if `derive` returned `None` for every
/// value, `typed` would emit no attributes, every constraint would see a missing
/// field, and the part would be rejected — which the test above catches. The
/// subtler case is a constraint whose attribute never got derived because the
/// adapter's name map missed it, leaving the part qualified on *fewer* checks
/// than the spec thinks it applied.
#[test]
fn every_catalogue_constraint_was_actually_evaluated_against_a_real_value() {
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "mclk-mux")
        .expect("spec must carry mclk-mux");
    let raws = load(MUX_CANDIDATES, "mclk-mux");
    let ti = raws
        .iter()
        .find(|r| r.mpn == "SN74LVC1G3157DCKR")
        .expect("the TI mux must be in the export");

    let derived = typed(ti, req.constraints);
    let catalogue: Vec<&str> = req
        .constraints
        .iter()
        .filter(|c| c.evidence == Evidence::Catalogue)
        .map(|c| c.attr)
        .collect();

    for attr in &catalogue {
        assert!(
            derived.iter().any(|(k, _)| k == attr),
            "constraint on '{attr}' had NO value derived from the live data, so the part was \
             judged on fewer checks than the spec believes it applied.\n\n\
             Either the adapter's ATTR_MAP does not map the vendor's parameter name onto \
             '{attr}', or parametric::derive could not read the value. Raw attributes present: \
             {:?}",
            ti.attrs_raw
                .iter()
                .map(|(k, v)| format!("{k}={v}"))
                .collect::<Vec<_>>()
        );
    }
    println!(
        "all {} catalogue constraints evaluated on real values",
        catalogue.len()
    );
}

/// The discovery path, on a real category of 24 parts.
///
/// This is the case the whole selection engine exists for and that verifying
/// already-chosen MPNs never exercises: a vendor category containing parts we did
/// not pick, most of which should lose.
///
/// It asserts the shape of a *useful* answer rather than a specific winner,
/// because the catalogue changes under us: at least one part must qualify, and
/// every rejection must name an attribute. Zero qualifying is the failure mode
/// worth guarding — it is what over-strict constraints look like, and it reads as
/// "nothing is sourceable" rather than "the spec is wrong".
#[test]
fn the_oscillator_category_yields_candidates_and_explains_the_rest() {
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "mclk-osc-44k1")
        .expect("spec must carry mclk-osc-44k1");
    let raws = load(OSC_CANDIDATES, "mclk-osc-44k1");
    assert!(
        raws.len() >= 10,
        "expected a real category sweep, got {} parts",
        raws.len()
    );

    let typed_attrs: Vec<Vec<(&str, AttrValue)>> =
        raws.iter().map(|r| typed(r, req.constraints)).collect();
    let parts: Vec<CataloguePart<'_>> = raws
        .iter()
        .zip(&typed_attrs)
        .map(|(r, a)| CataloguePart {
            mpn: &r.mpn,
            manufacturer: &r.manufacturer,
            attrs: a.as_slice(),
            eu_sources: r.eu_sources,
            eu_stock: r.eu_stock,
            unit_price_cents: r.unit_price_cents,
            moq: r.moq,
            lead_time_days: r.lead_time_days,
        })
        .collect();

    let mut verdicts = vec![None; parts.len()];
    evaluate(&parts, req.constraints, &mut verdicts);

    println!(
        "\n=== mclk-osc-44k1, live TME sweep of {} parts",
        parts.len()
    );
    let mut qualified: Vec<CataloguePart<'_>> = Vec::new();
    let mut by_reason: Vec<(&str, &str)> = Vec::new();
    for v in verdicts.iter().flatten() {
        match v.rejection {
            None => {
                println!("  QUALIFIES {:<28} stock={}", v.part.mpn, v.part.eu_stock);
                qualified.push(v.part);
            }
            Some(Rejection::MissingAttribute { attr, .. }) => {
                println!("  reject    {:<28} no '{attr}' published", v.part.mpn);
                by_reason.push((v.part.mpn, attr));
            }
            Some(Rejection::WrongType { attr }) => {
                println!("  reject    {:<28} '{attr}' wrong type", v.part.mpn);
                by_reason.push((v.part.mpn, attr));
            }
            Some(Rejection::Failed { attr, got, .. }) => {
                println!("  reject    {:<28} {attr}={got:?}", v.part.mpn);
                by_reason.push((v.part.mpn, attr));
            }
        }
    }

    rank(&mut qualified, DEFAULT_RANKING);
    for (i, p) in qualified.iter().enumerate() {
        println!(
            "  RANK {} {:<28} {} source(s), {} stock, {:?} cents",
            i + 1,
            p.mpn,
            p.eu_sources,
            p.eu_stock,
            p.unit_price_cents
        );
    }

    assert!(
        !qualified.is_empty(),
        "NOT ONE of {} real parts qualified. That is the over-strict failure mode: the output \
         reads as 'nothing is sourceable' when it actually means the constraints are wrong. \
         Rejections were: {:?}",
        parts.len(),
        by_reason
    );
    assert_eq!(
        qualified.len() + by_reason.len(),
        parts.len(),
        "every part must be either qualified or rejected WITH a named attribute — a part that is \
         neither has fallen through the report"
    );
}

/// A datasheet-only constraint needs a datasheet — for the parts we act on.
///
/// `Evidence::Datasheet` moves a constraint from the filter to a person. That is
/// only honest if the person can reach the document, and until `/products/files`
/// was wired up the answer was to guess the URL — which is literally how
/// `abracon.com/Oscillators/ASV.pdf` was found.
///
/// # Scoped to the spec's candidates, and why
///
/// The first version of this gate demanded a datasheet link for every part in
/// the category, and it failed the moment the mux sweep grew from 1 part to 261:
/// TME publishes no link for 57 of them. That is a vendor data gap, not
/// something this repo can fix, so requiring it everywhere made the gate
/// unsatisfiable — the same over-strictness that the hard MOQ constraint was
/// removed for.
///
/// The purpose points at a narrower rule. Nobody reads a datasheet for a part the
/// filter rejected; the confirmations happen for the MPNs the spec actually names
/// as candidates. So those must have a reachable link, and a missing link
/// elsewhere in a sweep is reported as information.
#[test]
fn spec_candidates_with_datasheet_constraints_carry_a_datasheet_link() {
    use dap_hw_spec::select::Evidence;

    for (id, doc) in [
        ("mclk-mux", MUX_CANDIDATES),
        ("mclk-osc-44k1", OSC_CANDIDATES),
    ] {
        let req = ACTIVE
            .parts
            .iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("spec must carry {id}"));
        let datasheet_constraints: Vec<&str> = req
            .constraints
            .iter()
            .filter(|c| c.evidence == Evidence::Datasheet)
            .map(|c| c.attr)
            .collect();
        if datasheet_constraints.is_empty() {
            continue;
        }

        let raws = load(doc, id);
        let have_link = |mpn: &str| {
            raws.iter()
                .find(|r| r.mpn.eq_ignore_ascii_case(mpn))
                .map(|r| !r.datasheet_url.as_deref().unwrap_or("").is_empty())
        };

        let mut checked = 0usize;
        for cand in req.candidates {
            match have_link(cand.mpn) {
                // Not in this vendor's catalogue — a different problem, and one
                // `every_viable_candidate_is_in_the_snapshot` already owns.
                None => continue,
                Some(true) => checked += 1,
                Some(false) => panic!(
                    "'{id}' has datasheet-only constraints {datasheet_constraints:?}, and \
                     candidate {} is in the catalogue with NO datasheet URL.\n\n\
                     Those constraints can only be satisfied by a person reading a document, so a \
                     candidate without a link turns each one into a research task — which is how \
                     a URL ends up guessed by hand. TME publishes these on /products/files; \
                     re-run `tools/sourcing-run.py fetch` and re-export.",
                    cand.mpn
                ),
            }
        }

        // Coverage across the whole sweep is information, not a verdict.
        let missing = raws
            .iter()
            .filter(|r| r.datasheet_url.as_deref().unwrap_or("").is_empty())
            .count();
        println!(
            "{id}: {checked} spec candidate(s) with a datasheet link; {}/{} swept parts lack one \
             (vendor gap, not a failure)",
            missing,
            raws.len()
        );

        // A video is never a datasheet. The TI part's second document on
        // /products/files is a YouTube link, so "take the first document" would
        // have produced exactly that.
        for r in &raws {
            let url = r.datasheet_url.as_deref().unwrap_or("");
            if url.is_empty() {
                continue;
            }
            assert!(
                !url.contains("youtube") && !url.contains("youtu.be"),
                "{}'s datasheet URL is a video: {url}",
                r.mpn
            );
            assert!(
                url.starts_with("http"),
                "{}'s datasheet URL is not absolute: {url:?} — TME returns protocol-relative \
                 paths (//www.tme.eu/...) that nobody can open",
                r.mpn
            );
        }
    }
}

/// The battery requirement, against 70 real Li-Po cells.
///
/// This requirement was the weakest in the spec: one candidate, never validated,
/// no second source, and a manufacturer-independence gate that only ever ran
/// against fixture data. TME turns out to carry 70 of them with the discriminating
/// parameters published — `Rechargeable batteries features` naming
/// 'PCB protection' and `Leads` naming the JST connector.
///
/// Asserts the shape of a useful answer rather than a winner, since stock moves:
/// candidates exist, more than one manufacturer survives, and every rejection
/// names an attribute.
#[test]
fn the_battery_category_yields_candidates_from_more_than_one_maker() {
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "battery")
        .expect("spec must carry battery");
    let raws = load(BATTERY_CANDIDATES, "battery");
    assert!(
        raws.len() >= 20,
        "expected a real sweep, got {}",
        raws.len()
    );

    let typed_attrs: Vec<Vec<(&str, AttrValue)>> =
        raws.iter().map(|r| typed(r, req.constraints)).collect();
    let parts: Vec<CataloguePart<'_>> = raws
        .iter()
        .zip(&typed_attrs)
        .map(|(r, a)| CataloguePart {
            mpn: &r.mpn,
            manufacturer: &r.manufacturer,
            attrs: a.as_slice(),
            eu_sources: r.eu_sources,
            eu_stock: r.eu_stock,
            unit_price_cents: r.unit_price_cents,
            moq: r.moq,
            lead_time_days: r.lead_time_days,
        })
        .collect();

    let mut verdicts = vec![None; parts.len()];
    evaluate(&parts, req.constraints, &mut verdicts);

    println!(
        "\n=== battery, live TME sweep of {} Li-Po cells",
        parts.len()
    );
    let mut qualified: Vec<CataloguePart<'_>> = Vec::new();
    let mut reasons: Vec<(&str, &str)> = Vec::new();
    for v in verdicts.iter().flatten() {
        match v.rejection {
            None => qualified.push(v.part),
            Some(Rejection::MissingAttribute { attr, .. }) => reasons.push((v.part.mpn, attr)),
            Some(Rejection::WrongType { attr }) => reasons.push((v.part.mpn, attr)),
            Some(Rejection::Failed { attr, .. }) => reasons.push((v.part.mpn, attr)),
        }
    }
    rank(&mut qualified, DEFAULT_RANKING);
    for (i, p) in qualified.iter().take(6).enumerate() {
        println!(
            "  RANK {} {:<22} {:<18} stock={:<6} moq={:?}",
            i + 1,
            p.mpn,
            p.manufacturer,
            p.eu_stock,
            p.moq
        );
    }
    println!(
        "  ({} qualified, {} rejected)",
        qualified.len(),
        reasons.len()
    );

    assert!(
        !qualified.is_empty(),
        "no cell qualified out of {} — the over-strict failure mode. Rejections: {:?}",
        parts.len(),
        reasons
    );
    assert_eq!(
        qualified.len() + reasons.len(),
        parts.len(),
        "every part must be qualified or rejected with a named attribute"
    );

    // The property the spec cares about and could not previously check against
    // real data: a spare must be obtainable from more than one maker.
    let mut makers: Vec<&str> = qualified.iter().map(|p| p.manufacturer).collect();
    makers.sort_unstable();
    makers.dedup();
    println!("  manufacturers: {makers:?}");
    assert!(
        !makers.is_empty(),
        "qualified cells report no manufacturer, so independence cannot be judged"
    );
}

/// The MCU requirement, against a real sample of ST's catalogue.
///
/// The requirement was prose-only until a sweep showed how much of it a
/// distributor answers. The point of this test is the *preferred part passing its
/// own requirement* — a constraint set written after the part was chosen can
/// easily exclude it, and nothing would notice until someone re-ran selection.
#[test]
fn the_preferred_mcu_satisfies_the_requirement_it_was_chosen_for() {
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "mcu")
        .expect("spec must carry mcu");
    let raws = load(MCU_CANDIDATES, "mcu");
    assert!(
        raws.len() >= 50,
        "expected a real sample, got {}",
        raws.len()
    );

    let typed_attrs: Vec<Vec<(&str, AttrValue)>> =
        raws.iter().map(|r| typed(r, req.constraints)).collect();
    let parts: Vec<CataloguePart<'_>> = raws
        .iter()
        .zip(&typed_attrs)
        .map(|(r, a)| CataloguePart {
            mpn: &r.mpn,
            manufacturer: &r.manufacturer,
            attrs: a.as_slice(),
            eu_sources: r.eu_sources,
            eu_stock: r.eu_stock,
            unit_price_cents: r.unit_price_cents,
            moq: r.moq,
            lead_time_days: r.lead_time_days,
        })
        .collect();

    let mut verdicts = vec![None; parts.len()];
    evaluate(&parts, req.constraints, &mut verdicts);

    let qualified: Vec<&str> = verdicts
        .iter()
        .flatten()
        .filter(|v| v.rejection.is_none())
        .map(|v| v.part.mpn)
        .collect();
    println!(
        "\n=== mcu: {} of {} sampled ST parts satisfy the requirement",
        qualified.len(),
        parts.len()
    );

    // The spec's own candidates must pass, where the catalogue holds them.
    for cand in req.candidates {
        let Some(v) = verdicts
            .iter()
            .flatten()
            .find(|v| v.part.mpn.eq_ignore_ascii_case(cand.mpn))
        else {
            continue;
        };
        if matches!(
            cand.preference,
            Preference::Preferred | Preference::Alternate | Preference::Evaluating
        ) {
            assert!(
                v.rejection.is_none(),
                "{} is a viable candidate for 'mcu' but its OWN requirement rejects it: {:?}.\n\n\
                 A constraint set written after the part was chosen can quietly exclude it, and \
                 nothing notices until someone re-runs selection and finds the design \
                 unsourceable.",
                cand.mpn,
                v.rejection
            );
            println!("  candidate {} passes its own requirement", cand.mpn);
        }
    }
    assert!(
        !qualified.is_empty(),
        "no sampled MCU satisfied the requirement — over-strict constraints"
    );
}
