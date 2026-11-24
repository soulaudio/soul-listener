//! Ranked, linked component proposals — the output a human picks from.
//!
//! ```text
//! cargo run -p dap-arch-tests --example propose
//! ```
//!
//! Writes `hardware/bom/proposals.json` and `hardware/bom/PROPOSALS.md`.
//!
//! # Why this is a separate artefact from the report
//!
//! `sourcing-report` answers "is the filter doing real work" and is diagnostic:
//! histograms, anomalies, coverage. This answers a different question — "what
//! should we buy" — and its output is meant to be committed, read by a person,
//! and then acted on. Mixing them produced a wall of text where the decision was
//! buried in the diagnostics.
//!
//! # The two formats are not redundant
//!
//! `PROPOSALS.md` is for reading: a table per requirement, ranked, with a
//! clickable link per distributor. `proposals.json` is for the next step —
//! choosing a part writes its MPN into the spec, and `pick` (see below) reads
//! this file to know what was on offer and at what price when the choice was
//! made. A decision record needs the alternatives, not just the winner.
//!
//! # Ranking is NOT re-implemented here
//!
//! It calls `dap_hw_spec::select::rank` with `DEFAULT_RANKING`, the same policy
//! the gates use: EU source depth first, then acquisition cost at the real build
//! quantity. A proposal ranked by a different rule than the one the spec
//! declares would be a second, competing opinion about what "best" means.
//!
//! # Links
//!
//! A proposal nobody can click is a proposal nobody checks. Every row carries
//! the distributor's own product page per vendor, because stock and price are
//! the figures that go stale fastest in a committed file — the link is how a
//! reader confirms the number is still true.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use dap_arch_tests::candidates::{load, part, typed};
use dap_hw_spec::select::{evaluate, rank, AttrValue, CataloguePart, Evidence, DEFAULT_RANKING};
use dap_hw_spec::{Preference, ACTIVE};

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
        "battery",
        include_str!("../../../hardware/bom/candidates/battery.json"),
    ),
    (
        "bluetooth",
        include_str!("../../../hardware/bom/candidates/bluetooth.json"),
    ),
    (
        "microsd",
        include_str!("../../../hardware/bom/candidates/microsd.json"),
    ),
];

/// How many to propose per requirement.
///
/// Ten, not three: the top of the list is usually several variants of one part
/// from one maker, and the interesting second source is often further down. Not
/// fifty either — a list nobody reads to the end is a list that hides its own
/// tail, and `sourcing-report` already states the full qualifying count.
const TOP_N: usize = 10;

