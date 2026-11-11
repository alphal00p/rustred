//! Small public parser target; does not build the full app libtest suite.
use rustred_app::CandidateDiscoveryStrategy;

#[test]
fn legacy_pre_screen_default_is_false_and_omitted() {
    let text = r#"{"version":2,"sectors":{"kind":"active-first"},"rows":{"kind":"input-order"},"rule_selection":{"kind":"bounded-portfolio","version":1,"alternatives":[{"kind":"input-order"}],"limits":{"max_depth":0,"max_rows":4,"max_exact_trace_rows":4,"max_exact_trace_terms":16},"quality":[{"feature":"rhs-terms","descending":false}],"trigger":{"kind":"always"}}}"#;
    let legacy = CandidateDiscoveryStrategy::from_json(text).unwrap();
    assert_eq!(serde_json::to_string(&legacy).unwrap(), text);
    let mut value: serde_json::Value = serde_json::from_str(text).unwrap();
    value["rule_selection"]["pre_screen"] = false.into();
    let explicit_off = CandidateDiscoveryStrategy::from_json(&value.to_string()).unwrap();
    assert_eq!(serde_json::to_string(&explicit_off).unwrap(), text);
    value["rule_selection"]["pre_screen"] = true.into();
    let on = CandidateDiscoveryStrategy::from_json(&value.to_string()).unwrap();
    assert_eq!(serde_json::to_value(&on).unwrap(), value);
}
