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

use dap_arch_tests::candidates::{load, raw_for, typed};
use dap_hw_spec::select::{evaluate, AttrValue, CataloguePart, Rejection};
use dap_hw_spec::{Preference, ACTIVE};

// `Raw`, `load`, `raw_for` and `typed` used to be defined here, and in every other
// test file that needed them. They live in `dap_arch_tests::candidates` now: turning a
// vendor string into the typed value a constraint reads is the step that DECIDES
// whether a part qualifies, and three copies of it were three chances for the gates
// to disagree with each other and with the sourcing report.

const DAC_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/dac.json");
const BT_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/bluetooth.json");
const MUX_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/mclk-mux.json");
const OSC_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/mclk-osc-44k1.json");
const MICROSD_CANDIDATES: &str = include_str!("../../../hardware/bom/candidates/microsd.json");

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

/// Mouser must stay a commerce source: no parametric attributes, no prose parsing.
///
/// # The finding this pins
///
/// Mouser's Search API publishes exactly two attribute names — `Packaging` and
/// `Standard Pack Qty` — on both the part-number and keyword endpoints. Every
/// parametric fact is inside `Description`, as an unlabelled positional blob:
///
/// ```text
/// Standard Clock Oscillators MultiVolt ECS-2520MVLC,Active,XO (Standard),
/// 22.5792MHz,Enable/Disable,CMOS,1.8V, 2.5V, 3.3V, 50ppm,-40 C   85 C,
/// Surface Mount,4-SMD,
/// ```
///
/// # Why a gate and not a comment
///
/// "Mouser returns no attributes" looks like an unfinished adapter, and the
/// obvious fix is to split that string on commas. That change would be a
/// SILENT-WRONG-ANSWER change: the field order varies by product category and
/// one value ("1.8V, 2.5V, 3.3V") contains the separator, so a shifted index
/// maps `50ppm` onto a supply voltage and the constraint then PASSES on a
/// fabricated number. A constraint that runs on invented data is worse than one
/// that never runs, because only the second is visible.
///
/// Value interpretation belongs in `dap_hw_spec::parametric`, which is the one
/// tested implementation, and it needs a NAMED field to interpret. Mouser does
/// not provide one, so Mouser contributes source depth, price, MOQ, lead time
/// and datasheet links instead — none of which require a parametric field.
#[test]
fn mouser_contributes_no_parametrics_and_never_parses_description() {
    let src = include_str!("../../../tools/sourcing/adapters/mouser.py");

    assert!(
        src.contains("ATTR_MAP: dict[str, str] = {}"),
        "mouser.py must declare an EMPTY ATTR_MAP. If Mouser has started publishing named \
         parameters, map them here and delete this assertion — but an entry that maps a \
         position in `Description` is not a name mapping."
    );
    assert!(
        src.contains("contributes_parametrics = False"),
        "mouser.py must declare `contributes_parametrics = False`, so a vendor's attribute \
         silence is a stated property rather than something a reader infers from an empty \
         export."
    );

    // The forbidden operation is "slice Description into fields", and a blocklist
    // of spellings does not catch it — a first draft of this gate listed
    // `Description").split` and was defeated by
    // `(p.get("Description") or "").split(",")`, which is the same operation one
    // character apart. So the invariant is stated positively instead.
    //
    // 1. Attributes may come from exactly one place: `map_attrs` over
    //    `ProductAttributes`. Any other producer of `attrs_raw` is inventing data.
    let producers = src.matches("attrs_raw=").count();
    assert_eq!(
        producers, 1,
        "mouser.py has {producers} producers of `attrs_raw`; there must be exactly one, \
         `self.map_attrs([... ProductAttributes ...])`. A second producer is a second \
         source of truth for what a part IS, and the one that reads Description is \
         fabricating it."
    );
    assert!(
        src.contains("attrs_raw=self.map_attrs("),
        "mouser.py's single `attrs_raw` producer must be `self.map_attrs(...)`, so the \
         vendor's parameter NAMES are what get mapped — `map_attrs` cannot be fed anything \
         that has no name."
    );
    let attrs_block = &src[src.find("attrs_raw=self.map_attrs(").unwrap()..];
    assert!(
        attrs_block[..240.min(attrs_block.len())].contains("ProductAttributes"),
        "mouser.py must feed `map_attrs` from `ProductAttributes` — the only container \
         Mouser gives field NAMES in. Feeding it anything derived from `Description` maps a \
         POSITION, not a name."
    );

    // 2. No string splitting anywhere. This adapter needs none: every extraction
    //    it does (stock, lead time, MPN normalisation) uses an anchored regex.
    //    So a `.split(` appearing at all is the forbidden operation arriving,
    //    whatever it is spelled like.
    assert!(
        !src.contains(".split("),
        "mouser.py contains `.split(`. This adapter has no legitimate need to split a \
         string — stock, lead time and MPN normalisation all use anchored regexes — so a \
         split is Mouser's comma-separated `Description` being carved into positional \
         fields. The field order varies by product category and one value (\"1.8V, 2.5V, \
         3.3V\") contains the separator, so this assigns meaning by index and the \
         constraint then PASSES on a fabricated number."
    );

    // An unresolved lookup must never be recorded as a stock-out. Mouser 503s in
    // clusters that outlast four retries, and the parts affected ROTATE between
    // runs — so a zero written for a failed lookup is indistinguishable from a
    // real shortage, which is the false negative this pipeline keeps finding.
    assert!(
        src.contains("self.unresolved"),
        "mouser.py must keep unresolved lookups separate from absence: a part whose lookup \
         FAILED is not a part the vendor does not carry, and writing a 0-stock row for it \
         would read as a shortage forever."
    );
    assert!(
        src.contains("except OSError as e:"),
        "mouser.py must retry on OSError, not just urllib.error.URLError: a READ timeout \
         arrives as a bare TimeoutError straight from the socket and otherwise escapes the \
         retry loop and aborts the whole run."
    );
}

