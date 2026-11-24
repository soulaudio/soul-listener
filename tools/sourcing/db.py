"""The catalogue store: one SQLite file across every vendor.

# Why a database and not more JSON

Three things JSON cannot do well here, and all three are needed:

1. **Join across vendors on MPN.** "EU source depth" — the first key in
   `DEFAULT_RANKING` — means *counting distinct distributors that stock the same
   part*. That count is a join, and without one store there is nothing to join.
2. **Ad-hoc research.** "every 22.5792 MHz oscillator whose range covers 3.3 V,
   by stock" is one query and no code.
3. **Incremental refresh.** Re-fetch one category without rewriting everything.

# Why it is NOT committed

Because CI must never depend on it. The committed artefact stays
`hardware/bom/snapshot.json`: small, diffable, and the thing the gates read — so
when a part we depend on drops to zero stock or goes NRND, it arrives as a
reviewable diff rather than silently changing under us. This file is the wide
discovery surface behind that; it is rebuildable, and cheaply (a 400-part
category costs about 20 API calls, against a 1000/day budget).

That is the repo's stated idiom for consumers that cannot read Rust: generate,
commit, and gate that the committed artefact matches a fresh regeneration.

# Schema shape

Parametric attributes are an **open set** that differs per category, so they are
stored entity-attribute-value rather than as columns. Values are the vendor's
**raw strings** — this layer never interprets them.

`fetch_runs` exists because provenance is the one thing this repo is consistent
about: every row can be traced to a run, a vendor and a timestamp.
"""

from __future__ import annotations

import pathlib
import sqlite3

from .canon import SYNONYMS, _is_range, canonical, merge_values

SCHEMA = """
CREATE TABLE IF NOT EXISTS fetch_runs (
    id           INTEGER PRIMARY KEY,
    vendor       TEXT NOT NULL,
    mode         TEXT NOT NULL,            -- 'mpns' | 'category'
    target       TEXT NOT NULL,            -- the mpn list or category id
    started_at   TEXT NOT NULL,
    api_calls    INTEGER NOT NULL DEFAULT 0,
    parts_seen   INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS parts (
    id            INTEGER PRIMARY KEY,
    vendor        TEXT NOT NULL,
    vendor_pn     TEXT NOT NULL,
    mpn           TEXT NOT NULL,          -- the vendor's own spelling, verbatim
    -- The identifier the SPEC asked for, when this row came from a named lookup.
    --
    -- Vendors decorate MPNs differently for the same die: Digi-Key returns the
    -- mux as '74LVC1G3157GW,125' where Mouser returns '74LVC1G3157GW'. Grouping
    -- by `mpn` made those two PHANTOM PARTS — the datasheet sat on one and the
    -- gate read the other, and source depth never joined, so a part stocked by
    -- two distributors counted as single-sourced.
    --
    -- Safe as a grouping key because every vendor is queried with the SAME
    -- identifier list (`spec_mpns()`, the snapshot's own keys), so this column
    -- holds the same string across vendors by construction. NULL for swept
    -- parts, which were never asked for by name.
    queried_as    TEXT,
    manufacturer  TEXT NOT NULL,
    description   TEXT,
    category      TEXT,                    -- the spec requirement id, if known
    stock         INTEGER NOT NULL DEFAULT 0,
    moq           INTEGER,
    order_multiple INTEGER,
    lead_time_days INTEGER,
    lifecycle_raw TEXT,
    status_raw    TEXT,                    -- JSON array, vendor's own wording
    datasheet_url TEXT,
    product_url   TEXT,   -- the distributor's own product page
    run_id        INTEGER NOT NULL REFERENCES fetch_runs(id),
    UNIQUE (vendor, vendor_pn)
);

-- Open attribute set, raw values. Never interpreted here.
CREATE TABLE IF NOT EXISTS attributes (
    part_id   INTEGER NOT NULL REFERENCES parts(id) ON DELETE CASCADE,
    name      TEXT NOT NULL,               -- SPEC attribute name, already mapped
    value_raw TEXT NOT NULL,               -- the vendor's untouched string
    PRIMARY KEY (part_id, name)
);

CREATE TABLE IF NOT EXISTS price_breaks (
    part_id    INTEGER NOT NULL REFERENCES parts(id) ON DELETE CASCADE,
    qty        INTEGER NOT NULL,
    unit_price REAL NOT NULL,
    currency   TEXT NOT NULL,
    PRIMARY KEY (part_id, qty)
);

CREATE INDEX IF NOT EXISTS parts_by_mpn      ON parts(mpn);
-- The grouping key `candidates()` actually uses, as an EXPRESSION index.
--
-- `parts_by_mpn` stopped being usable the moment grouping moved to
-- COALESCE(queried_as, mpn): SQLite cannot use a column index for an expression,
-- so every per-part subquery fell back to scanning `parts_by_category`. With
-- 1500 parts in one category that is ~2.25M row visits per subquery kind, five
-- kinds per part, and the export went from seconds to minutes as the catalogue
-- grew from 1065 parts to 5982.
CREATE INDEX IF NOT EXISTS parts_by_spec_key ON parts(COALESCE(queried_as, mpn), category);
CREATE INDEX IF NOT EXISTS parts_by_category ON parts(category);
CREATE INDEX IF NOT EXISTS attrs_by_name     ON attributes(name);
"""


