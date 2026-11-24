//! The selection engine: catalogue -> filter -> rank -> candidates.
//!
//! # What this is for
//!
//! [`crate::PartReq::what`] states a requirement in prose, which is right for a
//! human and useless to a machine. "45.1584 MHz fixed oscillator, <=3.3 V, low
//! phase noise, SMD" cannot reject a part; it can only be read and agreed with.
//!
//! [`Constraint`] is the machine-checkable half of the same sentence. Given a
//! catalogue of real parts with parametric attributes — the shape distributor
//! APIs return — the engine answers two questions the prose cannot:
//!
//! * **which parts qualify**, and for each rejected one, *which constraint it
//!   failed and why that constraint exists*;
//! * **in what order** the qualifying ones should be preferred.
//!
//! The rejection reason is the point. A filter that returns 3 of 400 parts is
//! not trustworthy unless it can say what happened to the other 397.
//!
//! # Why attribute-keyed rather than a closed enum of quantities
//!
//! An earlier sketch had `Constraint::FrequencyHz`, `Constraint::MaxSupplyMv`
//! and so on. That does not survive contact with a real catalogue: distributor
//! parametric data is an open set of named fields that differs per category, and
//! every new category would need new enum variants in this crate. So a
//! constraint is `(attribute name, predicate)`, which matches how the data
//! actually arrives and keeps the engine category-agnostic.
//!
//! # No ranking by "best"
//!
//! [`Rank`] criteria are explicit and ordered by the caller. There is no hidden
//! composite score, because a single number would bury the thing that matters:
//! that this project ranks EU stock depth above unit price, and says so.

use core::cmp::Ordering;

/// One parametric attribute value, as a catalogue carries it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AttrValue<'a> {
    Num(f64),
    Text(&'a str),
    Bool(bool),
}

impl AttrValue<'_> {
    fn as_num(&self) -> Option<f64> {
        match self {
            Self::Num(n) => Some(*n),
            _ => None,
        }
    }
}

/// A test applied to one attribute.
#[derive(Debug, Clone, Copy)]
pub enum Predicate<'a> {
    /// Exactly this number. Uses a relative tolerance, because a catalogue may
    /// carry 22.5792 MHz as 22579200, 22.5792e6 or 22579200.000001 depending on
    /// who parsed it, and an oscillator that is off by one part in 10^9 is the
    /// same oscillator.
    NumEq {
        value: f64,
        rel_tol: f64,
    },
    AtMost(f64),
    AtLeast(f64),
    /// Text equal to one of these, case-insensitively.
    OneOf(&'a [&'a str]),
    IsTrue,
    /// The attribute must be present AND false.
    ///
    /// Not the same as omitting the constraint, and not the same as `IsTrue`
    /// negated by the caller: several real requirements are negative facts that
    /// must be *published* rather than assumed — "this module needs no vendor
    /// NDA", "this cell is not soldered in". An absent attribute stays a
    /// rejection under this predicate, for the same reason it does under the
    /// others: unknown is not a pass.
    IsFalse,
}

/// Who publishes the fact a constraint tests.
///
/// This distinction was forced by real data. The first filter run against live
/// TME responses would have rejected the *correct* mux three times over, and one
/// of the three reasons was `break_before_make` — a property **no distributor
/// publishes**, because it is a datasheet characteristic rather than a
/// parametric search field.
///
/// The engine treats a missing attribute as a rejection, which is right when the
/// attribute is one a catalogue should supply (an oscillator with no published
/// phase noise really is disqualified). Applied to a fact no catalogue carries,
/// the same rule makes the requirement unsatisfiable by construction: every part
/// in the category is rejected, and the output reads like a sourcing problem
/// rather than a specification bug.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Evidence {
    /// Distributors publish this as a parametric field. The filter checks it, and
    /// its absence from a part is a rejection.
    Catalogue,
    /// Only the datasheet states it. The filter CANNOT check it and must not
    /// reject a part for its absence — it has to be confirmed by a human and
    /// recorded against the candidate.
    Datasheet,
}

