#![cfg(feature = "legacy-authored-oracles")]

use rustred::families::equal_mass_two_loop_vacuum;
use rustred::{
    CoefficientContext, Denominator, ExactRational, IbpGenerationError, IbpGenerator, Integral,
    LinearCombination, SeedConfig, SparseReducer, VacuumFamily, generate_seeds,
};

fn build_corner_reduction() -> (
    rustred::VacuumFamily,
    Vec<rustred::IbpIdentity>,
    rustred::ReductionTable,
) {
    let family = equal_mass_two_loop_vacuum().unwrap();
    let seeds = generate_seeds(&family, SeedConfig::default());
    let identities = IbpGenerator::new(&family).generate_for_seeds(&seeds);
    let table = SparseReducer::new(family.clone())
        .reduce(&identities)
        .unwrap();
    (family, identities, table)
}

fn build_one_dot_reduction() -> (
    rustred::VacuumFamily,
    Vec<rustred::IbpIdentity>,
    rustred::ReductionTable,
) {
    let family = equal_mass_two_loop_vacuum().unwrap();
    let seeds = generate_seeds(
        &family,
        SeedConfig {
            max_dots: 1,
            ..SeedConfig::default()
        },
    );
    let identities = IbpGenerator::new(&family).generate_for_seeds(&seeds);
    let table = SparseReducer::new(family.clone())
        .reduce(&identities)
        .unwrap();
    (family, identities, table)
}

fn check_symmetry_and_boundaries() {
    let family = equal_mass_two_loop_vacuum().unwrap();
    assert_eq!(family.symmetries().len(), 6);
    assert_eq!(
        family.canonicalize(&Integral::from([1, 2, 1])),
        Some(Integral::from([2, 1, 1]))
    );
    assert_eq!(family.canonicalize(&Integral::from([1, 0, 0])), None);
    assert_eq!(
        family.canonicalize(&Integral::from([1, 1, 0])),
        Some(Integral::from([1, 1, 0]))
    );
}

fn check_family_safety() {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients.parameter("m2").unwrap();
    let zero = coefficients.zero();

    // This complete basis has no unit-Jacobian map exchanging the first and
    // third propagators; accepting the permutation would create false rules.
    let invalid = VacuumFamily::new(
        "invalid_symmetry",
        2,
        coefficients.clone(),
        "d",
        vec![
            Denominator::propagator(vec![1.into(), 0.into()], mass.clone()),
            Denominator::propagator(vec![0.into(), 1.into()], mass.clone()),
            Denominator::propagator(vec![2.into(), 1.into()], mass.clone()),
        ],
        vec![vec![2, 1, 0]],
    );
    assert!(matches!(
        invalid,
        Err(rustred::FamilyError::InvalidPermutation(_))
    ));

    // A massive tadpole times a disconnected massless tadpole is scaleless.
    let disconnected = VacuumFamily::new(
        "mixed_components",
        2,
        coefficients,
        "d",
        vec![
            Denominator::propagator(vec![1.into(), 0.into()], mass),
            Denominator::propagator(vec![0.into(), 1.into()], zero.clone()),
            Denominator::auxiliary(
                vec![ExactRational::ZERO, ExactRational::ONE, ExactRational::ZERO],
                zero,
            ),
        ],
        vec![],
    )
    .unwrap();
    assert!(disconnected.is_scaleless(&Integral::from([1, 1, 0])));
    // Positive auxiliaries are kept conservatively until the full Lee
    // zero-sector criterion is implemented.
    assert!(!disconnected.is_scaleless(&Integral::from([1, 1, 1])));

    // Public family predicates and ordering never index past caller-provided
    // data.  Legacy Option/Boolean APIs stay conservative, while checked
    // counterparts preserve the distinction between invalid input and a
    // genuine auxiliary/scaleless result.
    assert!(!disconnected.is_propagator(3));
    assert!(matches!(
        disconnected.try_is_propagator(3),
        Err(rustred::FamilyError::DenominatorOutOfRange {
            position: 3,
            denominators: 3,
        })
    ));
    let wrong_arity = Integral::from([1, 1]);
    let valid = Integral::from([1, 1, 0]);
    assert_eq!(disconnected.canonicalize(&wrong_arity), None);
    assert!(!disconnected.is_scaleless(&wrong_arity));
    assert!(matches!(
        disconnected.try_canonicalize(&wrong_arity),
        Err(rustred::FamilyError::WrongIntegralArity {
            expected: 3,
            actual: 2,
        })
    ));
    assert!(matches!(
        disconnected.try_is_scaleless(&wrong_arity),
        Err(rustred::FamilyError::WrongIntegralArity {
            expected: 3,
            actual: 2,
        })
    ));
    let _legacy_ordering = disconnected.compare_integrals(&wrong_arity, &valid);
    assert!(matches!(
        disconnected.try_compare_integrals(&wrong_arity, &valid),
        Err(rustred::FamilyError::WrongIntegralArity {
            expected: 3,
            actual: 2,
        })
    ));
}

