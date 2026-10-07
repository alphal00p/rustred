use super::*;
use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;

fn family(mass: i64, shift: bool, nonunit_circuit: bool) -> Arc<IntegralFamily> {
    let context = CoefficientContext::new(["d"]);
    let momenta = [
        [1, 0, 0],
        [0, 1, 0],
        [0, 0, 1],
        [if nonunit_circuit { 2 } else { 1 }, 1, 1],
        [1, 0, 1],
        [0, 1, 1],
    ];
    let denominators = momenta
        .iter()
        .map(|q| {
            AffineDenominator::new(
                context.integer(-mass),
                (0..3)
                    .flat_map(|i| (i..3).map(move |j| q[i] * q[j] * if i == j { 1 } else { 2 }))
                    .map(|n| context.integer(n))
                    .collect(),
            )
        })
        .collect();
    Arc::new(
        IntegralFamily::new(
            "finite_circuit_equations",
            vec!["k1".into(), "k2".into(), "k3".into()],
            vec![],
            context.clone(),
            context.parameter("d").unwrap(),
            denominators,
            vec![],
            vec![context.integer(i64::from(shift)); 6],
        )
        .unwrap(),
    )
}

fn key(powers: [i64; 6]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}

#[test]
fn rank_two_retains_exact_weighted_offspring_and_cancels_its_self_term() {
    let family = family(1, false, false);
    let target = key([1, 1, 1, 1, -2, 0]);
    let plan = TerminalCircuitEquations::prepare(
        Arc::clone(&family),
        &BTreeSet::from([target.clone()]),
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.family_fingerprint(), family.fingerprint());
    assert_eq!(plan.statistics().analyzed_supports, 1);
    assert_eq!(plan.statistics().verified_generators, 3);
    let context = family.coefficient_context();
    // Independent oracle: k2<->k3 sends D5 to
    // 1+D1+D2+D3+D4-D5-D6. Its square has 28 monomials;
    // the D5² source cancels in I-T(I), leaving exactly 27.
    let row = plan
        .equations(&target)
        .iter()
        .find(|row| row.terms.len() == 27)
        .expect("nontrivial affine-square generator");
    assert!(!row.terms.contains_key(&target));
    assert!(row.nonzero_conditions.is_empty());
    for (powers, coefficient) in [
        ([1, 1, 1, 1, 0, 0], -1),
        ([0, 1, 1, 1, 0, 0], -2),
        ([1, 1, 1, 1, -1, -1], -2),
        ([0, 1, 1, 1, -1, 0], 2),
        ([-1, 1, 1, 1, 0, 0], -1),
    ] {
        assert_eq!(row.terms[&key(powers)], context.integer(coefficient));
    }
    // Offspring are kept as unresolved columns, never recursively prepared.
    assert!(
        row.terms
            .keys()
            .all(|child| plan.equations(child).is_empty())
    );
}

#[test]
fn unequal_positive_dots_are_permuted_and_multiple_numerators_are_expanded() {
    let family = family(1, false, false);
    let dotted = key([1, 2, 3, 1, -2, 0]);
    let mixed = key([1, 1, 1, 1, -1, -1]);
    let plan = TerminalCircuitEquations::prepare(
        Arc::clone(&family),
        &BTreeSet::from([dotted.clone(), mixed.clone()]),
        Default::default(),
    )
    .unwrap();
    assert_eq!(plan.statistics().analyzed_supports, 1);
    assert_eq!(plan.statistics().verified_generators, 3);
    let context = family.coefficient_context();
    let row = plan
        .equations(&dotted)
        .iter()
        .find(|row| row.terms.contains_key(&key([1, 3, 2, 1, 0, 0])))
        .expect("k2/k3 dots travel with the exact denominator permutation");
    assert_eq!(row.terms[&dotted], context.one());
    assert_eq!(row.terms[&key([1, 3, 2, 1, -2, 0])], context.integer(-1));
    assert_eq!(row.terms[&key([1, 3, 2, 1, 0, 0])], context.integer(-1));
    assert_eq!(row.terms.len(), 29);
    assert!(
        plan.equations(&mixed).iter().any(|row| {
            row.terms.get(&mixed) == Some(&context.integer(2)) && row.terms.len() > 2
        })
    );
    assert!(plan.statistics().trivial_equations > 0);
}

