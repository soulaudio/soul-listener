//! Sourcing gates: the spec's part requirements must be *buyable*, in the EU,
//! from the committed snapshot.
//!
//! # Why the snapshot is committed and read offline
//!
//! A gate that queries a distributor API is not a gate. It gives a different
//! answer on every run, fails when someone rate-limits it, and needs credentials
//! in CI. That is not hypothetical here: `hardware.yml` is **currently broken**
//! for exactly this reason — atopile's component picker calls a remote parts API
//! during the build, the request fails to resolve, and the job dies in an
//! upstream error handler.
//!
//! So the split is: the fetcher runs locally with credentials and writes
//! `hardware/bom/snapshot.json`; CI only ever *reads* that file. Re-retrieval is
//! a deliberate act whose result lands in a reviewable diff.
//!
//! # What these gates can and cannot tell you
//!
//! They check that every requirement the spec commits to has candidates that are
//! orderable in the EU, that nothing load-bearing is end-of-life, and that the
//! data is not so old it is fiction. They cannot check that a part is
//! *functionally* right — research is explicit that stock APIs
//! "can tell you what's in stock, but not that this part would work instead".
//! Functional equivalence is what `PartReq.what` is for, and it stays a human
//! judgement.

use dap_hw_spec::v2::SINGLE_SOURCE_ACCEPTED;
use dap_hw_spec::{Preference, ACTIVE};
use serde_json::Value;

const SNAPSHOT: &str = include_str!("../../../hardware/bom/snapshot.json");

/// Days after which the snapshot is treated as stale.
///
/// Stock moves faster than this, so the gate is not claiming the numbers are
/// live — it is claiming they are recent enough to reason about. Lifecycle
/// status (the figure that actually matters for a design decision) moves slowly,
/// which is why the window is generous rather than days.
const MAX_AGE_DAYS: i64 = 120;

fn snapshot() -> Value {
    serde_json::from_str(SNAPSHOT).expect("hardware/bom/snapshot.json must be valid JSON")
}

/// Crude date arithmetic on `YYYY-MM-DD`, good enough for an age check and
/// dependency-free. Returns days between two dates assuming a 30.44-day month.
fn approx_days_between(earlier: &str, later: &str) -> i64 {
    fn parts(s: &str) -> (i64, i64, i64) {
        let mut it = s.split('-').map(|p| p.parse::<i64>().unwrap_or(0));
        (
            it.next().unwrap_or(0),
            it.next().unwrap_or(0),
            it.next().unwrap_or(0),
        )
    }
    let (y1, m1, d1) = parts(earlier);
    let (y2, m2, d2) = parts(later);
    (y2 - y1) * 365 + (m2 - m1) * 30 + (d2 - d1)
}

/// The snapshot must be well-formed and self-describing.
///
/// Checked before anything else, because a gate that silently reads `{}` and
/// passes is worse than no gate.
#[test]
fn the_snapshot_is_well_formed_and_declares_its_schema() {
    let s = snapshot();
    assert_eq!(
        s["schema_version"].as_u64(),
        Some(1),
        "unexpected snapshot schema version — the gates below assume v1 field names"
    );
    assert!(
        s["generated_at"].as_str().is_some_and(|d| d.len() == 10),
        "snapshot must record when it was generated, as YYYY-MM-DD"
    );
    assert!(
        s["parts"].as_object().is_some_and(|p| !p.is_empty()),
        "snapshot has no parts — an empty snapshot would let every gate below pass vacuously"
    );
    assert!(
        s["distributors"].as_object().is_some_and(|d| !d.is_empty()),
        "snapshot must describe the distributors its offers refer to"
    );
}

/// Every offer must name a distributor the snapshot describes, and every
/// distributor must say whether it ships from the EU.
///
/// EU sourcing is a stated project preference, so it is data rather than a
/// habit: an offer from a non-EU warehouse is still recorded, but it cannot
/// satisfy the EU rule below.
#[test]
fn every_offer_names_a_described_eu_aware_distributor() {
    let s = snapshot();
    let dists = s["distributors"].as_object().unwrap();
    for (name, d) in dists {
        assert!(
            d["eu"].as_bool().is_some(),
            "distributor '{name}' does not say whether it ships from the EU"
        );
        assert!(
            d["warehouse"].as_str().is_some(),
            "distributor '{name}' does not record which warehouse serves EU orders — 'Mouser' \
             with a US warehouse and 'Mouser' with a DE warehouse are different lead times"
        );
    }
    for (mpn, part) in s["parts"].as_object().unwrap() {
        for offer in part["offers"].as_array().unwrap_or(&vec![]) {
            let d = offer["distributor"].as_str().unwrap_or("");
            assert!(
                dists.contains_key(d),
                "{mpn}: offer names distributor '{d}', which the snapshot does not describe"
            );
        }
    }
}

