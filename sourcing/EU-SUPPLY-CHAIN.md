# soul-listener — Full EU supply chain

**Date:** 2026-11-21 · **Buyer:** SK s.r.o., **delivery to Czechia** · **Prices:** Nov 2026, **net / ex-VAT**, EUR.
**Proposal:** source the entire project — **product + dev kit — inside the EU**, so every order is intra-EU **reverse-charge (0% VAT, net = cost)** with **zero customs, zero duty, zero courier handling**. Exactly **one** part cannot be EU-sourced (the Cirrus CS43131 DAC) — handled cleanly below.

> **Why all-EU matters now:** since **1 July 2026** the EU abolished the small-parcel duty-free threshold — *any* non-EU parcel now pays a **flat ~€3/item duty + a ~€10 courier clearance/handling fee** regardless of value. All-EU avoids that entirely (and is faster: 2–4 days vs 1–3 weeks).

---

## 1. Supplier roster — who, from where, how it's taxed

| Supplier | Ships from | Role | Tax for you (SK VAT, CZ delivery) |
|---|---|---|---|
| **TME** | Poland | connectors, passives, battery, buck, TVS | intra-EU **reverse-charge**, no customs |
| **Farnell / RS** | EU distribution centres | ICs + EU substitutes | **reverse-charge**, no customs |
| **Eckstein** | Germany | **e-ink panel** (Waveshare 3.97″ = GDEY0397T81P) | **reverse-charge**, no customs |
| **Botland / BerryBase** | Poland / Germany | **dev** breakouts + bench | **reverse-charge**, no customs |
| **Mouser / Digi-Key** | USA (DDP via Cologne/NL) | **CS43131 only** | **reverse-charge**, DDP → **no door charges** |

*VAT nuance:* billing an SK VAT ID while delivering to CZ is a standard intra-Community acquisition — if the company actually operates in CZ, it's cleanest to use a **CZ VAT registration** for CZ deliveries. Confirm with your accountant which ID to give at checkout; it nets to VAT-neutral either way.

---

## 2. Product BOM — EU sourcing (the shipped device)

Net qty-1 EUR. **"EU sub"** = an EU-stocked substitute for a design part that isn't EU-domestic — these are the choices that make the product ~all-EU (finalize at PCB design).

| Block | Part (EU sub noted) | Supplier | ~Net € | Note |
|---|---|---|---|---|
| **MCU** | STM32H7A3ZIT6 | **Farnell** | 13.80 | ✓ EU |
| **DAC + amp** | **Cirrus CS43131‑CNZ** | **Mouser / Digi‑Key** | 10.59 | ⚠️ the one non‑EU line (see §4) |
| **Charger** | **BQ24075** *(EU sub for BQ21080 — power‑path, QFN, better for hand build)* | **Farnell** | ~2.2 | EU sub |
| **Fuel gauge** | MAX17048 | **Farnell** | ~4 | ✓ EU |
| **Buck 3.3 V** | TLV62569DBVR | **TME** | 0.23 | ✓ EU |
| **Analog LDO** | low‑noise LDO (ADP7142‑class) *(EU sub for TPS7A2033)* | **Farnell** | ~1–2 | EU sub |
| **USB HS PHY** | USB3340‑EZK | **Farnell** | ~2 | ✓ EU |
| **Audio clocks ×2** | MEMS XO 22.5792 / 24.576 MHz (Abracon/IQD) *(EU sub for NDK)* | **Farnell** | ~0.7 ea | EU sub; NDK‑exact = Digi‑Key only |
| **Wheel encoder** | **AS5600‑ASOM** *(EU sub for MT6701)* + diametric magnet | **Farnell** | ~3–4 | EU sub (drops the China/LCSC line) |
| **USB‑C** | USB4085‑GF‑A | **TME** | 0.74 | ✓ EU |
| **3.5 mm jack** | SJ1‑3523N | **TME** | 0.81 | ✓ EU |
| **microSD socket** | DM3AT‑SF‑PEJM5 | **TME** | 2.89 | ✓ EU |
| **FPC connector** | FH12‑24S‑0.5SH(55) | **TME** | 0.65 | ✓ EU |
| **Battery connector** | JST B2B‑PH‑K‑S | **TME** | 0.10 | ✓ EU |
| **Battery** | Akyga AKY‑LP623255 (1100 mAh, PCM+JST) | **TME** | 4.59 | ✓ EU |
| **Side button** | C&K side tactile | **Farnell** | 1.23 | ✓ EU |
| **TVS + passives** | TPD4E05U06 + R/C/L lot | **TME** | ~8 | ✓ EU |
| **e‑ink panel** | **Waveshare 3.97″ "(G)" = GDEY0397T81P** | **Eckstein** | 29.74 | ✓ EU (same SSD1677/FPC‑7750 panel) |

