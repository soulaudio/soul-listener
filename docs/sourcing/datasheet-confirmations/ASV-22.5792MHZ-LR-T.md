# ASV-22.5792MHZ-LR-T — datasheet confirmation

**Confirmed:** 2026-11-24, from Abracon's own datasheet (`abracon.com/Oscillators/ASV.pdf`,
retrieved with Playwright/curl) plus the TME API record.

## Why this part needed confirming

The selection filter rejected it with *"no `part_type` published"* while TME held **17,000 units** —
the only deeply-stocked candidate in the category. A rejection on a missing attribute is correct
behaviour for an automated filter and expensive to ignore, so it is exactly the case the report is
meant to surface for a human.

## What it is

> *"3.3Vdc HCMOS SMD Crystal Clock Oscillator — ASV SERIES"*

An **active clock oscillator (XO)** with an HCMOS output driver — **not** a passive crystal. The
filter's rejection was a false negative: the part is the right class, TME simply does not publish
`Type of quartz` for it.

## Part-number decode

`ASV-22.5792MHZ-LR-T`, against the datasheet's option table:

| field | code | meaning |
|---|---|---|
| Vdd | *(blank)* | **3.3 V ±10%** → 2.97–3.63 V |
| frequency | `22.5792MHz` | in the 0.312–200 MHz range |
| operating temp | `L` | **−40 °C … +85 °C** (industrial) |
| freq. stability | `R` | **±25 ppm** |
| output load | *(blank)* | 15 pF |
| symmetry | *(blank)* | 40/60 % @ ½Vdd |
| packaging | `T` | **1000 pcs/reel** |

TME's own parameters corroborate the decode independently: `frequency_stability_ppm: "±25ppm"`,
`operating_temp_c: "-40...85°C"`, `series: "ASV"`.

## Against the spec's constraints

| constraint | value | verdict |
|---|---|---|
| `part_type` ∈ {oscillator, XO} | HCMOS clock oscillator | **pass** (datasheet, not catalogue) |
| `frequency_hz` = 22.5792 MHz | 22.5792 MHz | pass |
| `supply_mv_min` ≤ 3300 | 2970 | pass |
| `supply_mv_max` ≥ 3300 | 3630 | pass |
| `mounting` = SMD | SMD, 7.0 × 5.08 × 1.8 mm | pass — and easily hand-soldered |
| `phase_noise_dbc_1khz` ≤ −130 | **3.2 ps RMS period jitter** | **see below** |

## The two findings that matter

**1. Reel-only: MOQ 1000, order multiple 1000, €0.70 each → ~€700 minimum.** Not a defect, but it
means "17,000 in stock" does not mean "buy one". Measured across all 24 parts TME carries at this
frequency, **every one is reel-only** (1000–3000) — there is no singly-orderable 22.5792 MHz
oscillator at TME at all.

**2. Jitter is mid-grade, and this design's whole premise is jitter.** The datasheet gives
**3.2 ps RMS period jitter** for the 15.0–29.9 MHz band that contains 22.5792 MHz. The spec's own
reasoning is that "jitter in the MCLK path is the single thing the dual-oscillator design exists to
avoid", and dedicated low-phase-noise audio oscillators are roughly an order of magnitude better.
So this part is **functionally correct and not audiophile-grade**.

That is a design decision, not a filter outcome: accept a mid-grade clock, or spend more on a
low-phase-noise part and source it from a distributor that breaks reels.

## Consequences already applied

- `frequency_hz` tolerance widened from `1e-6` to `1e-4`. TME publishes this part's frequency as
  `"22.579MHz"` — **truncated to five significant figures** — which is 8.86e-6 away from the real
  value and was rejected. 1e-4 is still 200× tighter than the nearest frequency anyone could
  confuse it with (22.1184 MHz, 2.0e-2 away).
- A hard MOQ constraint was tried and **removed**: it rejected the entire category, and
  `the_oscillator_category_yields_candidates_and_explains_the_rest` caught it immediately. A reel is
  expensive, not impossible.

---

# Validation sweep of the remaining candidates — 2026-11-24

Primary sources, Playwright where sites refuse a plain GET.

## Confirmed

**TI `SN74LVC1G3157DCKR`** (mux Alternate, 6916 in stock) — datasheet SCES424O:
*"Specified break-before-make switching"* as a front-page feature, with a `T_B-M` **Break before
make time** parameter in four timing tables and a dedicated *"Figure 6-10. Break-Before-Make
Internal Timing"*. Operating frequency **typically 340 MHz** (14× the 24.576 MHz required).
**1.65–5.5 V VCC**, matching TME's catalogue value exactly. SPDT. Ron **7–9 Ω max** at VCC = 3 V,
25 °C.

**Nexperia `74LVC1G3157GW`** (mux Preferred, not carried by TME) — product page retrieved with a
*headed* browser, since the site 403s a plain GET: *"Break-before-make switching"*, Ron
**6.5 Ω typical at VCC = 3.3 V** and 7.5 Ω at 2.7 V. The spec's "6–7.5 Ω Ron" claim is accurate.

**Both mux candidates satisfy the `Evidence::Datasheet` constraints.** Note that TI publishes
*maximums* where Nexperia publishes *typicals*, so Ron does not actually separate them — they are
the same class. The difference that matters is availability.

**Abracon `ASDMB` series** (the spec's former placeholder for *both* oscillator requirements) —
datasheet `abracon.com/Oscillators/ASDMB.pdf`. A **MEMS** clock oscillator:

| | ASDMB (MEMS) | ASV (quartz) |
|---|---|---|
| period jitter RMS | **5–10 ps** | **3.2 ps** |
| operating temperature | **0…+70 °C** | −40…+85 °C |
| supply | 1.8–3.3 V (no margin) | 2.97–3.63 V |

Worse on every axis that matters in a design whose stated premise is that MCLK jitter is the one
thing to avoid — and a 0 °C floor is a real limitation on a *portable* device. **Rejected.**

## The availability finding

| family | active oscillators at TME | stocked |
|---|---|---|
| 22.5792 MHz | 24 parts swept | **one** (ASV, 17000) — reel-only, MOQ 1000 |
| 24.576 MHz | 40 parts swept, 8+ active | **none**; only passive crystals are stocked |

**TME alone cannot supply this clock tree.** The dual-oscillator design needs both frequencies, so
fetching Mouser and Digi-Key is a prerequisite rather than an improvement.

The one bright spot: **Kyocera AVX `KC2016Z` is a matched pair** — `KC2016Z22.5792C1KX00` and
`KC2016Z24.5760C1KX00` are the same series, package, supply, temperature range and ±20 ppm
stability, differing only in frequency. That is precisely what a dual-oscillator design wants, and
no other series TME carries covers both families. Both are zero-stock with MOQ 2000, and the
2.0 × 1.6 mm package is markedly harder to hand-place than the ASV's 7.0 × 5.08 mm.

## Could NOT be validated

- **Mouser / Digi-Key stock for `CS43131-CNZ`** (snapshot claims 141 / 401 from manual research).
  Both sites return HTTP 403 to Playwright even headed; Digi-Key sits behind a Cloudflare
  interstitial. Scraping them further would be fragile and contrary to their terms — **the API is
  the supported path**, which is one more reason those two keys matter.
- **Akyga `AKY-LP623255`** — no reachable product page found (404 on the paths tried), so the cell's
  dimensions, integrated PCM and per-model certification documents remain unconfirmed.
