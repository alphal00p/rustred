use crate::algebra::{CoefficientContext, ExactAlgebraError};

use super::super::{IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficientContext};

#[test]
fn base_field_may_be_q_and_indices_remain_distinct() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let context = IndexedCoefficientContext::try_new(&base, "empty-base", 2).unwrap();
    assert_eq!(base.parameter_names(), &[] as &[String]);
    assert_eq!(context.index_count(), 2);
    assert!(context.contains(&context.index(0).unwrap()));
}

#[test]
fn context_construction_is_fallible_and_preserves_semantic_error_ordering() {
    let base = CoefficientContext::new(["x"]);
    assert!(matches!(
        IndexedCoefficientContext::try_new(&base, "", 0),
        Err(IndexedAlgebraError::EmptyIndexSpace)
    ));
    assert!(matches!(
        IndexedCoefficientContext::try_new(&base, "", 1),
        Err(IndexedAlgebraError::InvalidScope)
    ));
    IndexedCoefficientContext::try_new(&base, "exact-minimum", 1).unwrap();

    assert!(matches!(
        IndexedCoefficientContext::try_new(&base, "count-overflow", usize::MAX),
        Err(IndexedAlgebraError::ResourceCountOverflow {
            resource: "indexed coefficient variables",
        })
    ));

    let rational = CoefficientContext::new(Vec::<String>::new());
    assert!(matches!(
        IndexedCoefficientContext::try_new(&rational, "allocation-failure", usize::MAX),
        Err(IndexedAlgebraError::AllocationFailure {
            resource: "indexed coefficient index variables",
            requested: usize::MAX,
        })
    ));
}

#[test]
fn rejects_foreign_maps_before_symbolica_can_unify_them() {
    let base = CoefficientContext::new(["d"]);
    let foreign = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "strict-map", 1).unwrap();
    assert!(matches!(
        context.lift(&foreign.one()),
        Err(IndexedAlgebraError::WrongContext)
    ));
    assert!(matches!(
        context.translate(&context.one(), &[], IndexedAlgebraLimits::default()),
        Err(IndexedAlgebraError::WrongIndexArity { .. })
    ));
}

#[test]
fn indexed_authentication_rejects_malformed_layout_before_arithmetic() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "malformed", 1).unwrap();
    let mut malformed = context.one();
    malformed.raw.numerator.exponents.push(0);

    assert!(!context.contains(&malformed));
    assert!(matches!(
        context.add(&malformed, &context.one()),
        Err(IndexedAlgebraError::ExactAlgebra(
            ExactAlgebraError::MalformedExponentLayout { .. }
        ))
    ));
}
