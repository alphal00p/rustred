use std::sync::{Arc, Barrier};

use symbolica::{
    atom::{NamespacedSymbol, SymbolAttribute, SymbolBuilder},
    prelude::{AtomCore, Integer},
};

use crate::algebra::{CoefficientContext, ExactAlgebraError, ExactAlgebraLimits};

use super::super::context::authenticate_index_symbol;
use super::super::scope::{
    aggregate_qualified_index_symbol_bytes, base_context_fingerprint, indexed_context_fingerprint,
};
use super::super::{
    IndexedAlgebraError, IndexedAlgebraLimits, IndexedCoefficientContext, IndexedContextLimits,
};

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
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "indexed coefficient index variables",
            requested: usize::MAX,
            limit: 4_096,
        })
    ));
    assert!(matches!(
        IndexedCoefficientContext::try_new_with_limits(
            &rational,
            "allocation-failure",
            usize::MAX,
            IndexedContextLimits {
                max_index_variables: usize::MAX,
                max_fingerprint_bytes: usize::MAX,
                max_native_symbol_name_bytes: usize::MAX,
            },
        ),
        Err(IndexedAlgebraError::AllocationFailure {
            resource: "indexed coefficient index variables",
            requested: usize::MAX,
        })
    ));
}

#[test]
fn construction_limits_accept_exact_bounds_and_reject_one_below() {
    let base = CoefficientContext::new(["x"]);
    let scope = "bounded-context";
    let index_count = 2;
    let base_fingerprint = base_context_fingerprint(&base).unwrap();
    let fingerprint_bytes = indexed_context_fingerprint(&base_fingerprint, scope, index_count)
        .unwrap()
        .len();
    let native_name_bytes = aggregate_qualified_index_symbol_bytes(index_count).unwrap();
    let exact = IndexedContextLimits {
        max_index_variables: index_count,
        max_fingerprint_bytes: fingerprint_bytes,
        max_native_symbol_name_bytes: native_name_bytes,
    };

    assert_eq!(
        IndexedContextLimits::default().max_index_variables,
        crate::family::IntegralFamilyLimits::default().max_scalar_products
    );
    IndexedCoefficientContext::try_new_with_limits(&base, scope, index_count, exact).unwrap();

    assert!(matches!(
        IndexedCoefficientContext::try_new_with_limits(
            &base,
            scope,
            index_count,
            IndexedContextLimits {
                max_index_variables: index_count - 1,
                ..exact
            },
        ),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "indexed coefficient index variables",
            requested: 2,
            limit: 1,
        })
    ));
    assert!(matches!(
        IndexedCoefficientContext::try_new_with_limits(
            &base,
            scope,
            index_count,
            IndexedContextLimits {
                max_fingerprint_bytes: fingerprint_bytes - 1,
                ..exact
            },
        ),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "indexed coefficient context fingerprint bytes",
            requested,
            limit,
        }) if requested == fingerprint_bytes && limit == fingerprint_bytes - 1
    ));
    assert!(matches!(
        IndexedCoefficientContext::try_new_with_limits(
            &base,
            scope,
            index_count,
            IndexedContextLimits {
                max_native_symbol_name_bytes: native_name_bytes - 1,
                ..exact
            },
        ),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "indexed coefficient aggregate Symbolica name bytes",
            requested,
            limit,
        }) if requested == native_name_bytes && limit == native_name_bytes - 1
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
        context.validate_with_limits(&malformed, ExactAlgebraLimits::default()),
        Err(IndexedAlgebraError::ExactAlgebra(
            ExactAlgebraError::MalformedExponentLayout { .. }
        ))
    ));
}

#[test]
fn separately_constructed_identical_scopes_use_exact_fallback_and_interoperate() {
    let first_base = CoefficientContext::new(["d", "m2"]);
    let second_base = CoefficientContext::new(["d", "m2"]);
    let first = IndexedCoefficientContext::try_new(&first_base, "stable-shared-scope", 2).unwrap();
    let second =
        IndexedCoefficientContext::try_new(&second_base, "stable-shared-scope", 2).unwrap();
    let segmented = IndexedCoefficientContext::try_new_with_scope_segments(
        &second_base,
        &["stable-", "shared-", "scope"],
        2,
    )
    .unwrap();

    let first_owner = first.fingerprint_owner();
    let second_owner = second.fingerprint_owner();
    let segmented_owner = segmented.fingerprint_owner();
    assert_eq!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.fingerprint(), segmented.fingerprint());
    assert!(!Arc::ptr_eq(&first_owner, &second_owner));
    assert!(!Arc::ptr_eq(&first_owner, &segmented_owner));
    assert!(second.owns_fingerprint(&first_owner));
    assert!(segmented.owns_fingerprint(&first_owner));
    assert_eq!(first.index_variables, second.index_variables);
    assert_eq!(first.index_variables, segmented.index_variables);

    let first_index = first.index(0).unwrap();
    assert_eq!(
        first_index.to_expression().to_canonical_string(),
        second
            .index(0)
            .unwrap()
            .to_expression()
            .to_canonical_string()
    );
    assert!(second.contains(&first_index));
    assert!(second.add(&first_index, &second.one()).is_ok());
    assert!(segmented.add(&first_index, &segmented.one()).is_ok());
}

