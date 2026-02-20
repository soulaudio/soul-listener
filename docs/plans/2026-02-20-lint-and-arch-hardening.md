# Lint and Architecture Hardening Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Add 10 new enforcement layers — workspace clippy lints, no-std CI validation, DMA alignment typing, cargo-geiger unsafe tracking, rustdoc CI, feature exclusivity arch tests, and per-section binary size limits.

**Architecture:** Expand `[workspace.lints]` in root `Cargo.toml` to deny/warn known-dangerous patterns; add 4 new CI jobs (`check-no-std`, `docs`, `geiger`, plus extended binary-size); add `Align32<T>` DMA newtype to firmware; add arch tests for alignment and feature exclusivity. Every new warn-level lint must have all existing violations fixed in the same commit — the CI runs clippy with `-- -D warnings` which promotes warns to errors.

**Tech Stack:** Rust clippy workspace lints, cargo-geiger ≥ 0.13.0 (requires Rust ≥ 1.85 to install; runs against any codebase), `RUSTDOCFLAGS="-D warnings"`, GitHub Actions YAML, `#[repr(align(32))]`.

**Key Research Findings:**
- All listed clippy lints are available since Rust 1.29–1.58; all work at `[workspace.lints.clippy]` level.
- `undocumented_unsafe_blocks` has known proc-macro false positives (Embassy, cortex-m-rt). Use `"warn"` not `"deny"`, and add `#[allow]` at proc-macro call sites.
- `cast_possible_truncation` fires on safe narrowing casts (no value-range analysis). Add per-site `#[allow]` with comment where the cast is provably safe.
- `missing_docs` at workspace level works for lib crates; has no effect on binary crates.
- `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps` is the canonical rustdoc CI pattern. The `--no-deps` flag is **required** to avoid failing on a dep's doc warning.
- `cargo-geiger` 0.13.0 requires Rust ≥ 1.85 to install; CI installs it with the stable toolchain (not MSRV toolchain).
- `[workspace.lints.clippy]` — if mixing lint GROUPS (like `pedantic`) with individual overrides, `priority` field is required to avoid alphabetical ordering bugs. We don't enable groups here, so no `priority` needed.

---

### Task 1: Workspace lints — zero-FP set (todo, unimplemented, dbg_macro, await_holding_lock)

These four lints have no false positives and are clean additions. Any violations found must be fixed (not suppressed) before committing.

**Files:**
- Modify: `Cargo.toml` (workspace root, `[workspace.lints.clippy]` section)
- Possibly modify: any `.rs` file that currently uses `todo!()`, `unimplemented!()`, `dbg!()`, or holds a lock across `.await`

**Step 1: Add the lints**

In `Cargo.toml`, update `[workspace.lints.clippy]` to:

```toml
[workspace.lints.clippy]
# Safety-critical: every arithmetic op must be explicitly audited.
# Use checked_*/saturating_*/wrapping_* or add #[allow] at the specific site.
arithmetic_side_effects = "deny"
# Safety-critical: direct slice indexing panics on OOB. Use .get() instead.
indexing_slicing = "deny"
# Debug macros must not appear in committed code.
dbg_macro = "deny"
# Placeholder code must not reach production.
todo = "deny"
unimplemented = "deny"
# Holding a mutex/lock across an await point can deadlock async tasks.
await_holding_lock = "deny"
```

**Step 2: Find violations**

```
cargo clippy --workspace --lib --tests 2>&1 | grep "error\[clippy"
```

For each violation:
- `todo!()` → replace with `unreachable!("context")`, a proper `Err(...)`, or `compile_error!()`
- `unimplemented!()` → same as above
- `dbg!(...)` → remove entirely (debug prints should use `defmt::debug!` or `tracing::debug!`)
- `await_holding_lock` → restructure to drop the guard before the `.await` point using a scope block

**Step 3: Re-run to confirm zero errors**

```
cargo clippy --workspace --lib --tests 2>&1 | grep "error\[clippy"
```
Expected: empty output.

**Step 4: Run full test suite to confirm nothing broke**

```
cargo test --workspace 2>&1 | tail -5
```
Expected: all pass.

**Step 5: Commit**

```
git add Cargo.toml
git commit -m "lint: deny todo/unimplemented/dbg_macro/await_holding_lock workspace-wide"
```

---

### Task 2: Cast safety lints (cast_possible_truncation, cast_sign_loss, cast_possible_wrap)

These warn on casts that may silently lose bits or sign. The `as u8` ordering bug found in Task 4 of the previous plan would have been caught by `cast_possible_truncation`. Every existing violation must either be fixed with a safe alternative or annotated with a `#[allow]` + comment explaining why the cast is provably safe.

**Files:**
- Modify: `Cargo.toml`
- Modify: any `.rs` files with unsafe cast patterns

**Step 1: Add the lints**

