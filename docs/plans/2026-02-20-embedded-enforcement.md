# Embedded Enforcement — Workspace Lints & Safety Invariants

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add five new enforcement layers to eliminate unsafe patterns that compile silently
but cause embedded firmware failures: panic-on-unwrap, oversized stack frames, misplaced DMA
buffers, heap allocation, and debug-format in production code.

**Architecture:** All lints are promoted to `[workspace.lints.clippy]` so every crate inherits
them. Desktop crates (`eink-emulator`, `eink-testing`, `xtask`) get file-level `#![allow]`
because those patterns are acceptable on the host. Hardware crates get per-site `#[allow]` with
`SAFETY:` comments at the rare legitimate uses. Arch tests use `include_str!` to verify source
invariants without running firmware. CI checks extend the binary-size job to detect heap
allocation via `arm-none-eabi-nm`.

**Tech Stack:** Rust 1.85, `cargo clippy`, `clippy.toml`, `arm-none-eabi-nm`, GitHub Actions CI.

---

## Research Summary (pre-plan exploration)

### Current unwrap/expect/panic enforcement

All 12 workspace crates have `[lints] workspace = true` in their `Cargo.toml`, so adding to
`[workspace.lints.clippy]` immediately covers every crate.

Per-crate `#![deny]` that will become redundant once workspace lints are added:
- `crates/bluetooth/src/lib.rs` lines 6-8: deny unwrap_used, panic, expect_used
- `crates/eink/eink-specs/src/lib.rs` lines 58-60: deny unwrap_used, expect_used, panic
- `crates/firmware/src/lib.rs` lines 40-41: deny unwrap_used, expect_used
- `crates/library/src/lib.rs` lines 11-13: deny unwrap_used, panic, expect_used
- `crates/platform/src/lib.rs` lines 51-53: deny unwrap_used, expect_used, panic
- `crates/playback/src/lib.rs` lines 3-5: deny unwrap_used, panic, expect_used
- `crates/ui/src/lib.rs` lines 6-8: deny unwrap_used, panic, expect_used

Crates with NO per-crate deny (will need fixes or file-level allow):
- `eink-emulator`: 78 unwraps, 6 expects — desktop only → `#![allow(clippy::unwrap_used, ...)]`
- `eink-system`: unwraps/expects all inside `#[cfg(test)]` blocks → per-module `#[allow]`
- `eink-components`: needs checking → likely allow at lib root (desktop/test crate)
- `eink-testing`: needs checking → likely allow at lib root (test helper crate)
- `firmware-ui`: needs checking → likely allow at lib root (UI dev crate)
- `xtask`: build tool → allow at lib root

### use_debug violations in builder.rs

Three `{:?}` format uses in `crates/firmware/src/input/builder.rs` inside
`#[cfg(feature = "keyboard-input")]`:
- Line 275: `format!("{:?}", a)` where `a: EmulatedAxis`
- Line 285: `format!("{:?}", k)` where `k: EmulatedKey`
- Line 288: `eprintln!("[InputBuilder] Button::{:?} → ...")` where `btn: Button`

Fix: replace with match arms returning `&'static str` for `EmulatedAxis` and `EmulatedKey`;
for `Button`, use `platform::button_name(btn)` helper or a local match.

### clippy.toml threshold

`clippy.toml` already exists and defines `enum-variant-size-threshold = 512`. Add
`array-size-threshold = 512` (512 bytes → 128 × u32 or 256 × u16 or 512 bytes).
The existing per-site `#[allow(clippy::large_stack_arrays)]` in
`crates/firmware/src/display/driver.rs` stays as-is (legitimate DMA staging buffer).

### memory_safety.rs current tests

`crates/firmware/tests/memory_safety.rs` has 4 tests:
1. `framebuffer_uses_static_cell_with_axisram_link_section`
2. `firmware_has_runtime_dma_buffer_address_assertion`
3. `watchdog_heartbeat_uses_correct_atomic_ordering`
4. `no_static_mut_with_link_section_in_firmware`

New tests added in Task 3 check that:
- Every `#[link_section = ".axisram"]` static uses `Align32` wrapper
- Every `StaticCell` containing a large `[u8; N]` (N ≥ 4096) also uses `Align32`
- Audio SAI DMA ping-pong (when added) must be in `.axisram` too

### no-malloc approach

