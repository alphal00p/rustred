//! Independent black-box oracles for Symbolica-native Feynman polynomials and
//! LiteRed's parametric zero-sector criterion.
//!
//! Concrete loop topologies are validation fixtures only.  The production
//! implementation under test must remain loop-count and topology independent.

use rustred::{
    AffineDenominator, Coefficient, CoefficientContext, CutConstraint, FeynmanPolynomialError,
    FeynmanPolynomialLimits, FullColumnRankWitness, GuardOrigin, IntegralFamily, PowerShiftPolicy,
    SectorMask, SectorPattern, SectorRestrictions, SymanzikPolynomials, ZeroSectorAnalyzer,
    ZeroSectorCertificate, ZeroSectorConditionSource, ZeroSectorDecision, ZeroSectorError,
    ZeroSectorLimits,
};

fn affine(
    constant: Coefficient,
    coefficients: impl IntoIterator<Item = Coefficient>,
) -> AffineDenominator {
    AffineDenominator::new(constant, coefficients.into_iter().collect())
}

fn tadpole(massive: bool, power_shift: &str) -> IntegralFamily {
    tadpole_named(
        massive,
        power_shift,
        if massive {
            "zero-oracle-massive-tadpole"
        } else {
            "zero-oracle-massless-tadpole"
        },
    )
}

fn tadpole_named(massive: bool, power_shift: &str, name: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2", "nu"]);
    let mass = if massive {
        context.parameter("m2").unwrap()
    } else {
        context.zero()
    };
    IntegralFamily::new(
        name,
        vec!["k".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![affine(mass, [context.one()])],
        Vec::new(),
        vec![context.parse(power_shift).unwrap()],
    )
    .unwrap()
}

fn shifted_pair_bubble() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "s", "nu"]);
    let s = context.parameter("s").unwrap();
    IntegralFamily::new(
        "zero-oracle-integer-separated-shifts",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            affine(context.zero(), [context.one(), context.zero()]),
            affine(s.clone(), [context.one(), context.integer(2)]),
        ],
        vec![vec![s]],
        vec![
            context.parameter("nu").unwrap(),
            context.parse("nu+1").unwrap(),
        ],
    )
    .unwrap()
}

fn proved_zero(decision: &ZeroSectorDecision) -> &ZeroSectorCertificate {
    match decision {
        ZeroSectorDecision::ProvedZero(certificate) => certificate,
        other => panic!("expected a zero certificate, got {other:?}"),
    }
}

fn no_zero_certificate(decision: &ZeroSectorDecision) -> &FullColumnRankWitness {
    match decision {
        ZeroSectorDecision::NoZeroCertificate(witness) => witness,
        other => panic!("expected full rank of the sufficient criterion, got {other:?}"),
    }
}

fn off_shell_bubble(off_shell: bool, second_power_shift: &str) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "s", "nu1"]);
    let invariant = if off_shell {
        context.parameter("s").unwrap()
    } else {
        context.zero()
    };
    IntegralFamily::new(
        if off_shell {
            "zero-oracle-off-shell-bubble"
        } else {
            "zero-oracle-on-shell-bubble"
        },
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            // D0 = k^2
            affine(context.zero(), [context.one(), context.zero()]),
            // D1 = (k+p)^2 = k^2 + 2 k.p + p^2
            affine(invariant.clone(), [context.one(), context.integer(2)]),
        ],
        vec![vec![invariant]],
        vec![context.zero(), context.parse(second_power_shift).unwrap()],
    )
    .unwrap()
}

fn sunset(massive: bool) -> IntegralFamily {
    let context = CoefficientContext::new(["d", "m2"]);
    let mass = if massive {
        context.parameter("m2").unwrap()
    } else {
        context.zero()
    };
    IntegralFamily::new(
        if massive {
            "zero-oracle-massive-sunset"
        } else {
            "zero-oracle-massless-sunset"
        },
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            // Coordinate order is k0^2, k0.k1, k1^2.
            affine(
                mass.clone(),
                [context.one(), context.zero(), context.zero()],
            ),
            affine(
                mass.clone(),
                [context.zero(), context.zero(), context.one()],
            ),
            affine(mass, [context.one(), context.integer(-2), context.one()]),
        ],
        Vec::new(),
        vec![context.zero(), context.zero(), context.zero()],
    )
    .unwrap()
}