```toml
[workspace.lints.clippy]
# ... existing entries ...
# Cast safety — warn on narrowing/sign-changing casts that may silently lose data.
# Prevents the class of bug where (x as u8).min(50) truncates before clamping;
# the correct form is x.min(50) as u8.
cast_possible_truncation = "warn"
cast_sign_loss = "warn"
cast_possible_wrap = "warn"
```

**Step 2: Find ALL violations immediately**

```
cargo clippy --workspace --lib --tests 2>&1 | grep "warning.*cast_possible\|warning.*cast_sign"
```

Count the violations. For each one, choose the right fix:

**Option A — Fix with a safe alternative (preferred):**
```rust
// Before (fires cast_possible_truncation):
let x = some_u32_value as u8;

// After — use try_from and handle failure:
let x = u8::try_from(some_u32_value).expect("value fits in u8");
// Or if clamping is the intent:
let x = some_u32_value.min(u32::from(u8::MAX)) as u8;
// Or if guarded by prior bounds check:
assert!(some_u32_value <= 255);
let x = some_u32_value as u8;  // still fires — use #[allow] + comment
```

**Option B — Suppress with documentation (when the cast is provably safe):**
```rust
// SAFETY: DISPLAY_WIDTH is defined as 800 which fits in u16 (max 65535).
#[allow(clippy::cast_possible_truncation)]
let w = DISPLAY_WIDTH as u16;
```

**Step 3: Verify all violations resolved**

```
cargo clippy --workspace --lib --tests 2>&1 | grep "cast_possible\|cast_sign"
```
Expected: empty (all fixed or annotated).

**Step 4: Run full test suite**

```
cargo test --workspace 2>&1 | tail -5
```

**Step 5: Commit**

```
git add Cargo.toml <modified-files>
git commit -m "lint: add cast_possible_truncation/sign_loss/wrap workspace warnings + fix violations"
```

---

### Task 3: Undocumented unsafe blocks (SAFETY comments)

Add `undocumented_unsafe_blocks = "warn"` and add `// SAFETY:` rationale to every unsafe block in workspace source code. Handle proc-macro false positives (Embassy, cortex-m-rt) with targeted `#[allow]`.

**Files:**
- Modify: `Cargo.toml`
- Modify: all `.rs` files containing `unsafe { ... }` blocks

**Step 1: Add the lint**

```toml
[workspace.lints.clippy]
# ... existing ...
# Every unsafe block must explain its safety invariant.
# Use "warn" not "deny" due to known proc-macro false positives in Embassy.
undocumented_unsafe_blocks = "warn"
```

**Step 2: Find all violations**

```
cargo clippy --workspace --lib --tests 2>&1 | grep "undocumented_unsafe"
```

**Step 3: Add SAFETY comments to each unsafe block**

For each violation, add a `// SAFETY:` comment immediately ABOVE the `unsafe` keyword. The comment must explain:
1. What invariant guarantees this operation is safe
2. What would go wrong if the invariant were violated

Example pattern for static mut access:
```rust
// SAFETY: FRAMEBUFFER is only accessed from a single task through this
// exclusive borrow. No concurrent access is possible because the display
// driver holds exclusive ownership of this reference for the duration of
// the refresh.
let buf = unsafe { &mut FRAMEBUFFER.0 };
```

Example pattern for raw pointer:
```rust
// SAFETY: `ptr` was obtained from `Box::into_raw` in `new()` and is only
// accessed here in `drop()`, which runs exactly once. The pointee is
// properly initialized and aligned.
unsafe { drop(Box::from_raw(ptr)); }
```

**Step 4: Handle proc-macro false positives**

If `#[embassy_executor::main]` or `#[cortex_m_rt::entry]` causes a false positive (clippy fires on a span inside the generated code), suppress it at the function level:

```rust
// The embassy_executor::main macro generates an unsafe block internally.
// The safety invariant is documented within Embassy itself. We cannot add
// a SAFETY comment to macro-generated code, so we suppress here.
#[allow(clippy::undocumented_unsafe_blocks)]
#[embassy_executor::main]
async fn main(spawner: Spawner) {
    // ...
}
```

**Step 5: Verify**

```
cargo clippy --workspace --lib --tests 2>&1 | grep "undocumented_unsafe"
```
Expected: empty.

**Step 6: Commit**

```
git add Cargo.toml <all-modified-files>
git commit -m "lint: require SAFETY comments on all unsafe blocks"
```

---

### Task 4: Missing docs workspace warning

Add `missing_docs = "warn"` at workspace level. Suppress with `#![allow(missing_docs)]` in crates not yet ready, and add docs to the most critical public API surface: `platform` HAL traits.

**Files:**
- Modify: `Cargo.toml`
- Modify: `crates/platform/src/lib.rs` (add docs to trait methods)
- Modify: any crate root that fires without being ready for docs (add `#![allow(missing_docs)]`)

