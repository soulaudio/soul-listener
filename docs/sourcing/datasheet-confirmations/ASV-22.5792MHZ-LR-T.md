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
