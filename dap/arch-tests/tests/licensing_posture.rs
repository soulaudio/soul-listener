//! The licensing posture must agree with the code and the board.
//!
//! `dap_hw_spec::licensing` holds the decisions; its own unit tests check that
//! the table is internally coherent. This file checks the thing a `no_std` spec
//! crate cannot: that the decisions match **what is actually built**.
//!
//! That gap is where a licensing claim goes stale. `EXPOSURES` can say AAC is
//! declined for as long as anyone likes while a dependency quietly enables the
//! feature; the spec has no way to look. These tests look.
//!
//! See `docs/research/2026-11-18-licensing-ip-and-workarounds.md`.

use dap_hw_spec::licensing::{
    regime_for, ComplianceRegime, Exposure, RoyaltyStatus, BATTERY_CERT_CONDITIONS, EXPOSURES,
};
use dap_hw_spec::select::{Evidence, Predicate};
use dap_hw_spec::{Preference, ACTIVE};

fn exposure(id: &str) -> &'static Exposure {
    EXPOSURES
        .iter()
        .find(|e| e.id == id)
        .unwrap_or_else(|| panic!("the licensing posture must record an exposure for '{id}'"))
}

/// The AAC decision, cross-checked against the resolved build.
///
/// `aac_license.rs` asserts the feature is off. This asserts the *posture* still
/// says it should be — so the two cannot drift apart in either direction: a
/// posture that quietly relaxed to `Licensed` while the gate kept failing builds,
/// or a posture still claiming `Declined` after someone decided to pay.
#[test]
// `AAC_ENABLED` is a const, so clippy sees a constant assertion. That is the
// point — it is `cfg!(feature = "aac")` evaluated inside soul-audio-core, which
// makes it a fact about the resolved graph rather than about this file.
// `aac_license.rs` carries the same allow for the same reason.
#[allow(clippy::assertions_on_constants)]
fn the_posture_and_the_build_agree_about_aac() {
    let aac = exposure("aac");
    match aac.status {
        RoyaltyStatus::Declined { instead } => {
            assert!(
                !instead.is_empty(),
                "declining AAC without naming what ships instead reads as a missing feature \
                 rather than a decision"
            );
            assert!(
                !soul_audio_core::AAC_ENABLED,
                "the licensing posture says AAC is DECLINED, but soul-audio-core was resolved \
                 with the `aac` feature enabled — so the patented decoder is in this build. One \
                 of the two is wrong, and the expensive one is the build: Via LA is $15,000 \
                 one-time plus $0.98 per unit, and a clean-room decoder is still patent-bearing."
            );
        }
        other => panic!(
            "AAC is recorded as {other:?}. If paying for it is now the intent, say so in the \
             licensing research and update its §4 takeaway — the per-unit royalty claim in §4.1 \
             is the thing this changes."
        ),
    }
}

/// Every declined exposure's enforcement reference must point at something real.
///
/// A gate named in `enforced_by` that does not exist is worse than an empty
/// field, because it reads as covered. Test names are checked against the test
/// sources; the non-test mechanisms describe themselves and are checked only for
/// being non-empty by the spec's own unit test.
#[test]
fn named_enforcement_gates_actually_exist() {
    const TEST_SOURCES: &[(&str, &str)] = &[
        ("aac_license.rs", include_str!("aac_license.rs")),
        ("selection.rs", include_str!("selection.rs")),
        ("sourcing_snapshot.rs", include_str!("sourcing_snapshot.rs")),
        ("budget_lines.rs", include_str!("budget_lines.rs")),
        ("licensing_posture.rs", include_str!("licensing_posture.rs")),
        (
            "filesystem_swappability.rs",
            include_str!("filesystem_swappability.rs"),
        ),
    ];

    let mut checked = 0usize;
    for e in EXPOSURES {
        // Only references that look like a Rust path to a test function are
        // mechanically checkable. The rest ("artwork review", "no SDA membership
        // exists") are honest prose and say so.
        let Some(fn_name) = e.enforced_by.rsplit("::").next() else {
            continue;
        };
        if !e.enforced_by.contains("::") || fn_name.contains(' ') {
            continue;
        }
        checked += 1;
        let found = TEST_SOURCES
            .iter()
            .any(|(_, src)| src.contains(&format!("fn {fn_name}(")));
        assert!(
            found,
            "exposure '{}' says it is enforced by `{}`, but no test function `{fn_name}` exists \
             in dap-arch-tests. A named gate that does not exist is worse than naming none, \
             because it reads as covered.",
            e.id, e.enforced_by
        );
    }
    assert!(
        checked > 0,
        "no exposure named a checkable gate, so this test verified nothing. If every enforcement \
         has become prose, that is itself worth knowing — but it should not pass silently."
    );
}

