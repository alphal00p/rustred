use super::*;
use crate::reduction::ReductionLimits;

#[test]
fn weighted_provider_combination_keeps_precancellation_poles_and_shared_work() {
    let f = family("weighted", 1);
    let c = f.coefficient_context();
    let mut remaining = Vec::new();
    let result = reduce_vacuum_factor_combination(
        f.clone(),
        [
            (key([3]), c.coefficient_fixture("1/(d-4)")),
            (key([4]), c.coefficient_fixture("-1/(d-4)")),
        ],
        [],
        ReductionLimits {
            diagnostics_enabled: true,
            max_rule_applications: 2,
            ..Default::default()
        },
        |_, limits| -> Result<_, String> {
            remaining.push(limits.max_rule_applications);
            Ok(VacuumFactorReduction {
                terms: vec![term(&f, [1], 1)],
                nonzero_conditions: vec![c.coefficient_fixture("d-7")],
                work: VacuumFactorWork {
                    rule_applications: 1,
                    ..Default::default()
                },
            })
        },
    )
    .unwrap();
    assert_eq!(remaining, [2, 1]);
    assert!(result.terms.is_empty());
    assert_eq!(result.work.rule_applications, 2);
    assert_eq!(result.work.coalescing_additions, 1);
    assert!(
        result
            .nonzero_conditions
            .contains(&c.coefficient_fixture("d-4"))
    );
    assert!(
        result
            .nonzero_conditions
            .contains(&c.coefficient_fixture("d-7"))
    );
    let zero = reduce_vacuum_factor_combination(
        f.clone(),
        [
            (key([3]), c.coefficient_fixture("1/(d-4)")),
            (key([3]), c.coefficient_fixture("-1/(d-4)")),
        ],
        [],
        Default::default(),
        |_, _| -> Result<VacuumFactorReduction, String> {
            panic!("canceled weights do not call an evaluator")
        },
    )
    .unwrap();
    assert!(zero.terms.is_empty());
    assert!(
        zero.nonzero_conditions
            .contains(&c.coefficient_fixture("d-4"))
    );
}

#[test]
fn weighted_provider_combination_rejects_foreign_zero_rows_and_exhausted_work() {
    let f = family("weighted", 1);
    let c = f.coefficient_context();
    let foreign = family("foreign", 2);
    assert!(
        reduce_vacuum_factor_combination(
            f.clone(),
            [(key([3]), c.one())],
            [],
            Default::default(),
            |_, _| -> Result<_, String> {
                Ok(VacuumFactorReduction {
                    terms: vec![term(&foreign, [1, 1, 1], 0)],
                    ..Default::default()
                })
            }
        )
        .is_err()
    );
    let mut remaining = Vec::new();
    let error = reduce_vacuum_factor_combination(
        f.clone(),
        [(key([3]), c.one()), (key([4]), c.one())],
        [],
        ReductionLimits {
            diagnostics_enabled: true,
            max_rule_applications: 1,
            ..Default::default()
        },
        |_, limits| -> Result<_, String> {
            remaining.push(limits.max_rule_applications);
            if limits.max_rule_applications == 0 {
                return Err("exhausted".into());
            }
            Ok(VacuumFactorReduction {
                terms: vec![term(&f, [1], 1)],
                work: VacuumFactorWork {
                    rule_applications: 1,
                    ..Default::default()
                },
                ..Default::default()
            })
        },
    )
    .unwrap_err();
    assert_eq!(remaining, [1, 0]);
    assert!(matches!(
        error,
        VacuumProductReductionError::Provider { .. }
    ));
}

#[test]
fn weighted_provider_combination_charges_retained_coefficients_before_callback() {
    let f = family("weighted", 1);
    let c = f.coefficient_context();
    assert!(
        reduce_vacuum_factor_combination(
            f.clone(),
            [(key([1]), c.one())],
            [],
            ReductionLimits {
                diagnostics_enabled: true,
                max_cached_coefficient_terms: 0,
                ..Default::default()
            },
            |_, _| -> Result<VacuumFactorReduction, String> {
                panic!("retained input exceeds budget")
            }
        )
        .is_err()
    );
}

#[test]
fn weighted_provider_guard_duplicates_are_admitted_before_retention() {
    use crate::reduction::coefficient_cache_weight;
    let f = family("guard-budget", 1);
    let c = f.coefficient_context();
    let one = c.one();
    let guard = c.coefficient_fixture("d-7");
    let family_conditions = f
        .domain()
        .conditions()
        .map(|value| value.polynomial().clone().into())
        .collect::<Vec<crate::algebra::Coefficient>>();
    let initial = coefficient_cache_weight(
        std::iter::once(&one)
            .chain(&family_conditions)
            .chain(&family_conditions),
    )
    .unwrap();
    let returned = coefficient_cache_weight([&one, &guard]).unwrap();
    let mut called = false;
    let result = reduce_vacuum_factor_combination(
        f.clone(),
        [(key([3]), one)],
        [],
        ReductionLimits {
            diagnostics_enabled: true,
            max_cached_coefficient_terms: initial.coefficient_terms + returned.coefficient_terms,
            ..Default::default()
        },
        |_, _| -> Result<_, String> {
            called = true;
            Ok(VacuumFactorReduction {
                terms: vec![term(&f, [1], 1)],
                nonzero_conditions: vec![guard.clone()],
                ..Default::default()
            })
        },
    );
    assert!(called, "raw callback result fits the remaining grant");
    assert!(
        result.is_err(),
        "both retained guard copies exceed the combined grant"
    );
}