**Step 1: Add the lint**

```toml
[workspace.lints.rust]
unsafe_op_in_unsafe_fn = "deny"
missing_docs = "warn"
```

**Step 2: Find violations**

```
cargo clippy --workspace --lib --tests 2>&1 | grep "missing documentation"
```

**Step 3: For each crate that fires — decide: document or suppress**

**Crates that MUST get docs (public API surface):**
- `crates/platform/src/lib.rs` — `DisplayDriver`, `InputDevice`, `AudioCodec`, `Storage` traits

Add docs to every method. Example:
```rust
/// Initialize the display and prepare it for rendering.
///
/// Must be called once before any call to [`refresh_full`] or [`refresh_partial`].
/// On hardware this performs the SSD1677 power-on sequence.
async fn init(&mut self) -> Result<(), Self::Error>;
```

**Crates that get `#![allow(missing_docs)]` now (document incrementally):**
- `xtask` — binary crate, internal tooling
- `crates/eink/eink-emulator` — desktop emulator, semi-private
- `crates/firmware` — application binary, not a library
- Other crates as found

Add at the TOP of each `src/lib.rs` or `src/main.rs`:
```rust
// TODO: Add rustdoc to all public items (tracked as tech debt)
#![allow(missing_docs)]
```

**Step 4: Verify clean**

```
cargo clippy --workspace --lib --tests 2>&1 | grep "missing documentation"
```
Expected: empty.

**Step 5: Commit**

```
git add Cargo.toml crates/platform/src/lib.rs <suppressed-files>
git commit -m "lint: add missing_docs workspace warning; document platform HAL traits"
```

---

### Task 5: No-std CI validation job

Add a `check-no-std` CI job that builds all no-std crates with `--target thumbv7em-none-eabihf --no-default-features` to catch accidental std leakage.

**Files:**
- Modify: `.github/workflows/ci.yml`
- Possibly modify: crate roots missing `#![no_std]`

**Step 1: Run locally to find which crates compile**

Run each check and record pass/fail:
```
cargo check -p eink-specs --target thumbv7em-none-eabihf --no-default-features
cargo check -p eink-system --target thumbv7em-none-eabihf --no-default-features
cargo check -p eink-components --target thumbv7em-none-eabihf --no-default-features
cargo check -p platform --target thumbv7em-none-eabihf --no-default-features
cargo check -p ui --target thumbv7em-none-eabihf --no-default-features
cargo check -p playback --target thumbv7em-none-eabihf --no-default-features
cargo check -p library --target thumbv7em-none-eabihf --no-default-features
cargo check -p bluetooth --target thumbv7em-none-eabihf --no-default-features
cargo check -p firmware-ui --target thumbv7em-none-eabihf --no-default-features
```

For any that fail: the fix is almost always one of:
- Add `#![no_std]` to the crate root (if it compiles as std by accident)
- Add `extern crate alloc;` for types using `alloc::` (Vec, String, etc.)
- Gate std-only code behind `#[cfg(feature = "std")]`

**Step 2: Add the CI job**

In `.github/workflows/ci.yml`, add after the `check-embedded` job:

```yaml
  # ---------------------------------------------------------------------------
  # No-std validation
  #
  # Verifies that embedded crates compile without std on the target.
  # Catches std types (String, Vec, println!) accidentally added to no_std crates.
  # Uses the ARM target because some no_std issues only surface cross-compiling.
  # ---------------------------------------------------------------------------
  check-no-std:
    name: No-std validation
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683  # v4.2.2

      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: thumbv7em-none-eabihf

      - uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5  # v2.8.2

      - name: eink-specs — no_std
        run: cargo check -p eink-specs --target thumbv7em-none-eabihf --no-default-features

      - name: eink-system — no_std
        run: cargo check -p eink-system --target thumbv7em-none-eabihf --no-default-features

      - name: eink-components — no_std
        run: cargo check -p eink-components --target thumbv7em-none-eabihf --no-default-features

      - name: platform — no_std
        run: cargo check -p platform --target thumbv7em-none-eabihf --no-default-features

      - name: ui — no_std
        run: cargo check -p ui --target thumbv7em-none-eabihf --no-default-features

      - name: playback — no_std
        run: cargo check -p playback --target thumbv7em-none-eabihf --no-default-features

      - name: library — no_std
        run: cargo check -p library --target thumbv7em-none-eabihf --no-default-features

      - name: bluetooth — no_std
        run: cargo check -p bluetooth --target thumbv7em-none-eabihf --no-default-features

      - name: firmware-ui — no_std
        run: cargo check -p firmware-ui --target thumbv7em-none-eabihf --no-default-features
```

**Step 3: Commit**

```
git add .github/workflows/ci.yml <any-fixed-crates>
git commit -m "ci: add no-std validation job for all embedded crates"
```

