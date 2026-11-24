//! The pipeline across TWO vendors — the cross-vendor join SQLite was chosen for.
//!
//! `tme_vertical_slice.rs` proves one vendor end to end. This proves the thing a
//! single vendor cannot: that "EU source depth", the first key in
//! `DEFAULT_RANKING`, is computed by actually joining two distributors on MPN.
//! Until Digi-Key existed, every part had at most one source and that ranking key
//! was decorative.
//!
//! It also pins two properties that only have meaning with more than one vendor:
//! canonical vocabulary, and the DAC requirement — which was prose-only until a
//! distributor that stocks the part was fetched.

use dap_hw_spec::parametric::{derive, expected_dimension};
use dap_hw_spec::select::{evaluate, AttrValue, CataloguePart, Evidence, Rejection};
use dap_hw_spec::{Preference, ACTIVE};
use serde_json::Value;

const DAC_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/dac.json");
const BT_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/bluetooth.json");
const MUX_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/mclk-mux.json");
const OSC_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/mclk-osc-44k1.json");

struct Raw {
    mpn: String,
    manufacturer: String,
    eu_sources: u32,
    eu_stock: u32,
    unit_price_cents: Option<u32>,
    moq: Option<u32>,
    lead_time_days: Option<u32>,
    vendors: Vec<String>,
    attrs_raw: Vec<(String, String)>,
}

fn load(doc: &str, category: &str) -> Vec<Raw> {
    let v: Value = serde_json::from_str(doc).expect("candidate file must be valid JSON");
    assert_eq!(v["category"].as_str(), Some(category));
    v["parts"]
        .as_array()
        .expect("parts array")
        .iter()
        .map(|p| Raw {
            mpn: p["mpn"].as_str().unwrap_or_default().to_string(),
            manufacturer: p["manufacturer"].as_str().unwrap_or_default().to_string(),
            eu_sources: p["eu_sources"].as_u64().unwrap_or(0) as u32,
            eu_stock: p["eu_stock"].as_u64().unwrap_or(0) as u32,
            unit_price_cents: p["unit_price_cents"].as_u64().map(|x| x as u32),
            moq: p["moq"].as_u64().map(|x| x as u32),
            lead_time_days: p["lead_time_days"].as_u64().map(|x| x as u32),
            vendors: p["vendors"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
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

fn typed<'a>(
    raw: &'a Raw,
    constraints: &'a [dap_hw_spec::select::Constraint<'static>],
) -> Vec<(&'static str, AttrValue<'a>)> {
    let mut out = Vec::new();
    for c in constraints {
        if c.evidence == Evidence::Datasheet {
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
            }
            None => out.push((c.attr, AttrValue::Text(s))),
        }
    }
    out
}

/// **Source depth is a real join across two distributors.**
///
/// `Rank::EuSourceDepth` is the first key in `DEFAULT_RANKING` and the stated
/// reason a dearer part can beat a cheaper one. With one vendor it could only
/// ever be 0 or 1, so the whole policy was untested. This asserts that at least
/// one part is genuinely stocked by two, and that the vendor list backs the count
/// up — a count without the names behind it is a number nobody can check.
#[test]
fn eu_source_depth_counts_two_distinct_distributors() {
    let mut multi: Vec<(&str, u32, &Vec<String>)> = Vec::new();
    let mut owned = Vec::new();
    for (doc, cat) in [
        (MUX_CANDIDATES, "mclk-mux"),
        (OSC_CANDIDATES, "mclk-osc-44k1"),
    ] {
        owned.push(load(doc, cat));
    }
    for set in &owned {
        for r in set {
            if r.eu_sources >= 2 {
                multi.push((&r.mpn, r.eu_sources, &r.vendors));
            }
        }
    }

    assert!(
        !multi.is_empty(),
        "no part in any category has two EU sources, so Rank::EuSourceDepth — the FIRST ranking \
         key and the reason a dearer part can beat a cheaper one — is still untested. Either the \
         second vendor was not fetched or the cross-vendor join on MPN is broken."
    );
    for (mpn, n, vendors) in &multi {
        let distinct: std::collections::BTreeSet<&str> =
            vendors.iter().map(String::as_str).collect();
        assert!(
            distinct.len() >= 2,
            "{mpn} claims {n} EU sources but names only {distinct:?}. The count and the vendor \
             list come from the same query; if they disagree, the join is wrong."
        );
        println!("  {mpn}: {n} sources {distinct:?}");
    }
}

/// Vendor vocabulary is canonical by the time the spec sees it.
///
/// TME says `mounting = 'SMD'` and Digi-Key says `'Surface Mount'` for the same
/// part, every time. The spec constrains `"SMD"`, and before canonicalisation the
/// winner depended on SQL row order — so whether a part qualified was decided by
/// a query plan. This asserts no vendor spelling survives into the export.
#[test]
fn vendor_vocabulary_is_canonical_in_the_export() {
    const FORBIDDEN: &[(&str, &str)] = &[
        ("mounting", "Surface Mount"),
        ("mounting", "surface mount"),
        ("part_type", "XO (Standard)"),
        ("part_type", "XO"),
        ("battery_chemistry", "Li-Pol"),
    ];
    for (doc, cat) in [
        (MUX_CANDIDATES, "mclk-mux"),
        (OSC_CANDIDATES, "mclk-osc-44k1"),
        (DAC_CANDIDATES, "dac"),
    ] {
        for r in load(doc, cat) {
            for (attr, bad) in FORBIDDEN {
                if let Some(v) = raw_for(&r.attrs_raw, attr) {
                    assert_ne!(
                        v, *bad,
                        "{} in '{cat}' still carries the vendor spelling {bad:?} for '{attr}'. \
                         canon.py translates these at export so the spec names each concept once; \
                         a surviving synonym means a part's verdict depends on which vendor's row \
                         won the merge.",
                        r.mpn
                    );
                }
            }
        }
    }
}

/// The DAC requirement finally filters something, and its own part passes.
///
/// `dac` was the last prose-only requirement: it had no constraints because TME
/// does not carry a single Cirrus part, so there was nothing to filter against.
/// Digi-Key publishes 15 parametric fields for the CS43131.
#[test]
fn the_dac_requirement_filters_a_real_catalogue_and_keeps_its_own_part() {
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "dac")
        .expect("spec must carry dac");
    assert!(
        !req.constraints.is_empty(),
        "the dac requirement is still prose-only. Its failure mode is a part that works and \
         sounds wrong, which is the case prose is least able to catch."
    );

    let raws = load(DAC_CANDIDATES, "dac");
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
        "\n=== dac: {} of {} sampled converters satisfy the requirement",
        qualified.len(),
        parts.len()
    );

    assert!(
        !qualified.is_empty(),
        "no sampled DAC qualified — over-strict constraints"
    );
    assert!(
        qualified.len() < parts.len(),
        "EVERY sampled converter qualified, so the constraints discriminate nothing. A filter \
         that rejects nobody is not a filter."
    );

    // And the chosen part must pass the requirement written for it.
    for cand in req.candidates {
        if !matches!(
            cand.preference,
            Preference::Preferred | Preference::Alternate | Preference::Evaluating
        ) {
            continue;
        }
        let Some(v) = verdicts
            .iter()
            .flatten()
            .find(|v| v.part.mpn.eq_ignore_ascii_case(cand.mpn))
        else {
            continue;
        };
        assert!(
            v.rejection.is_none(),
            "{} is a viable DAC candidate but its own requirement rejects it: {:?}. A constraint \
             set written after the part was chosen can quietly exclude it.",
            cand.mpn,
            match v.rejection {
                Some(Rejection::Failed { attr, got, .. }) => format!("{attr}={got:?}"),
                Some(Rejection::MissingAttribute { attr, .. }) => format!("no {attr}"),
                Some(Rejection::WrongType { attr }) => format!("{attr} wrong type"),
                None => String::new(),
            }
        );
        println!("  candidate {} passes its own requirement", cand.mpn);
    }
}

