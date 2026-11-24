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

from .canon import merge_values

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
    mpn           TEXT NOT NULL,
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
            INSERT INTO parts (vendor, vendor_pn, mpn, manufacturer, description, category,
                               stock, moq, order_multiple, lead_time_days, lifecycle_raw,
                               status_raw, datasheet_url, run_id)
            VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)
            ON CONFLICT (vendor, vendor_pn) DO UPDATE SET
                mpn=excluded.mpn, manufacturer=excluded.manufacturer,
                description=excluded.description,
                category=COALESCE(excluded.category, parts.category),
                stock=excluded.stock, moq=excluded.moq,
                order_multiple=excluded.order_multiple,
                lead_time_days=excluded.lead_time_days,
                lifecycle_raw=excluded.lifecycle_raw, status_raw=excluded.status_raw,
                datasheet_url=excluded.datasheet_url, run_id=excluded.run_id
            """,
            (
                part.vendor, part.vendor_pn, part.mpn, part.manufacturer, part.description,
                category, part.stock, part.moq, part.order_multiple, part.lead_time_days,
                part.lifecycle_raw, _json.dumps(part.status_raw), part.datasheet_url, run_id,
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
            SELECT mpn,
                   MIN(manufacturer)                     AS manufacturer,
                   COUNT(DISTINCT CASE WHEN stock > 0 THEN vendor END) AS eu_sources,
                   SUM(stock)                            AS eu_stock,
                   GROUP_CONCAT(DISTINCT vendor)         AS vendors
            FROM parts
            WHERE category = ?
            GROUP BY mpn
            ORDER BY mpn
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
            per_attr: dict[str, dict[str, str]] = {}
            for name, vendor, raw in self.conn.execute(
                """
                SELECT a.name, p.vendor, a.value_raw
                FROM attributes a JOIN parts p ON p.id = a.part_id
                WHERE p.mpn = ? AND p.category = ?
                """,
                (mpn, category),
            ):
                per_attr.setdefault(name, {})[vendor] = raw

            attrs: dict[str, str] = {}
            for name, by_vendor in per_attr.items():
                value, conflict = merge_values(name, by_vendor)
                if value:
                    attrs[name] = value
                if conflict:
                    # A real disagreement about one part: reported, never
                    # silently resolved.
                    conflicts.append(f"{mpn}.{name}: " + ", ".join(conflict))
            # The price at the LOWEST break the vendor publishes, not the qty-1
            # price. `qty <= 1` was returning NULL for every reel-only part — so
            # the entire oscillator category exported as unpriced and ranked last
            # on price while being the only thing available.
            price = self.conn.execute(
                """
                SELECT b.unit_price
                FROM price_breaks b JOIN parts p ON p.id = b.part_id
                WHERE p.mpn = ? AND p.category = ?
                ORDER BY b.qty ASC
                LIMIT 1
                """,
                (mpn, category),
            ).fetchone()
            price = price[0] if price else None
            lead = self.conn.execute(
                "SELECT MAX(lead_time_days) FROM parts WHERE mpn=? AND category=?",
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
                FROM parts WHERE mpn=? AND category=?
                """,
                (mpn, category),
            ).fetchone()
            # Any vendor's datasheet link will do; MIN is just determinism.
            ds = self.conn.execute(
                """
                SELECT MIN(datasheet_url) FROM parts
                WHERE mpn=? AND category=? AND datasheet_url IS NOT NULL
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

    def summary(self) -> list[tuple]:
        return self.conn.execute(
            """
            SELECT vendor, category, COUNT(*) AS parts,
                   SUM(CASE WHEN stock > 0 THEN 1 ELSE 0 END) AS stocked
            FROM parts GROUP BY vendor, category ORDER BY vendor, category
            """
        ).fetchall()
