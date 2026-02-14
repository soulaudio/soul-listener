# xtask Testing Guide

This document describes how to test the xtask commands to ensure they work correctly.

## Manual Testing Checklist

### ✅ 1. Test Help Commands

```bash
# Test main help
cargo run -p xtask -- --help
# Expected: Shows all commands (flash, dev, check, test, doc)

# Test subcommand help
cargo run -p xtask -- flash --help
cargo run -p xtask -- dev --help
cargo run -p xtask -- check --help
cargo run -p xtask -- test --help
cargo run -p xtask -- doc --help
# Expected: Each shows appropriate options
```

### ✅ 2. Test Cargo Aliases

```bash
# Test flash alias
cargo flash --help
# Expected: Shows flash command help

# Test dev alias
cargo dev --help
# Expected: Shows dev command help
```

### ✅ 3. Test Check Command

```bash
cargo run -p xtask -- check
# Expected:
# - Checks hardware target
# - Checks emulator target
# - Checks platform crate
# - Runs clippy
# - Checks formatting
# - Shows timing for each step
# - Green success messages if all pass
```

### ✅ 4. Test Doc Command

```bash
# Build docs
cargo run -p xtask -- doc
# Expected:
# - Builds documentation
# - Shows build time
# - Tells user where to find docs

# Build and open docs (if you have a browser)
cargo run -p xtask -- doc --open
# Expected:
# - Builds documentation
# - Opens in default browser
```

### ✅ 5. Test Test Command

```bash
# All tests
cargo run -p xtask -- test
# Expected:
# - Runs unit tests
# - Runs integration tests (if available)
# - Runs doc tests
# - Shows test summary

# Unit tests only
cargo run -p xtask -- test --unit
# Expected:
# - Runs only unit tests
# - Skips integration tests

# Integration tests only
cargo run -p xtask -- test --integration
# Expected:
# - Runs only integration tests
# - Skips unit tests
```

### ✅ 6. Test Dev Command (Without Running)

Since this spawns a long-running process, we'll test the build part:

```bash
# Build the emulator example to verify it works
cargo build --example display_emulator_test -p firmware --features emulator
# Expected:
# - Builds successfully
# - No errors

# Test headless mode (if brave enough to run it)
# Note: You'll need to Ctrl+C to stop it
# cargo dev --headless
```

### ✅ 7. Test Flash Command (Requires Hardware)

**⚠️ Only if you have STM32H7 hardware connected!**

```bash
# Test debug flash
cargo flash
# Expected:
# - Builds firmware for thumbv7em-none-eabihf
# - Shows binary size
# - Attempts to flash via probe-rs
# - Shows success or error message

# Test release flash
cargo flash --release
# Expected:
# - Builds release firmware
# - Shows binary size (should be smaller)
# - Flashes to hardware
```

## Automated Testing

### Build Test

Test that xtask itself compiles:

```bash
cargo build -p xtask
cargo check -p xtask
```

### Clippy Test

Test that xtask passes clippy:

```bash
cargo clippy -p xtask -- -D warnings
```

### Format Test

Test that xtask is properly formatted:

```bash
cargo fmt -p xtask --check
```

## Integration Testing

### Test Workspace Integration

```bash
# Verify xtask is in workspace
cargo metadata --format-version 1 | grep -q "xtask"

# Verify all workspace members compile
cargo check --workspace
```

### Test Alias Configuration

```bash
# Check that aliases are defined
grep -A 2 "\[alias\]" .cargo/config.toml | grep "flash"
grep -A 2 "\[alias\]" .cargo/config.toml | grep "dev"
```

## Error Testing

### Test Error Handling

```bash
# Test invalid command
cargo run -p xtask -- invalid
# Expected: Shows help and error message

# Test invalid option
cargo run -p xtask -- flash --invalid
# Expected: Shows error about unknown option
```

### Test Build Failure Handling

Create a temporary syntax error in firmware:

```bash
# Make a backup
cp crates/firmware/src/lib.rs crates/firmware/src/lib.rs.bak

# Introduce an error (add invalid syntax)
echo "invalid rust syntax" >> crates/firmware/src/lib.rs

# Test that xtask handles the error gracefully
cargo run -p xtask -- check
# Expected:
# - Shows build error
# - Displays error message
# - Exits with non-zero code

# Restore the file
mv crates/firmware/src/lib.rs.bak crates/firmware/src/lib.rs
```

## Performance Testing

### Measure Build Times

```bash
# Measure check command time
time cargo run -p xtask -- check

# Measure doc command time
time cargo run -p xtask -- doc

# Measure test command time
time cargo run -p xtask -- test
```

### Measure Hot-Reload Time

```bash
# Start dev mode
cargo dev

# In another terminal, touch a file and measure time
touch crates/firmware/src/lib.rs

# Observe rebuild time in dev terminal
# Expected: ~1-2 seconds
```

## Cross-Platform Testing

### Windows
```powershell
# Test on Windows (PowerShell)
cargo run -p xtask -- check
cargo run -p xtask -- test

# Test colored output
cargo run -p xtask -- check | Select-String "✓"
```

### Linux
```bash
# Test on Linux
cargo run -p xtask -- check
cargo run -p xtask -- test

# Test colored output
cargo run -p xtask -- check | grep --color=always "✓"
```

### macOS
```bash
# Test on macOS
cargo run -p xtask -- check
cargo run -p xtask -- test

# Test colored output
cargo run -p xtask -- check | grep "✓"
```

## Regression Testing

After making changes to xtask, verify:

1. All commands still work
2. Error messages are clear
3. Timing information is shown
4. Colored output works
5. Exit codes are correct (0 for success, non-zero for failure)

### Test Script

Create a test script `test_xtask.sh`:

```bash
#!/bin/bash

set -e

echo "Testing xtask commands..."

echo "1. Testing help"
cargo run -p xtask -- --help > /dev/null

echo "2. Testing check"
cargo run -p xtask -- check

echo "3. Testing doc"
cargo run -p xtask -- doc

echo "4. Testing test"
cargo run -p xtask -- test

echo "5. Testing aliases"
cargo flash --help > /dev/null
cargo dev --help > /dev/null

echo "✅ All xtask tests passed!"
```

Run with:
```bash
chmod +x test_xtask.sh
./test_xtask.sh
```

## Expected Outputs

### Successful Check Output

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
```

### Successful Test Output

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

### Successful Doc Output

```
📚 Building documentation...

✓ Documentation built in 4.56s

   Open target/doc/firmware/index.html in your browser
   Or run 'cargo run -p xtask -- doc --open'
```

## Troubleshooting Tests

### "error: package `xtask` not found"

Solution: Ensure xtask is in workspace members in root Cargo.toml

### "error: no such subcommand: `flash`"

Solution: Check .cargo/config.toml has the aliases

### "probe not found" during flash test

Solution: Expected if no hardware connected. Skip flash test or connect hardware.

### Tests hang or timeout

Solution: Check for infinite loops, ensure processes are killed properly

## CI/CD Testing

Example GitHub Actions workflow:

```yaml
name: Test xtask

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
          target: thumbv7em-none-eabihf
      - name: Test xtask check
        run: cargo run -p xtask -- check
      - name: Test xtask test
        run: cargo run -p xtask -- test
      - name: Test xtask doc
        run: cargo run -p xtask -- doc
```

## Summary

To fully test xtask:

1. ✅ Run `cargo run -p xtask -- check`
2. ✅ Run `cargo run -p xtask -- test`
3. ✅ Run `cargo run -p xtask -- doc`
4. ✅ Test aliases: `cargo flash --help`, `cargo dev --help`
5. ✅ Verify colored output appears
6. ✅ Check error handling with invalid commands
7. ✅ (Optional) Test `cargo flash` with hardware
8. ✅ (Optional) Test `cargo dev` for hot-reload

All commands should show clear output with colored status indicators (✓ green, ✗ red, ⚠ yellow).
