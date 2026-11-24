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

    def search_category(self, category: str, limit: int) -> list[VendorPart]:
        """Page through a category id.

        `category` is TME's numeric `category_id` as a string. The prose says to
        provide "either `phrase` or `category_id`, or both" even though the
        specification marks both required, so only the category is sent.
        """
        collected: list[dict] = []
        page = 1
        while len(collected) < limit:
            batch = self._get(
                "/products/search",
                [
                    ("country", self.country),
                    ("scope[]", "basic"),
                    ("category_id", str(category)),
                    ("limit", str(min(SEARCH_PAGE, limit - len(collected)))),
                    ("page", str(page)),
                ],
            )
            if "_http_error" in batch:
                break
            found = batch.get("data", {}).get("elements", []) or batch.get("data", {}).get(
                "product_list", []
            )
            if not found:
                break
            collected.extend(found)
            page += 1
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

    def _hydrate(self, elements: list[dict]) -> list[VendorPart]:
        """Attach parameters and pricing to identity records."""
        symbols = [e["symbol"] for e in elements if e.get("symbol")]
        params: dict[str, list[tuple[str, str]]] = {}
        pricing: dict[str, dict] = {}

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
        for e in elements:
            sym = e.get("symbol")
            if not sym:
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
