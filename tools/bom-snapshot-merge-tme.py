#!/usr/bin/env python3
"""Regenerate `hardware/bom/snapshot.json` from the committed catalogue DB.

    python3 tools/bom-snapshot-merge-tme.py

Reads only `hardware/bom/catalogue.db` — no network — so it is reproducible and
the result lands as a reviewable diff. Run `tools/sourcing-run.py fetch` first to
refresh the DB.

# What the snapshot is for, versus the DB

The DB is the wide discovery surface: hundreds of parts per category, rebuildable,
uncommitted. The snapshot is the **decision record**: only the parts the spec
names, with the figures the gates assert, small enough to diff. When a part the
design depends on drops to zero stock or goes reel-only, that has to show up in a
pull request rather than change silently underneath CI.

`catalogue_coverage` is the one place the two meet — a per-category summary of
what was actually swept, so the committed artefact says how much of the market
the conclusions rest on. Without it, "no stocked oscillator at 24.576 MHz" is an
assertion; with it, it is an assertion plus the size of the search behind it.

# Decisions this script makes, and why

**A part the vendor does not carry gets a zero-stock offer, not silence.** "TME
has no CS43131" is a sourcing *result*; dropping it means the next person re-runs
the query to learn the same thing. Both snapshot gates filter on `stock > 0`, so a
zero-stock offer informs without counting as a source.

**Identity is matched on either identifier.** TME holds the documented battery
cell as symbol `AKY-LP623255` with manufacturer PN `AKY0646`. Matching one field
only is what made an earlier run report "not carried" for a part with 1298 in
stock.

**`lifecycle` stays `unknown`.** These responses carry no lifecycle field.
`product_status` is an *ordering* restriction (`BLOCKED_FOR_ZBL_BACKORDERS` means
no zero-balance backorders), not a lifecycle state, so it goes in the note where
it cannot be mistaken for a vendor lifecycle claim.

**`sourcing_pending` is kept wherever nothing has stock**, with a reason saying
what is actually true now rather than a generic placeholder.
"""

from __future__ import annotations

import collections
import json
import pathlib
import sqlite3

ROOT = pathlib.Path(__file__).resolve().parent.parent
SNAPSHOT = ROOT / "hardware/bom/snapshot.json"
DB = ROOT / "hardware/bom/catalogue.db"
RETRIEVED_AT = "2026-11-24"

PENDING_NOTE = (
    "Recorded from a real category sweep rather than guessed. A Hard lock does not accept "
    "sourcing_pending."
)


