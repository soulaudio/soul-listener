# soul-listener — Dev-supply sourcing (firmware bring-up rig)

**Date:** 2026-11-21 · **Prices:** Nov 2026, **net / ex-VAT**, EUR · **Buyer:** SK s.r.o., **delivery to Czechia** (intra-EU reverse-charge).
**Purpose:** a bench rig to develop/validate firmware (SD/FAT + `.soulidx`, e-ink SSD1677 SPI + partial refresh, USB-DAC/MSC, scroll-wheel UX, audio I²S plumbing) **before the custom PCB exists.** Not the product — doesn't go in the enclosure.
**Goal of this revision: ZERO customs** — everything intra-EU (reverse-charge, no duty/handling), including the e-ink panel.

## Key decisions (2026-11-21)

- **ZERO customs ⇒ all-EU sourcing.** The only ex-EU part was the e-ink panel — now solved (see below). From 1 July 2026 the EU killed the small-parcel duty-free threshold (flat ~€3/item duty + ~€10 courier handling on *any* China import), so avoiding China for the dev kit is both cleaner and ~cost-neutral.
- **Panel = Waveshare 3.97″, EU-sourced — a drop-in for the GDEY0397T81P.** Waveshare's raw 3.97″ module is **96.62 × 56.24 × 0.92 mm, 86.40 × 51.84 active, 800×480, SSD1677** — *identical* to the Good Display GDEY0397T81P the enclosure is built around (verified against Waveshare's spec). **Eckstein (DE)** stocks the bare "(G)" panel at **€29.74**. So this works for **dev AND the product**, intra-EU, no customs. (The China GDEY0397T81P at ~€14 saves ~€15 on the part but adds ~€16 of new China duty/handling + a 1–3 week wait + import paperwork — net wash, so go EU.)
- **Dev MCU = NUCLEO-H743ZI2** (swap from the exact H7A3ZI-Q). The H7A3ZI-Q is broadline-only (not stocked by maker shops), which forces an extra order; the **H743ZI2 is the common H7 Nucleo every EU maker shop carries** and has the *same* STM32H7 peripherals/HAL — fully fine for the firmware logic (SD/FAT, e-ink, USB, UI, wheel, I²S). Build the dev firmware for the `stm32h743zi` target; confirm the H7A3's exact power numbers later on the custom board. *(Keep the exact H7A3ZI-Q from Farnell/RS only if you specifically want the literal product MCU — that costs one extra order.)*
- **Dev wheel = any EC11/PEC11-class encoder** with a switch (the exact PEC11H "feel" is irrelevant on the bench) — grab one from the maker shop, no separate order.

## Shopping list (all EU — zero customs)

| Item | Part | Role | Source | ~Net € | ✓ |
|---|---|---|---|---|---|
| **e-ink panel** | **Waveshare 3.97″ (G)** (= GDEY0397T81P) | the real panel + its 24-pin FPC | **Eckstein (DE)** | **29.74** | SEARCH✓ + mech-confirmed |
| **e-ink driver/adapter** | **Waveshare Universal e-Paper driver board** (ESP32/SPI, has the FPC socket) | FPC → SPI breakout for the panel (EU replacement for the China DESPI-C02) | **BerryBase / Eckstein (DE)** | ~15 | SEARCH✓ |
| **MCU dev board** | **NUCLEO-H743ZI2** | H7 MCU + ST-LINK + SAI/SDMMC/SPI/USB headers | **maker shop** (Botland/Eckstein/BerryBase) or **RS €27–30** | ~27–30 | SEARCH✓ (RS 1988645-class) |
| **I²S DAC breakout** | **PCM5102 module** | SAI→I²S→audio-out (stand-in for CS43131) | **Botland** | **4.90** | PW✓ |
| **microSD breakout** | **full-bus microSD breakout** (exposes DAT0-3) | exercise SDMMC **4-bit** + the DMA/D-cache gotcha on real silicon (not just SPI) | **maker shop** | ~3–7 | ~ |
| **scroll wheel** | **EC11/PEC11-class encoder + knob** | quadrature + push firmware | **maker shop** | ~2–3 | ~ |
| **bench** | breadboard + jumpers + tactile buttons + 2× USB cables (ST-LINK + user-USB) | wiring | **maker shop** | ~12–15 | ~ |

✓ legend: **PW✓** live Playwright (Botland PCM5102 €4.90, 2026-11-21); **SEARCH✓** confirmed listing (Eckstein Waveshare 3.97″ (G) €29.74; RS Nucleo EU €27–30); **mech-confirmed** = Waveshare 3.97″ module dims match the GDEY0397T81P; **~** = standard maker-shop stock, price to confirm at cart.

## Orders & cost

- **Target: 1–2 EU orders, zero customs.**
  - **If one German maker shop (Eckstein or BerryBase) carries the panel + driver board + Nucleo + breakouts + bench → 1 order.** (Eckstein confirmed for the panel; its full catalog breadth was Cloudflare-blocked from validation — confirm at cart.)
  - **Otherwise 2 EU orders:** a maker shop (**Botland**, cheapest to CZ — Nucleo + PCM5102 + microSD + encoder + bench) **+ Eckstein** (the bare 3.97″ panel + driver board, the one item Botland/BerryBase don't stock).
- **Dev rig total ≈ €95–115 net**, all intra-EU (GLS/DPD ~€6.50, 2–4 days, reverse-charge, **no duty, no handling fee**).
- **VAT note:** billing an SK VAT ID but delivering to CZ is a standard intra-EU acquisition (reverse-charge, 0% VAT); if the company operates in CZ it's cleanest to use a **CZ VAT registration** for CZ deliveries — confirm with your accountant which VAT ID to give at checkout.

## What this rig covers — and what it defers

**On the Nucleo now:** SD/FAT + `.soulidx`, e-ink SSD1677 SPI + partial refresh, USB-DAC (UAC2) + MSC, scroll-wheel UX, decode→I²S plumbing, clock/DMA/cache on real H7 silicon. (Pure decode/index/export logic is already host-testable in `soul-audio-core`.)

**Deferred to the custom board:**
- **CS43131 chain + external NDK MCLK** — the PCM5102 only proves the I²S plumbing; real DAC bring-up is custom-board (or a Cirrus CDB43131 eval board if wanted early).
- **H7A3-specific power profile** — develop on H743; confirm the H7A3's run-current on the real board.
- **Magnetic wheel** (MT6701 + magnet, sealed press) — PEC11/EC11 covers the firmware; keep MT6701 in **ABZ mode** so the dev code ports 1:1.

## Knock-on: the PRODUCT panel is now EU too
The Waveshare 3.97″ being physically identical to the GDEY0397T81P means the **product's one non-EU part is solved** — the final device panel can also be sourced EU (Eckstein) or, if you prefer the Good Display branding, via **OpenELAB** (Munich warehouse, handles the import for you — currently sold out). Update `SOURCING-EU-CZ-2026-11.md` accordingly. Verify at order that the Waveshare "(G)" FPC pinout mates with your custom-board FPC connector (same panel ⇒ expected to match).

*Methodology: Playwright (Botland, RS, BerryBase) + web research + Waveshare/Good Display spec comparison, 2026-11-21. Net/ex-VAT EUR.*
