# soul-listener — Licensing / IP surface & how others work around it

**Date:** 2026-11-18
**Scope:** Every licensing / patent / trademark exposure for a small-batch, EU-based, **open-source** hi-res DAP, what *we* do about each, and **how other players (open-source projects, commercial DAPs, open-hardware businesses) work around it.**
**Companion:** the device-level regulatory/cost plan is in the design spec `docs/superpowers/specs/2026-09-04-soul-listener-internals-design.md` **§12**; this doc widens it to the full IP surface + real-world precedents. Sourcing is in `sourcing/`.

> **The one principle that explains all of it:** open-sourcing clears **copyright**, not **patents** or **trademarks**. So the game is: ship patent-free/expired tech, don't populate the royalty-bearing radio, never print a logo you haven't licensed, and inherit certifications from pre-certified components. Done right, the device carries **zero per-unit royalty** and no licensing *money* beyond compliance + an optional trademark.

---

## 1. Battery (documented decision — ties to sourcing §6b)

**Decision (2026-11-18):** ship **Akyga AKY-LP623255** (1S Li-Po, 1100 mAh, 6.2×32×55 mm, PCM + JST) as the prototype cell — the best clean fit to the 35×59×7.1 mm bay. **Certification sits with the cell vendor** (Akyga): UN 38.3 + IEC 62133-2 + CE/RoHS are theirs to hold; we obtain the **per-model AKY0646 docs** and archive them. This holds for *any* cell we pick, provided we (1) buy a finished pre-certified cell with its own PCM, (2) don't modify the cell/PCM, (3) buy from a vendor that actually issues the docs, (4) re-collect docs per model if we switch cells. Runtime at 1100 mAh (~13–15 h SE, e-ink) **matches the compact-DAP class** (Shanling M0 Pro ~14.5 h, FiiO M6 ~12 h). Class-*leading* 1500 mAh is deferred (needs a bay tweak or semi-custom cell). See `sourcing/SOURCING-EU-CZ-2026-11.md` §6b.

**What stays ours regardless of cell:** WEEE + EU Battery-Regulation producer registration, Art. 11 user-replaceability (design, already met), finished-device transport class (~4 Wh → UN3481 PI967 Section II, no Class 9 label).

---

## 2. The full IP surface — item · our approach · how others work around it