/// The Bluetooth requirement, which had never had a candidate at all.
///
/// It carried constraints from the day it was written and nothing to apply them
/// to: TME stocks no Feasycom part, and the Digi-Key sweep that found modules
/// came later. 100 real pre-certified modules now exist in the catalogue —
/// u-blox, Murata, Silicon Labs, Insight SIP.
///
/// The point of the test is the correction those modules forced. A distributor
/// publishes a radio's frequency, supply, temperature and package; it does not
/// publish a QDID, a certification number, or whether an SDK is NDA-gated. Those
/// are the reason the requirement exists, so they are `Evidence::Datasheet` — and
/// left as catalogue constraints they rejected all 100.
#[test]
fn the_bluetooth_requirement_filters_real_modules_and_defers_certification() {
    use dap_hw_spec::select::Evidence;

    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "bluetooth")
        .expect("spec must carry bluetooth");

    // The certification facts must be datasheet-only: no distributor carries
    // them, and demanding them from a catalogue disqualifies every module.
    for attr in [
        "bt_sig_qualified",
        "radio_certified_eu",
        "requires_vendor_nda",
        "codec_sbc",
    ] {
        let c = req
            .constraints
            .iter()
            .find(|c| c.attr == attr)
            .unwrap_or_else(|| panic!("bluetooth must still constrain '{attr}'"));
        assert_eq!(
            c.evidence,
            Evidence::Datasheet,
            "'{attr}' is marked as a catalogue field, but a QDID and an NDA status are \
             certificates rather than parametric data. As a catalogue constraint it rejects every \
             real module — the trap break_before_make and phase noise both fell into."
        );
    }

    let raws = load(BT_CANDIDATES, "bluetooth");
    assert!(
        raws.len() >= 50,
        "expected a real module sweep, got {} — this requirement having no candidates is what \
         the sweep was for",
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
        "\n=== bluetooth: {} of {} swept modules pass the catalogue-checkable constraints",
        qualified.len(),
        parts.len()
    );
    for m in qualified.iter().take(5) {
        println!("  candidate {m}");
    }

    assert!(
        !qualified.is_empty(),
        "no module passed, so the catalogue-checkable half of this requirement rejects \
         everything — which is the over-strict failure mode, not a sourcing finding"
    );
    assert!(
        qualified.len() < parts.len(),
        "EVERY swept part passed, so the constraints discriminate nothing. The sweep keyword \
         pulls in adjacent parts (RF front-end modules and amplifiers), and a filter that cannot \
         tell those from a Bluetooth module is not filtering."
    );
}
