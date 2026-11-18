/* STM32H7A3ZI memory map.
 *
 * Shipped in the crate root (not via embassy-stm32's `memory-x` feature) and
 * put on the linker search path by build.rs, so both rust-lld and flip-link
 * find the RAM region. Keep `memory-x` OFF so exactly one memory.x is on the
 * search path.
 *
 * Two linker-script gotchas this file avoids on purpose:
 *   - NO inline comments on the FLASH/RAM lines: flip-link's parser is line-
 *     oriented and drops a region whose line has a trailing comment, failing
 *     with "MEMORY.RAM not found after scanning linker scripts".
 *   - LF line endings only (enforced via .gitattributes): a CR makes rust-lld
 *     report `unknown directive: <CR>`.
 *
 * Values mirror embassy-stm32's stm32-data map: FLASH = 2 MB dual-bank at
 * 0x08000000; RAM = AXI SRAM (D1 domain, DMA-capable) at 0x24000000.
 */
MEMORY
{
    FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
    RAM   : ORIGIN = 0x24000000, LENGTH = 1024K
}
