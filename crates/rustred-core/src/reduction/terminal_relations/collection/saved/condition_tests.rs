//! Stable indexed guard retention must match the legacy collection policy.
use super::*;

fn outcome(result: &Result<()>) -> Option<String> {
    result.as_ref().err().map(ToString::to_string)
}

#[test]
fn indexed_conditions_preserve_first_seen_order_and_exact_coefficients() {
    let context = CoefficientContext::new(["d", "s"]);
    let input = [
        "d-4",
        "s-2",
        "d-4",
        "4-d",
        "1/(d-4)",
        "(d-4)*(s-2)",
        "1",
        "-3",
        "s-2",
        "1/(d-4)",
    ]
    .map(|value| context.coefficient_fixture(value));
    let mut expected = Vec::new();
    let mut actual = Vec::new();
    {
        let mut union = CollectionConditions::new(&mut actual, usize::MAX).unwrap();
        for value in &input {
            retain(&mut expected, value.clone(), usize::MAX).unwrap();
            union.retain(value).unwrap();
        }
    }
    assert_eq!(actual, expected);
    assert_eq!(actual.len(), 5);
    assert_eq!(actual[2], context.coefficient_fixture("4-d"));
    assert_eq!(actual[3], context.coefficient_fixture("1/(d-4)"));
}

#[test]
fn indexed_conditions_match_zero_and_budget_errors_without_partial_insertion() {
    let context = CoefficientContext::new(["d"]);
    let input = ["d-4", "d-4", "1", "d-3", "0", "d-4", "d-2"]
        .map(|value| context.coefficient_fixture(value));
    for limit in [0, 1, 2, 3, usize::MAX] {
        let mut expected = Vec::new();
        let mut actual = Vec::new();
        let mut outcomes = Vec::new();
        {
            let mut union = CollectionConditions::new(&mut actual, limit).unwrap();
            for value in &input {
                outcomes.push(outcome(&union.retain(value)));
            }
        }
        for (value, actual_error) in input.iter().zip(outcomes) {
            let expected_result = retain(&mut expected, value.clone(), limit);
            assert_eq!(actual_error, outcome(&expected_result), "limit={limit}");
        }
        assert_eq!(actual, expected, "limit={limit}");
    }
}

#[test]
fn indexed_conditions_preserve_existing_prefix_and_duplicate_at_full_budget() {
    let context = CoefficientContext::new(["d"]);
    let a = context.coefficient_fixture("d-4");
    let b = context.coefficient_fixture("d-3");
    let c = context.coefficient_fixture("d-2");
    // The index must not canonicalize an existing prefix or apply its limit
    // retroactively. The legacy policy checks only a genuinely new guard.
    let mut actual = vec![b.clone(), a.clone(), b.clone()];
    let mut expected = actual.clone();
    {
        let mut union = CollectionConditions::new(&mut actual, 2).unwrap();
        for value in [&a, &b, &context.one(), &c, &context.zero()] {
            assert_eq!(
                outcome(&union.retain(value)),
                outcome(&retain(&mut expected, value.clone(), 2))
            );
        }
    }
    assert_eq!(actual, expected);
    assert_eq!(actual, [b.clone(), a, b]);
}

#[test]
fn indexed_conditions_handle_zero_variable_context_and_empty_output() {
    let context = CoefficientContext::new(std::iter::empty::<&str>());
    let mut conditions = Vec::new();
    {
        let mut union = CollectionConditions::new(&mut conditions, 0).unwrap();
        for value in [context.one(), context.integer(-7)] {
            union.retain(&value).unwrap();
        }
        assert!(matches!(
            union.retain(&context.zero()),
            Err(VacuumCollectionError::VanishingCondition)
        ));
    }
    assert!(conditions.is_empty());
}

#[test]
fn composition_index_keeps_session_conditions_and_cold_payload_identical() {
    let context = CoefficientContext::new(["d"]);
    let family = Arc::new(
        IntegralFamily::new(
            "indexed-collection-conditions",
            vec!["k".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            vec![crate::family::AffineDenominator::new(
                context.integer(-1),
                vec![context.one()],
            )],
            vec![],
            vec![context.zero()],
        )
        .unwrap(),
    );
    let keys = [1, 2]
        .into_iter()
        .map(|n| IntegralKey::try_new([n]).unwrap())
        .collect();
    let mut source =
        TerminalRelationSession::new(family.clone(), keys, 0, Default::default()).unwrap();
    source.conditions = ["d-7", "d-3", "d-7", "1", "3-d"]
        .map(|value| context.coefficient_fixture(value))
        .into();
    let mut expected = Vec::new();
    for value in &source.conditions {
        retain(&mut expected, value.clone(), usize::MAX).unwrap();
    }
    let plan = TerminalCollectionPlan::from_sessions(&[&source], Default::default()).unwrap();
    for key in source.raw_terminals() {
        assert_eq!(
            plan.apply(&family, key).unwrap().nonzero_conditions(),
            expected
        );
    }
    let first = plan
        .apply(&family, &IntegralKey::try_new([1]).unwrap())
        .unwrap()
        .shared_nonzero_conditions();
    let second = plan
        .apply(&family, &IntegralKey::try_new([2]).unwrap())
        .unwrap()
        .shared_nonzero_conditions();
    assert!(Arc::ptr_eq(&first, &second));
    let bytes = plan.to_native_bytes(Default::default()).unwrap();
    let cold = TerminalCollectionPlan::from_native_bytes(
        &bytes,
        &[&source],
        Default::default(),
        Default::default(),
    )
    .unwrap();
    assert_eq!(cold.to_native_bytes(Default::default()).unwrap(), bytes);
    for key in source.raw_terminals() {
        assert_eq!(
            cold.apply(&family, key).unwrap().nonzero_conditions(),
            expected
        );
    }
}

#[test]
fn row_specific_conditions_copy_only_the_changed_row_and_never_leak() {
    let context = CoefficientContext::new(["d"]);
    let a = context.coefficient_fixture("d-4");
    let b = context.coefficient_fixture("d-3");
    let c = context.coefficient_fixture("d-2");
    let base = Arc::new(vec![a.clone()]);
    let unchanged = with_additional_conditions(&base, Vec::new());
    let first = with_additional_conditions(&base, vec![b.clone()]);
    let second = with_additional_conditions(&base, vec![c.clone()]);
    assert!(Arc::ptr_eq(&unchanged, &base));
    assert!(!Arc::ptr_eq(&first, &base));
    assert!(!Arc::ptr_eq(&second, &base));
    assert_eq!(base.as_ref(), &[a.clone()]);
    assert_eq!(first.as_ref(), &[a.clone(), b]);
    assert_eq!(second.as_ref(), &[a, c]);
}
