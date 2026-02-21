//! Integration test: simulates a complete firmware boot sequence using mock peripherals.
//!
//! Tests that:
//!   1. Boot constants are self-consistent (timing, clock, memory map)
//!   2. MPU region configuration is correct (count, addresses, attributes)
//!   3. SDRAM timing constants compile and compute correctly
//!   4. Watchdog configuration is valid for the 8 s hardware timeout
//!   5. Audio power sequencer completes the full power-on sequence
//!      using mock I2C and GPIO -- verifying every register write and pin state
//!   6. PMIC init writes all required registers to the mock bus
//!   7. DAC init starts muted; fully unmuted after sequencer
//!   8. DMA buffer sizes fit within the AXI SRAM budget
//!
//! Does NOT require physical hardware.
//!
//! Run with: cargo test -p firmware --test integration_boot_sequence

// Integration test file -- intentional test patterns permitted.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::assertions_on_constants,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::cast_lossless,
    clippy::arithmetic_side_effects,
)]

use platform::audio_sequencer::AudioPowerSequencer;
use platform::bq25895;
use platform::es9038q2m;

// -- Mock I2C bus ---------------------------------------------------------

struct MockI2cBus {
    writes: std::vec::Vec<(u8, std::vec::Vec<u8>)>,
}

impl MockI2cBus {
    fn new() -> Self { Self { writes: std::vec::Vec::new() } }
    fn write_count(&self) -> usize { self.writes.len() }
    fn wrote_to_addr(&self, addr: u8) -> bool {
        self.writes.iter().any(|(a, _)| *a == addr)
    }
    fn wrote_register(&self, addr: u8, reg: u8) -> bool {
        self.writes.iter().any(|(a, data)| *a == addr && data.first() == Some(&reg))
    }
    fn wrote_register_value(&self, addr: u8, reg: u8, value: u8) -> bool {
        self.writes.iter().any(|(a, data)| {
            *a == addr && data.first() == Some(&reg) && data.get(1) == Some(&value)
        })
    }
    fn writes_for_register(&self, addr: u8, reg: u8) -> std::vec::Vec<&std::vec::Vec<u8>> {
        self.writes
            .iter()
            .filter(|(a, data)| *a == addr && data.first() == Some(&reg))
            .map(|(_, data)| data)
            .collect()
    }
}

impl embedded_hal::i2c::ErrorType for MockI2cBus {
    type Error = core::convert::Infallible;
}

impl embedded_hal::i2c::I2c for MockI2cBus {
    fn transaction(
        &mut self,
        address: u8,
        operations: &mut [embedded_hal::i2c::Operation],
    ) -> Result<(), Self::Error> {
        for op in operations.iter() {
            if let embedded_hal::i2c::Operation::Write(data) = op {
                self.writes.push((address, data.to_vec()));
            }
        }
        Ok(())
    }
}

// -- Mock GPIO pin --------------------------------------------------------

struct MockOutputPin { high: bool }

impl MockOutputPin {
    fn new() -> Self { Self { high: false } }
    fn is_high(&self) -> bool { self.high }
    fn is_low(&self) -> bool { !self.high }
}

impl embedded_hal::digital::ErrorType for MockOutputPin {
    type Error = core::convert::Infallible;
}

impl embedded_hal::digital::OutputPin for MockOutputPin {
    fn set_high(&mut self) -> Result<(), Self::Error> { self.high = true; Ok(()) }
    fn set_low(&mut self) -> Result<(), Self::Error> { self.high = false; Ok(()) }
}

// -- Audio sequencer integration tests ------------------------------------

