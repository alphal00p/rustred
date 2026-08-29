use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::{
    CoefficientContext, ExactAlgebraError, ExactAlgebraLimits, IndexedAlgebraError,
    IndexedAlgebraLimits, IndexedCoefficientContext,
};
use crate::identity::condition::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
    ParametricNonZeroCondition, borrowed_condition_deep_clone_counts,
};
use crate::identity::row::RowId;

use super::super::{Builder, IndexSpace, ParametricRelationError, RelationLimits};
use super::support::actual_input_denominator_condition;

#[test]
fn repeated_rational_term_merges_real_denominator_sources() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "condition-merge", 1).unwrap();
    let row_id = RowId::Derived {
        label: Arc::from("condition-source"),
    };
    let mut relation = Builder::new("family".to_owned(), row_id.clone(), &context);
    let shift = IndexSpace::try_new(1).unwrap().try_zero().unwrap();
    let reciprocal = context.lift(&base.coefficient_fixture("1/x")).unwrap();
    let input_source = IdentityConditionSource::RelationInputTermDenominator {
        row: row_id.clone(),
        shift: vec![0].into_boxed_slice(),
    };
    let collected_source = IdentityConditionSource::RelationCollectedTermDenominator {
        row: row_id.clone(),
        shift: vec![0].into_boxed_slice(),
    };
    let before = context.authentication_scan_counts();
    relation
        .add_term(
            &context,
            shift.clone(),
            reciprocal.clone(),
            RelationLimits::default(),
        )
        .unwrap();
    relation
        .add_term(&context, shift, reciprocal, RelationLimits::default())
        .unwrap();
    let after = context.authentication_scan_counts();
    assert_eq!(
        (after.0 - before.0, after.1 - before.1),
        (2, 1),
        "two relation ingresses need two scans; collecting them needs exactly one authenticated native result"
    );
    let relation = relation.finish();

    assert_eq!(
        relation.terms().values().next(),
        Some(&context.lift(&base.coefficient_fixture("2/x")).unwrap())
    );
    assert_eq!(relation.nonzero_conditions().len(), 1);
    assert_eq!(
        relation.nonzero_conditions()[0].polynomial(),
        &context
            .denominator_condition_with_limits(
                &context.lift(&base.coefficient_fixture("1/x")).unwrap(),
                ExactAlgebraLimits::default(),
            )
            .unwrap()
    );
    assert_eq!(
        relation.nonzero_conditions()[0].sources(),
        &BTreeSet::from([
            input_source,
            collected_source,
            IdentityConditionSource::RelationConditionAttached { row: row_id },
        ])
    );
}

#[test]
fn input_denominator_source_limit_is_enforced_by_real_term_insertion() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "condition-limit", 1).unwrap();
    let row_id = RowId::Derived {
        label: Arc::from("limited"),
    };
    let mut relation = Builder::new("family".to_owned(), row_id, &context);
    let limits = RelationLimits {
        identity_conditions: IdentityConditionLimits { max_sources: 1 },
        ..RelationLimits::default()
    };
    assert!(matches!(
        relation.add_term(
            &context,
            IndexSpace::try_new(1).unwrap().try_zero().unwrap(),
            context.lift(&base.coefficient_fixture("1/x")).unwrap(),
            limits,
        ),
        Err(ParametricRelationError::IdentityCondition(
            IdentityConditionError::ResourceLimit {
                resource: "identity condition sources",
                requested: 2,
                limit: 1,
            }
        ))
    ));
}

#[test]
fn wrong_context_coefficient_is_rejected_before_relation_mutation() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "expected-coefficient", 1).unwrap();
    let foreign = IndexedCoefficientContext::try_new(&base, "foreign-coefficient", 1).unwrap();
    let mut relation = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: Arc::from("wrong-coefficient-context"),
        },
        &context,
    );

    assert_eq!(
        relation.add_term(
            &context,
            IndexSpace::try_new(1).unwrap().try_zero().unwrap(),
            foreign.one(),
            RelationLimits::default(),
        ),
        Err(ParametricRelationError::Coefficient(
            IndexedAlgebraError::WrongContext,
        ))
    );
    let relation = relation.finish();
    assert!(relation.terms().is_empty());
    assert!(relation.nonzero_conditions().is_empty());
}

#[test]
fn wrong_context_scale_factor_is_rejected_before_relation_mutation() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "expected-factor", 1).unwrap();
    let foreign = IndexedCoefficientContext::try_new(&base, "foreign-factor", 1).unwrap();
    let source = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: Arc::from("factor-source"),
        },
        &context,
    )
    .finish();
    let mut target = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: Arc::from("factor-target"),
        },
        &context,
    );

    assert_eq!(
        target.add_scaled(&context, &source, &foreign.one(), RelationLimits::default(),),
        Err(ParametricRelationError::Coefficient(
            IndexedAlgebraError::WrongContext,
        ))
    );
    let target = target.finish();
    assert!(target.terms().is_empty());
    assert!(target.nonzero_conditions().is_empty());
}

