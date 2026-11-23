# Distributor API credentials — what to do, and why it can't be automated

**Status:** blocked on account creation. Everything downstream of the fetch is built and tested.
**Updated:** 2026-11-23

## Why this is a manual step

All three distributors gate API access behind an account created by a person, and in each case the
account creation involves accepting a contract and clearing an email loop:

| | account(s) needed | contract | email loop |
|---|---|---|---|
| **TME** | customer account at `tme.eu` **and** developer account at `developers.tme.eu` | "read and accept the TME API Terms and Conditions" during registration | activation link sent to the registration address |
| **Mouser** | Search API Request Form | API terms on the request form | the key is **emailed** to you |
| **Digi-Key** | developer account at `developer.digikey.com` | developer terms at registration | account verification |

Browser automation does not help here. The blocking steps are *agreeing to terms in your name* and
*receiving mail at your address* — neither is a UI problem. Driving a headless browser through them
would mean entering into three supplier agreements on your behalf, which is not a decision to
automate, and several of these sites forbid automated registration in the very terms being accepted.

So the division is: **you create the accounts (about ten minutes total), and nothing else about this
pipeline needs you.** The fetcher reads credentials from the environment, and everything it feeds is
already built and under test.

## TME — two accounts, in this order

> An earlier version of this section described the **v1** flow: create the application first, treat
> the customer-account link as an optional upgrade for personalised pricing, and sign requests with
> HMAC. All three of those are wrong for v2, and the error is not cosmetic — following that order
> leaves you staring at a "Generate new private key" panel demanding a temporary token you have no
> way to produce. Corrected below against TME's own v2 OpenAPI document
> (`https://developers.tme.eu/openapi.json`) and verified against the live endpoint.

**Both accounts are mandatory**, and the customer account comes first:

1. **Customer account** at `https://www.tme.eu` — register and get it *activated*. A new company
   account may wait on manual verification, and note the storefront you register on binds the
   account to that country's TME subsidiary (see the delivery-country note below).
2. **Developer account** at `https://developers.tme.eu` — sign up, accept the API Terms, click the
   email activation link.
3. Back at **tme.eu** → User Panel → **Applications** → **Register new application**. This generates
   a **temporary token**, whose only purpose is to prove your customer account to the developer
   portal.
4. At **developers.tme.eu** → create an application → **paste the temporary token** → state the key
   purpose → **Generate new private key** → **Show private keys**.

You end with a **50-character token** and a **20-character application secret**, both hex.

There is **no anonymous mode in v2**. `request-context: anonymous` was a v1 feature; it appears
nowhere in the v2 specification, which is why the customer account cannot be skipped.

### Auth is OAuth 2.0 client-credentials, not HMAC

The token is the Basic-auth *username* and the secret is the *password*; that pair buys a short-lived
bearer token:

```sh
curl -s -X POST 'https://api.tme.eu/auth/token' \
  -u "$TME_TOKEN:$TME_APP_SECRET" \
  -H 'Content-Type: application/x-www-form-urlencoded' \
  --data-urlencode 'grant_type=client_credentials'
# -> {"access_token":"…","token_type":"Bearer","expires_in":300,"refresh_token":"…"}
```

`expires_in` is **300 seconds**, so a run that fetches more than a few categories has to refresh
mid-run (`grant_type=refresh_token`) rather than authenticate once at startup.

### The endpoints the pipeline needs

Four, because TME splits identity, parameters and pricing. Verified against the live API:

| endpoint | gives | required params |
|---|---|---|
| `GET /products/search` | symbols matching a phrase and/or category | `scope[]`, and `phrase` **or** `category_id` |
| `GET /products` | identity: `symbol`, `ean`, `manufacturer`, `description`, `multiples`, `minimal_amount` | `symbols[]` **or** `mpns[]` |
| `GET /products/parameters` | the parametric attributes the selection constraints consume | `symbols[]` |
| `GET /products/data` | **prices, stock levels, delivery** — max **50 symbols per call** | `scope[]`, `symbols[]`, `amounts[]` |

Two things worth knowing before writing the mapping:

- **`/products` accepts `mpns[]`.** We can look parts up by manufacturer part number directly
  instead of first resolving TME's own `symbol`, which removes a whole lookup stage.
- **The spec's `required` flags are not reliable.** `/products/search` marks both `phrase` and
  `category_id` required while its prose says "provide either … or both", and `/products` marks both
  `symbols[]` and `mpns[]` required when they are alternatives. Trust the prose.

```sh
export TME_TOKEN=...      # 50 chars, hex
export TME_APP_SECRET=... # 20 chars, hex
```

