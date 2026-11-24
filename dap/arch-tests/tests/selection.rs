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
/// Rewritten against the real vendor vocabulary. TME's "Resonators and
/// Generators" category mixes passive quartz with active oscillators, publishes
/// supply as an operating range under "Operating voltage", and describes
/// mounting rather than package codes — so these are the failures a catalogue
/// can actually judge.
#[test]
fn the_oscillator_filter_explains_every_rejection() {
    let qualified = report("mclk-osc-44k1");

    assert!(
        qualified.contains(&"FIXTURE-OSC-22M5792-A".to_string()),
        "the double-sourced active oscillator must qualify"
    );
    assert!(
        qualified.contains(&"FIXTURE-OSC-22M5792-B".to_string()),
        "the single-sourced but compliant part must also qualify — depth is a RANKING criterion, \
         not a filter, so it loses rather than disappearing"
    );
    assert_eq!(
        qualified.len(),
        2,
        "exactly the two compliant active oscillators should qualify, got {qualified:?}"
    );
}

/// A passive crystal must lose to an active oscillator, cheaply though it comes.
///
/// This is the rejection the real catalogue forced into existence. TME returns
/// crystals, ceramics and resonators alongside oscillators and XOs in one
/// category, and the passive ones are consistently cheaper and deeper-stocked —
/// the fixture crystal is 12 cents with 250k in stock against 182 cents for the
/// part that qualifies.
///
/// A crystal is not a worse oscillator; it is a different component. It has no
/// output driver, so it cannot present a clock to the DAC's XTI pin and the
/// SAI's clock input at the same time, which is the entire topology.
#[test]
fn a_passive_crystal_is_rejected_however_cheap_and_well_stocked() {
    let qualified = report("mclk-osc-44k1");
    for passive in ["FIXTURE-XTAL-22M5792", "FIXTURE-CERAMIC-22M5792"] {
        assert!(
            !qualified.contains(&passive.to_string()),
            "{passive} is passive quartz with no output driver. It is the cheapest, \
             deepest-stocked, most-sourced part in the category and it must still lose: one \
             output has to drive both the DAC and the SAI"
        );
    }
}

/// Phase noise is a datasheet obligation, not a catalogue filter.
///
/// This replaces a test asserting the opposite, and the correction came from
/// data: a survey of **all 121 parts** TME returns for 22.5792 and 24.576 MHz
/// found no phase-noise or jitter parameter of any kind — not empty values, no
/// such field.
///
/// The principle that unknown jitter is disqualifying still holds. What changed
/// is where it is enforced: as a catalogue constraint it rejected every real
/// oscillator for a field no distributor publishes, which reads as "nothing is
/// sourceable" rather than "ask the datasheet".
#[test]
fn phase_noise_is_a_datasheet_obligation_rather_than_a_filter() {
    use dap_hw_spec::select::Evidence;
    for id in ["mclk-osc-44k1", "mclk-osc-48k"] {
        let req = ACTIVE
            .parts
            .iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("spec must have {id}"));
        let pn = req
            .constraints
            .iter()
            .find(|c| c.attr == "phase_noise_dbc_1khz")
            .unwrap_or_else(|| panic!("{id} must still constrain phase noise SOMEHOW"));
        assert_eq!(
            pn.evidence,
            Evidence::Datasheet,
            "'{id}' treats phase noise as a catalogue field. No distributor publishes it for this \
             category, so every candidate would be rejected for a missing attribute — the \
             requirement becomes unsatisfiable by construction."
        );
        assert!(
            pn.why.to_lowercase().contains("datasheet"),
            "the reason text must say it is datasheet-only, because that text is what a reader \
             sees when deciding whether the requirement has been checked"
        );
    }
}

