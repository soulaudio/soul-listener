# DAC + headphone‑amp selection — decision & evidence

**Date:** 2026-11-18
**Method:** deep‑research harness (110 agents, 27 sources, 25 claims adversarially verified 25/25) + primary‑datasheet / measurement cross‑check.
**Brief:** best‑value‑but‑class‑leading, warm/musical‑leaning DAC+amp for the soul‑listener portable DAP — STM32H7A3 I²S‑slave source + external dual‑oscillator MCLK, 3.5 mm SE now / 4.4 mm balanced later, single‑cell Li‑Po, 16–300 Ω loads, ~€10–25 section budget, EU small‑batch hand‑assembly.

## 1. Decision

**Keep the Cirrus CS43131** (the chip already in the design). The research independently re‑derived it as the best‑value, class‑leading, warm‑voiceable, constraint‑fitting choice.

| Rank | Part | Role | Why |
|---|---|---|---|
| 🥇 **Primary** | **Cirrus CS43131** | integrated DAC + ground‑centered Class‑H amp | 130 dB(A), −115 dB THD+N (line), 30.8 mW/32 Ω, no HV rail, DSD256 native, I²S‑slave + both MCLK families, **5 selectable filters → warm‑voiceable**, solderable QFN, ~€10.6 |
| 🥈 Runner‑up | **ESS ES9219C** | integrated DAC + SABRE amp | more SE power (~70–80 mW/32 Ω), proven dual‑chip SE/balanced in Shanling M0 Pro, DSD256/DoP/MQA, 40‑QFN — pick if driving harder full‑size cans |
| 🏆 Flagship | **AKM AK4493S + TI TPA6130A2** | warm hi‑fi DAC + portable DirectPath amp | measurably‑warm (2nd‑harmonic‑dominant) Velvet Sound; AK4493S confirmed in production (AKM PCN June 2026); more board + a separate amp |
| ❌ Ruled out | ~~AKM AK4377A~~ | — | only 25 mW/32 Ω, **PCM‑only (no DSD)**, 0.4 mm CSP/BGA (not hand‑solderable) |

## 2. Why the CS43131 holds up (verified)

