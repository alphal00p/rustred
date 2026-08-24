use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;

use rustred::{
    CoefficientContext, GuardOrigin, IndexSpace, ParametricArithmeticLimits,
    ParametricCoefficientContext, ParametricCoefficientError, ParametricRelation,
    ParametricRelationError, ParametricRowId, PartialIndexAssignment,
    PartialParametricRelationSpecializationLimits,
};

fn setup(scope: &str) -> (ParametricCoefficientContext, IndexSpace) {
    let base = CoefficientContext::new(["theta"]);
    let context = ParametricCoefficientContext::try_new(&base, scope, 2).unwrap();
    (context, IndexSpace::try_new(2).unwrap())
}

fn source_row(scope: &str) -> (ParametricCoefficientContext, Arc<ParametricRelation>) {
    let (context, space) = setup(scope);
    let n0 = context.index(0).unwrap();
    let n1 = context.index(1).unwrap();
    let theta = context
        .lift(&context.base().parameter("theta").unwrap())
        .unwrap();
    let denominator = context.add(&n0, &theta).unwrap();
    let coefficient = context.checked_div(&n1, &denominator).unwrap();
    let mut relation = ParametricRelation::new(
        "family",
        ParametricRowId::Derived {
            label: "row".into(),
        },
        &context,
    );
    relation
        .add_term(&context, space.zero(), coefficient)
        .unwrap();
    let guard = context.add(&n0, &n1).unwrap();
    relation
        .add_nonzero_condition(&context, context.numerator_condition(&guard).unwrap())
        .unwrap();
    (context, Arc::new(relation))
}

#[test]
fn sparse_assignment_is_canonical_and_rejects_collisions() {
    let assignment = PartialIndexAssignment::try_new([(1, -3), (0, 2)], 2, 2).unwrap();
    assert_eq!(assignment.entries(), &[(0, 2), (1, -3)]);
    assert_eq!(
        PartialIndexAssignment::try_new([(0, 1), (0, 2)], 2, 2),
        Err(ParametricCoefficientError::DuplicateIndexAssignment { position: 0 })
    );
    assert_eq!(
        PartialIndexAssignment::try_new([(2, 1)], 2, 1),
        Err(ParametricCoefficientError::IndexAssignmentOutOfRange {
            position: 2,
            arity: 2,
        })
    );
}

#[test]
fn partial_row_specializes_all_coefficients_and_guards_and_replays() {
    let (context, relation) = source_row("partial-row-replay");
    let assignment = PartialIndexAssignment::try_new([(0, 2)], 2, 1).unwrap();
    let certificate = relation
        .partially_specialized_on(
            &context,
            assignment.clone(),
            PartialParametricRelationSpecializationLimits::default(),
        )
        .unwrap();
    assert_eq!(certificate.assignment(), &assignment);
    assert_eq!(certificate.stats().terms(), 1);
    assert_eq!(certificate.stats().guards(), 2);
    assert!(certificate.stats().source_terms() > 0);
    certificate.replay(&context).unwrap();

    // Public proof data binds the conditional result to n0=2 and carries the
    // partial-specialization origin without exposing a globally valid row.
    let origins = certificate
        .base_assumptions()
        .iter()
        .flat_map(|assumption| assumption.condition().origins());
    assert!(origins.clone().any(|origin| matches!(
        origin,
        GuardOrigin::PartialIndexSpecialization { assignments }
            if assignments.as_ref() == [(0, 2)]
    )));
    assert!(origins.clone().any(|origin| matches!(
        origin,
        GuardOrigin::RelationPartialSpecializationTermDenominator { shift, .. }
            if shift.as_ref() == [0, 0]
    )));
}

#[test]
fn required_guard_becoming_zero_rejects_the_conditional_row() {
    let (context, space) = setup("partial-row-zero-guard");
    let n0 = context.index(0).unwrap();
    let guard = context.sub(&n0, &context.integer(7)).unwrap();
    let mut relation = ParametricRelation::new(
        "family",
        ParametricRowId::Derived {
            label: "zero".into(),
        },
        &context,
    );
    relation
        .add_term(&context, space.zero(), context.one())
        .unwrap();
    relation
        .add_nonzero_condition(&context, context.numerator_condition(&guard).unwrap())
        .unwrap();
    let relation = Arc::new(relation);
    let assignment = PartialIndexAssignment::try_new([(0, 7)], 2, 1).unwrap();
    assert!(matches!(
        relation.partially_specialized_on(
            &context,
            assignment,
            PartialParametricRelationSpecializationLimits::default(),
        ),
        Err(ParametricRelationError::UnsatisfiableDomain)
    ));
}

#[test]
fn base_only_guard_survives_as_a_typed_assumption() {
    let (context, space) = setup("partial-row-base-assumption");
    let n0 = context.index(0).unwrap();
    let theta = context
        .lift(&context.base().parameter("theta").unwrap())
        .unwrap();
    let guard = context.add(&n0, &theta).unwrap();
    let mut relation = ParametricRelation::new(
        "family",
        ParametricRowId::Derived {
            label: "base".into(),
        },
        &context,
    );
    relation
        .add_term(&context, space.zero(), context.one())
        .unwrap();
    relation
        .add_nonzero_condition(&context, context.numerator_condition(&guard).unwrap())
        .unwrap();
    let relation = Arc::new(relation);
    let certificate = relation
        .partially_specialized_on(
            &context,
            PartialIndexAssignment::try_new([(0, 0)], 2, 1).unwrap(),
            PartialParametricRelationSpecializationLimits::default(),
        )
        .unwrap();
    assert_eq!(certificate.base_assumptions().len(), 1);
    assert_eq!(certificate.stats().base_assumptions(), 1);
}