/// The 24.576 MHz part is the most attractive one in the catalogue on every
/// commercial axis, is a frequency the DAC fully accepts, and is still wrong.
///
/// This is the rejection that encodes the distinction the spec previously got
/// wrong. 24.576 MHz is a nominal CS43131 MCLK — the DAC would take it for every
/// rate the product supports — so a filter reasoning only about the DAC would
/// pass it. It fails because the *SAI* cannot divide it to an exact 64 x 44100
/// bit clock, and LRCK derived from that clock is the sample rate. The constraint
/// belongs to the MCU's serial port, not to the DAC.
#[test]
fn the_other_familys_clock_is_rejected_even_though_the_dac_accepts_it() {
    let qualified = report("mclk-osc-44k1");
    assert!(
        !qualified.contains(&"FIXTURE-OSC-24M576-WRONGFAMILY".to_string()),
        "24.576 MHz is triple-sourced, 250k in stock, the cheapest part here, and a frequency the \
         CS43131 accepts. It must still be rejected for the 44.1 kHz family: 24.576e6 / (64 x \
         44100) = 8.707..., so no integer SAI divider produces an exact 44.1 kHz LRCK."
    );
}

/// Ranking must prefer sourcing depth over price, on real spec constraints and
/// not just in a unit test.
#[test]
fn ranking_prefers_the_double_sourced_oscillator_over_the_cheaper_one() {
    let qualified = report("mclk-osc-44k1");
    assert_eq!(
        qualified.first().map(String::as_str),
        Some("FIXTURE-OSC-22M5792-A"),
        "A is 182 cents with 2 EU sources; B is 41 cents with 1. For a hand-assembled small \
         batch the 1.41 EUR is noise and the second source is the difference between a stalled \
         build and a delayed one, which is why DEFAULT_RANKING puts depth first"
    );
}

/// The mux category, in the vendor's own vocabulary.
///
/// This test was rewritten after the first live fetch, because the version before
/// it asserted something the filter cannot do. It claimed the filter rejects a
/// make-before-break part and a too-slow part — but **no distributor publishes
/// either fact**. TME returns an empty "Clock frequency" for analog switches and
/// nothing at all for break-before-make, so both are
/// [`Evidence::Datasheet`](dap_hw_spec::select::Evidence::Datasheet) now and the
/// catalogue filter correctly declines to judge them.
///
/// What remains is what a catalogue genuinely answers: switch configuration,
/// channel count in the vendor's convention, supply-range containment, and
/// mounting. Those reject four of the seven fixture parts.
#[test]
fn the_mux_filter_rejects_what_a_catalogue_can_actually_judge() {
    let qualified = report("mclk-mux");

    assert!(
        !qualified.contains(&"FIXTURE-MUX-SP4T-WRONGCONFIG".to_string()),
        "an SP4T is a different class of part: four throws where the design selects between two \
         oscillators"
    );
    assert!(
        !qualified.contains(&"FIXTURE-MUX-DUAL-CHANNEL".to_string()),
        "a dual switch is two channels where one is needed — and 'channels' counts SWITCHES in \
         the vendor's convention, which is the clash that made the old constraint reject the \
         correct part"
    );
    assert!(
        !qualified.contains(&"FIXTURE-MUX-5V-ONLY".to_string()),
        "a part whose operating range starts at 4.5 V cannot run on the 3.3 V rail. Note this is \
         the containment test: it rejects a range that does not REACH 3.3 V, and no longer \
         rejects a part merely for tolerating more"
    );
    assert!(
        !qualified.contains(&"FIXTURE-MUX-THROUGH-HOLE".to_string()),
        "through-hole does not belong on this board"
    );
    assert!(
        !qualified.contains(&"FIXTURE-MUX-NO-SUPPLY-SPEC".to_string()),
        "the cheapest, deepest-stocked, triple-sourced mux publishes no supply range at all. \
         Unknown is not a pass — that rule is still right where a catalogue SHOULD carry the \
         field"
    );

    assert_eq!(
        qualified,
        vec![
            "FIXTURE-MUX-SPDT-A".to_string(),
            "FIXTURE-MUX-SPDT-B".to_string()
        ],
        "both compliant SPDT parts should survive the catalogue filter, ranked by source depth"
    );
}