---

### Task 6: DMA alignment newtype (Align32<T>) and arch tests

The Cortex-M7's 32-byte cacheline means DMA buffers must be 32-byte aligned to prevent silent data corruption when the cache and DMA disagree on memory state. Add `Align32<T>` with `#[repr(align(32))]` and use it for FRAMEBUFFER.

**Files:**
- Create: `crates/firmware/src/dma.rs`
- Modify: `crates/firmware/src/lib.rs` (expose `pub mod dma`)
- Modify: `crates/firmware/src/main.rs` (wrap FRAMEBUFFER with `Align32`)
- Modify: `crates/firmware/tests/arch_boundaries.rs` (add alignment tests)

**Step 1: Write failing arch tests first**

In `crates/firmware/tests/arch_boundaries.rs`, add at the end:

```rust
// ─── DMA Alignment Tests ─────────────────────────────────────────────────────

/// Verify that `Align32<T>` provides 32-byte (Cortex-M7 cacheline) alignment.
///
/// All DMA buffers on STM32H743ZI must be 32-byte aligned. The CPU's D-cache
/// has 32-byte cachelines; unaligned DMA buffers cause cache coherency bugs
/// where the CPU reads stale cached data after a DMA write.
///
/// References: ST AN4839, ARM DDI0489F §B3.5
#[test]
fn align32_has_32_byte_alignment() {
    use firmware::dma::Align32;
    assert_eq!(core::mem::align_of::<Align32<u8>>(), 32);
    assert_eq!(core::mem::align_of::<Align32<[u8; 4]>>(), 32);
    assert_eq!(core::mem::align_of::<Align32<[u8; 65536]>>(), 32);
}

/// Verify that Align32 does not change the size of the inner type (only alignment).
#[test]
fn align32_size_equals_inner_size_rounded_up_to_alignment() {
    use firmware::dma::Align32;
    // For [u8; 32], size should be exactly 32 (already aligned).
    assert_eq!(core::mem::size_of::<Align32<[u8; 32]>>(), 32);
    // For [u8; 64], size should be exactly 64.
    assert_eq!(core::mem::size_of::<Align32<[u8; 64]>>(), 64);
}

/// Verify that FRAMEBUFFER_SIZE is divisible by 32 (cacheline size).
///
/// This is required so that the framebuffer occupies an exact number of
/// cachelines, preventing partial-cacheline DMA transfers that can corrupt
/// adjacent memory.
#[test]
fn framebuffer_size_is_cacheline_aligned() {
    assert_eq!(
        firmware::FRAMEBUFFER_SIZE % 32,
        0,
        "FRAMEBUFFER_SIZE ({}) must be divisible by 32 (Cortex-M7 cacheline size)",
        firmware::FRAMEBUFFER_SIZE
    );
}
```

Run: `cargo test -p firmware --test arch_boundaries 2>&1 | grep -E "align32|framebuffer_size"`
Expected: FAIL — `firmware::dma` module does not exist yet.

**Step 2: Create `crates/firmware/src/dma.rs`**

```rust
//! DMA buffer alignment for Cortex-M7 cache coherency.
//!
//! The STM32H743ZI Cortex-M7 has a 32-byte cacheline. Any buffer accessed by
//! a DMA peripheral must be aligned to at least 32 bytes to prevent cache
//! coherency bugs where the CPU cache and DMA controller disagree on memory
//! state.
//!
//! # The Problem
//!
//! When D-cache is enabled (embassy-stm32 enables it during init), the CPU
//! may cache DMA buffer contents. If DMA writes to memory while the CPU has
//! a stale cache line, the CPU will read the old (pre-DMA) data. Conversely,
//! if the CPU writes to a buffer before DMA reads it, those writes may sit
//! in cache and never reach RAM before DMA starts.
//!
//! # The Solution
//!
//! Either:
//! 1. Place buffers in non-cacheable SRAM (`.axisram` section is configured
//!    as non-cacheable by the MPU in `firmware::boot::hardware`) — preferred.
//! 2. Use cache maintenance operations (SCB::clean_dcache_by_address, etc.)
//!    before/after every DMA transfer — complex and error-prone.
//!
//! The `Align32` wrapper enforces proper alignment. Combined with placement
//! in `.axisram` via `#[link_section = ".axisram"]`, this fully prevents
//! cache coherency issues.
//!
//! # References
//! - ST AN4839: Level 1 cache on STM32F7 Series and STM32H7 Series
//! - ST AN4838: MPU programming model for STM32
//! - ARM DDI0489F §B3.5: Cache coherency

