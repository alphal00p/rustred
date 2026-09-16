use crate::algebra::CoefficientContext;
use crate::solver::{CoordinateCase, ExceptionalConditions, Integral, Power, RuleCandidate};

use super::*;

fn fixture() -> (CoefficientContext, SectorRule<2>, IntegralOrder<2>) {
    let context = CoefficientContext::new(["n0", "n1", "d"]);
    let case = CoordinateCase::new([None, Some(1)]).unwrap();
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: case.into(),
            target: case.integral(),
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    (
        context,
        rule,
        IntegralOrder::new([false, true], [false, false]),
    )
}

fn term(
    context: &CoefficientContext,
    shift: i16,
    fixed: i16,
    coefficient: &str,
) -> Term<2, Coefficient> {
    Term {
        integral: Integral::new([
            Power::new(true, shift).unwrap(),
            Power::new(false, fixed).unwrap(),
        ]),
        coefficient: context.coefficient_fixture(coefficient),
    }
}

#[test]
fn weighted_original_replay_proves_two_point_activation_only_after_combination() {
    let (context, rule, order) = fixture();
    let boxes = geometry::application_boxes(&rule, &[0, 1], order.sector(), &[]).unwrap();
    let desired = vec![term(&context, 0, 1, "1")];
    let rows = vec![
        vec![term(&context, 0, 1, "1"), term(&context, -1, 1, "n0")],
        vec![term(&context, -1, 1, "1"), term(&context, 2, 0, "n0+1")],
    ];
    let zero_product = |term: &Term<2, Coefficient>| {
        geometry::uniformly_zero_term(
            &rule,
            term,
            &boxes,
            order.sector(),
            &[[false, false]],
            &[0, 1],
        )
    };
    // Neither raw activating term is zero over n0 <= 0. The strict projected
    // frame therefore has no exact membership certificate for this target.
    assert!(
        native::propose(&rows, &desired, &order, zero_product)
            .unwrap()
            .is_none()
    );
    let mut weights = native::propose(&rows, &desired, &order, |term| {
        Ok(term.integral[1].value() == 0)
    })
    .unwrap()
    .unwrap();
    assert_eq!(
        weights,
        vec![context.one(), -context.parameter("n0").unwrap()]
    );
    // Full unprojected replay leaves -n0*(n0+1)*I(n0+2,0): the bulk is
    // independently zero-sector, and BOTH points {-1,0} are exactly zero.
    native::verify(&rows, &desired, &weights, &order, zero_product).unwrap();

    weights[1] = &weights[1] + &context.one();
    assert!(native::verify(&rows, &desired, &weights, &order, zero_product).is_err());
    weights[1] = -context.parameter("n0").unwrap();
    let mut corrupt_sources = rows.clone();
    corrupt_sources[0][0].coefficient = context.integer(2);
    assert!(native::verify(&corrupt_sources, &desired, &weights, &order, zero_product).is_err());
    assert!(native::verify(&rows, &desired, &weights[..1], &order, zero_product).is_err());
}

#[test]
fn finite_activation_checks_every_integer_and_rejects_poles_and_infinite_sampling() {
    let (context, rule, order) = fixture();
    let boxes = geometry::application_boxes(&rule, &[0, 1], order.sector(), &[]).unwrap();
    let check = |shift, coefficient| {
        geometry::uniformly_zero_term(
            &rule,
            &term(&context, shift, 0, coefficient),
            &boxes,
            order.sector(),
            &[[false, false]],
            &[0, 1],
        )
    };
    assert!(check(2, "n0*(n0+1)").unwrap());
    assert!(
        !check(2, "n0+1").unwrap(),
        "zero at -1 is insufficient at 0"
    );
    assert!(!check(2, "n0").unwrap(), "zero at 0 is insufficient at -1");
    assert!(!check(2, "1").unwrap());
    assert!(
        !check(2, "n0*(n0+1)/(n0+n1)").unwrap(),
        "0/0 at n0=-1,n1=1 is not zero"
    );
    // With no zero census, the unbounded negative tail remains a symbolic
    // polynomial, even though the finite activation boundary vanishes.
    assert!(
        !geometry::uniformly_zero_term(
            &rule,
            &term(&context, 2, 0, "n0*(n0+1)"),
            &boxes,
            order.sector(),
            &[],
            &[0, 1],
        )
        .unwrap()
    );
}

