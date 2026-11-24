"""TME (tme.eu) — the reference adapter.

API v2, OAuth 2.0 client-credentials. This is the vendor the pipeline was proved
against end to end, so the shape here is the pattern the other two should follow.

# Notes earned the hard way

* **v2 is not HMAC.** The 50-char token is the Basic-auth username and the
  20-char secret the password; that pair buys a Bearer token valid for 300
  seconds. Earlier documentation in this repo described the v1 HMAC scheme and
  was wrong.
* **The OpenAPI `required` flags are unreliable.** Four separate cases so far,
  and one of them breaks the call outright: sending `amounts[]` alongside
  `scope[]=prices,stock` returns HTTP 400
  `"product_data_accessor: This form should not contain extra fields"`, while the
  same document marks `amounts[]` required. The prose is authoritative.
* **Identity, parameters and pricing are three separate calls.** `/products`
  accepts `mpns[]`, which removes a symbol-resolution round trip.
* **`/products/data` caps at 50 symbols**; `/products/search` pages at 100.
"""

from __future__ import annotations

import base64
import json
import time
import urllib.error
import urllib.parse
import urllib.request

from .base import Adapter, PriceBreak, VendorPart

BASE = "https://api.tme.eu"
SYMBOL_BATCH = 50
SEARCH_PAGE = 100
TOKEN_REFRESH_MARGIN_S = 30

# `/products/parameters` and friends reject a symbol outside 2..18 characters
# with E_INPUT_PARAMS_VALIDATION_ERROR. This is enforced rather than hoped for
# because the failure is SILENT in the worst way: the batch errors, no attributes
# come back for any part in it, and every one is then rejected by the filter for
# a missing attribute — indistinguishable from "does not meet spec".
#
# Worth noting the trap is easy to fall into even knowing about it: the MPN is
# usually longer than TME's own symbol (ASV-22.5792MHZ-LR-T is 19 characters,
# ASV22.5792MHZLRT-0 is 18), so passing the wrong identifier fails validation
# rather than returning nothing.
SYMBOL_MAX_LEN = 18
SYMBOL_MIN_LEN = 2

# Document types on /products/files. DTE is a hosted datasheet PDF; LNK is a
# pointer whose `content` holds the manufacturer's own URL. YTB is a YouTube
# link and must never be mistaken for a datasheet.
DOC_TYPE_PREFERENCE = ("DTE", "LNK")


