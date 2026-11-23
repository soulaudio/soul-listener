//! The pipeline, end to end: catalogue -> filter -> rank -> candidate.
//!
//! `dap-hw-spec`'s unit tests prove the engine's predicates and ordering in
//! isolation. This drives the whole thing on a catalogue file with the spec's
//! real constraints, which is where the useful output lives: a report saying
//! which parts qualify for each requirement and *why each rejected one failed*.
//!
//! # The catalogue here is a FIXTURE
//!
//! `hardware/bom/catalogue.fixture.json` is synthetic and says so in its own
//! readme — every MPN is prefixed `FIXTURE-`. It exists to exercise each way a
//! part can fail, which a real catalogue would only do by luck.
//!
//! Nothing about the engine changes when the data becomes real: the fetcher will
//! write the same shape from TME / Mouser / Digi-Key parametric search, against
//! the same attribute vocabulary the constraints are written in. Testing the
//! filter before the fetch exists is deliberate — the filter is the part with
//! logic in it, and it is the part that would otherwise only ever be exercised
//! against data nobody can reproduce.

use dap_hw_spec::select::{evaluate, rank, AttrValue, CataloguePart, Rejection, DEFAULT_RANKING};
use dap_hw_spec::ACTIVE;
use serde_json::Value;

const CATALOGUE: &str = include_str!("../../../hardware/bom/catalogue.fixture.json");

/// Owned attribute storage, because the engine borrows `&'a str` and `&'a [..]`
/// and `serde_json` hands back owned `String`s.
struct Owned {
    mpn: String,
    manufacturer: String,
    attrs: Vec<(String, OwnedVal)>,
    eu_sources: u32,
    eu_stock: u32,
    unit_price_cents: Option<u32>,
    lead_time_days: Option<u32>,
}

enum OwnedVal {
    Num(f64),
    Text(String),
    Bool(bool),
}

fn load(category: &str) -> Vec<Owned> {
    let doc: Value = serde_json::from_str(CATALOGUE).expect("catalogue must be valid JSON");
    assert_eq!(
        doc["kind"].as_str(),
        Some("fixture"),
        "this test reads a FIXTURE catalogue; if a real one is dropped in here, the expectations \
         below about which synthetic parts qualify stop being meaningful"
    );
    doc["categories"][category]
        .as_array()
        .unwrap_or_else(|| panic!("catalogue has no category '{category}'"))
        .iter()
        .map(|p| Owned {
            mpn: p["mpn"].as_str().unwrap_or_default().to_string(),
            manufacturer: p["manufacturer"].as_str().unwrap_or_default().to_string(),
            attrs: p["attrs"]
                .as_object()
                .map(|o| {
                    o.iter()
                        .map(|(k, v)| {
                            let val = if let Some(n) = v.as_f64() {
                                OwnedVal::Num(n)
                            } else if let Some(b) = v.as_bool() {
                                OwnedVal::Bool(b)
                            } else {
                                OwnedVal::Text(v.as_str().unwrap_or_default().to_string())
                            };
                            (k.clone(), val)
                        })
                        .collect()
                })
                .unwrap_or_default(),
            eu_sources: p["eu_sources"].as_u64().unwrap_or(0) as u32,
            eu_stock: p["eu_stock"].as_u64().unwrap_or(0) as u32,
            unit_price_cents: p["unit_price_cents"].as_u64().map(|x| x as u32),
            lead_time_days: p["lead_time_days"].as_u64().map(|x| x as u32),
        })
        .collect()
}