/// A requirement expressed so a machine can reject a part.
#[derive(Debug, Clone, Copy)]
pub struct Constraint<'a> {
    /// Catalogue attribute name.
    pub attr: &'a str,
    pub pred: Predicate<'a>,
    /// Why this constraint exists. Carried into the rejection so a filtered-out
    /// part explains itself rather than merely disappearing.
    pub why: &'a str,
    /// Whether a catalogue can answer this at all. See [`Evidence`].
    pub evidence: Evidence,
}

/// A catalogue part: identity plus parametric attributes.
#[derive(Debug, Clone, Copy)]
pub struct CataloguePart<'a> {
    pub mpn: &'a str,
    pub manufacturer: &'a str,
    pub attrs: &'a [(&'a str, AttrValue<'a>)],
    /// Distinct EU distributors with stock > 0. The depth figure the AVL rule
    /// cares about (see `preferred_parts_have_more_than_one_eu_source`).
    pub eu_sources: u32,
    /// Total stock across EU distributors.
    pub eu_stock: u32,
    /// Unit price in EUR cents at the cheapest *reachable* break — the lowest
    /// quantity the vendor will actually sell. NOT the qty-1 price, which does
    /// not exist for a reel-only part.
    pub unit_price_cents: Option<u32>,
    /// Minimum order quantity. `None` means the vendor did not say, which is
    /// treated as unknown, never as 1.
    pub moq: Option<u32>,
    /// Worst lead time in days across EU offers, if known.
    pub lead_time_days: Option<u32>,
}

impl<'a> CataloguePart<'a> {
    fn attr(&self, name: &str) -> Option<AttrValue<'a>> {
        self.attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(name))
            .map(|(_, v)| *v)
    }
}

/// Why a part did not qualify.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Rejection<'a> {
    /// The catalogue has no such attribute for this part.
    ///
    /// Deliberately a rejection rather than a pass: an unknown value is not a
    /// satisfied constraint. A missing phase-noise figure is exactly the case
    /// where silently passing would put an unqualified oscillator in a
    /// bit-perfect clock path.
    MissingAttribute { attr: &'a str, why: &'a str },
    /// Present but the wrong kind (text where a number was wanted).
    WrongType { attr: &'a str },
    /// Present, right kind, failed the test.
    Failed {
        attr: &'a str,
        why: &'a str,
        got: AttrValue<'a>,
    },
}

impl Rejection<'_> {
    /// The attribute that caused it, for grouping a report by cause.
    #[must_use]
    pub const fn attr(&self) -> &str {
        match self {
            Self::MissingAttribute { attr, .. }
            | Self::WrongType { attr }
            | Self::Failed { attr, .. } => attr,
        }
    }
}

/// Apply one constraint.
fn check<'a>(part: &CataloguePart<'a>, c: &Constraint<'a>) -> Result<(), Rejection<'a>> {
    // A datasheet-only fact is not a catalogue question. Checking it here would
    // reject every part in the category for a field no distributor publishes;
    // `a_requirement_cannot_be_all_datasheet_constraints` keeps this from
    // quietly becoming "no filtering at all".
    if c.evidence == Evidence::Datasheet {
        return Ok(());
    }
    let Some(v) = part.attr(c.attr) else {
        return Err(Rejection::MissingAttribute {
            attr: c.attr,
            why: c.why,
        });
    };
    let ok = match c.pred {
        Predicate::NumEq { value, rel_tol } => match v.as_num() {
            Some(n) => (n - value).abs() <= value.abs() * rel_tol,
            None => return Err(Rejection::WrongType { attr: c.attr }),
        },
        Predicate::AtMost(limit) => match v.as_num() {
            Some(n) => n <= limit,
            None => return Err(Rejection::WrongType { attr: c.attr }),
        },
        Predicate::AtLeast(limit) => match v.as_num() {
            Some(n) => n >= limit,
            None => return Err(Rejection::WrongType { attr: c.attr }),
        },
        Predicate::OneOf(allowed) => match v {
            AttrValue::Text(t) => allowed.iter().any(|a| a.eq_ignore_ascii_case(t)),
            _ => return Err(Rejection::WrongType { attr: c.attr }),
        },
        Predicate::IsTrue => match v {
            AttrValue::Bool(b) => b,
            _ => return Err(Rejection::WrongType { attr: c.attr }),
        },
        Predicate::IsFalse => match v {
            AttrValue::Bool(b) => !b,
            _ => return Err(Rejection::WrongType { attr: c.attr }),
        },
    };
    if ok {
        Ok(())
    } else {
        Err(Rejection::Failed {
            attr: c.attr,
            why: c.why,
            got: v,
        })
    }
}

