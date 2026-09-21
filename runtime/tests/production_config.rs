// Integration tests link the library without cfg(test): check the configuration
// shipped in Wasm rather than the accelerated era settings used by unit tests.
use hypertensor_runtime::{BondingDuration, EpochDuration, SessionsPerEra, SLOT_DURATION};

#[test]
fn built_runtime_has_the_expected_staking_time_horizon() {
    let era_millis = EpochDuration::get() * SessionsPerEra::get() as u64 * SLOT_DURATION;
    if cfg!(feature = "fast-runtime") {
        assert_eq!(era_millis, 6 * 60 * 1000);
    } else {
        assert_eq!(era_millis, 24 * 60 * 60 * 1000);
        assert_eq!(
            era_millis * BondingDuration::get() as u64,
            28 * 24 * 60 * 60 * 1000
        );
    }
}
