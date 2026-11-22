# Good Display e-paper module — import research

Task: bring a Good Display (gd-epaper.com) e-ink module into `soul-listener.FCStd`
for enclosure cutout + mounting design.

## Selected display — GDEY0397T81P (3.97″, monochrome 4-gray)

Chosen via the Good Display product page
(<https://www.good-display.com/product/613.html>). Thin (0.92 mm) portrait module,
SSD1677 controller, SPI. Replaces the earlier GDEY029Z95 tri-color candidate.

| Parameter | Value |
| --- | --- |
| Outline (W×L×D) | 56.24 × 96.62 × 0.92 mm |
| Active area | 51.84 × 86.40 mm |
| Resolution | 480 (H) × 800 (V) |
| Pixel pitch | 0.108 × 0.108 mm, DPI 235 |
| Controller | SSD1677 |
| Interface | SPI (3/4-wire), 24-pin FPC @ 0.5 mm |
| Colors | monochrome, 4 grayscale |
| Weight | 10.45 ± 0.5 g |

Source: `GDEY0397T81P_Specification.pdf` (26 pp), downloaded from the
good-display.com download center.

## Verified facts (2026-09-01)

- Good Display publishes **no STEP/STL/CAD** — only PDF/DWG drawings, specs,
  schematics, sample code.
- FreeCAD Parts Library (FreeCAD/FreeCAD-library) contains **no e-paper parts**.
- Community models on GrabCAD etc. are unverified and mostly the Waveshare
  driver-board variant — not bare Good Display modules.

## Decision — parametric reusable part (built, v2 photo-accurate)

Standalone, spreadsheet-driven FreeCAD part (not imported):

- File: `D:/dev/soulaudio/parts/GDEY0397T81P.FCStd`
- Builder: `D:/dev/soulaudio/parts/build_gdey0397t81p.py` (re-runnable)
- Contents (separate colored parts in a `DisplayParts` group):
  - `ModuleFrame` — white/light-gray frame slab (56.24 × 96.62 × 0.92),
    recess-cut for the active area well
  - `ActiveSurface` — matte white e-ink surface (51.84 × 86.40), recessed
    0.05 mm below the glass top
  - `FPCCable` — orange/copper flex, folded per Good Display's own usage
    guideline (news/80.html + PDF): bends BACKWARD (down/behind) 180°, never up.
    Bend position is 1.0–1.5 mm OUT from the glass edge (model uses 1.25 mm),
    curvature radius R > 0.5 mm (model uses 0.6), then the cable lies flat
    against the back face and is fixed with PSA (no 2nd bend).
  - `FPCConnector` — black connector block flat behind the module
- Colors + bezel **verified against product photos** (good-display.com/product/613.html):
  frame is white (not black), FPC is orange (not amber), connector black,
  and the bezel is ASYMMETRIC — active area offset so the border is thinner on
  the top/sides (2.5 / 1.9 / 2.5 mm) and thicker (7.72 mm) at the FPC edge.
- FPC fold geometry (per Good Display guideline): bonds to the glass top edge
  ~1 mm, bends 1.25 mm out from the edge with R=0.6, 180° backward under the
  glass, back-run flush against the back face. Cable tucks behind the module so
  an enclosure frame can wrap around it.
- `Params` spreadsheet aliases (`module_w`, `module_l`, `bez_*`, `fpc_*`, …)
  drive every dimension — reusable across projects via File → Merge Project
  or an App::Link.

FPC front/back lengths (6 / 30 mm), bend gap (1.25 mm), thickness (0.15 mm), and
the bezel offsets are placeholders — confirm against the physical part before
finalizing a mount. The bend radius R=0.6 and the 1.0–1.5 mm gap are per Good
Display's usage guideline (authoritative).


> Mechanical drawing is page 5/26 (raster image; vision OCR unreliable on the
> fine CAD callouts — dimensions above are from the PDF text layer, authoritative).

## Waveshare 3.97″ = GDEY0397T81P — the EU/zero-customs source (2026-11-21)

The Good Display GDEY0397T81P and the **Waveshare 3.97″ "(G)" panel are the SAME physical panel** — authoritatively confirmed by **GxEPD2** (the de-facto e-paper driver library, "for Good Display *and* Waveshare"), which maps both to a single class:

```
GxEPD2_397_GDEM0397T81   // GDEM0397T81, 480x800, SSD1677, (FPC-7750)
```

Same **SSD1677**, same **800×480**, same **FPC-7750** 24-pin 0.5 mm connector, same module **96.62 × 56.24 × 0.92 mm**. So the parametric CAD part above fits both, and the enclosure/connector are vendor-agnostic.

**Sourcing impact — this removes the only non-EU part of the whole project:** the panel is EU-stockable as the Waveshare 3.97″ at **Eckstein (DE) ~€29.74** (intra-EU, reverse-charge, zero customs) instead of a China order (which now carries the post-1-Jul-2026 EU duty + handling). See `sourcing/EU-SUPPLY-CHAIN.md`. *Only vendor-specific bit: the waveform LUT (VCOM/VSH1) — tune it to whichever panel you actually ship; standardizing on Waveshare for dev + product keeps one LUT.* Firmware SSD1677 driver gotchas: spec §9.
