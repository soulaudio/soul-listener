/* STM32H7A3ZI memory map.
 *
 * Provided explicitly in the crate root rather than via embassy-stm32's
 * `memory-x` feature. embassy's `memory-x` writes this file into its build
 * OUT_DIR and adds that dir to the linker search path; `flip-link` (our linker,
 * for zero-cost stack-overflow protection) scans the linker scripts itself to
 * locate the `RAM` region and does NOT follow that build-script search path, so
 * the CI cross-build failed with:
 *     Error: "MEMORY.RAM not found after scanning linker scripts"
 * A crate-root memory.x is copied to cortex-m-rt's OUT_DIR and sits first on the
 * search path, so both rust-lld and flip-link find it. (A host `clippy`/`check`
 * never links, which is why this only ever surfaced in the full ARM build.)
 *
 * Values mirror embassy-stm32's stm32-data-derived map for this part:
 *   FLASH 2 MB (dual-bank) @ 0x0800_0000
 *   AXI SRAM (D1 domain, DMA-capable) @ 0x2400_0000
 * Keep `memory-x` OFF in board-h7a3's embassy-stm32 features so there is exactly
 * one memory.x on the search path (two would be ambiguous).
 */
MEMORY
{
    FLASH : ORIGIN = 0x08000000, LENGTH = 2048K  /* BANK_1 + BANK_2            */
    RAM   : ORIGIN = 0x24000000, LENGTH = 1024K  /* AXI SRAM (D1), DMA-capable */
}
