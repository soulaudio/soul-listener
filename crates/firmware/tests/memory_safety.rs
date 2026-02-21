//! Memory safety architecture tests.
// Architecture test file: expect/unwrap and cast lints are intentional.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
)]
//! Tests verify DMA buffer placement invariants at declaration level.
//! Runtime address verification is done via assert! in main() startup.

/// FRAMEBUFFER must use StaticCell with #[link_section = ".axisram"] together.
/// Having StaticCell alone (in DTCM) or link_section alone (with static mut)
/// would be incorrect. Both attributes on the same declaration are required.
#[test]
fn framebuffer_uses_static_cell_with_axisram_link_section() {
    let main_rs = include_str!("../src/main.rs");

    // Find the FRAMEBUFFER declaration block
    // It must have BOTH #[link_section = ".axisram"] AND StaticCell on the same item
    let has_link_section_axisram = main_rs.contains(r#"link_section = ".axisram""#);
    // Use "static FRAMEBUFFER" (the declaration), not just "FRAMEBUFFER" which also
    // matches use-imports like `use firmware::{..., FRAMEBUFFER_SIZE}`.
    let has_static_cell_framebuffer =
        main_rs.contains("StaticCell") && main_rs.contains("static FRAMEBUFFER");

    assert!(
        has_link_section_axisram,
        "FRAMEBUFFER must have #[link_section = \".axisram\"] to land in DMA-accessible AXI SRAM.\n\
         Without this, the linker places it in .bss (which maps to AXI SRAM in this memory.x, \n\
         but is fragile and undocumented — explicit is safer)."
    );

    assert!(
        has_static_cell_framebuffer,
        "FRAMEBUFFER must use StaticCell<T> for sound aliasing under Rust's memory model.\n\
         static mut is UB when any reference is taken (Rust 2024 static_mut_refs = deny)."
    );

    // Structural check: link_section must appear BEFORE the static FRAMEBUFFER declaration
    // (attributes must be on the static item, not elsewhere in the file)
    let link_section_pos = main_rs.find(r#"link_section = ".axisram""#).unwrap();
    let framebuffer_decl_pos = main_rs.find("static FRAMEBUFFER").unwrap();
    assert!(
        framebuffer_decl_pos > link_section_pos
            && framebuffer_decl_pos - link_section_pos < 200,
        "The #[link_section = \".axisram\"] attribute must be adjacent to the FRAMEBUFFER declaration.\n\
         Found link_section at byte {} and `static FRAMEBUFFER` at byte {} — {} bytes apart (must be < 200).",
        link_section_pos,
        framebuffer_decl_pos,
        framebuffer_decl_pos - link_section_pos
    );
}

/// Firmware must contain a runtime assert! for FRAMEBUFFER address placement.
/// String-grep tests cannot verify the linker actually placed the buffer correctly.
/// The runtime assert! provides defense-in-depth for the link_section attribute.
///
/// Note: on host (cargo test) the firmware never runs, so the assert! is not
/// executed — this test checks for its PRESENCE in the source to ensure the assertion
/// exists for when the firmware boots on actual hardware.
#[test]
fn firmware_has_runtime_dma_buffer_address_assertion() {
    let main_rs = include_str!("../src/main.rs");
    // The assertion must use assert! (not just comments mentioning 0x2400_0000).
    // It must also reference addr_of or AXI_SRAM_BASE — not just assert! alone,
    // because assert! is used for other purposes too.
    let has_assert = main_rs.contains("assert");
    let has_address_check =
        main_rs.contains("addr_of") || main_rs.contains("AXI_SRAM_BASE");

    assert!(
        has_assert && has_address_check,
        "firmware/src/main.rs must contain a runtime assert! with an address-range check.\n\
         Expected 'assert' and either 'addr_of' or 'AXI_SRAM_BASE' in source.\n\
         Use:\n\
           assert!(\n\
               core::ptr::addr_of!(*_framebuffer) as u32 >= platform::dma_safety::AXI_SRAM_BASE,\n\
               \"FRAMEBUFFER not in AXI SRAM — missing #[link_section]\"\n\
           );\n\
         This catches linker misconfiguration that #[link_section] alone cannot prevent at compile time.\n\
         Note: Use assert! (not debug_assert!) -- debug_assert! is compiled out in release builds."
    );
}

/// Watchdog guard must use AtomicBool with explicit Ordering (not just any AtomicBool).
/// Relaxed ordering is wrong for a visibility guarantee across task boundaries.
#[test]
fn watchdog_heartbeat_uses_correct_atomic_ordering() {
    let main_rs = include_str!("../src/main.rs");
    // Must have AtomicBool AND explicit Ordering usage
    assert!(
        main_rs.contains("AtomicBool"),
        "Watchdog guard must use AtomicBool for per-task heartbeat flags."
    );
    assert!(
        main_rs.contains("Ordering::Release")
            || main_rs.contains("Ordering::AcqRel")
            || main_rs.contains("Ordering::SeqCst"),
        "AtomicBool watchdog heartbeat must use explicit Ordering (Release/AcqRel).\n\
         Ordering::Relaxed does not provide the happens-before guarantee needed for\n\
         the main loop to observe task heartbeat stores from other tasks."
    );
}

/// All large static DMA buffers must pair #[link_section] with sound ownership.
/// This test verifies that `static mut` declarations do NOT appear alongside
/// link_section attributes (which would be UB under Rust's aliasing model).
///
/// Note: `&'static mut` in expression context is safe — only bare `static mut`
/// declarations are problematic. This test matches the full `\nstatic mut ` pattern
/// (newline + "static mut ") to avoid false positives from `&'static mut`.
#[test]
fn no_static_mut_with_link_section_in_firmware() {
    let main_rs = include_str!("../src/main.rs");
    // A correct declaration:    #[link_section = ".axisram"]\nstatic FRAMEBUFFER: StaticCell<...>
    // An incorrect declaration: #[link_section = ".axisram"]\nstatic mut FRAMEBUFFER: [u8; ...]
    //
    // We match "\nstatic mut " (at line start) to avoid matching &'static mut in expressions.
    // The latter is legitimate; only top-level static mut declarations are problematic.
    let has_static_mut_decl = main_rs.contains("\nstatic mut ");

    if has_static_mut_decl {
        let mut search_from = 0;
        while let Some(rel) = main_rs[search_from..].find("\nstatic mut ") {
            let static_mut_pos = search_from + rel;
            let context_start = static_mut_pos.saturating_sub(300);
            let context = &main_rs[context_start..static_mut_pos];
            assert!(
                !context.contains(r#"link_section = ".axisram""#),
                "static mut must not be combined with #[link_section = \".axisram\"]. \
                 Use StaticCell<T> instead for sound aliasing semantics.\n\
                 Found 'static mut' declaration at byte {} with link_section in preceding 300 bytes.",
                static_mut_pos
            );
            search_from = static_mut_pos + 1;
        }
    }
}

/// Audio DMA buffer must use DmaBuffer<AxiSramRegion> wrapper type — not a bare array.
///
/// Using `StaticCell<DmaBuffer<AxiSramRegion, [u8; N]>>` encodes the DMA-accessible
/// region requirement into the type system. A bare `StaticCell<[u8; N]>` with only
/// a comment for documentation provides no compile-time enforcement.
#[test]
fn audio_buffer_uses_dma_buffer_wrapper() {
    let main_rs = include_str!("../src/main.rs");
    // Must contain `DmaBuffer<AxiSramRegion` in the AUDIO_BUFFER declaration
    assert!(
        main_rs.contains("DmaBuffer<AxiSramRegion"),
        "AUDIO_BUFFER must use DmaBuffer<AxiSramRegion, ...> wrapper type.\n\
         A bare StaticCell<[u8; AUDIO_DMA_BUFFER_BYTES]> provides no compile-time DMA\n\
         region enforcement. Required declaration:\n\
           #[link_section = \".axisram\"]\n\
           static AUDIO_BUFFER: StaticCell<DmaBuffer<AxiSramRegion, [u8; AUDIO_DMA_BUFFER_BYTES]>>\n\
               = StaticCell::new();"
    );
}

/// The AUDIO_BUFFER static must carry #[link_section = ".axisram"] to guarantee
/// physical placement in DMA-accessible AXI SRAM (0x2400_0000).
///
/// Without this attribute, the linker may place the buffer in DTCM (CPU-only,
/// NOT DMA-accessible), causing SAI1 DMA transfers to silently produce garbage.
#[test]
fn audio_buffer_link_section_axisram() {
    let main_rs = include_str!("../src/main.rs");
    // Must have AUDIO_BUFFER static
    assert!(
        main_rs.contains("static AUDIO_BUFFER"),
        "AUDIO_BUFFER must be declared as a static item in main.rs.
         Expected: static AUDIO_BUFFER: StaticCell<DmaBuffer<AxiSramRegion, ...>>"
    );

    // Verify link_section appears in the 200 bytes IMMEDIATELY BEFORE the AUDIO_BUFFER
    // declaration. Using rfind() would find the LAST link_section in the file, which could
    // be a string literal in a debug_assert! message — not the attribute on the static.
    // Instead we look at the slice of text just before the declaration.
    let audio_buf_pos = main_rs.find("static AUDIO_BUFFER").unwrap();
    let look_back_start = audio_buf_pos.saturating_sub(200);
    let preceding_text = &main_rs[look_back_start..audio_buf_pos];

    assert!(
        preceding_text.contains(r#"link_section = ".axisram""#),
        "#[link_section = \"\".axisram\"\"\"] must appear in the 200 bytes immediately before
         `static AUDIO_BUFFER`. This attribute ensures the buffer lands in AXI SRAM (DMA-accessible)
         rather than DTCM (CPU-only, NOT DMA-accessible for SAI1 DMA).
         Required:
           #[link_section = \".axisram\"]
           static AUDIO_BUFFER: StaticCell<DmaBuffer<AxiSramRegion, [u8; AUDIO_DMA_BUFFER_BYTES]>>"
    );
}
/// SAI audio initialization must not remain as a commented-out block.
///
/// A TODO comment or commented-out code block indicates the wiring is missing.
/// The SAI init must be a real function call (even a stub) that compiles, so
/// the type system verifies the DMA buffer type at the call site.
#[test]
fn sai_init_not_commented_out() {
    let main_rs = include_str!("../src/main.rs");
    // Must NOT have the SAI init in a commented-out block (// let sai = Sai::new...)
    // We check that `audio_task` is called — meaning audio is wired, not just TODO'd.
    let has_audio_task_call = main_rs.contains("audio_task");
    assert!(
        has_audio_task_call,
        "SAI audio must be wired via an audio_task call — not left as a commented-out block.\n\
         A `// let sai = Sai::new(...)` comment provides no compile-time safety.\n\
         Replace with: firmware::audio::sai_task::audio_task(buffer)"
    );
}

/// Every #[link_section = ".axisram"] static must wrap its data in Align32 or DmaBuffer.
///
/// Cortex-M7 has a 32-byte cacheline.  DMA buffers not aligned to 32 bytes cause
/// cache coherency bugs: a CPU store to a neighbouring variable in the same cacheline
/// can corrupt the DMA buffer after cache flush (ST AN4839 §3.3).
///
/// This test scans main.rs for every occurrence of link_section=.axisram and
/// verifies that Align32 or DmaBuffer appears within 5 lines (the attribute + declaration block).
#[test]
fn all_axisram_statics_use_align32_or_dma_buffer() {
    let main_rs = include_str!("../src/main.rs");
    let lines: Vec<&str> = main_rs.lines().collect();

    for (i, line) in lines.iter().enumerate() {
        // Only match real Rust attribute lines: must start with `#[` after trimming.
        // This skips comment lines (// ...), string literals in assert messages, and doc-examples.
        if !line.trim().starts_with("#[") {
            continue;
        }
        if line.contains(r#"link_section = ".axisram""#) {
            // Check the next 5 lines for Align32 or DmaBuffer
            let window_end = (i + 5).min(lines.len());
            let window = lines.get(i..window_end).unwrap_or(&[]).join("\n");
            assert!(
                window.contains("Align32") || window.contains("DmaBuffer"),
                "Found #[link_section = \".axisram\"] at line {} without Align32 or DmaBuffer wrapper.\n\
                 All AXI SRAM statics must use Align32<T> or DmaBuffer<R,T> to ensure 32-byte cache-line alignment.\n\
                 See ST AN4839 §3.3 — misaligned DMA buffers cause cache coherency corruption.\n\
                 Line content: {}",
                i + 1,
                line.trim()
            );
        }
    }
}

/// Every StaticCell wrapping a large raw byte array must use Align32 or DmaBuffer.
///
/// StaticCell<[u8; N]> where N >= 4096 (4 KB) is almost certainly a DMA buffer.
/// Without Align32 or DmaBuffer, the buffer may not be 32-byte aligned, risking cache corruption.
/// Use StaticCell<Align32<[u8; N]>> or StaticCell<DmaBuffer<R, [u8; N]>> instead.
#[test]
fn large_static_cell_byte_arrays_use_align32_or_dma_buffer() {
    let main_rs = include_str!("../src/main.rs");

    for (i, line) in main_rs.lines().enumerate() {
        if line.contains("StaticCell<[u8;") && !line.contains("Align32") && !line.contains("DmaBuffer") {
            // Try to extract the array size
            // Pattern: StaticCell<[u8; 12345]>
            let size_hint = line
                .split("StaticCell<[u8;")
                .nth(1)
                .and_then(|s| s.split(']').next())
                .map(|s| s.trim().replace('_', ""))
                .and_then(|s| s.parse::<usize>().ok());

            if size_hint.is_none_or(|n| n >= 4096) {
                panic!(
                    "Found StaticCell<[u8; ...]> at line {} without Align32 or DmaBuffer wrapper.\n\
                     Use StaticCell<Align32<[u8; N]>> for DMA-capable buffers.\n\
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