/// The verdict on one part: qualified, or the FIRST constraint it failed.
///
/// First rather than all, on purpose: a report that lists every failure for
/// every part is unreadable at catalogue scale, and the first failure is almost
/// always the discriminating one (wrong frequency, wrong package). Callers that
/// want the full picture can re-run with a single constraint.
#[derive(Debug, Clone, Copy)]
pub struct Verdict<'a> {
    pub part: CataloguePart<'a>,
    pub rejection: Option<Rejection<'a>>,
}

impl Verdict<'_> {
    #[must_use]
    pub const fn qualified(&self) -> bool {
        self.rejection.is_none()
    }
}

/// Evaluate every part against every constraint, writing verdicts into `out`.
///
/// Takes an output slice rather than allocating, so the engine stays `no_std`
/// and usable from the firmware as well as from a host tool. Returns the number
/// written, which is `min(parts.len(), out.len())`.
pub fn evaluate<'a>(
    parts: &[CataloguePart<'a>],
    constraints: &[Constraint<'a>],
    out: &mut [Option<Verdict<'a>>],
) -> usize {
    let n = parts.len().min(out.len());
    for (slot, part) in out.iter_mut().zip(parts.iter()).take(n) {
        let rejection = constraints.iter().find_map(|c| check(part, c).err());
        *slot = Some(Verdict {
            part: *part,
            rejection,
        });
    }
    n
}

/// A soft preference, applied in the order the caller lists them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    /// More distinct EU distributors first. Listed first by this project on
    /// purpose: a cheaper part with one source is worse than a dearer part with
    /// two, because a single source is a single point of failure.
    EuSourceDepth,
    /// More EU stock first.
    EuStock,
    /// Cheaper first. Parts with no known price sort last — an unpriced part is
    /// not cheap, it is unknown.
    UnitPrice,
    /// **What it costs to get the quantity the build needs**, cheapest first.
    ///
    /// `max(moq, build_qty) * unit_price`. This exists because unit price is
    /// actively misleading wherever minimum order quantities bite — and a whole
    /// category can be like that: every active oscillator TME carries at either
    /// MCLK frequency is reel-only at 1000-3000 pieces, so a 0.70 EUR part is a
    /// 700 EUR line item while a 0.19 EUR part with MOQ 1 costs 19 cents.
    ///
    /// Ranking on unit price treated those as the same kind of cheap. Worse, it
    /// ranked the reel parts as *unpriced* and therefore last, because the only
    /// break published starts at 1000 — so in the live oscillator report every
    /// qualifying part sorted with no price at all, and the ranking was blind in
    /// precisely the category where cost decides.
    AcquisitionCost {
        /// How many the build needs. A policy number, not a property of a part.
        build_qty: u32,
    },
    /// Shorter lead time first; unknown sorts last.
    LeadTime,
}

fn cmp_by(a: &CataloguePart<'_>, b: &CataloguePart<'_>, r: Rank) -> Ordering {
    match r {
        Rank::EuSourceDepth => b.eu_sources.cmp(&a.eu_sources),
        Rank::EuStock => b.eu_stock.cmp(&a.eu_stock),
        // `None` last: `Option`'s natural order puts None first, which would
        // rank an unpriced part as the cheapest.
        Rank::UnitPrice => match (a.unit_price_cents, b.unit_price_cents) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
        Rank::AcquisitionCost { build_qty } => {
            match (
                acquisition_cost(a, build_qty),
                acquisition_cost(b, build_qty),
            ) {
                (Some(x), Some(y)) => x.cmp(&y),
                (Some(_), None) => Ordering::Less,
                (None, Some(_)) => Ordering::Greater,
                (None, None) => Ordering::Equal,
            }
        }
        Rank::LeadTime => match (a.lead_time_days, b.lead_time_days) {
            (Some(x), Some(y)) => x.cmp(&y),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
    }
}

/// Sort qualifying parts in place by the given criteria, most significant first.
///
/// Ties fall through to the next criterion, and an exhausted tie breaks on MPN.
/// That last step matters more than it looks: the result feeds a committed
/// artefact, and a re-fetch returning the same parts in a different order must
/// not reshuffle it. Keeping input order would have left the output dependent on
/// whatever sequence a distributor API happened to reply in.
///
/// `sort_unstable_by` because this crate is `no_std` without `alloc`; the MPN
/// tie-break makes the comparator total, so stability is not needed.
/// What it costs to obtain `build_qty` of this part, in EUR cents.
///
/// `max(moq, build_qty) * unit_price`, because a vendor selling in reels of 1000
/// charges for 1000 however many the board needs. `None` when either figure is
/// unknown — an unknown cost must never sort as free.
#[must_use]
pub fn acquisition_cost(part: &CataloguePart<'_>, build_qty: u32) -> Option<u32> {
    let unit = part.unit_price_cents?;
    // A missing MOQ is unknown, not 1. Assuming 1 would price a reel-only part
    // as if it could be bought singly, which is the error this criterion exists
    // to avoid.
    let moq = part.moq?;
    let qty = if moq > build_qty { moq } else { build_qty };
    unit.checked_mul(qty)
}

pub fn rank(parts: &mut [CataloguePart<'_>], by: &[Rank]) {
    parts.sort_unstable_by(|a, b| {
        for r in by {
            match cmp_by(a, b, *r) {
                Ordering::Equal => {}
                other => return other,
            }
        }
        a.mpn.cmp(b.mpn)
    });
}

/// This project's ranking policy, in one place so it is stated rather than
/// scattered: sourcing depth, then stock, then price, then lead time.
///
/// Price is third deliberately. For a hand-assembled EU small-batch build the
/// difference between a 0.10 EUR and a 0.30 EUR mux is noise; the difference
/// between one source and two is a stalled build.
pub const DEFAULT_RANKING: &[Rank] = &[
    Rank::EuSourceDepth,
    Rank::EuStock,
    // Acquisition cost, not unit price: this is a hand-assembled small batch and
    // minimum order quantities dominate the bill. See [`Rank::AcquisitionCost`].
    Rank::AcquisitionCost {
        build_qty: crate::BUILD_QUANTITY,
    },
    Rank::LeadTime,
];

#[cfg(test)]
mod tests {
    use super::*;

    fn part<'a>(
        mpn: &'a str,
        attrs: &'a [(&'a str, AttrValue<'a>)],
        eu_sources: u32,
        eu_stock: u32,
        price: Option<u32>,
    ) -> CataloguePart<'a> {
        CataloguePart {
            mpn,
            manufacturer: "test",
            attrs,
            eu_sources,
            eu_stock,
            unit_price_cents: price,
            // Singly orderable unless a test says otherwise; `part_moq` below
            // builds the reel-only case.
            moq: Some(1),
            lead_time_days: None,
        }
    }

    /// A part with a minimum order quantity, for the acquisition-cost tests.
    fn part_moq<'a>(
        mpn: &'a str,
        attrs: &'a [(&'a str, AttrValue<'a>)],
        price: Option<u32>,
        moq: Option<u32>,
    ) -> CataloguePart<'a> {
        CataloguePart {
            moq,
            ..part(mpn, attrs, 1, 1000, price)
        }
    }

    #[test]
    fn an_exact_frequency_tolerates_representation_but_not_a_different_part() {
        let c = Constraint {
            evidence: Evidence::Catalogue,
            attr: "frequency_hz",
            pred: Predicate::NumEq {
                value: 22_579_200.0,
                rel_tol: 1e-6,
            },
            why: "the 44.1 kHz family needs 512 x 44100 — a nominal CS43131 MCLK that also \
                  divides to an exact 64 x Fs bit clock",
        };
        let exact = [("frequency_hz", AttrValue::Num(22_579_200.0))];
        let rounded = [("frequency_hz", AttrValue::Num(22_579_200.000_01))];
        // The other family's nominal clock: accepted by the DAC, wrong for 44.1 kHz.
        let wrong = [("frequency_hz", AttrValue::Num(24_576_000.0))];

        assert!(check(&part("exact", &exact, 1, 1, None), &c).is_ok());
        assert!(
            check(&part("rounded", &rounded, 1, 1, None), &c).is_ok(),
            "a float-representation difference is the same oscillator"
        );
        assert!(
            check(&part("wrong", &wrong, 1, 1, None), &c).is_err(),
            "22.5792 MHz is a different part, and the one that cannot reach 176.4 kHz"
        );
    }

    /// Unit price lies wherever a minimum order quantity bites.
    ///
    /// Measured, not hypothetical: every active oscillator TME carries at either
    /// MCLK frequency is reel-only. The cheapest per unit is 24 cents with MOQ
    /// 1000 (240 EUR to obtain any), and one at 30 cents has MOQ 3000 (900 EUR).
    /// A unit-price ranking calls the 24-cent part cheaper than a 19-cent part
    /// with MOQ 1 — which costs 95 cents for five.
    #[test]
    fn acquisition_cost_ranks_a_reel_below_a_singly_orderable_part() {
        let none: [(&str, AttrValue); 0] = [];
        // 19 cents, buy one: 5 x 19 = 95 cents for a build of five.
        let singly = part_moq("singly-orderable", &none, Some(19), Some(1));
        // 24 cents but reel-only: 1000 x 24 = 24000 cents.
        let reel = part_moq("reel-only", &none, Some(24), Some(1000));

        assert_eq!(acquisition_cost(&singly, 5), Some(95));
        assert_eq!(acquisition_cost(&reel, 5), Some(24_000));

        let mut parts = [reel, singly];
        rank(&mut parts, &[Rank::AcquisitionCost { build_qty: 5 }]);
        assert_eq!(
            parts[0].mpn, "singly-orderable",
            "a 19-cent part you can buy five of beats a 24-cent part you must buy 1000 of. \
             Ranking on UNIT price gets this backwards, and the reel part is cheaper per unit."
        );
    }

    /// `build_qty` has to be a parameter, because the answer inverts with scale.
    ///
    /// The inversion needs a reel that is genuinely cheaper PER UNIT — my first
    /// attempt at this test used a reel costing more per unit and asserted it
    /// would win at scale, which it never can: once the build is larger than
    /// both MOQs, each part is bought in the same quantity and the cheaper unit
    /// price simply wins. A high MOQ is only worth paying for if the unit price
    /// is actually lower.
    #[test]
    fn the_cheapest_option_depends_on_the_build_quantity() {
        let none: [(&str, AttrValue); 0] = [];
        // Dearer per unit, but buy exactly what you need.
        let flexible = part_moq("flexible", &none, Some(19), Some(1));
        // Half the unit price, but 1000 minimum.
        let bulk = part_moq("bulk", &none, Some(10), Some(1000));

        // A prototype run of five: 95 cents vs 100 EUR.
        assert_eq!(acquisition_cost(&flexible, 5), Some(95));
        assert_eq!(acquisition_cost(&bulk, 5), Some(10_000));
        let mut small = [bulk, flexible];
        rank(&mut small, &[Rank::AcquisitionCost { build_qty: 5 }]);
        assert_eq!(small[0].mpn, "flexible");

        // A production run of 2000: 380 EUR vs 200 EUR, and the order flips.
        assert_eq!(acquisition_cost(&flexible, 2000), Some(38_000));
        assert_eq!(acquisition_cost(&bulk, 2000), Some(20_000));
        let mut large = [flexible, bulk];
        rank(&mut large, &[Rank::AcquisitionCost { build_qty: 2000 }]);
        assert_eq!(
            large[0].mpn, "bulk",
            "at 2000 boards the MOQ is no longer a penalty and the lower unit price wins. This is \
             why BUILD_QUANTITY lives in the spec as a stated policy rather than being inlined \
             into the comparator"
        );
    }

    /// An unknown MOQ must not be assumed to be 1.
    #[test]
    fn an_unknown_moq_makes_the_cost_unknown_rather_than_cheap() {
        let none: [(&str, AttrValue); 0] = [];
        let unknown = part_moq("moq-unpublished", &none, Some(10), None);
        assert_eq!(
            acquisition_cost(&unknown, 5),
            None,
            "assuming MOQ 1 would price a possibly reel-only part as if it could be bought \
             singly — the exact error this criterion exists to prevent"
        );

        // And unknown must sort AFTER a known cost, not before.
        let known = part_moq("moq-known", &none, Some(70), Some(1000));
        let mut parts = [unknown, known];
        rank(&mut parts, &[Rank::AcquisitionCost { build_qty: 5 }]);
        assert_eq!(
            parts[0].mpn, "moq-known",
            "a 700 EUR known cost still beats an unknown one: unknown is not free"
        );
    }

    /// The published price must be the lowest REACHABLE break.
    ///
    /// The export previously took `qty <= 1`, which is NULL for every reel-only
    /// part — so the whole oscillator category exported as unpriced and sorted
    /// last on price while being the only thing available. This pins the
    /// consequence at the engine boundary: a part with a price and an MOQ is
    /// comparable, and one without a price is not.
    #[test]
    fn a_part_with_no_published_price_cannot_be_costed() {
        let none: [(&str, AttrValue); 0] = [];
        let unpriced = part_moq("unpriced", &none, None, Some(1));
        assert_eq!(acquisition_cost(&unpriced, 5), None);
    }

    /// A vendor truncating the frequency must not reject the part.
    ///
    /// Not hypothetical: TME publishes Abracon `ASV-22.5792MHZ-LR-T` — 17000 in
    /// stock — with `Frequency: "22.579MHz"`, five significant figures where the
    /// part has six. That is 8.86e-6 relative, which a 1e-6 tolerance rejects.
    ///
    /// The tolerance has to absorb vendor FORMATTING while still rejecting a
    /// different part. These two assertions are the boundary.
    #[test]
    fn a_truncated_frequency_still_matches_but_a_different_one_does_not() {
        let c = Constraint {
            evidence: Evidence::Catalogue,
            attr: "frequency_hz",
            pred: Predicate::NumEq {
                value: 22_579_200.0,
                rel_tol: 1e-4,
            },
            why: "the 44.1 kHz family clock",
        };
        // TME's truncated value.
        let truncated = [("frequency_hz", AttrValue::Num(22_579_000.0))];
        assert!(
            check(&part("asv", &truncated, 1, 17_000, Some(60)), &c).is_ok(),
            "a frequency truncated to five significant figures is the SAME part; rejecting it \
             loses the only deeply-stocked candidate in the category"
        );
        // And the nearest frequencies anyone might confuse it with must still fail.
        for (name, hz) in [
            ("22.1184 MHz UART crystal", 22_118_400.0),
            ("the other MCLK family", 24_576_000.0),
            ("a round 22 MHz", 22_000_000.0),
        ] {
            let other = [("frequency_hz", AttrValue::Num(hz))];
            assert!(
                check(&part("other", &other, 1, 1, None), &c).is_err(),
                "{name} must still be rejected — widening the tolerance for formatting must not \
                 make it accept a different frequency"
            );
        }
    }

    #[test]
    fn a_missing_attribute_is_a_rejection_not_a_pass() {
        let c = Constraint {
            evidence: Evidence::Catalogue,
            attr: "phase_noise_dbc_1khz",
            pred: Predicate::AtMost(-130.0),
            why: "jitter in the MCLK path is the one thing the dual-oscillator design exists to \
                  avoid",
        };
        let no_figure = [("frequency_hz", AttrValue::Num(22_579_200.0))];
        let r = check(&part("unknown", &no_figure, 2, 100, None), &c).unwrap_err();
        assert!(
            matches!(r, Rejection::MissingAttribute { .. }),
            "an unknown phase-noise figure must not silently qualify an oscillator for a \
             bit-perfect clock path"
        );
        assert_eq!(r.attr(), "phase_noise_dbc_1khz");
    }

    #[test]
    fn a_rejection_carries_the_reason_the_constraint_exists() {
        let c = Constraint {
            evidence: Evidence::Catalogue,
            attr: "supply_mv_max",
            pred: Predicate::AtMost(3300.0),
            why: "the board has no rail above 3.3 V",
        };
        let five_volt = [("supply_mv_max", AttrValue::Num(5000.0))];
        match check(&part("5v", &five_volt, 2, 10, None), &c).unwrap_err() {
            Rejection::Failed { why, got, .. } => {
                assert_eq!(why, "the board has no rail above 3.3 V");
                assert_eq!(got, AttrValue::Num(5000.0));
            }
            other => panic!("expected a Failed rejection, got {other:?}"),
        }
    }

    #[test]
    fn text_predicates_ignore_case_because_catalogues_do_not_agree_on_it() {
        let c = Constraint {
            evidence: Evidence::Catalogue,
            attr: "package",
            pred: Predicate::OneOf(&["SC-70-6", "TSSOP-6"]),
            why: "hand-assembly needs a package with reachable leads",
        };
        let lower = [("package", AttrValue::Text("sc-70-6"))];
        assert!(check(&part("lower", &lower, 1, 1, None), &c).is_ok());
    }

    #[test]
    fn evaluate_reports_the_first_failing_constraint_per_part() {
        let cs = [
            Constraint {
                evidence: Evidence::Catalogue,
                attr: "frequency_hz",
                pred: Predicate::NumEq {
                    value: 22_579_200.0,
                    rel_tol: 1e-6,
                },
                why: "family",
            },
            Constraint {
                evidence: Evidence::Catalogue,
                attr: "supply_mv_max",
                pred: Predicate::AtMost(3300.0),
                why: "rail",
            },
        ];
        let good = [
            ("frequency_hz", AttrValue::Num(22_579_200.0)),
            ("supply_mv_max", AttrValue::Num(3300.0)),
        ];
        let bad_freq = [
            ("frequency_hz", AttrValue::Num(24_576_000.0)),
            ("supply_mv_max", AttrValue::Num(3300.0)),
        ];
        let parts = [
            part("good", &good, 2, 50, Some(120)),
            part("bad", &bad_freq, 2, 50, Some(90)),
        ];
        let mut out = [None; 2];
        let n = evaluate(&parts, &cs, &mut out);
        assert_eq!(n, 2);
        assert!(out[0].unwrap().qualified());
        assert_eq!(out[1].unwrap().rejection.unwrap().attr(), "frequency_hz");
    }

    #[test]
    fn ranking_puts_sourcing_depth_above_price() {
        let a = [("x", AttrValue::Num(0.0))];
        // Cheaper, single-sourced.
        let cheap_single = part("cheap", &a, 1, 1000, Some(10));
        // Dearer, double-sourced.
        let dear_double = part("dear", &a, 2, 50, Some(500));
        let mut parts = [cheap_single, dear_double];
        rank(&mut parts, DEFAULT_RANKING);
        assert_eq!(
            parts[0].mpn, "dear",
            "two sources must outrank a lower price: a single source is a single point of failure"
        );
    }

    #[test]
    fn an_unpriced_part_ranks_last_rather_than_cheapest() {
        let a = [("x", AttrValue::Num(0.0))];
        let unpriced = part("unknown", &a, 2, 100, None);
        let priced = part("priced", &a, 2, 100, Some(999));
        let mut parts = [unpriced, priced];
        rank(&mut parts, DEFAULT_RANKING);
        assert_eq!(
            parts[0].mpn, "priced",
            "Option's natural order would put None first and rank an unpriced part as free"
        );
    }

    #[test]
    fn ranking_is_deterministic_regardless_of_catalogue_order() {
        let a = [("x", AttrValue::Num(0.0))];
        // Same two parts, every ranking criterion tied, presented in both orders.
        let mut forward = [
            part("aaa-osc", &a, 2, 100, Some(100)),
            part("zzz-osc", &a, 2, 100, Some(100)),
        ];
        let mut reverse = [
            part("zzz-osc", &a, 2, 100, Some(100)),
            part("aaa-osc", &a, 2, 100, Some(100)),
        ];
        rank(&mut forward, DEFAULT_RANKING);
        rank(&mut reverse, DEFAULT_RANKING);
        assert_eq!(
            [forward[0].mpn, forward[1].mpn],
            [reverse[0].mpn, reverse[1].mpn],
            "a re-fetch returning the same parts in a different order must not reshuffle the \
             committed output"
        );
        assert_eq!(forward[0].mpn, "aaa-osc", "the tie-break is MPN order");
    }
}
