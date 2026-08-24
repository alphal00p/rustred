use rustred::{
    IndexedVector, LoopVector, LorentzIndex, Metric, ScalarProduct, ScalarProductMonomial,
    TensorConstructionLimits, TensorError, TensorMonomial,
};

fn vector(loop_id: u16, index: u32) -> IndexedVector {
    IndexedVector::new(LoopVector::new(loop_id), LorentzIndex::new(index))
}

fn scalar(left: u16, right: u16) -> ScalarProduct {
    ScalarProduct::new(LoopVector::new(left), LoopVector::new(right))
}

#[test]
fn checked_tensor_constructors_fail_before_retaining_over_limit_entries() {
    let vector_limits = TensorConstructionLimits {
        max_vectors: 1,
        ..TensorConstructionLimits::default()
    };
    assert!(matches!(
        TensorMonomial::try_new_with_limits([vector(0, 0), vector(0, 1)], vector_limits),
        Err(TensorError::ResourceLimit {
            resource: "tensor constructor vectors",
            requested: 2,
            limit: 1,
        })
    ));

    let endpoint_limits = TensorConstructionLimits {
        max_index_endpoints: 1,
        ..TensorConstructionLimits::default()
    };
    assert!(matches!(
        TensorMonomial::try_from_parts_with_limits(
            [],
            [Metric::new(LorentzIndex::new(10), LorentzIndex::new(11))],
            ScalarProductMonomial::one(),
            endpoint_limits,
        ),
        Err(TensorError::ResourceLimit {
            resource: "tensor constructor index endpoints",
            requested: 2,
            limit: 1,
        })
    ));

    let factor_limits = TensorConstructionLimits {
        max_scalar_product_factor_entries: 1,
        ..TensorConstructionLimits::default()
    };
    assert!(matches!(
        ScalarProductMonomial::try_from_factors_with_limits(
            [(scalar(0, 0), 1), (scalar(0, 0), 1)],
            factor_limits,
        ),
        Err(TensorError::ResourceLimit {
            resource: "tensor scalar-product constructor entries",
            requested: 2,
            limit: 1,
        })
    ));

    let distinct_limits = TensorConstructionLimits {
        max_distinct_scalar_products: 1,
        ..TensorConstructionLimits::default()
    };
    assert!(matches!(
        ScalarProductMonomial::try_from_factors_with_limits(
            [(scalar(0, 0), 1), (scalar(0, 1), 1)],
            distinct_limits,
        ),
        Err(TensorError::ResourceLimit {
            resource: "tensor distinct scalar products",
            requested: 2,
            limit: 1,
        })
    ));

    let degree_limits = TensorConstructionLimits {
        max_scalar_product_degree: 1,
        ..TensorConstructionLimits::default()
    };
    assert!(matches!(
        ScalarProductMonomial::try_from_factors_with_limits([(scalar(0, 0), 2)], degree_limits),
        Err(TensorError::ScalarProductDegreeLimit {
            requested: 2,
            limit: 1,
        })
    ));
}

#[test]
fn checked_scalar_product_mutation_is_transactional_on_exponent_overflow() {
    let limits = TensorConstructionLimits {
        max_scalar_product_degree: u64::MAX,
        ..TensorConstructionLimits::default()
    };
    let factor = scalar(0, 0);
    let mut monomial = ScalarProductMonomial::one();
    monomial
        .try_multiply_power_with_limits(factor, u32::MAX, limits)
        .unwrap();

    assert!(matches!(
        monomial.try_multiply_power_with_limits(factor, 1, limits),
        Err(TensorError::ScalarProductExponentOverflow { scalar_product })
            if scalar_product == factor
    ));
    assert_eq!(monomial.exponent(factor), u32::MAX);

    let mut left =
        ScalarProductMonomial::try_from_factors_with_limits([(factor, u32::MAX)], limits).unwrap();
    let before = left.clone();
    let right = ScalarProductMonomial::try_from_factors_with_limits(
        [(scalar(0, 1), 1), (factor, 1)],
        limits,
    )
    .unwrap();
    assert!(matches!(
        left.try_multiply_monomial_with_limits(&right, limits),
        Err(TensorError::ScalarProductExponentOverflow { scalar_product })
            if scalar_product == factor
    ));
    assert_eq!(left, before);
}
