#!/usr/bin/env python3
"""Fetch real TME data for the parts the hardware spec names, and record it.

Run from the repo root:

    python3 tools/bom-fetch-tme.py            # fetch + write raw responses
    python3 tools/bom-fetch-tme.py --dry-run  # show what it would ask for

Credentials come from `.env` (gitignored). Nothing is printed that could leak
them: the bearer token is never echoed, and the Basic-auth header is built
in-process.

# What this is and is not

It is the *I/O* half of the fetch. It authenticates, asks TME for the parts the
committed snapshot already names, and writes the **raw** JSON responses to
`hardware/bom/responses/tme/`. It does not interpret units or decide anything —
that is `dap_hw_spec::parametric` and `dap_hw_spec::select`, which are Rust,
`no_std`, and covered by the same test run as the constraints they serve.

Keeping the split here is deliberate. The HTTP call is trivial and untestable
without credentials; the interpretation is where sourcing pipelines actually go
wrong and it is testable offline. So this script's output is a reviewable
artefact, not an input to a decision.

# Why raw responses are committed

So the field mapping can be written and tested against something reproducible.
TME's own `required` flags are unreliable (`/products` marks both `symbols[]` and
`mpns[]` required when they are alternatives), so a mapping written from the
specification alone is a guess. A recorded response is not.

# API notes that shaped this script

* v2 is OAuth 2.0 client-credentials, not HMAC. The 50-char token is the
  Basic-auth username, the 20-char secret the password.
* Access tokens live **300 seconds**. A multi-category run must refresh; this
  script re-authenticates when the token is within 30 s of expiry.
* `/products/data` accepts at most **50 symbols** per call.
* `/products` accepts `mpns[]`, so parts resolve by manufacturer part number
  without first looking up TME's own `symbol`.
"""

from __future__ import annotations

import base64
import json
import pathlib
import sys
import time
import urllib.error
import urllib.parse
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parent.parent
SNAPSHOT = ROOT / "hardware/bom/snapshot.json"
OUT_DIR = ROOT / "hardware/bom/responses/tme"
BASE = "https://api.tme.eu"

# TME ships from Poland but invoices through the national subsidiary the account
# belongs to; the country here selects pricing/availability presentation, not the
# warehouse. See docs/sourcing/DISTRIBUTOR-API-SETUP.md.
COUNTRY = "SK"
CURRENCY = "EUR"

# /products/data caps at 50 symbols per call.
SYMBOL_BATCH = 50
TOKEN_REFRESH_MARGIN_S = 30


def load_env() -> dict[str, str]:
    env_path = ROOT / ".env"
    if not env_path.exists():
        sys.exit("no .env at the repo root — see docs/sourcing/README.md")
    out: dict[str, str] = {}
    for line in env_path.read_text().splitlines():
        s = line.strip()
        if s and not s.startswith("#") and "=" in s:
            k, v = s.split("=", 1)
            out[k.strip()] = v.strip()
    return out


class Tme:
    """A thin client. Holds the bearer token and refreshes it before expiry."""

    def __init__(self, token: str, secret: str) -> None:
        self._basic = base64.b64encode(f"{token}:{secret}".encode()).decode()
        self._bearer: str | None = None
        self._expires_at = 0.0
        self.calls = 0

    def _authenticate(self) -> None:
        body = urllib.parse.urlencode({"grant_type": "client_credentials"}).encode()
        req = urllib.request.Request(
            f"{BASE}/auth/token",
            data=body,
            method="POST",
            headers={
                "Authorization": f"Basic {self._basic}",
                "Content-Type": "application/x-www-form-urlencoded",
            },
        )
        with urllib.request.urlopen(req, timeout=30) as r:
            d = json.loads(r.read())
        self._bearer = d["access_token"]
        # Trust the server's lifetime rather than assuming 300.
        self._expires_at = time.monotonic() + float(d.get("expires_in", 300))
        self.calls += 1

    def _auth_header(self) -> str:
        if self._bearer is None or time.monotonic() > self._expires_at - TOKEN_REFRESH_MARGIN_S:
            self._authenticate()
        assert self._bearer is not None
        return f"Bearer {self._bearer}"

    def get(self, path: str, params: list[tuple[str, str]]) -> dict:
        """GET with repeated query keys (`symbols[]=a&symbols[]=b`)."""
        url = f"{BASE}{path}?{urllib.parse.urlencode(params)}"
        req = urllib.request.Request(url, headers={"Authorization": self._auth_header()})
        self.calls += 1
        try:
            with urllib.request.urlopen(req, timeout=45) as r:
                return json.loads(r.read())
        except urllib.error.HTTPError as e:
            detail = e.read().decode(errors="replace")[:400]
            # Reported, not raised: a 404 for a part TME does not stock is an
            # ANSWER (it tells us the part is unsourceable there), not a failure.
            return {"_http_error": e.code, "_detail": detail}