/// A part rated far above the rail must now QUALIFY, not be rejected.
///
/// The single most important assertion to come out of the first live fetch. The
/// TI `SN74LVC1G3157DCKR` is rated 1.65-5.5 V, has 6916 in stock at TME, and is
/// the part the ranking policy prefers — and the previous `supply_mv_max <=
/// 3300` constraint rejected it for *tolerating more voltage than the board
/// supplies*, which is the opposite of a defect.
///
/// This pins the direction so it cannot silently invert again.
#[test]
fn a_part_rated_above_the_rail_qualifies_rather_than_being_rejected() {
    let owned = load("mclk-mux");
    let parts = borrow(&owned);
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "mclk-mux")
        .expect("spec must have mclk-mux");
    let mut verdicts = vec![None; parts.len()];
    evaluate(&parts, req.constraints, &mut verdicts);

    let a = verdicts
        .iter()
        .flatten()
        .find(|v| v.part.mpn == "FIXTURE-MUX-SPDT-A")
        .expect("the compliant fixture part must be evaluated");
    assert!(
        a.rejection.is_none(),
        "a mux rated 1.65-5.5 V was rejected ({:?}). A maximum supply rating ABOVE the rail is a \
         capability, not a fault — this is the exact error that rejected the real TI part with \
         6916 in stock.",
        a.rejection
    );
}

/// Datasheet-only constraints must be surfaced, not silently skipped.
///
/// The engine deliberately does not judge them, which creates a new risk: a
/// requirement could look fully checked while its defining property — for the
/// mux, break-before-make — was never confirmed by anyone. So the report has to
/// name them as outstanding work.
#[test]
fn datasheet_only_constraints_are_reported_as_outstanding() {
    use dap_hw_spec::select::Evidence;
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "mclk-mux")
        .expect("spec must have mclk-mux");

    let datasheet: Vec<&str> = req
        .constraints
        .iter()
        .filter(|c| c.evidence == Evidence::Datasheet)
        .map(|c| c.attr)
        .collect();

    assert!(
        datasheet.contains(&"break_before_make"),
        "break-before-make is the defining property of this part and no distributor publishes \
         it, so it must be marked Datasheet rather than silently checked against a field that \
         will always be missing"
    );
    for c in req
        .constraints
        .iter()
        .filter(|c| c.evidence == Evidence::Datasheet)
    {
        assert!(
            c.why.to_lowercase().contains("datasheet"),
            "constraint on '{}' is Datasheet-only but its `why` does not say so. The reason text \
             is what a reader sees in the report; without it, an unchecked requirement looks \
             checked.",
            c.attr
        );
    }
    println!("mclk-mux needs datasheet confirmation for: {datasheet:?}");
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