#[test]
fn unsupported_geometry_is_counted_without_admitting_equations() {
    let target = key([1, 1, 1, 1, -2, 0]);
    for (family, reason) in [
        (
            family(2, false, false),
            TerminalNormalizationSkipReason::UnsupportedGeometry(ProductSkipReason::NonUnitMass),
        ),
        (
            family(1, true, false),
            TerminalNormalizationSkipReason::UnsupportedGeometry(
                ProductSkipReason::AnalyticPowerShifts,
            ),
        ),
        (
            family(1, false, true),
            TerminalNormalizationSkipReason::NonUnitCircuit,
        ),
    ] {
        let plan = TerminalCircuitEquations::prepare(
            family,
            &BTreeSet::from([target.clone()]),
            Default::default(),
        )
        .unwrap();
        assert!(plan.equations(&target).is_empty());
        assert_eq!(plan.statistics().equations, 0);
        assert_eq!(plan.statistics().skipped[&reason], 1);
    }
    let wrong_support = key([1, 1, 1, 0, -1, 0]);
    let plan = TerminalCircuitEquations::prepare(
        family(1, false, false),
        &BTreeSet::from([wrong_support.clone()]),
        Default::default(),
    )
    .unwrap();
    assert!(plan.equations(&wrong_support).is_empty());
    assert_eq!(plan.statistics().analyzed_supports, 0);
}

#[test]
fn malformed_arity_and_preparation_caps_fail_closed() {
    let family = family(1, false, false);
    let target = key([1, 1, 1, 1, -2, 0]);
    let keys = BTreeSet::from([target]);
    assert!(matches!(
        TerminalCircuitEquations::prepare(
            Arc::clone(&family),
            &BTreeSet::from([IntegralKey::try_new([1]).unwrap()]),
            Default::default(),
        ),
        Err(TerminalNormalizationError::Alias(
            TerminalAliasError::WrongArity { .. }
        ))
    ));
    for normalization in [
        TerminalNormalizationLimits {
            max_terminals: 0,
            ..Default::default()
        },
        TerminalNormalizationLimits {
            max_supports: 0,
            ..Default::default()
        },
        TerminalNormalizationLimits {
            max_matrix_cells: 0,
            ..Default::default()
        },
    ] {
        assert!(matches!(
            TerminalCircuitEquations::prepare(
                Arc::clone(&family),
                &keys,
                TerminalCircuitLimits {
                    normalization,
                    ..Default::default()
                },
            ),
            Err(TerminalNormalizationError::Limit { .. })
        ));
    }
}

#[test]
fn individual_and_aggregate_native_expansion_caps_remain_typed() {
    let family = family(1, false, false);
    let target = key([1, 1, 1, 1, -2, 0]);
    for limits in [
        TerminalCircuitLimits {
            expansion: integral_transport::ExpansionLimits {
                max_total_power: 1,
                ..Default::default()
            },
            ..Default::default()
        },
        TerminalCircuitLimits {
            max_transport_endpoints: 1,
            ..Default::default()
        },
        TerminalCircuitLimits {
            max_transport_operations: 0,
            ..Default::default()
        },
        TerminalCircuitLimits {
            normalization: TerminalNormalizationLimits {
                max_output_terms: 0,
                ..Default::default()
            },
            ..Default::default()
        },
    ] {
        assert!(matches!(
            TerminalCircuitEquations::prepare(
                Arc::clone(&family),
                &BTreeSet::from([target.clone()]),
                limits
            ),
            Err(TerminalNormalizationError::Transport(
                integral_transport::Error::Expansion(ExpansionError::ResourceLimit { .. })
            ))
        ));
    }
}

#[test]
fn expansion_budget_is_shared_across_keys_not_reset_per_map_or_target() {
    let family = family(1, false, false);
    let targets = [key([1, 1, 1, 1, -1, 0]), key([1, 1, 1, 1, -2, 0])];
    let individual_bound = targets
        .iter()
        .map(|target| {
            TerminalCircuitEquations::prepare(
                Arc::clone(&family),
                &BTreeSet::from([target.clone()]),
                Default::default(),
            )
            .unwrap()
            .statistics()
            .transport_endpoints
        })
        .max()
        .unwrap();
    let limits = TerminalCircuitLimits {
        max_transport_endpoints: individual_bound,
        ..Default::default()
    };
    for target in &targets {
        TerminalCircuitEquations::prepare(
            Arc::clone(&family),
            &BTreeSet::from([target.clone()]),
            limits,
        )
        .expect("each whole target fits alone");
    }
    assert!(matches!(
        TerminalCircuitEquations::prepare(family, &BTreeSet::from(targets), limits),
        Err(TerminalNormalizationError::Transport(
            integral_transport::Error::Expansion(ExpansionError::ResourceLimit {
                resource: "aggregate circuit transport endpoints",
                ..
            })
        ))
    ));
}
