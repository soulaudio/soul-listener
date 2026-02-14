# Mechanical Design

## Overview

The SoulAudio DAP mechanical design prioritizes:
- Premium feel and aesthetics
- Durability for daily use
- Easy assembly and serviceability
- Thermal management
- Ergonomic controls

## Design Files

### CAD Source Files (`cad/`)

Primary CAD software: **FreeCAD 0.21+**

```
cad/
├── enclosure/
│   ├── front-panel.FCStd        # Tempered glass + bezel
│   ├── back-cover.FCStd         # Aluminum back
│   ├── mid-frame.FCStd          # Internal structure
│   └── assembly.FCStd           # Full assembly
│
├── controls/
│   ├── volume-wheel.FCStd       # Rotary encoder knob
│   ├── button-caps.FCStd        # Button caps (4x)
│   └── sd-door.FCStd            # SD card access door
│
├── internals/
│   ├── pcb-mounting.FCStd       # PCB standoffs
│   ├── battery-holder.FCStd     # 18650 battery cradle
│   └── display-frame.FCStd      # E-ink display mount
│
└── accessories/
    ├── desk-stand.FCStd         # Optional desk stand
    └── protective-case.FCStd    # Silicone case
```

**Alternative Formats**:
- Fusion 360 `.f3d` files in `cad/fusion360/`
- SolidWorks `.sldprt` in `cad/solidworks/`

### Renders (`renders/`)

High-quality product renders for marketing:
```
renders/
├── hero-shot.png                # Main product image (4K)
├── front-view.png
├── back-view.png
├── side-view.png
├── exploded-view.png            # Assembly visualization
├── materials/                   # Render materials
└── scenes/                      # Blender scene files
```

**Rendering**:
- Software: Blender 3.6+
- Cycles renderer
- HDRI lighting
- Physically-based materials

### 3D Printable Files (`stl/`)

STL files for prototyping and small-scale production:
```
stl/
├── prototype/
│   ├── enclosure-front-v1.stl
│   ├── enclosure-back-v1.stl
│   ├── mid-frame-v1.stl
│   └── button-caps-v1.stl
│
├── production/
│   ├── sd-door.stl              # Injection molding ref
│   └── test-jig.stl             # Assembly fixture
│
└── accessories/
    ├── desk-stand.stl
    └── protective-bumpers.stl
```

**Print Settings** (for prototypes):
- Layer height: 0.2mm
- Infill: 20%
- Material: PLA/PETG
- Support: Tree supports for overhangs

### Manufacturing Files (`step/`)

STEP files for CNC and injection molding:
```
step/
├── enclosure-front.step         # Front panel (CNC)
├── enclosure-back.step          # Back cover (CNC)
├── mid-frame.step               # Injection molding
├── volume-knob.step             # CNC machined knob
└── assembly.step                # Full assembly
```

**File Specifications**:
- Format: STEP AP214
- Units: Millimeters
- Coordinate system: Origin at bottom-left-front
- Tolerances noted in drawings

### Engineering Drawings (`drawings/`)

PDF drawings with dimensions and tolerances:
```
drawings/
├── enclosure-front-panel.pdf    # GD&T drawings
├── enclosure-back-cover.pdf
├── mid-frame-injection.pdf
├── assembly-drawing.pdf         # Assembly instructions
└── detail-drawings.pdf          # Critical features
```

**Drawing Standards**:
- ISO 1101 (GD&T)
- ISO 2768-1 (general tolerances)
- Third angle projection

### Assembly Guide (`assembly/`)

```
assembly/
├── assembly-sequence.pdf        # Step-by-step guide
├── torque-specs.md              # Screw torque values
├── exploded-bom.csv             # Mechanical BOM
└── photos/                      # Assembly reference photos
    ├── step-01-pcb-install.jpg
    ├── step-02-battery-install.jpg
    └── step-03-final-assembly.jpg
```

## Design Specifications

### Overall Dimensions

```
Width:   100 mm (3.94")
Height:  60 mm (2.36")
Depth:   12 mm (0.47")
Weight:  145g (with battery)
```

### Materials

