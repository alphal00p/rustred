use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{
    Case, CoordinateCase, Integral, RuleCandidate, SectorConfig, SectorSolver, SeedSource, Term,
    instantiate_source_port,
};

fn source(context: &CoefficientContext, terms: &[(i16, &str)]) -> crate::solver::PolynomialRow<1> {
    terms
        .iter()
        .map(|&(shift, value)| Term {
            integral: Integral::symbolic([shift]).unwrap(),
            coefficient: context.coefficient_fixture(value).numerator,
        })
        .collect()
}

fn rule<const N: usize>(case: Case<N>, sources: Vec<SeedSource<N>>) -> SectorRule<N> {
    SectorRule {
        candidate: RuleCandidate {
            target: case.integral(),
            case,
            sources,
            rhs: Vec::new(),
            stats: Default::default(),
        },
        exceptions: Default::default(),
    }
}

#[test]
fn composed_multiseed_recentered_weights_replay_the_full_original_rows() {
    let context = CoefficientContext::new(["n", "d"]);
    let system = SourceSystem::new(
        vec![
            source(&context, &[(0, "n"), (-1, "1")]),
            source(&context, &[(0, "d"), (-1, "2")]),
        ],
        [0],
    )
    .unwrap();
    let (solver, derivation) =
        SectorSolver::new_with_provenance(&system, [true], SectorConfig::default()).unwrap();
    let plain = SectorSolver::new(&system, [true], SectorConfig::default()).unwrap();
    assert_eq!(solver.basis(), plain.basis());
    let seeds = [
        Seed {
            integral: Integral::symbolic([1]).unwrap(),
            shifts: [1],
        },
        Seed {
            integral: Integral::symbolic([0]).unwrap(),
            shifts: [0],
        },
    ];
    let rule = rule(
        Case::generic(),
        vec![
            SeedSource {
                basis_row: 0,
                seed: seeds[0],
            },
            SeedSource {
                basis_row: 1,
                seed: seeds[1],
            },
            SeedSource {
                basis_row: 0,
                seed: seeds[0],
            },
        ],
    );
    let selected = [
        (0, context.one()),
        (1, context.coefficient_fixture("d")),
        (2, context.integer(2)),
    ];
    let pivot = context.coefficient_fixture("n+3");
    let replay = BasisReplay {
        derivation: &derivation,
        weights: &selected,
        pivot: &pivot,
    };
    let original = compose(&system, &rule, &seeds, [-1], &replay).unwrap();
    let translated_pivot = translate_source_port(&pivot, &[0], &[-1]);
    let mut expected = Vec::new();
    for (ordinal, weight) in &selected {
        let source = rule.candidate.sources[*ordinal];
        let row = instantiate_source_port(
            &solver.basis()[source.basis_row],
            &source.seed,
            &[0],
            system.fixed(),
            solver.ordering(),
            &[],
            None,
        )
        .unwrap();
        for term in row {
            expected.push(Term {
                integral: term.integral.shifted([-1]).unwrap(),
                coefficient: &translate_source_port(&(weight * &term.coefficient), &[0], &[-1])
                    / &translated_pivot,
            });
        }
    }
    let mut rows = Vec::new();
    for seed in seeds {
        let translated = Seed {
            integral: seed.integral.shifted([-1]).unwrap(),
            shifts: [seed.shifts[0] - 1],
        };
        for row in system.rows() {
            rows.push(
                instantiate_source_port(
                    row,
                    &translated,
                    &[0],
                    system.fixed(),
                    solver.ordering(),
                    &[],
                    None,
                )
                .unwrap(),
            );
        }
    }
    super::super::native::verify(&rows, &expected, &original, solver.ordering(), |_| {
        Ok(false)
    })
    .unwrap();
}

