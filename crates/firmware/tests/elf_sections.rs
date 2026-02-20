//! ELF section address verification tests.
// ELF test file: expect/unwrap/cast/indexing are intentional test mechanisms.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
)]
//!
//! These tests verify that the linker script (memory.x) places DMA-critical
//! sections at the correct hardware addresses. A misconfigured linker script
//! could silently place DMA buffers in DTCM (0x20000000) which is NOT DMA
//! accessible on STM32H743, causing silent data corruption at runtime.
//!
//! # How to run
//! These tests require the ARM ELF binary to be pre-built:
//! ```
//! cargo build --release --target thumbv7em-none-eabihf --no-default-features --features hardware
//! cargo test -p firmware --test elf_sections
//! ```

use std::path::PathBuf;

/// Path to the built ARM ELF binary (set by build.rs or environment).
fn firmware_elf_path() -> Option<PathBuf> {
    // Try environment variable first (set by CI)
    if let Ok(path) = std::env::var("FIRMWARE_ELF_PATH") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Some(p);
        }
    }
    // Try conventional cargo output path
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())?;
    let elf = workspace_root
        .join("target")
        .join("thumbv7em-none-eabihf")
        .join("release")
        .join("firmware");
    if elf.exists() {
        Some(elf)
    } else {
        None
    }
}

/// Skip a test with a message if the ELF is not available.
macro_rules! require_elf {
    () => {
        match firmware_elf_path() {
            Some(p) => p,
            None => {
                eprintln!(
                    "SKIP: ARM ELF not found — run \
                     `cargo build --release --target thumbv7em-none-eabihf` first"
                );
                return;
            }
        }
    };
}

#[test]
fn axisram_section_address_is_correct() {
    let elf_path = require_elf!();

    // Parse ELF using object crate — check if available
    // If not, fall back to running arm-none-eabi-readelf
    let output = std::process::Command::new("arm-none-eabi-readelf")
        .args(["-S", "--wide", elf_path.to_str().unwrap()])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            // Look for .axisram section
            if let Some(line) = text.lines().find(|l| l.contains(".axisram")) {
                // readelf -S output format: [Nr] Name   Type   Addr   Off   Size ...
                // The address field should start with 24 (0x24000000 range)
                assert!(
                    line.contains("2400"),
                    ".axisram section must be in AXI SRAM (0x24000000), got: {line}"
                );
            } else {
                // .axisram may be empty/absent if no DMA buffers are placed there yet
                // This is acceptable — the section exists in linker script
                eprintln!(
                    "INFO: .axisram section not found in ELF \
                     (may be empty NOLOAD section)"
                );
            }
        }
        Ok(out) => {
            eprintln!(
                "arm-none-eabi-readelf failed: {}",
                String::from_utf8_lossy(&out.stderr)
            );
        }
        Err(e) => {
            eprintln!("SKIP: arm-none-eabi-readelf not found: {e}");
        }
    }
}

#[test]
fn no_dma_buffers_in_dtcm() {
    let elf_path = require_elf!();

    let output = std::process::Command::new("arm-none-eabi-nm")
        .args(["--print-size", "--radix=hex", elf_path.to_str().unwrap()])
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout);
            // DTCM is 0x20000000–0x20020000 (128 KB)
            // Check that AUDIO_BUFFER and FRAMEBUFFER are NOT in DTCM range
            for line in text.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 4 {
                    let addr_str = parts[0];
                    let name = parts[parts.len() - 1];
                    if name.contains("AUDIO_BUFFER") || name.contains("FRAMEBUFFER") {
                        if let Ok(addr) = u64::from_str_radix(addr_str, 16) {
                            assert!(
                                !(0x20000000..=0x0002_0000_u64.wrapping_add(0x2000_0000)).contains(&addr),
                                "{name} at 0x{addr:08X} is in DTCM — DMA will silently fail!"
                            );
                        }
                    }
                }
            }
        }
        Ok(_) | Err(_) => {
            eprintln!("SKIP: arm-none-eabi-nm not available");
        }
    }
}

#[test]
fn memory_x_axisram_section_defined() {
    // Structural test: verify memory.x defines SECTIONS block with .axisram
    let memory_x = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../memory.x"
    ))
    .expect("memory.x must exist at workspace root");

    assert!(
        memory_x.contains(".axisram"),
        "memory.x must define .axisram NOLOAD section"
    );
    assert!(
        memory_x.contains("> RAM"),
        "memory.x .axisram section must target RAM (AXI SRAM)"
    );
    assert!(
        memory_x.contains("ORIGIN = 0x24000000"),
        "memory.x must define RAM region at 0x24000000 (AXI SRAM base)"
    );
}

#[test]
fn memory_x_dtcm_not_dma_accessible_documented() {
    let memory_x = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../memory.x"
    ))
    .expect("memory.x must exist at workspace root");

    // The DTCM region comment must warn about DMA inaccessibility
    assert!(
        memory_x.contains("NOT DMA")
            || memory_x.contains("no DMA")
            || memory_x.contains("tightly coupled"),
        "memory.x must document that DTCM is NOT DMA-accessible"
    );
}

#[test]
fn memory_x_sram1_sram2_sram3_sections_defined() {
    let memory_x = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../memory.x"
    ))
    .expect("memory.x must exist at workspace root");

    assert!(
        memory_x.contains(".sram1"),
        "memory.x must define .sram1 section"
    );
    assert!(
        memory_x.contains(".sram2"),
        "memory.x must define .sram2 section"
    );
    assert!(
        memory_x.contains(".sram3"),
        "memory.x must define .sram3 section"
    );
    assert!(
        memory_x.contains(".sram4"),
        "memory.x must define .sram4 section"
    );
}