/// Verify the full power-on sequence writes correct I2C registers and
/// leaves the amp GPIO high.
#[test]
fn full_audio_power_on_sequence_writes_correct_registers() {
    let mut i2c = MockI2cBus::new();
    let mut amp_gpio = MockOutputPin::new();
    let dac_addr = es9038q2m::ES9038Q2M_I2C_ADDR_LOW;

    let seq = AudioPowerSequencer::new();
    let seq = seq
        .mute_dac_with_i2c(&mut i2c, dac_addr)
        .expect("mute_dac_with_i2c must not fail with infallible mock");
    let seq = seq
        .enable_amp_with_gpio(&mut amp_gpio)
        .expect("enable_amp_with_gpio must not fail with infallible mock");
    let _seq = seq
        .unmute_dac_with_i2c(&mut i2c, dac_addr)
        .expect("unmute_dac_with_i2c must not fail with infallible mock");

    assert!(
        i2c.wrote_register(dac_addr, es9038q2m::REG_ATT_L),
        "mute_dac_with_i2c must write REG_ATT_L to DAC at 0x{dac_addr:02X}"
    );
    assert!(
        i2c.wrote_register(dac_addr, es9038q2m::REG_ATT_R),
        "mute_dac_with_i2c must write REG_ATT_R to DAC at 0x{dac_addr:02X}"
    );
    assert!(
        amp_gpio.is_high(),
        "enable_amp_with_gpio must drive TPA6120A2 SHUTDOWN pin high"
    );

    let att_l_writes = i2c.writes_for_register(dac_addr, es9038q2m::REG_ATT_L);
    assert_eq!(att_l_writes.len(), 2,
        "REG_ATT_L must be written exactly twice: mute (0xFF) then unmute (0x00)");

    let att_r_writes = i2c.writes_for_register(dac_addr, es9038q2m::REG_ATT_R);
    assert_eq!(att_r_writes.len(), 2,
        "REG_ATT_R must be written exactly twice: mute (0xFF) then unmute (0x00)");

    assert_eq!(att_l_writes[0].get(1), Some(&es9038q2m::ATT_MUTED),
        "First ATT_L write must be ATT_MUTED (0xFF)");
    assert_eq!(att_l_writes[1].get(1), Some(&es9038q2m::ATT_FULL_VOLUME),
        "Second ATT_L write must be ATT_FULL_VOLUME (0x00)");
    assert_eq!(i2c.write_count(), 4,
        "Power-on sequence must produce exactly 4 I2C writes");
}

/// Verify amp GPIO starts low, stays low after mute, then goes high on enable.
#[test]
fn amp_gpio_starts_low_then_goes_high_on_enable() {
    let mut i2c = MockI2cBus::new();
    let mut amp_gpio = MockOutputPin::new();
    let dac_addr = es9038q2m::ES9038Q2M_I2C_ADDR_LOW;
    assert!(amp_gpio.is_low(), "TPA6120A2 SHUTDOWN must start low (amp disabled)");
    let seq = AudioPowerSequencer::new()
        .mute_dac_with_i2c(&mut i2c, dac_addr)
        .unwrap();
    assert!(amp_gpio.is_low(), "GPIO must remain low after mute step");
    let _seq = seq.enable_amp_with_gpio(&mut amp_gpio).unwrap();
    assert!(amp_gpio.is_high(), "GPIO must go high after enable_amp_with_gpio");
}

/// Verify the power-down sequence drives GPIO low from FullyOn state.
#[test]
fn audio_power_down_sequence_drives_gpio_low() {
    let mut i2c = MockI2cBus::new();
    let mut amp_gpio = MockOutputPin::new();
    let dac_addr = es9038q2m::ES9038Q2M_I2C_ADDR_LOW;
    let fully_on = AudioPowerSequencer::new()
        .mute_dac_with_i2c(&mut i2c, dac_addr).unwrap()
        .enable_amp_with_gpio(&mut amp_gpio).unwrap()
        .unmute_dac_with_i2c(&mut i2c, dac_addr).unwrap();
    assert!(amp_gpio.is_high(), "GPIO must be high in FullyOn state");
    let muted = fully_on
        .mute_dac_for_shutdown_with_i2c(&mut i2c, dac_addr)
        .expect("mute_dac_for_shutdown_with_i2c must succeed");
    let _off = muted
        .disable_amp_with_gpio(&mut amp_gpio)
        .expect("disable_amp_with_gpio must succeed");
    assert!(amp_gpio.is_low(),
        "TPA6120A2 SHUTDOWN must be low after power-down sequence");
}

/// Verify complete power-on + power-off cycle: 4 + 2 = 6 total I2C writes.
#[test]
fn power_cycle_produces_correct_i2c_write_count() {
    let mut i2c = MockI2cBus::new();
    let mut amp_gpio = MockOutputPin::new();
    let dac_addr = es9038q2m::ES9038Q2M_I2C_ADDR_LOW;
    let fully_on = AudioPowerSequencer::new()
        .mute_dac_with_i2c(&mut i2c, dac_addr).unwrap()
        .enable_amp_with_gpio(&mut amp_gpio).unwrap()
        .unmute_dac_with_i2c(&mut i2c, dac_addr).unwrap();
    assert_eq!(i2c.write_count(), 4, "Power-on must produce 4 I2C writes");
    let muted = fully_on.mute_dac_for_shutdown_with_i2c(&mut i2c, dac_addr).unwrap();
    let _off = muted.disable_amp_with_gpio(&mut amp_gpio).unwrap();
    assert_eq!(i2c.write_count(), 6,
        "Full power-on + power-off cycle must produce exactly 6 I2C writes");
}

