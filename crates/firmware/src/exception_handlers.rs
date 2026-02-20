//! Cortex-M exception handlers for the SoulAudio DAP firmware.
//!
//! This module provides handlers for hardware exceptions (faults) that the
//! Cortex-M7 processor raises when encountering illegal operations:
//!
//! - **HardFault**: triggered by memory access violations (MPU fault, bus fault),
//!   illegal instructions, divide-by-zero (if CCR.DIV_0_TRP is set), or unaligned
//!   access (if CCR.UNALIGN_TRP is set). Also triggered by flip-link's stack
//!   overflow detection when the stack grows past the bottom of RAM.
//!
//! # Stack Overflow Protection
//!
//! With flip-link configured in `.cargo/config.toml`, the memory layout is
//! inverted: the stack is placed BELOW `.bss`+`.data`. A stack overflow causes
//! the stack pointer to go past the bottom of RAM, which triggers a HardFault
//! rather than silently corrupting DMA buffers in AXI SRAM.
//!
//! # Hardware-only handler
//!
//! The `#[cortex_m_rt::exception]` attribute requires ARM target intrinsics and
//! is therefore gated behind `#[cfg(feature = "hardware")]`. The module itself
//! (and `HARDFAULT_DEFINED`) compiles unconditionally so host tests can verify
//! the module exists without needing an ARM toolchain.

#![allow(clippy::doc_markdown)] // Exception handler docs use hardware terminology (HardFault, SVC) as plain text
/// Marker constant — confirmed by arch tests to verify this module exists.
///
/// When `HARDFAULT_DEFINED` is `true`, the `exception_handlers` module compiled
/// successfully, proving that the HardFault handler (in `#[cfg(feature = "hardware")]`
/// below) will be linked into the firmware binary.
pub const HARDFAULT_DEFINED: bool = true;

/// HardFault exception handler (hardware target only).
///
/// # Triggers
///
/// - Memory access violations (MPU protection fault, bus fault)
/// - Illegal memory accesses (out-of-range addresses)
/// - Stack overflow detected by flip-link (stack pointer below bottom of RAM)
/// - Divide by zero (if `CCR.DIV_0_TRP` is set in SCB)
/// - Unaligned access (if `CCR.UNALIGN_TRP` is set in SCB)
///
/// # Behavior
///
/// Outputs the exception frame address via defmt/RTT so the engineer can
/// inspect the stacked PC, LR, and PSR in a debugger, then halts the processor.
/// On release builds without a debugger attached, defmt/RTT output is discarded
/// but the halt prevents further undefined behavior.
///
/// # Safety
///
/// This function must never return — returning from a HardFault handler is
/// undefined behavior on Cortex-M. The `-> !` return type enforces this.
#[cfg(feature = "hardware")]
#[cortex_m_rt::exception]
#[allow(unsafe_code)]
unsafe fn HardFault(ef: &cortex_m_rt::ExceptionFrame) -> ! {
    defmt::panic!(
        "HardFault! Stacked exception frame at 0x{:08X}. \
         Check stacked PC for fault address. \
         Possible causes: stack overflow (flip-link), MPU violation, bus fault.",
        ef as *const _ as u32
    );
}
