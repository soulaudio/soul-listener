//! The licensing posture, as data.
//!
//! `docs/research/2026-11-18-licensing-ip-and-workarounds.md` establishes the
//! claim this project rests on commercially: **no per-unit royalty anywhere in
//! the device as designed.** That claim is only true because of a set of
//! decisions — AAC dropped, exFAT deferred, no logos printed, Bluetooth not
//! populated — every one of which is a decision someone could reverse by
//! accident.
//!
//! A table in a document cannot notice that. So the exposures live here as typed
//! data, and the invariants in this module assert the property the document
//! claims. The important one is not "is the list complete" (no test can know
//! that) but **"does every declined item name the thing that stops it coming
//! back"** — because `Declined` is worth nothing if nothing enforces it.
//!
//! # Why a hardware spec crate carries licensing
//!
//! Because several of these are *hardware* facts with firmware consequences, and
//! the coupling is the part that gets missed. Populating the Bluetooth module
//! moves the device from the EMC Directive to the RED, adds a Bluetooth SIG
//! end-product Declaration, and needs an antenna window in the case. Those three
//! arrive together or not at all, and [`regime_for`] is where that is written
//! down once.

/// What a given exposure costs, and therefore what has to be true about it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoyaltyStatus {
    /// Royalty-free. Ship it; nothing to enforce.
    Free,
    /// Patented, trademarked or otherwise fee-bearing, and deliberately **not**
    /// shipped. The cost is zero *because of* the decision, which is why
    /// `Exposure::enforced_by` is mandatory for this variant.
    Declined {
        /// What ships instead, so declining is a choice rather than a gap.
        instead: &'static str,
    },
    /// Shipped and paid for, per unit and/or once.
    ///
    /// Present as a variant so that choosing to pay is *representable* — the
    /// gates below do not forbid it, they forbid it being invisible.
    Licensed {
        per_unit_eur_centi: u32,
        one_time_eur: u32,
    },
    /// Owed only if something is enabled or populated. Zero today, priced for
    /// the day someone flips it.
    OwedIfEnabled {
        /// The condition in words.
        trigger: &'static str,
        per_unit_eur_centi: u32,
        one_time_eur: u32,
    },
}

/// One licensing/IP exposure.
#[derive(Debug, Clone, Copy)]
pub struct Exposure {
    pub id: &'static str,
    pub item: &'static str,
    pub status: RoyaltyStatus,
    /// Where the decision is actually *enforced* — a test name, a feature flag,
    /// a spec field. Empty is allowed only for [`RoyaltyStatus::Free`], where
    /// there is nothing to enforce.
    ///
    /// This field is the whole point of the module. "We dropped AAC" is a
    /// sentence; `aac_decoder_is_never_enabled_on_the_dap` is a gate.
    pub enforced_by: &'static str,
    /// Section of the licensing research this comes from.
    pub source: &'static str,
}

/// Which EU directive the device is placed on the market under.
///
/// Not interchangeable: the RED requires radio testing and a Notified-Body path
/// consideration that the EMC Directive does not, and the licensing research
/// records this as a correction to an earlier spec that had assumed RED
/// throughout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComplianceRegime {
    /// EMC Directive 2014/30/EU — a wired-only device has no radio.
    Emc,
    /// Radio Equipment Directive 2014/53/EU — applies as soon as a radio is
    /// populated, even a pre-certified module.
    Red,
}

/// The regime follows from one fact: is a radio populated?
///
/// Written as a function rather than a constant so the coupling cannot be
/// half-applied. A pre-certified module is *not* an exemption — the licensing
/// research notes Tangara still needed its own EMC test and FCC filing with a
/// pre-qualified ESP32.
#[must_use]
pub const fn regime_for(radio_populated: bool) -> ComplianceRegime {
    if radio_populated {
        ComplianceRegime::Red
    } else {
        ComplianceRegime::Emc
    }
}

/// The conditions under which battery certification belongs to the **cell
/// vendor** rather than to us.
///
/// All four must hold. The first is the one with teeth in part selection: buying
/// bare cells and adding our own protection circuit would make us the battery
/// manufacturer, inheriting UN 38.3 and IEC 62133-2 testing. That is why
/// `integrated_pcm` is a hard constraint on the battery requirement and not a
/// preference.
pub const BATTERY_CERT_CONDITIONS: &[&str] = &[
    "a finished, pre-certified cell with its own PCM — not bare cells we protect ourselves",
    "the cell and its protection circuit are not modified",
    "the vendor actually issues the per-model docs (UN 38.3 summary, IEC 62133-2 report)",
    "docs are re-collected per model: switching cells means obtaining THAT model's reports",
];