/// The battery filter, where the cheapest, biggest, deepest-stocked cell loses.
///
/// This is the licensing filter with the most money behind it. Two of the
/// rejected cells are commercially the most attractive parts in the category:
///
/// - `FIXTURE-CELL-1200-SOLDERTAB` is the cheapest, has the deepest stock AND the
///   largest capacity. Solder tabs make a spare battery impossible and fail EU
///   Battery Regulation Art. 11 user-replaceability.
/// - `FIXTURE-CELL-1100-BARE` is cheaper than either survivor and triple-sourced.
///   Buying bare cells and adding our own protection circuit makes *us* the
///   battery manufacturer, inheriting UN 38.3 and IEC 62133-2 testing — a
///   five-figure certification programme acquired by a sourcing decision.
///
/// A price-first filter picks both. That is the whole argument for encoding these
/// as constraints rather than leaving them in a document.
#[test]
fn the_battery_filter_rejects_the_cheapest_and_largest_cells_for_licensing_reasons() {
    let qualified = report("battery");

    assert!(
        !qualified.contains(&"FIXTURE-CELL-1200-SOLDERTAB".to_string()),
        "the solder-tab cell is the cheapest, deepest-stocked and highest-capacity part in the \
         category, and must lose: a user cannot unplug it, so there is no spare battery and no \
         Art. 11 user-replaceability"
    );
    assert!(
        !qualified.contains(&"FIXTURE-CELL-1100-BARE".to_string()),
        "the bare cell without a protection circuit must lose even though it is cheap and \
         triple-sourced: protecting it ourselves would transfer UN 38.3 and IEC 62133-2 \
         certification from the vendor to us"
    );
    assert!(
        !qualified.contains(&"FIXTURE-CELL-1100-NODOCS".to_string()),
        "a vendor that will not issue the per-model transport report leaves us holding an \
         undocumented battery; certification is per model, so a general compliance claim is not \
         the document that ships"
    );
    assert!(
        !qualified.contains(&"FIXTURE-CELL-2S-2000".to_string()),
        "a 2S pack is a different power tree, not a bigger battery — the charger, gauge and every \
         rail in the power model assume one cell"
    );
    assert!(
        !qualified.contains(&"FIXTURE-CELL-1800-THICK".to_string()),
        "9 mm does not fit a 7.1 mm bay regardless of how good the cell is"
    );
    assert!(
        !qualified.contains(&"FIXTURE-CELL-600-SMALL".to_string()),
        "600 mAh drops the runtime out of the compact-DAP class the product is measured against"
    );

    assert_eq!(
        qualified.len(),
        2,
        "exactly the two connectorised, PCM-equipped, documented 1S cells should qualify, got \
         {qualified:?}"
    );
}

/// Hot-swap is a *termination* property, and the filter has to treat it as one.
///
/// The product wants users to be able to buy a second cell and swap it. At part
/// level that is exactly one thing: the cell is connectorised rather than
/// soldered. This test pins that the constraint discriminates on that alone — the
/// solder-tab part is otherwise superior on capacity, price and stock, so if it
/// ever qualifies, the swap story is gone and nothing else would notice.
#[test]
fn every_qualifying_cell_can_be_unplugged_by_a_user() {
    let owned = load("battery");
    let parts = borrow(&owned);
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "battery")
        .expect("the spec must have a battery requirement");
    let mut verdicts = vec![None; parts.len()];
    evaluate(&parts, req.constraints, &mut verdicts);

    for v in verdicts.iter().flatten() {
        if v.rejection.is_some() {
            continue;
        }
        let termination = v
            .part
            .attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("termination"))
            .map(|(_, val)| *val);
        match termination {
            Some(AttrValue::Text(t)) => assert!(
                !t.to_lowercase().contains("solder"),
                "{} qualified with a {t} termination. A soldered cell cannot be swapped by a \
                 user, which defeats both the spare-battery feature and Art. 11.",
                v.part.mpn
            ),
            other => panic!(
                "{} qualified without a usable termination attribute ({other:?}) — the \
                 constraint should have rejected it",
                v.part.mpn
            ),
        }
    }
}

/// The Bluetooth filter buys certification inheritance, not parts.
///
/// Every rejection here is a cost avoided rather than a specification missed: a
/// bare SoC at half the price costs a Bluetooth qualification programme and an EU
/// radio certification programme, and an NDA-gated module cannot ship with
/// open-source firmware at all.
#[test]
fn the_bluetooth_filter_requires_inheritable_qualification_and_no_nda() {
    let qualified = report("bluetooth");

    assert!(
        !qualified.contains(&"FIXTURE-BT-BARE-SOC".to_string()),
        "the bare SoC is half the price of the qualified module and triple-sourced. It must lose: \
         with no module QDID and no pre-certified radio, Bluetooth qualification and EU radio \
         certification both become ours to run"
    );
    assert!(
        !qualified.contains(&"FIXTURE-BT-MODULE-NO-QDID".to_string()),
        "a module with no SIG qualification offers nothing to inherit — the end-product \
         Declaration has to reference a QDID"
    );
    assert!(
        !qualified.contains(&"FIXTURE-BT-MODULE-NDA".to_string()),
        "an NDA-gated SDK is incompatible with shipping open-source firmware, which is why this \
         is a hard filter and not a preference"
    );
    assert!(
        !qualified.contains(&"FIXTURE-BT-MODULE-LICENSED-CODECS-ONLY".to_string()),
        "a module without SBC makes the audio path depend on a licensed codec; SBC is the \
         royalty-free baseline the product commits to"
    );

    assert_eq!(
        qualified,
        vec!["FIXTURE-BT-MODULE-QUALIFIED".to_string()],
        "only the pre-qualified, EU-certified, NDA-free, SBC-capable module should survive"
    );
}