| Part | Material | Finish | Cost (qty 100) |
|------|----------|--------|----------------|
| Front panel | Tempered glass (2mm) | Anti-glare coating | $8.50 |
| Back cover | Aluminum 6061-T6 (1.5mm) | Anodized black | $12.30 |
| Mid-frame | PC/ABS plastic | Matte black | $3.20 |
| Volume knob | Aluminum 6061 | Knurled, anodized | $4.50 |
| Button caps | Silicone | Matte finish | $0.80 |
| Screws | Stainless steel M2 | Black oxide | $0.15 |

### Tolerances

**General Tolerances** (ISO 2768-1, class 'm'):
- Linear: ±0.1mm
- Angular: ±0.5°
- Radius: ±0.2mm

**Critical Tolerances**:
- Display window: ±0.05mm
- Screw holes: ±0.025mm (H7 fit)
- Button alignment: ±0.1mm

## Manufacturing Processes

### CNC Machining

**Front and Back Panels**:
```
Process:
1. Material: 6061-T6 aluminum sheet (1.5mm)
2. CNC mill outline and pockets
3. Drill mounting holes (2.1mm for M2 screws)
4. Deburr and clean
5. Anodize (Type II, black, 10-25μm)
6. Laser engrave logo
```

**Machining Specs**:
- Spindle speed: 12,000 RPM
- Feed rate: 800 mm/min
- Tool: 2mm end mill (carbide)
- Coolant: Water-soluble

**Volume Knob**:
```
Process:
1. Material: Aluminum 6061 round stock (Ø20mm)
2. Turn on lathe to final diameter
3. Mill knurling pattern
4. Part off
5. Anodize
```

### Injection Molding

**Mid-Frame** (PC/ABS):
```
Specs:
- Material: PC/ABS (Bayblend T65 MN)
- Color: Black
- Mold: 2-cavity
- Cycle time: 45 seconds
- Gate: Edge gate
- Ejector pins: 8x
```

**Molding Parameters**:
- Barrel temp: 240°C
- Mold temp: 60°C
- Injection pressure: 80 MPa
- Cooling time: 25s

**Tooling Cost**: ~$8,000 (2-cavity mold)

### Tempered Glass

**Front Panel Glass**:
```
Process:
1. Float glass (2mm, ultra-clear)
2. CNC cut to size (100mm × 60mm)
3. Edge polish (1mm chamfer)
4. Anti-glare coating (AG film)
5. Temper at 650°C
```

**Properties**:
- Surface hardness: 9H
- Light transmission: 92%
- Impact resistance: 4× standard glass

## Assembly Instructions

### Tools Required

