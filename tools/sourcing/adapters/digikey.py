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

from .base import Adapter, PriceBreak, VendorPart

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
SEARCH_PAGE = 50


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
        "Data Interface": "data_interface",
        "Number of Bits": "resolution_bits",
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
        "Circuit": "switch_configuration",
        "Independent Circuits": "channels",
        "On-State Resistance (Max)": "ron_max_ohm",
        # --- MCU --------------------------------------------------------------
        "Core Processor": "cpu_core",
        "Speed": "core_clock_hz",
        "RAM Size": "sram_bytes",
        "Program Memory Size": "flash_bytes",
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
                 host: str = "api.digikey.com", site: str = "NL", currency: str = "EUR"):
        self._id = client_id
        self._secret = client_secret
        self.host = host
        self.site = site
        self.currency = currency
        self._bearer: str | None = None
        self._expires_at = 0.0
        self.calls = 0

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
        while len(collected) < limit:
            body = {
                "Keywords": phrase or "",
                "Limit": min(SEARCH_PAGE, limit - len(collected)),
                "Offset": offset,
            }
            if category:
                body["FilterOptionsRequest"] = {
                    "CategoryFilter": [{"Id": str(category)}]
                }
            d = self._post("/products/v4/search/keyword", body)
            if "_http_error" in d:
                raise RuntimeError(
                    f"Digi-Key keyword search failed at offset {offset}: HTTP "
                    f"{d['_http_error']} {d.get('_detail','')[:200]}. {len(collected)} part(s) "
                    f"collected; reporting that as the category would be wrong."
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
        if expected is not None and limit < expected:
            print(f"  NOTE: limit={limit} caps a category of {expected} part(s) — "
                  f"this is a SAMPLE, not the category.")
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
            datasheet_url=p.get("DatasheetUrl") or None,
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
        self.calls += 1

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

    def _get(self, path: str) -> dict:
        req = urllib.request.Request(f"https://{self.host}{path}", headers=self._headers())
        self.calls += 1
        try:
            with urllib.request.urlopen(req, timeout=45) as r:
                return json.loads(r.read())
        except urllib.error.HTTPError as e:
            # A 404 for a part Digi-Key does not carry is an ANSWER.
            return {"_http_error": e.code, "_detail": e.read().decode(errors="replace")[:300]}

    def _post(self, path: str, body: dict) -> dict:
        headers = {**self._headers(), "Content-Type": "application/json"}
        req = urllib.request.Request(
            f"https://{self.host}{path}", data=json.dumps(body).encode(), headers=headers,
            method="POST",
        )
        self.calls += 1
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                return json.loads(r.read())
        except urllib.error.HTTPError as e:
            return {"_http_error": e.code, "_detail": e.read().decode(errors="replace")[:300]}
