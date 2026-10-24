use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;
use std::collections::BTreeMap;
use symbolica::atom::{
    Atom, AtomCore, FunctionBuilder, NamespacedSymbol, Symbol, SymbolAttribute, SymbolBuilder,
};

fn labels(loops: usize) -> (Symbol, Vec<Atom>) {
    let head = SymbolBuilder::new(
        NamespacedSymbol::try_parse("rustred_product_chart_tests::dot").unwrap(),
    )
    .with_attributes(vec![SymbolAttribute::Symmetric])
    .build()
    .unwrap();
    let labels = (0..loops)
        .map(|i| {
            SymbolBuilder::new(
                NamespacedSymbol::try_parse(format!("rustred_product_chart_tests::q{i}")).unwrap(),
            )
            .build()
            .unwrap()
            .to_atom()
        })
        .collect();
    (head, labels)
}
fn dot(head: Symbol, a: &Atom, b: &Atom) -> Atom {
    FunctionBuilder::new(head).add_arg(a).add_arg(b).finish()
}
fn cross_family(changed: bool) -> IntegralFamily {
    let c = CoefficientContext::new(["d"]);
    // The third complete coordinate is a pure cross scalar numerator. The
    // optional parent basis change a=k0+k1, b=k1 has determinant one.
    let rows = if changed {
        [[1, 2, 1], [0, 0, 1], [0, 1, 1]]
    } else {
        [[1, 0, 0], [0, 0, 1], [0, 1, 0]]
    };
    IntegralFamily::new(
        "cross",
        vec!["a".into(), "b".into()],
        vec![],
        c.clone(),
        c.parameter("d").unwrap(),
        rows.into_iter()
            .enumerate()
            .map(|(i, row)| {
                AffineDenominator::new(
                    c.integer(if i == 2 { 0 } else { -1 }),
                    row.into_iter().map(|n| c.integer(n)).collect(),
                )
            })
            .collect(),
        vec![],
        vec![c.zero(); 3],
    )
    .unwrap()
}
fn rows(result: &VacuumProductProjection) -> BTreeMap<Vec<Vec<i64>>, Coefficient> {
    result
        .terms
        .iter()
        .map(|(factors, c)| {
            (
                factors
                    .iter()
                    .map(|f| f.integral.powers().to_vec())
                    .collect(),
                c.clone(),
            )
        })
        .collect()
}

#[test]
fn chart_scalar_resource_refusal_keeps_its_cause_and_allows_a_healthy_request() {
    use std::time::Duration;
    use symbolica::utils::resources::{ResourceBudget, ResourceLimitError, ResourceLimits};

    let source = family(4);
    let target = key([2, 3, 4, 5, 1, -1, 0, 1, 0, 0]);
    let plan = VacuumProductChartPlan::prepare(
        &source,
        &BTreeSet::from([target.clone()]),
        Default::default(),
    )
    .unwrap();
    let chart = &plan.charts()[&target];
    let (head, labels) = labels(4);
    let project = || {
        chart.project(
            head,
            labels.clone(),
            Default::default(),
            |_| -> Result<VacuumTensorProjection, String> {
                panic!("component-local numerator must not call the tensor projector")
            },
        )
    };
    let expected = project().unwrap();
    let error = ResourceBudget::new(ResourceLimits {
        max_elapsed: Some(Duration::ZERO),
        ..Default::default()
    })
    .run(project)
    .unwrap_err();
    assert!(matches!(
        crate::reduction::resource_limit_error(&error),
        Some(ResourceLimitError::Elapsed { .. })
    ));
    let actual = project().unwrap();
    assert_eq!(rows(&actual), rows(&expected));
    assert_eq!(actual.nonzero_conditions, expected.nonzero_conditions);
}

#[test]
fn local_numerator_chart_preserves_every_transport_endpoint_without_tensor_callback() {
    let source = family(4);
    let target = key([2, 3, 4, 5, 1, -1, 0, 1, 0, 0]);
    let plan = VacuumProductChartPlan::prepare(
        &source,
        &BTreeSet::from([target.clone()]),
        Default::default(),
    )
    .unwrap();
    let chart = &plan.charts()[&target];
    assert_eq!(
        chart
            .components()
            .iter()
            .map(|c| c.family().loop_count())
            .collect::<Vec<_>>(),
        [3, 1]
    );
    let expected = chart
        .transport()
        .transport(&target, Default::default())
        .unwrap();
    let (head, labels) = labels(4);
    let actual = chart
        .project(
            head,
            labels,
            Default::default(),
            |_| -> Result<VacuumTensorProjection, String> {
                panic!("component-local numerator must not call the tensor projector")
            },
        )
        .unwrap();
    let reconstructed = actual
        .terms
        .iter()
        .map(|(factors, c)| {
            let mut powers = factors
                .iter()
                .flat_map(|f| f.integral.powers().iter().copied())
                .collect::<Vec<_>>();
            powers.resize(chart.combined_family().denominator_count(), 0);
            (key(powers), c.clone())
        })
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        reconstructed,
        expected
            .terms()
            .iter()
            .map(|term| (term.key().clone(), term.coefficient().clone()))
            .collect()
    );
    assert!(actual.terms.iter().any(|(factors, _)| {
        factors
            .iter()
            .any(|f| f.integral.powers().iter().any(|&p| p < 0))
    }));
}