fn rational_affine_family() -> IntegralFamily {
    let context = CoefficientContext::new(["d", "a", "b", "h", "m0", "m1", "g"]);
    IntegralFamily::new(
        "zero-oracle-rational-affine",
        vec!["k".into()],
        vec!["p".into()],
        context.clone(),
        context.parameter("d").unwrap(),
        vec![
            affine(
                context.parameter("m0").unwrap(),
                [context.parse("a/h").unwrap(), context.integer(2)],
            ),
            affine(
                context.parameter("m1").unwrap(),
                [context.integer(3), context.parameter("b").unwrap()],
            ),
        ],
        vec![vec![context.parameter("g").unwrap()]],
        vec![context.zero(), context.zero()],
    )
    .unwrap()
}

#[test]
fn hand_oracle_fixtures_authenticate_as_complete_affine_families() {
    for family in [tadpole(true, "0"), tadpole(false, "0")] {
        assert_eq!(family.loop_count(), 1);
        assert_eq!(family.external_count(), 0);
        assert_eq!(family.denominator_count(), 1);
    }

    for family in [off_shell_bubble(true, "0"), off_shell_bubble(false, "0")] {
        assert_eq!(family.loop_count(), 1);
        assert_eq!(family.external_count(), 1);
        assert_eq!(family.denominator_count(), 2);
    }

    for family in [sunset(true), sunset(false)] {
        assert_eq!(family.loop_count(), 2);
        assert_eq!(family.external_count(), 0);
        assert_eq!(family.denominator_count(), 3);
    }

    let affine = rational_affine_family();
    assert_eq!(affine.denominator_count(), 2);
    assert!(affine.domain().conditions().count() >= 2);
}

#[test]
fn one_loop_hand_polynomials_match_litered_feynparuf() {
    let massive = tadpole(true, "0");
    let context = massive.coefficient_context();
    let m2 = context.parameter("m2").unwrap();
    let symanzik = SymanzikPolynomials::try_from_family(&massive).unwrap();
    assert_eq!(symanzik.u().term_count(), 1);
    assert_eq!(symanzik.u().coefficient(&[1]), Some(&context.one()));
    assert_eq!(symanzik.f().term_count(), 1);
    assert_eq!(symanzik.f().coefficient(&[2]), Some(&m2));
    assert_eq!(symanzik.g().term_count(), 2);

    let massless = tadpole(false, "0");
    let symanzik = SymanzikPolynomials::try_from_family(&massless).unwrap();
    assert_eq!(symanzik.u().coefficient(&[1]), Some(&context.one()));
    assert!(symanzik.f().is_zero());
    assert_eq!(symanzik.g(), symanzik.u());

    let off_shell = off_shell_bubble(true, "0");
    let context = off_shell.coefficient_context();
    let s = context.parameter("s").unwrap();
    let symanzik = SymanzikPolynomials::try_from_family(&off_shell).unwrap();
    assert_eq!(symanzik.u().term_count(), 2);
    assert_eq!(symanzik.u().coefficient(&[1, 0]), Some(&context.one()));
    assert_eq!(symanzik.u().coefficient(&[0, 1]), Some(&context.one()));
    assert_eq!(symanzik.f().term_count(), 1);
    assert_eq!(symanzik.f().coefficient(&[1, 1]), Some(&s));
    // Completing the square must cancel the apparent s*x1^2 term.
    assert_eq!(symanzik.f().coefficient(&[0, 2]), None);

    let on_shell = off_shell_bubble(false, "0");
    let symanzik = SymanzikPolynomials::try_from_family(&on_shell).unwrap();
    assert!(symanzik.f().is_zero());
}

#[test]
fn sunset_polynomials_match_the_independent_mass_formula() {
    let family = sunset(true);
    let context = family.coefficient_context();
    let m2 = context.parameter("m2").unwrap();
    let three_m2 = context
        .try_mul(&context.integer(3), &m2, Default::default())
        .unwrap();
    let symanzik = SymanzikPolynomials::try_from_family(&family).unwrap();

    for exponent in [[1, 1, 0], [1, 0, 1], [0, 1, 1]] {
        assert_eq!(symanzik.u().coefficient(&exponent), Some(&context.one()));
    }
    assert_eq!(symanzik.u().term_count(), 3);
    for exponent in [
        [2, 1, 0],
        [2, 0, 1],
        [1, 2, 0],
        [0, 2, 1],
        [1, 0, 2],
        [0, 1, 2],
    ] {
        assert_eq!(symanzik.f().coefficient(&exponent), Some(&m2));
    }
    assert_eq!(symanzik.f().coefficient(&[1, 1, 1]), Some(&three_m2));
    assert_eq!(symanzik.f().term_count(), 7);

    let massless = SymanzikPolynomials::try_from_family(&sunset(false)).unwrap();
    assert!(massless.f().is_zero());
    assert_eq!(massless.g(), massless.u());
}