| # | Item | The exposure | **Our approach** | **How others work around it** | Residual cost to us |
|---|---|---|---|---|---|
| 1 | **MP3** | Patents **expired 2017** | Ship it, free | Everyone ships MP3 freely now; the old Fraunhofer license is moot | €0 |
| 2 | **AAC** | Patented (Via LA): ~$0.98/unit decode, $1k min, **no free tier** | **Drop it** (not in firmware) | Commercial DAPs (FiiO/Shanling) **pay the per-unit license** (users expect AAC); **Tangara drops AAC**; Rockbox ships it as user-flashed firmware (see §3) | €0 |
| 3 | **FLAC / ALAC / Opus / Vorbis / WAV** | Royalty-free | Ship all | Universal | €0 |
| 4 | **MQA** | Proprietary license; MQA Ltd **went bankrupt 2023** (→ Lenbrook) | Skip | Most DAPs dropped MQA after the collapse; market moved to plain PCM/DSD | €0 |
| 5 | **Dolby / DTS** | Licensed, costly | N/A (2-ch DAP, not an AVR) | DAPs don't decode Dolby/DTS; it's a receiver feature | €0 |
| 6 | **FAT32 (LFN)** | MS long-filename patents **expired (~2013–2020)** | Ship FAT32 at launch | All players ship FAT32 now | €0 |
| 7 | **exFAT** | **Still patented** (Microsoft); needed for >32 GB SDXC out-of-box | **Defer** — FAT32 now, exFAT swappable later behind an FS trait | Linux/Android DAPs get exFAT via the **kernel** (MS opened exFAT patents *for the Linux kernel*, added in Linux 5.4, 2019) — that pledge covers the kernel impl, **not** bare-metal firmware; non-Linux DAPs either **license exFAT from MS** (per-device fee) or tell users to **reformat to FAT32** | €0 (FAT32); license only if we add exFAT |
| 8 | **USB VID/PID** | A sold USB device needs a **Vendor ID** (USB-IF membership ~$6k/yr for VID+logo) | **Use pid.codes** (shared VID `0x1209`) — **free PID for open-source hardware**; commercial sale of the open design is accepted | This is the standard open-hardware move; commercial vendors **buy their own VID**; some sublicense a PID under their USB-bridge-chip vendor's VID (Tangara-class designs use the bridge-chip vendor / pid.codes) | **€0** (requires genuinely open-source license — we qualify) |
| 9 | **USB Audio Class 2 (UAC2)** | Open USB-IF class spec | Implement | No license needed | €0 |
| 10 | **SD / microSD** | SDA membership (~$2k/yr) for the **logo** + full spec | Implement from the **public Simplified Spec** in SPI/SD mode; **no SD logo** | Everyone uses the simplified spec free; commercial vendors join SDA only to print the logo | €0 (no logo) |
| 11 | **DSD / aptX / LDAC / "Hi-Res Audio" logos** | Displaying the **logo/trademark** needs a brand agreement | **Plain-text claims only** ("plays DSD256", "24-bit/384 kHz") | Commercial DAPs **pay for the JAS "Hi-Res Audio" gold sticker** (it sells); small/open players skip the logos and describe in text | €0 |
| 12 | **Bluetooth SIG** | Any BT product needs a **Declaration ID** (Adopter/qualification ~$5k–12k) | **Ship wired-only** (BT module DNP) → €0 | **Inherit a pre-qualified module's QDID** and file a cheap Declaration referencing it — exactly what **Tangara does via the ESP32** (Espressif pre-qualified the radio). Commercial vendors are SIG members and pay the full fee | €0 (wired); ~$5k if a BT SKU is sold, via the module's QDID |
| 13 | **aptX / LDAC codecs** | Live in the BT chip: aptX = Qualcomm (license via module vendor); LDAC = Sony + **no off-the-shelf source module** | Wired-only → N/A | Open/small players stick to **SBC** (royalty-free default) — **Tangara is SBC-only**; commercial players pass aptX through the module license, few do LDAC *source* | €0 (wired) |
| 14 | **Chip vendor NDAs / SDKs** | Qualcomm QCC needs NDA + ADK | **Pre-made Feasycom module** hides the Qualcomm NDA; **CS43131/TI/ST** have open datasheets, no NDA, **no per-unit license** | Everyone uses pre-certified BT modules to dodge the Qualcomm NDA; audiophile builders pick open-datasheet DACs (Cirrus/TI/AKM) over NDA-gated parts | €0 |
| 15 | **Open-source license compliance** (our *own* obligation) | GPL codec libs / **GxEPD2 e-ink code is GPL-3.0** → copying pulls GPL + source-release onto the firmware | We're open-source anyway; **prefer permissive Rust crates (MIT/Apache)**, clean-room the e-ink waveforms or use Good Display's sample-code license; track everything in the **SBOM** | Commercial closed-firmware DAPs avoid GPL libs or buy commercial codec licenses; open projects just comply (release source) | €0 money; a source-release obligation (fine for us) |
| 16 | **Your own brand** | Open hardware lets **anyone clone** the design | **Register the product/brand trademark** (EUIPO ~€850, 1 class) + CERN-OHL the design | The **Arduino / Prusa / Adafruit model**: design stays open, the **trademark is protected** so clones can't be sold under your name — the brand + assembled units + community are the moat | ~€850 one-time (recommended, optional) |
| 17 | **CERN-OHL (HW) / FOSS (FW)** | Licenses we *grant*; they clear copyright for reusers but **grant no patent immunity** | CERN-OHL-S hardware + a FOSS firmware license (matches Tangara) | Same license used by Tangara and most open-hardware; it's what *unlocks* pid.codes eligibility and GPL-lib use | €0 |