#[test]
fn finite_zero_proof_budget_is_fail_closed_not_partial_coverage() {
    let context = CoefficientContext::new(["n0", "n1", "n2", "n3", "d"]);
    let sector = [false, true, false, true];
    let case = CoordinateCase::generic();
    let rule = SectorRule {
        candidate: RuleCandidate {
            case: case.into(),
            target: case.integral(),
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    let boxes = geometry::application_boxes(&rule, &[0, 1, 2, 3], &sector, &[]).unwrap();
    let contribution = Term {
        // Every shift is compact-representable, but the first physical sign
        // cell has 32^4 points, exceeding the default proof budget.
        integral: Integral::symbolic([32, -32, 32, -32]).unwrap(),
        coefficient: context.coefficient_fixture("n0*n1*n2*n3"),
    };
    let error =
        geometry::uniformly_zero_term(&rule, &contribution, &boxes, &sector, &[], &[0, 1, 2, 3])
            .unwrap_err();
    assert!(error.to_string().contains("budget"));
}

pub(super) fn affine_fixture() -> (
    CoefficientContext,
    SourceSystem<2>,
    SectorRule<2>,
    IntegralOrder<2>,
) {
    use crate::solver::{Case, Seed, SeedSource};
    let context = CoefficientContext::new(["a", "b"]);
    let system = SourceSystem::new(
        vec![vec![
            Term {
                integral: Integral::symbolic([-1, 0]).unwrap(),
                coefficient: context.coefficient_fixture("1+a-2*b").numerator,
            },
            Term {
                integral: Integral::symbolic([-2, 0]).unwrap(),
                coefficient: context.one().numerator,
            },
        ]],
        [0, 1],
    )
    .unwrap();
    let case = Case::generic()
        .intersect(
            &[context.coefficient_fixture("1+a-2*b").numerator],
            &[0, 1],
            &[true; 2],
        )
        .unwrap()
        .unwrap();
    let rule = SectorRule {
        candidate: RuleCandidate {
            target: case.integral(),
            case,
            rhs: vec![Term {
                integral: Integral::symbolic([-1, 0]).unwrap(),
                coefficient: context.integer(-1),
            }],
            sources: vec![SeedSource {
                basis_row: 0,
                seed: Seed {
                    integral: Integral::symbolic([1, 0]).unwrap(),
                    shifts: [1, 0],
                },
            }],
            stats: Default::default(),
        },
        exceptions: Default::default(),
    };
    (
        context,
        system,
        rule,
        IntegralOrder::new([true; 2], [false; 2]),
    )
}

#[test]
fn affine_ordinary_replay_shifts_before_restriction_and_retains_original_columns() {
    let (context, system, mut rule, order) = affine_fixture();
    let ids = [crate::identity::RowId::OrdinaryIbp {
        contraction_momentum: 0,
        differentiated_loop: 0,
    }];
    // The unshifted pivot vanishes on the case, whereas the transverse source
    // shift [1,0] restores it to 1. Restricting before shifting loses the rule.
    let replay = weights(&system, &ids, &order, &[], &rule, [0; 2], &[], None).unwrap();
    assert_eq!(replay.contributions.len(), 1);
    assert_eq!(replay.contributions[0].offset, [1, 0]);
    assert_eq!(replay.contributions[0].weight, context.one());
    assert_eq!(
        rule.candidate.rhs[0].integral,
        Integral::symbolic([-1, 0]).unwrap()
    );

    // The raw target [-2,-1] is tangent to a-2b+1=0. Its recentering gives
    // exactly the same original-source request, not a chart-coordinate key.
    rule.candidate.sources[0].seed.integral = Integral::symbolic([-1, -1]).unwrap();
    rule.candidate.sources[0].seed.shifts = [-1, -1];
    let recentered = weights(&system, &ids, &order, &[], &rule, [2, 1], &[], None).unwrap();
    assert_eq!(recentered.contributions[0].offset, [1, 0]);
    assert_eq!(recentered.contributions[0].weight, context.one());
    assert!(
        weights(&system, &ids, &order, &[], &rule, [1, 0], &[], None)
            .unwrap_err()
            .to_string()
            .contains("recentering")
    );
}

#[test]
fn affine_ordinary_replay_rejects_changed_source_shift_coefficient_or_equality() {
    for mutation in 0..4 {
        let (context, mut system, mut rule, order) = affine_fixture();
        let ids = [crate::identity::RowId::OrdinaryIbp {
            contraction_momentum: 0,
            differentiated_loop: 0,
        }];
        match mutation {
            0 => {
                let mut rows = system.rows().to_vec();
                rows[0][1].coefficient = context.integer(2).numerator;
                system = SourceSystem::new(rows, [0, 1]).unwrap();
            }
            1 => {
                rule.candidate.sources[0].seed.integral = Integral::symbolic([2, 0]).unwrap();
                rule.candidate.sources[0].seed.shifts = [2, 0];
            }
            2 => rule.candidate.rhs[0].coefficient = context.integer(2),
            3 => {
                rule.candidate.case = crate::solver::Case::generic()
                    .intersect(
                        &[context.coefficient_fixture("2+a-2*b").numerator],
                        &[0, 1],
                        &[true; 2],
                    )
                    .unwrap()
                    .unwrap()
            }
            _ => unreachable!(),
        }
        assert!(
            weights(&system, &ids, &order, &[], &rule, [0; 2], &[], None).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn affine_zero_projection_discards_only_impossible_activation_cells() {
    use crate::solver::Case;

    let context = CoefficientContext::new(["a", "b"]);
    let sector = [false; 2];
    let indices = [0, 1];
    let case = Case::generic()
        .intersect(
            &[context.coefficient_fixture("1+a-2*b").numerator],
            &indices,
            &sector,
        )
        .unwrap()
        .unwrap();
    let mut rule = SectorRule {
        candidate: RuleCandidate {
            target: case.integral(),
            case,
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    let activating = Term {
        integral: Integral::symbolic([0, 1]).unwrap(),
        coefficient: context.one(),
    };
    let zeros = [[false, false]];
    let boxes = geometry::application_boxes(&rule, &indices, &sector, &[]).unwrap();
    // The feasible point (a,b)=(-1,0) activates b+1. Neither the
    // coefficient-bearing proof nor the integral-only query may omit it.
    assert!(
        !geometry::uniformly_zero_term(&rule, &activating, &boxes, &sector, &zeros, &indices)
            .unwrap()
    );
    assert!(
        !geometry::uniformly_zero_column(&rule, activating.integral, &boxes, &sector, &zeros)
            .unwrap()
    );

    rule.exceptions.branches = vec![vec![context.coefficient_fixture("b").numerator]];
    let exceptional = rule.exceptional_cases(&indices, &sector).unwrap();
    assert_eq!(exceptional.len(), 1);
    assert_eq!(
        exceptional[0].coordinate().unwrap().fixed(),
        &[Some(-1), Some(0)],
        "on the affine target, b=0 also fixes a=-1"
    );
    let boxes = geometry::application_boxes(&rule, &indices, &sector, &[]).unwrap();
    // Removing that point still leaves box-prefilter cells with b=0 and
    // a!= -1. A rectangular-only zero check cannot discard those cells.
    assert!(!geometry::uniformly_zero_wide(&[0, 1], None, &boxes, &sector, &zeros).unwrap());
    assert!(
        geometry::uniformly_zero_term(&rule, &activating, &boxes, &sector, &zeros, &indices)
            .unwrap()
    );
    assert!(
        geometry::uniformly_zero_column(&rule, activating.integral, &boxes, &sector, &zeros)
            .unwrap()
    );
    assert!(
        geometry::uniformly_zero_term(&rule, &activating, &boxes, &sector, &zeros, &[1, 0])
            .unwrap_err()
            .to_string()
            .contains("index-variable map")
    );
    let foreign_context = CoefficientContext::new(["x", "y"]);
    let foreign = Term {
        integral: activating.integral,
        coefficient: foreign_context.one(),
    };
    assert!(
        geometry::uniformly_zero_term(&rule, &foreign, &boxes, &sector, &zeros, &indices)
            .unwrap_err()
            .to_string()
            .contains("polynomial variable map")
    );
}

#[test]
fn affine_mixed_finite_boundary_keeps_only_feasible_coefficient_leaves() {
    use crate::sector::OrderingPolicy;
    use crate::solver::Case;

    let context = CoefficientContext::new(["a", "b", "c", "d"]);
    let sector = [false; 3];
    let indices = [0, 1, 2];
    let case = Case::generic()
        .intersect(
            &[context.coefficient_fixture("1+a-2*b").numerator],
            &indices,
            &sector,
        )
        .unwrap()
        .unwrap();
    let activating = Term {
        integral: Integral::symbolic([0, 2, 1]).unwrap(),
        // The H boundary coefficient, with spectator c and generic d.
        coefficient: context.coefficient_fixture(
            "3*(b+1)*(4+3*c+6*b-3*d)/(8+8*c+2*c^2+16*b+8*b*c+6*b^2-10*d-5*d*c-9*d*b+3*d^2)",
        ),
    };
    let mut rule = SectorRule {
        candidate: RuleCandidate {
            target: case.integral(),
            case,
            rhs: vec![activating.clone()],
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions::default(),
    };
    // a<=-2 and c<=-1. The activating b+2 sign cell has b=-1 or0.
    // The former forces a=-3 and kills the numerator; the latter forces
    // a=-1 outside this box. Neither the whole sign cell nor its numerator
    // is identically empty/zero before that exact finite split.
    let boxes = [LatticeBox::try_new([2, 0, 1], [None; 3]).unwrap()];
    assert!(
        geometry::uniformly_zero_term(&rule, &activating, &boxes, &sector, &[sector], &indices)
            .unwrap()
    );
    geometry::prove_descent(
        &rule,
        &boxes,
        &[],
        &sector,
        OrderingPolicy::SpiredUncutV1,
        &indices,
    )
    .unwrap();
    assert!(
        !geometry::uniformly_zero_column(&rule, activating.integral, &boxes, &sector, &[sector])
            .unwrap(),
        "the feasible b=-1 leaf is not itself an empty integral"
    );

    let enlarged = [LatticeBox::try_new([1, 0, 1], [None; 3]).unwrap()];
    assert!(
        !geometry::uniformly_zero_term(&rule, &activating, &enlarged, &sector, &[sector], &indices)
            .unwrap(),
        "the now feasible (a,b)=(-1,0) has nonzero coefficient"
    );
    assert!(
        geometry::prove_descent(
            &rule,
            &enlarged,
            &[],
            &sector,
            OrderingPolicy::SpiredUncutV1,
            &indices
        )
        .is_err()
    );
    let singular = Term {
        integral: activating.integral,
        coefficient: context.coefficient_fixture("(b+1)/(c+1)"),
    };
    let pole = [LatticeBox::try_new([2, 0, 1], [None, None, Some(1)]).unwrap()];
    assert!(
        !geometry::uniformly_zero_term(&rule, &singular, &pole, &sector, &[sector], &indices)
            .unwrap(),
        "0/0 on the feasible b=-1,c=-1 leaf must not disappear"
    );
    rule.candidate.rhs = vec![singular];
    assert!(
        geometry::prove_descent(
            &rule,
            &pole,
            &[],
            &sector,
            OrderingPolicy::SpiredUncutV1,
            &indices
        )
        .is_err()
    );
}
