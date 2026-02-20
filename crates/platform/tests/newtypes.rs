//! Type system enforcement tests for audio domain newtypes.
//! These newtypes prevent common embedded audio configuration bugs at compile time.

// ── VolumePercent ────────────────────────────────────────────────────────────

#[test]
fn volume_percent_new_clamps_over_100() {
    use platform::audio_types::VolumePercent;
    let v = VolumePercent::new(150);
    assert_eq!(v.get(), 100, "VolumePercent::new(150) should clamp to 100");
}

#[test]
fn volume_percent_new_allows_0() {
    use platform::audio_types::VolumePercent;
    let v = VolumePercent::new(0);
    assert_eq!(v.get(), 0);
}

#[test]
fn volume_percent_new_allows_100() {
    use platform::audio_types::VolumePercent;
    let v = VolumePercent::new(100);
    assert_eq!(v.get(), 100);
}

#[test]
fn volume_percent_try_new_rejects_over_100() {
    use platform::audio_types::VolumePercent;
    assert!(VolumePercent::try_new(101).is_err());
    assert!(VolumePercent::try_new(255).is_err());
}

#[test]
fn volume_percent_try_new_accepts_valid_range() {
    use platform::audio_types::VolumePercent;
    assert!(VolumePercent::try_new(0).is_ok());
    assert!(VolumePercent::try_new(50).is_ok());
    assert!(VolumePercent::try_new(100).is_ok());
}

#[test]
fn volume_percent_is_zero_sized_enough() {
    use platform::audio_types::VolumePercent;
    // VolumePercent wraps a u8 — should be exactly 1 byte
    assert_eq!(core::mem::size_of::<VolumePercent>(), 1);
}

// ── AttenuationRegister ──────────────────────────────────────────────────────

#[test]
fn attenuation_register_from_volume_100_is_min_attenuation() {
    use platform::audio_types::{AttenuationRegister, VolumePercent};
    // ES9038Q2M: volume 100% → attenuation 0 (no attenuation, full volume)
    // Register value 0x00 = 0 dB (full volume)
    // Register value 0xFF = maximum attenuation (mute)
    let vol = VolumePercent::new(100);
    let att = AttenuationRegister::from_volume(vol);
    assert_eq!(att.get(), 0, "100% volume should give 0 attenuation (0dB)");
}

#[test]
fn attenuation_register_from_volume_0_is_max_attenuation() {
    use platform::audio_types::{AttenuationRegister, VolumePercent};
    // ES9038Q2M: volume 0% → attenuation 255 (mute)
    let vol = VolumePercent::new(0);
    let att = AttenuationRegister::from_volume(vol);
    assert_eq!(
        att.get(),
        255,
        "0% volume should give 255 attenuation (mute)"
    );
}

#[test]
fn attenuation_register_from_volume_50_is_midpoint() {
    use platform::audio_types::{AttenuationRegister, VolumePercent};
    let vol = VolumePercent::new(50);
    let att = AttenuationRegister::from_volume(vol);
    // 50% volume → (100 - 50) * 255 / 100 = 127 (approximately)
    assert!(
        att.get() >= 125 && att.get() <= 130,
        "50% volume should give ~127 attenuation, got {}",
        att.get()
    );
}

#[test]
fn attenuation_register_is_one_byte() {
    use platform::audio_types::AttenuationRegister;
    assert_eq!(core::mem::size_of::<AttenuationRegister>(), 1);
}

// ── SampleRateHz ─────────────────────────────────────────────────────────────

#[test]
fn sample_rate_hz_rejects_zero() {
    use platform::audio_types::SampleRateHz;
    assert!(SampleRateHz::new(0).is_err());
}

#[test]
fn sample_rate_hz_rejects_below_minimum() {
    use platform::audio_types::SampleRateHz;
    // Minimum meaningful sample rate: 8000 Hz
    assert!(SampleRateHz::new(7999).is_err());
}

#[test]
fn sample_rate_hz_accepts_standard_rates() {
    use platform::audio_types::SampleRateHz;
    assert!(SampleRateHz::new(44100).is_ok());
    assert!(SampleRateHz::new(48000).is_ok());
    assert!(SampleRateHz::new(96000).is_ok());
    assert!(SampleRateHz::new(192000).is_ok());
}

#[test]
fn sample_rate_hz_rejects_above_maximum() {
    use platform::audio_types::SampleRateHz;
    // ES9038Q2M max: 768 kHz PCM
    assert!(SampleRateHz::new(768_001).is_err());
}

#[test]
fn sample_rate_hz_accepts_max_768khz() {
    use platform::audio_types::SampleRateHz;
    assert!(SampleRateHz::new(768_000).is_ok());
}

#[test]
fn sample_rate_hz_get_returns_value() {
    use platform::audio_types::SampleRateHz;
    let sr = SampleRateHz::new(192_000).unwrap();
    assert_eq!(sr.get(), 192_000);
}

// ── I2cAddr phantom type ──────────────────────────────────────────────────────

#[test]
fn i2c_addr_pmic_address_is_correct() {
    use platform::audio_types::{I2cAddr, I2cBus2};
    // BQ25895 PMIC is on I2C2, address 0x6A (hardware-fixed, no ADDR pin)
    // Reference: TI BQ25895 datasheet section 7.3.3, E2E forum #872345
    let addr: I2cAddr<I2cBus2> = I2cAddr::new(0x6A);
    assert_eq!(addr.get(), 0x6A);
}

#[test]
fn i2c_addr_dac_address_is_correct() {
    use platform::audio_types::{I2cAddr, I2cBus3};
    // ES9038Q2M DAC is on I2C3, address 0x48 (ADDR pin pulled low)
    let addr: I2cAddr<I2cBus3> = I2cAddr::new(0x48);
    assert_eq!(addr.get(), 0x48);
}

#[test]
fn i2c_addr_is_zero_cost_abstraction() {
    use platform::audio_types::{I2cAddr, I2cBus2};
    // PhantomData ensures no runtime overhead
    assert_eq!(core::mem::size_of::<I2cAddr<I2cBus2>>(), 1);
}

#[test]
fn i2c_addr_rejects_reserved_addresses() {
    use platform::audio_types::{I2cAddr, I2cBus2};
    // I2C reserved addresses: 0x00-0x07 and 0x78-0x7F
    assert!(I2cAddr::<I2cBus2>::try_new(0x00).is_err());
    assert!(I2cAddr::<I2cBus2>::try_new(0x07).is_err());
    assert!(I2cAddr::<I2cBus2>::try_new(0x78).is_err());
    assert!(I2cAddr::<I2cBus2>::try_new(0x7F).is_err());
}

#[test]
fn i2c_addr_accepts_valid_7bit_range() {
    use platform::audio_types::{I2cAddr, I2cBus2};
    assert!(I2cAddr::<I2cBus2>::try_new(0x08).is_ok());
    assert!(I2cAddr::<I2cBus2>::try_new(0x48).is_ok());
    assert!(I2cAddr::<I2cBus2>::try_new(0x77).is_ok());
}

// ── Integration: volume_to_attenuation helper ────────────────────────────────

#[test]
fn volume_to_attenuation_uses_newtypes() {
    use platform::audio_types::{AttenuationRegister, VolumePercent};
    // Verify the conversion is monotone: higher volume → lower attenuation
    let high_vol = AttenuationRegister::from_volume(VolumePercent::new(80));
    let low_vol = AttenuationRegister::from_volume(VolumePercent::new(20));
    assert!(
        high_vol.get() < low_vol.get(),
        "Higher volume should give lower attenuation register value"
    );
}