/// A `#[repr(align(32))]` wrapper that enforces 32-byte alignment for
/// Cortex-M7 DMA-accessible buffers.
///
/// All static buffers accessed by DMA peripherals (SAI audio, SPI display,
/// SDMMC storage) must use this wrapper to guarantee that cache operations
/// do not corrupt DMA data.
///
/// # Example
///
/// ```rust,ignore
/// use firmware::dma::Align32;
///
/// #[link_section = ".axisram"]
/// #[allow(dead_code)]
/// static mut SAI_DMA_BUF: Align32<[u8; 8192]> = Align32([0u8; 8192]);
/// ```
#[derive(Clone, Copy)]
#[repr(align(32))]
pub struct Align32<T>(pub T);
```

**Step 3: Add `pub mod dma` to `crates/firmware/src/lib.rs`**

Find the existing `pub mod` declarations in `lib.rs` and add:
```rust
pub mod dma;
```

**Step 4: Update FRAMEBUFFER in `crates/firmware/src/main.rs`**

Add the import at the top:
```rust
use firmware::dma::Align32;
```

Change the FRAMEBUFFER declaration from:
```rust
#[link_section = ".axisram"]
#[allow(dead_code)]
static mut FRAMEBUFFER: [u8; FRAMEBUFFER_SIZE] = [0xFF; FRAMEBUFFER_SIZE];
```

To:
```rust
#[link_section = ".axisram"]
#[allow(dead_code)]
static mut FRAMEBUFFER: Align32<[u8; FRAMEBUFFER_SIZE]> = Align32([0xFF; FRAMEBUFFER_SIZE]);
```

> NOTE: If FRAMEBUFFER is accessed via raw pointer elsewhere, update those accesses
> to use `.0` to get the inner array: `&mut FRAMEBUFFER.0`.

**Step 5: Run the arch tests**

```
cargo test -p firmware --test arch_boundaries 2>&1 | grep -E "align32|framebuffer|PASS|FAIL|ok|error"
```
Expected: all 3 new tests pass.

**Step 6: Run full test suite**

```
cargo test --workspace 2>&1 | tail -5
```

**Step 7: Commit**

```
git add crates/firmware/src/dma.rs crates/firmware/src/lib.rs crates/firmware/src/main.rs crates/firmware/tests/arch_boundaries.rs
git commit -m "feat(firmware): add Align32 DMA alignment newtype + cacheline arch tests"
```

---

### Task 7: cargo-geiger unsafe code tracking

Add a `geiger` CI job that counts unsafe blocks in workspace crates and fails if the count grows beyond a stored baseline. Prevents silent accumulation of unsafe code.

**Files:**
- Modify: `.github/workflows/ci.yml`
- Create: `.geiger-baseline` (integer — total unsafe count for workspace crates)

**Step 1: Install cargo-geiger and measure baseline**

```
cargo install cargo-geiger
cargo geiger --output Json 2>/dev/null > /tmp/geiger.json
```

Parse the workspace unsafe count (exclude third-party deps):
```
python3 -c "
import json, sys
with open('/tmp/geiger.json') as f:
    data = json.load(f)
total = 0
for pkg in data.get('packages', []):
    if not pkg.get('is_local', False):
        continue
    name = pkg.get('package', {}).get('name', '?')
    used = pkg.get('unsafety', {}).get('used', {})
    count = sum(used.values())
    if count > 0:
        print(f'  {name}: {count}')
    total += count
print(f'Total: {total}')
"
```

Record the number. Create `.geiger-baseline`:
```
echo "<ACTUAL_COUNT_FROM_ABOVE>" > .geiger-baseline
```

**Step 2: Create `tools/geiger-count.py` helper script**

Create `tools/geiger-count.py`:
```python
#!/usr/bin/env python3
"""Parse cargo-geiger JSON output and report workspace unsafe counts.

Usage:
  cargo geiger --output Json 2>/dev/null | python3 tools/geiger-count.py
  cargo geiger --output Json 2>/dev/null | python3 tools/geiger-count.py --check .geiger-baseline
"""

import json
import sys
import argparse

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", metavar="BASELINE_FILE",
                        help="Fail if count exceeds this file's value")
    args = parser.parse_args()

    data = json.load(sys.stdin)
    total = 0
    rows = []

    for pkg in data.get("packages", []):
        if not pkg.get("is_local", False):
            continue
        name = pkg.get("package", {}).get("name", "?")
        used = pkg.get("unsafety", {}).get("used", {})
        count = sum(used.values())
        if count > 0:
            rows.append((name, count))
        total += count

    print(f"Workspace unsafe count: {total}")
    for name, count in sorted(rows):
        print(f"  {name}: {count}")

    if args.check:
        try:
            with open(args.check) as f:
                baseline = int(f.read().strip())
            if total > baseline:
                print(f"FAIL: count {total} exceeds baseline {baseline}")
                print(f"  If this is intentional: update .geiger-baseline with {total}")
                sys.exit(1)
            print(f"OK: {total} <= baseline {baseline}")
        except FileNotFoundError:
            print(f"WARNING: baseline file not found; writing {total} to {args.check}")
            with open(args.check, "w") as f:
                f.write(str(total))

