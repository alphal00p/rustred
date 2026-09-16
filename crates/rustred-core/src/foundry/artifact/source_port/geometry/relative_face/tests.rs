use super::*;
use crate::algebra::CoefficientContext;
use crate::solver::{Case, ExceptionalConditions, RuleCandidate, SectorRule};

fn affine(
    context: &CoefficientContext,
    fixed: [Option<i16>; 4],
    equations: &[&str],
    indices: [usize; 4],
) -> AffineCase<4> {
    let equations = equations
        .iter()
        .map(|equation| context.coefficient_fixture(equation).numerator)
        .collect::<Vec<_>>();
    let AffineIntersection::Affine(result) = AffineCase::from_coordinate(
        &CoordinateCase::new(fixed).unwrap(),
        &equations,
        &indices,
        &[false; 4],
    )
    .unwrap() else {
        panic!("expected coupled fixture");
    };
    result
}

#[test]
fn relative_faces_intersect_new_fixed_coordinates_before_testing_equations() {
    let context = CoefficientContext::new(["a", "b", "c", "e"]);
    // The five activation boundaries found in the four-loop diagnostic,
    // expressed without topology-specific engine inputs.
    for (parent_equation, axis, child_equation) in [
        ("a-2*b-c-1", 2, "a-2*b-1"),
        ("2*a-b-c-1", 2, "2*a-b-1"),
        ("2*a-b+c-1", 0, "1-c+b"),
        ("2*a-b-c-1", 1, "2*a-c-1"),
        ("2*a+b-c-1", 0, "b-c-1"),
    ] {
        let parent = affine(&context, [None; 4], &[parent_equation], [0, 1, 2, 3]);
        let mut fixed = [None; 4];
        fixed[axis] = Some(0);
        let child = affine(&context, fixed, &[child_equation], [0, 1, 2, 3]);
        assert!(
            !parent
                .restrict_equation(&child.equations()[0])
                .unwrap()
                .is_zero()
        );
        assert!(is_coordinate_exception(&parent, &child, &[false; 4]).unwrap());
        let case = Case::from(parent);
        let rule = SectorRule {
            candidate: RuleCandidate {
                target: case.integral(),
                case,
                rhs: Vec::new(),
                sources: Vec::new(),
                stats: Default::default(),
            },
            exceptions: ExceptionalConditions {
                branches: vec![vec![
                    context
                        .parameter(["a", "b", "c", "e"][axis])
                        .unwrap()
                        .numerator,
                ]],
            },
        };
        let partition =
            super::super::application_partition(&rule, &[0, 1, 2, 3], &[false; 4], &[]).unwrap();
        assert!(partition.affine_exclusions.is_empty());
        assert_eq!(partition.boxes.len(), 1);
        assert_eq!(partition.boxes[0].lower()[axis], 1);
    }
}

#[test]
fn genuinely_extra_equations_are_not_replaced_with_their_coordinate_hull() {
    let context = CoefficientContext::new(["a", "b", "c", "e"]);
    let parent = affine(&context, [None; 4], &["a-2*b-c-1"], [0, 1, 2, 3]);
    let child = affine(
        &context,
        [None, None, Some(0), None],
        &["a-2*b-1", "e-b"],
        [0, 1, 2, 3],
    );
    assert!(!is_coordinate_exception(&parent, &child, &[false; 4]).unwrap());
    let case = Case::from(parent);
    let rule = SectorRule {
        candidate: RuleCandidate {
            target: case.integral(),
            case,
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions {
            branches: Vec::new(),
        },
    };
    let partition =
        super::super::application_partition(&rule, &[0, 1, 2, 3], &[false; 4], &[child.into()])
            .unwrap();
    assert_eq!(partition.affine_exclusions.len(), 1);
    assert_eq!(partition.boxes[0].lower(), &[0; 4]);
}

#[test]
fn disjoint_fixed_faces_remove_no_parent_points_and_wrong_maps_are_rejected() {
    let context = CoefficientContext::new(["a", "b", "c", "e"]);
    let parent = affine(
        &context,
        [None, None, None, Some(0)],
        &["a-2*b-c-1"],
        [0, 1, 2, 3],
    );
    let child = affine(
        &context,
        [None, None, None, Some(-1)],
        &["a-2*b-c-1"],
        [0, 1, 2, 3],
    );
    assert!(is_coordinate_exception(&parent, &child, &[false; 4]).unwrap());
    let case = Case::from(parent.clone());
    let rule = SectorRule {
        candidate: RuleCandidate {
            target: case.integral(),
            case,
            rhs: Vec::new(),
            sources: Vec::new(),
            stats: Default::default(),
        },
        exceptions: ExceptionalConditions {
            branches: Vec::new(),
        },
    };
    let partition =
        super::super::application_partition(&rule, &[0, 1, 2, 3], &[false; 4], &[child.into()])
            .unwrap();
    assert_eq!(partition.boxes.len(), 1);
    assert_eq!(partition.boxes[0].lower()[3], 0);
    assert_eq!(partition.boxes[0].upper()[3], Some(0));
    let wrong = affine(&context, [None; 4], &["a-2*b-c-1"], [1, 0, 2, 3]);
    assert!(is_coordinate_exception(&parent, &wrong, &[false; 4]).is_err());
    let foreign = CoefficientContext::new(["x", "y", "z", "w"]);
    let wrong = affine(&foreign, [None; 4], &["x-2*y-z-1"], [0, 1, 2, 3]);
    assert!(is_coordinate_exception(&parent, &wrong, &[false; 4]).is_err());
}