#[test]
fn cross_odd_projection_zero_retains_source_and_projector_conditions() {
    let source = cross_family(false);
    let target = key([1, 1, -1]);
    let plan = VacuumProductChartPlan::prepare(
        &source,
        &BTreeSet::from([target.clone()]),
        Default::default(),
    )
    .unwrap();
    let chart = &plan.charts()[&target];
    let (head, labels) = labels(2);
    let guard = source.coefficient_context().coefficient_fixture("d-2");
    let actual = chart
        .project(
            head,
            labels,
            Default::default(),
            |input| -> Result<_, String> {
                assert_eq!(input.dimension, source.dimension().to_expression());
                assert_eq!(
                    input
                        .component_loop_momenta
                        .iter()
                        .map(Vec::len)
                        .collect::<Vec<_>>(),
                    [1, 1]
                );
                Ok(VacuumTensorProjection {
                    numerator: Atom::num(0),
                    nonzero_conditions: vec![guard.to_expression()],
                })
            },
        )
        .unwrap();
    assert!(actual.terms.is_empty());
    assert!(actual.nonzero_conditions.contains(&guard));
    for condition in chart.transport().verified_map().nonzero_conditions() {
        assert!(
            actual
                .nonzero_conditions
                .contains(&condition.polynomial().clone().into())
        );
    }
}

#[test]
fn even_cross_projection_is_exact_and_unimodular_parent_basis_independent() {
    let mut previous = None;
    for changed in [false, true] {
        let source = cross_family(changed);
        let target = key([2, 3, -2]);
        let plan = VacuumProductChartPlan::prepare(
            &source,
            &BTreeSet::from([target.clone()]),
            Default::default(),
        )
        .unwrap();
        let chart = &plan.charts()[&target];
        let (head, labels) = labels(2);
        let actual = chart
            .project(
                head,
                labels,
                Default::default(),
                |input| -> Result<_, String> {
                    let a = &input.component_loop_momenta[0][0];
                    let b = &input.component_loop_momenta[1][0];
                    assert_eq!(input.numerator, dot(input.dot_head, a, b).pow(2));
                    Ok(VacuumTensorProjection {
                        numerator: dot(input.dot_head, a, a) * dot(input.dot_head, b, b)
                            / &input.dimension,
                        nonzero_conditions: vec![input.dimension.clone()],
                    })
                },
            )
            .unwrap();
        let c = source.coefficient_context();
        let expected = BTreeMap::from([
            (vec![vec![1], vec![2]], c.coefficient_fixture("1/d")),
            (vec![vec![1], vec![3]], c.coefficient_fixture("1/d")),
            (vec![vec![2], vec![2]], c.coefficient_fixture("1/d")),
            (vec![vec![2], vec![3]], c.coefficient_fixture("1/d")),
        ]);
        assert_eq!(rows(&actual), expected);
        assert!(
            actual
                .nonzero_conditions
                .contains(&c.parameter("d").unwrap())
        );
        if let Some(previous) = previous {
            assert_eq!(rows(&actual), previous);
        }
        previous = Some(rows(&actual));
    }
}

#[test]
fn chart_projection_fails_closed_on_residual_cross_foreign_guards_and_budgets() {
    let source = cross_family(false);
    let target = key([1, 1, -2]);
    let targets = BTreeSet::from([target.clone()]);
    let mut limits = VacuumProductChartLimits::default();
    limits.products.max_canonical_candidates = 0;
    assert!(matches!(
        VacuumProductChartPlan::prepare(&source, &targets, limits),
        Err(VacuumProductError::ResourceLimit { .. })
    ));
    let plan = VacuumProductChartPlan::prepare(&source, &targets, Default::default()).unwrap();
    let chart = &plan.charts()[&target];
    let (head, labels) = labels(2);
    assert!(
        chart
            .project(
                head,
                labels.clone(),
                Default::default(),
                |input| -> Result<_, String> {
                    Ok(VacuumTensorProjection {
                        numerator: input.numerator.clone(),
                        nonzero_conditions: vec![],
                    })
                }
            )
            .is_err()
    );
    let foreign = SymbolBuilder::new(
        NamespacedSymbol::try_parse("rustred_product_chart_tests::epsilon").unwrap(),
    )
    .build()
    .unwrap()
    .to_atom();
    assert!(
        chart
            .project(
                head,
                labels.clone(),
                Default::default(),
                |_| -> Result<_, String> {
                    Ok(VacuumTensorProjection {
                        numerator: Atom::num(0),
                        nonzero_conditions: vec![foreign.clone()],
                    })
                }
            )
            .is_err()
    );
    let mut scalar = crate::scalar_numerator::ScalarNumeratorLimits::default();
    scalar.max_polynomial_terms = 0;
    assert!(
        chart
            .project(
                head,
                labels,
                scalar,
                |_| -> Result<VacuumTensorProjection, String> {
                    panic!("budget precedes callback")
                }
            )
            .is_err()
    );
    let connected = family(2);
    assert!(
        VacuumProductChartPlan::prepare(
            &connected,
            &BTreeSet::from([key([1, 1, 1])]),
            Default::default()
        )
        .unwrap()
        .charts()
        .is_empty()
    );
}

#[test]
fn projection_numerator_and_guards_share_one_exact_collection_budget() {
    let source = cross_family(false);
    let target = key([1, 1, -1]);
    let plan = VacuumProductChartPlan::prepare(
        &source,
        &BTreeSet::from([target.clone()]),
        Default::default(),
    )
    .unwrap();
    let (head, labels) = labels(2);
    let mut limits = crate::scalar_numerator::ScalarNumeratorLimits::default();
    limits.exact_algebra.max_term_operations = 3;
    let actual =
        plan.charts()[&target].project(head, labels, limits, |input| -> Result<_, String> {
            Ok(VacuumTensorProjection {
                numerator: Atom::num(0),
                nonzero_conditions: vec![input.dimension.clone(), Atom::num(2)],
            })
        });
    assert!(
        actual.is_err(),
        "two individually admissible guards cannot reset the request budget"
    );
}
