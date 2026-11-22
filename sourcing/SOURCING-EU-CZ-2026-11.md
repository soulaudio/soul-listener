# soul-listener — EU / Czechia–Slovakia Sourcing Plan (qty 1 prototype)

**Date:** 2026-11-18 · **Prices valid:** November 2026 · **Currency:** EUR, **net / ex‑VAT**
**Scope:** Core shipped electronics (wired‑only product), **quantity 1** (one hand‑built prototype).
**Buyer profile:** **SK s.r.o.** (VAT‑registered Slovak company), **delivery to Czechia (CZ)**. Prices below are **net (ex‑VAT)**, the real cost after intra‑EU **reverse‑charge** (supplier charges 0% VAT against the SK IČ DPH; you self‑account) and **import‑VAT reclaim** on the China panel. Ship‑to CZ with bill‑to SK is a standard intra‑EU acquisition — reverse‑charge and the net prices are unchanged; see §6c for the one delivery nuance.
**Validation:** Every orderable line below was checked live in a browser (Playwright) on 2026‑11‑18 — stock + qty‑1 net price + URL captured per part. Status column: `YES` = exact part in stock & validated; `SUB` = design part unavailable, validated substitute; `EST` = estimate (not individually priced).

> **Excluded (per scope decision):** PCB fabrication, enclosure/fasteners, Bluetooth FSC‑BT1035 (DNP), and optional upgrades (LT3045, MAX17262, SJ1‑3523NG gold, Crystek clocks). Add on request.

---

## 1. Headline — money & orders

| | Design‑exact (Option A) | Fewest‑sites (Option B) |
|---|---|---|
| **Separate orders / invoices** | **4** | **3** |
| **Suppliers** | TME · Mouser · LCSC · Good Display | TME · Mouser · Good Display |
| **Parts subtotal (net)** | **€71** | **€72** |
| **Shipping (net, est.)** | **~€55** | **~€47** |
| **Grand total (net, ex‑VAT)** | **≈ €126** | **≈ €120** |

**Both land around €120–130 net for one complete prototype's worth of parts.** Note the shipping (~€47–55) is almost as large as the parts (~€73) — that is inherent to buying **qty‑1 across multiple vendors**, and is exactly why minimising order count matters here.

**Two easy levers that *lower* the total:**
- **Top the Mouser cart to €50 net → free shipping** (saves ~€20; the cart is ~€37–41, so ~€10–13 of spares/decoupling gets you over the line and *reduces* the grand total by ~€7).
- **Option B** (swap the MagnTek MT6701 for an ams **AS5600** carried by Mouser) **deletes the LCSC order entirely** → 3 orders, no China‑B2B customs on that line, ~€6–15 saved. Trade‑off: AS5600 has no Z‑index and no built‑in push (use the wheel‑click tactile already in the TME bundle for the press). The design doc itself lists AS5600 as the sanctioned alternative.

VAT footnote: the China panel incurs ~€8 import VAT (fully **reclaimable** by the s.r.o.) and **0% customs duty** (e‑paper displays, HS 8531.20, are duty‑free). Everything from TME/Mouser is reverse‑charged (no VAT cash‑flow).

---

## 2. Order 1 — TME.eu  (primary; broad catalogue, cheap SK shipping)

TME (Transfer Multisort, Poland) ships to Slovakia in 1–2 days, issues a proper EU **faktúra**, and reverse‑charges with a valid SK IČ DPH. It carries the bulk of the connectors, passives, the battery and the cheap commodity ICs. Storefront set to **Slovakia / EN / EUR / net**.