if __name__ == "__main__":
    main()
```

**Step 3: Add CI job**

In `.github/workflows/ci.yml`, add after the `arch-boundaries` job:

```yaml
  # ---------------------------------------------------------------------------
  # Unsafe code audit — cargo-geiger
  #
  # Counts unsafe fn/impl/trait/block in workspace crates (not dependencies).
  # Fails if the count exceeds the baseline in .geiger-baseline.
  #
  # cargo-geiger requires Rust ≥ 1.85 to install; we use stable (not MSRV).
  # It analyses the codebase at whatever Rust version is installed.
  #
  # To update the baseline after intentionally adding new unsafe code:
  #   cargo geiger --output Json 2>/dev/null | python3 tools/geiger-count.py --check .geiger-baseline
  #   git add .geiger-baseline && git commit -m "chore: update geiger unsafe baseline"
  # ---------------------------------------------------------------------------
  geiger:
    name: Unsafe code audit
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683  # v4.2.2

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5  # v2.8.2

      - name: Install cargo-geiger
        run: cargo install cargo-geiger

      - name: Run unsafe code audit
        run: cargo geiger --output Json 2>/dev/null | python3 tools/geiger-count.py --check .geiger-baseline
```

**Step 4: Commit**

```
git add .github/workflows/ci.yml .geiger-baseline tools/geiger-count.py
git commit -m "ci: add cargo-geiger unsafe code tracking with baseline enforcement"
```

---

### Task 8: Rustdoc CI job (warnings as errors)

Add a `docs` CI job. `RUSTDOCFLAGS="-D warnings"` causes rustdoc to fail on broken intra-doc links, malformed doc examples, and other documentation issues.

**Files:**
- Modify: `.github/workflows/ci.yml`
- Possibly modify: any `.rs` file with broken doc links or malformed examples

**Step 1: Run locally first and fix warnings**

```
$env:RUSTDOCFLAGS="-D warnings"; cargo doc --no-deps --workspace
```
(On Unix: `RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace`)

Fix every warning before adding the CI job. Common issues:
- Broken intra-doc links: `[SomeType]` that doesn't resolve → fix the link or use full path
- Missing `///` on items that other docs reference
- Code examples in doc comments that don't compile

**Step 2: Add the CI job**

In `.github/workflows/ci.yml`, add after the `clippy` job:

```yaml
  # ---------------------------------------------------------------------------
  # Documentation build — warnings as errors
  #
  # RUSTDOCFLAGS="-D warnings" fails on broken intra-doc links, missing items
  # referenced in doc comments, and malformed code examples.
  #
  # --no-deps: only document workspace crates. Without this flag, a warning
  # in a transitive dependency's docs would break our build.
  # ---------------------------------------------------------------------------
  docs:
    name: Documentation
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683  # v4.2.2

      - uses: dtolnay/rust-toolchain@stable

      - uses: Swatinem/rust-cache@779680da715d629ac1d338a641029a2f4372abb5  # v2.8.2

      - name: Install Linux GUI dependencies
        run: |
          sudo apt-get update -qq
          sudo apt-get install -y --no-install-recommends \
            libwayland-dev libxkbcommon-dev libx11-dev libxi-dev libxrandr-dev

      - name: cargo doc (warnings as errors)
        env:
          RUSTDOCFLAGS: -D warnings
        run: cargo doc --no-deps --workspace
```

**Step 3: Commit**

```
git add .github/workflows/ci.yml <any-fixed-doc-files>
git commit -m "ci: add rustdoc warnings-as-errors job"
```

---

### Task 9: Feature exclusivity arch tests

Add tests to `arch_boundaries.rs` documenting and verifying the `hardware` + `emulator` mutual exclusion and the `time-driver-tim2` requirement. These tests provide a living specification of compile-time guarantees.

**Files:**
- Modify: `crates/firmware/tests/arch_boundaries.rs`

**Step 1: Read the existing arch_boundaries.rs**

Read the full file to understand its structure and find the right place to add the new tests.

**Step 2: Add the tests**

At the end of `arch_boundaries.rs`, add:

