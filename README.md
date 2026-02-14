# Soul Listener - Professional Digital Audio Player

[![CI](https://github.com/yourusername/soulaudio-dap/workflows/CI/badge.svg)](https://github.com/yourusername/soulaudio-dap/actions)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-2021-orange.svg)](https://www.rust-lang.org)

A professional-grade Digital Audio Player built with Rust, featuring E-ink display, high-quality audio output, and modular architecture.

> **Note:** Application name is centrally configured in `crates/platform/src/config.rs` to ensure consistency across code and UI.

## 📁 Project Structure

```
soulaudio-dap/
├── 📦 crates/              # Rust workspace (firmware)
│   ├── platform/           # Hardware abstraction layer
│   ├── playback/           # Audio playback engine
│   ├── ui/                 # User interface
│   ├── library/            # Music library management
│   ├── bluetooth/          # Bluetooth audio
│   ├── simulator/          # Desktop simulator
│   └── firmware/           # Main firmware application
│
├── ⚡ hardware/            # Electronics design
│   ├── schematics/         # KiCad schematics
│   ├── pcb/                # PCB design files
│   ├── bom/                # Bill of materials
│   ├── datasheets/         # Component datasheets
│   ├── manufacturing/      # Production files
│   │   ├── gerber/         # Gerber files for PCB
│   │   ├── assembly/       # Pick-and-place files
│   │   └── stencil/        # Solder paste stencil
│   └── simulations/        # SPICE/LTspice simulations
│
├── 🔧 mechanical/          # Mechanical design
│   ├── cad/                # 3D CAD source files (FreeCAD, Fusion360)
│   ├── renders/            # Product renders
│   ├── stl/                # 3D printable files
│   ├── step/               # STEP files for manufacturing
│   ├── drawings/           # Engineering drawings (PDF)
│   └── assembly/           # Assembly instructions
│
├── 📚 docs/                # Documentation
│   ├── api/                # API documentation (rustdoc)
│   ├── hardware/           # Hardware documentation
│   ├── mechanical/         # Mechanical documentation
│   ├── user-manual/        # User manual
│   ├── assembly-guide/     # Manufacturing/assembly guide
│   ├── ARCHITECTURE.md     # System architecture
│   ├── ABSTRACTIONS.md     # HAL guide
│   ├── TESTING.md          # Testing strategy
│   ├── BEST_PRACTICES.md   # Coding standards
│   └── AUDIO.md            # Audio architecture
│
├── 🛠️ tools/               # Development tools
│   ├── scripts/            # Build/flash/debug scripts
│   ├── jtag-configs/       # JTAG/SWD configurations
│   └── openocd/            # OpenOCD configurations
│
├── 🧪 tests/               # Testing
│   ├── integration/        # Integration tests
│   ├── hardware/           # Hardware-in-loop tests
│   └── fixtures/           # Test data
│
├── 🎨 assets/              # Binary assets
│   ├── fonts/              # Font files for UI
│   ├── icons/              # UI icons
│   ├── images/             # Images, logos
│   └── audio-samples/      # Test audio files
│
├── 📦 resources/           # Additional resources
│   ├── enclosure/          # Enclosure specs
│   ├── connectors/         # Connector specifications
│   └── cables/             # Cable specifications
│
├── 🔄 .github/             # GitHub Actions
│   └── workflows/          # CI/CD workflows
│
├── 🐳 ci-cd/               # CI/CD configuration
│   ├── docker/             # Docker images for builds
│   └── scripts/            # CI scripts
│
├── CLAUDE.md               # AI assistant context
├── LICENSE                 # License file
├── Cargo.toml              # Rust workspace
├── memory.x                # Linker script
└── .cargo/config.toml      # Cargo configuration
```

## ✨ Features

- **High-Quality Audio**: 24-bit/192kHz DAC, supports FLAC, MP3, WAV
- **E-ink Display**: Low-power, sunlight-readable 4.2" display
- **Long Battery Life**: >20 hours continuous playback
- **Bluetooth Audio**: Wireless streaming and control
- **SD Card Storage**: Support for large music libraries
- **Modular Architecture**: Vertical slice design, fully testable
- **Desktop Simulator**: Develop and test without hardware

## 🚀 Quick Start

### Prerequisites

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Add ARM target
rustup target add thumbv7em-none-eabihf

# Install probe-rs
curl --proto '=https' --tlsv1.2 -LsSf https://github.com/probe-rs/probe-rs/releases/latest/download/probe-rs-tools-installer.sh | sh

# Install tools
cargo install cargo-generate cargo-expand cargo-bloat
```

### Run Simulator (No Hardware Required)

```bash
cd crates/simulator
cargo run --release
```

### Build Firmware

```bash
cd crates/firmware
cargo build --release --target thumbv7em-none-eabihf
```

### Flash to Hardware

```bash
cd crates/firmware
cargo embed --release
```

### Run Tests

```bash
# Unit tests (host)
cargo test --workspace

# Integration tests
cargo test --test integration

# Hardware-in-loop (requires connected device)
cargo test --test hardware --features embedded-test
```

## 🏗️ Development Workflow

1. **Design**: Schematics in KiCad, 3D in FreeCAD
2. **Implement**: Code in Rust with TDD
3. **Test**: Simulator → Integration → Hardware
4. **Build**: CI/CD pipeline validates everything
5. **Manufacture**: Gerbers + BOM → PCB assembly

See [docs/DEVELOPMENT.md](docs/DEVELOPMENT.md) for detailed workflow.

## 📖 Documentation

- [Architecture Overview](docs/ARCHITECTURE.md)
- [Hardware Abstractions](docs/ABSTRACTIONS.md)
- [Testing Strategy](docs/TESTING.md)
- [Best Practices](docs/BEST_PRACTICES.md)
- [Audio System](docs/AUDIO.md)
- [API Documentation](https://docs.rs/soulaudio-dap) (generated)
- [Hardware Documentation](docs/hardware/)
- [User Manual](docs/user-manual/)

## 🛠️ Hardware

- **MCU**: STM32H743ZI (480MHz Cortex-M7, FPU)
- **Display**: Waveshare 4.2" E-ink (400×300)
- **Audio**: WM8960 Codec (24-bit, 192kHz)
- **Storage**: MicroSD card slot
- **Bluetooth**: ESP32-C3 module
- **Battery**: 18650 Li-ion (3000mAh)
- **Connectors**: USB-C (charge + data), 3.5mm audio jack

See [hardware/README.md](hardware/README.md) for schematics and BOM.

## 🔧 Mechanical

- Aluminum enclosure with matte black anodizing
- Tempered glass front panel
- Precision-machined control knobs
- Volume wheel with detents
- Magnetic battery cover

See [mechanical/README.md](mechanical/README.md) for 3D models.

## 🧪 Testing

**Test Coverage**: >85% (target)

- **Unit Tests**: Pure logic on host
- **Integration Tests**: Mock HAL
- **Simulator Tests**: Visual validation
- **HIL Tests**: Hardware validation

Run: `cargo test --workspace`

See [docs/TESTING.md](docs/TESTING.md) for details.

## 📊 Build Status

| Check | Status |
|-------|--------|
| Format | [![Format](https://github.com/yourusername/soulaudio-dap/workflows/Format/badge.svg)](https://github.com/yourusername/soulaudio-dap/actions) |
| Clippy | [![Clippy](https://github.com/yourusername/soulaudio-dap/workflows/Clippy/badge.svg)](https://github.com/yourusername/soulaudio-dap/actions) |
| Tests | [![Tests](https://github.com/yourusername/soulaudio-dap/workflows/Tests/badge.svg)](https://github.com/yourusername/soulaudio-dap/actions) |
| Build | [![Build](https://github.com/yourusername/soulaudio-dap/workflows/Build/badge.svg)](https://github.com/yourusername/soulaudio-dap/actions) |

## 🤝 Contributing

Contributions welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) first.

1. Fork the repository
2. Create feature branch (`git checkout -b feature/amazing-feature`)
3. Commit changes (`git commit -m 'Add amazing feature'`)
4. Push to branch (`git push origin feature/amazing-feature`)
5. Open Pull Request

## 📄 License

This project is dual-licensed under:
- MIT License ([LICENSE-MIT](LICENSE-MIT))
- Apache License 2.0 ([LICENSE-APACHE](LICENSE-APACHE))

Choose whichever license suits your needs.

## 🙏 Acknowledgments

- [Embassy](https://embassy.dev/) - Async embedded framework
- [embedded-hal](https://github.com/rust-embedded/embedded-hal) - Hardware abstraction
- [KiCad](https://www.kicad.org/) - PCB design
- [FreeCAD](https://www.freecad.org/) - Mechanical CAD
- Rust Embedded Community

## 📧 Contact

- Project: https://github.com/yourusername/soulaudio-dap
- Issues: https://github.com/yourusername/soulaudio-dap/issues
- Email: you@example.com

---

**Status**: 🚧 Active Development

Built with ❤️ using Rust and modern embedded practices.
