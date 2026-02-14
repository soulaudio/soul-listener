# Changelog

All notable changes to the xtask development tools will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-02-14

### Added

#### Commands
- `flash` - Flash firmware to STM32H7 hardware via probe-rs
  - Debug build with `cargo flash`
  - Release build with `cargo flash --release`
  - Shows binary size information
  - Displays timing metrics
  - Colored status output

- `dev` - Hot-reload development mode
  - Automatic rebuild on file changes
  - Emulator restart on rebuild
  - Headless mode support with `--headless`
  - ~1-2 second reload time
  - File watching with debouncing (200ms)
  - Clear screen on reload for clean output
  - Watches `crates/firmware/src`, `crates/platform`, and `*.toml` files

- `check` - Comprehensive build verification
  - Hardware target check (thumbv7em-none-eabihf)
  - Emulator target check (host)
  - Platform crate check (no_std)
  - Clippy lints
  - Code formatting check
  - Timing for each operation

- `test` - Test suite runner
  - All tests (unit, integration, doc)
  - `--unit` flag for unit tests only
  - `--integration` flag for integration tests only
  - Test summary parsing
  - Timing information

- `doc` - Documentation builder
  - Build workspace documentation
  - `--open` flag to open in browser
  - Includes private items
  - Shows build timing

#### Features
- Cargo aliases for common commands
  - `cargo flash` → `cargo run -p xtask -- flash`
  - `cargo dev` → `cargo run -p xtask -- dev`
- Cross-platform file watching (Windows, Linux, macOS)
- Colored terminal output with status indicators
- Unicode symbols for better UX (✓, ✗, ⚠, 🔨, 📡, 🎵)
- Proper error handling with anyhow
- Clear, actionable error messages
- Build timing information
- Binary size reporting (flash command)
- Process cleanup on Ctrl+C
- Debounced file watching to prevent multiple rebuilds

#### Documentation
- Comprehensive README with installation and usage
- USAGE_EXAMPLES.md with 10 practical scenarios
- QUICK_REFERENCE.md command cheat sheet
- TEST_XTASK.md testing guide
- IMPLEMENTATION_SUMMARY.md technical documentation
- Inline code comments
- Help text for all commands and options

#### Infrastructure
- Workspace integration
- Cargo alias configuration
- Optional desktop notifications (with `notifications` feature)
- Proper exit codes (0 for success, non-zero for errors)
- Clean modular code structure
- Each command in separate module

### Dependencies
- clap 4.5 - CLI argument parsing
- anyhow 1.0 - Error handling
- notify 6.1 - File watching
- colored 2.1 - Terminal colors
- notify-rust 4.11 - Desktop notifications (optional)

### Technical Details
- Source: 6 modules, ~24 KB
- Documentation: 5 files, ~31 KB
- Total: 12 files
- Zero unsafe code
- No unwrap() in production paths
- Comprehensive error handling

### Known Limitations
- Flash command requires probe-rs-tools installed
- Flash command requires STM32H7 hardware connected
- Dev mode file watching limited to specific directories
- No Windows terminal color support in older cmd.exe (works in PowerShell/Windows Terminal)

### Platform Support
- ✅ Windows - Tested and working
- ✅ Linux - File watching via inotify
- ✅ macOS - File watching via FSEvents

## [Unreleased]

### Future Enhancements (Planned)
- Binary size analysis with cargo-bloat
- Performance profiling with perf/flamegraph
- Code coverage reports
- Release automation
- Benchmark runner
- Progress bars for long operations
- Parallel build support
- Custom build profiles
- Automatic changelog generation

### Potential Improvements
- Caching for faster rebuilds
- Incremental flashing
- Remote debugging support
- Serial monitor integration
- Multiple probe support
- Custom flash memory regions
- Bootloader support
- OTA update support

---

## Version History

### [0.1.0] - 2026-02-14
Initial release with core commands:
- flash (debug/release)
- dev (hot-reload)
- check (multi-target)
- test (all suites)
- doc (with --open)

---

## Migration Guide

### From Manual Commands

**Before:**
```bash
# Manual flash
cargo build -p firmware --target thumbv7em-none-eabihf --features hardware --release
probe-rs run --chip STM32H743ZITx target/thumbv7em-none-eabihf/release/firmware

# Manual emulator
cargo run --example display_emulator_test -p firmware --features emulator

# Manual checks
cargo check -p firmware --target thumbv7em-none-eabihf --features hardware
cargo check -p firmware --features emulator
cargo clippy --workspace
```

**After:**
```bash
# Flash
cargo flash --release

# Hot-reload emulator
cargo dev

# Check everything
cargo run -p xtask -- check
```

### From Old Aliases

If you had custom aliases in `.cargo/config.toml`, replace with xtask:

**Before:**
```toml
flash = "embed --release"
```

**After:**
```toml
flash = "run -p xtask -- flash"
```

The xtask version provides:
- Better error messages
- Timing information
- Binary size reporting
- Colored output

## Breaking Changes

None - this is the initial release.

## Security

- No known security issues
- No network operations
- No file operations outside project directory
- All dependencies from crates.io with version pins

## Performance

| Operation | Time (approx) |
|-----------|---------------|
| xtask startup | ~0.3s |
| Hot-reload rebuild | ~1-2s |
| Full debug build | ~5-10s |
| Full release build | ~15-30s |
| Flash time | ~2-5s |
| Check all | ~5-10s |

## Compatibility

### Rust Version
- Minimum: 1.75 (Rust 2021 edition)
- Tested: 1.75+
- Recommended: Latest stable

### probe-rs Version
- Minimum: 0.20
- Tested: Latest
- Install: `cargo install probe-rs-tools`

### Target Support
- Primary: thumbv7em-none-eabihf (STM32H7)
- Emulator: Host platform (x86_64-pc-windows-msvc, etc.)

## Contributors

Initial implementation: SoulAudio DAP Team

## License

Same as parent project: MIT OR Apache-2.0

---

**Note:** This changelog tracks changes to the xtask development tools specifically, not the main SoulAudio DAP firmware.