#[test]
fn different_exact_scopes_share_native_symbols_and_never_interoperate() {
    let base = CoefficientContext::new(["d"]);
    let first = IndexedCoefficientContext::try_new(&base, "separate-scope-a", 1).unwrap();
    let second = IndexedCoefficientContext::try_new(&base, "separate-scope-b", 1).unwrap();
    let first_index = first.index(0).unwrap();

    assert_ne!(first.fingerprint(), second.fingerprint());
    assert_eq!(
        first_index.to_expression().to_canonical_string(),
        second
            .index(0)
            .unwrap()
            .to_expression()
            .to_canonical_string()
    );
    assert!(!second.contains(&first_index));
    assert!(matches!(
        second.add(&first_index, &second.one()),
        Err(IndexedAlgebraError::WrongContext)
    ));
}

#[test]
fn different_base_contexts_share_index_symbols_without_aliasing() {
    let first_base = CoefficientContext::new(["d"]);
    let second_base = CoefficientContext::new(["x"]);
    let first = IndexedCoefficientContext::try_new(&first_base, "same-scope", 2).unwrap();
    let second = IndexedCoefficientContext::try_new(&second_base, "same-scope", 2).unwrap();
    let first_index = first.index(1).unwrap();

    assert_ne!(first.fingerprint(), second.fingerprint());
    assert_eq!(first.index_variables, second.index_variables);
    assert_eq!(
        first_index.to_expression().to_canonical_string(),
        second
            .index(1)
            .unwrap()
            .to_expression()
            .to_canonical_string()
    );
    assert!(!second.contains(&first_index));
    assert!(matches!(
        second.add(&first_index, &second.one()),
        Err(IndexedAlgebraError::WrongContext)
    ));
}

#[test]
fn private_index_namespace_is_disjoint_from_every_base_label() {
    let base = CoefficientContext::new(["rustred_indexed_coefficient_v1::n0"]);
    let context = IndexedCoefficientContext::try_new(&base, "namespace-disjoint", 1).unwrap();
    let parameter = base
        .parameter("rustred_indexed_coefficient_v1::n0")
        .unwrap();
    let index = context.index(0).unwrap();

    assert_ne!(
        parameter.to_expression().to_canonical_string(),
        index.to_expression().to_canonical_string()
    );
    assert!(context.lift(&parameter).is_ok());
}

#[test]
fn unsafe_process_global_index_symbol_registration_is_rejected() {
    let qualified = "rustred_indexed_coefficient_collision_test::n0";
    let namespaced = NamespacedSymbol::try_parse(qualified).unwrap();
    let symbol = SymbolBuilder::new(namespaced)
        .with_attributes(&[SymbolAttribute::Symmetric])
        .build()
        .unwrap();

    assert!(matches!(
        authenticate_index_symbol(symbol, qualified, 17),
        Err(IndexedAlgebraError::IndexSymbolCollision { position: 17 })
    ));
}

#[test]
fn concurrent_context_construction_reuses_deterministic_positional_symbols() {
    const THREADS: usize = 12;
    const INDICES: usize = 4;
    let barrier = Arc::new(Barrier::new(THREADS));
    let handles = (0..THREADS)
        .map(|thread| {
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let base = CoefficientContext::new(["d"]);
                barrier.wait();
                let context = IndexedCoefficientContext::try_new(
                    &base,
                    &format!("concurrent-scope-{thread}"),
                    INDICES,
                )
                .unwrap();
                let names = (0..INDICES)
                    .map(|position| {
                        context
                            .index(position)
                            .unwrap()
                            .to_expression()
                            .to_canonical_string()
                    })
                    .collect::<Vec<_>>();
                (context.fingerprint().to_owned(), names)
            })
        })
        .collect::<Vec<_>>();
    let contexts = handles
        .into_iter()
        .map(|handle| handle.join().unwrap())
        .collect::<Vec<_>>();

    for (thread, (fingerprint, names)) in contexts.iter().enumerate() {
        assert!(fingerprint.contains(&format!("concurrent-scope-{thread}")));
        assert_eq!(names, &contexts[0].1);
    }
}