fn check_ibp_count() {
    let family = equal_mass_two_loop_vacuum().unwrap();
    let identities = IbpGenerator::new(&family).generate(&Integral::from([1, 1, 1]));
    assert_eq!(identities.len(), 4);
    // Some off-diagonal total derivatives vanish identically after the full
    // equal-mass S3 symmetry is imposed. At least the two diagonal identities
    // must remain nontrivial.
    assert!(
        identities
            .iter()
            .filter(|identity| !identity.equation.is_zero())
            .count()
            >= 2
    );
}

fn check_raw_ibp_oracle() {
    let family = equal_mass_two_loop_vacuum().unwrap();
    let coefficients = family.coefficients();
    let seed = Integral::from([2, 3, 4]);
    let identities = IbpGenerator::new(&family).generate_raw(&seed);
    assert_eq!(identities.len(), 4);

    let mut expected = Vec::new();
    let mut equation = LinearCombination::new();
    for (integral, coefficient) in [
        ([2, 3, 4], "d-8"),
        ([3, 3, 4], "4*m2"),
        ([1, 3, 5], "-4"),
        ([2, 2, 5], "4"),
        ([2, 3, 5], "4*m2"),
    ] {
        equation.add_term(
            Integral::from(integral),
            coefficients.parse(coefficient).unwrap(),
        );
    }
    expected.push(equation);

    let mut equation = LinearCombination::new();
    for (integral, coefficient) in [
        ([2, 3, 4], "-2"),
        ([3, 3, 3], "-2"),
        ([3, 2, 4], "2"),
        ([3, 3, 4], "-2*m2"),
        ([1, 3, 5], "4"),
        ([2, 2, 5], "-4"),
        ([2, 3, 5], "4*m2"),
    ] {
        equation.add_term(
            Integral::from(integral),
            coefficients.parse(coefficient).unwrap(),
        );
    }
    expected.push(equation);

    let mut equation = LinearCombination::new();
    for (integral, coefficient) in [
        ([2, 3, 4], "-1"),
        ([2, 4, 3], "-3"),
        ([1, 4, 4], "3"),
        ([2, 4, 4], "-3*m2"),
        ([2, 2, 5], "4"),
        ([1, 3, 5], "-4"),
        ([2, 3, 5], "4*m2"),
    ] {
        equation.add_term(
            Integral::from(integral),
            coefficients.parse(coefficient).unwrap(),
        );
    }
    expected.push(equation);

    let mut equation = LinearCombination::new();
    for (integral, coefficient) in [
        ([2, 3, 4], "d-10"),
        ([2, 4, 4], "6*m2"),
        ([1, 3, 5], "4"),
        ([2, 2, 5], "-4"),
        ([2, 3, 5], "4*m2"),
    ] {
        equation.add_term(
            Integral::from(integral),
            coefficients.parse(coefficient).unwrap(),
        );
    }
    expected.push(equation);

    for (identity, expected) in identities.iter().zip(expected) {
        assert_eq!(identity.equation, expected);
    }

    // Selected generation constructs exactly one requested row and agrees
    // with the corresponding member of the independently generated full set.
    let generator = IbpGenerator::new(&family);
    for differentiated in 0..2 {
        for contraction in 0..2 {
            let selected = generator
                .try_generate_raw_identity(&seed, differentiated, contraction)
                .unwrap();
            let full = &identities[differentiated * 2 + contraction];
            assert_eq!(selected.differentiated_loop, differentiated);
            assert_eq!(selected.contraction_loop, contraction);
            assert_eq!(selected.equation, full.equation);
        }
    }
    assert!(matches!(
        generator.try_generate_raw_identity(&seed, 2, 0),
        Err(IbpGenerationError::DifferentiatedLoopOutOfRange {
            requested: 2,
            loops: 2
        })
    ));
    assert!(matches!(
        generator.try_generate_raw_identity(&seed, 0, 2),
        Err(IbpGenerationError::ContractionLoopOutOfRange {
            requested: 2,
            loops: 2
        })
    ));
    assert!(matches!(
        generator.try_generate_raw_identity(&Integral::from([i32::MAX, 3, 4]), 0, 0),
        Err(IbpGenerationError::ExponentOverflow { .. })
    ));
}