/// The compliance regime follows from whether a radio is populated, and the
/// shipped board does not populate one.
///
/// The coupling is the point: the licensing research records this as a
/// *correction* to an earlier spec that assumed the RED throughout. A wired-only
/// device falls under the EMC Directive, which is one lab test rather than a
/// radio programme — and populating the module silently flips that back.
#[test]
fn the_shipped_board_is_an_emc_device_because_no_radio_is_populated() {
    let bt = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "bluetooth")
        .expect("the spec must carry a bluetooth requirement even while it is unpopulated");

    // Populated means a part is actually committed to — Preferred or Alternate.
    // `Evaluating` is the DNP state: specified, not fitted.
    let populated = bt
        .candidates
        .iter()
        .any(|c| matches!(c.preference, Preference::Preferred | Preference::Alternate));

    assert!(
        !populated,
        "a bluetooth candidate is marked Preferred or Alternate, which means the radio is \
         committed to. That moves the device from the EMC Directive (2014/30/EU) to the RED \
         (2014/53/EU), adds a Bluetooth SIG end-product Declaration (~EUR 5k) and needs an \
         antenna window in the case. Those three arrive together. If the intent is a Bluetooth \
         SKU, update the licensing research and this test deliberately — do not let a preference \
         change carry it."
    );
    assert_eq!(
        regime_for(populated),
        ComplianceRegime::Emc,
        "with no radio populated the device is an EMC-Directive product"
    );
}

/// The conditional Bluetooth cost must stay conditional while the module is DNP.
#[test]
fn the_bluetooth_fee_is_recorded_as_conditional_while_the_module_is_unpopulated() {
    let bt = exposure("bluetooth-sig");
    assert!(
        matches!(bt.status, RoyaltyStatus::OwedIfEnabled { .. }),
        "the Bluetooth SIG Declaration is recorded as {:?} while the module is not populated. \
         `Free` would claim it never costs anything; `Licensed` would claim it is already paid. \
         Neither is true.",
        bt.status
    );
}

/// The battery requirement must enforce the condition that keeps certification
/// with the cell vendor.
///
/// `BATTERY_CERT_CONDITIONS` lists four conditions. Three are process rules about
/// how we buy and what we archive, and no test can hold them. One —
/// *a finished cell with its own PCM* — is a property of the part, so it belongs
/// in the filter, and this asserts it is actually there rather than only written
/// down.
#[test]
fn the_integrated_pcm_condition_is_a_filter_and_not_only_a_note() {
    let battery = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "battery")
        .expect("the spec must carry a battery requirement");

    // Checked by CONDITION rather than by attribute name. The attribute moved
    // from an invented `integrated_pcm` to the vendor's own `battery_features`
    // once a real sweep showed TME publishes 'PCB protection' there — so pinning
    // the name would have failed for the right change.
    let pcm = battery
        .constraints
        .iter()
        .find(|c| {
            matches!(c.pred, Predicate::Contains(n) if n.to_lowercase().contains("protection"))
                || c.attr.eq_ignore_ascii_case("integrated_pcm")
        })
        .unwrap_or_else(|| {
            panic!(
                "the battery requirement does not check that the cell has its own protection \
                 circuit. That is the one condition in BATTERY_CERT_CONDITIONS which is a \
                 property of the PART rather than of our process, and dropping it lets a bare \
                 cell qualify — making us the battery manufacturer and transferring UN 38.3 and \
                 IEC 62133-2 testing to us. Conditions:\n  - {}",
                BATTERY_CERT_CONDITIONS.join("\n  - ")
            )
        });
    assert_eq!(
        pcm.evidence,
        Evidence::Catalogue,
        "the protection-circuit condition is marked datasheet-only, but distributors DO publish \
         it: TME lists 'PCB protection' under its features parameter for all 70 Li-Po cells. \
         Demoting a checkable condition to a human task loses the check."
    );

    // The transport-document conditions are per model, so both reports have to be
    // obtainable for whichever cell is selected.
    for attr in ["un38_3_report", "iec62133_2_report"] {
        assert!(
            battery
                .constraints
                .iter()
                .any(|c| c.attr.eq_ignore_ascii_case(attr)),
            "the battery requirement does not constrain `{attr}`. Certification is per model, so \
             a vendor's general compliance claim is not the document that ships with the product."
        );
    }
}

/// The spare-battery feature rests on one part property, so it must be a filter.
///
/// "Users can buy a second battery and swap it" is a product promise. At part
/// level it reduces entirely to the cell being connectorised rather than
/// soldered — and the soldered alternatives are consistently cheaper and bigger,
/// so nothing except this constraint would prevent one being chosen.
#[test]
fn the_swappable_battery_promise_is_enforced_at_part_level() {
    let battery = ACTIVE
        .parts
        .iter()
        .find(|r| r.id == "battery")
        .expect("the spec must carry a battery requirement");

    let termination = battery
        .constraints
        .iter()
        .find(|c| c.attr.eq_ignore_ascii_case("termination"))
        .expect(
            "the battery requirement does not constrain `termination`. Being able to buy a spare \
             cell and swap it depends on exactly that: a connectorised cell can be unplugged, a \
             solder-tab cell cannot. Without this constraint the cheapest, highest-capacity, \
             deepest-stocked cells — which are soldered — would win on every other axis.",
        );

    assert!(
        !termination.why.is_empty(),
        "the termination constraint must say why it exists, since a reader will otherwise take it \
         for a connector preference rather than a user-replaceability requirement"
    );
}