- Phillips screwdriver (#00)
- Hex driver set (1.5mm, 2mm)
- Tweezers (ESD-safe)
- Torque driver (0.1-0.5 Nm)
- Plastic opening tools
- Isopropyl alcohol (cleaning)

### Assembly Sequence

**Step 1: PCB Installation**
```
1. Place PCB in mid-frame
2. Install 4× M2×4mm standoffs
3. Secure with M2×3mm screws
4. Torque: 0.2 Nm
```

**Step 2: Display Installation**
```
1. Connect FPC cable to PCB
2. Place display in front bezel
3. Secure with adhesive gasket
4. Connect to display frame
```

**Step 3: Battery Installation**
```
1. Insert 18650 battery into cradle
2. Connect battery connector (JST-PH)
3. Route cables through channels
4. Secure with cable tie
```

**Step 4: Controls Installation**
```
1. Install button caps on tactile switches
2. Mount rotary encoder through mid-frame
3. Attach volume knob with set screw
4. Torque: 0.15 Nm
```

**Step 5: Final Assembly**
```
1. Place back cover on mid-frame
2. Insert 6× M2×6mm screws
3. Tighten in star pattern
4. Torque: 0.3 Nm
5. Install SD card door
```

**Total Assembly Time**: ~8 minutes (trained technician)

## Thermal Management

### Heat Sources

| Component | Power (mW) | Location |
|-----------|------------|----------|
| STM32H7 | 1350 | Center PCB |
| WM8960 | 175 | Near audio jack |
| ESP32-C3 | 250 | Top corner |

### Cooling Strategy

1. **Aluminum Back Cover** acts as heat sink
   - Thermal pad between MCU and cover (1.5 W/m·K)
   - Emissivity: 0.9 (anodized)

2. **Internal Air Gaps**
   - 2mm gap around PCB for convection
   - Ventilation slots (covered by mesh)

3. **Thermal Simulation**
   - Steady-state temp: 45°C (ambient 25°C)
   - Max skin temp: 42°C (safe for handling)

### Testing

- Thermal camera imaging
- Surface temperature probes
- Ambient temp: 25°C, 35°C (stressed)
- Test duration: 2 hours continuous playback

## Waterproofing

**IP Rating**: IP52 (dust protected, drip-proof)

**Sealing**:
- Silicone gasket around display
- O-ring seals on button stems
- USB-C port: dust cover (optional)
- 3.5mm jack: sealed connector

## Compliance

### Drop Test

- **Standard**: IEC 60068-2-32
- **Height**: 1.0m onto concrete
- **Orientation**: 6 faces, 8 corners, 12 edges
- **Result**: No functional damage

### Scratch Resistance

- **Front glass**: 9H pencil hardness
- **Aluminum**: Anodized coating (HV 400)

## Prototyping

### 3D Printing

**Recommended Settings**:
```
Printer: Prusa i3 MK3S or similar
Material: PETG
Layer height: 0.15mm
Infill: 20% gyroid
Perimeters: 4
Support: Tree supports
Speed: 50 mm/s
```

**Post-Processing**:
- Sand with 220, 400, 800 grit
- Vapor smoothing (acetone for ABS)
- Prime and paint (optional)

### CNC Prototyping

**Desktop CNC** (Nomad 3 or similar):
- Material: Aluminum 6061
- Finish: Bead blast + anodize (send out)

**Cost** (prototype, qty 1):
- CNC time: ~3 hours
- Material: $25
- Anodizing: $15/part
- Total: ~$40/unit

## Design for Manufacturing (DFM)

### Key Considerations

1. **Wall Thickness**: Min 1.2mm for plastic parts
2. **Draft Angles**: 2° for injection molded parts
3. **Undercuts**: Avoid or use side actions
4. **Fillet Radii**: Min 0.5mm on internal corners
5. **Hole Spacing**: Min 3× diameter from edge
6. **Fasteners**: Use standard sizes (M2, M2.5)

### Cost Optimization

**Volume Impact**:
- Prototype (1 unit): $250 each
- Small run (10 units): $80 each
- Production (100 units): $25 each
- Mass production (1000 units): $18 each

**Cost Breakdown** (qty 100):
- Aluminum parts (CNC): $12.30
- Plastic parts (molding): $3.20
- Glass: $8.50
- Hardware (screws, etc): $1.20
- **Total**: $25.20/unit

## Quality Control

### Inspection Points

1. **Incoming Material**
   - Verify aluminum alloy (6061-T6)
   - Check glass thickness (2mm ±0.1mm)

2. **Machined Parts**
   - Dimensional check (calipers, CMM)
   - Surface finish (Ra < 1.6μm)
   - Anodize thickness (10-25μm)

3. **Molded Parts**
   - Flash check
   - Sink marks
   - Warpage (flatness)

4. **Assembly**
   - Fit and finish
   - Gap consistency
   - Button feel
   - Screen alignment

### Testing Procedures

- Drop test (1m, 26 drops)
- Button life test (100k actuations)
- Thermal cycling (-10°C to +60°C, 100 cycles)
- UV aging (display window)

## Revision Control

| Rev | Date | Changes |
|-----|------|---------|
| A | 2026-01-10 | Initial design |
| B | 2026-01-28 | Increased back cover thickness |
| C | 2026-02-12 | Improved button tactility |

## Software Tools

### CAD
- FreeCAD 0.21+ (open source, primary)
- Fusion 360 (alternative, parametric)
- Blender 3.6+ (rendering)

### Analysis
- FreeCAD FEM workbench (stress analysis)
- OpenFOAM (thermal simulation)

### Documentation
- Inkscape (technical drawings)
- GIMP (image editing)

## Support

For mechanical design questions:
- Check `docs/mechanical/` for detailed docs
- Open issue on GitHub
- Email: mechanical@example.com

## License

Mechanical designs are licensed under CERN-OHL-P v2.
See [LICENSE-MECHANICAL](../LICENSE-MECHANICAL) for details.
