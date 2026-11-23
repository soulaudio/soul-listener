//! The per-distributor BOM files must stay honest about *which board they bill*.
//!
//! # The defect these gates exist to prevent
//!
//! `hardware/bom/sourcing/{tme,digikey-eu,mouser}.csv` listed `STM32H743ZIT6`,
//! `ES9038Q2M`, `TPA6120A2RGYR` and `W9825G6KH-6` — all four of them parts V2
//! eliminates — under a heading that said only "BOM sourcing", with notes like
//! "Check TME stock", in a repo where the firmware targets V2 and
//! `cargo xtask hardware` prints these three paths as where to go source parts.
//!
//! Every one of those rows was *correct*: `hardware/elec/src` is still the V1
//! schematic, and a BOM's defining invariant is that it matches the schematic it
//! bills. The defect was not the contents. It was that three files in a two-
//! revision repo did not say which revision they were for, so a correct V1 BOM
//! read as a current shopping list.
//!
//! The fix is therefore a declared `# Revision:` marker — and these gates, which
//! make the marker load-bearing instead of decorative. The one that matters most
//! is [`the_declared_revision_matches_the_schematic_being_billed`]: the day
//! someone points `elec/src/mcu.ato` at an H7A3, these files fail until they
//! migrate too. A comment would not have done that.
//!
//! # What is deliberately *not* checked
//!
//! That the V1 schematic exists at all. It is the as-built design and its
//! supersession is already tracked by `dap_hw_spec::invariants::
//! migration_deltas_are_tracked`; duplicating that here would turn a tracked,
//! decided migration into a recurring failure.

use dap_hw_spec::{v1::V1, v2::V2, BoardSpec, Preference, Revision};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// The three distributor files, with the `Format:` header each one declares.
const BOM_FILES: &[&str] = &["tme.csv", "digikey-eu.csv", "mouser.csv"];

fn hardware_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR is dap/arch-tests; the hardware tree is two levels up.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../hardware")
        .canonicalize()
        .expect("the hardware/ tree must exist next to dap/")
}

fn read_bom(name: &str) -> String {
    let p = hardware_dir().join("bom/sourcing").join(name);
    std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("cannot read {}: {e}", p.display()))
}

/// A `# Key: value` header line.
fn header<'a>(text: &'a str, key: &str) -> Option<&'a str> {
    text.lines()
        .filter(|l| l.starts_with('#'))
        .filter_map(|l| l.trim_start_matches('#').trim().split_once(':'))
        .find(|(k, _)| k.trim().eq_ignore_ascii_case(key))
        .map(|(_, v)| v.trim())
}

/// One billed line: the reference designator, the manufacturer part number and
/// the package, which is all these gates need.
#[derive(Debug)]
struct Row {
    reference: String,
    mpn: String,
    package: String,
    line_no: usize,
}

/// Data rows, skipping comments, blanks and the column-name header.
///
/// Every file declares `Format: Ref,QtyTotal,MfrPartNo,<DistPN>,Description,
/// Package,UnitPriceEUR,Notes`, so the column indices are fixed — but the MPN and
/// package positions are resolved from the actual column-name row rather than
/// hardcoded, so a file that reorders its columns is read correctly instead of
/// silently mis-parsed.
fn rows(text: &str) -> Vec<Row> {
    let mut cols: Option<Vec<String>> = None;
    let mut out = Vec::new();

    for (i, line) in text.lines().enumerate() {
        let line_no = i + 1;
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') {
            continue;
        }
        let fields = split_csv(t);
        if cols.is_none() {
            // The first non-comment, non-blank line is the column-name row.
            cols = Some(fields.iter().map(|f| f.trim().to_lowercase()).collect());
            continue;
        }
        let names = cols.as_ref().expect("column row seen above");
        let at = |want: &str| {
            names
                .iter()
                .position(|n| n == want)
                .and_then(|idx| fields.get(idx))
                .map(|s| s.trim().to_string())
                .unwrap_or_default()
        };
        out.push(Row {
            reference: at("ref"),
            mpn: at("mfrpartno"),
            package: at("package"),
            line_no,
        });
    }
    out
}