`arm-none-eabi-nm` is available in CI (`binutils-arm-none-eabi` package). Firmware ELF lives at
`target/thumbv7em-none-eabihf/release/firmware`. The binary-size job already builds the ELF.
Add a step after the build that runs:
```bash
arm-none-eabi-nm target/thumbv7em-none-eabihf/release/firmware 2>/dev/null | \
  grep -E '\b(malloc|free|_sbrk|__malloc_av_)\b' || true
```
and fails if any match is found.

---

## Task 1: unwrap_used / expect_used / panic → workspace lints

**Files:**
- Modify: `Cargo.toml` (workspace root)
- Modify: `crates/bluetooth/src/lib.rs`
- Modify: `crates/eink/eink-specs/src/lib.rs`
- Modify: `crates/firmware/src/lib.rs`
- Modify: `crates/library/src/lib.rs`
- Modify: `crates/platform/src/lib.rs`
- Modify: `crates/playback/src/lib.rs`
- Modify: `crates/ui/src/lib.rs`
- Modify: `crates/eink/eink-emulator/src/lib.rs` (add allow)
- Modify: any other crate lib.rs that needs allow (eink-testing, eink-components, firmware-ui, xtask)

**Step 1: Check which crates will fail**

Run:
```bash
cargo clippy --workspace --lib --tests 2>&1 | head -60
```
Expected: Passes (no new errors yet; lints not added).

**Step 2: Write the failing test**

Add to `crates/firmware/tests/arch_boundaries.rs` (at the bottom, before the closing):
```rust
/// Workspace Cargo.toml must enforce unwrap_used/expect_used/panic at workspace level.
#[test]
fn workspace_cargo_toml_has_panic_prevention_lints() {
    let cargo_toml = include_str!("../../../Cargo.toml");
    assert!(
        cargo_toml.contains("unwrap_used"),
        "workspace Cargo.toml must have `unwrap_used = \"deny\"` in [workspace.lints.clippy]"
    );
    assert!(
        cargo_toml.contains("expect_used"),
        "workspace Cargo.toml must have `expect_used = \"deny\"` in [workspace.lints.clippy]"
    );
    // Note: 'panic = "deny"' — search for 'clippy::panic' to avoid matching 'panic = "abort"'
    assert!(
        cargo_toml.contains(r#"panic = "deny""#),
        "workspace Cargo.toml must have `panic = \"deny\"` in [workspace.lints.clippy]"
    );
}
```

Run:
```bash
cargo test -p firmware --test arch_boundaries workspace_cargo_toml_has_panic_prevention_lints 2>&1
```
Expected: FAIL — "workspace Cargo.toml must have `unwrap_used`".

**Step 3: Add lints to workspace Cargo.toml**

In `Cargo.toml`, add to the `[workspace.lints.clippy]` section:
```toml
# Panic-prevention: no silent panics in production firmware.
# Desktop crates (eink-emulator, eink-testing, xtask) suppress these with #![allow].
# Hardware test modules suppress with #[allow] + SAFETY comment.
unwrap_used = "deny"
expect_used = "deny"
panic       = "deny"
```

**Step 4: Run clippy to discover violations**

```bash
cargo clippy --workspace --lib --tests 2>&1 | grep "^error" | sort -u
```
Note every crate that emits errors. For each:
- Desktop/test helper crate → add `#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]` at top of lib.rs
- Embedded crate with existing `#![deny]` (bluetooth, eink-specs, firmware, library, platform, playback, ui) → violations were already handled previously; the `#![deny]` lines are now redundant

**Step 5: Remove redundant per-crate #![deny] lines**

For each of the 7 crates that already had `#![deny(clippy::unwrap_used)]` etc., remove the
now-redundant lines. Example for `crates/platform/src/lib.rs`:
```
REMOVE:
    #![deny(clippy::unwrap_used)] // no .unwrap() in production code
    #![deny(clippy::expect_used)] // no .expect() in production code
    #![deny(clippy::panic)] // no panic!() in production code
```
These are now redundant — workspace lints cover them. Per-site `#[allow]` stays (they override
workspace lints just fine).

**Step 6: Add file-level allow to desktop/test crates**

For each crate that failed in Step 4 (expected: eink-emulator, eink-system, eink-testing,
eink-components, firmware-ui, xtask), add at the top of lib.rs (or main.rs for xtask):

