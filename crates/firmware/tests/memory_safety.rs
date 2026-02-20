//! Memory safety architecture tests.
//! Tests verify DMA buffer placement invariants at declaration level.
//! Runtime address verification is done via debug_assert! in main() startup.

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

/// Firmware must contain a runtime debug_assert! for FRAMEBUFFER address placement.
/// String-grep tests cannot verify the linker actually placed the buffer correctly.
/// The runtime debug_assert! provides defense-in-depth for the link_section attribute.
///
/// Note: on host (cargo test) the firmware never runs, so the debug_assert! is not
/// executed — this test checks for its PRESENCE in the source to ensure the assertion
/// exists for when the firmware boots on actual hardware.
#[test]
fn firmware_has_runtime_dma_buffer_address_assertion() {
    let main_rs = include_str!("../src/main.rs");
    // The assertion must use debug_assert! (not just comments mentioning 0x2400_0000).
    // It must also reference addr_of or AXI_SRAM_BASE — not just debug_assert! alone,
    // because debug_assert! may be used for other things too.
    let has_debug_assert = main_rs.contains("debug_assert");
    let has_address_check =
        main_rs.contains("addr_of") || main_rs.contains("AXI_SRAM_BASE");

    assert!(
        has_debug_assert && has_address_check,
        "firmware/src/main.rs must contain a runtime debug_assert! with an address-range check.\n\
         Expected both 'debug_assert' and either 'addr_of' or 'AXI_SRAM_BASE' in source.\n\
         Use:\n\
           debug_assert!(\n\
               core::ptr::addr_of!(*_framebuffer) as u32 >= platform::dma_safety::AXI_SRAM_BASE,\n\
               \"FRAMEBUFFER not in AXI SRAM — missing #[link_section]\"\n\
           );\n\
         This catches linker misconfiguration that #[link_section] alone cannot prevent at compile time.\n\
         Note: debug_assert! is compiled out in release builds (debug-assertions = false)."
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
