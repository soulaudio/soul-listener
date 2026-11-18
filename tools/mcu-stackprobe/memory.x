/* Generic Cortex-M7 memory map for the stack-probe harness.
 *
 * This is NOT the real H7A3 memory.x (see the repo root's memory.x for that
 * one, used by the old H743 `firmware` crate and inherited by dap/board-h7a3
 * via embassy-stm32's `memory-x` feature). This harness only needs to LINK
 * and run far enough for -Z emit-stack-sizes to record every function's
 * frame size in the ELF; it never touches real hardware. cortex-m-rt's
 * link.x hardcodes the region names FLASH/RAM, so those names are required
 * here regardless of the real target MCU's actual SRAM layout.
 */
MEMORY
{
    FLASH : ORIGIN = 0x08000000, LENGTH = 2M
    RAM   : ORIGIN = 0x24000000, LENGTH = 1M
}

/* cortex-m-rt's link.x sets _stack_start = ORIGIN(RAM) + LENGTH(RAM) by
 * default when this isn't overridden; left implicit here. */