```rust
// Desktop/test crate — unwrap/expect/panic are acceptable in non-embedded code.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
```

**Step 7: Handle eink-system test modules**

If `eink-system` uses unwrap/expect only in `#[cfg(test)]` blocks, apply the allow at each
test module level rather than file level:
```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    // ...
}
```

**Step 8: Verify zero errors**

```bash
cargo clippy --workspace --lib --tests -- -D warnings 2>&1 | grep "^error" | wc -l
```
Expected: 0

Also run the arch test:
```bash
cargo test -p firmware --test arch_boundaries workspace_cargo_toml_has_panic_prevention_lints
```
Expected: PASS

**Step 9: Run full test suite**

```bash
cargo test --workspace 2>&1 | tail -10
```
Expected: all tests pass.

**Step 10: Commit**

```bash
git add Cargo.toml \
  crates/bluetooth/src/lib.rs \
  crates/eink/eink-specs/src/lib.rs \
  crates/firmware/src/lib.rs \
  crates/library/src/lib.rs \
  crates/platform/src/lib.rs \
  crates/playback/src/lib.rs \
  crates/ui/src/lib.rs \
  crates/eink/eink-emulator/src/lib.rs \
  crates/firmware/tests/arch_boundaries.rs
# Add any other modified files discovered in Steps 4-7
git commit -m "enforce(lints): promote unwrap_used/expect_used/panic to workspace deny"
```

---

## Task 2: large_stack_arrays workspace lint + clippy.toml threshold

**Files:**
- Modify: `Cargo.toml` (workspace root) — add lint
- Modify: `clippy.toml` — add `array-size-threshold = 512`
- Modify: any crate lib.rs that needs allow (firmware-ui, eink-emulator probably)

**Step 1: Write the failing test**

Add to `crates/firmware/tests/arch_boundaries.rs`:
```rust
/// clippy.toml must set array-size-threshold to 512 bytes.
/// Default (512000) is far too high for embedded — even 1 KB stack frames
/// can overflow the 4 KB ISR stack on STM32H7 under nested interrupts.
#[test]
fn clippy_toml_has_array_size_threshold_512() {
    let clippy_toml = include_str!("../../../clippy.toml");
    assert!(
        clippy_toml.contains("array-size-threshold"),
        "clippy.toml must set array-size-threshold (recommended: 512)"
    );
    // Extract the value
    let line = clippy_toml
        .lines()
        .find(|l| l.contains("array-size-threshold"))
        .expect("line must exist after previous assert");
    assert!(
        line.contains("512"),
        "array-size-threshold must be <= 512 for embedded safety. Found: {line}"
    );
}
```

Run:
```bash
cargo test -p firmware --test arch_boundaries clippy_toml_has_array_size_threshold_512 2>&1
```
Expected: FAIL.

**Step 2: Add array-size-threshold to clippy.toml**

In `clippy.toml`, add after the existing `enum-variant-size-threshold` line:
```toml
# Array size threshold for large_stack_arrays lint.
# 512 bytes = 128 × u32 or 64 × f64.  Anything larger belongs in a StaticCell or heapless pool.
# Override per-site with #[allow(clippy::large_stack_arrays)] when genuinely stack-local.
array-size-threshold = 512
```

**Step 3: Add workspace lint**

In `Cargo.toml`, add to `[workspace.lints.clippy]`:
```toml
# Warn on arrays >512 bytes allocated on the stack.
# Large stack allocations overflow ISR stacks on STM32H7 (default 4 KB per task).
# Use StaticCell<[T; N]> or heapless::Vec<T, N> for large buffers.
large_stack_arrays = "warn"
```

**Step 4: Discover violations**

```bash
cargo clippy --workspace --lib --tests 2>&1 | grep "large_stack_arrays" | sort -u
```
For each violation decide:
- Embedded crate: convert to `StaticCell` or `heapless::Vec`, or add `#[allow]` + `// SAFETY: stack depth profiled, fits within N KB task stack`
- Desktop crate: add `#[allow(clippy::large_stack_arrays)]` to crate root

**Step 5: Fix violations or add allows**

Apply fixes. The known existing allow in `crates/firmware/src/display/driver.rs` stays untouched.

**Step 6: Verify and test**