/// Offers must be provenance-tagged, and dated.
///
/// `manual-research` and `api` are never merged: a figure a human read off a
/// product page carries different confidence than one an API returned, and after
/// a few months nobody remembers which was which.
#[test]
fn every_offer_is_provenance_tagged_and_dated() {
    let s = snapshot();
    let generated = s["generated_at"].as_str().unwrap();
    for (mpn, part) in s["parts"].as_object().unwrap() {
        for offer in part["offers"].as_array().unwrap_or(&vec![]) {
            let src = offer["source"].as_str().unwrap_or("");
            assert!(
                src == "api" || src == "manual-research",
                "{mpn}: offer source must be 'api' or 'manual-research', got {src:?}"
            );
            let at = offer["retrieved_at"].as_str().unwrap_or("");
            assert!(
                at.len() == 10,
                "{mpn}: offer has no retrieved_at date; an undated stock figure is not evidence"
            );
            let age = approx_days_between(at, generated);
            assert!(
                age <= MAX_AGE_DAYS,
                "{mpn}: offer from {at} is ~{age} days older than the snapshot's {generated} \
                 (limit {MAX_AGE_DAYS}). Re-fetch rather than reasoning from stale sourcing data."
            );
        }
    }
}

/// Every candidate the spec names must appear in the snapshot — or be rejected.
///
/// A candidate nobody has priced is not a candidate; it is a hope. `Rejected`
/// ones are exempt on purpose: the reason they were ruled out is recorded in the
/// spec, and chasing stock for a part we will not buy is waste.
#[test]
fn every_viable_candidate_is_in_the_snapshot() {
    let s = snapshot();
    let parts = s["parts"].as_object().unwrap();
    let mut missing: Vec<String> = Vec::new();

    for req in ACTIVE.parts {
        for cand in req.candidates {
            if matches!(cand.preference, Preference::Rejected(_)) {
                continue;
            }
            match parts.get(cand.mpn) {
                None => missing.push(format!(
                    "{} (for '{}', {:?}) — no entry at all",
                    cand.mpn, req.id, cand.preference
                )),
                Some(part) => {
                    let has_offers = part["offers"].as_array().is_some_and(|o| !o.is_empty());
                    let pending = part["sourcing_pending"].as_object();
                    if !has_offers {
                        // No offers is allowed ONLY as an explicit, reasoned record.
                        // That turns "nobody looked" from an absent key into a
                        // visible claim, which is the difference between a gap and
                        // a blind spot.
                        match pending {
                            Some(p)
                                if p.get("reason")
                                    .and_then(|r| r.as_str())
                                    .is_some_and(|r| r.len() > 20) =>
                            {
                                if !ACTIVE.is_soft() {
                                    missing.push(format!(
                                        "{} (for '{}') — sourcing_pending is not accepted on a \
                                         HARD-locked revision",
                                        cand.mpn, req.id
                                    ));
                                }
                            }
                            _ => missing.push(format!(
                                "{} (for '{}', {:?}) — no offers and no sourcing_pending reason",
                                cand.mpn, req.id, cand.preference
                            )),
                        }
                    }
                }
            }
        }
    }

    assert!(
        missing.is_empty(),
        "these viable candidates have no sourcing data:\n  {}\n\nRun the fetcher to price them, \
         or mark them Rejected with a reason if they are not real options. A candidate nobody \
         has priced cannot be relied on as a second source.",
        missing.join("\n  ")
    );
}

