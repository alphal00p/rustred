use std::collections::BTreeSet;
use std::sync::Arc;

use crate::algebra::{
    CoefficientContext, ExactAlgebraLimits, IndexedAlgebraError, IndexedAlgebraLimits,
    IndexedCoefficientContext,
};
use crate::identity::condition::{
    IdentityConditionError, IdentityConditionLimits, IdentityConditionSource,
};
use crate::identity::row::RowId;

use super::super::{IndexSpace, ParametricRelation, ParametricRelationError, RelationLimits};
use super::support::actual_input_denominator_condition;

#[test]
fn repeated_rational_term_merges_real_denominator_sources() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "condition-merge", 1).unwrap();
    let row_id = RowId::Derived {
        label: Arc::from("condition-source"),
    };
    let mut relation = ParametricRelation::new("family", row_id.clone(), &context);
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
    relation
        .add_term_with_limits(
            &context,
            shift.clone(),
            reciprocal.clone(),
            RelationLimits::default(),
        )
        .unwrap();
    relation
        .add_term_with_limits(&context, shift, reciprocal, RelationLimits::default())
        .unwrap();

    assert_eq!(relation.nonzero_conditions().len(), 1);
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
    let mut relation = ParametricRelation::new("family", row_id, &context);
    let limits = RelationLimits {
        identity_conditions: IdentityConditionLimits { max_sources: 1 },
        ..RelationLimits::default()
    };
    assert!(matches!(
        relation.add_term_with_limits(
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
    assert!(relation.terms().is_empty());
    assert!(relation.nonzero_conditions().is_empty());
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
