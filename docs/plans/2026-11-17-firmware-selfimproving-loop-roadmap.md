# Firmware self-improving loop — roadmap to a continuously-validated, hardware-free DAP

> The long-term plan for getting the SoulAudio DAP firmware "truly working well"
> **before hardware exists**, via repeated self-improving loops (research best
> practices → build/measure → evaluate → improve → iterate), with CI gates that
> accumulate so each gain stays locked in. Target: STM32H7A3ZI, on-chip SRAM
> (no external PSRAM — confirmed fits). Companion app: soul-player-ng.

## Guiding principles
- **Test what you ship.** The hexagonal split (`dap-core`/`soul-audio-core` =
  board-agnostic logic; ports `Sink`/`Storage`/`Display`/`Controls`/`Power` =
  thin adapters) means QEMU/host tests run the **byte-identical core** the device
  ships; only the adapter layer is swapped. The CI-green firmware artifact is the
  artifact soul-player-ng flashes.
- **Measure, don't argue.** Prefer real thumbv7em numbers (static
  `emit-stack-sizes`/`cargo-call-stack` + dynamic QEMU stack-painting) over host
  proxies. Every loop iteration starts with web research on the topic's best
  practices.
- **Gates accumulate.** Each proven property (fits-the-stack, decodes-bit-exact,
  gapless, no-alloc) becomes a CI check so regressions are caught forever.

## The testing pyramid (all hardware-free until the last rung)
host `sim` (fast, broad) → **QEMU `mps2-an500` Cortex-M7** (real ARM binary:
decode + stack + logic, via semihosting) → **Renode** (optional: real STM32H7
peripheral models) → **HIL on a board** (last, smallest: just the real adapters).

## Phases (each is a loop; CI grows at every phase)

### Phase A — Measurement & CI foundation  ← START HERE
- A1: install QEMU; make the `tools/mcu-stackprobe` harness **runnable** under
  `qemu-system-arm -machine mps2-an500` (cortex-m-rt entry, panic-semihosting,
  stack-painting, semihosting output).
- A2: dynamically measure the open→decode→pump stack high-water per format;
  confirm (or tighten) the static ~450–570 KB worst-case; prove on-chip fit.
- A3: close the one edge — Matroska-wrapped-FLAC's 48 KB u8 SimpleBlock buffer
  (needs a byte-arena seam alongside the i32 scratch) — or reserve for it.
- A4: wire **CI jobs**: static stack bound (`cargo-call-stack`/`emit-stack-sizes`)
  + dynamic QEMU stack high-water + decode/gapless scenarios; gate on budget.

### Phase B — `firmware-core` factoring (same-as-shipped)
- Extract a board-agnostic firmware run-loop so `board-h7a3` (embassy HAL) and the
  QEMU test binary share the exact same core + swapped adapters.
- Expand QEMU scenarios: decode-all-formats bit-exactness on-target, gapless
  transitions, the App state machine (controls/power/display events).

### Phase C — Full-logic coverage + the deferred memory/battery work (task 17)
- On-target (QEMU) decode correctness for every format; resampler/pipeline;
  no-alloc enforcement; stack-high-water per format as CI gates.
- Battery/energy model validation + instruction-count timing under QEMU.
- Renode for peripheral-adapter integration where it adds signal.

### Phase D — Firmware delivery pipeline (companion-app model)
- DFU or a custom **A/B bootloader** (flash inactive slot, verify signature/CRC,
  switch-on-boot, auto-rollback) — robust field updates, can't brick.
- **soul-player-ng** "Update firmware" UI over USB (`nusb`/`rusb`, DFU/vendor
  protocol), beside the existing SD library-import flow — one companion hub:
  sync library + update firmware when the device is connected.
- The CI artifact == the flashed artifact.

### Phase E — Hardware bring-up (when a board exists)
- HIL tests for the real peripheral adapters (SAI/I²S DMA, e-ink SPI, SD, wheel/
  button controls, BQ fuel-gauge). The last mile QEMU can't cover.

## Status at roadmap creation
- On-chip fit: **confirmed** (actual at-rest ~549 KB now / ~720 KB full-UI +
  ~326–450 KB transient < 1,208 KB AXI SRAM). No PSRAM needed. One edge
  (Matroska-FLAC worst case) tracked in A3.
- Decoder memory: all big decoders externalized; single-decoder (PRELOAD=0)
  gapless; honest scratch-pool budget; soul-player-ng validated on desktop.
- Not yet pushed; `feat/embedded-profile` (core, merged with main) + `feat/eink-ui`
  (DAP + tools).