**Product orders: 3 EU (TME + Farnell + Eckstein) + 1 small US line (CS43131) = 4.** **Zero customs** — the CS43131 ships DDP (no door charge); everything else is intra‑EU.

---

## 3. Dev kit — EU sourcing (firmware bring-up rig)

| Item | Part | Supplier | ~Net € |
|---|---|---|---|
| MCU dev board | **NUCLEO‑H743ZI2** *(common H7; build for `stm32h743zi`)* | maker shop / **RS** | ~27–30 |
| I²S DAC breakout | PCM5102 module | **Botland** | 4.90 |
| microSD breakout | full‑bus (SDMMC 4‑bit) | maker shop | ~3–7 |
| scroll wheel | EC11/PEC11‑class encoder + knob | maker shop | ~2–3 |
| bench | breadboard + jumpers + tactile buttons + 2× USB cables | maker shop | ~12–15 |
| e‑ink panel + driver board | Waveshare 3.97″ + Waveshare e‑paper driver board | **Eckstein / BerryBase** | ~45 |

**Dev orders: 1–2 EU (maker shop + Eckstein; 1 if one German shop carries it all) ≈ €95–115. Zero customs.**

---

## 4. The CS43131 — EU‑sourceable, just not from the catalog shops (2026-11-21)

The CS43131 isn't at the **catalog shops** that serve qty‑1 EU (TME, Farnell, RS don't carry Cirrus at all). But **Cirrus's actual EMEA franchise distributors DO stock it EU‑domestically** — per [franchised‑distributors.eu](https://www.franchised-distributors.eu/cirrus-logic/) / [ECIA](https://www.eciaauthorized.com/en/manufacturers/cirrus): **Avnet Silica** (EMEA warehouses) plus German/Dutch specialists **Azzurri Technology (Munich)**, **TOPAS Electronic (DE)**, **TOP‑electronics (NL)**. These are **B2B / quote‑based** (contact for stock, likely reel/MOQ — no slick qty‑1 web shop). So:

- **Qty‑1 prototype:** order from **Mouser / Digi‑Key** (US‑fulfilled but **DDP** — Digi‑Key imports via Cologne → intra‑community delivery → reverse‑charge, **no door customs/VAT**). ~€16, instant, one small line. The pragmatic choice now — "non‑EU" here is US‑*origin*, not a tax cost.
- **Production (or 100 % EU even for the proto):** source via **Avnet Silica** or **Azzurri / TOP‑electronics** → the CS43131 becomes **EU‑domestic too**, making the whole product **100 % EU**.

So the project is **100 % EU‑sourceable at volume**; the single "non‑EU line" is a *qty‑1 convenience* choice, not a hard constraint. (The only all‑EU *redesign* alternative — a 2‑chip PCM5102 + discrete amp — is ~112 dB + DSD‑via‑conversion, a real audio downgrade; **not recommended** over one DDP chip.)

**CS43131 is the best‑value, class‑leading, warm‑voiceable pick** — deep‑research‑backed (`docs/research/2026-11-18-dac-amp-selection.md`); keep it.

---

## 5. Summary

| | Orders | EU‑origin | Non‑EU | Customs |
|---|---|---|---|---|
| **Product** | 4 | TME + Farnell + Eckstein | 1 line: CS43131 (Mouser/DK, DDP) | **zero** |
| **Dev kit** | 1–2 | maker shop + Eckstein | — | **zero** |

Everything is **reverse‑charge, net = cost, zero customs/duty/handling.** The project is effectively an **EU supply chain with a single DDP line for the DAC.**

## 6. Open confirmations before ordering
1. **EU substitutions** (BQ24075 / AS5600 / MEMS XO / low‑noise LDO) are design choices — lock them when the PCB is designed (`SOURCING-EU-CZ-2026-11.md` §6c has the matrix).
2. **CZ‑vs‑SK VAT ID** for CZ deliveries — accountant.
3. **Panel LUT**: standardize on the Waveshare 3.97″ for dev + product so one waveform LUT applies (SSD1677 driver notes: internals spec §9).
4. **Build the exact carts at order time** — prices/stock move; validate the Eckstein + Botland + Farnell + TME carts fresh.

*Grounded in Playwright + web validation across this session (TME, Farnell/RS, Mouser SK, Botland, Eckstein, Good Display/OpenELAB, GxEPD2). Companion docs: `SOURCING-EU-CZ-2026-11.md` (product detail), `DEV-SUPPLY-EU-CZ-2026-11.md` (dev detail), `docs/research/2026-11-18-dac-amp-selection.md`, `docs/research/2026-11-18-licensing-ip-and-workarounds.md`.*
