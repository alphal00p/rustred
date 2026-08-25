use rustred::{
    AffineDenominator, Coefficient, CoefficientContext, ConcreteIntegralKey, CutConstraint,
    ExactAlgebraLimits, IntegralFamily, InternalSymmetrySearchCompletion,
    InternalSymmetrySearchError, InternalSymmetrySearchLimits, SectorPattern, SectorRestrictions,
    discover_bounded_vacuum_internal_symmetries,
};

fn affine(
    constant: Coefficient,
    coefficients: impl IntoIterator<Item = Coefficient>,
) -> AffineDenominator {
    AffineDenominator::new(constant, coefficients.into_iter().collect())
}

fn equal_mass_two_loop_vacuum() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let m2 = coefficients.parameter("m2").unwrap();
    IntegralFamily::new(
        "discovery-equal-mass-two-loop-vacuum",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                m2.clone(),
                [coefficients.one(), coefficients.zero(), coefficients.zero()],
            ),
            affine(
                m2.clone(),
                [coefficients.zero(), coefficients.zero(), coefficients.one()],
            ),
            affine(
                m2,
                [
                    coefficients.one(),
                    coefficients.integer(-2),
                    coefficients.one(),
                ],
            ),
        ],
        Vec::new(),
        vec![coefficients.zero(); 3],
    )
    .unwrap()
}

fn shifted_equal_mass_two_loop_vacuum() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2", "nu0", "nu1", "nu2"]);
    let m2 = coefficients.parameter("m2").unwrap();
    IntegralFamily::new(
        "discovery-shifted-equal-mass-two-loop-vacuum",
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                m2.clone(),
                [coefficients.one(), coefficients.zero(), coefficients.zero()],
            ),
            affine(
                m2.clone(),
                [coefficients.zero(), coefficients.zero(), coefficients.one()],
            ),
            affine(
                m2,
                [
                    coefficients.one(),
                    coefficients.integer(-2),
                    coefficients.one(),
                ],
            ),
        ],
        Vec::new(),
        vec![
            coefficients.parameter("nu0").unwrap(),
            coefficients.parameter("nu1").unwrap(),
            coefficients.parameter("nu2").unwrap(),
        ],
    )
    .unwrap()
}

fn asymmetric_rational_two_loop_vacuum() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m0", "m1", "m2", "x"]);
    let x = coefficients.parameter("x").unwrap();
    let x_plus_one = coefficients
        .try_add(&x, &coefficients.one(), ExactAlgebraLimits::default())
        .unwrap();
    let rho = coefficients
        .try_div(&x, &x_plus_one, ExactAlgebraLimits::default())
        .unwrap();
    IntegralFamily::new(
        "discovery-asymmetric-rational-two-loop-vacuum",
        vec!["q0".into(), "q1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                coefficients.parameter("m0").unwrap(),
                [coefficients.one(), coefficients.zero(), coefficients.zero()],
            ),
            affine(
                coefficients.parameter("m1").unwrap(),
                [coefficients.zero(), coefficients.zero(), coefficients.one()],
            ),
            affine(
                coefficients.parameter("m2").unwrap(),
                [
                    rho.clone(),
                    coefficients
                        .try_mul(
                            &coefficients.integer(-2),
                            &rho,
                            ExactAlgebraLimits::default(),
                        )
                        .unwrap(),
                    rho,
                ],
            ),
        ],
        Vec::new(),
        vec![coefficients.zero(); 3],
    )
    .unwrap()
}