fn check_isp_seed_bounds() {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients.parameter("m2").unwrap();
    let zero = coefficients.zero();
    let family = VacuumFamily::new(
        "two_loop_with_isp",
        2,
        coefficients,
        "d",
        vec![
            Denominator::propagator(vec![1.into(), 0.into()], mass.clone()),
            Denominator::propagator(vec![0.into(), 1.into()], mass),
            Denominator::auxiliary(
                vec![ExactRational::ZERO, ExactRational::ONE, ExactRational::ZERO],
                zero,
            ),
        ],
        vec![],
    )
    .unwrap();
    let seeds = generate_seeds(
        &family,
        SeedConfig {
            max_numerator_degree: 1,
            include_subsectors: false,
            ..SeedConfig::default()
        },
    );
    assert_eq!(seeds.len(), 2);
    assert!(seeds.iter().all(|seed| seed.powers()[0..2] == [1, 1]));
    assert!(seeds.iter().all(|seed| seed.powers()[2] <= 0));
}

fn check_dotted_sunset_reduction() {
    let (family, identities, table) = build_corner_reduction();
    table.validate_identities(&identities).unwrap();

    let target = Integral::from([2, 1, 1]);
    let master = Integral::from([1, 1, 1]);
    let reduction = table.reduce_integral(&target).unwrap();
    assert_eq!(reduction.len(), 1);
    assert_eq!(
        reduction.coefficient(&master),
        Some(&family.coefficients().parse("(3-d)/(3*m2)").unwrap())
    );
}

fn check_extended_golden_reductions() {
    let (family, identities, table) = build_one_dot_reduction();
    table.validate_identities(&identities).unwrap();
    let coefficients = family.coefficients();
    let sunset = Integral::from([1, 1, 1]);
    let product = Integral::from([1, 1, 0]);

    let check = |target: Integral, expected_sunset: &str, expected_product: &str| {
        let reduction = table.reduce_integral(&target).unwrap();
        let sunset_coefficient = reduction
            .coefficient(&sunset)
            .cloned()
            .unwrap_or_else(|| coefficients.zero());
        let product_coefficient = reduction
            .coefficient(&product)
            .cloned()
            .unwrap_or_else(|| coefficients.zero());
        assert_eq!(
            sunset_coefficient,
            coefficients.parse(expected_sunset).unwrap()
        );
        assert_eq!(
            product_coefficient,
            coefficients.parse(expected_product).unwrap()
        );
        assert_eq!(
            reduction.len(),
            usize::from(expected_sunset != "0") + usize::from(expected_product != "0")
        );
    };

    check(
        Integral::from([2, 2, 1]),
        "(d-2)*(d-3)/(9*m2^2)",
        "(d-2)^2/(12*m2^3)",
    );
    check(
        Integral::from([3, 1, 1]),
        "(d-8)*(d-3)/(18*m2^2)",
        "-(d-2)^2/(12*m2^3)",
    );
    check(Integral::from([0, 2, 1]), "0", "(2-d)/(2*m2)");
}

fn check_generated_identities() {
    let (_, identities, table) = build_corner_reduction();
    table.validate_identities(&identities).unwrap();
    assert!(table.stats().rules > 0);
    assert_eq!(table.stats().input_equations, identities.len());
}

fn check_reduction_api_validation() {
    let (family, identities, table) = build_corner_reduction();

    // Public reduction entry points return typed arity errors instead of
    // reaching VacuumFamily::canonicalize's internal assertion.
    let wrong_arity = Integral::from([1, 1]);
    assert!(matches!(
        table.reduce_integral(&wrong_arity),
        Err(rustred::ReductionError::WrongIntegralArity {
            integral,
            expected: 3,
            actual: 2,
        }) if integral == wrong_arity
    ));
    let mut malformed_combination = LinearCombination::new();
    malformed_combination.add_term(wrong_arity.clone(), family.coefficients().one());
    assert!(matches!(
        table.reduce_combination(&malformed_combination),
        Err(rustred::ReductionError::WrongIntegralArity {
            integral,
            expected: 3,
            actual: 2,
        }) if integral == wrong_arity
    ));

    // IbpIdentity is publicly constructible.  Malformed metadata, malformed
    // terms, and forged equations must not become reduction rules.
    let mut bad_loop = identities[0].clone();
    bad_loop.differentiated_loop = family.loops();
    assert!(matches!(
        SparseReducer::new(family.clone()).reduce(&[bad_loop]),
        Err(rustred::ReductionError::IdentityLoopOutOfRange { loops: 2, .. })
    ));

    let mut bad_seed = identities[0].clone();
    bad_seed.seed = wrong_arity.clone();
    assert!(matches!(
        SparseReducer::new(family.clone()).reduce(&[bad_seed]),
        Err(rustred::ReductionError::WrongIntegralArity {
            expected: 3,
            actual: 2,
            ..
        })
    ));

    let mut bad_term = identities[0].clone();
    bad_term.equation = malformed_combination;
    assert!(matches!(
        SparseReducer::new(family.clone()).reduce(&[bad_term]),
        Err(rustred::ReductionError::WrongIntegralArity {
            expected: 3,
            actual: 2,
            ..
        })
    ));

    let mut forged = identities
        .iter()
        .find(|identity| !identity.equation.is_zero())
        .unwrap()
        .clone();
    forged
        .equation
        .add_term(forged.seed.clone(), family.coefficients().one());
    assert!(matches!(
        SparseReducer::new(family.clone()).reduce(&[forged.clone()]),
        Err(rustred::ReductionError::IdentityEquationMismatch { .. })
    ));
    assert!(matches!(
        table.validate_identities(&[forged]),
        Err(rustred::ReductionError::IdentityEquationMismatch { .. })
    ));

    // A genuine raw row is accepted after safe symmetry/zero-sector
    // canonicalization, preserving the generator-oracle workflow.
    let raw = IbpGenerator::new(&family).generate_raw(&Integral::from([2, 3, 4]));
    let raw_table = SparseReducer::new(family).reduce(&raw).unwrap();
    raw_table.validate_identities(&raw).unwrap();
}

