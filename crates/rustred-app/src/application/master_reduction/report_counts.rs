//! Cheap projections of the recorded native inventory, never a new census.
use serde_json::{Value, json};

pub(super) const RULE_SCOPE: &str = "installed executable owner-program rule records, including preferred programs and overlays; excludes uninstalled sectors embedded in source bundles; not a count of unique identities or newly generated rules";

pub(super) fn copy_from_inventory(source: &Value, output: &mut Value) {
    output["published_rule_records"] = json!(source["inventory"]["installed"]["rules"].as_u64());
    output["published_rule_records_scope"] = json!(RULE_SCOPE);
    output["observed_cover_rules"] = json!(source["inventory"]["encountered"]["rules"].as_u64());
}

pub(super) fn update(report: &mut Value) {
    let rules = report["inventory"]["installed"]["rules"].as_u64();
    let observed = report["inventory"]["encountered"]["rules"].as_u64();
    report["published_rule_records"] = json!(rules);
    report["published_rule_records_scope"] = json!(RULE_SCOPE);
    report["observed_cover_rules"] = json!(observed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_records_come_from_installed_inventory_not_encountered_subset() {
        let mut report: Value = serde_json::from_str(
            r#"{
            "status":"published_unrefined","published_rule_records":1,
            "inventory":{"installed":{"rules":9982},"encountered":{"rules":6698}}
        }"#,
        )
        .unwrap();
        let inventory = report["inventory"].clone();
        update(&mut report);
        assert_eq!(report["published_rule_records"], 9982);
        assert_eq!(report["observed_cover_rules"], 6698);
        assert_eq!(report["inventory"], inventory);
        let mut event = json!({"event":"master_reduction_finished"});
        copy_from_inventory(&report, &mut event);
        assert_eq!(event["published_rule_records"], 9982);
        assert_eq!(event["observed_cover_rules"], 6698);
        assert!(event["inventory"].is_null());
        assert_eq!(event["published_rule_records_scope"], RULE_SCOPE);
    }

    #[test]
    fn absent_or_invalid_inventory_count_is_unknown_not_zero() {
        for rules in [Value::Null, json!(-1), json!(true), json!("9982")] {
            let mut report = json!({"inventory":{"installed":{"rules":rules}}});
            update(&mut report);
            assert!(report["published_rule_records"].is_null());
            assert!(report["observed_cover_rules"].is_null());
        }
        let mut empty = json!({"inventory":{"installed":{"rules":0},"encountered":{"rules":0}}});
        update(&mut empty);
        assert_eq!(empty["published_rule_records"], 0);
        assert_eq!(empty["observed_cover_rules"], 0);
    }
}
