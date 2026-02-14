# xtask Usage Examples

This document provides practical examples of using the xtask commands in real development scenarios.

## Scenario 1: Starting a New Feature

You're starting work on a new UI feature for the music player.

```bash
# Start hot-reload development mode
cargo dev

# Edit files in crates/firmware/src/ui/...
# Emulator automatically rebuilds and restarts on save
# Iterate quickly on UI changes
```

**Expected output:**
```
═══════════════════════════════════════════
    🎵 SoulAudio DAP - Development Mode
═══════════════════════════════════════════

🚀 Starting hot-reload development mode
   Watching crates/firmware for changes...

🔨 Building emulator...
✓ Built in 2.34s
✓ Hot-reload active
  Save any .rs or .toml file to trigger rebuild
  Press Ctrl+C to stop

🎮 Emulator running...

📝 Changes detected!

🔨 Building emulator...
✓ Built in 1.12s

✓ Reload complete!
```

## Scenario 2: Testing on Hardware

You've completed a feature and want to test it on actual hardware.

```bash
# First, check that everything compiles
cargo run -p xtask -- check

# Flash debug build to hardware for testing
cargo flash

# Monitor RTT logs in separate terminal
probe-rs attach --chip STM32H743ZITx
```

**Expected output:**
```
🔍 Checking firmware builds...

  Checking hardware target (STM32H7)...
  ✓ Hardware check passed in 3.45s

  Checking emulator target (host)...
  ✓ Emulator check passed in 1.23s

  Checking platform crate (no_std)...
  ✓ Platform check passed in 0.89s

  Running clippy lints...
  ✓ Clippy passed in 2.11s

  Checking code formatting...
  ✓ Formatting check passed

✓ All checks completed in 7.68s

🔨 Building firmware (debug mode)...

✓ Build successful in 5.43s

📊 Binary size:
   .text    : 45KB
   .rodata  : 12KB
   .data    : 2KB
   .bss     : 8KB

📡 Flashing to STM32H7...
   Connecting to probe...
✓ Flash successful in 3.21s

🎵 SoulAudio DAP is running on hardware!
   Use 'probe-rs attach --chip STM32H743ZITx' to view RTT logs
```

## Scenario 3: Preparing for Release

You're preparing to create a release build for distribution.

```bash
# Run all checks
cargo run -p xtask -- check

# Run all tests
cargo run -p xtask -- test

# Flash optimized release build
cargo flash --release

# Test performance on hardware
# ...

# Generate documentation
cargo run -p xtask -- doc --open
```

**Test output:**
```
🧪 Running tests...

  Running unit tests...
  ✓ Unit tests passed ok. 42 passed; 0 failed; 0 ignored in 1.23s

  Running integration tests...
  ✓ Integration tests passed ok. 8 passed; 0 failed in 0.56s

  Running doc tests...
  ✓ Doc tests passed ok. 15 passed; 0 failed in 0.34s

✓ All tests completed in 2.13s
```

## Scenario 4: Debugging Build Errors

You've made changes and the build is failing.

```bash
# Start dev mode - it shows errors inline
cargo dev

# Edit files to fix errors
# Save - xtask shows the error details
# Fix and save again - automatically retries build
```

**Error output example:**
```
📝 Changes detected!

🔨 Building emulator...

✗ Build failed: Build failed

Build errors:
  error[E0308]: mismatched types
   --> crates/firmware/src/display.rs:45:20
    |
 45 |     let buffer = [0u8; 256];
    |                  ^^^^^^^^^^^ expected `u16`, found `u8`

   Fix the errors and save to trigger rebuild...
```

## Scenario 5: Working Without Hardware

You're developing on a laptop without the STM32H7 hardware.

```bash
# Use emulator exclusively
cargo dev

# Run checks (skips hardware-specific tests)
cargo run -p xtask -- check

# Run unit tests
cargo run -p xtask -- test --unit
```

## Scenario 6: CI/CD Pipeline

Automated testing and building in CI.

```bash
# Full check (run in CI)
cargo run -p xtask -- check

# All tests
cargo run -p xtask -- test

# Build release firmware
cargo build -p firmware --release --target thumbv7em-none-eabihf --features hardware

# Generate documentation
cargo run -p xtask -- doc
```

## Scenario 7: Quick Iteration Loop

Fast development loop for rapid prototyping.

```bash
# Terminal 1: Hot-reload mode
cargo dev

# Terminal 2: Watch logs (if needed)
tail -f emulator.log

# Edit, save, see changes in ~1-2 seconds
```

## Scenario 8: Testing Multiple Configurations

Testing different feature combinations.

```bash
# Check hardware build
cargo check -p firmware --target thumbv7em-none-eabihf --features hardware

# Check emulator build
cargo check -p firmware --features emulator

# Or use xtask to check both
cargo run -p xtask -- check
```

## Scenario 9: Documentation Generation

Creating API documentation for the project.

```bash
# Build docs
cargo run -p xtask -- doc

# Build and open in browser
cargo run -p xtask -- doc --open

# View specific crate docs
firefox target/doc/firmware/index.html
```

## Scenario 10: Debugging Flash Issues

Troubleshooting hardware flashing problems.

