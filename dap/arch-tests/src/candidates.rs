//! Loading `hardware/bom/candidates/*.json` into the selection engine.
//!
//! # Why this is a library and not a test helper
//!
//! Turning a vendor's string into the typed value a constraint reads is the step
//! that decides whether a part qualifies, and it had three copies — one per test
//! file that needed it. A fourth copy was about to go into the sourcing report,
//! where it would have been free to disagree with the gates: a report claiming a
//! part passes while the gate rejects it is worse than no report, because the
//! disagreement is invisible until someone orders the part.
//!
//! So there is one implementation, and both the gates and the report call it.
//!
//! # What this does NOT do
//!
//! It does not interpret units. `parametric::derive` does that, in `dap-hw-spec`,
//! which is `no_std` and dependency-free so the firmware and the sourcing
//! pipeline cannot drift apart on what `"1.65...5.5V DC"` means. This module only
//! decides WHICH attribute a constraint wants and hands the raw string over.

use dap_hw_spec::parametric::{derive, expected_dimension};
use dap_hw_spec::select::{AttrValue, CataloguePart, Constraint, Evidence};
use serde_json::Value;

/// One exported candidate, with the vendor's values still as strings.
#[derive(Debug, Clone)]
pub struct Raw {
    pub mpn: String,
    pub manufacturer: String,
    pub eu_sources: u32,
    pub eu_stock: u32,
    pub unit_price_cents: Option<u32>,
    pub moq: Option<u32>,
    pub lead_time_days: Option<u32>,
    pub datasheet_url: Option<String>,
    pub vendors: Vec<String>,
    pub attrs_raw: Vec<(String, String)>,
    /// Distributor product pages, keyed by vendor.
    ///
    /// Per vendor rather than reduced to one, because the price and stock a
    /// reader wants to verify differ per vendor — which is the reason the
    /// pipeline fetches more than one in the first place.
    pub product_urls: Vec<(String, String)>,
    /// Attributes this part's vendors disagree about, after canonicalisation.
    ///
    /// The export still has to pick one value and picks the
    /// alphabetically-first vendor's, so for a discriminating attribute the
    /// verdict depends on vendor NAME order rather than on the part.
    pub conflicts: Vec<String>,
}

/// Parse a candidate export, asserting it is the category asked for.
///
/// The category assertion is not ceremony: the files are generated per
/// requirement and reading the wrong one would silently filter oscillators
/// against the DAC's constraints, which rejects everything and looks like a
/// sourcing problem.
///
/// # Panics
///
/// If `doc` is not valid JSON, has no `parts` array, or declares a different
/// category.
pub fn load(doc: &str, category: &str) -> Vec<Raw> {
    let v: Value = serde_json::from_str(doc).expect("candidate file must be valid JSON");
    assert_eq!(
        v["category"].as_str(),
        Some(category),
        "candidate file declares a different category than the one being loaded"
    );
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
            datasheet_url: p["datasheet_url"].as_str().map(str::to_string),
            product_urls: p["product_urls"]
                .as_object()
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| v.as_str().map(|u| (k.clone(), u.to_string())))
                        .collect()
                })
                .unwrap_or_default(),
            conflicts: p["conflicts"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|x| x.as_str())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
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

/// The raw string a constraint's attribute wants, honouring `_min`/`_max`.
///
/// A constraint on `supply_mv_max` is satisfied from a vendor's `supply_mv`
/// string, because one published range answers both bounds and
/// `parametric::derive` is what splits it. Looking only for the exact attribute
/// name would make every range constraint fail as a missing attribute.
#[must_use]
pub fn raw_for<'a>(attrs: &'a [(String, String)], attr: &str) -> Option<&'a str> {
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

/// Typed attributes for exactly the constraints that can be catalogue-checked.
///
/// `Evidence::Datasheet` constraints are skipped rather than failed. They ask
/// things no distributor publishes — a QDID, an integrated amplifier's dynamic
/// range, break-before-make — and treating them as missing attributes would
/// reject every part in the category and read as "nothing is sourceable".
#[must_use]
pub fn typed<'a>(
    raw: &'a Raw,
    constraints: &'a [Constraint<'static>],
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
            // A dimensioned attribute whose string cannot be parsed is LEFT OUT
            // rather than pushed as text: it then reads as a missing attribute
            // and is rejected visibly, instead of a text comparison silently
            // passing on an unparseable number.
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

