"""Digi-Key — Product Information v4.

Written against a recorded response rather than documentation, for the reason the
TME work established the hard way: transcribing field names from prose produces a
parser that looks right and silently maps nothing.

# Notes earned on first contact

* **OAuth 2.0 two-legged, same shape as TME v2** — but the credentials go in the
  request BODY (`client_id`/`client_secret`), not as HTTP Basic auth. Tokens last
  ~600 s, shorter than TME's 300 s is long, so a multi-category run refreshes
  mid-flight.
* **A Production App cannot authenticate against the sandbox.** `sandbox-api`
  returns `401 Invalid clientId — "clientId invalid for requested resource"`,
  which reads like a bad secret and is not: the sandbox needs its own separate
  app. Advice to "prove the mapping on the sandbox first" was wrong, and the
  mitigation is instead what it always should have been — probe ONE part, record
  it, write the mapping against the recording.
* **Every call needs `X-DIGIKEY-Client-Id`** alongside the bearer token, plus the
  locale headers that decide currency and warehouse.
* Stock, MOQ and pricing live under `ProductVariations[]`, not on the product —
  one entry per packaging option (Tray, Cut Tape, Digi-Reel), each with its own
  availability. Which variation you pick IS a sourcing decision, so the one with
  the lowest MOQ wins here.
"""

from __future__ import annotations

import json
import time
import urllib.error
import urllib.parse
import urllib.request

from ..throttle import Throttle
from .base import Adapter, PriceBreak, VendorPart

def _absolute(url: str | None) -> str | None:
    """Make a protocol-relative datasheet URL openable."""
    if not url:
        return None
    u = url.strip()
    if u.startswith("//"):
        return "https:" + u
    return u if u.startswith("http") else None


def _weeks_to_days(weeks) -> int | None:
    """Digi-Key reports `ManufacturerLeadWeeks` as a string; 0 means unstated."""
    try:
        w = int(str(weeks).strip())
    except (TypeError, ValueError):
        return None
    return w * 7 if w > 0 else None


_TODAY = "2026-11-24"
TOKEN_PATH = "/v1/oauth2/token"
TOKEN_REFRESH_MARGIN_S = 60
# Digi-Key rejects Limit=100 with HTTP 400, so 50 is the documented ceiling and
# the clamp is real rather than defensive.
SEARCH_PAGE = 50
# The keyword endpoint returns transient 500s ("An error occurred while
# processing your request") for requests that succeed on retry — the same
# flakiness `productdetails` shows. A 5xx is never an answer.
SEARCH_RETRIES = 4
# **Digi-Key will not paginate past 300 results for any query.** Measured:
# `Offset=300, Limit=50` returns HTTP 400 "Offset + Limit must be less than or
# equal to 300".
#
# This is a hard ceiling on the whole discovery path, not a tuning knob, and it
# is the reason `SWEEPS` declares several keyword angles per requirement instead
# of one broad query: with a 300-result window, unioning angles is the ONLY way
# to see more than 300 parts. Measured gain for a second angle on
# "22.5792MHz" vs "22.5792MHz XO": 241 of 300 overlap, union 359, so +20% — real
# but with clear diminishing returns, and the parts it adds are genuine
# oscillators (CB3LV, DSC1001, CMX-309FAB) rather than noise.
SEARCH_MAX_WINDOW = 300