#[test]
fn public_arithmetic_admits_each_operand_and_authenticates_the_native_result() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "scan-census", 2).unwrap();
    let left = context.index(0).unwrap();
    let right = context.index(1).unwrap();
    let before = context.authentication_scan_counts();

    let sum = context.add(&left, &right).unwrap();
    let after_sum = context.authentication_scan_counts();
    assert_eq!(
        (after_sum.0 - before.0, after_sum.1 - before.1),
        (2, 1),
        "each public operand and the native result must be admitted under the current limits"
    );

    context
        .denominator_condition_with_limits(&sum, ExactAlgebraLimits::default())
        .unwrap();
    let after_denominator = context.authentication_scan_counts();
    assert_eq!(
        (
            after_denominator.0 - after_sum.0,
            after_denominator.1 - after_sum.1,
        ),
        (1, 0),
        "denominator extraction is one ingress scan, not a scan plus a result recheck"
    );
}

#[test]
fn checked_division_and_polynomial_extraction_stay_on_the_indexed_map() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "division-seam", 1).unwrap();
    let index = context.index(0).unwrap();
    let x = context.lift(&base.parameter("x").unwrap()).unwrap();
    let numerator = context.add(&index, &context.one()).unwrap();
    let denominator = context.mul(&x, &index).unwrap();
    let quotient = context.div(&numerator, &denominator).unwrap();

    assert!(context.contains(&quotient));
    assert_eq!(
        context
            .numerator_condition_with_limits(&quotient, ExactAlgebraLimits::default())
            .unwrap()
            .raw(),
        &quotient.raw().numerator
    );
    assert_eq!(
        context
            .denominator_condition_with_limits(&quotient, ExactAlgebraLimits::default())
            .unwrap()
            .raw(),
        &quotient.raw().denominator
    );
    assert!(matches!(
        context.div(&context.one(), &context.zero()),
        Err(IndexedAlgebraError::ExactAlgebra(
            ExactAlgebraError::DivisionByZero
        ))
    ));
}

#[test]
fn native_result_ingress_authenticates_once_before_sealing() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "native-result-seam", 1).unwrap();
    let raw = context.index(0).unwrap().raw().clone();
    let before = context.authentication_scan_counts();
    let admitted = context
        .admit_native_result_with_limits(raw, ExactAlgebraLimits::default())
        .unwrap();
    let after = context.authentication_scan_counts();

    assert!(context.contains(&admitted));
    assert_eq!(after.0 - before.0, 0);
    assert_eq!(after.1 - before.1, 1);
}

#[test]
fn primitive_guard_serialization_rejects_large_integers_before_formatting() {
    let base = CoefficientContext::new(["x"]);
    let context = IndexedCoefficientContext::try_new(&base, "guard-byte-preflight", 1).unwrap();
    let mut raw = context.one().raw().clone();
    raw.numerator.coefficients[0] = Integer::from(1) << 2_000_u32;
    let coefficient = context
        .admit_native_result_with_limits(raw, ExactAlgebraLimits::default())
        .unwrap();
    let polynomial = context
        .numerator_condition_with_limits(&coefficient, ExactAlgebraLimits::default())
        .unwrap();
    assert!(matches!(
        context.primitive_guard_associate_with_limits(
            &polynomial,
            ExactAlgebraLimits::default(),
            16,
        ),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard polynomial serialized payload bytes",
            requested,
            limit: 16,
        }) if requested > 16
    ));
}

#[test]
fn primitive_guard_serialization_bounds_the_exponent_clone_before_allocation() {
    let base = CoefficientContext::new(["x"]);
    let context =
        IndexedCoefficientContext::try_new(&base, "guard-exponent-preflight", 64).unwrap();
    let polynomial = context
        .numerator_condition_with_limits(&context.index(0).unwrap(), ExactAlgebraLimits::default())
        .unwrap();
    assert!(matches!(
        context.primitive_guard_associate_with_limits(
            &polynomial,
            ExactAlgebraLimits::default(),
            16,
        ),
        Err(IndexedAlgebraError::ResourceLimit {
            resource: "guard polynomial serialized payload bytes",
            requested,
            limit: 16,
        }) if requested > 16
    ));
}

#[test]
fn native_symbol_name_size_is_independent_of_long_semantic_scope() {
    let base = CoefficientContext::new(Vec::<String>::new());
    let short = IndexedCoefficientContext::try_new(&base, "short-scope", 1).unwrap();
    let long_scope = "scope-component/".repeat(16_384);
    let long = IndexedCoefficientContext::try_new(&base, &long_scope, 1).unwrap();

    let short_name = short
        .index(0)
        .unwrap()
        .to_expression()
        .to_canonical_string();
    let long_name = long.index(0).unwrap().to_expression().to_canonical_string();
    assert_eq!(short_name, long_name);
    assert_eq!(short_name.len(), long_name.len());
    assert!(
        long_name.len() < 128,
        "native name was not compact: {long_name}"
    );
    assert!(long.fingerprint().len() > long_scope.len());
    assert!(!long_name.contains("scope-component"));
}
