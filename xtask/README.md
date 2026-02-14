# xtask - SoulAudio Development Tools

Custom development tools for the SoulAudio Digital Audio Player project.

## Installation

The xtask commands are automatically available through cargo aliases defined in `.cargo/config.toml`. No separate installation is needed.

## Commands

### Flash Firmware to Hardware

Flash the firmware to STM32H7 hardware via probe-rs:

```bash
# Flash debug build (faster compilation, includes debug symbols)
cargo flash

# Flash release build (optimized for size and performance)
cargo flash --release
```

**What it does:**
1. Builds firmware for `thumbv7em-none-eabihf` target with `hardware` features
2. Displays binary size information
3. Flashes to STM32H7 via probe-rs (SWD interface)
4. Shows flash time and success status

**Requirements:**
- probe-rs tools installed: `cargo install probe-rs-tools`
- STM32H7 board connected via SWD/JTAG probe
- Proper probe configuration in `Embed.toml`

### Hot-Reload Development Mode

Run the emulator with automatic rebuilding on file changes:

```bash
# Run emulator with hot-reload
cargo dev

# Run in headless mode (no window, build-only)
cargo dev --headless
```

**What it does:**
1. Builds and runs the emulator (`display_emulator_test` example)
2. Watches `crates/firmware/src` for file changes
3. Automatically rebuilds and restarts emulator when files are saved
4. Shows build time and status
5. Clears screen for clean output on each reload

**Features:**
- ~1-2 second reload time
- Debounced file watching (200ms) to avoid duplicate rebuilds
- Shows build errors inline
- Cross-platform (Windows, Linux, macOS)
- Colored terminal output
- Optional desktop notifications (with `notifications` feature)

### Check All Builds

Verify that the code compiles for all targets:

```bash
cargo run -p xtask -- check
```

**What it does:**
1. Checks hardware target (STM32H7, `thumbv7em-none-eabihf`)
2. Checks emulator target (host platform)
3. Checks platform crate for no_std compatibility
4. Runs clippy lints
5. Checks code formatting

This is useful before committing to ensure compatibility across all targets.

### Run Tests

Execute the test suite:

```bash
# Run all tests (unit, integration, doc)
cargo run -p xtask -- test

# Run only unit tests
cargo run -p xtask -- test --unit

# Run only integration tests
cargo run -p xtask -- test --integration
```

**What it does:**
1. Runs unit tests (`cargo test --lib --workspace`)
2. Runs integration tests (if available)
3. Runs doc tests
4. Shows test summary and timing

### Build Documentation

Generate and view project documentation:

```bash
# Build documentation
cargo run -p xtask -- doc

# Build and open in browser
cargo run -p xtask -- doc --open
```

**What it does:**
1. Generates documentation for all workspace crates
2. Includes private items for complete API docs
3. Optionally opens in default browser

## Cargo Aliases

For convenience, common commands have short aliases in `.cargo/config.toml`:

```toml
[alias]
flash = "run -p xtask -- flash"
dev = "run -p xtask -- dev"
```

This allows you to use:
- `cargo flash` instead of `cargo run -p xtask -- flash`
- `cargo dev` instead of `cargo run -p xtask -- dev`

## Hot-Reload Workflow

Recommended development workflow:

1. **Start hot-reload mode:**
   ```bash
   cargo dev
   ```

2. **Edit code** in `crates/firmware/src` or `crates/platform`

3. **Save file** - emulator automatically rebuilds and restarts

4. **See changes immediately** in the emulator window

5. **Fix any build errors** shown in terminal

6. **Repeat** - the cycle is ~1-2 seconds per iteration

## Flashing Workflow

When you're ready to test on hardware:

1. **Connect STM32H7 board** via SWD/JTAG probe

2. **Flash debug build** for faster iteration:
   ```bash
   cargo flash
   ```

3. **View RTT logs** (defmt output):
   ```bash
   probe-rs attach --chip STM32H743ZITx
   ```

4. **Flash release build** for performance testing:
   ```bash
   cargo flash --release
   ```

## Dependencies

The xtask crate uses:

- **clap** - Command-line argument parsing
- **anyhow** - Error handling
- **notify** - Cross-platform file watching
- **colored** - Terminal color output
- **notify-rust** (optional) - Desktop notifications

## Features

### Default Features

Standard features enabled by default.

### Optional Features

#### `notifications`

Enable desktop notifications on rebuild completion:

```bash
cargo run -p xtask --features notifications -- dev
```

This shows a system notification when hot-reload completes, useful when your editor is in fullscreen.

## Troubleshooting

### Flash fails with "probe not found"

**Solution:** Check that probe-rs is installed and the probe is connected:
```bash
cargo install probe-rs-tools
probe-rs list
```

### Hot-reload doesn't detect changes

**Solution:** Ensure you're editing files in the watched directories:
- `crates/firmware/src`
- `crates/firmware/Cargo.toml`
- `crates/platform`

### Build errors don't clear

**Solution:** The terminal shows all output. If you want a clean view, restart `cargo dev`.

### Emulator window doesn't appear

**Solution:** Check that the emulator example builds:
```bash
cargo build --example display_emulator_test -p firmware --features emulator
```

## Performance Tips

- **Use debug builds** for fast iteration: `cargo flash` (not `--release`)
- **Use hot-reload** for UI development: `cargo dev`
- **Use release builds** for performance testing: `cargo flash --release`
- **Run checks before commit**: `cargo run -p xtask -- check`

## Cross-Platform Notes

### Windows

- Uses Windows notification system (with `notifications` feature)
- File watching works with all common editors

### Linux

- Requires D-Bus for notifications (with `notifications` feature)
- File watching uses inotify (kernel support)

### macOS

- Uses macOS notification center (with `notifications` feature)
- File watching uses FSEvents (macOS native)

## Contributing

When adding new xtask commands:

1. Create a new module in `xtask/src/`
2. Add the command to the `Commands` enum in `main.rs`
3. Implement error handling with `anyhow`
4. Use `colored` for user-friendly output
5. Add timing information with `std::time::Instant`
6. Update this README with usage examples

## License

Same as the parent project (MIT OR Apache-2.0).
