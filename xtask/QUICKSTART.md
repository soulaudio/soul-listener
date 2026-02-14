# xtask Quick Start Guide

Get started with SoulAudio DAP development in 3 minutes.

## Installation

The xtask commands are already installed! They're part of the workspace.

## Basic Usage

### 🚀 Start Developing

```bash
# Start hot-reload development mode
cargo dev
```

This will:
1. Build the emulator
2. Open the emulator window
3. Watch for file changes
4. Auto-rebuild and restart when you save

**That's it!** Edit code in `crates/firmware/src/` and see changes in ~1-2 seconds.

### 📱 Flash to Hardware

```bash
# Flash debug build (fast, for testing)
cargo flash

# Flash release build (optimized, for performance testing)
cargo flash --release
```

Requires:
- STM32H743ZI board connected via SWD
- probe-rs-tools installed: `cargo install probe-rs-tools`

### ✅ Verify Everything Works

```bash
# Check all builds and run lints
cargo run -p xtask -- check
```

This checks:
- Hardware target builds
- Emulator target builds
- Platform crate (no_std)
- Clippy lints
- Code formatting

## Development Workflow

### Daily Development Loop

```bash
# 1. Start hot-reload
cargo dev

# 2. Edit code in your IDE
# Files in: crates/firmware/src/

# 3. Save - emulator auto-reloads
# See changes in ~1-2 seconds

# 4. When done, press Ctrl+C
```

### Testing on Hardware

```bash
# 1. Connect STM32H7 board via SWD

# 2. Flash firmware
cargo flash

# 3. In another terminal, view logs
probe-rs attach --chip STM32H743ZITx

# 4. Test on hardware

# 5. Make changes, reflash
cargo flash
```

### Before Committing

```bash
# Run all checks
cargo run -p xtask -- check

# Run all tests
cargo run -p xtask -- test

# Format code
cargo fmt --all

# Ready to commit!
git add .
git commit -m "your message"
```

## All Commands

| Command | What it does |
|---------|--------------|
| `cargo dev` | Hot-reload development |
| `cargo flash` | Flash to hardware |
| `cargo run -p xtask -- check` | Verify builds |
| `cargo run -p xtask -- test` | Run tests |
| `cargo run -p xtask -- doc --open` | Generate docs |

## Help

Get help for any command:

```bash
cargo run -p xtask -- --help           # All commands
cargo run -p xtask -- flash --help     # Flash options
cargo run -p xtask -- dev --help       # Dev options
```

## Troubleshooting

### "probe not found" when flashing

**Problem:** No STM32H7 board detected

**Solution:**
```bash
# Install probe-rs
cargo install probe-rs-tools

# Check probe is connected
probe-rs list
```

### Hot-reload not working

**Problem:** Changes not detected

**Solution:**
- Make sure you're editing files in `crates/firmware/src/`
- Try saving the file again
- Restart: Ctrl+C and `cargo dev` again

### Build errors

**Problem:** Code doesn't compile

**Solution:**
- Check terminal for error details
- Fix the code
- Save - xtask automatically retries build
- No need to restart xtask

## Next Steps

1. **Read full docs:** See `xtask/README.md`
2. **Check examples:** See `xtask/USAGE_EXAMPLES.md`
3. **Quick reference:** See `xtask/QUICK_REFERENCE.md`

## Tips

1. **Keep cargo dev running** - Don't restart for every change
2. **Use debug builds** during development - Much faster
3. **Only use --release** for final testing
4. **Run check before commit** - Catches issues early
5. **Use probe-rs attach** to see defmt logs from hardware

## Example Session

Here's a typical development session:

```bash
# Terminal 1: Start hot-reload
$ cargo dev
═══════════════════════════════════════════
    🎵 SoulAudio DAP - Development Mode
═══════════════════════════════════════════

🚀 Starting hot-reload development mode
🔨 Building emulator...
✓ Built in 2.34s
✓ Hot-reload active
🎮 Emulator running...

📝 Changes detected!
🔨 Building emulator...
✓ Built in 1.12s
✓ Reload complete!
```

```bash
# Terminal 2: Run checks when ready
$ cargo run -p xtask -- check
🔍 Checking firmware builds...
✓ All checks completed in 7.68s

$ cargo run -p xtask -- test
🧪 Running tests...
✓ All tests completed in 2.13s

$ git add .
$ git commit -m "feat: add new UI feature"
```

## That's It!

You're ready to develop! Start with:

```bash
cargo dev
```

Happy coding! 🎵

---

**For more information:**
- Full documentation: `xtask/README.md`
- Command reference: `xtask/QUICK_REFERENCE.md`
- Usage examples: `xtask/USAGE_EXAMPLES.md`
- Testing guide: `xtask/TEST_XTASK.md`
