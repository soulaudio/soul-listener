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

## 3b. TME is verified working

The TME pair has been tested against the live `/auth/token` endpoint: it returns a Bearer token
(300-second expiry, with a refresh token), and an authenticated `/products` call succeeds. So the
TME half of the pipeline is credential-ready.

Note v2 is **OAuth 2.0 client-credentials, not HMAC**, and an activated tme.eu *customer* account is
mandatory — there is no anonymous mode. See
[`DISTRIBUTOR-API-SETUP.md`](DISTRIBUTOR-API-SETUP.md) for the corrected flow and the four endpoints
the fetch needs (`/products/search`, `/products`, `/products/parameters`, `/products/data`).

## 4. What the credentials unlock

All three distributors are implemented and fetched. The order of work below is the sequence each one
followed, kept because it is what a fourth distributor should do — and because step 1 is the step
that caught a fabricated number in the spec:

1. **One request per vendor, for a part we already know.** `CS43131-CNZ` was the probe: it was
   already in `snapshot.json` with hand-researched stock figures, so the response could be checked
   against something rather than merely parsed. Worth doing — Mouser's real figure is **0 stock,
   reel-only at MOQ 490**, not the `141` the snapshot claimed, and that one number was the whole
   basis of the preferred DAC's two-source compliance.
2. **Commit the response as a fixture** under `hardware/bom/responses/`, redacted of anything
   account-specific. This is deliberate: Mouser's Swagger spec is not retrievable unauthenticated
   (it 302s into a geo-context setter), and transcribing field names from prose produces a parser
   that looks right and silently maps nothing.
3. **Write the per-vendor field mapping against that fixture**, with a test.
4. **Then** run it across the categories and commit the new snapshot — except where a vendor
   publishes no parametric fields. Mouser does not (see `DISTRIBUTOR-API-SETUP.md`), so it runs the
   named-lookup path only: a part discovered solely at Mouser has nothing for a constraint to judge,
   and Mouser has the tightest daily budget of the three.

Step 3 is small because the hard half is already done. Vendors publish parametric values as human
strings, and `dap_hw_spec::parametric` already turns those into the typed attributes the constraints
read — `"1.8 V ~ 3.3 V"` → `supply_mv_max: 3300`, `"22.5792MHz"` → `frequency_hz: 22579200` — and
refuses the ambiguous cases rather than guessing. A bare `"3.3"` returns `None`, because a number
with no unit cannot be told from the same number in another unit, and guessing is how a 5 V
oscillator reaches a 3.3 V board.

## Commands

```sh
tools/sourcing-run.py refresh                          # EVERYTHING: fetch, export, snapshot
tools/sourcing-run.py plan                             # how big each sweep's category really is
tools/sourcing-run.py selftest                         # pure helpers, no network
tools/sourcing-run.py fetch  --vendor tme              # the MPNs the snapshot names
tools/sourcing-run.py fetch  --vendor tme --category 100269 \
     --phrase 22.5792MHz --as mclk-osc-44k1 --limit 60 # a category sweep
tools/sourcing-run.py export --category mclk-osc-44k1  # -> candidates/<id>.json
tools/sourcing-run.py status                           # what the DB holds

cargo run -p dap-arch-tests --example sourcing-report  # what the FILTER makes of it
cargo run -p dap-arch-tests --example propose          # ranked proposals, with links
tools/sourcing-run.py sweep --vendor digikey           # one vendor's sweeps, nothing else
```

### `propose` — the shortlist you pick from

Writes `hardware/bom/PROPOSALS.md` (read this), `proposals.json` (the decision record,
including the alternatives) and `selected.json` (the handoff: what is actually picked).

Ranked by the spec's own `DEFAULT_RANKING`, with a clickable distributor link per row, and
parts the spec has already **ruled out** marked as such with the reason — a two-source socket
with 73 000 in stock and no card-detect switch otherwise keeps getting re-offered, because
the missing switch is datasheet evidence the filter cannot see.

Picking a part, and how it reaches firmware and the board, is
[`PICKING-AND-HANDOFF.md`](PICKING-AND-HANDOFF.md).

### `plan` — size a sweep before running it

Every `SWEEPS` limit is a number someone chose, and a limit means nothing without the size of the
thing it limits. `plan` probes each declared sweep's first page and prints the vendor's own total,
the coverage that limit achieves, and what exhausting the rest would cost against today's remaining
budget.

It was worth writing immediately. Two limits turned out to be badly wrong in opposite directions:
Digi-Key's `"lithium polymer battery pack"` matches **2 products in the entire catalogue**, while
`"24.576MHz oscillator"` matches **6110**. Neither was visible before.

Two measured facts shape how the sweeps are now written:

- **Digi-Key ranks by relevance**, so a capped sweep keeps the most relevant N rather than an
  arbitrary N — `SN74LVC1G3157DCKR` is the first result for `"SPDT analog switch SC70"`. What a
  relevance engine does badly is breadth of *phrasing*, so several angles per requirement are
  declared and unioned (deduplication is free: `parts` is UNIQUE on `(vendor, vendor_pn)`).
- **Digi-Key keyword search is not conjunctive.** Adding words can collapse the result set to a
  near-exact phrase match: `"24.576MHz standard clock oscillator"` returns 5. A longer query is not
  a safely narrower one, so every phrase in `SWEEPS` carries its measured total in a comment.

### `sourcing-report` — what the constraints actually do

The arch tests answer "is anything broken", and a green suite says nothing about whether the filter
is doing real work. Two failure modes are invisible to it: a constraint that rejects **everything**
(a hard `moq <= 100` once eliminated the entire oscillator category) and a constraint that rejects
**nothing**, which is decoration that looks identical to a good one.

So the report prints, per requirement: candidates swept, how many qualify, a histogram of *what did
the rejecting*, whether the spec's own preferred and alternate parts survive their own constraints,
and the ranked head by `DEFAULT_RANKING`. It flags zero-pass and all-pass requirements as anomalies.

The histogram distinguishes `not published by any vendor` from `out of range`, because the first is
a catalogue **coverage** gap and the second is a real rejection — conflating them makes a data
problem look like a market problem.

It shares `dap_arch_tests::candidates` with the gates on purpose. That loader used to be copied into
every test file that needed it, and a fourth copy was about to go into this report, where it would
have been free to disagree: a report claiming a part qualifies while a gate rejects it is worse than
no report, because nobody sees the disagreement until the part is ordered.

`refresh` is the one command that matters day to day. The category sweeps it performs are declared
in `SWEEPS` at the top of the script rather than remembered, so a refresh is reproducible and
"which categories is this conclusion based on?" has an answer. A vendor without credentials is
skipped and reported — a partial refresh is normal, not an error.

Vendor vocabulary is reconciled at export by `tools/sourcing/canon.py`: TME says
`mounting = 'SMD'` where Digi-Key says `'Surface Mount'` for the same part, and the spec names each
concept once. Conflicts that survive canonicalisation are **reported**, never silently resolved —
two vendors disagreeing about one component means one of them is wrong.

A sweep is **checked for completeness** against TME's own `counters.count`: if fewer parts come
back than the vendor says match, the run fails rather than reporting a partial category. An
incomplete sweep presented as a whole category is how "nothing is sourceable" gets claimed on
partial data.

Use a **phrase**, not the API's `parameters[]` filter, to narrow a category. That is counter-
intuitive and was measured: for 22.5792 MHz the phrase returns 24 parts and the parameter filter
returns 21, a strict subset — because the filter keys on exact value-ids and the one in-stock part
has its frequency recorded as the truncated `"22.579MHz"`. **The parameter filter would have missed
it.**

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