// -- PMIC integration tests -----------------------------------------------

/// Verify bq25895_init writes all required configuration registers:
/// REG00 (input current), REG02 (charge current), REG04 (charge voltage).
#[test]
fn pmic_init_writes_all_required_registers() {
    let mut i2c = MockI2cBus::new();
    bq25895::bq25895_init(&mut i2c, bq25895::BQ25895_I2C_ADDR)
        .expect("bq25895_init must not fail with infallible mock I2C");
    assert!(
        i2c.wrote_to_addr(bq25895::BQ25895_I2C_ADDR),
        "PMIC init must write to BQ25895 at 0x{:02X}", bq25895::BQ25895_I2C_ADDR
    );
    assert!(
        i2c.wrote_register(bq25895::BQ25895_I2C_ADDR, bq25895::REG00_INPUT_SOURCE),
        "PMIC init must write REG00 (input current limit)"
    );
    assert!(
        i2c.wrote_register(bq25895::BQ25895_I2C_ADDR, bq25895::REG02_CHARGE_CURRENT),
        "PMIC init must write REG02 (charge current)"
    );
    assert!(
        i2c.wrote_register(bq25895::BQ25895_I2C_ADDR, bq25895::REG04_CHARGE_VOLTAGE),
        "PMIC init must write REG04 (charge voltage 4.208 V)"
    );
    assert!(
        i2c.wrote_register_value(
            bq25895::BQ25895_I2C_ADDR,
            bq25895::REG04_CHARGE_VOLTAGE,
            bq25895::VREG_4208MV
        ),
        "REG04 must be written with VREG_4208MV (0x{:02X})", bq25895::VREG_4208MV
    );
    assert!(
        i2c.write_count() >= 3,
        "PMIC init must write at least 3 registers; wrote {}", i2c.write_count()
    );
}

/// Verify bq25895_init only writes to the PMIC address.
#[test]
fn pmic_init_does_not_write_to_wrong_address() {
    let mut i2c = MockI2cBus::new();
    bq25895::bq25895_init(&mut i2c, bq25895::BQ25895_I2C_ADDR).unwrap();
    for (addr, _) in &i2c.writes {
        assert_eq!(
            *addr, bq25895::BQ25895_I2C_ADDR,
            "PMIC init must only write to BQ25895 (0x{:02X}), not 0x{addr:02X}",
            bq25895::BQ25895_I2C_ADDR
        );
    }
}

// -- DAC init integration tests -------------------------------------------

/// Verify es9038q2m_init starts with ATT_L and ATT_R set to MUTED (0xFF).
#[test]
fn dac_init_sequence_starts_muted() {
    let mut i2c = MockI2cBus::new();
    es9038q2m::es9038q2m_init(&mut i2c, es9038q2m::ES9038Q2M_I2C_ADDR_LOW)
        .expect("es9038q2m_init must not fail with infallible mock I2C");
    let att_l_writes =
        i2c.writes_for_register(es9038q2m::ES9038Q2M_I2C_ADDR_LOW, es9038q2m::REG_ATT_L);
    let att_r_writes =
        i2c.writes_for_register(es9038q2m::ES9038Q2M_I2C_ADDR_LOW, es9038q2m::REG_ATT_R);
    assert_eq!(att_l_writes.len(), 1, "DAC init must write REG_ATT_L exactly once");
    assert_eq!(att_r_writes.len(), 1, "DAC init must write REG_ATT_R exactly once");
    assert_eq!(att_l_writes[0].get(1), Some(&es9038q2m::ATT_MUTED),
        "DAC init must write ATT_MUTED (0xFF) to REG_ATT_L");
    assert_eq!(att_r_writes[0].get(1), Some(&es9038q2m::ATT_MUTED),
        "DAC init must write ATT_MUTED (0xFF) to REG_ATT_R");
}

