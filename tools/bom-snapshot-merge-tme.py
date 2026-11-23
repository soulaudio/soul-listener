#!/usr/bin/env python3
"""Merge the recorded TME responses into `hardware/bom/snapshot.json`.

    python3 tools/bom-snapshot-merge-tme.py

Reads only the committed raw responses under `hardware/bom/responses/tme/` — it
makes no network calls, so it is reproducible and reviewable. Run
`tools/bom-fetch-tme.py` first to refresh those.

# Decisions this script makes, and why

**A part TME does not carry gets a zero-stock offer, not silence.** "TME has no
CS43131" is a sourcing *result*, and losing it means the next person re-runs the
same query to learn the same thing. Both snapshot gates filter on `stock > 0`, so
a zero-stock offer informs without being counted as a source.

**`lifecycle` is recorded as `unknown`, not `active`.** The TME responses used
here contain no lifecycle field. They do carry `product_status`, but
`BLOCKED_FOR_ZBL_BACKORDERS` is an *ordering restriction* rather than a lifecycle
state, and mapping it onto `lifecycle` would misreport the part. The raw status
goes in the note instead, where it cannot be mistaken for a vendor lifecycle
claim.

**`sourcing_pending` is kept wherever no EU distributor has stock.** The gap is
real: TME has been asked and answered, and Mouser/Digi-Key have not been fetched.
The reason string says exactly that, so the declaration stays honest rather than
becoming a way to keep gates quiet.
"""

from __future__ import annotations

import collections
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parent.parent
SNAPSHOT = ROOT / "hardware/bom/snapshot.json"
RESP = ROOT / "hardware/bom/responses/tme"
RETRIEVED_AT = "2026-11-23"


def load(name: str) -> dict:
    p = RESP / name
    if not p.exists():
        raise SystemExit(f"missing {p.relative_to(ROOT)} — run tools/bom-fetch-tme.py first")
    return json.loads(p.read_text())


def main() -> int:
    products = load("products.json")
    data = load("data-0.json")

    identity = {e["symbol"]: e for e in products.get("data", {}).get("elements", [])}
    pricing = {e["symbol"]: e for e in data.get("data", {}).get("elements", [])}

    snap = json.loads(SNAPSHOT.read_text(), object_pairs_hook=collections.OrderedDict)
    parts = snap["parts"]

    # TME resolves by manufacturer part number, so an MPN in the snapshot maps to
    # a TME symbol only if the lookup returned it.
    def tme_symbol_for(mpn: str) -> str | None:
        for sym, e in identity.items():
            if mpn.upper() in (s.upper() for s in (e.get("manufacturer_symbols") or [])):
                return sym
            if sym.upper() == mpn.upper():
                return sym
        return None

    changed, carried, absent = [], [], []

    for mpn, part in parts.items():
        sym = tme_symbol_for(mpn)
        offers = [o for o in part.get("offers", []) if o.get("distributor") != "tme"]

        if sym is None:
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
                            "NOT CARRIED BY TME: a GET /products lookup by mpns[] on "
                            f"{RETRIEVED_AT} returned no element for this MPN. Recorded rather "
                            "than omitted so the negative result is not re-discovered by the "
                            "next person; both snapshot gates filter on stock > 0, so this "
                            "counts as information and not as a source.",
                        ),
                    ]
                )
            )
            absent.append(mpn)
        else:
            ident = identity[sym]
            price = pricing.get(sym, {})
            breaks = [
                collections.OrderedDict([("qty", el["amount"]), ("unit_price", el["price"])])
                for el in (price.get("prices", {}) or {}).get("elements", [])
            ]
            prices_meta = price.get("prices", {}) or {}
            status = ident.get("product_status") or []
            special = any(
                el.get("special") for el in prices_meta.get("elements", [])
            )
            note = (
                f"GET /products + /products/data, country=SK, currency=EUR. Prices are "
                f"{prices_meta.get('type', '?')} of tax "
                f"{(prices_meta.get('tax') or {}).get('rate', '?')}% "
                f"({(prices_meta.get('tax') or {}).get('type', '?')}). "
                f"special={special} on every break, so these are LIST prices and not "
                f"account-negotiated — reproducible by anyone with an API key."
            )
            if status:
                note += (
                    f" TME product_status={status}. Note BLOCKED_FOR_ZBL_BACKORDERS is an "
                    "ORDERING restriction (no zero-balance backorders accepted), not a "
                    "lifecycle state, which is why lifecycle stays 'unknown' rather than "
                    "being inferred from it."
                )
            offers.append(
                collections.OrderedDict(
                    [
                        ("distributor", "tme"),
                        ("distributor_pn", sym),
                        ("stock", price.get("stock_quantity", 0)),
                        ("moq", ident.get("minimal_amount")),
                        ("order_multiple", ident.get("multiples")),
                        ("lead_time_days", None),
                        ("price_breaks", breaks),
                        ("currency", (prices_meta.get("currency") or "EUR")),
                        # TME returns no lifecycle field; see the module docstring.
                        ("lifecycle", "unknown"),
                        ("compliance", []),
                        ("datasheet_url", None),
                        ("source", "api"),
                        ("retrieved_at", RETRIEVED_AT),
                        ("note", note),
                    ]
                )
            )
            carried.append((mpn, price.get("stock_quantity", 0)))

        part["offers"] = offers

        # Is there now ANY EU-stocked offer? The snapshot's distributors are all
        # EU-flagged, so stock > 0 anywhere is enough to retire the declared gap.
        stocked = [o for o in offers if (o.get("stock") or 0) > 0]
        if stocked and "sourcing_pending" in part:
            del part["sourcing_pending"]
            changed.append(f"{mpn}: sourcing_pending cleared ({len(stocked)} stocked offer(s))")
        elif not stocked and "sourcing_pending" in part:
            part["sourcing_pending"]["reason"] = (
                "TME has been fetched and reports no stock for this part"
                + (" (and does not carry it)" if sym is None else "")
                + "; Mouser and Digi-Key have not been fetched yet, so the EU sourcing question "
                "is still open rather than answered negatively."
            )
            part["sourcing_pending"]["since"] = RETRIEVED_AT

    snap["generated_at"] = RETRIEVED_AT
    snap["generator"] = (
        "tools/bom-fetch-tme.py + tools/bom-snapshot-merge-tme.py (TME API v2); "
        "Mouser and Digi-Key offers are still manual-research"
    )
    SNAPSHOT.write_text(json.dumps(snap, indent=2) + "\n")

    print(f"carried by TME ({len(carried)}):")
    for mpn, st in carried:
        print(f"  {mpn:<26} stock={st}")
    print(f"\nnot carried by TME ({len(absent)}):")
    for mpn in absent:
        print(f"  {mpn}")
    if changed:
        print("\nchanges:")
        for c in changed:
            print(f"  {c}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