**Compliance that feels like licensing (mandatory to sell, not a license — see §12):** CE/RED self-declaration (Module A, no Notified Body), EN 62368-1 / EN 55032-35 as evidence, **WEEE + EU Battery-Reg registration** (~€300–500/yr), pre-certified modules to inherit RF approvals, EU-only first (defer FCC/UKCA), a DIY pre-compliance EMC scan before the paid lab.

---

## 3. How specific players navigate it (precedents)

- **Tangara** (the closest cousin — open-source ESP32 iPod-like player): ships **WAV/MP3/FLAC/Opus/Vorbis — no AAC**; Bluetooth is **SBC-only** (royalty-free) over the **pre-qualified ESP32** radio (inherits the BT QDID); hardware + firmware under **CERN-OHL-S + FOSS** on Sourcehut; USB handled by a **SAMD21 bridge**. It is almost a 1:1 template for our licensing posture. ([Crowd Supply](https://www.crowdsupply.com/cool-tech-zone/tangara), [CNX](https://www.cnx-software.com/2024/02/06/tangara-portable-open-source-esp32-music-player/))
- **Rockbox** (open *firmware* for existing players, hardware **not sold**): ships many codecs including patented ones — the patent exposure is different when you distribute **software that users flash onto hardware they already own**, rather than *placing a product on the market*. **We can't lean on this** — we sell the hardware+firmware bundle, so the product-on-market rules apply to us.
- **FiiO / Shanling** (commercial): **license what sells** — pay AAC per-unit, buy the Hi-Res gold sticker, join Bluetooth SIG; many run **Android/Linux** so exFAT comes free via the kernel; they **buy a USB VID**. Volume justifies the fixed costs.
- **Arduino / Prusa / Adafruit** (open-hardware businesses): **open design + protected trademark + sell assembled/kitted units**; the brand and community are the moat, not secrecy. This is our commercial model.

---

## 4. Takeaways & the one real gap

1. **No per-unit royalty anywhere in the device as designed** — AAC dropped, exFAT deferred (FAT32), no logos, BT not populated. Confirmed against the BOM in `sourcing/`.
2. **The one actionable gap vs spec §12: USB Vendor ID.** The spec never mentions it. **Resolve with pid.codes** (free PID under VID `0x1209`, legitimate for our open-source, commercially-sold design). Claim a PID when the USB stack is built. ([pid.codes](https://pid.codes/), [FAQ](https://pid.codes/faq/))
3. **Recommended spends (not royalties): optional EUIPO trademark ~€850**, CE/EMC test ~€3–6k one-time, WEEE/Battery registration ~€300–500/yr.
4. **Open-source license hygiene is the quiet obligation** — keep GPL (e.g., GxEPD2 GPL-3.0) vs permissive straight in the SBOM; prefer MIT/Apache Rust crates; clean-room or correctly-license any copied e-ink waveform code.

## 5. Software-side AAC — the `aac` feature gate & cross-repo enforcement

AAC is not only a *device* concern: the **full AAC decoder lives in `soul-audio-core`** (`crates/soul-audio-core/src/formats/aac/`, a clean-room pure-Rust AAC-LC + HE-AAC + HE-AACv2). Clean-room ≠ patent-clean — the Via LA **AAC patent** (**$15,000 one-time + $0.98/unit**, tiered) attaches to any *product* that ships a decoder, regardless of whose code it is.

**Where the obligation lands (per target):**
- **`soul-audio-core`** (library) — ships no "units", so it doesn't itself owe the fee, but with no opt-out it was the **uncontrolled vector**: every downstream product compiled AAC in.
- **`soul-player-ng`** (distributed desktop app) — **this is where the fee attaches**: each distributed copy of `soul-player-rune.exe` with a bundled AAC decoder is a Via LA "unit".
- **firmware** (`soul-listener-gh`) — each *device* shipping AAC-LC is a unit.

**Fix (implemented 2026-11): an opt-in `aac` cargo feature in `soul-audio-core`.**
- `aac = []`, **default OFF** — gates the decoder (`formats/aac`, `AnyDecoder::Aac` + arms, the construction sites → graceful `DecodeError::Unsupported` when off, the AAC-in-MKV arm, AAC tests). The container *parsers* stay (they're not the patented codec).
- `pub const AAC_ENABLED: bool = cfg!(feature = "aac")` — the enforcement hook.
- **Our products never enable it.** Desktop AAC playback, if ever wanted, should **delegate to the OS codec** (Windows Media Foundation / macOS CoreAudio) so the OS vendor's licence covers it and the app bundles no decoder — *not* by enabling this feature.

**Enforcement (arch tests — never re-add AAC):**
- `soul-player-ng/crates/soul-player-backend/tests/architecture.rs` — `cargo metadata` scan asserting `soul-audio-core` is **not** resolved with the `aac` feature.
- `soul-listener-gh/dap/arch-tests/tests/aac_license.rs` — asserts `soul_audio_core::AAC_ENABLED == false`.
- Both test messages cite the **$15k + $0.98/unit** cost and link back to this document.

## 6. Locked decisions & a CE-framing correction (2026-11-19)

- **USB Vendor ID → pid.codes, no logo.** Free PID under VID `0x1209` for the open-source design; describe the port as a plain-text "USB-C port", never the "Certified USB" / USB-IF logo. **€0.**
- **Filesystem → FAT32, final.** Accept the 4 GB/file cap (a non-issue for music; a 4 GB file ≈ 6 h of 16/44 FLAC); TB cards are reformatted to FAT32. **exFAT not pursued → €0.**
- **Bluetooth → pre-certified module** (Feasycom FSC-BT1035 / QCC3056) *if/when* populated — inherits the module's RED/FCC/BQB radio certs (no radio re-test); still owes a Bluetooth SIG end-product Declaration (~$5k) + a plastic antenna window. **Shipped v1 stays wired-only (BT DNP) → none of that applies.**
- **DAC → Cirrus CS43131** (see `2026-11-18-dac-amp-selection.md`) — the one non-EU-domestic active part (Mouser/Digi-Key); warm-voiceable via its selectable digital filters (Slow Roll-Off / Apodizing).

**CE-framing correction (supersedes spec §12):** a **wired-only** soul-listener has **no radio**, so it falls under the **EMC Directive (2014/30/EU) — NOT the RED.** RED (2014/53/EU) applies *only* when BT/WiFi is populated; LVD doesn't apply (5 V/battery, below scope). So shipped wired-only v1 = **one EMC lab test (EN 55032/55035, ~€3–6k, self-declared Module A) + RoHS/REACH self-declaration + WEEE / EU-Battery-Reg registration (~€300–500/yr)**. Populating BT flips it to RED (+ host EMC/radiated test + SIG Declaration + antenna window). Precedent: **Tangara did exactly this** — CE + FCC + a lab EMC test + an FCC filing — even with a pre-certified ESP32 module (a pre-cert module is "not a get-out-of-jail-free card"; the end product still tests). ([Tangara FCC update](https://www.crowdsupply.com/cool-tech-zone/tangara/updates/fcc-filing-user-manuals-and-case-manufacturing) · [Crowd Supply CE guide](https://www.crowdsupply.com/guide/about-ce-certification))

## Sources
- MP3 patent expiry (2017); Microsoft exFAT-to-Linux-kernel (Linux 5.4, 2019); MQA bankruptcy (2023, Lenbrook).
- USB open-source VID: [pid.codes](https://pid.codes/) · [VID 1209](https://pid.codes/1209/) · [FAQ](https://pid.codes/faq/).
- Tangara precedent: [Crowd Supply](https://www.crowdsupply.com/cool-tech-zone/tangara) · [CNX Software](https://www.cnx-software.com/2024/02/06/tangara-portable-open-source-esp32-music-player/) · [Hackster](https://www.hackster.io/news/cool-tech-zone-s-tangara-is-the-open-hardware-2000s-aesthetic-answer-to-apple-s-ipod-classic-64d71e89fd31).
- Design spec licensing baseline: `docs/superpowers/specs/2026-09-04-soul-listener-internals-design.md` §12.

*Figures are 2026 ballparks and not legal advice — confirm the Bluetooth SIG fee path, exFAT terms, and any trademark filing with counsel before committing.*