```bash
# Check probe connection
probe-rs list

# Flash with verbose output (use probe-rs directly)
probe-rs run --chip STM32H743ZITx target/thumbv7em-none-eabihf/debug/firmware

# Or use xtask (which wraps probe-rs)
cargo flash

# If flash fails, check:
# 1. Is probe connected? (probe-rs list)
# 2. Is board powered?
# 3. Is SWD cable seated properly?
# 4. Try different USB port
```

## Common Workflows

### Daily Development
```bash
# Start of day
cargo dev

# Edit code, save, repeat
# Emulator shows changes immediately
```

### Before Commit
```bash
# Run all checks
cargo run -p xtask -- check

# Run all tests
cargo run -p xtask -- test

# Format code
cargo fmt --all

# Commit
git add .
git commit -m "feat: add new UI feature"
```

### Hardware Testing Session
```bash
# Flash latest code
cargo flash

# Attach debugger in another terminal
probe-rs attach --chip STM32H743ZITx

# Test features on hardware
# Make changes based on results
# Reflash and retest
cargo flash
```

### Pre-Release Checklist
```bash
# 1. All checks pass
cargo run -p xtask -- check

# 2. All tests pass
cargo run -p xtask -- test

# 3. Release build succeeds
cargo flash --release

# 4. Documentation is up to date
cargo run -p xtask -- doc --open

# 5. Test on hardware thoroughly

# 6. Create release tag
git tag -a v0.1.0 -m "Release v0.1.0"
```

## Performance Tips

### Faster Builds
- Use `cargo dev` for development (incremental builds)
- Only use `--release` for final hardware testing
- Enable sccache for faster rebuilds: `export RUSTC_WRAPPER=sccache`

### Faster Flashing
- Use debug builds for iteration: `cargo flash` (not `--release`)
- probe-rs is faster than OpenOCD for STM32
- Keep probe firmware updated

### Faster Hot-Reload
- Only edit files in watched directories
- Save files individually (not "save all")
- Close unnecessary applications for more CPU
- Use SSD for faster rebuilds

## Troubleshooting

### "probe not found"
```bash
# Install probe-rs tools
cargo install probe-rs-tools

# Check probe connection
probe-rs list

# Try different USB port
```

### "target not found"
```bash
# Install target
rustup target add thumbv7em-none-eabihf

# Verify installation
rustup target list --installed
```

### Build errors in dev mode
- Check terminal output for error details
- Fix errors and save - xtask automatically retries
- If stuck, restart: Ctrl+C and `cargo dev` again

### Emulator doesn't start
```bash
# Build emulator manually to see errors
cargo build --example display_emulator_test -p firmware --features emulator

# Run manually
cargo run --example display_emulator_test -p firmware --features emulator
```

### Hot-reload not detecting changes
- Ensure you're editing files in watched directories:
  - `crates/firmware/src`
  - `crates/firmware/Cargo.toml`
  - `crates/platform`
- Check file system permissions
- Try saving the file again

## Advanced Usage

### Custom probe-rs Configuration

Edit `crates/firmware/Embed.toml` to customize flashing:

```toml
[default.probe]
speed = 8000  # Faster SWD speed (4000 default)

[default.rtt]
enabled = true  # Enable RTT logging
show_timestamps = true
```

### Running Specific Tests
```bash
# Only unit tests
cargo run -p xtask -- test --unit

# Only integration tests
cargo run -p xtask -- test --integration

# Specific test
cargo test -p firmware test_name
```

### Building for Different Targets
```bash
# Hardware (STM32H7)
cargo build -p firmware --target thumbv7em-none-eabihf --features hardware

# Emulator (host)
cargo build -p firmware --features emulator

# Platform library (no_std check)
cargo build -p platform --target thumbv7em-none-eabihf --no-default-features
```

### Headless Development
```bash
# Build without opening window
cargo dev --headless

# Useful for:
# - CI/CD pipelines
# - Remote development
# - Build verification
```

## Integration with IDEs

### VS Code
Add tasks to `.vscode/tasks.json`:
```json
{
  "version": "2.0.0",
  "tasks": [
    {
      "label": "Flash Debug",
      "type": "shell",
      "command": "cargo flash",
      "problemMatcher": "$rustc"
    },
    {
      "label": "Hot Reload",
      "type": "shell",
      "command": "cargo dev",
      "isBackground": true
    }
  ]
}
```

### CLion/IntelliJ IDEA
Add run configurations:
- Name: Flash Debug
- Command: `cargo flash`
- Working directory: `$ProjectFileDir$`

## Environment Variables

### Build Configuration
```bash
# Enable logging
export DEFMT_LOG=debug

# Use sccache for faster builds
export RUSTC_WRAPPER=sccache
```

### xtask Configuration
No environment variables currently supported. All configuration is via command-line flags.

## Summary

The xtask commands streamline the SoulAudio DAP development workflow:

- **`cargo dev`** - Fast iteration with hot-reload
- **`cargo flash`** - Easy hardware deployment
- **`cargo run -p xtask -- check`** - Pre-commit validation
- **`cargo run -p xtask -- test`** - Comprehensive testing
- **`cargo run -p xtask -- doc`** - Documentation generation

These tools make it easy to switch between emulator development and hardware testing, ensuring a smooth development experience.
