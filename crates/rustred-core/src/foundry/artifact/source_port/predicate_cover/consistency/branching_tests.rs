use super::*;
use crate::algebra::CoefficientContext;
use crate::foundry::artifact::source_port::{
    AffineApplicationDomain,
    predicate_cover::{
        Clause, PredicateCoverError, PredicateCoverLimits, PredicateCoveragePiece, TraversalWork,
        certify_predicate_cover, check_valuations,
    },
};
use crate::solver::{AffineCase, AffineIntersection, CoordinateCase};

#[test]
fn early_impossible_literal_avoids_late_only_boolean_explosion_for_the_same_cover() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let sector = [false; 3];
    let domains: Vec<_> = (1..=21)
        .map(|factor| {
            let equation = context
                .coefficient_fixture(&format!("a-{factor}*b"))
                .numerator;
            let AffineIntersection::Affine(case) = AffineCase::from_coordinate(
                &CoordinateCase::generic(),
                &[equation],
                &[1, 2, 3],
                &sector,
            )
            .unwrap() else {
                panic!("expected coupled equality")
            };
            AffineApplicationDomain::from_case(&case, &sector).unwrap()
        })
        .collect();
    let outside = [
        LatticeBox::try_new([1, 0, 0], [None; 3]).unwrap(),
        LatticeBox::try_new([0, 1, 0], [Some(0), None, None]).unwrap(),
    ];
    let face = [LatticeBox::try_new([0; 3], [Some(0), Some(0), None]).unwrap()];
    let mut owners = vec![
        PredicateCoveragePiece {
            boxes: &outside,
            affine_target: None,
            affine_exclusions: &[],
        },
        PredicateCoveragePiece {
            boxes: &face,
            affine_target: Some(&domains[0]),
            affine_exclusions: &[],
        },
    ];
    owners.extend(domains[1..].iter().map(|domain| PredicateCoveragePiece {
        boxes: &outside,
        affine_target: Some(domain),
        affine_exclusions: &[],
    }));
    let limits = PredicateCoverLimits {
        max_boolean_nodes: 4,
        ..Default::default()
    };
    // The false a=b branch leaves only a=b=0,c arbitrary. Every later atom
    // occurs in a redundant outside-face owner and vanishes on this face.
    // Late-only consistency must descend through all20 remaining atoms before
    // seeing its first leaf: already >4 nodes, and full exploration has2^20
    // descendants. Early pruning discharges the branch at its second node.
    assert!(domains.len() + 1 > limits.max_boolean_nodes);
    let proof = certify_predicate_cover(&sector, &owners, &[], limits).unwrap();
    assert_eq!(proof.predicates, 21);
    assert_eq!(proof.boolean_nodes, 3);
}

#[test]
fn exhausted_native_work_surfaces_as_budget_error_not_an_uncovered_box() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let atoms = [Atom {
        indices: &[1, 2, 3],
        equation: &equation,
    }];
    let clauses = [
        Clause {
            domain: LatticeBox::try_new([1, 0, 0], [None; 3]).unwrap(),
            literals: vec![],
        },
        Clause {
            domain: LatticeBox::try_new([0, 1, 0], [Some(0), None, None]).unwrap(),
            literals: vec![],
        },
    ];
    let mut work = TraversalWork {
        nodes: 0,
        consistency: WorkBudget { remaining: 0 },
    };
    let mut assignments = [Some(false)];
    assert_eq!(
        check_valuations(
            &[false; 3],
            &atoms,
            &clauses,
            &[],
            &mut assignments,
            &mut work,
            PredicateCoverLimits::default(),
        ),
        Err(PredicateCoverError::Budget(
            "singleton literal substitutions"
        ))
    );
    assert_eq!(assignments, [Some(false)]);
    assert_eq!(work.nodes, 1);
}

#[test]
fn empty_and_unassigned_predicates_do_not_consume_native_work() {
    let context = CoefficientContext::new(["d", "a", "b", "c"]);
    let equation = context.coefficient_fixture("a-b").numerator;
    let atoms = [Atom {
        indices: &[1, 2, 3],
        equation: &equation,
    }];
    let face = LatticeBox::try_new([0; 3], [Some(0), Some(0), None]).unwrap();
    let mut work = WorkBudget { remaining: 0 };
    assert_eq!(work.contradicts(&[false; 3], &face, &[], &[]), Ok(false));
    assert_eq!(
        work.contradicts(&[false; 3], &face, &atoms, &[None]),
        Ok(false)
    );
    assert_eq!(work.remaining, 0);
}
