MEMORY
{
    /* STM32H743ZI — 2 MB dual-bank Flash, 1 MB SRAM                       */
    /* Flash bank 1 starts at 0x0800_0000 (bank 2 at 0x0810_0000)          */
    FLASH  : ORIGIN = 0x08000000, LENGTH = 2048K

    /* AXI SRAM (512 KB) — default RAM region; accessible by all bus masters */
    RAM    : ORIGIN = 0x24000000, LENGTH = 512K

    /* DTCM (128 KB) — tightly coupled to Cortex-M7; fastest for stack/ISR  */
    DTCM   : ORIGIN = 0x20000000, LENGTH = 128K

    /* SRAM1 + SRAM2 (128 KB each) — accessible via AHB                     */
    SRAM1  : ORIGIN = 0x30000000, LENGTH = 128K
    SRAM2  : ORIGIN = 0x30020000, LENGTH = 128K

    /* SRAM4 (64 KB) — in D3 domain, accessible by BDMA                     */
    SRAM4  : ORIGIN = 0x38000000, LENGTH = 64K
}

/* cortex-m-rt places .data/.bss/.stack in RAM by default */