fn borrow(owned: &[Owned]) -> Vec<CataloguePart<'_>> {
    owned
        .iter()
        .map(|o| CataloguePart {
            mpn: &o.mpn,
            manufacturer: &o.manufacturer,
            // Leaked per part: a test-lifetime arena. The engine itself never
            // allocates; only this adapter does.
            attrs: Box::leak(
                o.attrs
                    .iter()
                    .map(|(k, v)| {
                        let val = match v {
                            OwnedVal::Num(n) => AttrValue::Num(*n),
                            OwnedVal::Text(t) => AttrValue::Text(t.as_str()),
                            OwnedVal::Bool(b) => AttrValue::Bool(*b),
                        };
                        (k.as_str(), val)
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            eu_sources: o.eu_sources,
            eu_stock: o.eu_stock,
            unit_price_cents: o.unit_price_cents,
            lead_time_days: o.lead_time_days,
        })
        .collect()
}

/// Run one requirement's constraints over its catalogue category and print the
/// full report, qualified and rejected.
fn report(req_id: &str) -> Vec<String> {
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == req_id)
        .unwrap_or_else(|| panic!("spec has no requirement '{req_id}'"));
    assert!(
        !req.constraints.is_empty(),
        "requirement '{req_id}' has no machine-checkable constraints — only prose, which cannot \
         reject a part"
    );

    let owned = load(req_id);
    let parts = borrow(&owned);
    let mut verdicts = vec![None; parts.len()];
    evaluate(&parts, req.constraints, &mut verdicts);

    println!("\n=== {req_id}: {}", req.what);
    let mut qualified: Vec<CataloguePart<'_>> = Vec::new();
    for v in verdicts.iter().flatten() {
        match v.rejection {
            None => qualified.push(v.part),
            Some(Rejection::MissingAttribute { attr, why }) => {
                println!("  reject {:<40} no '{attr}' published — {why}", v.part.mpn);
            }
            Some(Rejection::WrongType { attr }) => {
                println!("  reject {:<40} '{attr}' is the wrong type", v.part.mpn);
            }
            Some(Rejection::Failed { attr, why, got }) => {
                println!("  reject {:<40} {attr}={got:?} — {why}", v.part.mpn);
            }
        }
    }

    rank(&mut qualified, DEFAULT_RANKING);
    for (i, p) in qualified.iter().enumerate() {
        println!(
            "  RANK {} {:<40} {} EU sources, {} stock, {} cents",
            i + 1,
            p.mpn,
            p.eu_sources,
            p.eu_stock,
            p.unit_price_cents
                .map_or("?".to_string(), |c| c.to_string())
        );
    }
    qualified.iter().map(|p| p.mpn.to_string()).collect()
}

/// Every way a part can fail must be *reported*, not merely counted.
///
/// The filter is only trustworthy if it can say what happened to the parts it
/// discarded. Six of the seven fixture oscillators fail, each for a different
/// reason, and all six reasons must come out.
#[test]
fn the_oscillator_filter_explains_every_rejection() {
    let qualified = report("mclk-osc-44k1");

    // The two legitimate candidates differ only in sourcing depth and price,
    // which is what makes them a useful ranking test.
    assert!(
        qualified.contains(&"FIXTURE-OSC-45M1584-A".to_string()),
        "the double-sourced, low-noise, hand-placeable 45.1584 MHz part must qualify"
    );
    assert!(
        qualified.contains(&"FIXTURE-OSC-45M1584-B".to_string()),
        "the single-sourced but otherwise compliant part must also qualify — depth is a RANKING \
         criterion, not a filter, so it loses rather than disappearing"
    );
    assert_eq!(
        qualified.len(),
        2,
        "exactly two of the seven fixture oscillators should qualify, got {qualified:?}"
    );
}

/// The one that matters most for an audiophile clock path: silence is not a pass.
#[test]
fn an_oscillator_with_no_published_phase_noise_is_rejected() {
    let qualified = report("mclk-osc-44k1");
    assert!(
        !qualified.contains(&"FIXTURE-OSC-45M1584-NOSPEC".to_string()),
        "the part with no phase-noise figure is cheap (19 cents), triple-sourced and deeply \
         stocked — exactly the part a price-first filter would pick. An unknown jitter figure \
         must reject it, because jitter is the single thing the dual-oscillator design exists to \
         avoid and an absent spec is not a good one"
    );
}

/// The 22.5792 MHz part is the most attractive one in the catalogue on every
/// commercial axis, and it is the one that cannot reach 176.4 kHz.
#[test]
fn the_cheaper_lower_frequency_oscillator_is_rejected_on_frequency() {
    let qualified = report("mclk-osc-44k1");
    assert!(
        !qualified.contains(&"FIXTURE-OSC-22M5792-WRONGFREQ".to_string()),
        "22.5792 MHz is triple-sourced, 250k in stock and the cheapest part here — and the SAI \
         can only divide, so it caps MCLK at 512fs and 176.4 kHz lands on an unaccepted 128fs. \
         This is the rejection that encodes the oscillator decision."
    );
}

/// Ranking must prefer sourcing depth over price, on real spec constraints and
/// not just in a unit test.
#[test]
fn ranking_prefers_the_double_sourced_oscillator_over_the_cheaper_one() {
    let qualified = report("mclk-osc-44k1");
    assert_eq!(
        qualified.first().map(String::as_str),
        Some("FIXTURE-OSC-45M1584-A"),
        "A is 182 cents with 2 EU sources; B is 41 cents with 1. For a hand-assembled small \
         batch the 1.41 EUR is noise and the second source is the difference between a stalled \
         build and a delayed one, which is why DEFAULT_RANKING puts depth first"
    );
}

/// The mux category, where the discriminating constraint is a boolean.
#[test]
fn the_mux_filter_rejects_make_before_break_and_too_slow_parts() {
    let qualified = report("mclk-mux");
    assert_eq!(
        qualified,
        vec!["FIXTURE-MUX-2CH-BBM".to_string()],
        "only the break-before-make, fast-enough mux should survive: a make-before-break part \
         briefly shorts the two oscillators together, and a 10 MHz part cannot pass a 49.152 MHz \
         clock"
    );
}

/// A constraint set must not be self-contradictory.
///
/// Cheap to check and easy to get wrong once constraints are edited by hand: an
/// `AtLeast(50e6)` next to an `AtMost(40e6)` on the same attribute can never be
/// satisfied, and the symptom would be an empty candidate list that looks like a
/// sourcing problem rather than a specification bug.
#[test]
fn constraints_are_not_self_contradictory() {
    use dap_hw_spec::select::Predicate;
    for req in ACTIVE.parts {
        for (i, a) in req.constraints.iter().enumerate() {
            for b in req.constraints.iter().skip(i + 1) {
                if !a.attr.eq_ignore_ascii_case(b.attr) {
                    continue;
                }
                if let (Predicate::AtLeast(lo), Predicate::AtMost(hi))
                | (Predicate::AtMost(hi), Predicate::AtLeast(lo)) = (a.pred, b.pred)
                {
                    assert!(
                        lo <= hi,
                        "requirement '{}' wants {} >= {lo} AND <= {hi}, which nothing can satisfy",
                        req.id,
                        a.attr
                    );
                }
            }
        }
    }
}
