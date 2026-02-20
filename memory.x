MEMORY
{
    /* STM32H743ZI — 2 MB dual-bank Flash, 1 MB SRAM                       */
    /* Flash bank 1 starts at 0x0800_0000 (bank 2 at 0x0810_0000)          */
    FLASH  (rx)  : ORIGIN = 0x08000000, LENGTH = 2048K

    /* AXI SRAM (512 KB) — D1 domain; accessible by all bus masters + DMA  */
    /* This is the default RAM region used by cortex-m-rt for .data/.bss    */
    RAM    (xrw) : ORIGIN = 0x24000000, LENGTH = 512K

    /* DTCM (128 KB) — tightly coupled to Cortex-M7; fastest for stack/ISR */
    /* NOT DMA-accessible — do not place DMA buffers here                   */
    DTCM   (xrw) : ORIGIN = 0x20000000, LENGTH = 128K

    /* SRAM1 (128 KB) — D2 domain, DMA-accessible via AHB                  */
    SRAM1  (xrw) : ORIGIN = 0x30000000, LENGTH = 128K

    /* SRAM2 (128 KB) — D2 domain, DMA-accessible via AHB                  */
    SRAM2  (xrw) : ORIGIN = 0x30020000, LENGTH = 128K

    /* SRAM3 (32 KB) — D2 domain, DMA-accessible via AHB                   */
    /* DS12110 Rev 9 Table 4: contiguous with SRAM1/2 at 0x30040000        */
    SRAM3  (xrw) : ORIGIN = 0x30040000, LENGTH = 32K

    /* SRAM4 (64 KB) — D3 domain, accessible by BDMA only                  */
    /* Required for: SPI6, SAI4, LPUART1, I2C4, ADC3                       */
    SRAM4  (xrw) : ORIGIN = 0x38000000, LENGTH = 64K

    /* External SDRAM (32 MB) — W9825G6KH6 via FMC Bank 5                  */
    /* Holds: library index cache, album-art cache, FLAC decode scratch     */
    EXTSDRAM (xrw) : ORIGIN = 0xC0000000, LENGTH = 32M

    /* External QSPI NOR Flash (16 MB) — W25Q128JV via QUADSPI             */
    /* Holds: fonts, icons, waveform LUTs, OTA staging partition (XiP)     */
    QSPI   (rx)  : ORIGIN = 0x90000000, LENGTH = 16M
}

/* Stack goes at the top of AXI SRAM (cortex-m-rt default)                 */
/* With flip-link, the linker inverts the layout: the stack is placed BELOW  */
/* .bss+.data so a stack overflow triggers a HardFault (RAM boundary hit)    */
/* rather than silently corrupting DMA buffers. _stack_start anchors the top.*/
_stack_start = ORIGIN(RAM) + LENGTH(RAM);

/* Minimum stack size guard (informational; enforced by flip-link at link time).*/
/* If the static footprint grows too large, a linker error is emitted before   */
/* the firmware can be flashed. 32 KB is conservative for the Embassy async    */
/* executor with several concurrent tasks.                                      */
_min_stack_size = 32768;

/* ── Custom output sections ─────────────────────────────────────────────── */
/*                                                                           */
/* These sections are placed AFTER the regions defined by cortex-m-rt's     */
/* link.x.  cortex-m-rt includes memory.x via INCLUDE, so any SECTIONS      */
/* block here is merged with (and appended after) link.x's own sections.    */
/*                                                                           */
/* Usage in Rust:                                                            */
/*   #[link_section = ".axisram"]   — DMA1/DMA2/MDMA buffers (D1 domain)   */
/*   #[link_section = ".sram4"]     — BDMA buffers (SPI6, SAI4, etc.)      */
/*   #[link_section = ".extsdram"]  — large caches that need not survive    */
/*                                    power-cycle (uninitialised NOLOAD)    */

SECTIONS
{
    /* AXI SRAM section: DMA-accessible buffers for SAI, SDMMC, SPI display */
    /* Aligned to 8 bytes so StaticCell<[u32; N]> works correctly           */
    .axisram (NOLOAD) : ALIGN(8)
    {
        *(.axisram .axisram.*);
        . = ALIGN(8);
    } > RAM

    /* SRAM4 section: BDMA-accessible buffers (SPI6, SAI4, LPUART1, I2C4)  */
    .sram4 (NOLOAD) : ALIGN(4)
    {
        *(.sram4 .sram4.*);
        . = ALIGN(4);
    } > SRAM4

    /* External SDRAM section: large uninitialised buffers                  */
    /* FMC SDRAM must be initialised by firmware before any access          */
    .extsdram (NOLOAD) : ALIGN(4)
    {
        *(.extsdram .extsdram.*);
        . = ALIGN(4);
    } > EXTSDRAM
}