class Catalogue:
    def __init__(self, path: pathlib.Path):
        self.path = path
        path.parent.mkdir(parents=True, exist_ok=True)
        self.conn = sqlite3.connect(path)
        self.conn.execute("PRAGMA foreign_keys = ON")
        self.conn.executescript(SCHEMA)
        self._migrate()

    def _migrate(self) -> None:
        """Add columns a `CREATE TABLE IF NOT EXISTS` cannot add to a live table.

        The catalogue is rebuildable and gitignored, so dropping it would also
        work — but a rebuild costs distributor quota, and Mouser's budget is 1000
        calls a day. Migrating in place keeps a schema change from spending it.
        """
        have = {r[1] for r in self.conn.execute("PRAGMA table_info(parts)")}
        if "product_url" not in have:
            self.conn.execute("ALTER TABLE parts ADD COLUMN product_url TEXT")
            self.conn.commit()
        if "queried_as" not in have:
            self.conn.execute("ALTER TABLE parts ADD COLUMN queried_as TEXT")
            self.conn.commit()
            # The expression index in SCHEMA references `queried_as`, so on a
            # pre-existing catalogue it could not be created before the column
            # existed. Create it now rather than leaving the export to scan.
            self.conn.execute(
                "CREATE INDEX IF NOT EXISTS parts_by_spec_key "
                "ON parts(COALESCE(queried_as, mpn), category)"
            )
            self.conn.commit()

    def start_run(self, vendor: str, mode: str, target: str, started_at: str) -> int:
        cur = self.conn.execute(
            "INSERT INTO fetch_runs (vendor, mode, target, started_at) VALUES (?,?,?,?)",
            (vendor, mode, target, started_at),
        )
        self.conn.commit()
        return int(cur.lastrowid)

    def finish_run(self, run_id: int, api_calls: int, parts_seen: int) -> None:
        self.conn.execute(
            "UPDATE fetch_runs SET api_calls=?, parts_seen=? WHERE id=?",
            (api_calls, parts_seen, run_id),
        )
        self.conn.commit()

    def upsert(self, part, run_id: int, category: str | None = None) -> int:
        """Insert or replace one vendor part and its attributes/prices.

        Replace rather than merge: a re-fetch is the newer truth, and silently
        keeping a stale attribute because the vendor stopped publishing it would
        make the catalogue claim something nobody currently says.
        """
        import json as _json

        cur = self.conn.execute(
            """
            INSERT INTO parts (vendor, vendor_pn, mpn, queried_as, manufacturer,
                               description, category,
                               stock, moq, order_multiple, lead_time_days, lifecycle_raw,
                               status_raw, datasheet_url, product_url, run_id)
            VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
            ON CONFLICT (vendor, vendor_pn) DO UPDATE SET
                mpn=excluded.mpn,
                -- COALESCE, not overwrite: a later category sweep re-touches a
                -- row that a named lookup already identified, and it arrives
                -- with queried_as=NULL. Overwriting would silently un-identify
                -- the part and split its offers again.
                queried_as=COALESCE(excluded.queried_as, parts.queried_as),
                manufacturer=excluded.manufacturer,
                description=excluded.description,
                category=COALESCE(excluded.category, parts.category),
                stock=excluded.stock, moq=excluded.moq,
                order_multiple=excluded.order_multiple,
                lead_time_days=excluded.lead_time_days,
                lifecycle_raw=excluded.lifecycle_raw, status_raw=excluded.status_raw,
                datasheet_url=excluded.datasheet_url,
                product_url=excluded.product_url, run_id=excluded.run_id
            """,
            (
                part.vendor, part.vendor_pn, part.mpn, part.queried_as,
                part.manufacturer, part.description,
                category, part.stock, part.moq, part.order_multiple, part.lead_time_days,
                part.lifecycle_raw, _json.dumps(part.status_raw), part.datasheet_url,
                part.product_url, run_id,
            ),
        )
        # Always re-read the id. `cur.lastrowid` is only meaningful for a real
        # INSERT: when ON CONFLICT DO UPDATE fires it can hold a STALE rowid from
        # an earlier statement, and writing attributes against that id fails the
        # foreign key — or worse, would attach them to another part.
        #
        # This only shows up on a RE-fetch, which is exactly the path the
        # catalogue's "uncommitted but rebuildable" premise depends on, so
        # trusting lastrowid broke the one property the design promises.
        del cur
        pid = self.conn.execute(
            "SELECT id FROM parts WHERE vendor=? AND vendor_pn=?",
            (part.vendor, part.vendor_pn),
        ).fetchone()[0]

        self.conn.execute("DELETE FROM attributes WHERE part_id=?", (pid,))
        self.conn.executemany(
            "INSERT INTO attributes (part_id, name, value_raw) VALUES (?,?,?)",
            [(pid, k, v) for k, v in part.attrs_raw.items()],
        )
        self.conn.execute("DELETE FROM price_breaks WHERE part_id=?", (pid,))
        self.conn.executemany(
            "INSERT INTO price_breaks (part_id, qty, unit_price, currency) VALUES (?,?,?,?)",
            [(pid, b.qty, b.unit_price, b.currency) for b in part.price_breaks],
        )
        self.conn.commit()
        return int(pid)

    def candidates(self, category: str) -> list[dict]:
        """Every part in a category, across vendors, aggregated for the filter.

        Source depth and total stock are computed here because they are facts
        about the *set* of offers for an MPN, which is what SQL is for. Whether a
        part QUALIFIES is not decided here — that is the Rust engine's job, and
        splitting it would put the same rule in two places.
        """
        rows = self.conn.execute(
            """
            SELECT COALESCE(queried_as, mpn)             AS key,
                   MIN(manufacturer)                     AS manufacturer,
                   COUNT(DISTINCT CASE WHEN stock > 0 THEN vendor END) AS eu_sources,
                   SUM(stock)                            AS eu_stock,
                   GROUP_CONCAT(DISTINCT vendor)         AS vendors
            FROM parts
            WHERE category = ?
            GROUP BY key
            ORDER BY key
            """,
            (category,),
        ).fetchall()

        out = []
        conflicts: list[str] = []
        for mpn, manufacturer, eu_sources, eu_stock, vendors in rows:
            # Per-vendor values, then a deterministic canonical merge.
            #
            # A plain dict() over these rows let SQL row order decide the winner,
            # and the winner mattered: TME says mounting='SMD' where Digi-Key says
            # 'Surface Mount' for the SAME part, and the spec constrains "SMD". So
            # whether a part qualified depended on which row came back last.
            # Attributes this part's vendors genuinely disagree about, AFTER
            # canonicalisation. Exported per part rather than only printed:
            # `merge_values` must still pick one value, and it picks the
            # alphabetically-first vendor — so for a discriminating field the
            # verdict depends on vendor NAME order. `ASV-24.576MHZ-EJ-T` passes
            # `part_type` only because TME calls it a 'crystal' (wrong — it is
            # Abracon's XO family) and 'digikey' happens to sort before 'tme'.
            # A gate cannot check that fragility from a log line.
            conflicted: list[str] = []
            # Collected per (attribute, vendor) as a LIST, not assigned.
            #
            # One vendor can hold several rows for one MPN — Digi-Key files the
            # cut-tape and reel packagings separately — and they do not always
            # publish the same thing: for `SN74LVC1G3157DCKR` one row says
            # `switch_configuration = 'SPDT'` and another `'SPDT - NO/NC'`.
            # Assigning into a dict took whichever row SQL returned last, which
            # is the same row-order bug `canon.py` was written to kill, just
            # inside a single vendor instead of across two. It rejected the
            # PREFERRED mux against its own requirement.
            per_vendor_vals: dict[str, dict[str, list[str]]] = {}
            for name, vendor, raw in self.conn.execute(
                """
                SELECT a.name, p.vendor, a.value_raw
                FROM attributes a JOIN parts p ON p.id = a.part_id
                WHERE COALESCE(p.queried_as, p.mpn) = ? AND p.category = ?
                """,
                (mpn, category),
            ):
                per_vendor_vals.setdefault(name, {}).setdefault(vendor, []).append(raw)

            per_attr: dict[str, dict[str, str]] = {}
            for name, by_vendor_list in per_vendor_vals.items():
                for vendor, vals in by_vendor_list.items():
                    # Canonicalise first, then take the distinct values. Two rows
                    # spelling one fact differently must collapse to agreement
                    # rather than be reported as a vendor contradicting itself.
                    distinct = sorted({canonical(name, v) for v in vals if v and v.strip()})
                    if not distinct:
                        continue
                    if len(distinct) > 1 and name.strip().lower() in SYNONYMS:
                        # Reported only for ENUMERATED attributes, exactly as
                        # `merge_values` does across vendors — and for the same
                        # reason. The first version of this reported every
                        # formatting difference, so one vendor publishing
                        # '-40°C ~ 125°C (TA)' on one packaging row and
                        # '-40°C ~ 125°C' on another counted as self-
                        # contradiction, and conflicts went from 5 to 27 with
                        # the real ones buried in the noise. For a dimensioned
                        # field `parametric::derive` reconciles the spellings and
                        # the constraints compare parsed numbers.
                        conflicts.append(
                            f"{mpn}.{name}: {vendor} publishes {len(distinct)} different "
                            f"values across its own rows: {distinct}"
                        )
                        conflicted.append(name)
                    # Prefer a range over a bare nominal, then the first
                    # alphabetically — the same rule `merge_values` applies
                    # across vendors, so the two levels of merge cannot disagree
                    # about which of two values is more informative.
                    ranged = [v for v in distinct if _is_range(v)]
                    per_attr.setdefault(name, {})[vendor] = (
                        ranged[0] if ranged else distinct[0]
                    )

            attrs: dict[str, str] = {}
            for name, by_vendor in per_attr.items():
                value, conflict = merge_values(name, by_vendor)
                if value:
                    attrs[name] = value
                if conflict:
                    # A real disagreement about one part: reported, never
                    # silently resolved.
                    conflicts.append(f"{mpn}.{name}: " + ", ".join(conflict))
                    conflicted.append(name)
            # The price at the LOWEST break the vendor publishes, not the qty-1
            # price. `qty <= 1` was returning NULL for every reel-only part — so
            # the entire oscillator category exported as unpriced and ranked last
            # on price while being the only thing available.
            price = self.conn.execute(
                """
                SELECT b.unit_price
                FROM price_breaks b JOIN parts p ON p.id = b.part_id
                WHERE COALESCE(p.queried_as, p.mpn) = ? AND p.category = ?
                ORDER BY b.qty ASC
                LIMIT 1
                """,
                (mpn, category),
            ).fetchone()
            price = price[0] if price else None
            lead = self.conn.execute(
                "SELECT MAX(lead_time_days) FROM parts\n                 WHERE COALESCE(queried_as, mpn)=? AND category=?",
                (mpn, category),
            ).fetchone()[0]
            # MOQ and order multiple are exported as ATTRIBUTES so the
            # constraints can judge them. They are the cheapest vendor fields to
            # ignore and among the most consequential: a part with 17000 in stock
            # and a 1000-piece reel MOQ is not a cheap part, it is a 700 EUR
            # minimum order for something a prototype needs one of.
            #
            # MIN across vendors: if any distributor will sell ones, that is the
            # buyable truth for the MPN.
            moq, mult = self.conn.execute(
                """
                SELECT MIN(COALESCE(moq, 1)), MIN(COALESCE(order_multiple, 1))
                FROM parts WHERE COALESCE(queried_as, mpn)=? AND category=?
                """,
                (mpn, category),
            ).fetchone()
            # Per-vendor product links, so a proposal can be checked at the
            # distributor it would actually be ordered from. Keyed by vendor
            # rather than reduced to one, because the price and stock a reader
            # wants to verify differ per vendor — which is the whole reason the
            # pipeline fetches more than one.
            links = {
                v: u
                for v, u in self.conn.execute(
                    """
                    SELECT vendor, MIN(product_url) FROM parts
                    WHERE COALESCE(queried_as, mpn)=? AND category=?
                      AND product_url IS NOT NULL AND product_url <> ''
                    GROUP BY vendor
                    """,
                    (mpn, category),
                )
            }
            # Any vendor's datasheet link will do; MIN is just determinism.
            ds = self.conn.execute(
                """
                SELECT MIN(datasheet_url) FROM parts
                WHERE COALESCE(queried_as, mpn)=? AND category=?
                  AND datasheet_url IS NOT NULL
                """,
                (mpn, category),
            ).fetchone()[0]
            out.append(
                {
                    "mpn": mpn,
                    "manufacturer": manufacturer or "",
                    "eu_sources": eu_sources or 0,
                    "eu_stock": eu_stock or 0,
                    "unit_price_cents": None if price is None else round(price * 100),
                    "moq": moq,
                    "lead_time_days": lead,
                    "datasheet_url": ds,
                    "vendors": (vendors or "").split(","),
                    "product_urls": links,
                    "conflicts": sorted(conflicted),
                    "attrs_raw": {
                        **attrs,
                        **({"moq": str(moq)} if moq is not None else {}),
                        **({"order_multiple": str(mult)} if mult is not None else {}),
                    },
                }
            )
        if conflicts:
            print(f"  WARNING: {len(conflicts)} cross-vendor value conflict(s) — two vendors "
                  f"describe the same part differently AFTER canonicalisation, so one of them "
                  f"is wrong:")
            for c in conflicts[:10]:
                print(f"      {c}")
        return out

    def calls_today(self, today: str) -> dict[str, int]:
        """API calls each vendor has already spent today, by vendor.

        Mouser's cap is 1000 calls per CALENDAR DAY, which no in-process counter
        can track: a refresh gets run several times a day and each process would
        start from zero, so the cap would be discovered by being refused
        mid-sweep — and a refused call looks like a vendor that does not carry
        the part.

        `started_at` is stored as a date string by the runner, so a prefix match
        is the right comparison and also tolerates a fuller timestamp if one is
        ever written.
        """
        rows = self.conn.execute(
            """
            SELECT vendor, COALESCE(SUM(api_calls), 0)
            FROM fetch_runs
            WHERE started_at LIKE ? || '%'
            GROUP BY vendor
            """,
            (today,),
        ).fetchall()
        return {vendor: int(n or 0) for vendor, n in rows}

    def summary(self) -> list[tuple]:
        return self.conn.execute(
            """
            SELECT vendor, category, COUNT(*) AS parts,
                   SUM(CASE WHEN stock > 0 THEN 1 ELSE 0 END) AS stocked
            FROM parts GROUP BY vendor, category ORDER BY vendor, category
            """
        ).fetchall()