fn esc(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn money(cents: Option<u32>) -> String {
    cents.map_or_else(|| "?".to_string(), |c| format!("{:.2}", c as f64 / 100.0))
}

fn main() {
    let mut md = String::new();
    let mut json = String::new();
    // The decisions, as opposed to the options. Written to `selected.json`,
    // which is the handoff file: it is what a firmware constant table or an
    // atopile part generator reads, so that "switch a component" means editing
    // ONE `Preference` in the spec and regenerating.
    let mut selected: Vec<String> = Vec::new();

    md.push_str(
        "# Proposed components\n\n\
         **Generated** — do not edit by hand. Regenerate with\n\
         `cargo run -p dap-arch-tests --example propose`.\n\n\
         Every part below PASSES its requirement's catalogue-checkable constraints and is \
         ranked by the spec's own `DEFAULT_RANKING`: EU source depth first, then what it \
         actually costs to buy the build quantity (`max(moq, build_qty) x unit_price`), then \
         lead time. Source depth leads deliberately — a cheaper part with one distributor is \
         worse than a dearer part with two.\n\n\
         What this list does NOT decide: every requirement also carries \
         `Evidence::Datasheet` constraints that no distributor publishes — dynamic range, \
         phase noise, break-before-make, QDID inheritance, UN 38.3 reports. Those are read \
         off a datasheet by a person, and a part ranking first here can still be the wrong \
         choice because of one of them. Ranking is the shortlist, not the decision.\n\n\
         Stock and price were true when this was generated. Follow the distributor link in \
         each row before ordering.\n",
    );
    json.push_str("{\n  \"generated_by\": \"dap-arch-tests::propose\",\n  \"requirements\": {\n");

    let mut first_req = true;
    for req in ACTIVE.parts {
        let Some((_, doc)) = EXPORTS.iter().find(|(id, _)| *id == req.id) else {
            continue;
        };
        let raws = load(doc, req.id);
        let typedv: Vec<Vec<(&'static str, AttrValue<'_>)>> =
            raws.iter().map(|r| typed(r, req.constraints)).collect();
        let parts: Vec<CataloguePart<'_>> =
            raws.iter().zip(&typedv).map(|(r, t)| part(r, t)).collect();

        let mut slots: Vec<Option<_>> = vec![None; parts.len()];
        let n = evaluate(&parts, req.constraints, &mut slots);
        assert_eq!(n, parts.len());
        let mut passing: Vec<CataloguePart<'_>> = slots
            .into_iter()
            .flatten()
            .filter(|v| v.qualified())
            .map(|v| v.part)
            .collect();
        rank(&mut passing, DEFAULT_RANKING);

        // Index back to the raw row for links and vendor names, which the
        // engine's `CataloguePart` deliberately does not carry — it is `no_std`
        // and has no business holding URLs.
        let by_mpn: BTreeMap<&str, &dap_arch_tests::candidates::Raw> =
            raws.iter().map(|r| (r.mpn.as_str(), r)).collect();

        let datasheet_only: Vec<&str> = req
            .constraints
            .iter()
            .filter(|c| c.evidence == Evidence::Datasheet)
            .map(|c| c.attr)
            .collect();

        // Parts the spec has ALREADY ruled out, with the reason.
        //
        // They still pass the catalogue filter — `2908-05WB-MG` is a two-source
        // socket with 73000 in stock that ranks second on sourcing and has no
        // card-detect switch, which is datasheet evidence the filter cannot
        // see — so without this they are silently re-offered and the reader has
        // to remember a decision somebody already made and wrote down.
        let rejected: Vec<(&str, &str)> = req
            .candidates
            .iter()
            .filter_map(|c| match c.preference {
                Preference::Rejected(why) => Some((c.mpn, why)),
                _ => None,
            })
            .collect();

        let chosen: Vec<&str> = req
            .candidates
            .iter()
            .filter(|c| matches!(c.preference, Preference::Preferred))
            .map(|c| c.mpn)
            .collect();

        for mpn in &chosen {
            let raw = raws.iter().find(|r| r.mpn == *mpn);
            let links = raw.map(|r| r.product_urls.as_slice()).unwrap_or(&[]);
            selected.push(format!(
                "    {{\"requirement\": \"{}\", \"mpn\": \"{}\", \
                 \"manufacturer\": \"{}\", \"datasheet_url\": {}, \"links\": {{{}}}}}",
                esc(req.id),
                esc(mpn),
                esc(raw.map(|r| r.manufacturer.as_str()).unwrap_or("")),
                raw.and_then(|r| r.datasheet_url.as_deref())
                    .map_or("null".to_string(), |u| format!("\"{}\"", esc(u))),
                links
                    .iter()
                    .map(|(v, u)| format!("\"{}\": \"{}\"", esc(v), esc(u)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }

        let _ = write!(md, "\n## `{}`\n\n{}\n\n", req.id, req.what);
        let _ = write!(
            md,
            "{} of {} swept candidates qualify.",
            passing.len(),
            raws.len()
        );
        if chosen.is_empty() {
            md.push_str(" **Nothing is picked yet for this requirement.**");
        } else {
            let _ = write!(md, " Currently preferred: `{}`.", chosen.join("`, `"));
        }
        if !datasheet_only.is_empty() {
            let _ = write!(
                md,
                "\n\nStill to confirm by hand for whichever part is chosen: `{}`.",
                datasheet_only.join("`, `")
            );
        }
        md.push_str("\n\n| # | MPN | manufacturer | EU src | stock | MOQ | EUR @1 | where |\n");
        md.push_str("|---|---|---|---:|---:|---:|---:|---|\n");

        if !first_req {
            json.push_str(",\n");
        }
        first_req = false;
        let _ = write!(json, "    \"{}\": [", req.id);

        for (i, p) in passing.iter().take(TOP_N).enumerate() {
            let raw = by_mpn.get(p.mpn);
            let links = raw.map(|r| r.product_urls.as_slice()).unwrap_or(&[]);
            let where_md = if links.is_empty() {
                raw.map(|r| r.vendors.join(", ")).unwrap_or_default()
            } else {
                links
                    .iter()
                    .map(|(v, u)| format!("[{v}]({u})"))
                    .collect::<Vec<_>>()
                    .join(" · ")
            };
            let mark = if chosen.contains(&p.mpn) {
                " **<-- picked**".to_string()
            } else if let Some((_, why)) = rejected.iter().find(|(m, _)| *m == p.mpn) {
                // Truncated: the full reason lives in the spec, and a table cell
                // is not where a paragraph belongs.
                let short: String = why.chars().take(90).collect();
                format!(" ~~RULED OUT~~<br>_{short}…_")
            } else {
                String::new()
            };
            let _ = writeln!(
                md,
                "| {} | `{}`{mark} | {} | {} | {} | {} | {} | {} |",
                i + 1,
                p.mpn,
                p.manufacturer,
                p.eu_sources,
                p.eu_stock,
                p.moq.map_or("?".to_string(), |m| m.to_string()),
                money(p.unit_price_cents),
                where_md
            );
            if i > 0 {
                json.push(',');
            }
            let _ = write!(
                json,
                "\n      {{\"rank\": {}, \"mpn\": \"{}\", \"manufacturer\": \"{}\", \
                 \"eu_sources\": {}, \"eu_stock\": {}, \"moq\": {}, \
                 \"unit_price_cents\": {}, \"picked\": {}, \"ruled_out\": {}, \
                 \"links\": {{{}}}}}",
                i + 1,
                esc(p.mpn),
                esc(p.manufacturer),
                p.eu_sources,
                p.eu_stock,
                p.moq.map_or("null".to_string(), |m| m.to_string()),
                p.unit_price_cents
                    .map_or("null".to_string(), |c| c.to_string()),
                chosen.contains(&p.mpn),
                rejected
                    .iter()
                    .find(|(m, _)| *m == p.mpn)
                    .map_or("null".to_string(), |(_, why)| format!("\"{}\"", esc(why))),
                links
                    .iter()
                    .map(|(v, u)| format!("\"{}\": \"{}\"", esc(v), esc(u)))
                    .collect::<Vec<_>>()
                    .join(", ")
            );
        }
        if passing.len() > TOP_N {
            let _ = write!(
                md,
                "\n_{} further qualifying parts not shown; `sourcing-report` states the full \
                 count and what rejected the rest._\n",
                passing.len() - TOP_N
            );
        }
        if passing.is_empty() {
            md.push_str(
                "\n_Nothing qualifies. That is a finding, not an empty table — see \
                 `sourcing-report` for which constraint rejected everything, and whether it \
                 is a real market gap or a catalogue-coverage one._\n",
            );
        }
        json.push_str("\n    ]");
    }
    json.push_str("\n  }\n}\n");

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("repo root");
    std::fs::write(root.join("hardware/bom/PROPOSALS.md"), &md).expect("write PROPOSALS.md");
    std::fs::write(root.join("hardware/bom/proposals.json"), &json).expect("write proposals.json");

    // The handoff file. Deliberately minimal and deliberately NOT the snapshot:
    // the snapshot records every offer for every candidate and is a sourcing
    // audit trail, while this answers one question — "which part is this
    // requirement built with" — for consumers that have no business reading
    // price breaks.
    let sel = format!(
        "{{\n  \"readme\": \"GENERATED from dap_hw_spec::ACTIVE by \
         dap-arch-tests::propose. The parts this board is built with, one per \
         requirement, taken from Preference::Preferred in the spec. To switch a \
         component, change the Preference in the spec and regenerate — do not edit \
         this file. every_picked_part_qualifies_against_its_own_requirement checks the \
         new pick against the requirement it was picked for.\",\n  \"selected\": [\n{}\n  ]\n}}\n",
        selected.join(",\n")
    );
    std::fs::write(root.join("hardware/bom/selected.json"), &sel).expect("write selected.json");
    println!(
        "wrote hardware/bom/PROPOSALS.md, proposals.json and selected.json ({} picked)",
        selected.len()
    );
}
