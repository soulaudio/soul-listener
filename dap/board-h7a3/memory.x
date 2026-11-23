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
 *
 * AHB_SRAM = AHB-SRAM1 (64K @ 0x30000000) + AHB-SRAM2 (64K @ 0x30020000),
 * declared as ONE 128K region because the two are address-contiguous
 * (0x30000000 + 0x20000 == 0x30020000), so a single buffer larger than 64K --
 * e.g. the 96 KB e-ink framebuffer -- can span them. CD domain, DMA-capable.
 * `dap-arch-tests`' `linker_script_regions_match_the_rust_memory_map` parses
 * this file and pins both regions against `memory_map`, so the Rust-side map
 * and the linker can no longer drift (they already had: `USER_SRAM` claimed
 * 1208K of a part with 1152K, while this file mapped only the 1024K AXI).
 *
 * Nothing is placed in AHB_SRAM yet. THREE prerequisites before anything is --
 * see `.ahbram` below and `dap_core::power::peripherals`' SRAM1/SRAM2 notes:
 *   1. RCC: SRAM1EN/SRAM2EN (RCC_AHB2ENR bits 29/30) must be SET before any
 *      access. They are currently in `MUST_STAY_GATED` precisely because this
 *      region was unlinked; a read or write with the clock off bus-faults.
 *   2. Zero-init: cortex-m-rt's startup zeroes `.bss` in RAM only. `.ahbram` is
 *      NOLOAD and NOT zeroed, so anything relying on a zeroed start (a
 *      `StaticCell`'s `used` flag, an `AtomicBool`, an `Option` discriminant)
 *      is UNSOUND there until a pre_init zeroes it.
 *   3. Cache/DMA: if the D-cache is ever enabled, this region needs an MPU
 *      non-cacheable entry and 32-byte-aligned buffers (AN4839). The `.ahbram`
 *      section is ALIGN(32) to make the alignment half free.
 */
MEMORY
{
    FLASH : ORIGIN = 0x08000000, LENGTH = 2048K
    RAM   : ORIGIN = 0x24000000, LENGTH = 1024K
    AHB_SRAM : ORIGIN = 0x30000000, LENGTH = 128K
}

/* An output section so a static can be placed with
 * `#[link_section = ".ahbram"]`, rather than every future caller inventing its
 * own. NOLOAD: this is RAM that is never loaded from flash, and deliberately
 * not initialised -- see prerequisite 2 above. ALIGN(32) at both ends so every
 * buffer in here starts and ends on a Cortex-M7 cache line, which is what the
 * MPU/DMA discipline needs if caches are enabled later.
 *
 * INSERT AFTER .bss keeps cortex-m-rt's own link.x in charge of everything
 * else; this only appends a region it does not know about. The __sahbram /
 * __eahbram symbols exist so a pre_init can zero the span when prerequisite 2
 * is taken on.
 */
SECTIONS
{
  .ahbram (NOLOAD) : ALIGN(32)
  {
    __sahbram = .;
    *(.ahbram .ahbram.*);
    . = ALIGN(32);
    __eahbram = .;
  } > AHB_SRAM
} INSERT AFTER .bss;
