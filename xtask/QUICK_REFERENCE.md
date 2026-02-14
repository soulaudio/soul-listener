# xtask Quick Reference

## Essential Commands

| Command | Description | Use Case |
|---------|-------------|----------|
| `cargo dev` | Hot-reload emulator | Daily development |
| `cargo flash` | Flash debug build | Hardware testing |
| `cargo flash --release` | Flash optimized build | Performance testing |
| `cargo run -p xtask -- check` | Check all builds | Pre-commit |
| `cargo run -p xtask -- test` | Run all tests | CI/CD |

## Command Details

### Development Mode
```bash
cargo dev                    # Hot-reload with window
cargo dev --headless         # Build-only, no window
```

**Watches:** `crates/firmware/src`, `crates/platform`, `*.toml`
**Rebuild time:** ~1-2 seconds
**Exit:** Ctrl+C

### Flash Firmware
```bash
cargo flash                  # Flash debug build
cargo flash --release        # Flash release build
```

**Target:** STM32H743ZITx
**Interface:** SWD via probe-rs
**Requires:** probe-rs-tools, connected probe

### Check Builds
```bash
cargo run -p xtask -- check
```

**Checks:**
- ✅ Hardware target (thumbv7em-none-eabihf)
- ✅ Emulator target (host)
- ✅ Platform crate (no_std)
- ✅ Clippy lints
- ✅ Code formatting

### Run Tests
```bash
cargo run -p xtask -- test              # All tests
cargo run -p xtask -- test --unit       # Unit only
cargo run -p xtask -- test --integration # Integration only
```

**Test types:**
- Unit tests (lib tests)
- Integration tests
- Doc tests

### Build Documentation
```bash
cargo run -p xtask -- doc              # Build docs
cargo run -p xtask -- doc --open       # Build + open
```

**Output:** `target/doc/firmware/index.html`

## Typical Workflows

### Daily Development
```bash
cargo dev               # Start hot-reload
# Edit code → Auto-rebuild → See changes
# Ctrl+C when done
```

### Hardware Testing
```bash
cargo flash            # Flash to hardware
probe-rs attach --chip STM32H743ZITx  # View logs
```

### Pre-Commit
```bash
cargo run -p xtask -- check
cargo run -p xtask -- test
cargo fmt --all
git commit
```

### Release Build
```bash
cargo run -p xtask -- check
cargo run -p xtask -- test
cargo flash --release
# Test on hardware
```

## File Locations

| Item | Path |
|------|------|
| xtask source | `xtask/src/` |
| Firmware | `crates/firmware/` |
| Platform | `crates/platform/` |
| Build output | `target/` |
| Documentation | `target/doc/` |

## Keyboard Shortcuts

| Action | Key |
|--------|-----|
| Stop xtask | Ctrl+C |
| Clear terminal | Ctrl+L |

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | Build/flash/test failed |
| 101 | Runtime error |

## Troubleshooting

### Problem: "probe not found"
**Solution:**
```bash
cargo install probe-rs-tools
probe-rs list
```

### Problem: Build fails in dev mode
**Solution:**
- Check terminal for errors
- Fix code and save
- Xtask auto-retries

### Problem: Hot-reload not working
**Solution:**
- Edit files in watched directories
- Check file permissions
- Restart: Ctrl+C, `cargo dev`

### Problem: Emulator window doesn't appear
**Solution:**
```bash
cargo build --example display_emulator_test -p firmware --features emulator
cargo run --example display_emulator_test -p firmware --features emulator
```

## Requirements

### Software
- Rust toolchain (1.75+)
- probe-rs-tools (`cargo install probe-rs-tools`)
- thumbv7em-none-eabihf target (`rustup target add thumbv7em-none-eabihf`)

### Hardware (for flashing)
- STM32H743ZI board
- SWD/JTAG probe (ST-Link, J-Link, etc.)
- USB cable

### Optional
- sccache (faster builds)
- cargo-bloat (size analysis)
- cargo-expand (macro expansion)

## Tips

1. **Fast iteration:** Use `cargo dev` for UI work
2. **Debug builds:** Use `cargo flash` (not `--release`) during development
3. **Check before commit:** Run `cargo run -p xtask -- check`
4. **Monitor logs:** Use `probe-rs attach` in separate terminal
5. **Incremental builds:** Keep dev session running, don't restart

## Performance

| Operation | Time |
|-----------|------|
| Hot-reload rebuild | ~1-2s |
| Full debug build | ~5-10s |
| Full release build | ~15-30s |
| Flash time | ~2-5s |
| Check all | ~5-10s |
| All tests | ~2-5s |

*Times are approximate and depend on your system*

## Color Guide

- 🟢 Green: Success
- 🟡 Yellow: Warning
- 🔴 Red: Error
- 🔵 Cyan: Info
- ⚪ Gray: Verbose/Details

## Examples

### Example 1: Start Development
```bash
$ cargo dev
═══════════════════════════════════════════
    🎵 SoulAudio DAP - Development Mode
═══════════════════════════════════════════

🚀 Starting hot-reload development mode
🔨 Building emulator...
✓ Built in 2.34s
✓ Hot-reload active
🎮 Emulator running...
```

### Example 2: Flash Firmware
```bash
$ cargo flash --release

🔨 Building firmware (release mode)...
✓ Build successful in 23.45s

📊 Binary size:
   .text    : 45KB
   .rodata  : 12KB

📡 Flashing to STM32H7...
✓ Flash successful in 3.21s

🎵 SoulAudio DAP is running on hardware!
```

### Example 3: Run Checks
```bash
$ cargo run -p xtask -- check

🔍 Checking firmware builds...

  Checking hardware target (STM32H7)...
  ✓ Hardware check passed in 3.45s

  Checking emulator target (host)...
  ✓ Emulator check passed in 1.23s

✓ All checks completed in 7.68s
```

## Links

- Full documentation: `xtask/README.md`
- Usage examples: `xtask/USAGE_EXAMPLES.md`
- Project docs: `CLAUDE.md`

## Version

xtask v0.1.0 - SoulAudio DAP Development Tools

---

**Quick Start:** Run `cargo dev` to start hot-reload development!