```bash
cargo clippy --workspace --lib --tests -- -D warnings 2>&1 | grep "^error" | wc -l
cargo test -p firmware --test arch_boundaries clippy_toml_has_array_size_threshold_512
cargo test --workspace
```
Expected: 0 errors, test PASS, all tests pass.

**Step 7: Commit**

```bash
git add Cargo.toml clippy.toml crates/firmware/tests/arch_boundaries.rs
# Plus any crate files modified for violations
git commit -m "enforce(lints): add large_stack_arrays workspace warn + 512-byte clippy.toml threshold"
```

---

## Task 3: DMA buffer placement arch tests

**Files:**
- Modify: `crates/firmware/tests/memory_safety.rs` — add 3 new tests

**Background:**
The existing `framebuffer_uses_static_cell_with_axisram_link_section` test verifies the current
FRAMEBUFFER. New tests verify:
1. Every `#[link_section = ".axisram"]` static uses the `Align32` wrapper (cacheline alignment)
2. Every `StaticCell` wrapping a large raw `[u8; N]` (N ≥ 4096) also uses `Align32`
3. The `dma.rs` module documents the ST AN4839 cacheline-alignment requirement

**Step 1: Write the tests (they will pass if code is correct)**

These tests validate source-level invariants. Run them first to confirm they pass on current code,
then use them as regression guards.

Add to the bottom of `crates/firmware/tests/memory_safety.rs`:

```rust
/// Every #[link_section = ".axisram"] static must wrap its data in Align32.
///
/// Cortex-M7 has a 32-byte cacheline.  DMA buffers not aligned to 32 bytes cause
/// cache coherency bugs: a CPU store to a neighbouring variable in the same cacheline
/// can corrupt the DMA buffer after cache flush (ST AN4839 §3.3).
///
/// This test scans main.rs for every occurrence of link_section=.axisram and
/// verifies that Align32 appears within 5 lines (the attribute + declaration block).
#[test]
fn all_axisram_statics_use_align32_wrapper() {
    let main_rs = include_str!("../src/main.rs");
    let lines: Vec<&str> = main_rs.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        if line.contains(r#"link_section = ".axisram""#) {
            // Check the next 5 lines for Align32
            let window_end = (i + 5).min(lines.len());
            let window = lines[i..window_end].join("\n");
            assert!(
                window.contains("Align32"),
                "Found #[link_section = \".axisram\"] at line {} without Align32 wrapper.\n\
                 All AXI SRAM statics must use Align32<T> to ensure 32-byte cache-line alignment.\n\
                 See ST AN4839 §3.3 — misaligned DMA buffers cause cache coherency corruption.\n\
                 Line content: {}",
                i + 1,
                line.trim()
            );
        }
    }
}

/// Every StaticCell wrapping a large raw byte array must use Align32.
///
/// StaticCell<[u8; N]> where N >= 4096 (4 KB) is almost certainly a DMA buffer.
/// Without Align32, the buffer may not be 32-byte aligned, risking cache corruption.
/// Use StaticCell<Align32<[u8; N]>> instead.
#[test]
fn large_static_cell_byte_arrays_use_align32() {
    let main_rs = include_str!("../src/main.rs");

    // Look for StaticCell<[u8; without Align32 in the same declaration
    // Pattern: "StaticCell<[u8;" followed eventually by "]>" without "Align32" in between
    // We do a simple line-by-line scan: if "StaticCell<[u8;" is found without "Align32"
    // on the same or adjacent line, that's a violation.
    for (i, line) in main_rs.lines().enumerate() {
        if line.contains("StaticCell<[u8;") && !line.contains("Align32") {
            // Check if it's a small array (< 4096) by looking for the size
            // If we can't determine the size, treat as violation
            let size_hint = extract_array_size(line);
            if size_hint.map_or(true, |n| n >= 4096) {
                panic!(
                    "Found StaticCell<[u8; ...]> at line {} without Align32 wrapper.\n\
                     Use StaticCell<Align32<[u8; N]>> for DMA-capable buffers (N >= 4096).\n\
                     This ensures 32-byte cache-line alignment required by Cortex-M7 DMA.\n\
                     Line: {}",
                    i + 1,
                    line.trim()
                );
            }
        }
    }
}

/// dma.rs module must reference the authoritative ST application note AN4839.
///
/// AN4839 'Level 1 cache on STM32F7, STM32H7 and STM32MP1' is the authoritative
/// guide for DMA/cache coherency on Cortex-M7.  The reference ensures future
/// maintainers can find the source of the 32-byte alignment requirement.
#[test]
fn dma_module_references_st_an4839() {
    let dma_rs = include_str!("../src/dma.rs");
    assert!(
        dma_rs.contains("AN4839"),
        "crates/firmware/src/dma.rs must reference ST AN4839 in its documentation.\n\
         AN4839 'Level 1 cache on STM32F7/H7/MP1' is the authoritative guide for\n\
         DMA cache coherency on Cortex-M7.  Its presence ensures future maintainers\n\
         can find the rationale for the 32-byte Align32 requirement."
    );
}

// Helper: attempt to extract the numeric array size from a line like
// `static FRAMEBUFFER: StaticCell<Align32<[u8; 384_000]>>`
// Returns None if the size cannot be parsed.
fn extract_array_size(line: &str) -> Option<usize> {
    // Find the last semicolon before "]>" which precedes the array size
    let after_semi = line.rsplit(';').next()?;
    let trimmed = after_semi
        .trim()
        .trim_end_matches(']')
        .trim_end_matches('>')
        .trim_end_matches(')')
        .trim()
        .replace('_', "");
    trimmed.parse::<usize>().ok()
}
```

