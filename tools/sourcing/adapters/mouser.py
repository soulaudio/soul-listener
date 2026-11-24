"""Mouser Electronics — Search API v2.

# Transport

Simplest of the three: a single API key passed as a `?apiKey=` query parameter,
no OAuth, no token refresh. Two POST endpoints matter:

* `search/partnumber` — the verification path. `partSearchOptions: "Exact"`
  returns the one part or nothing.
* `search/keyword` — the discovery path, and the fallback when an exact lookup
  returns nothing.

Rate limits are the tightest of any vendor here — **30 requests/minute, 1000/day**
— so every call goes through `sourcing.throttle`, which paces by minimum interval
and tracks the daily budget across process invocations.

# Mouser is a COMMERCE source, not a parametric source

This is the governing fact about this adapter and the reason `ATTR_MAP` is empty.

Mouser's Search API returns, in `ProductAttributes`, only **`Packaging`** and
**`Standard Pack Qty`**. Not one parametric field — measured across 21 in-stock
oscillators and three named parts, on both endpoints. The parametric data is
present, but only inside `Description`, as an unlabelled positional blob::

    "Standard Clock Oscillators MultiVolt ECS-2520MVLC,Active,XO (Standard),
     22.5792MHz,Enable/Disable,CMOS,1.8V, 2.5V, 3.3V, 50ppm,-40 C   85 C,
     Surface Mount,4-SMD,"

Frequency, supply, stability, temperature range, mounting and package are all in
there. None of them has a name, the field order varies by product category, and
one value ("1.8V, 2.5V, 3.3V") contains the separator. Splitting that on commas
and assigning meaning by index is not name mapping — it is value interpretation
by position, which the adapter seam exists to forbid. It would also fail in the
invisible direction: one shifted field maps `50ppm` onto a supply voltage, and a
constraint that then passes is worse than one that never ran.

So this adapter publishes no attributes, deliberately. What it contributes
instead is exactly what the pipeline was short of:

* **Source depth.** `eu_source_depth` is the first key in `DEFAULT_RANKING` and
  the reason `preferred_parts_have_more_than_one_eu_source` exists. Ten of
  eighteen spec candidates were single-sourced.
* **Price and MOQ at qty 1.** `candidates()` takes `MIN` across vendors, so a
  Mouser offer of one piece improves a record whose only other offer is a reel.
* **Datasheet URLs**, where another vendor published none.
* **Lead time and customs/RoHS claims.**

None of that requires a single parametric field, and the export needs no special
case: `candidates()` merges per attribute, so a vendor contributing none simply
leaves the others' values standing.

`mouser_contributes_no_parametrics` in the arch tests pins this, because the
tempting "fix" — parse `Description` — is a silent-wrong-answer change and
someone will propose it.

# Only an empty SUCCESSFUL response means "not carried"

Mouser returned `503 Service Unavailable` on a request shape that had succeeded
two minutes earlier. Every vendor in this pipeline has produced a transient
failure that, counted as absence, writes off a part that is in stock. So 5xx and
429 retry with backoff, and a verdict of "not carried" is only ever reached from
a 200 with zero results — confirmed, for `NSC5083D-22.5792M`, by a keyword search
that shows Mouser does stock NDK parts, just not that one.
"""

from __future__ import annotations

import json
import re
import time
import urllib.error
import urllib.parse
import urllib.request

from ..throttle import Throttle
from .base import Adapter, PriceBreak, VendorPart

_BASE = "https://api.mouser.com/api/v2"
_TODAY = time.strftime("%Y-%m-%d")

#: Attribute names Mouser does publish, and which the spec has no use for.
#: Listed so the empty `ATTR_MAP` reads as a measured finding rather than an
#: unfinished one: these two are the whole vocabulary.
PUBLISHED_ATTRIBUTE_NAMES = ("Packaging", "Standard Pack Qty")