#[test]
fn discovers_all_six_equal_mass_two_loop_denominator_permutations_and_replays() {
    let family = equal_mass_two_loop_vacuum();
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let limits = InternalSymmetrySearchLimits::default();
    let report =
        discover_bounded_vacuum_internal_symmetries(&family, &restrictions, limits).unwrap();

    assert!(report.completion().is_exhaustive_within_bounds());
    assert_eq!(report.stats().enumerated_matrices(), 81);
    assert_eq!(report.stats().affine_candidates_rejected(), 0);
    assert_eq!(
        report.stats().verifier_calls(),
        report.stats().unimodular_candidates()
    );
    assert_eq!(
        report.stats().verifier_calls(),
        report.stats().retained_symmetries()
            + report.stats().duplicate_row_actions()
            + report.stats().incompatible_integral_maps()
    );
    assert_eq!(report.stats().retained_symmetries(), 6);
    assert!(report.stats().retained_certificate_entries() > 0);
    assert!(report.stats().retained_certificate_bytes() > 0);
    assert!(
        report.stats().retained_certificate_entries() <= limits.max_retained_certificate_entries
    );
    assert!(report.stats().retained_certificate_bytes() <= limits.max_retained_certificate_bytes);
    let actual = report
        .symmetries()
        .iter()
        .map(|symmetry| symmetry.denominator_permutation().to_vec())
        .collect::<Vec<_>>();
    assert_eq!(
        actual,
        vec![
            vec![0, 1, 2],
            vec![0, 2, 1],
            vec![1, 0, 2],
            vec![1, 2, 0],
            vec![2, 0, 1],
            vec![2, 1, 0],
        ]
    );
    assert!(report.stats().duplicate_row_actions() > 0);
    for symmetry in report.symmetries() {
        assert_eq!(
            symmetry.affine_map().momentum().loop_external().columns(),
            0
        );
        symmetry
            .replay(&family, &restrictions, Default::default())
            .unwrap();
    }

    let cycle = report
        .symmetries()
        .iter()
        .find(|symmetry| symmetry.denominator_permutation() == [1, 2, 0])
        .unwrap();
    let source = ConcreteIntegralKey::try_new([3, -2, 7]).unwrap();
    let target = cycle.transport_source_key(&source).unwrap();
    assert_eq!(target.powers(), &[7, 3, -2]);
    cycle.replay_key_transport(&source, &target).unwrap();
    assert!(
        cycle
            .restrictions_fingerprint()
            .contains("cuts=000|pattern=***")
    );
}

#[test]
fn cuts_and_formal_power_shifts_are_part_of_compilation_not_affine_verification() {
    let family = equal_mass_two_loop_vacuum();
    let cut_restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(3, [0]).unwrap(),
        SectorPattern::any(3).unwrap(),
    )
    .unwrap();
    let cut_report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &cut_restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(cut_report.completion().is_exhaustive_within_bounds());
    assert_eq!(
        cut_report
            .symmetries()
            .iter()
            .map(|symmetry| symmetry.denominator_permutation().to_vec())
            .collect::<Vec<_>>(),
        vec![vec![0, 1, 2], vec![0, 2, 1]]
    );

    let shifted = shifted_equal_mass_two_loop_vacuum();
    let unrestricted = SectorRestrictions::unrestricted(3).unwrap();
    let shifted_report = discover_bounded_vacuum_internal_symmetries(
        &shifted,
        &unrestricted,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(shifted_report.completion().is_exhaustive_within_bounds());
    assert_eq!(shifted_report.symmetries().len(), 1);
    assert_eq!(
        shifted_report.symmetries()[0].denominator_permutation(),
        &[0, 1, 2]
    );
}

#[test]
fn asymmetric_sector_pattern_is_preserved_by_compiled_integral_symmetries() {
    let family = equal_mass_two_loop_vacuum();
    let restrictions = SectorRestrictions::try_new(
        CutConstraint::none(3).unwrap(),
        SectorPattern::try_from_string("1*0").unwrap(),
    )
    .unwrap();
    let report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();

    assert!(report.completion().is_exhaustive_within_bounds());
    assert_eq!(report.symmetries().len(), 1);
    assert_eq!(report.symmetries()[0].denominator_permutation(), &[0, 1, 2]);
    assert!(report.stats().incompatible_integral_maps() > 0);
    report.symmetries()[0]
        .replay(&family, &restrictions, Default::default())
        .unwrap();
}

