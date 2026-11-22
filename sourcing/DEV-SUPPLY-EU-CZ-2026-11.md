# soul-listener — Dev-supply sourcing (firmware bring-up rig)

**Date:** 2026-11-21 (links re-validated 2026-11-22) · **Prices:** net / ex-VAT, EUR unless noted · **Buyer:** SK s.r.o., **delivery to Czechia** (intra-EU reverse-charge).
**Purpose:** a bench rig to develop/validate firmware (SD/FAT + `.soulidx`, e-ink SSD1677 SPI + partial refresh, USB-DAC/MSC, scroll-wheel UX, audio I²S plumbing) **before the custom PCB exists.** Not the product — doesn't go in the enclosure.
**Goal: ZERO customs** — everything intra-EU (reverse-charge, no duty/handling), including the e-ink panel.

## Key decisions

- **ZERO customs ⇒ all-EU sourcing.** Since 1 July 2026 the EU killed the small-parcel duty-free threshold (flat ~€3/item duty + ~€10 courier handling on *any* China import), so avoiding China for the dev kit is cleaner and ~cost-neutral.
- **Dev MCU = NUCLEO-H753ZI (not H743ZI2).** The exact **NUCLEO-H743ZI2 is going EOL / supply-constrained across the EU** (2026-11-22: Mouser SK "Restricted Availability", Farnell CZ "Discontinued", RS CZ "no longer stocked", TME/Reichelt/Botland 0 results). The **NUCLEO-H753ZI is in stock (Mouser SK, €35.79 ex-VAT, immediate dispatch)** and is **pin/peripheral-identical** (H753 = H743 + a crypto block you don't use) — same STM32H7 HAL, SAI/SDMMC/SPI/USB, so the firmware (SD/FAT, e-ink, USB, UI, wheel, I²S) is unchanged. Build the dev firmware for the `stm32h743zi`/`stm32h753zi` target; confirm the H7A3's exact power numbers later on the custom board.
- **e-ink dev panel = bare Waveshare 3.97″ (G) + a Universal e-Paper Driver HAT**, driven by the Nucleo over **SPI** (= the "3.97inch e-Paper HAT+" external-MCU setup; Waveshare's manual has a dedicated STM32 section). Same **SSD1677 / 800×480** glass as the product's GDEY0397T81P, so the SSD1677 driver (init, LUTs, dual-RAM partial refresh) ports **1:1** to the custom board.
- **❌ NOT the "ESP32-S3 3.97inch e-Paper Dev Board".** That all-in-one wires the panel's SPI straight to an **onboard ESP32-S3** (DIN=IO14/CLK=IO13/CS=IO15/DC=IO27/RST=IO26/BUSY=IO25/PWR=IO33) — the ESP32 drives it, not your STM32. To use its panel from the Nucleo you'd have to detach the FPC and wire it to your own adapter, wasting the ESP32. Use the bare-panel + driver-HAT above instead. (The ESP32-S3 board is only useful as an optional known-good reference demo.)
- **Dev wheel = any EC11/PEC11-class encoder** with a switch (a KY-040 module is fine; the exact PEC11H "feel" is irrelevant on the bench).

## Shopping list — real EU listings (validated 2026-11-22)

| Item | Part (as listed) | Source | Net € | Link |
|---|---|---|---|---|
| **MCU dev board** | **NUCLEO-H753ZI** (in-stock swap for the EOL H743ZI2) | **Mouser SK** | **35.79** | [mouser.sk/…/NUCLEO-H753ZI](https://www.mouser.sk/en/ProductDetail/STMicroelectronics/NUCLEO-H753ZI?qs=%252B6g0mu59x7JptTWmsgHt6Q%3D%3D) |
| MCU (exact, ⚠️ EOL) | NUCLEO-H743ZI2 — "Restricted Availability" | Mouser SK / Farnell CZ | ~30 ref | [mouser.sk/…/NUCLEO-H743ZI2](https://www.mouser.sk/en/ProductDetail/STMicroelectronics/NUCLEO-H743ZI2?qs=lYGu3FyN48cfUB5JhJTnlw%3D%3D) · [cz.farnell.com/…/3106010](https://cz.farnell.com/stmicroelectronics/nucleo-h743zi2/dev-board-32bit-arm-cortex-m7/dp/3106010) |
| **I²S DAC breakout** | PCM5102 DAC Module – Stereo I2S 5V w/ 3.5mm out *(⚠️ re-check stock)* | **Botland** | **4.90** | [botland.store/…/pcm5102-dac-module](https://botland.store/ac-and-ca-converters/29246-pcm5102-dac-module-stereo-i2s-5v-with-35mm-audio-output.html) |
| **microSD breakout (4-bit SDIO)** | Adafruit Micro SD SPI **or SDIO** Card Breakout – 3V (#4682, brings out DAT0–3) | **BerryBase** | **3.61** (4.30 incl) | [berrybase.de/…/adafruit-micro-sd-spi-or-sdio](https://www.berrybase.de/en/adafruit-micro-sd-spi-or-sdio-card-breakout-board-3v) |
| **scroll wheel** | Rotary encoder 20-pulse Okystar KY-040 (incl. push-switch) | **Botland** | **1.90** | [botland.store/…/ky-040](https://botland.store/encoders/16558-rotation-sensor-encoder-20-pulses-okystar-ky-040-5904422366278.html) |
| **bench — breadboard** | Breadboard 830 + 140 jumper wires set | **Botland** | **5.90** | [botland.store/…/breadboard-830-140-jumpers](https://botland.store/breadoards/1504-set-of-breadboard-830-140-jumper-wires-5904422303556.html) |
| **bench — buttons** | Tact Switch Buttons Kit – 140 pcs | **Botland** | **7.90** | [botland.store/…/tact-switch-140pcs](https://botland.store/tact-switch/9781-tact-switch-buttons-kit-140pcs-5904422314170.html) |
| **bench — ST-LINK cable** | microUSB B–A 2.0 cable Lanberg 1 m | **Botland** | **0.90** | [botland.store/…/microusb-b-a-1m](https://botland.store/usb-20-cables/18217-microusb-b-a-20-cable-lanberg-black-1m-5901969413663.html) |
| **bench — user cable** | USB A–C 2.0 cable Lanberg 1 m | **Botland** | **0.90** | [botland.store/…/usb-a-c-1m](https://botland.store/usb-c-cables/14817-lanberg-usb-cable-type-a-c-20-black-1m-5901969413731.html) |
| **e-ink panel** | Waveshare 3.97″ e-Paper **(G)** raw panel (MPN 31371) *(⚠️ see note)* | **Eckstein (DE)** | **14.99** (17.84 incl) | [eckstein-shop.de/waveshare-31371-3-97-zoll-e-paper-display-g](https://eckstein-shop.de/waveshare-31371-3-97-zoll-e-paper-display-g) |
| **e-ink driver HAT** | Waveshare Universal e-Paper Panel Driver HAT, SPI, 40-pin GPIO, 3.3–5 V | **BerryBase** | **8.24** (9.80 incl) | [berrybase.de/…/waveshare-universal-epaper-driver-hat](https://www.berrybase.de/en/waveshare-universal-epaper-panel-driver-hat-spi-40pin-gpio-3.3-5v) |

**Validation:** all titles/prices read live 2026-11-22 (WebFetch, or the Playwright browser where Cloudflare/bot-walled — Mouser, Eckstein). BerryBase/Eckstein show **incl. 19 % DE VAT**; for the SK VAT ID they invoice **net under reverse-charge** (net column = gross − 19 %). Botland/Mouser already show net.

### ⚠️ Caveats to confirm at cart
- **NUCLEO-H743ZI2 is EOL-ish in the EU** — go **H753ZI** (validated in stock) unless you specifically need the literal H743 part number.
- **PCM5102 (Botland)** showed **out of stock** at check — Botland restocks it often; re-check, or grab the equivalent HiLetgo/GY-PCM5102 from BerryBase if needed.
- **Eckstein 3.97″ (G):** the old `…-display-g-e-ink-display_1` URL now 404s; the live slug is above at **€14.99 net** (was quoted ~€29.74 — corrected). Its title oddly mixes "mit Treiber-HAT" and "Rohdisplay", delivery shows **~3 weeks** + a **"request a quote"** buy flow — **confirm it's the bare panel** (and whether a HAT is bundled, which could drop the separate BerryBase HAT) before ordering.

## Orders & cost

- **Botland (PL)** consolidates **6 line items** (PCM5102 + KY-040 + breadboard/jumpers + buttons + 2 USB cables) ≈ **€22.40 net**, one order, cheap PL→CZ.
- **BerryBase (DE)** = microSD SDIO breakout + e-paper driver HAT ≈ **€11.85 net**.
- **Eckstein (DE)** = the bare 3.97″ (G) panel ≈ **€14.99 net** (~3-week lead).
- **Mouser SK** = the dev board (**H753ZI €35.79**, in stock).
- **Complete kit ≈ €80–85 net** + shipping, **4 EU orders, zero customs** (all reverse-charge, no duty/handling). *Trim to 3 vendors* by moving the bench-basics onto the BerryBase order (it also stocks breadboards/jumpers/buttons/cables) if you'd rather drop the Botland order.
- **VAT:** billing an SK VAT ID while delivering to CZ is a standard intra-EU acquisition (reverse-charge, 0 % VAT); if the company operates in CZ it's cleanest to use a **CZ VAT registration** — confirm with your accountant which ID to give at checkout.

## What this rig covers — and what it defers

**On the Nucleo now:** SD/FAT + `.soulidx`, e-ink SSD1677 SPI + partial refresh, USB-DAC (UAC2) + MSC, scroll-wheel UX, decode→I²S plumbing, clock/DMA/cache on real H7 silicon. (Pure decode/index/export logic is already host-testable in `soul-audio-core`.)

**Deferred to the custom board:**
- **CS43131 chain + external MCLK oscillator** — the PCM5102 only proves the I²S plumbing; real DAC bring-up is custom-board (or a Cirrus CDB43131 eval board if wanted early).
- **H7A3-specific power profile** — develop on H743/H753; confirm the H7A3's run-current on the real board.
- **Magnetic wheel** (MT6701 + magnet, sealed press) — the KY-040/EC11 covers the firmware; keep MT6701 in **ABZ mode** so the dev code ports 1:1.

## Knock-on: the PRODUCT panel is EU too
The Waveshare 3.97″ (G) being physically identical to the GDEY0397T81P means the **product's one non-EU panel is solved** — the final device can source the bare panel EU (Eckstein) or, for Good Display branding, via **OpenELAB** (Munich). Verify at order that the Waveshare "(G)" FPC pinout mates with the custom-board FPC connector (same panel ⇒ expected to match). See `SOURCING-EU-CZ-2026-11.md`.

*Methodology: live Playwright + WebFetch across Mouser SK, Farnell/RS CZ, Botland, BerryBase, Eckstein + Waveshare/Good Display spec comparison, 2026-11-22. Net/ex-VAT EUR.*