#[test]
fn arbitrary_rational_affine_basis_matches_independent_a_q_c_assembly() {
    let family = rational_affine_family();
    let context = family.coefficient_context();
    let symanzik = SymanzikPolynomials::try_from_family(&family).unwrap();

    // A=(a/h)x0+3x1, Q=x0+(b/2)x1, C=m0*x0+m1*x1,
    // hence F=A*C-g*Q^2.  These expressions are independent of the
    // production matrix/adjugate assembly.
    assert_eq!(
        symanzik.u().coefficient(&[1, 0]),
        Some(&context.parse("a/h").unwrap())
    );
    assert_eq!(symanzik.u().coefficient(&[0, 1]), Some(&context.integer(3)));
    assert_eq!(
        symanzik.f().coefficient(&[2, 0]),
        Some(&context.parse("a*m0/h-g").unwrap())
    );
    assert_eq!(
        symanzik.f().coefficient(&[1, 1]),
        Some(&context.parse("a*m1/h+3*m0-b*g").unwrap())
    );
    assert_eq!(
        symanzik.f().coefficient(&[0, 2]),
        Some(&context.parse("3*m1-b^2*g/4").unwrap())
    );
    assert_eq!(symanzik.f().term_count(), 3);
}

#[test]
fn feynman_polynomial_resource_failure_is_typed() {
    let limits = FeynmanPolynomialLimits {
        max_parameters: 0,
        ..FeynmanPolynomialLimits::default()
    };
    assert!(matches!(
        SymanzikPolynomials::try_from_family_with_limits(&tadpole(true, "0"), limits),
        Err(FeynmanPolynomialError::ResourceLimit {
            resource: "Feynman parameters",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn gradient_and_face_restriction_replay_g_on_the_full_parameter_map() {
    let family = off_shell_bubble(true, "0");
    let context = family.coefficient_context();
    let s = context.parameter("s").unwrap();
    let symanzik = SymanzikPolynomials::try_from_family(&family).unwrap();
    let gradient = symanzik.try_gradient().unwrap();
    assert_eq!(gradient.len(), 2);
    assert_eq!(gradient[0].coefficient(&[0, 0]), Some(&context.one()));
    assert_eq!(gradient[0].coefficient(&[0, 1]), Some(&s));
    assert_eq!(gradient[1].coefficient(&[0, 0]), Some(&context.one()));
    assert_eq!(gradient[1].coefficient(&[1, 0]), Some(&s));

    let pinch = SectorMask::try_from_bit_string("10").unwrap();
    let face = symanzik.try_restrict_face(symanzik.g(), &pinch).unwrap();
    assert_eq!(face.raw().nvars(), 2);
    assert_eq!(face.term_count(), 1);
    assert_eq!(face.coefficient(&[1, 0]), Some(&context.one()));

    let empty = SectorMask::try_from_bit_string("00").unwrap();
    assert!(
        symanzik
            .try_restrict_face(symanzik.g(), &empty)
            .unwrap()
            .is_zero()
    );

    let wrong_arity = SectorMask::try_from_bit_string("1").unwrap();
    assert!(matches!(
        symanzik.try_restrict_face(symanzik.g(), &wrong_arity),
        Err(FeynmanPolynomialError::MalformedPolynomial { .. })
    ));
}

#[test]
fn tadpole_and_bubble_sector_tables_match_hand_rank_oracles() {
    let massive = tadpole(true, "0");
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&massive, PowerShiftPolicy::FormalGeneric).unwrap();
    let empty = SectorMask::try_from_bit_string("0").unwrap();
    let top = SectorMask::try_from_bit_string("1").unwrap();
    let empty_decision = analyzer.analyze_sector(&empty);
    let empty_certificate = proved_zero(&empty_decision);
    assert_eq!(empty_certificate.rank(), 0);
    assert_eq!(empty_certificate.primitive_kernel()[0].to_string(), "1");
    empty_certificate.replay(&massive).unwrap();
    let top_decision = analyzer.analyze_sector(&top);
    let top_witness = no_zero_certificate(&top_decision);
    assert_eq!(top_witness.rank(), 2);
    assert_eq!(top_witness.column_count(), 2);

    let massless = tadpole(false, "0");
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&massless, PowerShiftPolicy::FormalGeneric).unwrap();
    let top_decision = analyzer.analyze_sector(&top);
    let top_certificate = proved_zero(&top_decision);
    assert_eq!(top_certificate.rank(), 1);
    assert_eq!(
        top_certificate
            .primitive_kernel()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["1", "-1"]
    );
    top_certificate.replay(&massless).unwrap();

    let off_shell = off_shell_bubble(true, "0");
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&off_shell, PowerShiftPolicy::FormalGeneric).unwrap();
    for bits in ["00", "01", "10"] {
        let sector = SectorMask::try_from_bit_string(bits).unwrap();
        proved_zero(&analyzer.analyze_sector(&sector))
            .replay(&off_shell)
            .unwrap();
    }
    let bubble_top = SectorMask::try_from_bit_string("11").unwrap();
    let bubble_decision = analyzer.analyze_sector(&bubble_top);
    let witness = no_zero_certificate(&bubble_decision);
    assert_eq!(witness.rank(), 3);
    assert_eq!(witness.column_count(), 3);

    let on_shell = off_shell_bubble(false, "0");
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&on_shell, PowerShiftPolicy::FormalGeneric).unwrap();
    let bubble_decision = analyzer.analyze_sector(&bubble_top);
    let certificate = proved_zero(&bubble_decision);
    assert_eq!(
        certificate
            .primitive_kernel()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["1", "1", "-1"]
    );
    certificate.replay(&on_shell).unwrap();
}

#[test]
fn sunset_exhausts_all_eight_masks_and_has_the_expected_massless_kernel() {
    let massive = sunset(true);
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&massive, PowerShiftPolicy::FormalGeneric).unwrap();
    let analysis = analyzer.analyze_all().unwrap();
    assert_eq!(analysis.decisions().len(), 8);
    assert!(analysis.monotone_zero_closure_verified());
    for (mask, decision) in analysis.decisions() {
        if mask.active_count() <= 1 {
            proved_zero(decision).replay(&massive).unwrap();
        } else {
            no_zero_certificate(decision);
        }
    }

    let massless = sunset(false);
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&massless, PowerShiftPolicy::FormalGeneric).unwrap();
    let analysis = analyzer.analyze_all().unwrap();
    assert_eq!(analysis.decisions().len(), 8);
    assert!(analysis.monotone_zero_closure_verified());
    for (_, decision) in analysis.decisions() {
        proved_zero(decision).replay(&massless).unwrap();
    }
    let top = SectorMask::try_from_bit_string("111").unwrap();
    let certificate = proved_zero(analysis.decision(&top).unwrap());
    assert_eq!(certificate.active_parameter_order(), &[0, 1, 2]);
    assert_eq!(certificate.rank(), 3);
    assert_eq!(
        certificate
            .primitive_kernel()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>(),
        ["1", "1", "1", "-2"]
    );
}