- **Measured class:** 130 dB(A) DR, −115 dB THD+N at the line condition; ground‑centered **Class‑H** amp hitting **30.8 mW/32 Ω** (15.6 mW/16 Ω, 5 mW/600 Ω) with **no separate high‑voltage rail**, ~23 mW total playback power from a 1.8 V core via an internal inverting charge pump (which also removes output DC‑blocking caps). Up to 2 Vrms line‑level headroom. [[Cirrus DS1155F2](https://statics.cirrus.com/pubs/proDatasheet/CS43131_DS1155F2.pdf), [DigiKey](https://www.digikey.com/en/product-highlight/c/cirrus-logic/cs43131-masterhifi-audio-dacs)]
- **Warm is a firmware knob:** 5 selectable digital filters (Fast/Slow Roll‑Off, Apodizing, Low Dispersion, Sharp). **Slow Roll‑Off / Apodizing** = less pre‑ringing, warmer/more‑analog presentation — so the warm/musical lean is tunable in software without a different chip. [[Cirrus DS](https://www.mouser.com/datasheet/2/76/CS43131_F1-1379771.pdf)]
- **Clocking confirmed for our topology:** in slave mode SCLK/LRCK are inputs and the chip accepts **MCLK/LRCK = 512/256/384**, SCLK/LRCK = 32/48/64. Our oscillators give a clean **512fs** (22.5792 MHz→44.1k, 24.576 MHz→48k), SCLK=64fs. Topology: external oscillator → MCLK to the DAC *and* into the STM32 SAI (I2S_CKIN); the SAI generates SCLK/LRCK from that same clock; CS43131 = I²S slave receiving all three **synchronously** (no PLL jitter, no slip). This is the standard CS43131‑dongle clocking. Firmware must program the CS43131's expected SCLK/LRCK format per the register map. [[Cirrus DS](https://www.mouser.com/datasheet/2/76/CS43131_F1-1379771.pdf)]

> **Correction, 2026-11-23 — the ratio framing above is wrong, the conclusion is right.**
> A read of DS1155F2 shows the CS43131 does **not** constrain MCLK as a multiple of Fs at all. §4.6.1: *"When the MCLK is supplied to the device through the XTI/MCLK pin … its frequency must be one of the nominal MCLK_INT frequencies (22.5792 or 24.576 MHz)"*, and register `0x1000B` (ASP_SPRATE) states that apart from 384 kHz in master mode, *"MCLK_INT can be either 22.5792 MHz or 24.576 MHz"* for every rate. MCLK is a **fixed system clock**; the sample rate is carried by ASP_SPRATE plus LRCK. "512/256/384" is how ESS and Wolfson parts work, not this one, so "does it accept 128fs?" is a malformed question for it.
>
> The oscillator pair named above — 22.5792 / 24.576 MHz — is correct, and so is the topology. What forces **two** oscillators is the **SAI**, not the DAC: SCK must be exactly `64 × Fs` so LRCK is exactly Fs, and 24.576 MHz does not divide into the 44.1 kHz family by an integer (24.576e6 / 2.8224e6 = 8.707…). Each oscillator reaches its family with MCKDIV 8/4/2.
>
> This correction matters because `dap/hw-spec` had drifted from this document: it encoded the ratio phrasing as the governing rule, inferred that 176.4 kHz "falls off the bottom at 128fs", and resolved that by specifying a 45.1584/49.152 MHz (1024×Fs) pair. The spec now models the distinction explicitly as `MclkContract::{FsMultiple, FixedSystemClock}` — V1's ES9038Q2M genuinely is the former — and gates both the SAI arithmetic and the DAC's declared contract separately.

## 3. Integrated vs discrete — integrated wins here

For a portable on a ~3.3 V battery rail, integrated DAC+amp wins on power efficiency, board area, no‑HV‑rail simplicity, solderability, and already‑class‑leading measurements. The audiophile discrete amps don't run on our single rail: **OPA1622 needs ≥4 V** (would need an LM27762 charge‑pump rail), **TPA6120A2 needs split/±5–15 V** supplies. The only portable‑friendly discrete amp is **TPA6130A2** (single 2.5–5.5 V, DirectPath charge pump, ~53–64 mW/32 Ω, I²C 64‑step volume, QFN, EU/TME stock) — reserved for the flagship AKM path. [[TI TPA6130A2](https://www.ti.com/lit/ds/symlink/tpa6130a2.pdf), [OPA1622](https://www.ti.com/lit/ds/symlink/opa1622.pdf)]

## 4. "Chip house sound" — warmth is implementation‑dominated

AKM "Velvet Sound" and TI Burr‑Brown do show a **measurable 2nd‑harmonic‑dominant** distortion profile (the euphonic signature) vs ESS's odd‑order pattern [[Archimago](http://archimago.blogspot.com/2022/07/hi-res-thdn-vs-output-level.html)] — so the reputations are real. But the audible character is **dominated by the analog implementation**: an ES9219C DAP (ESS, stereotyped "clinical") was reviewed "musical, natural yet euphonic, no sabre glare" [[M0 Pro](https://www.headphonesty.com/2023/02/review-shanling-m0-pro/)]. Conclusion: a warm‑but‑detailed voicing is achievable on the CS43131 via its filter selection + analog output‑stage tuning — treat warmth as a **design goal**, not a chip lottery. (Medium‑confidence on the causal claim; design/measure it, don't assume it.)

## 5. Balanced v2 path

Combo jacks that take both a 3.5 mm SE *and* a 4.4 mm plug **do not exist** (different barrel diameters). v2 adds a **second physical 4.4 mm jack** and **dual‑monos a 2nd CS43131** (one DAC per channel). Real‑world proof: the **iBasso DC04 Pro ships dual CS43131 with 3.5 SE + 4.4 balanced**; the Shanling M0 Pro does the same with dual ES9219C (1.7 V/90 mW SE → 2.75 V/236 mW balanced, ~2.6×). Both chips are dual‑mono‑able the same way. Requirement: the two DACs share a synchronized MCLK and matched channel phase; measure the balanced power/noise in a prototype. [[DC04 Pro](https://hifigo.com/blogs/news/ibasso-dc04-pro-brand-new-usb-dac-amp-with-dual-cs43131-dac-and-dual-headphone-output)]

## 6. DSD path (already in internals spec §4 — confirmed)

The CS43131's **DoP ceiling is DSD128 (~384 kHz)**. Our firmware packs DSD→DoP, so **DSD256 needs the chip's native DSD interface**, not DoP — which the internals design spec §4 already states. No change needed; the firmware implementation must route DSD256 via the native DSD path (or accept a DSD128 DoP ceiling, which covers nearly all real DSD content).

## 7. Sourcing reality (reconciles with the EU‑sourcing analysis)

**There is no EU‑domestic audiophile DAC** — Cirrus, ESS and AKM all distribute via Mouser/Digi‑Key/Arrow/LCSC only (confirmed: none at TME or Farnell). So "proper audiophile feel" and "the one non‑EU line" are the same conclusion: **order the CS43131 from Mouser/Digi‑Key** (~€10.59, reverse‑charge‑clean via the NL entity). See `sourcing/SOURCING-EU-CZ-2026-11.md` §6c. The ES9219C (ESS) carries the same sourcing profile — no EU advantage over Cirrus.

## 8. Caveats & open items (from the research)

- **Design to LOADED numbers:** 130 dB/−115 dB are line‑level into 10 kΩ. Into 32 Ω, real dongle measurements land ~−103 to −110 dB — still class‑leading, but spec to the loaded figure.
- **ES9219C 32 Ω power** is not in its datasheet (only a 2.8 mW THD+N point); ~70–80 mW is inferred from shipping dongles — prototype‑measure if chosen.
- **Verify current CS43131/ES9219C EU price + stock** at small‑batch quantities before committing (CS43131 was €10.59 at Mouser SK, 2026‑11‑18).
- **Prototype‑confirm** the dual‑mono balanced topology (shared MCLK, matched phase, measured power/noise penalty) before the v2 board.

## Sources (primary first)
- Cirrus CS43131 datasheet DS1155F2 / F1‑1379771 · DigiKey CS43131 MasterHIFI highlight
- ESS ES9219 datasheet v1.2 (Mouser) · Shanling M0 Pro teardown (Headphonesty) · iBasso DC04 Pro (HiFiGo)
- TI TPA6130A2 / OPA1622 / TPA6120A2 datasheets · AKM AK4377AECB datasheet · AKM PCN #2026013‑1e (June 2026)
- Archimago THD+N‑vs‑level (AKM/BB 2nd‑harmonic vs ESS odd‑order)

*Full verified claim set + per‑agent transcripts: workflow run `wf_d3c83803-0ea`.*