/// Minimal CSV field split that respects `"..."` quoting.
///
/// Needed because part numbers here legitimately contain commas — the TME symbol
/// for the ESD array is `PESD5V0L5UY,115`. A naive `split(',')` would read that
/// as two fields and shift every later column, which is exactly the kind of
/// silent mis-parse that makes a gate report nonsense.
fn split_csv(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    for c in line.chars() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Normalise an MPN for comparison against the spec.
///
/// A BOM carries the *orderable* part number and the spec carries the base part:
/// `TPA6120A2RGYR` is the VQFN reel of `TPA6120A2`, and `LT3042EMSE#PBF` is the
/// lead-free order code of `LT3042EMSE`. Comparing those as plain strings would
/// find no match and report every row as unknown.
fn normalise(mpn: &str) -> String {
    mpn.to_uppercase()
        .split('#')
        .next()
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Does a BOM part number refer to the same part as a spec MPN?
///
/// Prefix containment in either direction, because the suffix may be on either
/// side: the BOM may be more specific than the spec (`TPA6120A2RGYR` vs
/// `TPA6120A2`) or the spec more specific than the BOM (`CS43131-CNZ` vs a BOM
/// row saying `CS43131`). The two MCUs this repo cares about, `STM32H743ZIT6` and
/// `STM32H7A3ZIT6`, are not prefixes of each other, so this cannot confuse them —
/// which `prefix_matching_does_not_confuse_the_two_mcus` pins.
fn same_part(bom_mpn: &str, spec_mpn: &str) -> bool {
    let (a, b) = (normalise(bom_mpn), normalise(spec_mpn));
    !a.is_empty() && !b.is_empty() && (a.starts_with(&b) || b.starts_with(&a))
}

/// Every MPN a revision's spec lists, with the preference attached.
fn spec_mpns(spec: &BoardSpec) -> Vec<(&'static str, Preference)> {
    spec.parts
        .iter()
        .flat_map(|r| r.candidates.iter().map(|c| (c.mpn, c.preference)))
        .collect()
}

fn declared_revision(text: &str, file: &str) -> Revision {
    match header(text, "Revision") {
        Some("v1") => Revision::V1,
        Some("v2") => Revision::V2,
        other => panic!(
            "{file} declares Revision: {other:?}. A BOM must say which schematic it bills — this \
             repo holds two revisions and `cargo xtask hardware` prints these files as where to \
             source parts, so an unlabelled one reads as a current shopping list regardless of \
             which board it is actually for. Add `# Revision: v1` or `# Revision: v2`."
        ),
    }
}

fn spec_for(rev: Revision) -> &'static BoardSpec {
    match rev {
        Revision::V1 => &V1,
        Revision::V2 => &V2,
    }
}

/// Subsystems the board spec does not model yet.
///
/// `dap/hw-spec` only carries the requirements that firmware conclusions depend
/// on — the MCU, the DAC, the clock tree. The board needs rather more than that,
/// and those parts are legitimately in the BOM while absent from the spec.
///
/// This table is the list of what is missing, which makes it a to-do list with
/// teeth rather than an escape hatch: adding a part to a BOM means either
/// pointing it at a spec requirement or naming the subsystem it serves here.
const UNMODELLED_SUBSYSTEMS: &[(&str, &str)] = &[
    ("BQ25895RTWT", "battery charge / USB-C PMIC"),
    ("AP2112K-3.3TRG1", "3V3 digital rail LDO"),
    ("LT3042EMSE", "ultra-low-noise analog rail LDO (DAC AVDD)"),
    ("W25Q128JVSIQ", "QSPI NOR flash"),
    ("STM32WB55RGV6TR", "BLE co-processor"),
    ("GDEM0397T81P", "e-ink panel module"),
    ("FH12-24S-0.5SH(55)", "panel FPC connector"),
    ("MBR0530T1G", "display boost rectifier"),
    ("SI1308EDL-T1-GE3", "display boost switch"),
    ("PESD5V0L5UY", "ESD protection array"),
];

fn unmodelled(mpn: &str) -> Option<&'static str> {
    UNMODELLED_SUBSYSTEMS
        .iter()
        .find(|(m, _)| same_part(mpn, m))
        .map(|(_, s)| *s)
}

/// Each file says which revision it bills.
#[test]
fn every_distributor_bom_declares_the_revision_it_bills() {
    for f in BOM_FILES {
        let text = read_bom(f);
        // Panics with the explanation if absent or unrecognised.
        let rev = declared_revision(&text, f);
        assert!(
            header(&text, "Updated").is_some(),
            "{f} declares a revision but no `# Updated:` date, so there is no way to tell how old \
             its prices are (revision {rev:?})"
        );
    }
}