**Step 2: Run tests**

```bash
cargo test -p firmware --test memory_safety 2>&1
```
Expected: All 7 tests PASS (3 new + 4 existing).

If any new test fails, the source code has a violation — fix it (likely add `Align32` wrapper)
before proceeding.

**Step 3: Verify no regressions**

```bash
cargo test --workspace 2>&1 | tail -10
```
Expected: all tests pass.

**Step 4: Commit**

```bash
git add crates/firmware/tests/memory_safety.rs
git commit -m "test(arch): add Align32/DMA placement arch tests to memory_safety.rs"
```

---

## Task 4: No-malloc linker check in CI

**Files:**
- Modify: `.github/workflows/ci.yml` — add no-malloc step to `binary-size` job

**Background:**
The Cortex-M7 firmware has no heap. Any `malloc`/`free` (or `_sbrk` which is the syscall
hook that `malloc` calls) in the final ELF means a dependency accidentally pulled in heap
allocation. `arm-none-eabi-nm` lists all symbols in the ELF; grep for the heap symbols.

**Step 1: Locate the binary-size job**

Open `.github/workflows/ci.yml` and find the `binary-size` job. It currently:
1. Builds the release ELF
2. Runs `arm-none-eabi-size` to check section sizes

**Step 2: Write the failing test**

Add an arch test that verifies the CI file contains the no-malloc check:
```rust
// In crates/firmware/tests/arch_boundaries.rs:
/// CI must verify firmware ELF has no malloc/free/sbrk symbols.
#[test]
fn ci_binary_size_job_has_no_malloc_check() {
    let ci_yml = include_str!("../../../../.github/workflows/ci.yml");
    assert!(
        ci_yml.contains("malloc") || ci_yml.contains("_sbrk"),
        "binary-size CI job must check for malloc/free/_sbrk symbols using arm-none-eabi-nm.\n\
         Heap allocation in firmware means a dependency accidentally uses the allocator.\n\
         Add: arm-none-eabi-nm ... | grep -E '(malloc|free|_sbrk)' | (read && exit 1 || exit 0)"
    );
}
```

Run:
```bash
cargo test -p firmware --test arch_boundaries ci_binary_size_job_has_no_malloc_check 2>&1
```
Expected: FAIL.

**Step 3: Add no-malloc step to CI**

In `.github/workflows/ci.yml`, find the `binary-size` job. After the `arm-none-eabi-size`
step, add:

```yaml
      - name: No-malloc check
        run: |
          # Fail if any heap allocation symbols appear in the firmware ELF.
          # malloc/free/_sbrk indicate a dependency accidentally uses the allocator.
          # The Cortex-M7 firmware is configured with panic="abort" and no global allocator.
          echo "Checking for heap allocation symbols..."
          HEAP_SYMS=$(arm-none-eabi-nm target/thumbv7em-none-eabihf/release/firmware 2>/dev/null \
            | grep -E '\b(malloc|free|_sbrk|__malloc_av_)\b' || true)
          if [ -n "$HEAP_SYMS" ]; then
            echo "ERROR: Heap allocation symbols found in firmware ELF:"
            echo "$HEAP_SYMS"
            echo ""
            echo "This means a dependency is using malloc/free."
            echo "Audit new dependencies with 'cargo tree' and check for std allocator usage."
            exit 1
          fi
          echo "OK: No heap allocation symbols found."
```