/// Verify es9038q2m_init performs a soft reset (assert bit0=1, then de-assert).
#[test]
fn dac_init_performs_soft_reset() {
    let mut i2c = MockI2cBus::new();
    es9038q2m::es9038q2m_init(&mut i2c, es9038q2m::ES9038Q2M_I2C_ADDR_LOW).unwrap();
    let system_writes =
        i2c.writes_for_register(es9038q2m::ES9038Q2M_I2C_ADDR_LOW, es9038q2m::REG_SYSTEM);
    assert!(system_writes.len() >= 2,
        "DAC init must write REG_SYSTEM at least twice; got {} writes",
        system_writes.len());
    let first_val = system_writes[0].get(1).copied().unwrap_or(0);
    assert_eq!(first_val & 0x01, 1,
        "First REG_SYSTEM write must assert soft reset (bit 0=1); got 0x{first_val:02X}");
    let second_val = system_writes[1].get(1).copied().unwrap_or(0xFF);
    assert_eq!(second_val & 0x01, 0,
        "Second REG_SYSTEM write must de-assert reset (bit 0=0); got 0x{second_val:02X}");
}

/// Verify es9038q2m_init writes only to the DAC address.
#[test]
fn dac_init_writes_to_correct_i2c_address() {
    let mut i2c = MockI2cBus::new();
    es9038q2m::es9038q2m_init(&mut i2c, es9038q2m::ES9038Q2M_I2C_ADDR_LOW).unwrap();
    assert!(i2c.write_count() > 0, "es9038q2m_init must produce at least one I2C write");
    for (addr, _) in &i2c.writes {
        assert_eq!(
            *addr, es9038q2m::ES9038Q2M_I2C_ADDR_LOW,
            "All DAC init writes must target 0x{:02X}, not 0x{addr:02X}",
            es9038q2m::ES9038Q2M_I2C_ADDR_LOW
        );
    }
}

// -- Memory map integration tests -----------------------------------------

/// Verify AXI SRAM and DTCM are distinct, non-overlapping regions.
#[test]
fn boot_memory_regions_do_not_overlap() {
    use platform::dma_safety::{AXI_SRAM_BASE, AXI_SRAM_SIZE_BYTES, EXTSDRAM_BASE};
    let dtcm_base: usize = 0x2000_0000;
    let dtcm_end: usize = dtcm_base + 128 * 1024;
    let axi_base = AXI_SRAM_BASE as usize;
    let axi_end = axi_base + AXI_SRAM_SIZE_BYTES;
    assert!(axi_base >= dtcm_end,
        "AXI SRAM (0x{axi_base:08X}) must start after DTCM ends (0x{dtcm_end:08X})");
    let sdram_base = EXTSDRAM_BASE as usize;
    assert!(axi_end <= sdram_base,
        "AXI SRAM end (0x{axi_end:08X}) must not reach SDRAM (0x{sdram_base:08X})");
}

/// Verify audio DMA + two framebuffers fit in AXI SRAM.
#[test]
fn audio_dma_buffer_fits_in_axi_sram_budget() {
    use platform::dma_safety::{
        AUDIO_DMA_BUFFER_BYTES, AXI_SRAM_SIZE_BYTES, FRAMEBUFFER_SIZE_BYTES,
    };
    let required = AUDIO_DMA_BUFFER_BYTES * 2 + FRAMEBUFFER_SIZE_BYTES * 2;
    assert!(required <= AXI_SRAM_SIZE_BYTES,
        "DMA+FB budget ({required} bytes) fits in AXI SRAM ({AXI_SRAM_SIZE_BYTES} bytes)");
    assert_eq!(AUDIO_DMA_BUFFER_BYTES, 16_384,
        "AUDIO_DMA_BUFFER_BYTES must be 16384 (2048 x 2ch x 4 bytes)");
    assert_eq!(FRAMEBUFFER_SIZE_BYTES, 96_000,
        "FRAMEBUFFER_SIZE_BYTES must be 96000 (800 x 480 / 4 px/byte)");
}

// -- Watchdog configuration tests -----------------------------------------

/// Verify the IWDG timeout constant is exactly 8 seconds.
#[test]
fn watchdog_timeout_constant_is_8_seconds() {
    use firmware::boot::WATCHDOG_TIMEOUT_MS;
    assert_eq!(WATCHDOG_TIMEOUT_MS, 8_000,
        "WATCHDOG_TIMEOUT_MS must be 8000 ms (8 s)");
}

/// Verify the watchdog timeout is in a reasonable embedded range.
#[test]
fn watchdog_timeout_is_in_reasonable_range() {
    use firmware::boot::WATCHDOG_TIMEOUT_MS;
    let min_ms: u32 = 5_000;
    let max_ms: u32 = 30_000;
    assert!(WATCHDOG_TIMEOUT_MS >= min_ms,
        "WATCHDOG_TIMEOUT_MS ({WATCHDOG_TIMEOUT_MS} ms) >= {min_ms} ms required");
    assert!(WATCHDOG_TIMEOUT_MS <= max_ms,
        "WATCHDOG_TIMEOUT_MS ({WATCHDOG_TIMEOUT_MS} ms) <= {max_ms} ms required");
}