| Ref | Part | MPN | Qty | Net € | Stock | ✓ |
|---|---|---|---|---|---|---|
| J1 | USB‑C 2.0 receptacle, 16‑pin | **USB4085‑GF‑A** (GCT) | 1 | 0.74 | 10 619 | YES |
| J2 | 3.5 mm stereo jack | **SJ1‑3523N** (Same Sky) | 1 | 0.81 | 152 | YES |
| J3 | microSD push‑push socket | **DM3AT‑SF‑PEJM5** (Hirose) | 1 | 2.89 | 247 | YES |
| J5 | Battery header 2‑pin PH 2.0 | **B2B‑PH‑K‑S** (JST) | 1 | 0.10 | 87 601 | YES |
| U‑BUCK | Sync buck → 3.3 V | **TLV62569DBVR** (TI) | 1 | 0.23 | 505 | YES |
| J4 | FPC 24‑pin 0.5 mm ZIF | **FH12‑24S‑0.5SH(55)** (Hirose) | 1 | 0.65 | 241 | YES |
| BT1 | Li‑Po 1S + PCM + JST (best bay fit) | **AKY‑LP623255 / AKY0646** (Akyga) | 1 | 4.59 | 1 482 | YES |
| D‑TVS | USB‑C ESD/TVS array (4‑ch, low‑cap) | **TPD4E05U06DQAR** (TI) | 1 | ~0.50 | yes | YES |
| PASSIVES | Passives + small‑parts lot¹ | various | 1 lot | ~8.00 | yes | EST |
| | | | | **≈ €18.5** | | |

¹ 2× 5.1 kΩ CC resistors (USB‑C Rd), decoupling + bulk MLCCs, buck 2.2 µH + e‑ink boost inductors, the **wheel‑click SMD tactile dome**, and misc R/C. Estimated as a lot because no routed schematic exists yet — buy per‑value in small quantity at TME.

