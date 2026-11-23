# Sourcing pipeline — setup and run

Everything about turning a requirement in `dap/hw-spec` into a ranked list of orderable parts.

| file | what it is |
|---|---|
| `.env.example` (repo root) | committed template of credential variable names |
| `.env` (repo root) | **your** credentials — gitignored, never committed |
| [`DISTRIBUTOR-API-SETUP.md`](DISTRIBUTOR-API-SETUP.md) | how to obtain each credential, per distributor |
| `hardware/bom/snapshot.json` | the committed sourcing snapshot — **what CI reads** |
| `hardware/bom/catalogue.fixture.json` | synthetic catalogue for testing the filter |

## 1. Fill in `.env`

`.env` already exists and is gitignored. Open it and paste the five values:

```sh
TME_TOKEN=              # developers.tme.eu -> API Token keys (50 chars)
TME_APP_SECRET=         #   same screen (20 chars)
MOUSER_SEARCH_API_KEY=  # emailed after the Search API Request Form
DIGIKEY_CLIENT_ID=      # developer.digikey.com -> Organization -> Production Apps
DIGIKEY_CLIENT_SECRET=  #   same screen
```

Leave `DIGIKEY_API_HOST=sandbox-api.digikey.com` for the first run. The sandbox flow is identical
and spends no production quota, which is what you want while the field mapping is unproven.

Getting those five values is the only manual step in this pipeline, and it takes about ten minutes.
See [`DISTRIBUTOR-API-SETUP.md`](DISTRIBUTOR-API-SETUP.md) for the click-path and why it cannot be
automated (all three require accepting a supplier agreement in your name and clearing an email
loop — a browser can click the buttons but cannot be the party to the contract).

## 2. Load it

Nothing reads `.env` implicitly. Export it for the shell that runs the fetch:

```sh
set -a && . ./.env && set +a
```

To check without printing the secrets:

```sh
for v in TME_TOKEN TME_APP_SECRET MOUSER_SEARCH_API_KEY DIGIKEY_CLIENT_ID DIGIKEY_CLIENT_SECRET; do
  printf '%-24s %s\n' "$v" "$([ -n "${!v}" ] && echo "set (${#:-0} chars)" || echo MISSING)"
done
```

## 3. What runs offline, today, with no credentials

The filter and rank stages are complete and tested against the fixture catalogue. These work now:

```sh
cd dap
cargo test -p dap-hw-spec            # constraints, value parsing, ranking
cargo test -p dap-arch-tests         # the pipeline end to end on the fixture
cargo test -p dap-arch-tests --test selection -- --nocapture   # the rejection report
```

That last command is the useful one. It prints, per requirement, which parts qualify and **why each
rejected one failed** — e.g. a 24.576 MHz oscillator being rejected for the 44.1 kHz family even
though the DAC accepts it, because the SAI cannot divide it to an exact 64 × Fs bit clock.

## 4. What the credentials unlock

The one missing stage is fetching a real catalogue. The order of work once `.env` is filled:

1. **One request per vendor, for a part we already know.** `CS43131-CNZ` is the right probe: it is
   already in `snapshot.json` with hand-researched stock figures, so the response can be checked
   against something rather than merely parsed.
2. **Commit the response as a fixture** under `hardware/bom/responses/`, redacted of anything
   account-specific. This is deliberate: Mouser's Swagger spec is not retrievable unauthenticated
   (it 302s into a geo-context setter), and transcribing field names from prose produces a parser
   that looks right and silently maps nothing.
3. **Write the per-vendor field mapping against that fixture**, with a test.
4. **Then** run it across the categories and commit the new snapshot.

Step 3 is small because the hard half is already done. Vendors publish parametric values as human
strings, and `dap_hw_spec::parametric` already turns those into the typed attributes the constraints
read — `"1.8 V ~ 3.3 V"` → `supply_mv_max: 3300`, `"22.5792MHz"` → `frequency_hz: 22579200` — and
refuses the ambiguous cases rather than guessing. A bare `"3.3"` returns `None`, because a number
with no unit cannot be told from the same number in another unit, and guessing is how a 5 V
oscillator reaches a 3.3 V board.

## Rules

- **CI never needs credentials.** It reads the committed snapshot. A gate that calls a live API
  gives a different answer on every run and fails when someone rate-limits it. (This is not
  hypothetical: `hardware.yml` is currently broken because atopile's component picker calls a remote
  parts API during the build.)
- **Re-retrieval is a deliberate act.** The fetcher writes `snapshot.json`, and the change arrives
  as a reviewable diff rather than as a silent shift in what the gates believe.
- **Both free tiers are 1,000 requests/day.** Enough to snapshot several categories, not enough for
  exploratory looping. Fetch, commit, then work offline against the snapshot.
- **`.env` stays out of git.** Enforced by `dap-arch-tests/tests/credentials_hygiene.rs`, which also
  checks that the template lists exactly the variables the setup doc names — so a variable cannot be
  added to one and forgotten in the other. That gate never reads `.env` itself.