/// **The coupling that makes the marker load-bearing.**
///
/// A declared revision that nothing checks is a comment. This reads the MCU
/// actually instantiated by the atopile schematic and requires the BOMs to bill
/// that same board — so a schematic migration cannot leave the BOMs behind, and
/// cannot leave them *claiming* V1 while billing V2 parts either.
#[test]
fn the_declared_revision_matches_the_schematic_being_billed() {
    let mcu_ato = hardware_dir().join("elec/src/mcu.ato");
    let ato = std::fs::read_to_string(&mcu_ato)
        .unwrap_or_else(|e| panic!("cannot read {}: {e}", mcu_ato.display()));

    // Which revision's MCU does the schematic instantiate?
    let schematic_rev = if ato.contains(V2.mcu.mpn) {
        Revision::V2
    } else if ato.contains(V1.mcu.mpn) {
        Revision::V1
    } else {
        panic!(
            "elec/src/mcu.ato instantiates neither {} (V1) nor {} (V2). If the schematic moved to \
             a third MCU, the spec needs a revision for it before the BOMs can bill it.",
            V1.mcu.mpn, V2.mcu.mpn
        )
    };

    for f in BOM_FILES {
        let declared = declared_revision(&read_bom(f), f);
        assert_eq!(
            declared, schematic_rev,
            "{f} says `# Revision: {declared:?}` but elec/src/mcu.ato instantiates the \
             {schematic_rev:?} MCU. A BOM bills a schematic; if the schematic migrated, this file \
             has to migrate with it — its part numbers, prices and distributor stock checks are \
             all for the other board."
        );
    }
}

/// A revision's BOM may not bill the other revision's parts.
///
/// The specific drift this catches: a v1 BOM gaining a `CS43131` row (billing a
/// part the V1 schematic has no footprint for), or — after migration — a v2 BOM
/// still listing the SDRAM that V2 deleted.
#[test]
fn no_bom_bills_a_part_from_the_other_revision() {
    for f in BOM_FILES {
        let text = read_bom(f);
        let rev = declared_revision(&text, f);
        let other = spec_for(match rev {
            Revision::V1 => Revision::V2,
            Revision::V2 => Revision::V1,
        });
        let mine = spec_mpns(spec_for(rev));

        for row in rows(&text) {
            // A part this revision also uses is fine even if the other revision
            // lists it too — shared parts are the normal case.
            if mine.iter().any(|(m, _)| same_part(&row.mpn, m)) {
                continue;
            }
            if let Some((m, pref)) = other
                .parts
                .iter()
                .flat_map(|r| r.candidates.iter())
                .find(|c| same_part(&row.mpn, c.mpn))
                .map(|c| (c.mpn, c.preference))
            {
                // Only a part the *other* revision actually commits to is a
                // problem; one it rejected is just a part nobody uses.
                if matches!(
                    pref,
                    Preference::Preferred | Preference::Alternate | Preference::Evaluating
                ) {
                    panic!(
                        "{f}:{} bills {} ({m}), which belongs to {:?} — but this file declares \
                         {rev:?}. Either the file's revision marker is wrong or it is billing a \
                         part the {rev:?} schematic has no footprint for.",
                        row.line_no, row.mpn, other.revision
                    );
                }
            }
        }
    }
}

/// Every billed row is accounted for: a spec requirement, or a named subsystem.
///
/// This is the gate that refuses "a part appeared in the BOM and nobody can say
/// what it is for".
#[test]
fn every_bom_row_resolves_to_the_spec_or_a_named_subsystem() {
    for f in BOM_FILES {
        let text = read_bom(f);
        let rev = declared_revision(&text, f);
        let mine = spec_mpns(spec_for(rev));
        let mut unresolved = Vec::new();

        for row in rows(&text) {
            assert!(
                !row.mpn.is_empty(),
                "{f}:{} has no manufacturer part number (reference {:?}) — a BOM row that names \
                 no part cannot be sourced or checked",
                row.line_no,
                row.reference
            );
            let in_spec = mine.iter().any(|(m, _)| same_part(&row.mpn, m));
            if !in_spec && unmodelled(&row.mpn).is_none() {
                unresolved.push(format!("{}:{} {}", f, row.line_no, row.mpn));
            }
        }

        assert!(
            unresolved.is_empty(),
            "these BOM rows match neither a {rev:?} spec requirement nor a named unmodelled \
             subsystem: {unresolved:?}. Either add the requirement to dap/hw-spec (preferred — \
             then the selection constraints apply to it) or name the subsystem in \
             UNMODELLED_SUBSYSTEMS so the gap is visible instead of implied."
        );
    }
}

