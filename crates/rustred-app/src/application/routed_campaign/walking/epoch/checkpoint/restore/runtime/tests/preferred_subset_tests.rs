use super::*;

#[test]
fn preferred_subset_cp6_same_policy_restores_changed_policy_refuses() {
    let mut fixture = Fixture::new();
    // The fixture already binds an actual immutable owner payload. Only this
    // policy identity changes; no traversal or source-proved rule is invented.
    let selection = |ids: Value| {
        json!({"preferred_owner_programs":[{
        "owner_mask":"1","path":"same.rrbin","bytes":1,
        "residual_policy":"defer-to-baseline","rule_ordinals":ids}]})
        .to_string()
    };
    fixture.request.matching.selection_json = selection(json!([0]));
    fixture.save(3, 0);
    drop(fixture.open().unwrap());
    fixture.request.matching.selection_json = selection(json!([]));
    assert!(fixture.open().is_err());
    fixture.request.matching.selection_json = selection(json!([0]));
    drop(fixture.open().unwrap());
}