#[test]
fn scalar_instantiation_shifts_before_affine_restriction_and_keeps_fixed_values_absolute() {
    let context = CoefficientContext::new(["a", "b"]);
    let case = Case::<2>::generic()
        .intersect(
            &[context.coefficient_fixture("1+a-2*b").numerator],
            &[0, 1],
            &[true; 2],
        )
        .unwrap()
        .unwrap();
    let affine = case.affine().unwrap();
    assert!(affine.is_tangent(&[2, 1]));
    let combined = Seed {
        integral: Integral::symbolic([3, 1]).unwrap(),
        shifts: [3, 1],
    };
    let result = instantiate_polynomial_source_port(
        &context.coefficient_fixture("a+1").numerator,
        &combined,
        &[0, 1],
        &[None; 2],
        Some(affine),
    )
    .unwrap();
    assert_eq!(result, context.coefficient_fixture("2*b+3"));
    let fixed = CoordinateCase::new([None, Some(-2)]).unwrap();
    let seed = Seed {
        integral: fixed.integral().shifted([1, 0]).unwrap(),
        shifts: [1, 0],
    };
    let result = instantiate_polynomial_source_port(
        &context.coefficient_fixture("a+b").numerator,
        &seed,
        &[0, 1],
        &[None; 2],
        None,
    )
    .unwrap();
    assert_eq!(result, context.coefficient_fixture("a-1"));
    assert!(
        instantiate_polynomial_source_port(
            &context.one().numerator,
            &seed,
            &[0, 1],
            &[None, Some(1)],
            None
        )
        .is_err()
    );
    assert!(
        instantiate_polynomial_source_port(
            &context.one().numerator,
            &combined,
            &[1, 0],
            &[None; 2],
            Some(affine)
        )
        .is_err()
    );
}

#[test]
fn avoidable_composed_pole_and_invalid_replay_fall_back_to_the_existing_certificate() {
    let context = CoefficientContext::new(["n"]);
    let system = SourceSystem::new(
        vec![source(&context, &[(0, "n")]), source(&context, &[(0, "1")])],
        [0],
    )
    .unwrap();
    let (solver, derivation) =
        SectorSolver::new_with_provenance(&system, [false], SectorConfig::default()).unwrap();
    let seed = Seed {
        integral: Integral::symbolic([0]).unwrap(),
        shifts: [0],
    };
    let rule = rule(
        Case::generic(),
        vec![
            SeedSource { basis_row: 0, seed },
            SeedSource { basis_row: 1, seed },
        ],
    );
    let ids = [0, 1].map(|contraction_momentum| crate::identity::RowId::OrdinaryIbp {
        contraction_momentum,
        differentiated_loop: 0,
    });
    let boxes = super::super::geometry::application_boxes(&rule, &[0], &[false], &[]).unwrap();
    let pivot = context.one();
    // This valid polynomial certificate differs from the old complete
    // proposal [0,1], so retaining both sources proves the fast path was used.
    let accepted = [(0, context.one()), (1, context.integer(-1))];
    let frame = BasisReplay {
        derivation: &derivation,
        weights: &accepted,
        pivot: &pivot,
    };
    let result = super::super::weights(
        &system,
        &ids,
        solver.ordering(),
        &[],
        &rule,
        [0],
        &boxes,
        Some(&frame),
    )
    .unwrap();
    assert_eq!(result.contributions.len(), 2);
    assert_eq!(result.contributions[0].weight, context.one());
    assert_eq!(
        result.contributions[1].weight,
        context.coefficient_fixture("1-n")
    );
    let selected = [(0, context.one()), (1, context.coefficient_fixture("-1/n"))];
    let frame = BasisReplay {
        derivation: &derivation,
        weights: &selected,
        pivot: &pivot,
    };
    let candidate = compose(&system, &rule, &[seed], [0], &frame).unwrap();
    assert_eq!(
        candidate,
        vec![context.coefficient_fixture("1/n"), context.zero()]
    );
    assert!(
        !super::super::guards::preserves_domain(&candidate, &rule, &boxes, &[0], &[false]).unwrap()
    );
    for selected in [
        selected.to_vec(),
        vec![(0, context.integer(2))],
        vec![(99, context.one())],
    ] {
        let frame = BasisReplay {
            derivation: &derivation,
            weights: &selected,
            pivot: &pivot,
        };
        let result = super::super::weights(
            &system,
            &ids,
            solver.ordering(),
            &[],
            &rule,
            [0],
            &boxes,
            Some(&frame),
        )
        .unwrap();
        assert_eq!(result.contributions.len(), 1);
        assert_eq!(result.contributions[0].source_row, ids[1]);
        assert_eq!(result.contributions[0].weight, context.one());
    }
}

