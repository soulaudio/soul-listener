//! Property-based tests for audio domain math.
//! Verifies invariants hold for ALL valid inputs, not just fixed examples.

use platform::audio_types::{AttenuationRegister, SampleRateHz, VolumePercent};

proptest::proptest! {
    /// VolumePercent::new never panics for any u8 input (clamps to 100).
    #[test]
    fn volume_percent_new_never_panics(pct in 0u8..=255u8) {
        let v = VolumePercent::new(pct);
        // After clamping, value must be in 0..=100
        assert!(v.get() <= 100);
    }

    /// AttenuationRegister::from_volume never panics for any valid VolumePercent.
    #[test]
    fn attenuation_from_volume_never_panics(pct in 0u8..=100u8) {
        let v = VolumePercent::new(pct);
        let att = AttenuationRegister::from_volume(v);
        // Attenuation must be in valid u8 range (implicit in type)
        let _ = att.get();
    }

    /// Higher volume → lower or equal attenuation (monotone inverse relationship).
    #[test]
    fn attenuation_is_monotone_inverse_of_volume(a in 0u8..=100u8, b in 0u8..=100u8) {
        let va = VolumePercent::new(a);
        let vb = VolumePercent::new(b);
        let aa = AttenuationRegister::from_volume(va);
        let ab = AttenuationRegister::from_volume(vb);
        if a > b {
            // Higher volume (a) should give lower or equal attenuation
            assert!(aa.get() <= ab.get(),
                "volume {} → attenuation {} should be <= volume {} → attenuation {}",
                a, aa.get(), b, ab.get());
        } else if a < b {
            assert!(aa.get() >= ab.get(),
                "volume {} → attenuation {} should be >= volume {} → attenuation {}",
                a, aa.get(), b, ab.get());
        }
    }

    /// SampleRateHz::new never panics for any u32 input.
    #[test]
    fn sample_rate_hz_new_never_panics(hz in 0u32..=u32::MAX) {
        // May return Err but must not panic
        let _ = SampleRateHz::new(hz);
    }

    /// SampleRateHz valid range [8000, 768000] always succeeds.
    #[test]
    fn sample_rate_hz_valid_range_always_ok(hz in 8000u32..=768_000u32) {
        assert!(SampleRateHz::new(hz).is_ok(),
            "SampleRateHz::new({}) should be Ok within [8000, 768000]", hz);
    }

    /// SampleRateHz out of range always fails.
    #[test]
    fn sample_rate_hz_out_of_range_always_err(hz in 768_001u32..=u32::MAX) {
        assert!(SampleRateHz::new(hz).is_err(),
            "SampleRateHz::new({}) should be Err above 768000", hz);
    }
}