/// A preferred part must be orderable in the EU today.
///
/// The gate that would have caught the `-CNZR` trap: CS43131-CNZ and
/// CS43131-CNZR are the same die, so a BOM naming either looks correct — but
/// -CNZR is not stocked at Mouser. Suffixes decide availability, and only data
/// catches that.
#[test]
fn preferred_parts_are_orderable_from_an_eu_warehouse() {
    let s = snapshot();
    let dists = s["distributors"].as_object().unwrap();
    let parts = s["parts"].as_object().unwrap();

    for req in ACTIVE.parts {
        let Some(preferred) = req.preferred() else {
            // Soft lock: a requirement may still be Evaluating with no preferred
            // part. The spec's own invariants govern that; this gate only asks
            // that a *chosen* part is buyable.
            continue;
        };
        let Some(part) = parts.get(preferred.mpn) else {
            // Covered by `every_viable_candidate_is_in_the_snapshot`; not
            // duplicated into a second failure here.
            continue;
        };

        // Soft lock: a preferred part may legitimately be un-sourced yet, as long
        // as the gap is declared. `every_viable_candidate_is_in_the_snapshot`
        // enforces that the declaration exists and has a reason; this gate only
        // judges parts that CLAIM to be sourced. On a hard lock the exemption
        // disappears, so freezing the board forces the fetch to have happened.
        if part["sourcing_pending"].is_object() && ACTIVE.is_soft() {
            continue;
        }

        let eu_stocked: Vec<String> = part["offers"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter(|o| {
                let d = o["distributor"].as_str().unwrap_or("");
                let eu = dists
                    .get(d)
                    .and_then(|x| x["eu"].as_bool())
                    .unwrap_or(false);
                eu && o["stock"].as_i64().unwrap_or(0) > 0
            })
            .map(|o| {
                format!(
                    "{} ({})",
                    o["distributor"].as_str().unwrap_or("?"),
                    o["stock"].as_i64().unwrap_or(0)
                )
            })
            .collect();

        assert!(
            !eu_stocked.is_empty(),
            "requirement '{}' prefers {} but no EU distributor in the snapshot has stock.\n\
             Resolve by one of:\n  \
             (a) promote an Alternate that is stocked;\n  \
             (b) re-fetch — the snapshot may predate a restock;\n  \
             (c) check the part suffix: {} and a tape-and-reel variant of the same die can have \
             completely different EU availability, which is exactly how a correct-looking BOM \
             becomes unbuildable.",
            req.id,
            preferred.mpn,
            preferred.mpn
        );
    }
}

/// A preferred part must have EU sourcing DEPTH, not just availability.
///
/// This is the gate the `-CNZR` case actually calls for, and writing it
/// corrected a claim of mine. I had described CS43131-CNZR as a "sourcing trap"
/// that would make a BOM unbuildable. It is not: Digi-Key stocks it (221 units),
/// so a BOM naming it can be built. Single-sourcing is a single point of
/// failure, and research puts the cost concretely: an AVL with 2-3 second
/// sources cuts shortage delays by roughly 80%.
///
/// CORRECTION 2026-11-24: this docstring used to add "`-CNZ` is stocked at two
/// EU distributors, `-CNZR` at one". The Mouser API refutes it — Mouser stocks
/// NEITHER variant (reel-only, zero on hand), so both are single-sourced and the
/// suffix carries no sourcing difference. The second source was a hand-researched
/// 141-unit figure that no API ever confirmed, which is precisely the kind of
/// number this gate exists to stop the spec resting on.
///
/// Depth is counted across DISTRIBUTORS for one MPN, which is a different axis
/// from the spec's `Alternate` candidates (different MPNs for one requirement).
/// A design wants both: two ways to buy the same part, and a different part to
/// fall back to.
#[test]
fn preferred_parts_have_more_than_one_eu_source() {
    let s = snapshot();
    let dists = s["distributors"].as_object().unwrap();
    let parts = s["parts"].as_object().unwrap();

    for req in ACTIVE.parts {
        let Some(preferred) = req.preferred() else {
            continue;
        };
        let Some(part) = parts.get(preferred.mpn) else {
            continue;
        };
        if part["sourcing_pending"].is_object() && ACTIVE.is_soft() {
            continue;
        }
        // An explicitly declared acceptance — resolution (c) below. Skipped here
        // rather than asserted away, and kept honest by
        // `accepted_single_sourcing_is_still_warranted`, which fails the build if
        // an entry stops being true.
        if SINGLE_SOURCE_ACCEPTED
            .iter()
            .any(|(mpn, _)| *mpn == preferred.mpn)
        {
            continue;
        }

        let empty = vec![];
        let sources: Vec<&str> = part["offers"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .filter(|o| {
                let d = o["distributor"].as_str().unwrap_or("");
                dists
                    .get(d)
                    .and_then(|x| x["eu"].as_bool())
                    .unwrap_or(false)
                    && o["stock"].as_i64().unwrap_or(0) > 0
            })
            .filter_map(|o| o["distributor"].as_str())
            .collect();

        assert!(
            sources.len() >= 2,
            "requirement '{}' prefers {}, which only {} EU distributor(s) stock ({:?}).\n\
             Two sources for the same MPN is the AVL rule: an AVL with 2-3 second sources cuts \
             shortage delays by roughly 80%.\nResolve by one of:\n  \
             (a) prefer a variant with broader stock — a tray/tube suffix is often stocked where \
             tape-and-reel is not, and vice versa, for the SAME die;\n  \
             (b) re-fetch, since this may be a transient stock-out;\n  \
             (c) accept it explicitly by adding (mpn, reason) to \
             `v2::SINGLE_SOURCE_ACCEPTED` — the reason is the audit record, and the entry \
             must be deleted once a second distributor stocks the part.",
            req.id,
            preferred.mpn,
            sources.len(),
            sources
        );
    }
}

/// Nothing load-bearing may be end-of-life or not-recommended.
///
/// `unknown` is tolerated while the lock is soft and the data is partly
/// hand-entered; `eol`/`obsolete`/`nrnd` is not, because designing in a part
/// that is already dying is a decision nobody made on purpose.
#[test]
fn no_preferred_part_is_end_of_life() {
    let s = snapshot();
    let parts = s["parts"].as_object().unwrap();
    for req in ACTIVE.parts {
        let Some(preferred) = req.preferred() else {
            continue;
        };
        let Some(part) = parts.get(preferred.mpn) else {
            continue;
        };
        for offer in part["offers"].as_array().unwrap_or(&vec![]) {
            let lc = offer["lifecycle"]
                .as_str()
                .unwrap_or("unknown")
                .to_lowercase();
            assert!(
                !matches!(lc.as_str(), "eol" | "obsolete" | "nrnd" | "discontinued"),
                "requirement '{}' prefers {}, which {} reports as '{}'. Designing in a dying part \
                 is a decision — make it explicitly by promoting an Alternate, or record why the \
                 risk is accepted.",
                req.id,
                preferred.mpn,
                offer["distributor"].as_str().unwrap_or("?"),
                lc
            );
        }
    }
}

/// Compliance claims must carry a date and the regulation revision.
///
/// Per IPC-1752A practice: a bare "RoHS: yes" is not durable. A claim made in
/// 2019 says nothing about today's exemption list, and an exemption a part
/// relies on has an expiry. Storing the claim with its date and the list version
/// is what lets a part be re-checked when the rules change without going back to
/// the supplier.
///
/// Empty is allowed for now — the compliance fetch is not implemented — but a
/// claim that EXISTS must be well-formed, so the field cannot degrade into a
/// boolean later.
#[test]
fn compliance_claims_are_dated_and_versioned() {
    let s = snapshot();
    for (mpn, part) in s["parts"].as_object().unwrap() {
        for offer in part["offers"].as_array().unwrap_or(&vec![]) {
            for claim in offer["compliance"].as_array().unwrap_or(&vec![]) {
                for field in ["regulation", "revision", "status", "asserted_at"] {
                    assert!(
                        claim[field].as_str().is_some(),
                        "{mpn}: a compliance claim is missing '{field}'. Per IPC-1752A practice a \
                         claim is (regulation, revision, status, date) — a bare boolean cannot be \
                         re-checked when the rules change."
                    );
                }
            }
        }
    }
}

/// Every accepted single-sourcing entry must still be warranted.
///
/// An exception list is only safe if it cannot outlive its reason. This asserts
/// each entry is still a **Preferred** candidate and still has fewer than two
/// stocking EU distributors, so the list shrinks by itself as sourcing improves
/// instead of quietly excusing a part that no longer needs excusing — and a
/// typo'd MPN fails loudly rather than silently exempting nothing.
#[test]
fn accepted_single_sourcing_is_still_warranted() {
    let s = snapshot();
    let dists = s["distributors"].as_object().unwrap();
    let parts = s["parts"].as_object().unwrap();

    for (mpn, reason) in SINGLE_SOURCE_ACCEPTED {
        assert!(
            reason.len() > 80,
            "SINGLE_SOURCE_ACCEPTED entry {mpn} has a {}-char reason. This field IS the \
             audit record — it has to say which distributors were checked and why the \
             alternatives are worse, or a reviewer cannot tell an accepted risk from an \
             unexamined one.",
            reason.len()
        );

        let req = ACTIVE
            .parts
            .iter()
            .find(|r| r.preferred().is_some_and(|c| c.mpn == *mpn));
        assert!(
            req.is_some(),
            "SINGLE_SOURCE_ACCEPTED names {mpn}, which is not the Preferred candidate of any \
             requirement. Either the MPN is a typo — in which case it exempts nothing and the \
             real part is unguarded — or the preference moved on and the entry is stale. \
             Delete it."
        );

        let Some(part) = parts.get(*mpn) else {
            panic!(
                "SINGLE_SOURCE_ACCEPTED names {mpn}, which the sourcing snapshot does not \
                 contain. An acceptance has to be checkable against real offers."
            )
        };
        let empty = vec![];
        let stocking: Vec<&str> = part["offers"]
            .as_array()
            .unwrap_or(&empty)
            .iter()
            .filter(|o| {
                let d = o["distributor"].as_str().unwrap_or("");
                dists
                    .get(d)
                    .and_then(|x| x["eu"].as_bool())
                    .unwrap_or(false)
                    && o["stock"].as_i64().unwrap_or(0) > 0
            })
            .filter_map(|o| o["distributor"].as_str())
            .collect();
        assert!(
            stocking.len() < 2,
            "SINGLE_SOURCE_ACCEPTED still excuses {mpn}, but {} EU distributors now stock it \
             ({stocking:?}). The exception has outlived its reason: DELETE the entry so the \
             real gate guards the part again.",
            stocking.len()
        );
    }
}
