use std::collections::BTreeMap;

use crate::algebra::CoefficientContext;
use crate::family::AffineDenominator;
use crate::solver::{
    CoordinateCase, IntegralOrder, SearchOptions, SectorConfig, SectorSolver, Seed,
};

use super::*;

fn two_external_family(scaled: bool, zero_gram: bool) -> IntegralFamily {
    let base = CoefficientContext::new(["d", "s00", "s11", "c1", "c2", "a", "b", "nu1", "nu2"]);
    let parameter = |name| base.parameter(name).unwrap();
    IntegralFamily::new(
        "spired-li-source-fixture",
        vec!["k".into()],
        vec!["p0".into(), "p1".into()],
        base.clone(),
        parameter("d"),
        vec![
            AffineDenominator::new(base.zero(), vec![base.one(), base.zero(), base.zero()]),
            AffineDenominator::new(
                parameter("c1"),
                vec![
                    base.zero(),
                    if scaled { parameter("a") } else { base.one() },
                    base.zero(),
                ],
            ),
            AffineDenominator::new(
                parameter("c2"),
                vec![
                    base.zero(),
                    base.zero(),
                    if scaled { parameter("b") } else { base.one() },
                ],
            ),
        ],
        vec![
            vec![
                if zero_gram {
                    base.zero()
                } else {
                    parameter("s00")
                },
                base.zero(),
            ],
            vec![
                base.zero(),
                if zero_gram {
                    base.zero()
                } else {
                    parameter("s11")
                },
            ],
        ],
        vec![base.zero(), parameter("nu1"), parameter("nu2")],
    )
    .unwrap()
}

#[test]
fn two_external_li_matches_direct_external_derivative_with_native_denominator_clearing() {
    for scaled in [false, true] {
        let system = SourceSystem::<3>::from_family(&two_external_family(scaled, false)).unwrap();
        assert_eq!(system.rows.len(), 4);
        let prototype = &system.rows.iter().flatten().next().unwrap().coefficient;
        let variable = |position| {
            prototype
                .variable(&prototype.variables()[position])
                .unwrap()
        };
        let s00 = variable(1);
        let s11 = variable(2);
        let c1 = variable(3);
        let c2 = variable(4);
        let n1 = &variable(system.indices[1]) + &variable(7);
        let n2 = &variable(system.indices[2]) + &variable(8);
        let aa = if scaled {
            &variable(5) * &variable(5)
        } else {
            prototype.one()
        };
        let bb = if scaled {
            &variable(6) * &variable(6)
        } else {
            prototype.one()
        };
        // D0=k², D1=a k.p0+c1, D2=b k.p1+c2, p0.p1=0.
        // SpIRed contracts the external Lorentz generator with (p1,p0).
        // Its four coefficients are a/b * n1*s00*(c2-D2) and
        // b/a * n2*s11*(D1-c1); clearing their LCM a*b gives these rows.
        // Noninteger power offsets are included once, as in shiftNonIntPows.
        let expected = BTreeMap::from([
            (
                Integral::symbolic([0, 1, 0]).unwrap(),
                &(&(&aa * &c2) * &s00) * &n1,
            ),
            (
                Integral::symbolic([0, 1, -1]).unwrap(),
                -(&(&aa * &s00) * &n1),
            ),
            (
                Integral::symbolic([0, 0, 1]).unwrap(),
                -(&(&(&bb * &c1) * &s11) * &n2),
            ),
            (Integral::symbolic([0, -1, 1]).unwrap(), &(&bb * &s11) * &n2),
        ]);
        let actual: BTreeMap<_, _> = system.rows[3]
            .iter()
            .map(|term| (term.integral, term.coefficient.clone()))
            .collect();
        assert_eq!(actual, expected, "scaled={scaled}");
        assert_eq!(!system.conditions.is_empty(), scaled);
    }
}

#[test]
fn ordinary_and_li_source_order_matches_cpp_not_generic_generator_order() {
    let family = two_external_family(false, false);
    let system = SourceSystem::<3>::from_family(&family).unwrap();
    let ordinary_only = SourceSystem::<3>::from_family_with_lorentz(&family, false).unwrap();
    assert_eq!(ordinary_only.rows.len(), 3);
    assert_eq!(ordinary_only.rows, system.rows[..3]);
    let generator = ParametricIbpGenerator::try_new(&family).unwrap();
    let batch = generator.prepare_ordinary_ibp().unwrap();
    let generated = (0..batch.len())
        .map(|index| batch.generate(index))
        .collect();
    let completed = batch.complete(generated).unwrap();
    for (ported, generic) in [2, 1, 0].into_iter().enumerate() {
        assert_eq!(
            system.rows[ported],
            lower_relation::<3>(&completed.relations()[generic], &mut Vec::new()).unwrap()
        );
    }

    // Four external vectors distinguish reversed triangular traversal from
    // simply reversing the generic lexicographic pair vector.
    let mut identities = Vec::new();
    for contraction in 0..6 {
        for differentiated_loop in 0..2 {
            identities.push(RowId::OrdinaryIbp {
                contraction_momentum: contraction,
                differentiated_loop,
            });
        }
    }
    for first_external in 0..4 {
        for second_external in first_external + 1..4 {
            identities.push(RowId::LorentzInvariance {
                first_external,
                second_external,
            });
        }
    }
    identities.sort_unstable_by_key(|row| reference_source_order(row, 2, 4));
    let ordinary_expected: Vec<_> = (0..2)
        .flat_map(|differentiated_loop| {
            [5, 4, 3, 2, 0, 1].map(|contraction_momentum| RowId::OrdinaryIbp {
                contraction_momentum,
                differentiated_loop,
            })
        })
        .collect();
    assert_eq!(identities[..12], ordinary_expected);
    let pairs_expected = [(2, 3), (1, 3), (0, 3), (1, 2), (0, 2), (0, 1)].map(
        |(first_external, second_external)| RowId::LorentzInvariance {
            first_external,
            second_external,
        },
    );
    assert_eq!(identities[12..], pairs_expected);
}