/// Every requirement whose failure mode is a licence fee must carry constraints.
///
/// The requirements where getting it wrong costs money rather than a redesign are
/// exactly the ones where prose is least adequate, because the wrong part works
/// perfectly and the bill arrives later. This refuses to let them sit in
/// `NO_CONSTRAINTS`.
#[test]
fn licensing_sensitive_requirements_are_not_prose_only() {
    for id in ["battery", "bluetooth"] {
        let req = ACTIVE
            .parts
            .iter()
            .find(|r| r.id == id)
            .unwrap_or_else(|| panic!("the spec must have a '{id}' requirement"));
        assert!(
            !req.constraints.is_empty(),
            "requirement '{id}' has no machine-checkable constraints. Its failure mode is a \
             certification or licensing cost, not a part that does not work — so a reviewer \
             reading the prose would see nothing wrong. These are the requirements that most need \
             filters."
        );
    }
}

/// A spare battery has to be *buyable*, which is a property of the candidate SET.
///
/// The other licensing constraints are per-part: this one cannot be, and that
/// distinction is worth stating. "Widely available" is not a fact about a cell —
/// a single perfect cell from a single manufacturer satisfies every per-part
/// constraint in the spec and still leaves the user unable to buy a spare when
/// that manufacturer discontinues it. For a *consumable the user replaces*, the
/// supply question outlives the build: a soldered cell only has to be available
/// while we are manufacturing, a user-swappable one has to be available for as
/// long as people own the device.
///
/// So availability is checked over the qualifying set, and it is checked for
/// manufacturer diversity rather than stock depth. Depth is a ranking criterion
/// (`DEFAULT_RANKING` puts source depth first); *independence* is the thing that
/// survives one vendor exiting the market.
#[test]
fn a_spare_cell_is_obtainable_from_more_than_one_manufacturer() {
    let owned = load("battery");
    let parts = borrow(&owned);
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "battery")
        .expect("the spec must have a battery requirement");
    let mut verdicts = vec![None; parts.len()];
    evaluate(&parts, req.constraints, &mut verdicts);

    let mut makers: Vec<&str> = verdicts
        .iter()
        .flatten()
        .filter(|v| v.rejection.is_none())
        .map(|v| v.part.manufacturer)
        .collect();
    makers.sort_unstable();
    makers.dedup();

    assert!(
        makers.len() >= 2,
        "only {} manufacturer(s) ({makers:?}) produce a cell that satisfies the battery \
         constraints. The battery is the one part an END USER re-buys, so single-sourcing it is \
         worse than single-sourcing anything else on the board: when that manufacturer \
         discontinues the model, every device in the field loses its spare supply, and the \
         swappable-battery feature quietly becomes a dead promise.\n\n\
         Resolve by one of:\n  \
         (a) find a second manufacturer's cell that fits the bay and carries its own per-model \
         certification — the preferred answer, and the reason the constraints target a STANDARD \
         pouch size and a STANDARD connector rather than a specific part;\n  \
         (b) relax a dimensional constraint if the bay can afford it (that is a mechanical \
         decision, not a sourcing one);\n  \
         (c) drop the user-swappable-spare claim and solder the cell — which also removes the \
         Art. 11 user-replaceability design and should not be done quietly.",
        makers.len()
    );
}