```rust
// ─── Feature Flag Invariants ─────────────────────────────────────────────────

/// Verify that `hardware` and `emulator` are not simultaneously active.
///
/// These features are mutually exclusive:
/// - `hardware` pulls in embassy-stm32, cortex-m, defmt (no_std, ARM-only)
/// - `emulator` pulls in tokio, winit (std, desktop-only)
///
/// The compile-time enforcement is in `crates/firmware/build.rs`:
/// ```
/// if cfg!(feature = "hardware") && cfg!(feature = "emulator") {
///     panic!("hardware and emulator features are mutually exclusive");
/// }
/// ```
///
/// This test verifies the runtime invariant and documents the expected behavior.
/// The arch job in CI also never uses --all-features for exactly this reason.
#[test]
fn hardware_and_emulator_features_are_mutually_exclusive() {
    let is_hardware = cfg!(feature = "hardware");
    let is_emulator = cfg!(feature = "emulator");
    assert!(
        !(is_hardware && is_emulator),
        "hardware and emulator features must not be active simultaneously. \
         See crates/firmware/build.rs for compile-time enforcement and \
         .github/workflows/ci.yml (NOTE on feature flag exclusivity) for CI guidance."
    );
}

/// Verify that embassy-stm32 uses `time-driver-tim2` rather than `time-driver-any`.
///
/// `time-driver-any` causes a linker error when multiple crates in the same
/// binary each try to claim the generic timer interrupt. `time-driver-tim2`
/// pins the driver to TIM2 and gives it a specific interrupt symbol name,
/// avoiding the conflict.
///
/// Enforcement: `Cargo.toml` workspace dependencies must specify
/// `embassy-stm32 = { features = ["time-driver-tim2", ...] }` explicitly.
/// This test documents the requirement; if the constraint is ever accidentally
/// removed, the embedded build will fail with a linker error.
#[test]
fn embassy_time_driver_is_tim2_not_any() {
    // This is a documentation test — the actual enforcement is at link time
    // (the embedded build fails if time-driver-any is used instead of tim2).
    // The test exists to document the constraint and provide a search anchor
    // for engineers who encounter the linker error.
    //
    // Verify: grep Cargo.toml for "time-driver-tim2" should match.
    // grep Cargo.toml for "time-driver-any" should NOT match.
    let workspace_cargo = include_str!("../../../Cargo.toml");
    assert!(
        workspace_cargo.contains("time-driver-tim2"),
        "Cargo.toml must use embassy-stm32 with time-driver-tim2 feature"
    );
    assert!(
        !workspace_cargo.contains("time-driver-any"),
        "Cargo.toml must NOT use time-driver-any (causes linker conflict)"
    );
}

/// Verify that the firmware crate's Cargo.toml does not accidentally enable
/// both hardware and emulator features in any single feature group.
///
/// A feature like `full = ["hardware", "emulator"]` would silently pass
/// `cargo check` but fail to link on any target.
#[test]
fn no_single_feature_enables_both_hardware_and_emulator() {
    let firmware_cargo = include_str!("../../Cargo.toml");

    // Parse [features] section lines that contain BOTH "hardware" AND "emulator"
    // on the same line (which would mean a single feature enables both).
    for line in firmware_cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue; // skip comments
        }
        if trimmed.contains("\"hardware\"") && trimmed.contains("\"emulator\"") {
            panic!(
                "Cargo.toml line enables both hardware and emulator features: {:?}",
                trimmed
            );
        }
    }
}
```

**Step 3: Run the tests**

```
cargo test -p firmware --test arch_boundaries 2>&1 | grep -E "mutually|tim2|single_feature|ok|FAILED"
```
Expected: all 3 tests pass.

**Step 4: Commit**

```
git add crates/firmware/tests/arch_boundaries.rs
git commit -m "test(arch): document hardware/emulator mutual exclusion + time-driver-tim2 invariants"
```

---

### Task 10: Per-section binary size limits (.text / .data / .bss)

The existing `binary-size` CI job checks only the `.text` section. Add `.data` and `.bss` limits to catch large lookup tables accidentally placed in RAM, or zero-init buffers that should live in external SDRAM.

**Files:**
- Modify: `.github/workflows/ci.yml`

**Step 1: Understand current section layout**

Build locally and measure:
```
cargo build --package firmware --release --target thumbv7em-none-eabihf --features hardware
arm-none-eabi-size target/thumbv7em-none-eabihf/release/firmware
```

Note the actual `.text`, `.data`, and `.bss` values. The limits below are conservative headroom values — adjust if the actual sizes differ.

**Step 2: Replace the existing "Check binary size" step**

In the `binary-size` job, find the step:
```yaml
      - name: Check binary size (arm-none-eabi-size, flash section)