#[test]
fn wrong_context_condition_is_rejected_before_relation_mutation() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "expected-condition", 1).unwrap();
    let foreign = IndexedCoefficientContext::try_new(&base, "foreign-condition", 1).unwrap();
    let foreign_coefficient = foreign.lift(&base.coefficient_fixture("1/x")).unwrap();
    let foreign_polynomial = foreign
        .denominator_condition_with_limits(&foreign_coefficient, ExactAlgebraLimits::default())
        .unwrap();
    let foreign_condition = ParametricNonZeroCondition::try_new_with_limits(
        &foreign,
        foreign_polynomial,
        [IdentityConditionSource::FamilyBasisDeterminantNumerator],
        ExactAlgebraLimits::default(),
        IdentityConditionLimits::default(),
    )
    .unwrap();
    let mut relation = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: Arc::from("wrong-condition-context"),
        },
        &context,
    );

    assert_eq!(
        relation.add_nonzero_condition(&context, foreign_condition, RelationLimits::default()),
        Err(ParametricRelationError::Coefficient(
            IndexedAlgebraError::WrongContext,
        ))
    );
    let relation = relation.finish();
    assert!(relation.terms().is_empty());
    assert!(relation.nonzero_conditions().is_empty());
}

#[test]
fn checked_condition_insertion_still_enforces_current_arithmetic_limits() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "condition-readmission", 1).unwrap();
    let coefficient = context.lift(&base.coefficient_fixture("1/(x+1)")).unwrap();
    let polynomial = context
        .denominator_condition_with_limits(&coefficient, ExactAlgebraLimits::default())
        .unwrap();
    let condition = ParametricNonZeroCondition::try_new_with_limits(
        &context,
        polynomial,
        [IdentityConditionSource::FamilyBasisDeterminantNumerator],
        ExactAlgebraLimits::default(),
        IdentityConditionLimits::default(),
    )
    .unwrap();
    let mut relation = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: Arc::from("condition-readmission"),
        },
        &context,
    );
    let limits = RelationLimits {
        arithmetic: IndexedAlgebraLimits {
            exact_algebra: ExactAlgebraLimits {
                max_polynomial_terms: 1,
                ..ExactAlgebraLimits::default()
            },
            ..IndexedAlgebraLimits::default()
        },
        ..RelationLimits::default()
    };

    assert!(matches!(
        relation.add_nonzero_condition(&context, condition, limits),
        Err(ParametricRelationError::Coefficient(
            IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                requested: 2,
                limit: 1,
                ..
            })
        ))
    ));
    assert!(relation.finish().nonzero_conditions().is_empty());
}

#[test]
fn scaled_relation_preserves_exact_terms_and_guard_polynomials() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "scaled-guards", 1).unwrap();
    let zero = IndexSpace::try_new(1).unwrap().try_zero().unwrap();
    let source_coefficient = context.lift(&base.coefficient_fixture("1/x")).unwrap();
    let factor = context.lift(&base.coefficient_fixture("1/(x+1)")).unwrap();
    let source_row = RowId::Derived {
        label: Arc::from("scaled-source"),
    };
    let mut source = Builder::new("family".to_owned(), source_row, &context);
    source
        .add_term(
            &context,
            zero.clone(),
            source_coefficient.clone(),
            RelationLimits::default(),
        )
        .unwrap();
    let source = source.finish();
    let mut target = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: Arc::from("scaled-target"),
        },
        &context,
    );
    let before_scaling = context.authentication_scan_counts();
    target
        .add_scaled(&context, &source, &factor, RelationLimits::default())
        .unwrap();
    let after_scaling = context.authentication_scan_counts();
    assert_eq!(
        (
            after_scaling.0 - before_scaling.0,
            after_scaling.1 - before_scaling.1,
        ),
        (1, 1),
        "scaling authenticates the factor once; the sealed source term is not rescanned and the product is authenticated once"
    );
    let relation = target.finish();

    let scaled = context
        .mul_with_limits(&source_coefficient, &factor, ExactAlgebraLimits::default())
        .unwrap();
    assert_eq!(relation.terms().get(&zero), Some(&scaled));
    let expected_guards = [
        context
            .denominator_condition_with_limits(&source_coefficient, ExactAlgebraLimits::default())
            .unwrap(),
        context
            .denominator_condition_with_limits(&factor, ExactAlgebraLimits::default())
            .unwrap(),
        context
            .denominator_condition_with_limits(&scaled, ExactAlgebraLimits::default())
            .unwrap(),
    ];
    assert_eq!(
        relation
            .nonzero_conditions()
            .iter()
            .map(ParametricNonZeroCondition::polynomial)
            .collect::<Vec<_>>(),
        expected_guards.iter().collect::<Vec<_>>(),
    );
}