**+ Shipping to SK ≈ €5** (DPD/GLS; free over TME's SK threshold).

**Notes / watch‑items**
- **FPC connector:** confirm the panel's FPC is **bottom‑contact** to match FH12‑24S‑0.5SH (flip‑contact variant exists if top‑contact).
- **Battery (fit‑checked against the CAD bay 35 × 59 × 7.1 mm):** **AKY‑LP623255** (6.2 × 32 × 55 mm, 1100 mAh) is the best in‑stock fit — sits inside the 33 × 56 retention frame with ~0.9 mm swelling room. Vendor **Akyga**; **CE + WEEE + on‑cell PCM** confirmed (photo `assets/akyga-lp623255-tme.png`); Akyga runs **IEC 62133‑2** CB testing on its Li‑Po line — **request the AKY0646‑specific UN 38.3 summary + IEC 62133‑2 report** for the compliance file. **Capacity trade‑off:** 1100 mAh vs the spec's 1585 mAh target (~13 h vs ~19 h IEM runtime) — the 1500 mAh Akyga cells don't fit (LP803450 is 8.0 mm thick > 7.1 mm; LP584070 is 40 × 70 mm > bay). To keep ~1500 mAh you'd need a ~7 × 34 × 58 mm cell from another brand, or a bay change. Match the cell's JST lead to the board header. **Ships road‑only** (no air).

---

## 3. Order 2 — Mouser Slovakia  (specialty ICs; the parts TME can't stock)

Mouser (mouser.sk, EUR, prices ex‑VAT) covers every IC TME lacks or had out of stock, all **In Stock** today. EU entity → reverse‑charge with SK IČ DPH, DDP (no surprise customs). **Free shipping over €50 net** — otherwise ~€20.

| Ref | Part | MPN | Qty | Net € | ✓ |
|---|---|---|---|---|---|
| U1 | MCU STM32H7A3 (LQFP144) | **STM32H7A3ZIT6** (ST) | 1 | 13.52 | YES |
| U‑DAC | DAC + Class‑H HP amp | **CS43131‑CNZ** (Cirrus) | 1 | 10.59 | YES |
| U‑CHG | Charger / power‑path | **BQ21080YBGR** (TI) | 1 | 2.13 | YES |
| U‑GAU | Fuel gauge | **MAX17048G+T10** (ADI) | 1 | 3.37 | YES |
| U‑LDO | Analog LDO 3.3 V (low‑noise) | **TPS7A2033PDBVR** (TI) | 1 | 0.33 | YES |
| U‑PHY | USB HS ULPI PHY | **USB3340‑EZK** (Microchip) | 1 | 1.93 | YES |
| SW2 | Side push‑button tactile | **KSC4V2 / KSC‑series** (C&K) | 1 | 1.23 | YES |
| Y1 | Osc 22.5792 MHz (low‑jitter) | **DSC1001CE2‑022.5792** (Microchip) | 1 | 0.73 | SUB |
| Y2 | Osc 24.576 MHz (low‑jitter) | **ASDMB‑24.576MHZ‑LY‑T** (Abracon) | 1 | 0.73 | SUB |
| MAG | Diametric magnet for MT6701 (Ø6) | Mouser *Diametrical Magnets* | 1 | ~2.59 | YES |
| | | | | **≈ €37.1** | |

**+ Shipping ≈ €20** under €50 — **or €0 if you top the cart to €50** (recommended; add spares of the cheap ICs/passives).

**Notes / watch‑items**
- **BQ21080YBGR is DSBGA‑8** — effectively un‑hand‑solderable on a prototype. For a hand build, use the design's sanctioned fallback **BQ25185** (SOT‑23/WSON, pin‑strap, no firmware needed).
- **Oscillators:** the design's **NDK NZ2520SDA** at 22.5792 / 24.576 MHz is **not stocked at TME, Mouser or LCSC**. The two MEMS XOs above are in‑stock, same 2520 footprint, and fine for bring‑up. For final audio, the NDK exact part is **Digi‑Key‑only** (adds a site) or upgrade to a **Crystek CCHD‑957** (~€8, ultra‑low phase noise).
- **Magnet:** must be **diametrically** magnetised. TME's Ø6 neodymium discs (Elesa+Ganter) are **axial = wrong**; Mouser's *Diametrical Magnets* category is the right source and folds into this order.

---

## 4. Order 3 — LCSC  (only the MT6701 — *Option A only*)

The MagnTek **MT6701** magnetic encoder is a niche Chinese part not carried by TME, Mouser or Digi‑Key. LCSC is the native source (commercial invoice; ships to SK via Global Standard, DDP option).

| Ref | Part | MPN | Qty | Net € | Stock | ✓ |
|---|---|---|---|---|---|---|
| U‑ENC | Magnetic encoder, 14‑bit ABZ + push | **MT6701CT‑ACD‑R** (C3202694) | 1 | 1.78 | In Stock | YES |

(`$1.93` @ 0.92 FX. QFN variant MT6701QT‑ACD and MT6701CT‑STD‑R also in stock.) **+ Shipping ≈ €8.**

> ⚠️ A single ~€2 item carrying ~€8 of shipping + its own customs line is the weakest part of this plan. **Option B deletes this order** by substituting the **ams AS5600** (bare IC **AS5600‑ASOM**, ~€3–4) on the Mouser order — recommended if "fewest sites / least shipping" is the priority.

---

## 5. Order 4 — e‑ink panel  (Good Display, unavoidable China order)

No EU distributor stocks the bare **GDEY0397T81P** panel. Good Display's own storefront (buy‑lcd.com) has it in stock; it issues a commercial invoice and ships DHL/registered from China.

| Ref | Part | MPN | Qty | Net € | Stock | ✓ |
|---|---|---|---|---|---|---|
| DISP | 3.97″ e‑paper 800×480 SSD1677 SPI | **GDEY0397T81P / GDEM0397T81P**² | 1 | 13.20 | 94 | YES |

² The GDEY/GDEM variants are interchangeable per the BOM. `$14.35` Black‑Friday sale (reg $17.50) @ 0.92 FX. **+ Shipping ≈ €12–22** (registered air vs DHL) + ~€8 reclaimable import VAT, 0% duty.

**Cleaner EU alternative (when back in stock):** **OpenELAB** lists the same panel at **€17.34 excl. VAT**, takes a B2B VAT number, and **handles EU import + customs + VAT for you** (ship to their EU office then locally) — the tidiest accounting path for an s.r.o., but it was **Sold out** on 2026‑11‑18. Worth a stock‑alert. Marketplace fallback: AliExpress $18.48 (accounting‑awkward — avoid for a company invoice).

Good Display official product page (confirms "Strong Stock Availability" + STM32 sample code, useful for the firmware): <https://www.good-display.com/product/613.html>

---

## 6. Why this split (fewest sites / least shipping)

- **TME + Mouser** is the natural 2‑way backbone: TME wins on connectors/passives/battery and cheap SK delivery; Mouser holds the specialty audio/power/USB ICs TME can't. Everything on both is reverse‑charged on a clean EU invoice.
- **LCSC** appears **only** because of the MT6701 — remove it with Option B.
- **The panel is the one hard China dependency**; OpenELAB is the way to pull even that into the EU once restocked.
- **Minimum achievable: 3 orders** (TME, Mouser, Good Display) with Option B — or effectively **2 EU orders + 1 panel** which is as consolidated as this BOM allows at qty 1.

## 6b. Battery — certification ownership & runtime parity

**Who owns the cert (all cell options): the CELL VENDOR, not us** — provided we:
1. buy a **finished, pre‑certified cell with its own PCM** (not bare cells we protect ourselves — that would make *us* the battery manufacturer);
2. **don't modify** the cell or its protection circuit;
3. buy from a vendor that **supplies the per‑model docs** (Akyga does; avoid no‑name AliExpress cells);
4. accept it's **per model** — switch cells → obtain *that* model's UN 38.3 summary + IEC 62133‑2 report.

**Stays our responsibility regardless of cell** (device‑level, spec §12): CE/RED self‑declaration + EMC test; **WEEE + EU Battery Regulation producer registration** (~€300–500/yr); the **Art. 11 user‑replaceable** design (already met — screwed cover + connectorised cell); finished‑device transport (~4 Wh → UN3481 PI967 Section II, no Class 9 label).

**Runtime vs the compact‑DAP class** (SE playback; ours estimated with the e‑ink advantage, ±15%, confirm at bring‑up):

| DAP | Battery | SE playback |
|---|---|---|
| Shanling M0 Pro | 650 mAh | ~14.5 h |
| FiiO M6 | 1200 mAh | ~12 h |
| HiBy R3 (Pro Saber) | — | ~6–8 h (16+ light codecs) |
| **soul‑listener @ LP623255 (1100 mAh)** | 1100 mAh | **~13–15 h (est.)** → matches/beats class |
| soul‑listener @ bay‑max (~1585 mAh) | 1585 mAh | ~19 h (est.) → class‑leading |

**Finding:** 1100 mAh already **matches the DAP class** (≈ M0 Pro, > M6/R3) thanks to the e‑ink screen. **No off‑the‑shelf JST‑terminated cell at TME offers ~1500 mAh in the 35×59×7.1 bay** (the 1500 mAh Akyga cells are too thick / too big), so class‑*leading* runtime needs either a **bay tweak** (allow ~8 mm thickness → AKY‑LP803450 1500 mAh, device ~0.9 mm thicker) or a **semi‑custom ~7×33×57 cell** (MOQ). Recommendation: ship **LP623255** for the prototype (meets parity, clean fit, cert‑at‑vendor); revisit capacity only if bring‑up current draw exceeds the estimate.

## 6c. EU‑domestic sourcing & VAT treatment (SK s.r.o.) — validated 2026‑11‑18

**Tax reality: for a VAT‑registered SK s.r.o. every candidate supplier is effectively VAT‑neutral — there is no literal "US tax" penalty.** The real differences are customs friction, speed, and order count.

| Supplier | Base | VAT on B2B order w/ SK IČ DPH | Customs | Delivery |
|---|---|---|---|---|
| **TME** | Poland (EU) | reverse‑charge, 0% | none (intra‑EU) | 1–2 days |
| **Farnell** | EU (AT/NL) | reverse‑charge, 0% | none (intra‑EU) | 1–2 days |
| **SOS electronic** | Slovakia | domestic SK VAT (deductible) | none | same/next day |
| **Mouser** | US warehouse, NL invoice | reverse‑charge, 0% (NL entity) | Mouser pays DDP (no buyer customs) | ~3–5 days, US‑origin |
| **LCSC / Good Display** | China | import VAT 23% (reclaimable) | mostly 0% duty + admin | 1–3 weeks |

So "easiest EU" is about **speed + zero customs admin**, not tax savings — keep everything on **TME + Farnell** and shrink non‑EU orders to the minimum.

### Full‑BOM EU‑domestic verification (TME / Farnell, Playwright‑checked 2026‑11‑18)

| Part | Function | TME | Farnell | EU‑domestic verdict |
|---|---|---|---|---|
| USB4085‑GF‑A | USB‑C | ✓ €0.74 | ✓ | **TME** |
| SJ1‑3523N | 3.5 mm jack | ✓ €0.81 | ✓ | **TME** |
| DM3AT‑SF‑PEJM5 | microSD | ✓ €2.89 | ✓ | **TME** |
| FH12‑24S‑0.5SH | FPC conn | ✓ €0.65 | ✓ | **TME** |
| B2B‑PH‑K‑S | batt conn | ✓ €0.10 | ✓ | **TME** |
| TLV62569DBVR | buck | ✓ €0.23 | ✓ | **TME** |
| TPD4E05U06 + passives | TVS/R/C/L | ✓ | ✓ | **TME** |
| AKY‑LP623255 | battery | ✓ €4.59 | — | **TME** |
| STM32H7A3ZIT6 | MCU | hardly‑avail | ✓ €13.80 | **Farnell** |
| MAX17048G | fuel gauge | ✗ | ✓ ~€4 | **Farnell** |
| USB3340‑EZK | USB PHY | 0 stock | ✓ ~€2 | **Farnell** |
| C&K side tactile | side button | ✗ | ✓ (C&K) | **Farnell** |
| **BQ21080** | charger | ✗ | ✗ | exact none → **sub BQ24075 @ Farnell €2.2** (power‑path, QFN — better for proto) |
| **TPS7A2033** | analog LDO | hardly‑avail | ✗ | exact none → **sub low‑noise LDO @ Farnell** (ADP7142 / LT3045) |
| **NDK NZ2520SDA ×2** | audio clocks | ✗ | ✗ | → **sub MEMS @ Farnell** (Abracon/Microchip, both freqs) |
| **MT6701** | wheel encoder | ✗ | ✗ | → **sub AS5600‑ASOM @ Farnell** (wheel Option B) |
| diametric magnet | wheel magnet | ✗ (axial only) | ~ | Farnell/Mouser "diametrical magnets" |
| **CS43131** | **DAC + amp** | ✗ | ✗ | **HARD HOLE — no EU‑domestic source** (Cirrus = Mouser/Digi‑Key/Arrow only); no clean drop‑in sub |
| GDEY0397T81P | e‑ink panel | ✗ | ✗ | **EU alt found (2026‑11‑21):** the **Waveshare 3.97″ "(G)"** panel at **Eckstein (DE), €29.74** is physically identical (96.62×56.24×0.92 mm, 86.40×51.84 active, 800×480, SSD1677) → **EU‑domestic, zero customs.** (Good Display branding via **OpenELAB** Munich — currently sold out. Verify the Waveshare FPC pinout mates the custom‑board connector.) |

**Verdict: with the 2026‑11‑21 panel finding, only ONE part is truly non‑EU — the Cirrus CS43131** (no EU‑domestic audiophile DAC exists; substituting it is a real audio‑design change). The **e‑ink panel is now EU‑sourceable** (Waveshare 3.97″ at Eckstein = the GDEY0397T81P physically), so it's no longer a forced China order. Four exact MPNs (charger, LDO, oscillators, encoder) aren't on TME/Farnell but have clean **EU‑domestic substitutes at Farnell** — the charger (BQ24075) and encoder (AS5600) subs are arguably *better* than the originals for a hand‑built prototype. **So the product can be ~100% EU + one small Cirrus/Mouser line for the CS43131.**

**Easiest all‑EU product plan:** **TME + Farnell** for the whole BOM (with the four substitutions), **+ one tiny Mouser/Digi‑Key line for the CS43131**, **+ the Good Display panel**. (Dev supply is already fully TME + Farnell + panel, zero Mouser.)

## 7. Open confirmations before ordering
1. Pick **Option A (design‑exact MT6701)** or **Option B (AS5600, 3 orders)**.
2. **BQ21080 DSBGA** vs **BQ25185** for a hand‑built prototype (recommend BQ25185).
3. Oscillators: accept the in‑stock MEMS subs, or add Digi‑Key for exact NDK / Crystek upgrade.
4. Battery cell fit vs enclosure cavity (AKY‑LP803450 vs LP584070) + connector match.
5. Confirm TME SK free‑shipping threshold and whether to top Mouser to €50 for free shipping.

---

*Methodology: live Playwright validation on 2026‑11‑18 against TME (Slovakia storefront), Mouser Slovakia, LCSC, Good Display / buy‑lcd.com, OpenELAB. Prices are qty‑1, net/ex‑VAT, EUR; USD converted at 0.92. Machine‑readable data: [`soul-listener-sourcing-eu-cz.csv`](soul-listener-sourcing-eu-cz.csv).*

Panel price/availability cross‑references: [Good Display](https://www.good-display.com/product/613.html) · [buy‑lcd.com](https://buy-lcd.com/products/gdem0397t81p) · [OpenELAB](https://openelab.io/products/3-97-inch-e-paper-800x480-spi-interface-e-ink-screen).