def main() -> int:
    if not DB.exists():
        raise SystemExit(f"no catalogue at {DB.relative_to(ROOT)} — run tools/sourcing-run.py fetch")
    conn = sqlite3.connect(DB)
    snap = json.loads(SNAPSHOT.read_text(), object_pairs_hook=collections.OrderedDict)

    carried, absent = [], []
    for mpn, part in snap["parts"].items():
        row = conn.execute(
            """
            SELECT id, vendor_pn, stock, moq, order_multiple, lead_time_days,
                   status_raw, datasheet_url
            FROM parts
            WHERE vendor = 'tme' AND (mpn = ? COLLATE NOCASE OR vendor_pn = ? COLLATE NOCASE)
            LIMIT 1
            """,
            (mpn, mpn),
        ).fetchone()

        offers = [o for o in part.get("offers", []) if o.get("distributor") != "tme"]

        if row is None:
            offers.append(
                collections.OrderedDict(
                    [
                        ("distributor", "tme"),
                        ("distributor_pn", None),
                        ("stock", 0),
                        ("moq", None),
                        ("order_multiple", None),
                        ("lead_time_days", None),
                        ("price_breaks", []),
                        ("currency", "EUR"),
                        ("lifecycle", "unknown"),
                        ("compliance", []),
                        ("datasheet_url", None),
                        ("source", "api"),
                        ("retrieved_at", RETRIEVED_AT),
                        (
                            "note",
                            "NOT CARRIED BY TME: looked up by both manufacturer part number and "
                            f"TME symbol on {RETRIEVED_AT} with no match. Recorded rather than "
                            "omitted so the negative result is not re-discovered; both snapshot "
                            "gates filter on stock > 0, so this counts as information and not as "
                            "a source.",
                        ),
                    ]
                )
            )
            absent.append(mpn)
        else:
            pid, vendor_pn, stock, moq, mult, lead, status_raw, ds = row
            breaks = [
                collections.OrderedDict([("qty", q), ("unit_price", up)])
                for q, up in conn.execute(
                    "SELECT qty, unit_price FROM price_breaks WHERE part_id=? ORDER BY qty",
                    (pid,),
                )
            ]
            status = json.loads(status_raw or "[]")
            note = (
                f"TME API v2, country=SK, currency=EUR. MOQ {moq}, order multiple {mult}: "
                f"stock {stock} does not mean one unit is purchasable."
            )
            if status:
                note += (
                    f" TME product_status={status}. BLOCKED_FOR_ZBL_BACKORDERS is an ORDERING "
                    "restriction (no zero-balance backorders accepted), not a lifecycle state, "
                    "which is why lifecycle stays 'unknown' rather than being inferred from it."
                )
            if vendor_pn and vendor_pn.upper() != mpn.upper():
                note += f" TME indexes this part as symbol {vendor_pn}."
            offers.append(
                collections.OrderedDict(
                    [
                        ("distributor", "tme"),
                        ("distributor_pn", vendor_pn),
                        ("stock", stock or 0),
                        ("moq", moq),
                        ("order_multiple", mult),
                        ("lead_time_days", lead),
                        ("price_breaks", breaks),
                        ("currency", "EUR"),
                        ("lifecycle", "unknown"),
                        ("compliance", []),
                        ("datasheet_url", ds),
                        ("source", "api"),
                        ("retrieved_at", RETRIEVED_AT),
                        ("note", note),
                    ]
                )
            )
            carried.append((mpn, stock or 0, moq))

        part["offers"] = offers
        stocked = [o for o in offers if (o.get("stock") or 0) > 0]
        if stocked and "sourcing_pending" in part:
            del part["sourcing_pending"]
        elif not stocked and "sourcing_pending" in part:
            part["sourcing_pending"]["reason"] = (
                "TME has been fetched and reports no stock"
                + ("" if row is not None else " (and does not carry this part)")
                + "; Mouser and Digi-Key are unfetched, so the EU sourcing question is still open "
                "rather than answered negatively."
            )
            part["sourcing_pending"]["since"] = RETRIEVED_AT
            part["sourcing_pending"]["note"] = PENDING_NOTE

    # How much of the market the conclusions rest on.
    coverage = collections.OrderedDict()
    for cat, n, stocked, with_ds in conn.execute(
        """
        SELECT category, COUNT(*), SUM(CASE WHEN stock > 0 THEN 1 ELSE 0 END),
               SUM(CASE WHEN datasheet_url IS NOT NULL THEN 1 ELSE 0 END)
        FROM parts WHERE vendor='tme' AND category IS NOT NULL
        GROUP BY category ORDER BY category
        """
    ):
        coverage[cat] = collections.OrderedDict(
            [("parts_swept", n), ("stocked", stocked), ("with_datasheet", with_ds)]
        )

    snap["generated_at"] = RETRIEVED_AT
    snap["generator"] = (
        "tools/sourcing-run.py fetch (TME API v2) + tools/bom-snapshot-merge-tme.py; "
        "Mouser and Digi-Key offers are still manual-research"
    )
    snap["catalogue_coverage"] = collections.OrderedDict(
        [
            (
                "readme",
                "Per-requirement TME sweep sizes behind this snapshot. The catalogue DB itself "
                "(hardware/bom/catalogue.db) is rebuildable and NOT committed; this is the part "
                "of it the gates depend on. A category marked as a sample was capped by a --limit "
                "and is not the whole category.",
            ),
            ("vendor", "tme"),
            ("by_requirement", coverage),
        ]
    )
    SNAPSHOT.write_text(json.dumps(snap, indent=2) + "\n")

    print(f"carried by TME ({len(carried)}):")
    for mpn, st, moq in carried:
        print(f"  {mpn:<24} stock={st:<7} moq={moq}")
    print(f"\nnot carried ({len(absent)}): {absent}")
    print("\ncatalogue coverage:")
    for cat, c in coverage.items():
        print(f"  {cat:<16} swept={c['parts_swept']:<5} stocked={c['stocked']:<5} "
              f"datasheets={c['with_datasheet']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