```

Replace it (and everything after it in that job) with:

```yaml
      - name: Check binary size (per-section limits)
        run: |
          ELF=target/thumbv7em-none-eabihf/release/firmware

          # Print the standard size table for CI log readability
          echo "=== arm-none-eabi-size ==="
          arm-none-eabi-size "$ELF"

          # Extract individual section sizes
          read TEXT DATA BSS <<< $(arm-none-eabi-size "$ELF" | tail -1 | awk '{print $1, $2, $3}')
          FLASH_TOTAL=$((TEXT + DATA))

          echo ""
          echo "=== Section limits ==="
          echo "  .text  (code + read-only data, stored in flash): ${TEXT} bytes"
          echo "  .data  (initialised RAM variables, stored in flash, copied at boot): ${DATA} bytes"
          echo "  .bss   (zero-initialised RAM, not stored in flash): ${BSS} bytes"
          echo "  Flash total (.text + .data): ${FLASH_TOTAL} bytes"

          # ── .text limit ──────────────────────────────────────────────────────
          # STM32H743ZI has 2 MB dual-bank flash. Leave 200 KB headroom for OTA.
          MAX_TEXT=1887436   # 1.8 * 1024 * 1024
          if [ "$TEXT" -gt "$MAX_TEXT" ]; then
            echo "ERROR: .text ${TEXT} bytes exceeds 1.8 MB limit (${MAX_TEXT} bytes)"
            echo "  Run: cargo size --release --target thumbv7em-none-eabihf -p firmware --features hardware -- -A"
            exit 1
          fi

          # ── .data limit ──────────────────────────────────────────────────────
          # Initialised variables (non-zero statics) are stored in flash and
          # copied to RAM at boot. Large .data typically means font/LUT tables
          # that should instead live in QSPI NOR flash (read via DMA, not copied).
          MAX_DATA=65536   # 64 KB — large .data = wrong; use QSPI for tables
          if [ "$DATA" -gt "$MAX_DATA" ]; then
            echo "ERROR: .data ${DATA} bytes exceeds 64 KB limit (${MAX_DATA} bytes)"
            echo "  Large .data usually means static lookup tables that should live in QSPI NOR."
            echo "  Mark them #[link_section = \".qspi\"] or load them lazily from NOR flash."
            exit 1
          fi

          # ── .bss limit ───────────────────────────────────────────────────────
          # Zero-initialised statics (heapless buffers, DMA regions).
          # Internal SRAM is 992 KB total; external SDRAM is 32-64 MB.
          # Buffers over 256 KB that aren't DMA-critical should use external SDRAM.
          MAX_BSS=786432   # 768 KB — generous; actual target SRAM is ~992 KB total
          if [ "$BSS" -gt "$MAX_BSS" ]; then
            echo "ERROR: .bss ${BSS} bytes exceeds 768 KB limit (${MAX_BSS} bytes)"
            echo "  Large .bss usually means big buffers that should use external SDRAM at 0xC0000000."
            exit 1
          fi

          echo ""
          echo "=== OK ==="
          echo "  .text  ${TEXT} / ${MAX_TEXT} bytes ($(( TEXT * 100 / MAX_TEXT ))% of limit)"
          echo "  .data  ${DATA} / ${MAX_DATA} bytes ($(( DATA * 100 / MAX_DATA ))% of limit)"
          echo "  .bss   ${BSS} / ${MAX_BSS} bytes ($(( BSS * 100 / MAX_BSS ))% of limit)"
          echo "  Flash  ${FLASH_TOTAL} bytes total"
```

**Step 3: Verify the job works locally**

If you have `arm-none-eabi-size` installed:
```
ELF=target/thumbv7em-none-eabihf/release/firmware
arm-none-eabi-size "$ELF" | tail -1 | awk '{print $1, $2, $3}'
```

**Step 4: Commit**

```
git add .github/workflows/ci.yml
git commit -m "ci: expand binary-size job with per-section (.text/.data/.bss) limits"
```

---

## Verification

After all 10 tasks, run the full verification:

```bash
# All lints clean
cargo clippy --workspace --lib --tests 2>&1 | grep "error\["

# All tests pass
cargo test --workspace 2>&1 | tail -5

# Hardware target compiles
cargo check --target thumbv7em-none-eabihf --features hardware -p firmware

# Doc build clean
RUSTDOCFLAGS="-D warnings" cargo doc --no-deps --workspace

# Arch boundary tests pass
cargo test -p firmware --test arch_boundaries 2>&1 | tail -5
```

---

## Summary Table

| Task | What it enforces |
|------|-----------------|
| 1. todo/unimplemented/dbg_macro/await_holding_lock | No placeholder code, no debug prints, no async deadlocks |
| 2. Cast safety lints | Truncating/sign-losing casts (the `.min(50) as u8` bug class) |
| 3. Undocumented unsafe | Every unsafe block has a SAFETY rationale |
| 4. Missing docs | Public APIs have documentation |
| 5. No-std CI job | std types can't accidentally leak into embedded crates |
| 6. Align32 + arch tests | DMA buffers have 32-byte Cortex-M7 cacheline alignment |
| 7. cargo-geiger | Unsafe code count cannot silently increase |
| 8. Rustdoc CI | Broken doc links and malformed examples caught at CI time |
| 9. Feature exclusivity arch tests | hardware+emulator can't collide; time-driver-tim2 is enforced |
| 10. Per-section binary size | Flash/RAM budget tracked by section, not just ELF total |
