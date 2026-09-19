use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::CoefficientContext;
use crate::family::symanzik::SymanzikPolynomials;
use crate::family::{AffineDenominator, IntegralFamily, IntegralKey};
use crate::sector::OrderingPolicy;

use super::super::{ProductSkipReason as Skip, TerminalAliasError as Error, TerminalAliasPlan};
use super::{VacuumParametricLimits, geometry, verify};

fn family(rows: [[i64; 3]; 3], constants: [i64; 3]) -> IntegralFamily {
    let context = CoefficientContext::new(["d"]);
    IntegralFamily::new(
        "parametric_test",
        vec!["k1".into(), "k2".into()],
        vec![],
        context.clone(),
        context.parameter("d").unwrap(),
        rows.into_iter()
            .zip(constants)
            .map(|(row, constant)| {
                AffineDenominator::new(
                    context.integer(constant),
                    row.into_iter().map(|n| context.integer(n)).collect(),
                )
            })
            .collect(),
        vec![],
        vec![context.zero(); 3],
    )
    .unwrap()
}

fn standard() -> IntegralFamily {
    family([[1, 0, 0], [0, 0, 1], [1, 2, 1]], [-1; 3])
}
fn key(powers: [i64; 3]) -> IntegralKey {
    IntegralKey::try_new(powers).unwrap()
}
fn prepare(
    family: &IntegralFamily,
    raw: &BTreeSet<IntegralKey>,
    limits: VacuumParametricLimits,
) -> TerminalAliasPlan {
    TerminalAliasPlan::vacuum_parametric_equivalences(
        family,
        raw,
        OrderingPolicy::SpiredUncutV1,
        limits,
    )
    .unwrap()
}

#[test]
fn positive_products_and_dotted_sunsets_use_distinct_parametric_proofs() {
    let family = standard();
    let raw = BTreeSet::from([
        key([1, 1, 0]),
        key([1, 0, 1]),
        key([0, 1, 1]),
        key([2, 1, 1]),
        key([1, 2, 1]),
        key([1, 1, 2]),
    ]);
    let plan = prepare(&family, &raw, Default::default());
    assert_eq!(plan.statistics().eligible_parametric, 6);
    assert_eq!(plan.statistics().analyzed_parametric_supports, 4);
    assert_eq!(plan.statistics().parametric_canonicalizations, 6);
    assert_eq!(plan.aliases().len(), 4);
    for (source, alias) in plan.aliases() {
        assert!(alias.witness().as_momentum().is_none());
        let witness = alias.witness().as_parametric().unwrap();
        assert_eq!(
            witness.source_u_term_count(),
            witness.representative_u_term_count()
        );
        assert!(!plan.aliases().contains_key(alias.representative()));
        assert!(raw.contains(alias.representative()));
        assert!(
            plan.ordering()
                .compare(alias.representative().powers(), source.powers())
                .unwrap()
                .is_lt()
        );
    }
    for _ in 0..3 {
        let next = prepare(&family, &raw, Default::default());
        assert_eq!(next.statistics(), plan.statistics());
        assert_eq!(next.canonical_terminals(), plan.canonical_terminals());
        for (source, alias) in plan.aliases() {
            let a = alias.witness().as_parametric().unwrap();
            let b = next.aliases()[source].witness().as_parametric().unwrap();
            assert_eq!(
                alias.representative(),
                next.aliases()[source].representative()
            );
            assert_eq!(a.parameter_permutation(), b.parameter_permutation());
        }
    }
}

#[test]
fn inactive_nonsquare_nonunit_mass_coordinate_does_not_limit_the_proof() {
    let family = family([[1, 0, 0], [0, 0, 1], [0, 1, 0]], [-1, -1, 7]);
    let raw = BTreeSet::from([key([1, 2, 0]), key([2, 1, 0]), key([1, 0, 1])]);
    let plan = prepare(&family, &raw, Default::default());
    assert_eq!(plan.aliases().len(), 1);
    assert_eq!(plan.statistics().skipped[&Skip::NonUnitMass], 1);
    assert!(plan.canonical_terminals().contains(&key([1, 0, 1])));
}

#[test]
fn active_non_square_forms_cannot_gain_authority_from_equal_determinants() {
    let family = family([[1, 0, 0], [0, 0, 1], [0, 1, 0]], [-1; 3]);
    let raw = BTreeSet::from([key([1, 0, 1]), key([0, 1, 1])]);
    let plan = prepare(&family, &raw, Default::default());
    assert_eq!(plan.canonical_terminals(), &raw);
    assert_eq!(plan.statistics().skipped[&Skip::NotLinearSquare], 2);
}

#[test]
fn explicit_preparation_caps_retain_raw_keys_without_false_aliases() {
    let family = standard();
    let raw = BTreeSet::from([key([1, 1, 0]), key([1, 0, 1]), key([0, 1, 1])]);
    let mut policies = vec![
        VacuumParametricLimits {
            max_supports: 0,
            ..Default::default()
        },
        VacuumParametricLimits {
            max_canonicalizations: 0,
            ..Default::default()
        },
        VacuumParametricLimits {
            max_graph_vertices: 0,
            ..Default::default()
        },
        VacuumParametricLimits {
            max_graph_edges: 0,
            ..Default::default()
        },
    ];
    let mut polynomial_budget = VacuumParametricLimits::default();
    polynomial_budget.symanzik.max_polynomial_terms = 0;
    policies.push(polynomial_budget);
    for limits in policies {
        let plan = prepare(&family, &raw, limits);
        assert_eq!(plan.canonical_terminals(), &raw);
        assert!(plan.aliases().is_empty());
        assert_eq!(
            plan.statistics().skipped[&Skip::ParametricPreparationLimit],
            raw.len()
        );
    }
}

#[test]
fn exact_replay_rejects_invalid_or_power_mismatched_parameter_bijections() {
    let family = standard();
    let symanzik =
        SymanzikPolynomials::try_from_family_with_limits(&family, Default::default()).unwrap();
    let variables = Arc::new(
        (0..2)
            .map(symbolica::prelude::PolyVariable::Temporary)
            .collect(),
    );
    let mut momenta = vec![None; 3];
    let support = geometry::prepare(
        &family,
        &[0, 1, 2],
        symanzik.u().raw(),
        &variables,
        &mut momenta,
        &mut Default::default(),
    )
    .unwrap()
    .unwrap();
    for permutation in [vec![0, 0, 1], vec![0, 1, 3], vec![0, 1], vec![0, 1, 2]] {
        assert!(matches!(
            verify::prove(
                &family,
                &key([2, 1, 1]),
                &key([1, 2, 1]),
                support.clone(),
                support.clone(),
                permutation
            ),
            Err(Error::InvalidParametricWitness)
        ));
    }
    assert!(
        verify::prove(
            &family,
            &key([2, 1, 1]),
            &key([1, 2, 1]),
            support.clone(),
            support,
            vec![1, 0, 2]
        )
        .is_ok()
    );
}

#[test]
fn empty_inputs_and_wrong_arities_do_not_construct_polynomial_proofs() {
    let family = standard();
    let empty = prepare(&family, &BTreeSet::new(), Default::default());
    assert_eq!(empty.statistics().analyzed_parametric_supports, 0);
    let wrong = BTreeSet::from([IntegralKey::try_new([1, 1]).unwrap()]);
    assert!(matches!(
        TerminalAliasPlan::vacuum_parametric_equivalences(
            &family,
            &wrong,
            OrderingPolicy::SpiredUncutV1,
            Default::default()
        ),
        Err(Error::WrongArity { .. })
    ));
}
