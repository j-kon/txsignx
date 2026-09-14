use txsignx_policy::{FindingLocation, PolicyConfig, PolicyError, Severity};
#[test]
fn configuration_defaults_and_inclusive_bounds() {
    let config = PolicyConfig::default();
    assert_eq!(config.max_absolute_fee_sats, 100_000);
    assert_eq!(config.max_fee_ratio_bps, 1_000);
    for bps in [0, 1, 10_000] {
        assert!(
            PolicyConfig {
                max_absolute_fee_sats: u64::MAX,
                max_fee_ratio_bps: bps
            }
            .validate()
            .is_ok()
        );
    }
}
#[test]
fn invalid_ratio_is_typed_and_configuration_is_serializable() {
    let config = PolicyConfig {
        max_absolute_fee_sats: 0,
        max_fee_ratio_bps: 10_001,
    };
    assert_eq!(config.validate(), Err(PolicyError::InvalidFeeRatio));
    assert_eq!(
        serde_json::to_value(PolicyConfig::default()).unwrap(),
        serde_json::json!({"max_absolute_fee_sats":100000,"max_fee_ratio_bps":1000})
    );
}
#[test]
fn locations_and_severity_use_stable_primitives() {
    assert_eq!(
        serde_json::to_value(FindingLocation::Input { index: 3 }).unwrap(),
        serde_json::json!({"type":"input","index":3})
    );
    assert_eq!(
        serde_json::to_value(FindingLocation::Global).unwrap(),
        serde_json::json!({"type":"global"})
    );
    assert_eq!(
        serde_json::to_value(Severity::Critical).unwrap(),
        "critical"
    );
}