#[test]
fn asymmetric_rational_family_does_not_forge_denominator_permutations() {
    let family = asymmetric_rational_two_loop_vacuum();
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let report = discover_bounded_vacuum_internal_symmetries(
        &family,
        &restrictions,
        InternalSymmetrySearchLimits::default(),
    )
    .unwrap();
    assert!(report.completion().is_exhaustive_within_bounds());
    assert_eq!(report.symmetries().len(), 1);
    assert_eq!(report.symmetries()[0].denominator_permutation(), &[0, 1, 2]);
    report.symmetries()[0]
        .replay(&family, &restrictions, Default::default())
        .unwrap();
}

#[test]
fn truncated_search_is_explicitly_resource_limited_not_a_negative_proof() {
    let family = equal_mass_two_loop_vacuum();
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();
    let mut limits = InternalSymmetrySearchLimits::default();
    limits.max_enumerated_matrices = 1;
    let report =
        discover_bounded_vacuum_internal_symmetries(&family, &restrictions, limits).unwrap();
    assert!(matches!(
        report.completion(),
        InternalSymmetrySearchCompletion::ResourceLimited {
            resource: "enumerated loop maps",
            requested: 2,
            limit: 1,
            ..
        }
    ));
    assert_eq!(report.stats().enumerated_matrices(), 1);
    assert!(report.symmetries().is_empty());
    assert!(
        report
            .completion()
            .domain_fingerprint()
            .contains("integer[-1,1]")
    );
}

#[test]
fn zero_execution_limits_are_typed_partial_reports_and_never_panics() {
    let family = equal_mass_two_loop_vacuum();
    let restrictions = SectorRestrictions::unrestricted(3).unwrap();

    let assert_limited = |limits: InternalSymmetrySearchLimits| {
        let report =
            discover_bounded_vacuum_internal_symmetries(&family, &restrictions, limits).unwrap();
        assert!(matches!(
            report.completion(),
            InternalSymmetrySearchCompletion::ResourceLimited { .. }
        ));
    };

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.max_loop_map_entries = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.max_enumerated_matrices = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.max_integer_determinant_operations = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.max_integer_bits = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.max_verifier_calls = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.max_retained_symmetries = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.max_retained_certificate_entries = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.max_retained_certificate_bytes = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.max_matrix_entries = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.max_exact_operations = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.max_symbolica_single_matrix_entries = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.max_symbolica_live_matrix_entries = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.max_symbolica_input_retained_bytes = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.max_symbolica_output_retained_bytes = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.max_guard_polynomials = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.max_guard_origins = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.exact_algebra.max_polynomial_terms = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.exact_algebra.max_term_operations = 0;
    assert_limited(limits);

    let mut limits = InternalSymmetrySearchLimits::default();
    limits.verification.exact_algebra.max_exponent = 0;
    assert_limited(limits);
}

#[test]
fn v1_bounded_backend_rejects_nonvacuum_input_without_claiming_completion() {
    let coefficients = CoefficientContext::new(["d", "s"]);
    let family = IntegralFamily::new(
        "nonvacuum-search-rejection",
        vec!["k".into()],
        vec!["p".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                coefficients.zero(),
                [coefficients.one(), coefficients.zero()],
            ),
            affine(
                coefficients.parameter("s").unwrap(),
                [coefficients.one(), coefficients.integer(2)],
            ),
        ],
        vec![vec![coefficients.parameter("s").unwrap()]],
        vec![coefficients.zero(); 2],
    )
    .unwrap();
    let restrictions = SectorRestrictions::unrestricted(2).unwrap();
    assert_eq!(
        discover_bounded_vacuum_internal_symmetries(
            &family,
            &restrictions,
            InternalSymmetrySearchLimits::default(),
        )
        .unwrap_err(),
        InternalSymmetrySearchError::NonVacuumFamily {
            external_momenta: 1
        }
    );
}