#[test]
fn identical_base_assumptions_merge_all_term_specific_origins() {
    let (context, space) = setup("partial-row-base-merge");
    let n0 = context.index(0).unwrap();
    let theta = context
        .lift(&context.base().parameter("theta").unwrap())
        .unwrap();
    let denominator = context.add(&n0, &theta).unwrap();
    let coefficient = context.checked_div(&context.one(), &denominator).unwrap();
    let mut relation = ParametricRelation::new(
        "family",
        ParametricRowId::Derived {
            label: "merge".into(),
        },
        &context,
    );
    relation
        .add_term(&context, space.zero(), coefficient.clone())
        .unwrap();
    relation
        .add_term(&context, space.unit(1, 1).unwrap(), coefficient)
        .unwrap();
    let relation = Arc::new(relation);
    let assignment = PartialIndexAssignment::try_new([(0, 3)], 2, 1).unwrap();
    let certificate = relation
        .partially_specialized_on(
            &context,
            assignment.clone(),
            PartialParametricRelationSpecializationLimits::default(),
        )
        .unwrap();
    assert_eq!(certificate.base_assumptions().len(), 1);
    let origins = certificate.base_assumptions()[0].condition().origins();
    for expected in [&[0, 0][..], &[0, 1][..]] {
        assert!(origins.iter().any(|origin| matches!(
            origin,
            GuardOrigin::RelationPartialSpecializationTermDenominator { shift, .. }
                if shift.as_ref() == expected
        )));
    }

    let specialized = context
        .partially_specialize_coefficient(
            relation.terms().first_key_value().unwrap().1,
            &assignment,
            ParametricArithmeticLimits::default(),
        )
        .unwrap();
    assert_eq!(specialized.assignment(), &assignment);
}

#[test]
fn foreign_context_is_rejected_by_construction_and_replay() {
    let (context, relation) = source_row("partial-row-context");
    let assignment = PartialIndexAssignment::try_new([(0, 2)], 2, 1).unwrap();
    let certificate = relation
        .partially_specialized_on(
            &context,
            assignment.clone(),
            PartialParametricRelationSpecializationLimits::default(),
        )
        .unwrap();
    let foreign =
        ParametricCoefficientContext::try_new(context.base(), "partial-row-context-foreign", 2)
            .unwrap();
    assert_eq!(
        certificate.replay(&foreign),
        Err(ParametricRelationError::WrongContext)
    );
    assert!(matches!(
        relation.partially_specialized_on(
            &foreign,
            assignment,
            PartialParametricRelationSpecializationLimits::default(),
        ),
        Err(ParametricRelationError::WrongContext)
    ));
}

#[test]
fn cumulative_work_and_retained_bytes_fail_closed_transactionally() {
    let (context, relation) = source_row("partial-row-limits");
    let assignment = PartialIndexAssignment::try_new([(0, 2)], 2, 1).unwrap();

    let mut limits = PartialParametricRelationSpecializationLimits::default();
    limits.max_source_terms = 1;
    assert!(matches!(
        relation.partially_specialized_on(&context, assignment.clone(), limits),
        Err(ParametricRelationError::ResourceLimit {
            resource: "partial relation source terms",
            ..
        })
    ));

    limits = PartialParametricRelationSpecializationLimits::default();
    limits.max_retained_bytes = 1;
    assert!(matches!(
        relation.partially_specialized_on(&context, assignment, limits),
        Err(ParametricRelationError::ResourceLimit {
            resource: "partial relation retained bytes",
            ..
        })
    ));
    // No input mutation: a subsequent unrestricted proof still succeeds.
    relation
        .partially_specialized_on(
            &context,
            PartialIndexAssignment::try_new([(0, 2)], 2, 1).unwrap(),
            PartialParametricRelationSpecializationLimits::default(),
        )
        .unwrap();
}

#[test]
fn extreme_assignments_are_bounded_and_never_unwind() {
    let (context, space) = setup("partial-row-extreme");
    let n0 = context.index(0).unwrap();
    let square = context.mul(&n0, &n0).unwrap();
    let mut relation = ParametricRelation::new(
        "family",
        ParametricRowId::Derived {
            label: "extreme".into(),
        },
        &context,
    );
    relation.add_term(&context, space.zero(), square).unwrap();
    let relation = Arc::new(relation);
    let assignment = PartialIndexAssignment::try_new([(0, i64::MIN)], 2, 1).unwrap();
    let mut limits = PartialParametricRelationSpecializationLimits::default();
    limits.arithmetic = ParametricArithmeticLimits {
        max_specialization_integer_bits: 128,
        ..ParametricArithmeticLimits::default()
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        relation.partially_specialized_on(&context, assignment, limits)
    }));
    assert!(matches!(
        outcome,
        Ok(Err(ParametricRelationError::Coefficient(
            ParametricCoefficientError::ResourceLimit {
                resource: "partial polynomial specialization integer bits",
                ..
            }
        )))
    ));
}