/// Every vendor's request pacing must be declared, including "no published cap".
///
/// Mouser allows 30 requests/minute and 1000/day — the tightest of the three by
/// an order of magnitude. A sweep paced politely for Digi-Key gets throttled
/// halfway through Mouser, and a 429 counted as absence reads exactly like a
/// vendor that does not carry the part.
///
/// The daily budget specifically cannot live in process memory: the cap is per
/// calendar day and a refresh runs several times a day, so an in-process counter
/// starts at zero each time and discovers the cap by being refused.
#[test]
fn every_vendor_declares_its_rate_limit() {
    let src = include_str!("../../../tools/sourcing/throttle.py");
    for vendor in ["mouser", "digikey", "tme"] {
        assert!(
            src.contains(&format!("\"{vendor}\": RateLimit(")),
            "throttle.py has no declared RateLimit for {vendor:?}. A vendor with no entry \
             silently gets the default interval, which makes an unknown cap look like a \
             decision."
        );
    }
    assert!(
        src.contains("per_minute=30, per_day=1000"),
        "throttle.py must carry Mouser's published 30/minute and 1000/day verbatim, so the \
         numbers can be checked against the vendor's documentation rather than reverse \
         engineered from the pacing."
    );
    // EVERY adapter must go through the limiter, not just the one it was written
    // for. `throttle.py` was added for Mouser's 30/minute cap and only Mouser was
    // wired to it; Digi-Key kept making unpaced calls with no daily accounting,
    // hit HTTP 429 mid-run, and lost seven categories — which it reported
    // loudly, but the limiter existed precisely so that would not happen.
    for (vendor, src) in [
        (
            "mouser",
            include_str!("../../../tools/sourcing/adapters/mouser.py"),
        ),
        (
            "digikey",
            include_str!("../../../tools/sourcing/adapters/digikey.py"),
        ),
        (
            "tme",
            include_str!("../../../tools/sourcing/adapters/tme.py"),
        ),
    ] {
        assert!(
            src.contains("self.throttle = Throttle("),
            "{vendor}'s adapter does not construct a Throttle. An unpaced adapter spends quota \
             nothing is counting, and the first symptom is a 429 the sweep cannot distinguish \
             from a vendor refusing the request."
        );
        assert!(
            src.contains("self.throttle.acquire()"),
            "{vendor}'s adapter never calls throttle.acquire(), so constructing the Throttle \
             paces nothing."
        );
        assert!(
            src.contains("Retry-After"),
            "{vendor}'s adapter must honour the vendor's Retry-After header on a 429. Digi-Key \
             classified 429 as a non-retryable 4xx and died at offset 0 on a rate limit that a \
             few seconds of backoff would have cleared."
        );
    }

    assert!(
        include_str!("../../../tools/sourcing/db.py").contains("def calls_today"),
        "db.py must be able to report calls already spent today per vendor. Without it the \
         daily budget resets every process and the cap is discovered by being refused."
    );
}

