/* Memory map for the stack-probe harness, laid out for QEMU's `mps2-an500`
 * (Cortex-M7) machine so the SAME binary that `-Z emit-stack-sizes` reads
 * statically can also be RUN under QEMU for a dynamic stack high-water
 * measurement (the `qemu` feature + tools/mcu-stackprobe/check-stack-qemu.sh).
 *
 * This is NOT the real H7A3 map (the dap/board-h7a3 crate carries that). Frame
 * sizes in `.stack_sizes` are codegen-determined and address-INDEPENDENT, so
 * using the mps2 addresses here does not change the static measurement at all;
 * it just makes the binary bootable on QEMU's mps2-an500 (code/ZBT-SSRAM at
 * 0x00000000, data SRAM at 0x20000000, 4 MB each in QEMU's model). cortex-m-rt's
 * link.x hardcodes the region names FLASH/RAM, so those names are required.
 */
MEMORY
{
    FLASH : ORIGIN = 0x00000000, LENGTH = 4M
    RAM   : ORIGIN = 0x20000000, LENGTH = 4M
}

/* cortex-m-rt's link.x sets _stack_start = ORIGIN(RAM) + LENGTH(RAM) by
 * default when this isn't overridden; left implicit here. The 4 MB RAM gives
 * the dynamic QEMU run megabytes of paintable stack — far more than the
 * hundreds of KB the open->decode->pump transient needs. */