#[test]
fn cuts_and_patterns_are_exclusions_not_analytic_zero_proofs() {
    let family = off_shell_bubble(true, "0");
    let restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(2, [0]).unwrap(),
        SectorPattern::try_from_string("*1").unwrap(),
    )
    .unwrap();
    let analyzer =
        ZeroSectorAnalyzer::try_new(&family, restrictions, PowerShiftPolicy::FormalGeneric)
            .unwrap();
    for bits in ["00", "01", "10"] {
        let sector = SectorMask::try_from_bit_string(bits).unwrap();
        let ZeroSectorDecision::Excluded(exclusion) = analyzer.analyze_sector(&sector) else {
            panic!("restricted sector {bits} was not classified as Excluded");
        };
        if bits == "00" {
            assert!(exclusion.violates_cut());
            assert!(exclusion.violates_pattern());
        }
    }
    let top = SectorMask::try_from_bit_string("11").unwrap();
    no_zero_certificate(&analyzer.analyze_sector(&top));
}

#[test]
fn formal_power_shift_support_changes_the_face_and_carries_provenance() {
    let shifted = off_shell_bubble(true, "nu1");
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&shifted, PowerShiftPolicy::FormalGeneric).unwrap();
    assert_eq!(analyzer.power_support().to_bit_string(), "01");
    let support = analyzer
        .domain()
        .conditions()
        .iter()
        .find(|condition| {
            condition
                .sources()
                .contains(&ZeroSectorConditionSource::PowerShiftSupport { denominator: 1 })
        })
        .expect("symbolic power support guard");
    assert!(
        support
            .origins()
            .contains(&GuardOrigin::PowerShiftSupport { denominator: 1 })
    );
    assert_eq!(
        support.polynomial().to_expression(),
        shifted
            .coefficient_context()
            .parameter("nu1")
            .unwrap()
            .numerator
            .to_expression()
    );

    let raw_pinch = SectorMask::try_from_bit_string("10").unwrap();
    let shifted_decision = analyzer.analyze_sector(&raw_pinch);
    let witness = no_zero_certificate(&shifted_decision);
    assert_eq!(witness.raw_sector().to_bit_string(), "10");
    assert_eq!(witness.effective_sector().to_bit_string(), "11");

    // At nu1=0 the family must be rebuilt and the same raw face recomputed.
    let unshifted = off_shell_bubble(true, "0");
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&unshifted, PowerShiftPolicy::FormalGeneric).unwrap();
    let unshifted_decision = analyzer.analyze_sector(&raw_pinch);
    let certificate = proved_zero(&unshifted_decision);
    assert_eq!(certificate.effective_sector().to_bit_string(), "10");

    // A nonzero constant noninteger has support but no exceptional numerator
    // locus to guard.
    let half_shifted = off_shell_bubble(true, "1/2");
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&half_shifted, PowerShiftPolicy::FormalGeneric)
            .unwrap();
    assert_eq!(analyzer.power_support().to_bit_string(), "01");
    assert!(!analyzer.domain().conditions().iter().any(|condition| {
        condition
            .sources()
            .contains(&ZeroSectorConditionSource::PowerShiftSupport { denominator: 1 })
    }));
}