// -- MPU configuration tests ----------------------------------------------

/// Verify the SoulAudio MPU config produces exactly 3 non-cacheable region pairs.
#[test]
fn mpu_config_produces_exactly_three_non_cacheable_regions() {
    use platform::mpu::MpuApplier;
    let pairs = MpuApplier::soul_audio_register_pairs();
    assert_eq!(pairs.len(), 3,
        "SoulAudio MPU boot must configure 3 non-cacheable regions; got {}",
        pairs.len());
}

/// Verify AXI SRAM is region 0 at base 0x2400_0000 with ENABLE set in RASR.
#[test]
fn mpu_region_0_is_axi_sram_at_correct_base() {
    use platform::mpu::MpuApplier;
    let pairs = MpuApplier::soul_audio_register_pairs();
    let (rbar0, rasr0) = pairs[0];
    let base_addr = rbar0 & 0xFFFF_FFE0;
    assert_eq!(base_addr, 0x2400_0000,
        "MPU region 0 must have AXI SRAM base 0x2400_0000; RBAR = 0x{rbar0:08X}");
    assert_ne!(rasr0 & 1, 0,
        "MPU region 0 RASR must have ENABLE bit set; RASR = 0x{rasr0:08X}");
}

/// Verify SRAM4 is region 1 at base 0x3800_0000 with ENABLE set in RASR.
#[test]
fn mpu_region_1_is_sram4_at_correct_base() {
    use platform::mpu::MpuApplier;
    let pairs = MpuApplier::soul_audio_register_pairs();
    let (rbar1, rasr1) = pairs[1];
    let base_addr = rbar1 & 0xFFFF_FFE0;
    assert_eq!(base_addr, 0x3800_0000,
        "MPU region 1 must have SRAM4 base 0x3800_0000; RBAR = 0x{rbar1:08X}");
    assert_ne!(rasr1 & 1, 0,
        "MPU region 1 RASR must have ENABLE bit set; RASR = 0x{rasr1:08X}");
}

/// Verify AXI SRAM and SRAM4 MPU regions carry the NonCacheable attribute.
#[test]
fn mpu_regions_have_non_cacheable_attribute() {
    use platform::mpu::{MpuAttributes, SoulAudioMpuConfig};
    let axi = SoulAudioMpuConfig::axi_sram_dma_region();
    assert_eq!(axi.attrs(), MpuAttributes::NonCacheable,
        "AXI SRAM MPU region must be NonCacheable (ST AN4838)");
    let sram4 = SoulAudioMpuConfig::sram4_bdma_region();
    assert_eq!(sram4.attrs(), MpuAttributes::NonCacheable,
        "SRAM4 MPU region must be NonCacheable for BDMA safety");
}

// -- Clock / timing sanity tests ------------------------------------------

/// Verify the SDRAM refresh count formula at W9825G6KH-6 / 100 MHz.
#[test]
fn sdram_refresh_count_formula_is_correct() {
    use platform::sdram::{sdram_refresh_count, W9825G6KH6_REFRESH_COUNT};
    let count = sdram_refresh_count(100_000_000, 8192, 64);
    assert_eq!(count, 761,
        "W9825G6KH-6 refresh count must be 761 at 100 MHz FMC clock");
    assert_eq!(W9825G6KH6_REFRESH_COUNT, 761,
        "W9825G6KH6_REFRESH_COUNT must match formula result");
}

/// Verify PLL3P 49.152 MHz / 256 = 192 kHz audio sample rate.
#[test]
fn audio_mclk_divided_by_256_equals_192khz() {
    let mclk_hz: u32 = 49_152_000;
    let mclk_fs_ratio: u32 = 256;
    let sample_rate = mclk_hz / mclk_fs_ratio;
    assert_eq!(sample_rate, 192_000,
        "PLL3P {mclk_hz} Hz / {mclk_fs_ratio} must yield 192 kHz");
}

/// Verify the default QSPI prescaler keeps flash clock within W25Q128JV limits.
#[test]
fn qspi_clock_within_w25q128jv_spec() {
    use platform::qspi_config::{validate_qspi_prescaler, QSPI_PRESCALER};
    validate_qspi_prescaler(240_000_000, QSPI_PRESCALER)
        .expect("QSPI clock must be within W25Q128JV 133 MHz limit");
}