def _stock(p: dict) -> int:
    """Stock as an integer.

    Mouser publishes it twice: `AvailabilityInStock` as a bare count string, and
    `Availability` as prose ("530597 In Stock"). Prefer the bare field and fall
    back to extracting the leading number, because the prose form also carries
    values like "Call for Availability" that must read as zero rather than crash.
    """
    raw = p.get("AvailabilityInStock")
    if raw not in (None, ""):
        try:
            return int(str(raw).strip())
        except ValueError:
            pass
    m = re.match(r"\s*([\d,]+)", str(p.get("Availability") or ""))
    if m:
        return int(m.group(1).replace(",", ""))
    return 0


def _int_or_none(raw) -> int | None:
    if raw in (None, ""):
        return None
    try:
        return int(str(raw).strip())
    except ValueError:
        return None


def _lead_days(raw) -> int | None:
    """`'182 Days'` -> 182.

    Mouser states lead time in days already, unlike Digi-Key's weeks — so this is
    unit *extraction*, not conversion, and there is no factor to get wrong.
    """
    m = re.match(r"\s*(\d+)\s*day", str(raw or ""), re.IGNORECASE)
    return int(m.group(1)) if m else None


def _norm(mpn: str) -> str:
    """Comparison form for matching a result back to the request.

    Vendors decorate MPNs with packaging suffixes and punctuation that the spec
    does not carry. Matching on the raw strings is what reported stocked parts as
    absent at Digi-Key.
    """
    return re.sub(r"[^A-Za-z0-9]", "", mpn or "").upper()


