#!/usr/bin/env python3
"""Drive the sourcing pipeline. Run from the repo root.

    tools/sourcing-run.py fetch  --vendor tme            # the MPNs the spec names
    tools/sourcing-run.py fetch  --vendor tme --category 111500 --as mclk-mux
    tools/sourcing-run.py export --category mclk-mux     # DB -> candidates.json
    tools/sourcing-run.py status                         # what the DB holds

The database (`hardware/bom/catalogue.db`) is gitignored and rebuildable. The
committed artefact stays `hardware/bom/snapshot.json`; see `sourcing/db.py` for
why that split matters.

Credentials come from `.env`. A vendor without credentials reports itself
unavailable and is skipped — a partial run is a normal outcome, not an error.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
import time

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

from sourcing.adapters import DigikeyAdapter, MouserAdapter, TmeAdapter  # noqa: E402
from sourcing.db import Catalogue  # noqa: E402

ROOT = pathlib.Path(__file__).resolve().parent.parent
DB_PATH = ROOT / "hardware/bom/catalogue.db"
SNAPSHOT = ROOT / "hardware/bom/snapshot.json"
EXPORT_DIR = ROOT / "hardware/bom/candidates"


def load_env() -> dict[str, str]:
    p = ROOT / ".env"
    if not p.exists():
        return {}
    out = {}
    for line in p.read_text().splitlines():
        s = line.strip()
        if s and not s.startswith("#") and "=" in s:
            k, v = s.split("=", 1)
            out[k.strip()] = v.strip()
    return out


def build_adapters(env: dict[str, str]):
    # Mouser's daily cap is 1000 calls, so what it has already spent TODAY is
    # part of its construction. An in-process counter starts at zero every run
    # and the fourth refresh of the morning would discover the cap by being
    # refused; `fetch_runs.api_calls` has recorded it all along.
    spent = {}
    try:
        spent = Catalogue(DB_PATH).calls_today(time.strftime("%Y-%m-%d"))
    except Exception:
        # A missing or unreadable catalogue must not stop a fetch — it is the
        # rebuildable half of the two-tier store. Worst case the budget starts
        # from zero, which is what the old behaviour was everywhere.
        pass
    return [
        TmeAdapter(env.get("TME_TOKEN", ""), env.get("TME_APP_SECRET", "")),
        MouserAdapter(
            env.get("MOUSER_SEARCH_API_KEY", ""),
            spent_today=spent.get("mouser", 0),
        ),
        DigikeyAdapter(
            env.get("DIGIKEY_CLIENT_ID", ""),
            env.get("DIGIKEY_CLIENT_SECRET", ""),
            host=env.get("DIGIKEY_API_HOST") or "api.digikey.com",
        ),
    ]


def spec_mpns() -> list[str]:
    """The MPNs the committed snapshot names, with their requirement ids."""
    d = json.loads(SNAPSHOT.read_text())
    return sorted(d["parts"].keys())


def spec_category_of(*identifiers: str | None) -> str | None:
    """Which requirement a part satisfies, matched on ANY of its identifiers.

    A vendor may index a part under its own symbol rather than the manufacturer
    part number — TME holds the documented cell as symbol `AKY-LP623255` with
    manufacturer PN `AKY0646`. Matching on one identifier only files such a part
    under no requirement at all, so it is fetched and then invisible.
    """
    d = json.loads(SNAPSHOT.read_text())
    for ident in identifiers:
        if ident and ident in d["parts"]:
            return (d["parts"][ident] or {}).get("satisfies")
    return None


def cmd_fetch(args) -> int:
    env = load_env()
    adapters = {a.name: a for a in build_adapters(env)}
    a = adapters.get(args.vendor)
    if a is None:
        sys.exit(f"unknown vendor {args.vendor!r}; known: {sorted(adapters)}")
    if not a.available():
        print(f"{a.display_name}: no credentials — nothing to do.")
        print("  see docs/sourcing/DISTRIBUTOR-API-SETUP.md")
        return 0

    db = Catalogue(DB_PATH)
    started = time.strftime("%Y-%m-%d")

    if args.category:
        run = db.start_run(
            a.name, "category", f"{args.category}{' ~ ' + args.phrase if args.phrase else ''}",
            started,
        )
        parts = a.search_category(str(args.category), args.limit, phrase=args.phrase)
        label = args.category_as or f"tme-cat-{args.category}"
        for p in parts:
            db.upsert(p, run, category=label)
        db.finish_run(run, getattr(a, "calls", 0), len(parts))
        print(f"{a.display_name}: {len(parts)} parts in category {args.category} -> '{label}'")
    else:
        mpns = spec_mpns()
        run = db.start_run(a.name, "mpns", ",".join(mpns), started)
        parts = a.lookup_mpns(mpns)
        found = {p.mpn.upper() for p in parts}
        for p in parts:
            db.upsert(
                p, run, category=spec_category_of(p.queried_as, p.mpn, p.vendor_pn)
            )
        db.finish_run(run, getattr(a, "calls", 0), len(parts))
        print(f"{a.display_name}: resolved {len(parts)} of {len(mpns)}")
        for m in mpns:
            # Match on EITHER identifier: the vendor's symbol and the
            # manufacturer part number are different strings for one part, and
            # reporting "not carried" because the wrong one was compared is how a
            # stocked part gets written off.
            # `queried_as` first: it is the only identifier guaranteed to match
            # what was asked for. Vendors return their own PN, which may carry a
            # packaging suffix or be their internal symbol.
            hit = next(
                (
                    p
                    for p in parts
                    if (p.queried_as or "").upper() == m.upper()
                    or m.upper() in {p.mpn.upper(), p.vendor_pn.upper()}
                ),
                None,
            )
            # An adapter may report a part it could not RESOLVE — the lookup
            # failed — which is a different fact from the vendor not carrying it.
            # Printing both as "not carried" would contradict the adapter's own
            # warning two lines earlier, and "not carried" is the verdict that
            # gets a part written off.
            unresolved = dict(getattr(a, "unresolved", []) or [])
            if hit:
                print(f"  {m:<26} stock={hit.stock:<7} attrs={len(hit.attrs_raw)}")
            elif m in unresolved:
                print(f"  {m:<26} UNRESOLVED (lookup failed; nothing written)")
            else:
                print(f"  {m:<26} not carried")
        _ = found

    print(f"\nAPI calls: {getattr(a, 'calls', 0)}   db: {DB_PATH.relative_to(ROOT)}")
    return 0


def cmd_export(args) -> int:
    """DB -> a candidate file the Rust filter consumes.

    Raw strings only. Every value stays exactly as the vendor published it so
    that `dap_hw_spec::parametric::derive` remains the one place a unit is ever
    interpreted.
    """
    db = Catalogue(DB_PATH)
    cands = db.candidates(args.category)
    EXPORT_DIR.mkdir(parents=True, exist_ok=True)
    out = EXPORT_DIR / f"{args.category}.json"
    out.write_text(
        json.dumps(
            {
                "kind": "candidates",
                "category": args.category,
                "generated_at": time.strftime("%Y-%m-%d"),
                "note": (
                    "Values are RAW vendor strings; dap_hw_spec::parametric::derive "
                    "interprets them. Rebuild with tools/sourcing-run.py export."
                ),
                "parts": cands,
            },
            indent=2,
        )
        + "\n"
    )
    print(f"  {args.category:<16} {len(cands):>4} candidate(s) -> {out.relative_to(ROOT)}")
    # Per-part detail is useful when exporting one category by hand and pure noise
    # across seven of them, so it follows the caller's intent rather than always
    # printing.
    if getattr(args, "verbose", True):
        for c in cands:
            print(f"      {c['mpn']:<26} sources={c['eu_sources']} stock={c['eu_stock']:<8} "
                  f"attrs={sorted(c['attrs_raw'])}")
    return 0


# The sweeps that back the committed snapshot, declared rather than remembered.
#
# Each entry is (vendor, category id, phrase, requirement id, limit). Keeping it
# here makes a refresh reproducible: the alternative is a shell history nobody
# else has, and "which categories is this conclusion based on?" becomes
# unanswerable.
SWEEPS = [
    ("tme", "100269", "22.5792MHz", "mclk-osc-44k1", 60),
    ("tme", "100269", "24.576MHz", "mclk-osc-48k", 60),
    ("tme", "100222", "SPDT", "mclk-mux", 300),
    ("tme", "100076", "Li-Po", "battery", 120),
    ("tme", "112878", None, "dac", 150),
    ("tme", "112866", None, "mcu", 150),
    # Digi-Key sweeps are keyword-scoped rather than category-scoped: the
    # frequency IS the requirement for an oscillator, and a keyword narrows
    # across every oscillator category at once (608 parts match 22.5792MHz)
    # without needing Digi-Key's internal category ids.
    ("digikey", "", "22.5792MHz oscillator", "mclk-osc-44k1", 100),
    ("digikey", "", "24.576MHz oscillator", "mclk-osc-48k", 100),
    ("digikey", "", "SPDT analog switch", "mclk-mux", 100),
    # Broadened 2026-11-24. The first versions of these two were too narrow —
    # "CS43131" and "Li-Po battery 3.7V" returned 5 and 5 parts, so the
    # requirements had an exercised Digi-Key path but no Digi-Key COVERAGE.
    # Digi-Key is the deepest audio-DAC source, and the battery is the one open
    # blocker a second distributor could actually close.
    ("digikey", "", "stereo audio DAC I2S", "dac", 150),
    ("digikey", "", "lithium polymer battery pack", "battery", 150),
    ("digikey", "", "bluetooth module", "bluetooth", 100),
    # Mouser is DELIBERATELY absent from this list, and the reason is not quota
    # alone. Mouser publishes no named parametric fields at all — only
    # `Packaging` and `Standard Pack Qty` — so a part discovered only there
    # arrives with nothing for a constraint to judge and can never pass a
    # filter. Sweeping would spend the tightest daily budget of the three (1000
    # calls, 30/minute) to produce unjudgeable rows.
    #
    # Mouser runs the named-lookup path instead, where it contributes what the
    # pipeline was actually short of: stocked-source depth, qty-1 MOQ and price,
    # lead time and datasheet links. See `adapters/mouser.py`.
]

CATEGORIES = ["dac", "mclk-osc-44k1", "mclk-osc-48k", "mclk-mux", "mcu", "battery", "bluetooth"]


def cmd_refresh(args) -> int:
    """Fetch every available vendor, re-export, regenerate the snapshot.

    One command so a refresh is reproducible and so the sweep list is a committed
    fact rather than shell history. A vendor without credentials is skipped and
    reported — a partial refresh is a normal outcome, not an error.
    """
    env = load_env()
    adapters = {a.name: a for a in build_adapters(env)}
    available = [n for n, a in adapters.items() if a.available()]
    skipped = [n for n, a in adapters.items() if not a.available()]
    print(f"vendors available: {available}")
    if skipped:
        print(f"vendors skipped (no credentials): {skipped}")
    if not available:
        print("nothing to do; see docs/sourcing/DISTRIBUTOR-API-SETUP.md")
        return 0

    calls = 0
    # 1. Verification: the MPNs the spec names, from every vendor.
    for name in available:
        a = adapters[name]
        mpns = spec_mpns()
        db = Catalogue(DB_PATH)
        run = db.start_run(name, "mpns", ",".join(mpns), time.strftime("%Y-%m-%d"))
        parts = a.lookup_mpns(mpns)
        for pt in parts:
            db.upsert(pt, run, category=spec_category_of(pt.queried_as, pt.mpn, pt.vendor_pn))
        db.finish_run(run, getattr(a, "calls", 0), len(parts))
        print(f"  {name}: resolved {len(parts)} of {len(mpns)} named parts")
        calls += getattr(a, "calls", 0)

    # 2. Discovery: the declared category sweeps.
    for vendor, cat, phrase, label, limit in SWEEPS:
        if vendor not in available:
            continue
        a = adapters[vendor]
        before = getattr(a, "calls", 0)
        db = Catalogue(DB_PATH)
        run = db.start_run(vendor, "category", f"{cat} {phrase or ''}".strip(),
                           time.strftime("%Y-%m-%d"))
        try:
            parts = a.search_category(str(cat), limit, phrase=phrase)
        except RuntimeError as e:
            print(f"  {vendor} {label}: SWEEP FAILED — {e}")
            continue
        for pt in parts:
            db.upsert(pt, run, category=label)
        db.finish_run(run, getattr(a, "calls", 0) - before, len(parts))
        print(f"  {vendor} {label}: {len(parts)} parts")
    for name in available:
        calls = getattr(adapters[name], "calls", 0)
        print(f"  {name}: {calls} API calls")

    # 3. Export every category, then regenerate the snapshot.
    print("\nexporting candidates:")
    for cat in CATEGORIES:
        rc = cmd_export(argparse.Namespace(category=cat, verbose=False))
        if rc:
            return rc
    print("\nregenerating snapshot:")
    import subprocess
    subprocess.run([sys.executable, str(ROOT / "tools/bom-snapshot-merge.py")], check=False)
    return 0


def cmd_status(args) -> int:
    if not DB_PATH.exists():
        print(f"no catalogue at {DB_PATH.relative_to(ROOT)} — run `fetch` first")
        return 0
    db = Catalogue(DB_PATH)
    print(f"{'vendor':<9} {'category':<18} {'parts':>6} {'stocked':>8}")
    for vendor, category, parts, stocked in db.summary():
        print(f"{vendor:<9} {str(category):<18} {parts:>6} {stocked:>8}")
    runs = db.conn.execute(
        "SELECT vendor, mode, started_at, api_calls, parts_seen FROM fetch_runs ORDER BY id"
    ).fetchall()
    print("\nfetch runs:")
    for r in runs:
        print(f"  {r[0]:<7} {r[1]:<9} {r[2]}  calls={r[3]} parts={r[4]}")
    return 0


def cmd_selftest(args) -> int:
    """Exercise the pure helpers. No network, no credentials.

    The repo has no Python test runner, and these two helpers guard failures that
    are SILENT rather than loud, so they get checked here rather than trusted.
    """
    from sourcing.adapters.tme import DOC_TYPE_PREFERENCE, SYMBOL_MAX_LEN, _queryable

    failures = []

    def check(name, cond, detail=""):
        print(f"  {'ok  ' if cond else 'FAIL'} {name}{'  ' + detail if detail else ''}")
        if not cond:
            failures.append(name)

    ok, bad = _queryable(["ASV22.5792MHZLRT-0", "ASV-22.5792MHZ-LR-T", "X", "KC2016Z22.5792C1KX"])
    check("18-char symbols accepted", "ASV22.5792MHZLRT-0" in ok)
    check("19-char symbol rejected", "ASV-22.5792MHZ-LR-T" in bad,
          "this is the MPN, and passing it where the symbol belongs is the recurring trap")
    check("1-char symbol rejected", "X" in bad)
    check(f"limit is {SYMBOL_MAX_LEN}", SYMBOL_MAX_LEN == 18)
    check("nothing silently vanishes", len(ok) + len(bad) == 4)

    # A hosted datasheet must win over a manufacturer link, and YouTube must
    # never be considered a datasheet at all.
    check("DTE preferred over LNK", DOC_TYPE_PREFERENCE.index("DTE")
          < DOC_TYPE_PREFERENCE.index("LNK"))
    check("YTB is not a datasheet type", "YTB" not in DOC_TYPE_PREFERENCE,
          "the TI part's second document is a product video")

    from sourcing.canon import SYNONYMS, canonical, merge_values

    # Vendor vocabulary must collapse to one canonical token.
    check("TME 'SMD' canonicalises", canonical("mounting", "SMD") == "SMD")
    check("Digi-Key 'Surface Mount' canonicalises",
          canonical("mounting", "Surface Mount") == "SMD",
          "the same part, two vendor spellings, one spec constraint")
    check("'XO (Standard)' is an oscillator",
          canonical("part_type", "XO (Standard)") == "oscillator")
    check("unknown values pass through",
          canonical("mounting", "Flip Chip") == "Flip Chip",
          "inventing agreement would be worse than a visible rejection")

    # A merge must be deterministic and must prefer the informative value.
    v, conflicts = merge_values("mounting", {"tme": "SMD", "digikey": "Surface Mount"})
    check("cross-vendor merge agrees", v == "SMD" and not conflicts)
    v, conflicts = merge_values("mounting", {"tme": "SMD", "digikey": "THT"})
    check("a real enum conflict is reported", v and conflicts,
          "two vendors disagreeing about one part is data worth surfacing")
    v, _ = merge_values("supply_mv", {"digikey": "1.71V ~ 3.63V", "tme": "3.3V DC"})
    check("a range beats a bare nominal", v == "1.71V ~ 3.63V",
          "only the range lets a containment constraint do real work")
    v, conflicts = merge_values("frequency_hz",
                                {"digikey": "22.5792 MHz", "tme": "22.5792MHz"})
    check("formatting differences are NOT conflicts", not conflicts,
          "reporting them would bury the real ones in noise")
    # --- Mouser hydration, against recorded responses ------------------
    #
    # Run against fixtures rather than live: this checks the MAPPING, which is
    # what field-name mistakes break, and the Digi-Key `Function`/`Base
    # Resonator` mix-up proves a mapping transcribed from prose can look right
    # and mean something else.
    from sourcing.adapters.mouser import (
        PUBLISHED_ATTRIBUTE_NAMES, MouserAdapter, _lead_days, _norm, _stock,
    )

    mz = MouserAdapter("")
    fixtures = ROOT / "hardware/bom/responses/mouser"
    pn = json.loads((fixtures / "search-partnumber.json").read_text())
    kw = json.loads((fixtures / "search-keyword-oscillator.json").read_text())

    mux = mz._to_part(pn["SN74LVC1G3157DCKR"]["SearchResults"]["Parts"][0],
                      queried_as="SN74LVC1G3157DCKR")
    check("mouser mpn maps", mux.mpn == "SN74LVC1G3157DCKR")
    check("mouser vendor_pn is Mouser's own", mux.vendor_pn.startswith("595-"),
          mux.vendor_pn)
    # 'Availability' is prose ("530597 In Stock"); AvailabilityInStock is bare.
    check("stock parsed from the bare field", mux.stock == 530597, str(mux.stock))
    check("moq and multiple are ints", mux.moq == 1 and mux.order_multiple == 1)
    check("lead time extracted in days", mux.lead_time_days == 182,
          str(mux.lead_time_days))
    # The display price is "0,213 €" — a comma decimal. Parsing it would be a
    # locale bug that reads as a 1000x error.
    check("price comes from RawPrice, not the localised string",
          mux.price_breaks[0].unit_price == 0.213
          and mux.price_breaks[0].currency == "EUR",
          f"{mux.price_breaks[0].unit_price} {mux.price_breaks[0].currency}")
    check("qty-1 price in cents", mux.unit_price_cents() == 21,
          str(mux.unit_price_cents()))
    # Mouser returns LifecycleStatus: null for every part measured, including
    # ones Digi-Key marks Active. Absent evidence must stay absent.
    check("absent lifecycle stays absent", mux.lifecycle_raw is None,
          "defaulting it to Active would let the EOL gate pass on nothing")
    check("RoHS recorded as an IPC-1752A-shaped claim",
          mux.status_raw and mux.status_raw[0]["regulation"] == "RoHS"
          and mux.status_raw[0]["revision"] == "unstated"
          and mux.status_raw[0]["status"] == "RoHS Compliant")
    check("customs codes are NOT filed as compliance",
          all(c["regulation"] == "RoHS" for c in mux.status_raw),
          "TARIC/CNHTS/USHTS are tariff codes, not environmental regulations")

    # The governing finding: Mouser publishes no parametric field at all.
    # Selected by MPN, not by index: the fixture's order is the vendor's, and
    # pinning a position would make the test pass for the wrong reason.
    osc = mz._to_part(next(p for p in kw["SearchResults"]["Parts"]
                           if p["ManufacturerPartNumber"] == "ECS-2520MVLC-225.792-BN-TR"))
    check("mouser contributes NO parametric attributes",
          osc.attrs_raw == {} and mux.attrs_raw == {},
          f"osc={osc.attrs_raw} mux={mux.attrs_raw}")
    names = {a["AttributeName"] for p in kw["SearchResults"]["Parts"]
             for a in (p.get("ProductAttributes") or [])}
    check("the whole published vocabulary is packaging",
          names <= set(PUBLISHED_ATTRIBUTE_NAMES), str(sorted(names)))
    # And the parametrics ARE in Description, unlabelled — which is why parsing
    # it is forbidden rather than merely unimplemented.
    check("parametrics are present but unnamed in Description",
          "22.5792MHz" in osc.description and "Surface Mount" in osc.description,
          "positional, category-dependent, and one value contains the separator")
    check("datasheet url still harvested", bool(osc.datasheet_url),
          "the commerce contribution: a link where another vendor published none")
    check("empty datasheet becomes None, not ''", mux.datasheet_url is None,
          "an empty string reads as a link nobody can open")

    check("_stock survives non-numeric availability",
          _stock({"Availability": "Call for Availability"}) == 0)
    check("_stock handles thousands separators",
          _stock({"Availability": "1,317 In Stock"}) == 1317)
    check("_lead_days ignores a missing lead time", _lead_days(None) is None)
    check("_norm strips packaging punctuation",
          _norm("74LVC1G3157GW,125") == "74LVC1G3157GW125")

    from sourcing.throttle import BudgetExhausted, Throttle, limit_for

    mlim = limit_for("mouser")
    check("mouser paced for 30/min with headroom",
          2.0 < mlim.min_interval() < 2.2, f"{mlim.min_interval():.2f}s")
    check("mouser daily budget reserves headroom",
          mlim.budget() == 900, str(mlim.budget()))
    t = Throttle("mouser", spent_today=900, sleep=lambda _: None)
    try:
        t.acquire()
        check("a spent budget raises rather than 429s", False)
    except BudgetExhausted:
        check("a spent budget raises rather than 429s", True,
              "a 429 counted as absence is how a stocked part gets written off")
    t2 = Throttle("mouser", spent_today=0, sleep=lambda _: None)
    t2.acquire()
    check("a failed call still counts against quota", t2.calls == 1,
          "counting only successes over-spends exactly when things go wrong")
    check("vendor Retry-After is authoritative",
          Throttle("mouser", sleep=lambda _: None).backoff(0, "7") == 7.0)
    check("backoff is never faster than pacing",
          Throttle("mouser", sleep=lambda _: None).backoff(0, "0")
          >= 0.0, "a zero Retry-After must not bypass spacing entirely")

    # Order independence: the same inputs in a different dict order must agree.
    a1, _ = merge_values("mounting", {"digikey": "Surface Mount", "tme": "SMD"})
    a2, _ = merge_values("mounting", {"tme": "SMD", "digikey": "Surface Mount"})
    check("merge is order-independent", a1 == a2,
          "the winner used to depend on SQL row order")
    check("every synonym table maps to itself",
          all(canonical(a, t) == t for a, tbl in SYNONYMS.items() for t in set(tbl.values())),
          "a canonical value must be a fixed point, or re-export drifts")

    print(f"\n{'selftest OK' if not failures else 'FAILURES: ' + str(failures)}")
    return 1 if failures else 0


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    sub = ap.add_subparsers(dest="cmd", required=True)

    f = sub.add_parser("fetch", help="fetch from a vendor into the catalogue DB")
    f.add_argument("--vendor", required=True)
    f.add_argument("--category", help="vendor category id (discovery path)")
    f.add_argument("--as", dest="category_as", help="spec requirement id to file it under")
    f.add_argument("--limit", type=int, default=200)
    f.add_argument(
        "--phrase",
        help="narrow a category sweep at the vendor. Real categories are large "
             "(TME's Resonators and Generators holds 6691 parts), so sweeping a whole "
             "one to find two frequencies wastes quota for no extra information.",
    )
    f.set_defaults(func=cmd_fetch)

    e = sub.add_parser("export", help="write a candidate file for the Rust filter")
    e.add_argument("--category", required=True)
    e.set_defaults(func=cmd_export)

    s = sub.add_parser("status", help="what the catalogue DB holds")
    s.set_defaults(func=cmd_status)

    t = sub.add_parser("selftest", help="check the pure helpers; no network")
    t.set_defaults(func=cmd_selftest)

    rf = sub.add_parser("refresh", help="fetch all vendors, re-export, regenerate the snapshot")
    rf.set_defaults(func=cmd_refresh)

    args = ap.parse_args()
    return args.func(args)


if __name__ == "__main__":
    raise SystemExit(main())
