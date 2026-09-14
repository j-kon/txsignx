mod common;
use txsignx_policy::*;
#[test]
fn development_registry_has_eight_unique_active_rules_in_order() {
    let engine = PolicyEngine::development().unwrap();
    let metadata = engine.metadata();
    assert_eq!(
        metadata.iter().map(|m| m.code).collect::<Vec<_>>(),
        vec![
            "TG002", "TG003", "TG009", "TG010", "TG011", "TG012", "TG013", "TG014"
        ]
    );
    assert!(
        metadata
            .iter()
            .all(|m| m.active && !m.required_context.is_empty())
    );
    let report = engine
        .evaluate(&common::inspection(), &PolicyConfig::default())
        .unwrap();
    assert_eq!(report.decision, PolicyDecision::Pass);
    assert_eq!(
        report.evaluated_rules,
        metadata
            .iter()
            .map(|m| m.code.to_owned())
            .collect::<Vec<_>>()
    );
}
#[test]
fn deferred_codes_are_metadata_only_and_never_evaluated() {
    let catalog = rule_catalog().unwrap();
    assert_eq!(
        catalog
            .deferred_rules
            .iter()
            .map(|m| m.code)
            .collect::<Vec<_>>(),
        vec!["TG001", "TG004", "TG005", "TG006", "TG007", "TG008"]
    );
    assert!(
        catalog
            .deferred_rules
            .iter()
            .all(|m| !m.active && !m.required_context.is_empty())
    );
    let json = serde_json::to_value(catalog).unwrap();
    assert_eq!(json["active_rules"].as_array().unwrap().len(), 8);
    assert_eq!(json["deferred_rules"].as_array().unwrap().len(), 6);
}