class MouserAdapter(Adapter):
    name = "mouser"
    display_name = "Mouser Electronics"
    #: Mouser ships EU orders from its Munich-served European operation; prices
    #: come back in EUR for this account.
    warehouse = "DE"
    #: Declared, not inferred: see the module docstring. The pipeline reports
    #: this so a vendor's attribute silence is a stated property rather than
    #: something a reader has to deduce from an empty export.
    contributes_parametrics = False

    #: Empty BY MEASUREMENT, not by omission. Mouser's `ProductAttributes` holds
    #: only `Packaging` and `Standard Pack Qty`; the parametrics live unlabelled
    #: inside `Description`. See the module docstring for why parsing that is
    #: forbidden here.
    ATTR_MAP: dict[str, str] = {}

    def __init__(self, api_key: str, currency: str = "EUR", spent_today: int = 0):
        self._key = api_key or ""
        self.currency = currency
        self.throttle = Throttle(self.name, spent_today=spent_today)

    @property
    def calls(self) -> int:
        """Calls made, for `fetch_runs.api_calls`.

        Read from the throttle rather than counted separately, so the number the
        daily budget is computed from is the same number the run log records.
        """
        return self.throttle.calls

    def available(self) -> bool:
        return bool(self._key)

    # --- Transport ------------------------------------------------------

    def _post(self, path: str, body: dict, attempts: int = 4) -> dict:
        """One POST, paced and retried.

        Retries 429 and 5xx. Does NOT retry 4xx other than 429: a 400 means the
        request shape is wrong and repeating it just spends quota.
        """
        url = f"{_BASE}/{path}?apiKey={urllib.parse.quote(self._key)}"
        last: Exception | None = None
        for attempt in range(attempts):
            self.throttle.acquire()
            req = urllib.request.Request(
                url,
                data=json.dumps(body).encode(),
                method="POST",
                headers={"Content-Type": "application/json",
                         "Accept": "application/json"},
            )
            try:
                with urllib.request.urlopen(req, timeout=60) as r:
                    self.throttle.note_headers(r.headers)
                    payload = json.loads(r.read())
            except urllib.error.HTTPError as e:
                last = e
                if e.code in (429, 500, 502, 503, 504):
                    time.sleep(self.throttle.backoff(attempt, e.headers.get("Retry-After")))
                    continue
                raise
            except OSError as e:
                # OSError, not URLError: a READ timeout surfaces as a bare
                # TimeoutError straight from the socket rather than wrapped, so
                # a URLError clause lets it escape the retry loop entirely and
                # abort the run. Both are OSError subclasses, as is URLError.
                last = e
                time.sleep(self.throttle.backoff(attempt))
                continue
            # Mouser reports application errors in a 200 body. An invalid key
            # arrives this way, so it must not be mistaken for an empty result.
            errors = payload.get("Errors") or []
            if errors:
                raise RuntimeError(f"mouser {path}: {json.dumps(errors)[:300]}")
            return payload
        raise RuntimeError(f"mouser {path}: failed after {attempts} attempt(s): {last}")

    @staticmethod
    def _parts(payload: dict) -> list[dict]:
        return (payload.get("SearchResults") or {}).get("Parts") or []

    # --- Paths ----------------------------------------------------------

    def lookup_mpns(self, mpns: list[str]) -> list[VendorPart]:
        """Resolve each MPN. One call per part — Mouser has no batch lookup.

        An exact-match miss falls back to a keyword search, whose results are
        matched back on a normalised MPN. The fallback found nothing for
        `NSC5083D-22.5792M` and that is a genuine absence; it is kept because an
        exact lookup is only authoritative when the identifier the vendor indexes
        is the one being asked for, which is exactly what cannot be assumed.
        """
        wanted = [m for m in mpns if m and m.strip()]
        if not wanted:
            return []
        # Two calls per part worst case (exact, then keyword fallback).
        print(f"  {self.throttle.plan(len(wanted))}")
        if not self.throttle.affordable(len(wanted)):
            print(f"  WARNING: mouser daily budget cannot cover {len(wanted)} lookup(s); "
                  f"{self.throttle.remaining_today()} left — the run will stop when spent "
                  f"rather than silently fetch a subset.")

        out: list[VendorPart] = []
        # Parts whose lookup could not be COMPLETED, as opposed to parts the
        # vendor does not carry. Mouser 503s in clusters that outlast four
        # retries, and one unlucky part used to abort the other seventeen.
        #
        # Kept strictly separate from absence, and never written to the
        # catalogue: a 0-stock row for an unresolved part is indistinguishable
        # from a real stock-out, and that is the false negative this pipeline
        # keeps finding. An unresolved part leaves whatever the last successful
        # run recorded, which is the honest state.
        self.unresolved: list[tuple[str, str]] = []
        for mpn in wanted:
            try:
                found = self._parts(self._post("search/partnumber", {
                    "SearchByPartRequest": {
                        "mouserPartNumber": mpn,
                        "partSearchOptions": "Exact",
                    },
                }))
                match = next((p for p in found
                              if _norm(p.get("ManufacturerPartNumber")) == _norm(mpn)), None)
                if match is None:
                    found = self._parts(self._post("search/keyword", {
                        "SearchByKeywordRequest": {
                            "keyword": mpn, "records": 20, "startingRecord": 0,
                        },
                    }))
                    match = next((p for p in found
                                  if _norm(p.get("ManufacturerPartNumber")) == _norm(mpn)), None)
            except RuntimeError as e:
                self.unresolved.append((mpn, str(e)))
                continue
            if match is None:
                # Reached only from successful responses — a 503 would have
                # landed in `unresolved` rather than becoming "not carried".
                continue
            out.append(self._to_part(match, queried_as=mpn))
        if self.unresolved:
            print(f"  WARNING: {len(self.unresolved)} part(s) UNRESOLVED at mouser — "
                  f"the lookup failed, which is NOT the same as not carried. "
                  f"Nothing was written for them; re-run to retry:")
            for mpn, err in self.unresolved:
                print(f"      {mpn}: {err}")
        return out

    def search_category(
        self, category: str, limit: int, phrase: str | None = None
    ) -> list[VendorPart]:
        """Keyword discovery, paged 50 at a time.

        Used to find *second sources* for parts another vendor has already
        described parametrically — `CCHD-957-25-22.5792` turned up here with 59 in
        stock while the spec had it as Digi-Key-only. It is not used for
        parametric discovery, because a part known only to Mouser arrives with no
        attributes and so can never pass a filter; that would spend the daily
        budget to produce unjudgeable rows.
        """
        keyword = phrase or category
        out: list[VendorPart] = []
        page = min(50, max(1, limit))
        start = 0
        total: int | None = None
        while len(out) < limit:
            payload = self._post("search/keyword", {
                "SearchByKeywordRequest": {
                    "keyword": keyword,
                    "records": page,
                    "startingRecord": start,
                },
            })
            if total is None:
                total = (payload.get("SearchResults") or {}).get("NumberOfResult")
            batch = self._parts(payload)
            if not batch:
                break
            out.extend(self._to_part(p) for p in batch)
            start += len(batch)
            if len(batch) < page:
                break
        self.last_total = total
        if total is not None and total > len(out):
            # Never let a capped sweep read as a census.
            print(f"  limit={limit} caps '{keyword}' at {len(out)} of {total} match(es) — "
                  f"this is a SAMPLE, not the category")
        return out[:limit]

    # --- Mapping --------------------------------------------------------

    def _to_part(self, p: dict, queried_as: str | None = None) -> VendorPart:
        # Compliance as IPC-1752A-shaped claims, matching the other adapters: a
        # bare "RoHS Compliant" cannot be re-checked when the rules change.
        #
        # Only RoHS is taken. Mouser's `ProductCompliance` is mostly customs
        # classification — TARIC, CNHTS, CAHTS, USHTS — which are tariff codes,
        # not environmental regulations, and filing them as compliance claims
        # would make the posture look better evidenced than it is.
        rohs = (p.get("ROHSStatus") or "").strip()
        compliance = []
        if rohs:
            compliance.append({
                "regulation": "RoHS",
                # Mouser states compliance without a directive revision, so
                # "unstated" rather than a guessed one — same convention as the
                # Digi-Key adapter, and still re-checkable.
                "revision": "unstated",
                "status": rohs,
                "asserted_at": _TODAY,
                "source": self.name,
            })

        breaks = []
        for b in (p.get("PriceBreaks") or []):
            # `RawPrice` is a float; `Price` is a LOCALISED string ("0,213 €")
            # whose decimal separator is a comma on this account. Parsing the
            # display string would be a 1000x error dressed as a locale bug.
            raw = b.get("RawPrice")
            qty = _int_or_none(b.get("Quantity"))
            if raw is None or qty is None:
                continue
            breaks.append(PriceBreak(qty=qty, unit_price=float(raw),
                                     currency=b.get("Currency") or self.currency))

        return VendorPart(
            vendor=self.name,
            vendor_pn=p.get("MouserPartNumber") or "",
            mpn=p.get("ManufacturerPartNumber") or "",
            manufacturer=p.get("Manufacturer") or "",
            description=p.get("Description") or "",
            stock=_stock(p),
            # Empty by design — see the module docstring. Routed through
            # `map_attrs` anyway so that if Mouser ever starts publishing named
            # parameters, adding them here is the only change needed.
            attrs_raw=self.map_attrs([
                (a.get("AttributeName") or "", a.get("AttributeValue") or "")
                for a in (p.get("ProductAttributes") or [])
            ]),
            price_breaks=breaks,
            moq=_int_or_none(p.get("Min")),
            order_multiple=_int_or_none(p.get("Mult")),
            lead_time_days=_lead_days(p.get("LeadTime")),
            # Mouser returns `LifecycleStatus: null` for every part measured,
            # including ones Digi-Key marks Active. Passed through as-is: absent
            # evidence must stay absent rather than become "Active" by default,
            # which would let `no_preferred_part_is_end_of_life` pass on nothing.
            lifecycle_raw=p.get("LifecycleStatus") or None,
            queried_as=queried_as,
            status_raw=compliance,
            datasheet_url=(p.get("DataSheetUrl") or "").strip() or None,
        )