/// No spec candidate may owe its verdict to a cross-vendor tie-break.
///
/// # The fragility this pins
///
/// `merge_values` must produce one value per attribute, and when two vendors
/// disagree *after* canonicalisation it picks the alphabetically-first vendor's.
/// That is deterministic, which was the point — it replaced a winner chosen by
/// SQL row order — but it is also arbitrary, and for a discriminating attribute
/// arbitrary is not good enough.
///
/// The live case: `ASV-24.576MHZ-EJ-T` is Abracon's XO family, and TME publishes
/// its `part_type` as `'crystal'` where Digi-Key says `'oscillator'`. TME is
/// simply wrong. The oscillator requirement rejects anything that is not an
/// active clock source, so this candidate qualifies **only** because `digikey`
/// sorts before `tme`. Rename a vendor and a recorded candidate silently stops
/// qualifying.
///
/// So: a Preferred or Alternate candidate must not have a conflict on an
/// attribute its own requirement constrains. An `Evaluating` candidate may —
/// that is what evaluating means, and the conflict is exactly the kind of thing
/// evaluation should surface — but it has to be stated in the candidate's note,
/// which `ASV-24.576MHZ-EJ-T`'s is.
#[test]
fn no_spec_candidate_depends_on_a_conflicted_attribute() {
    let exports: &[(&str, &str)] = &[
        ("dac", DAC_CANDIDATES),
        ("bluetooth", BT_CANDIDATES),
        ("mclk-mux", MUX_CANDIDATES),
        ("mclk-osc-44k1", OSC_CANDIDATES),
    ];

    for req in ACTIVE.parts {
        let Some((_, doc)) = exports.iter().find(|(id, _)| *id == req.id) else {
            continue;
        };
        let constrained: Vec<&str> = req.constraints.iter().map(|c| c.attr).collect();
        for raw in load(doc, req.id) {
            let Some(cand) = req.candidates.iter().find(|c| c.mpn == raw.mpn) else {
                continue;
            };
            if !matches!(
                cand.preference,
                Preference::Preferred | Preference::Alternate
            ) {
                continue;
            }
            for attr in &raw.conflicts {
                // `_min`/`_max` constraints are satisfied from the bare
                // attribute, so a conflict on `supply_mv` bears on
                // `supply_mv_min` too.
                let bears = constrained.iter().any(|c| {
                    *c == attr.as_str()
                        || c.strip_suffix("_min").or_else(|| c.strip_suffix("_max"))
                            == Some(attr.as_str())
                });
                assert!(
                    !bears,
                    "requirement '{}' lists {} as {:?}, but its vendors DISAGREE about '{attr}' \
                     — an attribute this requirement constrains. The export resolves that by \
                     taking the alphabetically-first vendor's value, so whether this candidate \
                     qualifies depends on vendor name order rather than on the part.\n\
                     Resolve by one of:\n  \
                     (a) establish which vendor is right and canonicalise the other's spelling \
                     in tools/sourcing/canon.py, if it is a vocabulary gap;\n  \
                     (b) demote the candidate to Evaluating and record the disagreement in its \
                     note, if the vendors genuinely contradict each other;\n  \
                     (c) drop the candidate.",
                    req.id, cand.mpn, cand.preference
                );
            }
        }
    }
}

/// A PICKED part must pass the requirement it was picked for.
///
/// # Why this is the gate that makes switching components cheap
///
/// Choosing a part means setting `Preference::Preferred` in the spec. Nothing
/// else — there is no second place to update, which is the whole point of the
/// requirement/candidate split. But that only stays safe if changing the pick is
/// immediately checked against the requirement's own constraints, otherwise a
/// swap can quietly install a part that does not meet the spec it was swapped
/// into, and the next thing to notice would be a board.
///
/// So: every Preferred and Alternate candidate is run through its own
/// requirement's filter, from the same exported catalogue the gates and the
/// proposals use. Swap an MPN, run the tests, and either it qualifies or you get
/// told which constraint it fails and why that constraint exists.
///
/// A part missing from the catalogue entirely is NOT failed here — that is
/// `every_viable_candidate_is_in_the_snapshot`'s job, and failing it twice would
/// make one problem look like two.
#[test]
fn every_picked_part_qualifies_against_its_own_requirement() {
    let exports: &[(&str, &str)] = &[
        ("dac", DAC_CANDIDATES),
        ("bluetooth", BT_CANDIDATES),
        ("mclk-mux", MUX_CANDIDATES),
        ("mclk-osc-44k1", OSC_CANDIDATES),
        ("microsd", MICROSD_CANDIDATES),
    ];

    let mut checked = 0;
    for req in ACTIVE.parts {
        let Some((_, doc)) = exports.iter().find(|(id, _)| *id == req.id) else {
            continue;
        };
        let raws = load(doc, req.id);
        for cand in req.candidates {
            if !matches!(
                cand.preference,
                Preference::Preferred | Preference::Alternate
            ) {
                continue;
            }
            let Some(raw) = raws.iter().find(|r| r.mpn == cand.mpn) else {
                continue;
            };
            let attrs = typed(raw, req.constraints);
            let p = dap_arch_tests::candidates::part(raw, &attrs);
            let mut slot = [None];
            evaluate(&[p], req.constraints, &mut slot);
            let verdict = slot[0].expect("one verdict per part");
            checked += 1;
            assert!(
                verdict.qualified(),
                "requirement '{}' picks {} as {:?}, but it FAILS that requirement's own \
                 constraints: {}\n\nA pick is made by setting Preference in the spec and \
                 nothing else, so this is the check that makes swapping a component safe. \
                 Either the part is wrong for the requirement, or the constraint is wrong \
                 about the requirement — both are decisions, and neither should be discovered \
                 from a fabricated board.",
                req.id,
                cand.mpn,
                cand.preference,
                verdict
                    .rejection
                    .as_ref()
                    .map(|r| format!("{} — {}", r.attr(), rejection_why(r)))
                    .unwrap_or_default()
            );
        }
    }
    assert!(
        checked >= 4,
        "only {checked} picked parts were checked against their own constraints — the export \
         list in this test has gone stale, so the gate is guarding less than it appears to"
    );
}

fn rejection_why<'a>(r: &Rejection<'a>) -> &'a str {
    match r {
        Rejection::MissingAttribute { why, .. } | Rejection::Failed { why, .. } => why,
        Rejection::WrongType { .. } => "published but unparseable",
    }
}