fn check_cache_roundtrip() {
    let (family, identities, table) = build_one_dot_reduction();
    let mut first = Vec::new();
    table.write(&mut first).unwrap();
    let restored = rustred::ReductionTable::read(family.clone(), first.as_slice()).unwrap();
    restored.validate_identities(&identities).unwrap();
    assert_eq!(restored.rules(), table.rules());
    assert_eq!(restored.stats(), table.stats());
    let mut second = Vec::new();
    restored.write(&mut second).unwrap();
    assert_eq!(first, second);

    // The cache envelope is checksummed; changing a payload byte must be
    // rejected before any untrusted coefficient text is parsed.
    const CACHE_HEADER_BYTES: usize = 8 + 4 + 8 + 8;
    assert!(first.len() > CACHE_HEADER_BYTES);
    let mut corrupted = first.clone();
    corrupted[CACHE_HEADER_BYTES] ^= 1;
    assert!(matches!(
        rustred::ReductionTable::read(family.clone(), corrupted.as_slice()),
        Err(rustred::ReductionCacheError::InvalidFormat(message))
            if message.contains("checksum")
    ));

    // Both a short payload and bytes beyond the declared payload are invalid.
    assert!(matches!(
        rustred::ReductionTable::read(family.clone(), &first[..first.len() - 1]),
        Err(rustred::ReductionCacheError::Io(error))
            if error.kind() == std::io::ErrorKind::UnexpectedEof
    ));
    let mut trailing = first.clone();
    trailing.push(0);
    assert!(matches!(
        rustred::ReductionTable::read(family.clone(), trailing.as_slice()),
        Err(rustred::ReductionCacheError::InvalidFormat(message))
            if message.contains("trailing bytes")
    ));

    // Caller-provided limits are enforced on both input and output paths.
    let payload_limited = rustred::ReductionCacheLimits {
        max_payload_bytes: 0,
        ..rustred::ReductionCacheLimits::default()
    };
    assert!(matches!(
        rustred::ReductionTable::read_with_limits(
            family.clone(),
            first.as_slice(),
            payload_limited,
        ),
        Err(rustred::ReductionCacheError::ResourceLimit(_))
    ));
    let rule_limited = rustred::ReductionCacheLimits {
        max_rules: 0,
        ..rustred::ReductionCacheLimits::default()
    };
    assert!(matches!(
        table.write_with_limits(Vec::new(), rule_limited),
        Err(rustred::ReductionCacheError::ResourceLimit(_))
    ));

    // Algebraically identical denominators under a different family identity
    // still have a distinct fingerprint and cannot reuse this table silently.
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = coefficients.parameter("m2").unwrap();
    let differently_named_family = VacuumFamily::new(
        "cache_fingerprint_mismatch",
        2,
        coefficients,
        "d",
        vec![
            Denominator::propagator(vec![1.into(), 0.into()], mass.clone()),
            Denominator::propagator(vec![0.into(), 1.into()], mass.clone()),
            Denominator::propagator(vec![1.into(), 1.into()], mass),
        ],
        vec![vec![1, 0, 2], vec![1, 2, 0]],
    )
    .unwrap();
    assert!(matches!(
        rustred::ReductionTable::read(differently_named_family, first.as_slice()),
        Err(rustred::ReductionCacheError::InvalidFormat(message))
            if message.contains("fingerprint")
    ));
}

// Restricted Symbolica binds an instance to the first OS thread that enters
// it. Rust's test harness runs each #[test] on a distinct worker even with
// `--test-threads=1`, so keep the whole integration suite in one test/thread.
#[test]
fn two_loop_vacuum_milestone() {
    check_symmetry_and_boundaries();
    check_family_safety();
    check_ibp_count();
    check_raw_ibp_oracle();
    check_isp_seed_bounds();
    check_dotted_sunset_reduction();
    check_extended_golden_reductions();
    check_generated_identities();
    check_reduction_api_validation();
    check_cache_roundtrip();
}