/// A `CataloguePart` borrowing from `raw` and `attrs`, ready for `evaluate`.
#[must_use]
pub fn part<'a>(raw: &'a Raw, attrs: &'a [(&'static str, AttrValue<'a>)]) -> CataloguePart<'a> {
    CataloguePart {
        mpn: &raw.mpn,
        manufacturer: &raw.manufacturer,
        attrs,
        eu_sources: raw.eu_sources,
        eu_stock: raw.eu_stock,
        unit_price_cents: raw.unit_price_cents,
        moq: raw.moq,
        lead_time_days: raw.lead_time_days,
    }
}

/// Comparison key for a MANUFACTURER name, for counting distinct makers.
///
/// # Why this is needed
///
/// `user_swappable_requirements_span_two_manufacturers` asks whether two
/// companies make a qualifying part. The first run reported two for the battery
/// — `"AKYGA"` and `"AKYGA BATTERY"` — which is one company written down twice,
/// and would have deleted an accepted exposure on a spelling.
///
/// # Deliberately conservative
///
/// The failure modes are not symmetric. Treating one maker as two **weakens** a
/// gate and does it silently; treating two makers as one only makes the gate
/// stricter than reality, which fails loudly and gets looked at. So this
/// normalises only what is unambiguous:
///
/// 1. Case and punctuation, which vendors disagree about freely.
/// 2. A trailing corporate form or product-line word from a NAMED list. Not a
///    general "strip the last word" rule, which would merge `MOLEX` with a
///    hypothetical `MOLEX SOMETHINGELSE` that was genuinely separate.
///
/// A prefix rule is deliberately NOT applied. `TE` (TE Connectivity) is a prefix
/// of `TEXAS INSTRUMENTS`, and those are emphatically different companies — so
/// prefix matching would merge two real manufacturers and quietly satisfy a gate
/// that exists to catch exactly that.
#[must_use]
pub fn maker_key(name: &str) -> String {
    /// Trailing words that describe a corporate form or a product line rather
    /// than identifying a different company. Each one is here because it was
    /// observed in real vendor data.
    const TRAILING: &[&str] = &[
        "BATTERY", // TME: 'AKYGA BATTERY' vs Digi-Key: 'AKYGA'
        "INC",
        "INC.",
        "CORP",
        "CORPORATION",
        "CO",
        "LTD",
        "LIMITED",
        "GMBH",
        "AG",
        "SA",
        "BV",
        "LLC",
        "TECHNOLOGY",
        "TECHNOLOGIES",
        "ELECTRONICS",
        "ELECTRONIC",
        "SEMICONDUCTOR",
        "SEMICONDUCTORS",
    ];

    let upper = name.trim().to_uppercase();
    // Vendors append acquisition notes: 'Same Sky (Formerly CUI Devices)'.
    // Everything from the first parenthesis is dropped — it names the company's
    // history, not the company.
    let upper = upper.split('(').next().unwrap_or(&upper).trim().to_string();
    let mut words: Vec<&str> = upper
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|w| !w.is_empty())
        .collect();
    while words.len() > 1 && TRAILING.contains(words.last().unwrap()) {
        words.pop();
    }
    words.join("")
}

#[cfg(test)]
mod tests {
    use super::maker_key;

    #[test]
    fn one_company_spelled_two_ways_is_one_maker() {
        // The live case: this pair was reported as two manufacturers and would
        // have deleted an accepted single-manufacturer exposure.
        assert_eq!(maker_key("AKYGA"), maker_key("AKYGA BATTERY"));
        assert_eq!(maker_key("Cirrus Logic"), maker_key("Cirrus Logic Inc."));
        assert_eq!(
            maker_key("Same Sky (Formerly CUI Devices)"),
            maker_key("Same Sky")
        );
    }

    #[test]
    fn genuinely_different_makers_stay_different() {
        // The reason there is no prefix rule: TE is a prefix of TEXAS.
        assert_ne!(maker_key("TE Connectivity"), maker_key("Texas Instruments"));
        assert_ne!(maker_key("Molex"), maker_key("Amphenol"));
        assert_ne!(maker_key("AKYGA"), maker_key("Jauch Quartz"));
        // A trailing word is only stripped when something identifying remains.
        assert_eq!(maker_key("Battery"), "BATTERY");
    }
}