class DigikeyAdapter(Adapter):
    name = "digikey"
    display_name = "Digi-Key"
    # The NL warehouse is what makes this an EU-domestic delivery.
    warehouse = "NL"

    # Digi-Key parameter name -> spec attribute name.
    #
    # `Voltage - Supply, Analog` is a RANGE string ("1.66V ~ 1.94V") and maps to
    # the unsuffixed `supply_mv`, exactly as TME's does: parametric::derive splits
    # it into whichever bound a constraint asks for. Splitting it here would put
    # unit interpretation on the vendor side of the adapter boundary.
    ATTR_MAP = {
        # --- DAC / analogue ---------------------------------------------------
        "Number of D/A Converters": "channels",
        # Digi-Key's other spelling, used by parts filed outside the dedicated
        # DAC category. Verified not to co-occur with `Number of D/A Converters`
        # on any part — `map_attrs` JOINS duplicate keys with '; ', which is
        # correct for a multi-valued list and corrupting for a scalar.
        "Number of Channels": "channels",
        "Data Interface": "data_interface",
        "Number of Bits": "resolution_bits",
        "Resolution (Bits)": "resolution_bits",
        "Architecture": "converter_architecture",
        "Voltage - Supply, Analog": "supply_mv",
        "Voltage - Supply, Digital": "supply_digital_mv",
        # --- Shared -----------------------------------------------------------
        "Operating Temperature": "operating_temp_c",
        "Mounting Type": "mounting",
        "Package / Case": "package",
        "Supplier Device Package": "supplier_package",
        # --- Oscillators ------------------------------------------------------
        "Frequency": "frequency_hz",
        "Frequency Stability": "frequency_stability_ppm",
        # `Type` carries 'XO (Standard)' — the active-vs-passive discriminator.
        #
        # NOT `Function`, which was mapped here first and holds the power-down
        # mode ('Standby (Power Down)'). And NOT `Base Resonator`, which says
        # 'Crystal' for a perfectly good XO because it describes the resonating
        # element INSIDE the oscillator. Either mistake rejects a correct part as
        # passive quartz.
        "Type": "part_type",
        "Output": "output_signal",
        "Size / Dimension": "body_size",
        "Height - Seated (Max)": "height_mm",
        "Voltage - Supply": "supply_mv",
        # --- Switches / muxes -------------------------------------------------
        #
        # `Switch Circuit` and `Number of Circuits` are the REAL names, verified
        # against 50 parts where both appear on 50 and 49 of them. This slot used
        # to hold `Circuit` and `Independent Circuits`, which were transcribed
        # from Digi-Key's web UI rather than read off a response and therefore
        # never matched anything.
        #
        # The cost was invisible and large: `switch_configuration` and `channels`
        # were reaching the filter from TME ALONE, so 386 of 668 muxes were
        # rejected as "switch_configuration not published" and the Nexperia
        # alternate `74LVC1G3157GW` — which TME does not carry — failed its own
        # requirement entirely. A mapping gap reads exactly like a vendor data
        # gap, and only the rejection histogram told them apart.
        "Switch Circuit": "switch_configuration",
        "Number of Circuits": "channels",
        # Digi-Key's supply field for analog switches, 50/50 coverage. Separate
        # from `Voltage - Supply, Dual (V±)`, which this design has no use for
        # and which comes back '-' for every single-supply part anyway.
        "Voltage - Supply, Single (V+)": "supply_mv",
        "On-State Resistance (Max)": "ron_max_ohm",
        # --- MCU --------------------------------------------------------------
        "Core Processor": "cpu_core",
        "Speed": "core_clock_hz",
        # 48/50 coverage, and the reason 101 MCUs were rejected for an
        # unpublished supply voltage.
        "Voltage - Supply (Vcc/Vdd)": "supply_mv",
        "RAM Size": "sram_bytes",
        "Program Memory Size": "flash_bytes",
        # --- Card connectors --------------------------------------------------
        #
        # Verified against 25 real microSD sockets, where `Card Type` appears on
        # 25 of 25 and the rest on 23-24. `Card Type` is the microSD-vs-full-SD
        # discriminator and the only field that answers the requirement at all.
        "Card Type": "card_type",
        "Insertion, Removal Method": "insertion_removal",
        # Carries 'Switch' for a socket with a card-detect contact, which is what
        # lets firmware notice a card arriving rather than polling for it.
        "Features": "connector_features",
        # Mapped but NOT constrained — see MICROSD_CONSTRAINTS. Digi-Key states
        # it as '0.071" (1.80mm)', dual-unit, which is exactly the kind of string
        # that needs `parametric::derive` to be taught a rule before a constraint
        # can safely rest on it.
        "Height Above Board": "height_mm",
        "Number of Positions": "pin_count",
        # --- Battery ----------------------------------------------------------
        #
        # Mapped hopefully rather than observed: Digi-Key returns ZERO parametric
        # fields for the Akyga cell it stocks, so it is a second DISTRIBUTOR for
        # the battery but not a second data source. These names come from
        # Digi-Key's battery category generally and are unverified for cells.
        "Battery Chemistry": "battery_chemistry",
        "Capacity": "capacity_mah",
        "Voltage - Rated": "rated_voltage_mv",
        "Termination": "termination",
    }

    def __init__(self, client_id: str, client_secret: str,
                 host: str = "api.digikey.com", site: str = "NL", currency: str = "EUR",
                 spent_today: int = 0):
        self._id = client_id
        self._secret = client_secret
        self.host = host
        self.site = site
        self.currency = currency
        self._bearer: str | None = None
        self._expires_at = 0.0
        # Paced and budgeted through the shared throttle, like Mouser.
        #
        # This was missing and it cost a run: `throttle.py` was written for
        # Mouser's 30/minute cap and only Mouser was wired to it, so Digi-Key
        # made unpaced calls with no daily accounting and a sweep hit HTTP 429
        # mid-run. Seven sweeps then failed at offset 0, correctly refusing to
        # report a partial category — but the whole point of having a limiter was
        # not to get there.
        self.throttle = Throttle(self.name, spent_today=spent_today)

    # --- Adapter surface ------------------------------------------------

    def available(self) -> bool:
        return bool(self._id and self._secret)

    def lookup_mpns(self, mpns: list[str]) -> list[VendorPart]:
        """One `productdetails` call per part.

        `productdetails` first, then KEYWORD SEARCH for anything it 404s.
        
        The fallback is not redundancy; the exact lookup produces false negatives.
        Digi-Key's `productdetails` wants its own product number or the full
        manufacturer PN *including the packaging suffix*, so a bare
        `74LVC1G3157GW` 404s while a keyword search finds `74LVC1G3157GW,125`
        with 40729 in stock. Without this, the Preferred mux was reported
        unsourceable at both distributors — and it is stocked six figures deep.
        
        An earlier version of this docstring argued the opposite: that an exact
        lookup is the right source for a load-bearing "not carried" verdict. That
        is true only if the identifier the vendor indexes is the one being asked
        for, which is exactly what cannot be assumed — the same lesson TME taught
        with symbol-versus-MPN.
        """
        out: list[VendorPart] = []
        failed: list[tuple[str, int]] = []
        for mpn in mpns:
            quoted = urllib.parse.quote(mpn, safe="")
            d = self._get(f"/products/v4/search/{quoted}/productdetails")

            # 404 is an ANSWER: Digi-Key does not carry this part. Anything else
            # is a FETCH FAILURE and must never be mistaken for absence.
            #
            # This distinction is not academic. Digi-Key intermittently returns
            # 500 "Error converting response" for valid parts — SN74LVC1G3157DCKR
            # among them — and swallowing it made the same run report 10 parts
            # once and 6 the next, with four silently reclassified as "not
            # carried". A not-carried verdict is load-bearing, so it only ever
            # comes from a 404.
            for attempt in range(3):
                code = d.get("_http_error")
                if code is None or code == 404:
                    break
                time.sleep(1.5 * (attempt + 1))
                d = self._get(f"/products/v4/search/{quoted}/productdetails")

            code = d.get("_http_error")
            if code is not None and code != 404:
                failed.append((mpn, code))
                continue

            product = d.get("Product") if code is None else None
            if product is None:
                # 404 from the exact lookup: try the keyword index before
                # concluding anything.
                kw = self._post(
                    "/products/v4/search/keyword",
                    {"Keywords": mpn, "Limit": 10, "Offset": 0},
                )
                if "_http_error" in kw:
                    failed.append((mpn, kw["_http_error"]))
                    continue
                wanted = mpn.upper()
                for cand in kw.get("Products") or []:
                    got = (cand.get("ManufacturerProductNumber") or "").upper()
                    # Prefix match, because the vendor's PN carries a packaging
                    # suffix the spec does not: 74LVC1G3157GW vs
                    # 74LVC1G3157GW,125. Guard against matching a longer DIFFERENT
                    # part by requiring the remainder to be punctuation-led.
                    if got == wanted or (
                        got.startswith(wanted)
                        and got[len(wanted) :][:1] in {",", "-", "/", " ", "T"}
                    ):
                        product = cand
                        break
            if product:
                part = self._to_part(product)
                part.queried_as = mpn
                out.append(part)

        if failed:
            # Loud, and not an exception: a partial result is still useful, but
            # the caller must not read the gap as absence.
            print(
                f"  WARNING: {len(failed)} lookup(s) FAILED after retries and are NOT 'not "
                f"carried' — their availability is unknown: {failed}"
            )
        return out

    def search_category(
        self, category: str, limit: int, phrase: str | None = None
    ) -> list[VendorPart]:
        """Keyword search, paged.

        `category` is a Digi-Key category id; `phrase` narrows within it. Unlike
        TME there is no separate parameters call — the search returns full
        products — so a sweep is one call per page.
        """
        collected: list[dict] = []
        offset = 0
        expected: int | None = None
        # Clamp to the vendor's hard pagination window. Asking for more produced
        # an HTTP 400 that aborted the sweep, so five declared sweeps returned
        # nothing at all — correctly, since reporting the 300 already collected
        # as the category would have been a lie, but the fix belongs here rather
        # than in every caller's limit.
        reachable = min(limit, SEARCH_MAX_WINDOW)
        while len(collected) < reachable:
            body = {
                "Keywords": phrase or "",
                # Both terms clamped: the page size AND the remaining window, so
                # `Offset + Limit` can never exceed 300.
                "Limit": min(SEARCH_PAGE, reachable - len(collected),
                             SEARCH_MAX_WINDOW - offset),
                "Offset": offset,
            }
            if category:
                body["FilterOptionsRequest"] = {
                    "CategoryFilter": [{"Id": str(category)}]
                }
            d = self._post("/products/v4/search/keyword", body)
            # Retry 5xx with backoff. Without this a single transient failure
            # aborted the whole sweep: all five Digi-Key sweeps failed at
            # offset 0 on a request shape that had worked minutes earlier.
            for attempt in range(SEARCH_RETRIES):
                code = d.get("_http_error")
                if code is None or code < 500:
                    break
                time.sleep(2.0 * (attempt + 1))
                d = self._post("/products/v4/search/keyword", body)
            if "_http_error" in d:
                code = d["_http_error"]
                # Say what actually happened. A 4xx is not retried (repeating a
                # malformed request only spends quota), so claiming "after 4
                # retries" for a 400 sent me looking for a flaky endpoint when
                # the request shape was simply wrong.
                tried = (
                    f"after {SEARCH_RETRIES} retries"
                    if code >= 500
                    else "rate limited — retried with Retry-After backoff and still refused; "
                    "this is usually the DAILY cap, which no backoff can wait out"
                    if code == 429
                    else "not retried (4xx)"
                )
                raise RuntimeError(
                    f"Digi-Key keyword search failed at offset {offset}: HTTP "
                    f"{code} {tried}: "
                    f"{d.get('_detail','')[:160]}. {len(collected)} part(s) collected; reporting "
                    f"that as the category would be wrong."
                )
            if expected is None:
                expected = d.get("ProductsCount")
            products = d.get("Products") or []
            if not products:
                break
            collected.extend(products)
            offset += len(products)
            if expected is not None and offset >= expected:
                break
        self.last_total = expected
        if expected is not None and len(collected) < expected:
            why = (
                f"Digi-Key will not paginate past {SEARCH_MAX_WINDOW}"
                if limit > SEARCH_MAX_WINDOW
                else f"limit={limit}"
            )
            print(f"  NOTE: {why}, so this is {len(collected)} of {expected} part(s) — "
                  f"a SAMPLE, not the category. Digi-Key ranks by relevance, so these are the "
                  f"most relevant {len(collected)}; add another keyword angle to see more.")
        return [self._to_part(p) for p in collected]

    # --- Internals ------------------------------------------------------

    def _to_part(self, p: dict) -> VendorPart:
        params = [
            (x.get("ParameterText") or "", x.get("ValueText") or "")
            for x in (p.get("Parameters") or [])
            # '-' is Digi-Key's "not specified". Treating it as a value would
            # make an unpublished figure look published.
            if (x.get("ValueText") or "").strip() not in ("", "-")
        ]
        variations = p.get("ProductVariations") or []
        # Lowest MOQ wins: which packaging you buy IS a sourcing decision, and a
        # Digi-Reel of 490 is not the same offer as a tray of one.
        best = min(
            variations,
            key=lambda v: (v.get("MinimumOrderQuantity") or 10**9),
            default={},
        )
        # Compliance as IPC-1752A-shaped claims, not bare strings.
        #
        # A claim is (regulation, revision, status, asserted_at): a bare
        # "ROHS3 Compliant" cannot be re-checked when the rules change, and the
        # snapshot gate rejects it for exactly that reason. The regulation name
        # comes from WHICH API field carried it — that is a mapping, the adapter's
        # job — while the vendor's wording is preserved verbatim as the status so
        # nothing is paraphrased away.
        classifications = p.get("Classifications") or {}
        compliance = [
            {
                "regulation": regulation,
                # Digi-Key folds the revision into the status text ("ROHS3
                # Compliant"), so it is extracted only when it is unambiguous and
                # left null otherwise rather than guessed.
                # "unstated" rather than null. RoHS folds its revision into the
                # status text ("ROHS3 Compliant") and REACH has none Digi-Key
                # supplies at all — so a null here is ambiguous between "absent"
                # and "not applicable", and the snapshot gate rightly refuses it.
                #
                # Saying "unstated" keeps the claim RE-CHECKABLE, which is the
                # gate's actual purpose: a reader knows the regulation, the
                # vendor's exact wording, when they asserted it, and that no
                # revision was given. That is strictly more than dropping the
                # claim, and it is not a guess.
                "revision": next(
                    (r for r in ("ROHS3", "ROHS2", "ROHS") if raw.upper().startswith(r)),
                    "unstated",
                ),
                "status": raw,
                "asserted_at": _TODAY,
                "source": "digikey",
            }
            for regulation, raw in (
                ("RoHS", classifications.get("RohsStatus") or ""),
                ("REACH", classifications.get("ReachStatus") or ""),
            )
            if raw
        ]
        lead_weeks = p.get("ManufacturerLeadWeeks")
        return VendorPart(
            vendor=self.name,
            vendor_pn=best.get("DigiKeyProductNumber") or p.get("ManufacturerProductNumber") or "",
            mpn=p.get("ManufacturerProductNumber") or "",
            manufacturer=(p.get("Manufacturer") or {}).get("Name") or "",
            description=(p.get("Description") or {}).get("ProductDescription") or "",
            stock=best.get("QuantityAvailableforPackageType") or p.get("QuantityAvailable") or 0,
            attrs_raw=self.map_attrs(params),
            price_breaks=[
                PriceBreak(
                    qty=int(b.get("BreakQuantity") or 1),
                    unit_price=float(b.get("UnitPrice") or 0.0),
                    currency=b.get("Currency") or self.currency,
                )
                for b in (best.get("StandardPricing") or [])
                if b.get("UnitPrice")
            ],
            moq=best.get("MinimumOrderQuantity"),
            order_multiple=best.get("StandardPackage"),
            # Weeks -> days. Digi-Key returns this as a STRING ('18', '0'), so an
            # isinstance(int|float) check silently discarded every lead time.
            lead_time_days=_weeks_to_days(lead_weeks),
            # Digi-Key DOES publish lifecycle, unlike TME — this is the first
            # vendor that can answer `no_preferred_part_is_end_of_life`.
            lifecycle_raw=(p.get("ProductStatus") or {}).get("Status"),
            status_raw=compliance,
            # Absolutised here: Digi-Key returns protocol-relative paths
            # (//media.digikey.com/...) exactly as TME does, and a URL nobody can
            # open is not a datasheet link. Caught by the slice gate.
            datasheet_url=_absolute(p.get("DatasheetUrl")),
            product_url=_absolute(p.get("ProductUrl")),
        )

    def _authenticate(self) -> None:
        import time

        body = urllib.parse.urlencode(
            {
                "client_id": self._id,
                "client_secret": self._secret,
                "grant_type": "client_credentials",
            }
        ).encode()
        req = urllib.request.Request(
            f"https://{self.host}{TOKEN_PATH}",
            data=body,
            method="POST",
            headers={"Content-Type": "application/x-www-form-urlencoded"},
        )
        with urllib.request.urlopen(req, timeout=30) as r:
            d = json.loads(r.read())
        self._bearer = d["access_token"]
        self._expires_at = time.monotonic() + float(d.get("expires_in", 600))
        self.throttle.calls += 1

    def _headers(self) -> dict[str, str]:
        import time

        if self._bearer is None or time.monotonic() > self._expires_at - TOKEN_REFRESH_MARGIN_S:
            self._authenticate()
        return {
            "Authorization": f"Bearer {self._bearer}",
            # Required alongside the bearer token; omitting it is a 401 that
            # looks like bad credentials.
            "X-DIGIKEY-Client-Id": self._id,
            "X-DIGIKEY-Locale-Site": self.site,
            "X-DIGIKEY-Locale-Language": "en",
            "X-DIGIKEY-Locale-Currency": self.currency,
            "Accept": "application/json",
        }

    @property
    def calls(self) -> int:
        """Calls made, read from the throttle so budget and log agree."""
        return self.throttle.calls

    def _retry_429(self, e) -> float | None:
        """Backoff for a rate-limited call, or None if it must not be retried.

        **429 is retryable and was not being retried.** The search path only
        retried `>= 500`, so a rate-limited sweep died at offset 0 and the error
        message even called it "not retried (4xx)" — true, and the wrong
        behaviour. Mouser's adapter had honoured `Retry-After` from the start;
        this is the same rule.
        """
        if e.code != 429:
            return None
        return self.throttle.backoff(0, e.headers.get("Retry-After"))

    def _get(self, path: str, attempts: int = 3) -> dict:
        for attempt in range(attempts):
            self.throttle.acquire()
            req = urllib.request.Request(f"https://{self.host}{path}", headers=self._headers())
            try:
                with urllib.request.urlopen(req, timeout=45) as r:
                    self.throttle.note_headers(r.headers)
                    return json.loads(r.read())
            except urllib.error.HTTPError as e:
                wait = self._retry_429(e)
                if wait is not None and attempt + 1 < attempts:
                    time.sleep(wait)
                    continue
                # A 404 for a part Digi-Key does not carry is an ANSWER.
                return {"_http_error": e.code,
                        "_detail": e.read().decode(errors="replace")[:300]}
        return {"_http_error": 429, "_detail": "rate limited after retries"}

    def _post(self, path: str, body: dict, attempts: int = 3) -> dict:
        for attempt in range(attempts):
            self.throttle.acquire()
            headers = {**self._headers(), "Content-Type": "application/json"}
            req = urllib.request.Request(
                f"https://{self.host}{path}", data=json.dumps(body).encode(), headers=headers,
                method="POST",
            )
            try:
                with urllib.request.urlopen(req, timeout=60) as r:
                    self.throttle.note_headers(r.headers)
                    return json.loads(r.read())
            except urllib.error.HTTPError as e:
                wait = self._retry_429(e)
                if wait is not None and attempt + 1 < attempts:
                    time.sleep(wait)
                    continue
                return {"_http_error": e.code,
                        "_detail": e.read().decode(errors="replace")[:300]}
        return {"_http_error": 429, "_detail": "rate limited after retries"}