#[test]
fn unsupported_power_shift_semantics_fail_typed() {
    assert!(matches!(
        ZeroSectorAnalyzer::try_unrestricted(
            &off_shell_bubble(true, "1"),
            PowerShiftPolicy::FormalGeneric,
        ),
        Err(ZeroSectorError::UnsupportedNonzeroIntegerPowerShift { denominator: 1 })
    ));
    assert!(matches!(
        ZeroSectorAnalyzer::try_unrestricted(
            &shifted_pair_bubble(),
            PowerShiftPolicy::FormalGeneric,
        ),
        Err(ZeroSectorError::UnsupportedIntegerSeparatedPowerShifts { left: 0, right: 1 })
    ));

    let family = off_shell_bubble(true, "nu1");
    let restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(2, [1]).unwrap(),
        SectorPattern::any(2).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        ZeroSectorAnalyzer::try_new(&family, restrictions, PowerShiftPolicy::FormalGeneric,),
        Err(ZeroSectorError::UnsupportedShiftedCut { denominator: 1 })
    ));
}

#[test]
fn zero_certificates_inherit_family_domain_guards_and_replay_deterministically() {
    let family = rational_affine_family();
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&family, PowerShiftPolicy::FormalGeneric).unwrap();
    let empty = SectorMask::try_from_bit_string("00").unwrap();
    let first = proved_zero(&analyzer.analyze_sector(&empty)).clone();
    let second = proved_zero(&analyzer.analyze_sector(&empty)).clone();
    assert_eq!(first, second);
    assert_eq!(first.raw_sector(), &empty);
    assert_eq!(first.effective_sector(), &empty);
    for family_condition in family.domain().conditions() {
        let inherited = first
            .domain()
            .conditions()
            .iter()
            .find(|condition| condition.polynomial() == family_condition.polynomial())
            .expect("family-domain condition inherited by certificate");
        assert!(family_condition.origins().is_subset(inherited.origins()));
    }
    first.replay(&family).unwrap();
    first.clone().replay(&family).unwrap();

    let foreign = tadpole_named(true, "0", "foreign-certificate-family");
    assert!(matches!(
        first.replay(&foreign),
        Err(ZeroSectorError::ForeignCertificateFamily)
    ));
}

