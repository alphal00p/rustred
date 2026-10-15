use super::*;

#[test]
fn aggregate_overrides_touch_only_the_three_explicit_count_fields() {
    let mut original = ReductionLimits::default();
    original.exact_algebra.max_exponent = 37;
    original.exact_algebra.max_polynomial_terms = 101;
    original.exact_algebra.max_term_operations = 103;
    original.indexed_algebra.exact_algebra.max_exponent = 39;
    original.indexed_algebra.max_specialization_power_operations = 113;
    original.indexed_algebra.max_specialization_integer_bits = 127;
    original.max_cached_integrals = 131;
    original.max_cached_coefficient_terms = 137;
    original.max_cached_coefficient_bytes = 139;
    original.max_factorization_terms = 149;
    for (rules, pending, coalescing) in
        [(0, 0, 0), (7, 11, 13), (usize::MAX, usize::MAX, usize::MAX)]
    {
        let override_limits = ReductionAggregateOverrides {
            max_rule_applications: rules,
            max_pending_frames: pending,
            max_coalescing_additions: coalescing,
        };
        let mut effective = override_limits.apply(original);
        assert_eq!(effective.max_rule_applications, rules);
        assert_eq!(effective.max_pending_frames, pending);
        assert_eq!(effective.max_coalescing_additions, coalescing);
        effective.max_rule_applications = original.max_rule_applications;
        effective.max_pending_frames = original.max_pending_frames;
        effective.max_coalescing_additions = original.max_coalescing_additions;
        assert_eq!(effective, original);
    }
}

#[test]
fn aggregate_override_object_is_strict_and_zero_and_max_are_literal_grants() {
    for n in [0, usize::MAX] {
        let value =
            json!({"max_rule_applications":n,"max_pending_frames":n,"max_coalescing_additions":n});
        let decoded: ReductionAggregateOverrides = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(decoded.max_rule_applications, n);
        assert_eq!(serde_json::to_value(decoded).unwrap(), value);
        let mut missing = value.clone();
        missing
            .as_object_mut()
            .unwrap()
            .remove("max_pending_frames");
        assert!(serde_json::from_value::<ReductionAggregateOverrides>(missing).is_err());
        let mut extra = value.clone();
        extra["max_polynomial_terms"] = json!(1);
        assert!(serde_json::from_value::<ReductionAggregateOverrides>(extra).is_err());
        for invalid in [json!(-1), json!("unlimited")] {
            let mut changed = value.clone();
            changed["max_rule_applications"] = invalid;
            assert!(serde_json::from_value::<ReductionAggregateOverrides>(changed).is_err());
        }
    }
}
