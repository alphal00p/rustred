#[test]
fn sparse_term_insertion_order_cannot_change_the_chart_lift() {
    let limits = OrdinaryChartLiftLimits::default();
    let context = context("ordinary-chart-lift-order", 2);
    let ordering = ordering(&[true, false], limits.involutive);
    let forward = build_mixed_relation(&context, false);
    let reverse = build_mixed_relation(&context, true);
    assert_eq!(forward, reverse);
    assert_eq!(
        lift_relation(&forward, 0, &ordering, &context, limits).unwrap(),
        lift_relation(&reverse, 0, &ordering, &context, limits).unwrap()
    );
}

#[test]
fn multi_guard_ingress_is_canonical_and_input_order_independent() {
    let limits = OrdinaryChartLiftLimits::default();
    let context = context("ordinary-chart-lift-guard-order", 2);
    let ordering = ordering(&[true, false], limits.involutive);
    let forward = build_multi_guard_relation(&context, false);
    let reverse = build_multi_guard_relation(&context, true);
    assert_ne!(
        forward.nonzero_conditions(),
        reverse.nonzero_conditions(),
        "the fixture must exercise distinct incoming guard order"
    );
    let forward = lift_relation(&forward, 0, &ordering, &context, limits).unwrap();
    let reverse = lift_relation(&reverse, 0, &ordering, &context, limits).unwrap();
    assert_eq!(forward, reverse);
    assert_eq!(forward.consequence().required_nonzero_guards().len(), 2);
}

#[test]
fn real_chart_lift_construction_is_attributed_to_its_named_diagnostic_site() {
    let limits = OrdinaryChartLiftLimits::default();
    let context = context("ordinary-chart-lift-diagnostic", 2);
    let ordering = ordering(&[true, false], limits.involutive);
    let relation = build_mixed_relation(&context, false);

    crate::foundry::completion::involutive::diagnostics::begin();
    lift_relation(&relation, 0, &ordering, &context, limits).unwrap();
    let checkpoint = crate::foundry::completion::involutive::diagnostics::take().unwrap();
    assert_eq!(checkpoint.coefficient_payload_attempts, 1);
    assert_eq!(
        checkpoint
            .coefficient_payload_attempts_by_site
            .chart_lift_source,
        1
    );
}
