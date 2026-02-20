# Hardware-in-Loop (HIL) Tests

These tests require a physical STM32H743ZI board connected via probe-rs (ST-Link or J-Link).

## Prerequisites

1. Install probe-rs: `cargo install probe-rs --features cli`
2. Connect board via SWD (CN14 on NUCLEO-H743ZI2)
3. Ensure target power is on

## Running HIL Tests

```bash
# Run all hardware tests
cargo test --features hardware --target thumbv7em-none-eabihf

# Run with probe-rs runner (configured in .cargo/config.toml)
cargo embed --release --features hardware

# Run specific test
cargo test --features hardware --target thumbv7em-none-eabihf -- boot_sequence_completes
```

## Test Structure

Each test:
1. Resets the MCU via SWD
2. Waits for probe-rs RTT output
3. Checks expected log output
4. Times out after 10 seconds

## Test List

| Test | What it validates |
|---|---|
| `boot_sequence_completes` | MPU config → SDRAM → Embassy init without HardFault |
| `watchdog_fires_on_deadlock` | IWDG triggers reset if main task stops heartbeat |
| `display_spi_responds` | SPI2 clock + CS toggling, BUSY pin responds |
| `audio_i2s_clocks_running` | PLL3 lock + SAI1 MCLK present on scope/logic analyzer |
| `pmic_i2c_responds` | BQ25895 ACKs on I2C2 at 0x6A |
| `dac_i2c_responds` | ES9038Q2M ACKs on I2C3 at 0x48 |

## Adding Tests

HIL tests use `defmt-test` crate:
```rust
#[defmt_test::tests]
mod tests {
    #[test]
    fn boot_sequence_completes() {
        // defmt::assert!(some_condition)
    }
}
```