/// The three files must describe shared parts identically.
///
/// Non-vacuous today: five parts (the two boost rectifiers, the boost MOSFET and
/// the two ESD arrays) are listed in all three files, and the FPC connector and
/// several ICs in two. A transcription slip that gave one file `SOD-323` where
/// another says `SOD-123` would order the wrong footprint, and nothing else in
/// the repo compares these files to each other.
#[test]
fn the_distributor_files_agree_on_package_and_reference_designator() {
    // reference -> (file, mpn); and mpn -> (file, package)
    let mut by_ref: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut by_mpn: BTreeMap<String, (String, String)> = BTreeMap::new();
    let mut compared = 0usize;

    for f in BOM_FILES {
        for row in rows(&read_bom(f)) {
            if let Some((prev_file, prev_mpn)) = by_ref.get(&row.reference) {
                compared += 1;
                assert!(
                    same_part(&row.mpn, prev_mpn),
                    "reference {} is {} in {prev_file} but {} in {f}:{} — the same designator on \
                     the same board must be the same part",
                    row.reference,
                    prev_mpn,
                    row.mpn,
                    row.line_no
                );
            } else {
                by_ref.insert(row.reference.clone(), ((*f).to_string(), row.mpn.clone()));
            }

            if row.package.is_empty() {
                continue;
            }
            let key = normalise(&row.mpn);
            if let Some((prev_file, prev_pkg)) = by_mpn.get(&key) {
                compared += 1;
                assert_eq!(
                    prev_pkg.to_lowercase(),
                    row.package.to_lowercase(),
                    "{} is {prev_pkg} in {prev_file} but {} in {f}:{} — one of the two will order \
                     the wrong footprint",
                    row.mpn,
                    row.package,
                    row.line_no
                );
            } else {
                by_mpn.insert(key, ((*f).to_string(), row.package.clone()));
            }
        }
    }

    assert!(
        compared >= 5,
        "this gate only compared {compared} shared entries across the three files. It is meant to \
         cross-check parts that appear in more than one distributor file; if that number has \
         dropped to near zero the gate has gone vacuous and should be re-aimed or removed rather \
         than left passing."
    );
}

/// A reference designator appears at most once per file.
///
/// Catches the copy-paste that duplicates a row — which would double-order the
/// part and, if the two copies disagree, make the file self-contradictory.
#[test]
fn reference_designators_are_unique_within_a_file() {
    for f in BOM_FILES {
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        for row in rows(&read_bom(f)) {
            if let Some(first) = seen.insert(row.reference.clone(), row.line_no) {
                panic!(
                    "{f} lists reference {} twice, at lines {first} and {} — one board position \
                     cannot be two rows",
                    row.reference, row.line_no
                );
            }
        }
    }
}

/// The MPN matcher must not treat the two candidate MCUs as the same part.
///
/// [`same_part`] matches on prefix containment, which is what makes
/// `TPA6120A2RGYR` resolve to `TPA6120A2`. That is a deliberately loose rule, and
/// the pair it must never conflate is the V1/V2 MCU — if it did,
/// `the_declared_revision_matches_the_schematic_being_billed` would silently
/// accept either board and the whole file would stop meaning anything.
#[test]
fn prefix_matching_does_not_confuse_the_two_mcus() {
    assert!(
        !same_part(V1.mcu.mpn, V2.mcu.mpn),
        "{} and {} must not compare equal",
        V1.mcu.mpn,
        V2.mcu.mpn
    );
    // The loose direction still has to work, in both orders.
    assert!(same_part("TPA6120A2RGYR", "TPA6120A2"));
    assert!(same_part("CS43131", "CS43131-CNZ"));
    assert!(same_part("LT3042EMSE#PBF", "LT3042EMSE"));
    // And an empty cell never matches anything.
    assert!(!same_part("", "CS43131"));
}

/// The quoted-comma case, pinned.
///
/// `PESD5V0L5UY,115` is a real TME symbol in these files. If the splitter ever
/// stops honouring quotes, every column after it shifts by one and the package
/// checks above start comparing descriptions — passing or failing for reasons
/// that have nothing to do with the data.
#[test]
fn csv_splitting_survives_a_part_number_containing_a_comma() {
    let f = split_csv(r#"TVS1,1,PESD5V0L5UY,"PESD5V0L5UY,115",ESD array,SOT-363,,Nexperia"#);
    assert_eq!(f.len(), 8, "expected 8 fields, got {f:?}");
    assert_eq!(f[3], "PESD5V0L5UY,115");
    assert_eq!(f[5], "SOT-363");
}