#[test]
fn identically_zero_li_rows_keep_reference_source_ordinals() {
    let family = two_external_family(false, true);
    let system = SourceSystem::<3>::from_family(&family).unwrap();
    assert_eq!(system.rows.len(), 4);
    assert!(system.rows[3].is_empty());
    assert!(system.rows[..3].iter().any(|row| !row.is_empty()));
}

#[test]
fn vacuum_source_counts_are_unchanged_by_default_li_inclusion() {
    fn check<const N: usize>(family: IntegralFamily, count: usize) {
        let all = SourceSystem::<N>::from_family(&family).unwrap();
        let ordinary = SourceSystem::<N>::from_family_with_lorentz(&family, false).unwrap();
        assert_eq!(all.rows.len(), count);
        assert_eq!(all.rows, ordinary.rows);
    }
    check::<1>(crate::solver::tests::tadpole(), 1);
    check::<3>(crate::solver::tests::sunset(), 4);
    check::<6>(crate::solver::tests::vac3(), 9);
}

fn prepared_fixture() -> SourceSystem<2> {
    let context = CoefficientContext::new(["a", "b", "d"]);
    SourceSystem::new_with_fixed(
        vec![vec![Term {
            integral: Integral::new([Power::new(false, 1).unwrap(), Power::new(true, 1).unwrap()]),
            coefficient: context.coefficient_fixture("b^2+d").numerator,
        }]],
        [0, 1],
        [Some(1), None],
    )
    .unwrap()
}

#[test]
fn prepared_coordinates_stay_absolute_under_symbolic_and_numeric_seeding() {
    let system = prepared_fixture();
    let context = CoefficientContext::new(["a", "b", "d"]);
    let order = IntegralOrder::new([true; 2], [true, false]);
    let instantiate = |seed| {
        super::super::instantiate::instantiate(
            &system.rows()[0],
            &seed,
            system.index_variables(),
            system.fixed(),
            &order,
            &[],
            None,
        )
        .unwrap()
    };
    let symbolic = instantiate(Seed {
        integral: Integral::new([Power::new(false, 1).unwrap(), Power::new(true, -2).unwrap()]),
        shifts: [0, -2],
    });
    assert_eq!(symbolic.len(), 1);
    assert_eq!(
        symbolic[0].integral,
        Integral::new([Power::new(false, 1).unwrap(), Power::new(true, -1).unwrap(),])
    );
    assert_eq!(
        symbolic[0].coefficient,
        context.coefficient_fixture("(b-2)^2+d")
    );

    let numeric = instantiate(Seed {
        integral: Integral::numeric([1, 3]).unwrap(),
        shifts: [0; 2],
    });
    assert_eq!(numeric[0].integral, Integral::numeric([1, 4]).unwrap());
    assert_eq!(numeric[0].coefficient, context.coefficient_fixture("9+d"));
}

#[test]
fn prepared_sources_reject_inconsistent_term_patterns_and_unspecialized_coefficients() {
    let context = CoefficientContext::new(["a", "b", "d"]);
    let correct = prepared_fixture().rows()[0][0].clone();
    let mut symbolic_fixed = correct.clone();
    symbolic_fixed.integral = Integral::symbolic([1, 1]).unwrap();
    let mut wrong_fixed = correct.clone();
    wrong_fixed.integral =
        Integral::new([Power::new(false, 2).unwrap(), Power::new(true, 1).unwrap()]);
    let mut numeric_free = correct.clone();
    numeric_free.integral = Integral::numeric([1, 1]).unwrap();
    let mut unspecialized = correct.clone();
    unspecialized.coefficient = context.coefficient_fixture("a+b").numerator;
    for malformed in [symbolic_fixed, wrong_fixed, numeric_free, unspecialized] {
        assert!(matches!(
            SourceSystem::new_with_fixed(
                vec![vec![correct.clone(), malformed]],
                [0, 1],
                [Some(1), None],
            ),
            Err(SolverError::InvalidInput(_))
        ));
    }
    assert!(matches!(
        SourceSystem::new(vec![vec![correct]], [0, 1]),
        Err(SolverError::InvalidInput(_))
    ));
    assert!(SourceSystem::<2>::new_with_fixed(vec![Vec::new()], [0, 1], [Some(1), None]).is_err());
}