/// Wide availability must not be bought by relaxing a licensing constraint.
///
/// The two requirements pull against each other on purpose: availability wants
/// the broadest possible candidate set, and the certification constraints
/// deliberately narrow it. The failure mode worth guarding is satisfying the
/// first by weakening the second — admitting a bare cell or a solder-tab part to
/// get the manufacturer count up.
///
/// So this asserts both hold *at once*: every cell counted as available also
/// carries its own protection circuit, its per-model reports, and a connector a
/// user can unplug.
#[test]
fn every_available_cell_also_satisfies_the_certification_constraints() {
    let owned = load("battery");
    let parts = borrow(&owned);
    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "battery")
        .expect("the spec must have a battery requirement");
    let mut verdicts = vec![None; parts.len()];
    evaluate(&parts, req.constraints, &mut verdicts);

    let qualifying: Vec<_> = verdicts
        .iter()
        .flatten()
        .filter(|v| v.rejection.is_none())
        .collect();

    assert!(
        !qualifying.is_empty(),
        "no cell qualifies at all — availability is moot if the constraint set admits nothing"
    );

    for v in qualifying {
        for (attr, expect_true) in [
            ("integrated_pcm", true),
            ("un38_3_report", true),
            ("iec62133_2_report", true),
        ] {
            let val = v
                .part
                .attrs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(attr))
                .map(|(_, val)| *val);
            assert!(
                matches!(val, Some(AttrValue::Bool(b)) if b == expect_true),
                "{} is counted as an available spare but its '{attr}' is {val:?}. Widening the \
                 candidate set must not come from relaxing the certification constraints: a bare \
                 or undocumented cell makes US the battery manufacturer, which is a five-figure \
                 certification programme rather than a sourcing inconvenience.",
                v.part.mpn
            );
        }
    }
}

/// The connector allow-list must contain only industry-standard connectors.
///
/// This is what makes "widely available" achievable rather than aspirational.
/// Availability of a spare depends on the *interface* being standard: a cell with
/// a proprietary moulded connector can be the best cell in the world and still
/// leave the user with exactly one supplier. JST PH 2.0 is the de-facto standard
/// for single-cell Li-Po, which is why the board's connector choice is itself a
/// sourcing decision.
#[test]
fn the_accepted_battery_connectors_are_industry_standards() {
    use dap_hw_spec::select::Predicate;

    // Deliberately a literal list rather than a reference to the spec's own
    // constant: this test is asking "are these standards?", and importing the
    // answer from the thing under test would make it tautological.
    const INDUSTRY_STANDARD: &[&str] = &[
        "JST-PH-2.0",
        "JST-PH",
        "JST-XH",
        "molex-picoblade",
        "molex-pico-ezmate",
        "connector",
    ];

    let req = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "battery")
        .expect("the spec must have a battery requirement");
    let termination = req
        .constraints
        .iter()
        .find(|c| c.attr.eq_ignore_ascii_case("termination"))
        .expect("the battery requirement must constrain termination");

    let Predicate::OneOf(allowed) = termination.pred else {
        panic!(
            "the termination constraint is no longer a OneOf list, so the set of accepted \
             connectors cannot be checked against the industry standards"
        )
    };

    for a in allowed {
        assert!(
            INDUSTRY_STANDARD.iter().any(|s| s.eq_ignore_ascii_case(a)),
            "'{a}' is accepted as a battery termination but is not in the industry-standard list. \
             A spare cell is only widely available if its connector is: a proprietary or \
             vendor-specific termination leaves exactly one supplier, no matter how good the cell \
             is. If '{a}' really is a standard, add it to INDUSTRY_STANDARD here with a note — do \
             not widen only the spec side, because then nothing is comparing the two."
        );
    }
    assert!(
        !allowed.is_empty(),
        "the termination allow-list is empty, so no cell can qualify"
    );
}