/// The exposure list for the shipped, wired-only device.
///
/// Ordered as in the research document's table so the two can be read together.
pub const EXPOSURES: &[Exposure] = &[
    Exposure {
        id: "mp3",
        item: "MP3 decode",
        status: RoyaltyStatus::Free,
        enforced_by: "",
        source: "licensing §2.1 — patents expired 2017",
    },
    Exposure {
        id: "aac",
        item: "AAC-LC / HE-AAC decode",
        status: RoyaltyStatus::Declined {
            instead: "FLAC/ALAC/Opus/Vorbis/WAV, all royalty-free",
        },
        // The strongest enforcement in the project: a clean-room decoder is
        // still patent-bearing, so the gate asserts the cargo feature is off in
        // the resolved graph rather than trusting a default.
        enforced_by: "dap-arch-tests::aac_license::aac_decoder_is_never_enabled_on_the_dap",
        source: "licensing §5 — Via LA, $15k one-time + $0.98/unit",
    },
    Exposure {
        id: "lossless-codecs",
        item: "FLAC / ALAC / Opus / Vorbis / WAV",
        status: RoyaltyStatus::Free,
        enforced_by: "",
        source: "licensing §2.3",
    },
    Exposure {
        id: "exfat",
        item: "exFAT filesystem",
        status: RoyaltyStatus::Declined {
            instead: "FAT32, accepting the 4 GB/file cap; TB cards are reformatted",
        },
        enforced_by: "dap-storage-fat32 is the only filesystem implementation in the tree",
        source: "licensing §6 — Microsoft patents; the kernel pledge does not cover bare metal",
    },
    Exposure {
        id: "sd-logo",
        item: "SD / microSD logo and full specification",
        status: RoyaltyStatus::Declined {
            instead: "the public Simplified Specification in SPI/SD mode, and no SD logo printed",
        },
        // Enforced by what is absent: SDA membership buys the logo and the full
        // spec, and we use neither. The honest enforcement point is the artwork
        // review, which no test can perform — recorded as such rather than
        // claimed.
        enforced_by: "no SDA membership exists, so no logo may be placed on artwork or packaging \
                      — a release checklist item, not a test",
        source: "licensing §2.10 — SDA membership ~$2k/yr for the logo",
    },
    Exposure {
        id: "hi-res-logos",
        item: "\"Hi-Res Audio\" / DSD / aptX / LDAC logos",
        status: RoyaltyStatus::Declined {
            instead: "plain-text claims only (\"plays DSD256\", \"24-bit/384 kHz\")",
        },
        enforced_by: "no brand agreement exists — artwork review, not a test",
        source: "licensing §2.11",
    },
    Exposure {
        id: "usb-vid",
        item: "USB Vendor ID",
        status: RoyaltyStatus::Free,
        enforced_by: "",
        source: "licensing §6 — pid.codes PID under VID 0x1209, free for open-source designs",
    },
    Exposure {
        id: "bluetooth-sig",
        item: "Bluetooth SIG end-product Declaration",
        status: RoyaltyStatus::OwedIfEnabled {
            trigger: "the Bluetooth module is populated (it is DNP on the shipped board)",
            per_unit_eur_centi: 0,
            one_time_eur: 5_000,
        },
        enforced_by: "dap-hw-spec::licensing::regime_for, and the bluetooth requirement's \
                      pre-qualification constraints",
        source: "licensing §6 — inherit the module's QDID, file the Declaration",
    },
    Exposure {
        id: "bluetooth-codecs",
        item: "aptX / LDAC",
        status: RoyaltyStatus::Declined {
            instead: "SBC, the royalty-free mandatory codec — as Tangara ships",
        },
        enforced_by: "the bluetooth requirement constrains codec_sbc and does not require aptX \
                      or LDAC, so no module is selected for them",
        source: "licensing §2.13",
    },
    Exposure {
        id: "dac-vendor-nda",
        item: "DAC/BT vendor NDA and gated SDK",
        status: RoyaltyStatus::Declined {
            instead: "open-datasheet parts (Cirrus/TI/ST) and a pre-made module that hides the \
                      radio vendor's NDA",
        },
        enforced_by: "the bluetooth requirement constrains requires_vendor_nda to false",
        source: "licensing §2.14",
    },
    Exposure {
        id: "mqa",
        item: "MQA",
        status: RoyaltyStatus::Declined {
            instead: "plain PCM/DSD; MQA Ltd went bankrupt in 2023",
        },
        enforced_by: "no MQA decoder exists in any soul crate",
        source: "licensing §2.4",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// The commercial claim, as a test: nothing shipped costs money per unit.
    ///
    /// This does not forbid licensing something — it forbids licensing it
    /// *quietly*. A `Licensed` entry with a per-unit fee fails here and has to be
    /// argued for explicitly, which is the correct amount of friction for a
    /// decision that changes the unit economics of every device sold.
    #[test]
    fn no_shipped_feature_carries_a_per_unit_royalty() {
        for e in EXPOSURES {
            if let RoyaltyStatus::Licensed {
                per_unit_eur_centi, ..
            } = e.status
            {
                assert_eq!(
                    per_unit_eur_centi, 0,
                    "'{}' ({}) is licensed at {} cents per unit. The project's stated position is \
                     ZERO per-unit royalty in the device as designed ({}). If this is a deliberate \
                     reversal, say so in the licensing research and update its §4 takeaway — do \
                     not let it arrive as a data edit.",
                    e.id, e.item, per_unit_eur_centi, e.source
                );
            }
        }
    }

    /// A declined exposure must name what stops it coming back.
    ///
    /// The entire value of recording "we dropped AAC" is that something prevents
    /// the next person from enabling it. An entry that declines something and
    /// points at nothing is a comment wearing a type.
    #[test]
    fn every_declined_or_conditional_exposure_names_its_enforcement() {
        for e in EXPOSURES {
            let needs_enforcement = matches!(
                e.status,
                RoyaltyStatus::Declined { .. } | RoyaltyStatus::OwedIfEnabled { .. }
            );
            if needs_enforcement {
                assert!(
                    !e.enforced_by.is_empty(),
                    "'{}' ({}) is declined or conditional, but names nothing that enforces it. \
                     Either point at the gate, feature flag or spec field that holds the line, or \
                     say plainly that it is a checklist item — an empty field reads as enforced \
                     when it is not.",
                    e.id,
                    e.item
                );
            }
            assert!(!e.source.is_empty(), "'{}' cites no source section", e.id);
        }
    }

    /// Free things have nothing to enforce, and should not pretend to.
    #[test]
    fn free_exposures_do_not_claim_enforcement() {
        for e in EXPOSURES {
            if matches!(e.status, RoyaltyStatus::Free) {
                assert!(
                    e.enforced_by.is_empty(),
                    "'{}' is royalty-free but names an enforcement mechanism ({}). Nothing needs \
                     holding; if the field is doing something else, it belongs in the source note.",
                    e.id,
                    e.enforced_by
                );
            }
        }
    }

    /// Populating a radio changes the directive. Both directions.
    #[test]
    fn a_populated_radio_moves_the_device_to_the_red() {
        assert_eq!(regime_for(false), ComplianceRegime::Emc);
        assert_eq!(regime_for(true), ComplianceRegime::Red);
        assert_ne!(
            regime_for(true),
            regime_for(false),
            "the two regimes must differ, or the coupling this function exists to express is lost"
        );
    }

    /// The conditional Bluetooth cost must actually be priced.
    ///
    /// "Owed if enabled" with a zero one-time fee would quietly say the SIG
    /// Declaration is free, which is the specific mistake that makes a BT SKU
    /// look cheaper than it is.
    #[test]
    fn the_conditional_bluetooth_cost_is_priced_not_zeroed() {
        let bt = EXPOSURES
            .iter()
            .find(|e| e.id == "bluetooth-sig")
            .expect("the Bluetooth SIG exposure must exist while a BT module is in the design");
        match bt.status {
            RoyaltyStatus::OwedIfEnabled {
                one_time_eur,
                trigger,
                ..
            } => {
                assert!(
                    one_time_eur > 0,
                    "the Bluetooth SIG Declaration is recorded as costing nothing. It does not; \
                     the research puts it at ~$5k. A zero here makes a Bluetooth SKU look free."
                );
                assert!(
                    !trigger.is_empty(),
                    "a conditional cost with no stated trigger cannot be evaluated"
                );
            }
            other => panic!(
                "the Bluetooth SIG fee should be OwedIfEnabled while the module is DNP, got \
                 {other:?}"
            ),
        }
    }

    /// Exposure ids are unique — they are how other tests and docs refer to these.
    #[test]
    fn exposure_ids_are_unique() {
        for (i, a) in EXPOSURES.iter().enumerate() {
            for b in EXPOSURES.iter().skip(i + 1) {
                assert_ne!(a.id, b.id, "duplicate exposure id '{}'", a.id);
            }
        }
    }

    /// All four battery conditions are recorded, because dropping any one of them
    /// moves the certification burden onto us.
    #[test]
    fn the_battery_certification_conditions_are_all_present() {
        assert_eq!(
            BATTERY_CERT_CONDITIONS.len(),
            4,
            "the licensing research lists four conditions for the cell vendor holding the \
             certification; this list has {}. Removing one does not relax the requirement, it \
             silently transfers UN 38.3 and IEC 62133-2 testing to us.",
            BATTERY_CERT_CONDITIONS.len()
        );
        assert!(
            BATTERY_CERT_CONDITIONS
                .iter()
                .any(|c| c.contains("own PCM")),
            "the integrated-PCM condition is the one that constrains part selection and must stay \
             stated: protecting bare cells ourselves would make us the battery manufacturer"
        );
    }
}