#[test]
fn incompatible_prepared_seeds_fail_before_even_an_empty_row_is_instantiated() {
    let system = prepared_fixture();
    let order = IntegralOrder::new([true; 2], [true, false]);
    let seeds = [
        Seed {
            integral: Integral::numeric([2, 1]).unwrap(),
            shifts: [0; 2],
        },
        Seed {
            integral: Integral::symbolic([0; 2]).unwrap(),
            shifts: [0; 2],
        },
        Seed {
            integral: Integral::numeric([1, 1]).unwrap(),
            shifts: [1, 0],
        },
    ];
    for seed in seeds {
        for row in [&system.rows()[0], &Vec::new()] {
            assert!(matches!(
                super::super::instantiate::instantiate(
                    row,
                    &seed,
                    system.index_variables(),
                    system.fixed(),
                    &order,
                    &[],
                    None,
                ),
                Err(SolverError::InvalidInput(_))
            ));
        }
    }
}

#[test]
fn prepared_replacement_preserves_metadata_even_when_every_row_vanishes() {
    let context = CoefficientContext::new(["a", "b", "d"]);
    let mut system = SourceSystem::<2>::new(
        vec![vec![Term {
            integral: Integral::symbolic([0; 2]).unwrap(),
            coefficient: context.coefficient_fixture("a+b").numerator,
        }]],
        [0, 1],
    )
    .unwrap();
    system.coefficient_priority = vec![1, 2, 0];
    system.conditions = vec![context.coefficient_fixture("d").numerator];
    let variables = system.variables.clone();
    let prepared = prepared_fixture().rows;
    let system = system
        .replace_prepared_rows(prepared, [Some(1), None])
        .unwrap();
    assert_eq!(system.fixed(), &[Some(1), None]);
    assert_eq!(system.coefficient_order(), &[1, 2, 0]);
    assert_eq!(
        system.conditions(),
        &[context.coefficient_fixture("d").numerator]
    );
    assert!(Arc::ptr_eq(&variables, &system.variables));
    let system = system
        .replace_prepared_rows(vec![Vec::new(), Vec::new()], [Some(1), None])
        .unwrap();
    assert_eq!(system.rows().len(), 2);
    assert!(system.rows().iter().all(Vec::is_empty));
    assert_eq!(system.variable_count, 3);
    assert_eq!(system.index_variables(), &[0, 1]);
    assert_eq!(system.coefficient_order(), &[1, 2, 0]);
    assert!(Arc::ptr_eq(&variables, &system.variables));
    // Even after losing all terms, the original native variable map remains
    // authoritative. A same-length but differently named map is not accepted.
    let other = CoefficientContext::new(["x", "y", "z"]);
    let foreign = vec![vec![Term {
        integral: Integral::new([Power::new(false, 1).unwrap(), Power::new(true, 0).unwrap()]),
        coefficient: other.coefficient_fixture("1").numerator,
    }]];
    assert!(matches!(
        system.replace_prepared_rows(foreign, [Some(1), None]),
        Err(SolverError::InvalidInput(_))
    ));
}

#[test]
fn fixed_source_is_only_reused_for_its_prepared_coordinate_case() {
    let context = CoefficientContext::new(["a", "b"]);
    let integral = |shift| {
        Integral::new([
            Power::new(false, 1).unwrap(),
            Power::new(true, shift).unwrap(),
        ])
    };
    let system = SourceSystem::new_with_fixed(
        vec![vec![
            Term {
                integral: integral(0),
                coefficient: context.coefficient_fixture("b").numerator,
            },
            Term {
                integral: integral(-1),
                coefficient: context.coefficient_fixture("-1").numerator,
            },
        ]],
        [0, 1],
        [Some(1), None],
    )
    .unwrap();
    assert!(matches!(
        SectorSolver::new(&system, [true; 2], SectorConfig::default()),
        Err(SolverError::InvalidInput(_))
    ));
    let solver = SectorSolver::new(
        &system,
        [true; 2],
        SectorConfig {
            deltas: [true, false],
            removed_deltas: [true, false],
            ..Default::default()
        },
    )
    .unwrap();
    let options = SearchOptions {
        max_depth: Some(0),
        ..Default::default()
    };
    assert!(matches!(
        solver.solve_case(CoordinateCase::generic(), options),
        Err(SolverError::InvalidInput(_))
    ));
    let rule = solver
        .solve_case(CoordinateCase::new([Some(1), None]).unwrap(), options)
        .unwrap();
    assert_eq!(rule.target, integral(0));
    assert_eq!(rule.rhs.len(), 1);
    assert_eq!(rule.rhs[0].integral, integral(-1));
    assert_eq!(rule.rhs[0].coefficient, context.coefficient_fixture("1/b"));
}