class TmeAdapter(Adapter):
    name = "tme"
    display_name = "Transfer Multisort Elektronik"
    # TME invoices through the national subsidiary the account belongs to, but
    # every order ships from the parent's Polish warehouse.
    warehouse = "PL"

    # TME parameter name -> spec attribute name.
    #
    # `supply_mv` deliberately has no _min/_max suffix: TME publishes ONE field
    # ("1.65...5.5V DC") and `parametric::derive` splits it into whichever bound a
    # constraint asks for. Doing that split here would put unit interpretation in
    # Python, which is exactly what the adapter boundary exists to prevent.
    ATTR_MAP = {
        # --- Semiconductors -------------------------------------------------
        "Supply voltage": "supply_mv",
        "Number of channels": "channels",
        "Output configuration": "switch_configuration",
        "Mounting": "mounting",
        "Case": "package",
        "Clock frequency": "core_clock_hz",
        "Manufacturer": "manufacturer_name",
        "Type of integrated circuit": "part_type",
        "Kind of architecture": "cpu_core",
        "Number of inputs/outputs": "gpio_count",
        "Operating temperature": "operating_temp_c",
        # --- Resonators and Generators (category 100269) ---------------------
        #
        # A different vocabulary for the same physics, which is why the map is
        # per-vendor rather than global. Two notes earned by reading real data:
        #
        #  * Active parts publish "Operating voltage", NOT "Supply voltage" —
        #    two vendor names for one spec attribute, which `map_attrs` allows.
        #  * "Type of quartz" is the discriminator the design depends on. The
        #    category mixes `crystal`, `ceramic` and `resonator` (passive, no
        #    output driver) with `oscillator` and `XO` (active). Only the latter
        #    two can drive the DAC's XTI and the SAI simultaneously — and TME
        #    uses BOTH words for them, which is why the constraint is a OneOf.
        "Frequency": "frequency_hz",
        "Operating voltage": "supply_mv",
        "Type of quartz": "part_type",
        "Output signal": "output_signal",
        "Frequency stability": "frequency_stability_ppm",
        "Manufacturer series": "series",
    }

    def __init__(self, token: str, secret: str, country: str = "SK", currency: str = "EUR"):
        self._token = token
        self._secret = secret
        self.country = country
        self.currency = currency
        self._bearer: str | None = None
        self._expires_at = 0.0
        self.calls = 0

    # --- Adapter surface ------------------------------------------------

    def available(self) -> bool:
        return bool(self._token and self._secret)

    def lookup_mpns(self, mpns: list[str]) -> list[VendorPart]:
        if not mpns:
            return []
        products = self._get("/products", [("country", self.country)]
                             + [("mpns[]", m) for m in mpns])
        elements = products.get("data", {}).get("elements", [])
        return self._hydrate(elements)

    def search_category(
        self, category: str, limit: int, phrase: str | None = None
    ) -> list[VendorPart]:
        """Page through a category id, optionally narrowed by a phrase.

        `category` is TME's numeric `category_id` as a string. The prose says to
        provide "either `phrase` or `category_id`, or both" even though the
        specification marks both required — so sending the category alone is
        valid, and adding a phrase narrows it.
        """
        collected: list[dict] = []
        page = 1
        expected: int | None = None
        pages: int | None = None
        while True:
            batch = self._get(
                "/products/search",
                [
                    ("country", self.country),
                    # `basic` is NOT a valid scope. The enum is
                    # products|parameters|counters, and the first attempt at this
                    # method used "basic" — it would have failed on every call.
                    #
                    # `counters` is requested for a reason beyond curiosity: it
                    # returns the vendor's OWN total, which is the only way to
                    # know a sweep is complete. See the check below.
                    ("scope[]", "products"),
                    ("scope[]", "counters"),
                    ("category_id", str(category)),
                    ("limit", str(min(SEARCH_PAGE, max(1, limit - len(collected))))),
                    ("page", str(page)),
                ]
                + ([("phrase", phrase)] if phrase else []),
            )
            if "_http_error" in batch:
                # NOT a silent break. A mid-sweep failure leaves a partial
                # category that looks complete, and every "all N parts" claim
                # downstream would be wrong. Raise with what is known.
                raise RuntimeError(
                    f"TME search failed on page {page} of category {category}"
                    f"{' phrase=' + phrase if phrase else ''}: HTTP "
                    f"{batch['_http_error']} {batch.get('_detail', '')[:200]}. "
                    f"{len(collected)} part(s) had been collected; treating that as the whole "
                    f"category would be wrong, so nothing is returned."
                )
            data = batch.get("data", {})
            # `/products/search` nests under data.products.elements, NOT
            # data.elements like `/products` does. The first version of this
            # method guessed the latter and returned zero parts for every query —
            # which read exactly like "TME does not stock this", the single most
            # misleading way for a fetcher to fail. The fallbacks are kept because
            # two sibling endpoints in the same API already disagree on this.
            found = (
                (data.get("products") or {}).get("elements")
                or data.get("elements")
                or data.get("product_list")
                or []
            )
            counters = data.get("counters") or {}
            if expected is None:
                expected = counters.get("count")
                pages = counters.get("pages")
            if not found:
                break
            collected.extend(found)
            if pages is not None and page >= pages:
                break
            if len(collected) >= limit:
                break
            page += 1

        # Completeness check. The vendor states how many parts match; if fewer
        # were collected and the caller did not cap it, the sweep is incomplete
        # and must say so rather than be reported as the category.
        if expected is not None and len(collected) < expected and limit >= expected:
            raise RuntimeError(
                f"TME says category {category}"
                f"{' phrase=' + phrase if phrase else ''} has {expected} matching part(s) across "
                f"{pages} page(s), but only {len(collected)} were collected. An incomplete sweep "
                f"reported as a category is how 'nothing is sourceable' gets claimed on partial "
                f"data."
            )
        if expected is not None and limit < expected:
            print(
                f"  NOTE: limit={limit} caps a category of {expected} part(s) — "
                f"this is a SAMPLE, not the category."
            )

        symbols = [e.get("symbol") for e in collected if e.get("symbol")]
        if not symbols:
            return []
        # Search results are thin; re-read identity so every part has the same
        # fields regardless of which path found it.
        full: list[dict] = []
        for chunk in _chunks(symbols, SYMBOL_BATCH):
            r = self._get("/products", [("country", self.country)]
                          + [("symbols[]", s) for s in chunk])
            full.extend(r.get("data", {}).get("elements", []))
        return self._hydrate(full)

    # --- Internals ------------------------------------------------------

    def _datasheets(self, symbols: list[str]) -> dict[str, str]:
        """Datasheet URL per symbol, from `/products/files`.

        Worth wiring up rather than reading product pages by hand: for the
        Abracon ASV this returns `https://abracon.com/Oscillators/ASV.pdf` — the
        exact URL that was previously guessed. Every `Evidence::Datasheet`
        constraint needs a datasheet, so automating the link is leverage on all
        of them.

        `DTE` (a hosted PDF) is preferred over `LNK` (a pointer to the
        manufacturer's own copy) because the hosted one cannot rot; both beat
        nothing. `YTB` entries are YouTube links and are excluded explicitly —
        taking "the first document" would have returned a product video for the
        TI part.
        """
        out: dict[str, str] = {}
        ok, _bad = _queryable(symbols)
        for chunk in _chunks(ok, SYMBOL_BATCH):
            r = self._get(
                "/products/files",
                [("country", self.country)] + [("symbols[]", s) for s in chunk],
            )
            for e in r.get("data", {}).get("elements", []):
                docs = (e.get("documents") or {}).get("elements") or []
                best = None
                for want in DOC_TYPE_PREFERENCE:
                    for d in docs:
                        if d.get("type") != want:
                            continue
                        if want == "LNK":
                            url = (d.get("content") or "").strip()
                        else:
                            url = (d.get("url") or "").strip()
                            if url.startswith("//"):
                                url = "https:" + url
                        if url.startswith("http"):
                            best = url
                            break
                    if best:
                        break
                if best:
                    out[e["symbol"]] = best
        return out

    def _hydrate(self, elements: list[dict]) -> list[VendorPart]:
        """Attach parameters and pricing to identity records."""
        all_symbols = [e["symbol"] for e in elements if e.get("symbol")]
        symbols, unqueryable = _queryable(all_symbols)
        if unqueryable:
            # Loud, not silent. These parts would otherwise arrive with zero
            # attributes and be rejected as if they failed the spec.
            print(
                f"  WARNING: {len(unqueryable)} symbol(s) outside TME's "
                f"{SYMBOL_MIN_LEN}-{SYMBOL_MAX_LEN} character limit; their parameters, pricing "
                f"and datasheets CANNOT be fetched and they are omitted rather than returned "
                f"attribute-less: {unqueryable}"
            )
        params: dict[str, list[tuple[str, str]]] = {}
        pricing: dict[str, dict] = {}
        datasheets = self._datasheets(symbols)

        for chunk in _chunks(symbols, SYMBOL_BATCH):
            r = self._get("/products/parameters", [("country", self.country)]
                          + [("symbols[]", s) for s in chunk])
            for e in r.get("data", {}).get("elements", []):
                params[e["symbol"]] = [
                    (p["name"], v["value"])
                    for p in e.get("parameters", {}).get("elements", [])
                    for v in p.get("values", [])
                ]
            # NO amounts[] — see the module docstring. Sending it with
            # prices/stock is rejected as an extra field.
            r = self._get(
                "/products/data",
                [("country", self.country), ("currency", self.currency),
                 ("scope[]", "prices"), ("scope[]", "stock")]
                + [("symbols[]", s) for s in chunk],
            )
            for e in r.get("data", {}).get("elements", []):
                pricing[e["symbol"]] = e

        out: list[VendorPart] = []
        queryable = set(symbols)
        for e in elements:
            sym = e.get("symbol")
            if not sym or sym not in queryable:
                continue
            pr = pricing.get(sym, {})
            prices = pr.get("prices") or {}
            mfr = e.get("manufacturer")
            out.append(
                VendorPart(
                    vendor=self.name,
                    vendor_pn=sym,
                    mpn=(e.get("manufacturer_symbols") or [sym])[0],
                    manufacturer=(mfr.get("name") if isinstance(mfr, dict) else mfr) or "",
                    description=e.get("description") or "",
                    stock=pr.get("stock_quantity", 0) or 0,
                    attrs_raw=self.map_attrs(params.get(sym, [])),
                    price_breaks=[
                        PriceBreak(
                            qty=int(el["amount"]),
                            unit_price=float(el["price"]),
                            currency=prices.get("currency") or self.currency,
                        )
                        for el in prices.get("elements", [])
                    ],
                    moq=e.get("minimal_amount"),
                    order_multiple=e.get("multiples"),
                    # TME publishes no lifecycle field. `product_status` is an
                    # ORDERING restriction (BLOCKED_FOR_ZBL_BACKORDERS means no
                    # zero-balance backorders), not a lifecycle state, so it is
                    # kept separate rather than mapped onto one.
                    lifecycle_raw=None,
                    status_raw=e.get("product_status") or [],
                    datasheet_url=datasheets.get(sym),
                )
            )
        return out

    def _authenticate(self) -> None:
        basic = base64.b64encode(f"{self._token}:{self._secret}".encode()).decode()
        req = urllib.request.Request(
            f"{BASE}/auth/token",
            data=urllib.parse.urlencode({"grant_type": "client_credentials"}).encode(),
            method="POST",
            headers={
                "Authorization": f"Basic {basic}",
                "Content-Type": "application/x-www-form-urlencoded",
            },
        )
        with urllib.request.urlopen(req, timeout=30) as r:
            d = json.loads(r.read())
        self._bearer = d["access_token"]
        self._expires_at = time.monotonic() + float(d.get("expires_in", 300))
        self.calls += 1

    def _get(self, path: str, params: list[tuple[str, str]]) -> dict:
        if self._bearer is None or time.monotonic() > self._expires_at - TOKEN_REFRESH_MARGIN_S:
            self._authenticate()
        url = f"{BASE}{path}?{urllib.parse.urlencode(params)}"
        req = urllib.request.Request(url, headers={"Authorization": f"Bearer {self._bearer}"})
        self.calls += 1
        try:
            with urllib.request.urlopen(req, timeout=45) as r:
                return json.loads(r.read())
        except urllib.error.HTTPError as e:
            # A 404 for a part the vendor does not stock is an ANSWER, not a
            # failure, so errors are returned rather than raised.
            return {"_http_error": e.code, "_detail": e.read().decode(errors="replace")[:400]}


def _chunks(xs: list, n: int):
    for i in range(0, len(xs), n):
        yield xs[i : i + n]


def _queryable(symbols: list[str]) -> tuple[list[str], list[str]]:
    """Split symbols into those the API will accept and those it will not.

    Returns `(ok, rejected)`. The caller must SURFACE `rejected` — dropping them
    quietly is how a part ends up with no attributes and looks unqualified.
    """
    ok, bad = [], []
    for sym in symbols:
        (ok if SYMBOL_MIN_LEN <= len(sym) <= SYMBOL_MAX_LEN else bad).append(sym)
    return ok, bad