**Step 4: Verify arch test passes**

```bash
cargo test -p firmware --test arch_boundaries ci_binary_size_job_has_no_malloc_check 2>&1
```
Expected: PASS.

**Step 5: Run all tests**

```bash
cargo test --workspace 2>&1 | tail -10
```
Expected: all tests pass.

**Step 6: Run clippy**

```bash
cargo clippy --workspace --lib --tests -- -D warnings 2>&1 | grep "^error" | wc -l
```
Expected: 0.

**Step 7: Commit**

```bash
git add .github/workflows/ci.yml crates/firmware/tests/arch_boundaries.rs
git commit -m "enforce(ci): add no-malloc linker check to binary-size CI job"
```

---

## Task 5: use_debug workspace lint + fix builder.rs violations

**Files:**
- Modify: `Cargo.toml` (workspace root) — add lint
- Modify: `crates/firmware/src/input/builder.rs` — fix 3 violations
- Modify: any desktop crate lib.rs that needs allow

**Background:**
`clippy::use_debug` warns on `{:?}` format strings in non-test code. `{:?}` uses the `Debug`
trait which can produce verbose output, is not guaranteed stable across Rust versions, and
pulls in more code than a purpose-written display string. On embedded firmware where binary
size matters, every avoidable format string counts.

Three violations in `builder.rs` (all inside `#[cfg(feature = "keyboard-input")]`):
- Line 275: `format!("{:?}", a)` — `a: EmulatedAxis`
- Line 285: `format!("{:?}", k)` — `k: EmulatedKey`
- Line 288: `eprintln!("[InputBuilder] Button::{:?} → ...")` — `btn: Button`

Fix by implementing `fn axis_name(a: EmulatedAxis) -> &'static str` and
`fn key_name(k: EmulatedKey) -> &'static str` local helpers, plus a match on `Button`.

**Step 1: Write the failing test**

Add to `crates/firmware/tests/arch_boundaries.rs`:
```rust
/// builder.rs must not use {:?} debug format in production code paths.
/// use_debug lint requires explicit display strings instead of Debug trait formatting.
#[test]
fn builder_rs_has_no_debug_format_in_production_paths() {
    // The keyboard-input feature gate is compile-time only; we check source.
    let builder_rs = include_str!("../src/input/builder.rs");
    // Count {:?} occurrences outside of doc comments and test blocks.
    // Simple heuristic: count in non-comment, non-test lines.
    let violations: Vec<(usize, &str)> = builder_rs
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let trimmed = line.trim();
            !trimmed.starts_with("///") && !trimmed.starts_with("//") && line.contains("{:?}")
        })
        .collect();
    assert!(
        violations.is_empty(),
        "builder.rs has {:?} debug format strings in non-comment lines. \
         Replace with match arms returning &'static str.\n\
         Violations at lines: {:?}",
        violations.len(),
        violations.iter().map(|(i, _)| i + 1).collect::<Vec<_>>()
    );
}
```

Run:
```bash
cargo test -p firmware --test arch_boundaries builder_rs_has_no_debug_format_in_production_paths 2>&1
```
Expected: FAIL (finds 3 violations).

**Step 2: Add use_debug to workspace lints**

In `Cargo.toml`, add to `[workspace.lints.clippy]`:
```toml
# Warn when {:?} Debug format is used in non-test code.
# Debug output is verbose, unstable, and larger than purpose-written display strings.
# Use a match arm returning &'static str instead.
# Desktop crates may suppress with #[allow(clippy::use_debug)].
use_debug = "warn"
```

**Step 3: Run clippy to find all violations**

```bash
cargo clippy --workspace --lib --tests 2>&1 | grep "use_debug" | sort -u
```
Expected: violations in `firmware` (builder.rs, keyboard-input feature) and possibly
`eink-emulator`, `xtask`, `eink-testing` (desktop crates — add file-level allow there).

**Step 4: Fix builder.rs violations**

In `crates/firmware/src/input/builder.rs`, inside the `#[cfg(feature = "keyboard-input")]`
section of `build_emulated`, replace the `{:?}` uses.