#[test]
fn scaled_relation_readmits_source_conditions_under_current_limits() {
    let base = CoefficientContext::new(["x"]);
    let context =
        IndexedCoefficientContext::try_new(&base, "scaled-condition-readmission", 1).unwrap();
    let zero = IndexSpace::try_new(1).unwrap().try_zero().unwrap();
    let mut source = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: Arc::from("permissive-source"),
        },
        &context,
    );
    source
        .add_term(
            &context,
            zero,
            context.lift(&base.coefficient_fixture("1/(x+1)")).unwrap(),
            RelationLimits::default(),
        )
        .unwrap();
    let source = source.finish();

    let mut target = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: Arc::from("strict-target"),
        },
        &context,
    );
    let strict = RelationLimits {
        arithmetic: IndexedAlgebraLimits {
            exact_algebra: ExactAlgebraLimits {
                max_polynomial_terms: 1,
                ..ExactAlgebraLimits::default()
            },
            ..IndexedAlgebraLimits::default()
        },
        ..RelationLimits::default()
    };
    let before_arithmetic_rejection = borrowed_condition_deep_clone_counts();
    assert!(matches!(
        target.add_scaled(&context, &source, &context.one(), strict),
        Err(ParametricRelationError::Coefficient(
            IndexedAlgebraError::ExactAlgebra(ExactAlgebraError::ResourceLimit {
                requested: 2,
                limit: 1,
                ..
            })
        ))
    ));
    assert_eq!(
        borrowed_condition_deep_clone_counts(),
        before_arithmetic_rejection,
        "source-condition arithmetic admission must precede polynomial and provenance clones"
    );

    let source_limited = RelationLimits {
        identity_conditions: IdentityConditionLimits { max_sources: 2 },
        ..RelationLimits::default()
    };
    let before_source_rejection = borrowed_condition_deep_clone_counts();
    assert_eq!(
        target
            .add_scaled(&context, &source, &context.one(), source_limited)
            .unwrap_err(),
        ParametricRelationError::IdentityCondition(IdentityConditionError::ResourceLimit {
            resource: "identity condition sources",
            requested: 3,
            limit: 2,
        })
    );
    assert_eq!(
        borrowed_condition_deep_clone_counts(),
        before_source_rejection,
        "prospective target provenance admission must precede polynomial and provenance clones"
    );
    let target = target.finish();
    assert!(target.terms().is_empty());
    assert!(target.nonzero_conditions().is_empty());

    let mut merge_target = Builder::new(
        "family".to_owned(),
        RowId::Derived {
            label: Arc::from("merge-limited-target"),
        },
        &context,
    );
    merge_target
        .add_term(
            &context,
            IndexSpace::try_new(1).unwrap().try_zero().unwrap(),
            context.lift(&base.coefficient_fixture("1/(x+1)")).unwrap(),
            RelationLimits::default(),
        )
        .unwrap();
    let merge_limited = RelationLimits {
        identity_conditions: IdentityConditionLimits { max_sources: 3 },
        ..RelationLimits::default()
    };
    let before_merge_rejection = borrowed_condition_deep_clone_counts();
    assert_eq!(
        merge_target
            .add_scaled(&context, &source, &context.one(), merge_limited)
            .unwrap_err(),
        ParametricRelationError::IdentityCondition(IdentityConditionError::ResourceLimit {
            resource: "identity condition sources",
            requested: 4,
            limit: 3,
        })
    );
    assert_eq!(
        borrowed_condition_deep_clone_counts(),
        before_merge_rejection,
        "target merge admission must precede all borrowed condition clones"
    );
    let merge_target = merge_target.finish();
    assert_eq!(merge_target.terms().len(), 1);
    assert_eq!(merge_target.nonzero_conditions().len(), 1);
    assert_eq!(merge_target.nonzero_conditions()[0].sources().len(), 2);
}

#[test]
fn real_relation_condition_source_limit_precedes_polynomial_translation() {
    let (context, condition) = actual_input_denominator_condition("translation-source-order");
    let arithmetic_limits = IndexedAlgebraLimits {
        exact_algebra: ExactAlgebraLimits {
            max_polynomial_terms: 0,
            ..ExactAlgebraLimits::default()
        },
        ..IndexedAlgebraLimits::default()
    };
    assert!(matches!(
        condition.translated(
            &context,
            &[1],
            arithmetic_limits,
            IdentityConditionLimits { max_sources: 2 },
        ),
        Err(IdentityConditionError::ResourceLimit {
            resource: "identity condition sources",
            requested: 3,
            limit: 2,
        })
    ));
}

#[test]
fn real_relation_condition_index_arity_precedes_source_preflight() {
    let (context, condition) = actual_input_denominator_condition("translation-arity-order");
    assert!(matches!(
        condition.translated(
            &context,
            &[],
            IndexedAlgebraLimits::default(),
            IdentityConditionLimits { max_sources: 2 },
        ),
        Err(IdentityConditionError::Coefficient(
            IndexedAlgebraError::WrongIndexArity {
                expected: 1,
                actual: 0,
            }
        ))
    ));
}