#[test]
fn zero_sector_resource_failures_are_never_misclassified_as_zero() {
    let family = tadpole(true, "0");
    let rank_limited = ZeroSectorLimits {
        max_rank_rows: 0,
        ..ZeroSectorLimits::default()
    };
    let analyzer = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        rank_limited,
    )
    .unwrap();
    let top = SectorMask::try_from_bit_string("1").unwrap();
    let ZeroSectorDecision::ResourceLimited(resource) = analyzer.analyze_sector(&top) else {
        panic!("rank exhaustion was not a ResourceLimited decision");
    };
    assert_eq!(resource.resource(), "rank matrix rows");
    assert_eq!(resource.requested(), 1);
    assert_eq!(resource.limit(), 0);

    let sector_limited = ZeroSectorLimits {
        max_sectors: 1,
        ..ZeroSectorLimits::default()
    };
    let analyzer = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        sector_limited,
    )
    .unwrap();
    assert!(matches!(
        analyzer.analyze_all(),
        Err(ZeroSectorError::ResourceLimit {
            resource: "raw sectors",
            requested: 2,
            limit: 1,
        })
    ));

    let feynman_limited = ZeroSectorLimits {
        feynman: FeynmanPolynomialLimits {
            max_parameters: 0,
            ..FeynmanPolynomialLimits::default()
        },
        ..ZeroSectorLimits::default()
    };
    assert!(matches!(
        ZeroSectorAnalyzer::try_unrestricted_with_limits(
            &family,
            PowerShiftPolicy::FormalGeneric,
            feynman_limited,
        ),
        Err(ZeroSectorError::Feynman(
            FeynmanPolynomialError::ResourceLimit {
                resource: "Feynman parameters",
                ..
            }
        ))
    ));

    // A certificate-size budget is irrelevant on a full-column-rank face.
    let no_certificate_storage = ZeroSectorLimits {
        max_certificate_entries: 0,
        ..ZeroSectorLimits::default()
    };
    let analyzer = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        no_certificate_storage,
    )
    .unwrap();
    no_zero_certificate(&analyzer.analyze_sector(&top));

    // The same budget blocks an actually deficient nonempty face and remains
    // a typed resource outcome rather than a zero claim.
    let massless = tadpole(false, "0");
    let analyzer = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &massless,
        PowerShiftPolicy::FormalGeneric,
        no_certificate_storage,
    )
    .unwrap();
    let ZeroSectorDecision::ResourceLimited(resource) = analyzer.analyze_sector(&top) else {
        panic!("certificate storage exhaustion was not ResourceLimited");
    };
    assert_eq!(resource.resource(), "certificate kernel entries");

    let no_rref_bits = ZeroSectorLimits {
        max_rref_integer_bits: 0,
        ..ZeroSectorLimits::default()
    };
    let analyzer = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        no_rref_bits,
    )
    .unwrap();
    let ZeroSectorDecision::ResourceLimited(resource) = analyzer.analyze_sector(&top) else {
        panic!("RREF bit exhaustion was not ResourceLimited");
    };
    assert_eq!(resource.resource(), "RREF integer bits");

    let no_kernel_bits = ZeroSectorLimits {
        max_kernel_integer_bits: 0,
        ..ZeroSectorLimits::default()
    };
    let analyzer = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        no_kernel_bits,
    )
    .unwrap();
    let empty = SectorMask::try_from_bit_string("0").unwrap();
    let ZeroSectorDecision::ResourceLimited(resource) = analyzer.analyze_sector(&empty) else {
        panic!("kernel bit exhaustion was not ResourceLimited");
    };
    assert_eq!(resource.resource(), "certificate kernel integer bits");

    let no_shift_pairs = ZeroSectorLimits {
        max_power_shift_pair_checks: 0,
        ..ZeroSectorLimits::default()
    };
    assert!(matches!(
        ZeroSectorAnalyzer::try_unrestricted_with_limits(
            &off_shell_bubble(true, "0"),
            PowerShiftPolicy::FormalGeneric,
            no_shift_pairs,
        ),
        Err(ZeroSectorError::ResourceLimit {
            resource: "power-shift pair checks",
            requested: 1,
            limit: 0,
        })
    ));
}

#[test]
fn effective_mask_cache_never_retains_more_entries_than_its_limit() {
    let family = off_shell_bubble(true, "0");
    let limits = ZeroSectorLimits {
        max_effective_masks: 1,
        ..ZeroSectorLimits::default()
    };
    let analyzer = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        limits,
    )
    .unwrap();
    assert!(matches!(
        analyzer.analyze_all(),
        Err(ZeroSectorError::ResourceLimit {
            resource: "effective mask cache",
            requested: 2,
            limit: 1,
        })
    ));
}