def chunks(xs: list[str], n: int):
    for i in range(0, len(xs), n):
        yield xs[i : i + n]


def main() -> int:
    dry = "--dry-run" in sys.argv
    mpns = sorted(json.loads(SNAPSHOT.read_text())["parts"].keys())

    print(f"parts named by the snapshot: {len(mpns)}")
    for m in mpns:
        print(f"  {m}")
    if dry:
        print("\n--dry-run: would call /products (mpns[]), then /products/parameters")
        print("and /products/data (scope prices+stock) for whichever resolve.")
        return 0

    env = load_env()
    if not env.get("TME_TOKEN") or not env.get("TME_APP_SECRET"):
        sys.exit("TME_TOKEN / TME_APP_SECRET are empty in .env")

    api = Tme(env["TME_TOKEN"], env["TME_APP_SECRET"])
    OUT_DIR.mkdir(parents=True, exist_ok=True)

    # 1. Resolve MPNs to TME symbols.
    products = api.get(
        "/products",
        [("country", COUNTRY)] + [("mpns[]", m) for m in mpns],
    )
    (OUT_DIR / "products.json").write_text(json.dumps(products, indent=2) + "\n")

    if "_http_error" in products:
        print(f"\n/products failed: HTTP {products['_http_error']}")
        print(products["_detail"])
        return 1

    elements = products.get("data", {}).get("elements", [])
    symbols = [e["symbol"] for e in elements if e.get("symbol")]
    print(f"\nresolved {len(symbols)} of {len(mpns)} MPNs to TME symbols")

    found_mpns = {
        (e.get("manufacturer_symbols") or [None])[0] or e.get("symbol"): e.get("symbol")
        for e in elements
    }
    for m in mpns:
        hit = next(
            (
                e["symbol"]
                for e in elements
                if m.upper() in json.dumps(e).upper()
            ),
            None,
        )
        print(f"  {m:<26} {'-> ' + hit if hit else 'NOT STOCKED AT TME'}")

    if not symbols:
        print("\nnothing resolved; no further calls made")
        return 0

    # 2. Parametric attributes — what the selection constraints consume.
    for i, batch in enumerate(chunks(symbols, SYMBOL_BATCH)):
        params = api.get(
            "/products/parameters",
            [("country", COUNTRY)] + [("symbols[]", s) for s in batch],
        )
        (OUT_DIR / f"parameters-{i}.json").write_text(json.dumps(params, indent=2) + "\n")

    # 3. Prices and stock.
    #
    # NO `amounts[]` here, despite the specification marking it `required: true`.
    # Its own prose says the opposite — "if scope[] includes prices or stock,
    # only symbols[] is required" and amounts[] "is mandatory when requesting
    # data within the delivery ... scope" — and sending it anyway returns
    # HTTP 400 E_INPUT_PARAMS_VALIDATION_ERROR: "product_data_accessor: This
    # form should not contain extra fields."
    #
    # Third case of this API's required-flags disagreeing with its prose, and the
    # first where believing the flag breaks the call outright. Trust the prose.
    for i, batch in enumerate(chunks(symbols, SYMBOL_BATCH)):
        data = api.get(
            "/products/data",
            [("country", COUNTRY), ("currency", CURRENCY)]
            + [("scope[]", "prices"), ("scope[]", "stock")]
            + [("symbols[]", s) for s in batch],
        )
        (OUT_DIR / f"data-{i}.json").write_text(json.dumps(data, indent=2) + "\n")

    print(f"\nwrote raw responses to {OUT_DIR.relative_to(ROOT)}")
    print(f"API calls used: {api.calls}")
    print("\nNEXT: review the raw files for anything account-specific before committing.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