Add two private helper functions (inside the `#[cfg(feature = "keyboard-input")]` mod or at
module level with `#[cfg(feature = "keyboard-input")]`):

```rust
#[cfg(feature = "keyboard-input")]
fn axis_name(axis: EmulatedAxis) -> &'static str {
    match axis {
        EmulatedAxis::ArrowUpDown => "ArrowUpDown",
        EmulatedAxis::ScrollWheel => "ScrollWheel",
    }
}

#[cfg(feature = "keyboard-input")]
fn key_name(key: EmulatedKey) -> &'static str {
    match key {
        EmulatedKey::Space      => "Space",
        EmulatedKey::KeyK       => "KeyK",
        EmulatedKey::ArrowRight => "ArrowRight",
        EmulatedKey::KeyL       => "KeyL",
        EmulatedKey::ArrowLeft  => "ArrowLeft",
        EmulatedKey::KeyJ       => "KeyJ",
        EmulatedKey::ArrowUp    => "ArrowUp",
        EmulatedKey::ArrowDown  => "ArrowDown",
        EmulatedKey::KeyM       => "KeyM",
        EmulatedKey::Escape     => "Escape",
        EmulatedKey::Enter      => "Enter",
    }
}

#[cfg(feature = "keyboard-input")]
fn button_name(btn: Button) -> &'static str {
    match btn {
        Button::Play       => "Play",
        Button::Next       => "Next",
        Button::Previous   => "Previous",
        Button::VolumeUp   => "VolumeUp",
        Button::VolumeDown => "VolumeDown",
        Button::Menu       => "Menu",
        Button::Back       => "Back",
        Button::Select     => "Select",
    }
}
```

Then replace in `build_emulated`:
```rust
// BEFORE:
let axis_desc = self
    .emulated_axis
    .map(|a| format!("{:?}", a))
    .unwrap_or_else(|| "ScrollWheel (default)".to_string());

// AFTER:
let axis_desc = self
    .emulated_axis
    .map(|a| axis_name(a))
    .unwrap_or("ScrollWheel (default)");
```

```rust
// BEFORE:
let key_desc = self
    .emulated_key
    .map(|k| format!("{:?}", k))
    .unwrap_or_else(|| "global key map (default)".to_string());
eprintln!(
    "[InputBuilder] Button::{:?} → emulated key: {}",
    btn, key_desc
);

// AFTER:
let key_desc = self
    .emulated_key
    .map(|k| key_name(k))
    .unwrap_or("global key map (default)");
eprintln!(
    "[InputBuilder] Button::{} → emulated key: {}",
    button_name(btn), key_desc
);
```

Note: the `format!()` + `String` becomes `&'static str` — update `axis_desc` and `key_desc`
types accordingly. The `eprintln!` call now uses `{}` instead of `{:?}`.

Also remove the `.unwrap_or_else(|| ...)` closures — they allocate `String`. Change to
`.unwrap_or("...")` to get `&'static str`.

**Step 5: Add file-level allows to desktop crates**

For any desktop crate that showed `use_debug` violations (expected: `eink-emulator`, `xtask`):
```rust
#![allow(clippy::use_debug)]
```

**Step 6: Verify zero warnings and test pass**

```bash
cargo clippy --workspace --lib --tests -- -D warnings 2>&1 | grep "^error" | wc -l
cargo test -p firmware --test arch_boundaries builder_rs_has_no_debug_format_in_production_paths
cargo test --workspace
```
Expected: 0 errors, test PASS, all tests pass.

**Step 7: Commit**

```bash
git add Cargo.toml crates/firmware/src/input/builder.rs crates/firmware/tests/arch_boundaries.rs
# Plus any desktop crate lib.rs files
git commit -m "enforce(lints): add use_debug workspace warn + fix builder.rs debug format violations"
```

---

## Verification Checklist (after all 5 tasks)

```bash
# Full clippy — must be 0 errors
cargo clippy --workspace --lib --tests -- -D warnings 2>&1 | grep "^error" | wc -l

# Full test suite — must be 0 failures
cargo test --workspace

# no-std check — embedded crates must compile without std
cargo check --target thumbv7em-none-eabihf --features hardware -p firmware

# Rustdoc — must be 0 warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps 2>&1 | grep "^error" | wc -l
```