### Delivery country: the SK/CZ trap

TME is not one company. The group runs separate national subsidiaries — TME Czech Republic s.r.o.
and TME Slovakia s.r.o. — and each "is responsible for issuing invoices with VAT rates applicable"
in its own country, while the parent in Łódź holds the warehouse and ships everything **from
Poland**. So an account registered on the Slovak storefront belongs to TME Slovakia and will not
accept a Czech delivery address: the constraint is the invoicing entity, not logistics. Decide the
delivery country before registering, because it is baked into which subsidiary holds the account.

## Mouser — Search API

1. Complete the **Search API Request Form** (linked from `mouser.com/api-search/`).
2. The key arrives by email.

Free tier: **1,000 requests/day, 30 requests/minute.** The key is passed in the request body or as
`apiKey`, depending on endpoint.

```sh
export MOUSER_SEARCH_API_KEY=...
```

## Digi-Key — `developer.digikey.com`

1. Register a free developer account.
2. **Organization** → **Production Apps** → **Create Production App**.
3. Copy the **Client ID** and **Client Secret**.

Auth is **OAuth 2.0, two-legged** (client-credentials): exchange the ID and secret for a short-lived
bearer token, then call the product search endpoints with it. There is a sandbox at
`sandbox-api.digikey.com` with the same flow, which is the right place to shake out the field mapping
before spending production quota.

Free tier: **1,000 requests/day, 120 requests/minute.**

```sh
export DIGIKEY_CLIENT_ID=...
export DIGIKEY_CLIENT_SECRET=...
# Which host to call. Start on the sandbox — identical flow, no production quota
# spent while the per-vendor field mapping is still unproven.
export DIGIKEY_API_HOST=sandbox-api.digikey.com   # then api.digikey.com
```

## Rules for handling these

- **Never commit them.** They go in the environment, or in a file outside the repo. There is no
  credential file in this tree and there should not be one.
- **CI never needs them.** The committed `hardware/bom/snapshot.json` is what CI reads; the fetcher
  runs locally and its output lands in a reviewable diff. This split is deliberate and is why
  `sourcing_snapshot.rs` reads a file rather than calling an API — a gate that queries a live API
  gives a different answer on every run. (It is also why `hardware.yml` is currently broken:
  atopile's component picker calls a remote parts API during the build.)
- **The rate limits are per day, not per minute, in practice.** 1,000 requests/day is plenty for
  snapshotting a few categories but not for exploratory looping — fetch a category, write the
  snapshot, then work offline against it.

## What is already built

| stage | state |
|---|---|
| vendor field plumbing (which JSON key is the MPN) | **needs one real response per vendor** |
| value parsing — units, ranges, booleans | done: `dap-hw-spec::parametric`, 12 tests |
| filter with reasons | done: `dap-hw-spec::select`, 8 tests |
| rank with stated policy | done |
| candidates → spec → firmware gates | done |

The remaining per-vendor work is field renaming, and it should be written against a **recorded**
response rather than from documentation. Mouser's Swagger spec is not retrievable unauthenticated
(it 302s into a geo-context setter), and transcribing field names from prose is exactly the kind of
guess that produces a parser which looks right and silently maps nothing.

So the sequence once credentials exist:

1. Make one request per vendor for a part we already know (`CS43131-CNZ` is a good probe — it is in
   the snapshot with hand-researched stock figures, so the response can be checked against something).
2. Commit the response as a fixture under `hardware/bom/responses/`, redacted of anything
   account-specific.
3. Write the field mapping against that fixture, with a test.
4. Only then run it across the categories.

`parametric.rs` is the piece that makes step 3 small: the vendor hands over `"3.3 V"` or
`"1.8 V ~ 3.3 V"` or `"22.5792MHz"`, and turning those into `supply_mv_max: 3300` and
`frequency_hz: 22579200` is already done and tested — including refusing a bare `"3.3"`, because a
number with no unit cannot be told from the same number in a different unit, and guessing is how a
5 V oscillator reaches a 3.3 V board.

## Sources

- [TME API documentation v2](https://developers.tme.eu/en/api-doc/v2) ·
  [registration instructions (PDF)](https://developers.tme.eu/pdfs/en/instruction.pdf) ·
  [terms of service](https://developers.tme.eu/en/rules)
- [Mouser Search API](https://www.mouser.com/en/api-search/)
- [Digi-Key OAuth 2.0 two-legged flow](https://developer.digikey.com/tutorials-and-resources/oauth-20-2-legged-flow)
