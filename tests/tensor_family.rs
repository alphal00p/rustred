use rustred::{
    CoefficientContext, Denominator, ExactRational, FamilyError, IndexedVector, Integral,
    LoopVector, LorentzIndex, Metric, MetricPairing, ScalarProduct, ScalarProductMonomial,
    TensorFamilyReducer, TensorMonomial, VacuumFamily, VacuumTensorProjector,
};

fn equal_mass_two_loop_vacuum() -> Result<VacuumFamily, FamilyError> {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients
        .parameter("m2")
        .ok_or_else(|| FamilyError::UnknownCoefficientParameter("m2".to_owned()))?;
    let momentum =
        |left: i64, right: i64| vec![ExactRational::from(left), ExactRational::from(right)];
    VacuumFamily::new(
        "tensor-family-local-equal-mass-two-loop-vacuum",
        2,
        coefficients,
        "d",
        vec![
            Denominator::propagator(momentum(1, 0), mass.clone()),
            Denominator::propagator(momentum(0, 1), mass.clone()),
            Denominator::propagator(momentum(1, 1), mass),
        ],
        vec![vec![1, 0, 2], vec![1, 2, 0]],
    )
}

fn vector(loop_id: u16, index_id: u32) -> IndexedVector {
    IndexedVector::new(LoopVector::new(loop_id), LorentzIndex::new(index_id))
}

// Keep all Symbolica-backed checks on one test worker.
#[test]
fn tensor_numerators_lower_to_family_integrals() {
    let family = equal_mass_two_loop_vacuum().unwrap();
    let coefficients = family.coefficients();
    let mut projector =
        VacuumTensorProjector::with_dimension(coefficients, family.dimension().clone());
    let lowering = TensorFamilyReducer::new(&family);
    let base = Integral::from([1, 1, 1]);

    // k1(mu) k1(nu) / (D1 D2 D3)
    //   = g(mu,nu)/d [ I(0,1,1) - m2 I(1,1,1) ].
    let projected = projector
        .reduce(&TensorMonomial::new([vector(0, 10), vector(0, 11)]))
        .unwrap();
    let lowered = lowering.lower(&base, &projected).unwrap();
    let metric = MetricPairing::new([Metric::new(LorentzIndex::new(10), LorentzIndex::new(11))]);
    let scalar = lowered.coefficient(&metric).unwrap();
    assert_eq!(
        scalar.coefficient(&Integral::from([0, 1, 1])),
        Some(&coefficients.parse("1/d").unwrap())
    );
    assert_eq!(
        scalar.coefficient(&base),
        Some(&coefficients.parse("-m2/d").unwrap())
    );

    // k1.k2 = (D3-D1-D2+m2)/2, checked before any symmetry is applied.
    let projected = projector
        .reduce(&TensorMonomial::new([vector(0, 20), vector(1, 21)]))
        .unwrap();
    let lowered = lowering.lower(&base, &projected).unwrap();
    let metric = MetricPairing::new([Metric::new(LorentzIndex::new(20), LorentzIndex::new(21))]);
    let scalar = lowered.coefficient(&metric).unwrap();
    for (integral, expected) in [
        ([1, 1, 0], "1/(2*d)"),
        ([0, 1, 1], "-1/(2*d)"),
        ([1, 0, 1], "-1/(2*d)"),
        ([1, 1, 1], "m2/(2*d)"),
    ] {
        assert_eq!(
            scalar.coefficient(&Integral::from(integral)),
            Some(&coefficients.parse(expected).unwrap())
        );
    }

    // Generic generated-provider composition, total replay, and the two-loop
    // master-basis assertions live in `vakint_two_loop_tensor_ibp_oracle.rs`.
    // The old authored boundary helper is intentionally unavailable on the
    // default production surface.

    // Existing scalar-product numerator powers are expanded as a polynomial,
    // not dropped by the rank-zero projector.
    let squared_norm = ScalarProductMonomial::from_factors([(
        ScalarProduct::new(LoopVector::new(0), LoopVector::new(0)),
        2,
    )]);
    let projected = projector
        .reduce(&TensorMonomial::from_parts([], [], squared_norm))
        .unwrap();
    assert!(matches!(
        TensorFamilyReducer::new(&family)
            .with_max_expansion_operations(1)
            .lower(&base, &projected),
        Err(rustred::TensorFamilyError::OperationLimit { limit: 1, .. })
    ));
    let lowered = lowering.lower(&base, &projected).unwrap();
    let scalar = lowered.coefficient(&MetricPairing::empty()).unwrap();
    for (integral, expected) in [([-1, 1, 1], "1"), ([0, 1, 1], "-2*m2"), ([1, 1, 1], "m2^2")] {
        assert_eq!(
            scalar.coefficient(&Integral::from(integral)),
            Some(&coefficients.parse(expected).unwrap())
        );
    }

    // Rank-four projection and denominator lowering remain covered here. The
    // generated-provider end-to-end reduction is covered by the test cited
    // above, without restoring a topology-specific production method.
    let rank_four = projector
        .reduce(&TensorMonomial::new([
            vector(0, 100),
            vector(0, 101),
            vector(1, 102),
            vector(1, 103),
        ]))
        .unwrap();
    let lowered = lowering.lower(&base, &rank_four).unwrap();
    assert!(!lowered.is_zero());
    assert!(
        lowered
            .structures()
            .values()
            .all(|combination| !combination.is_zero())
    );
}