#[test]
fn composition_rejects_foreign_maps_bad_basis_and_nontangent_affine_recentering() {
    let (context, system, mut rule, order) = super::super::tests::affine_fixture();
    let (_, derivation) =
        SectorSolver::new_with_provenance(&system, *order.sector(), SectorConfig::default())
            .unwrap();
    let selected = [(0, context.one())];
    let pivot = context.one();
    let frame = BasisReplay {
        derivation: &derivation,
        weights: &selected,
        pivot: &pivot,
    };
    let seeds = [rule.candidate.sources[0].seed];
    assert!(compose(&system, &rule, &seeds, [1, 0], &frame).is_err());
    rule.candidate.sources[0].basis_row = 999;
    assert!(compose(&system, &rule, &seeds, [0, 0], &frame).is_err());
    let foreign = CoefficientContext::new(["foreign"]);
    let bad_pivot = foreign.one();
    let frame = BasisReplay {
        derivation: &derivation,
        weights: &selected,
        pivot: &bad_pivot,
    };
    assert!(compose(&system, &rule, &seeds, [0, 0], &frame).is_err());
}

#[test]
fn tangent_affine_recentered_composition_passes_full_original_replay() {
    let context = CoefficientContext::new(["a", "b"]);
    let term = |shift, value| Term {
        integral: Integral::symbolic(shift).unwrap(),
        coefficient: context.coefficient_fixture(value).numerator,
    };
    let system = SourceSystem::new(
        vec![
            vec![term([-3, -1], "a+1"), term([-4, -1], "1")],
            vec![term([-3, -1], "1"), term([-5, -1], "1")],
        ],
        [0, 1],
    )
    .unwrap();
    let case = Case::<2>::generic()
        .intersect(
            &[context.coefficient_fixture("1+a-2*b").numerator],
            &[0, 1],
            &[true; 2],
        )
        .unwrap()
        .unwrap();
    let (solver, derivation) =
        SectorSolver::new_with_provenance(&system, [true; 2], SectorConfig::default()).unwrap();
    let order = solver.ordering();
    let seed = Seed {
        integral: Integral::symbolic([1, 0]).unwrap(),
        shifts: [1, 0],
    };
    // The first basis row is (a+1) times the second original row: its
    // nonconstant DAG edge must really undergo seed+recenter then chart.
    let physical = instantiate_source_port(
        &solver.basis()[0],
        &seed,
        &[0, 1],
        system.fixed(),
        order,
        &[],
        case.affine(),
    )
    .unwrap();
    let pivot = physical[0].coefficient.clone();
    assert_eq!(pivot, context.coefficient_fixture("2*b+1"));
    let (target, rhs) = crate::solver::canonicalize_source_port(physical, &[0, 1]).unwrap();
    assert_eq!(target, case.integral());
    let rule = SectorRule {
        candidate: RuleCandidate {
            case,
            target,
            rhs,
            sources: vec![SeedSource { basis_row: 0, seed }],
            stats: Default::default(),
        },
        exceptions: Default::default(),
    };
    let selected = [(0, context.one())];
    let frame = BasisReplay {
        derivation: &derivation,
        weights: &selected,
        pivot: &pivot,
    };
    let composed = compose(&system, &rule, &[seed], [2, 1], &frame).unwrap();
    assert_eq!(composed, vec![context.zero(), context.one()]);
    let translated = Seed {
        integral: seed.integral.shifted([2, 1]).unwrap(),
        shifts: [3, 1],
    };
    let original = system
        .rows()
        .iter()
        .map(|row| {
            instantiate_source_port(
                row,
                &translated,
                system.index_variables(),
                system.fixed(),
                order,
                &[],
                rule.candidate.case.affine(),
            )
            .unwrap()
        })
        .collect::<Vec<_>>();
    let mut desired = vec![Term {
        integral: rule.candidate.target,
        coefficient: context.one(),
    }];
    desired.extend(rule.candidate.rhs.iter().map(|term| Term {
        integral: term.integral,
        coefficient: -term.coefficient.clone(),
    }));
    super::super::native::verify(&original, &desired, &composed, order, |_| Ok(false)).unwrap();
    assert!(
        super::super::guards::preserves_domain(
            &composed,
            &rule,
            &[],
            system.index_variables(),
            order.sector()
        )
        .unwrap()
    );
}
