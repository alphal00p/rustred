use symbolica::prelude::Integer;

use rustred::{
    AffineDenominator, Coefficient, CoefficientContext, CutConstraint, ExactRational,
    IntegralFamily, PowerShiftPolicy, SectorMask, SectorPattern, SectorRestrictions,
    ZeroSectorAnalyzer, ZeroSectorConditionSource, ZeroSectorDecision, ZeroSectorError,
    ZeroSectorLimits,
};

fn affine(
    constant: Coefficient,
    coefficients: impl IntoIterator<Item = Coefficient>,
) -> AffineDenominator {
    AffineDenominator::new(constant, coefficients.into_iter().collect())
}

fn mask(bits: &str) -> SectorMask {
    SectorMask::try_from_bit_string(bits).unwrap()
}

fn tadpole(massive: bool) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = if massive {
        coefficients.parameter("m2").unwrap()
    } else {
        coefficients.zero()
    };
    IntegralFamily::new(
        if massive {
            "zero-sector-massive-tadpole"
        } else {
            "zero-sector-massless-tadpole"
        },
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![affine(mass, [coefficients.one()])],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn bubble(second_shift: Option<Coefficient>) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "s", "nu"]);
    let s = coefficients.parameter("s").unwrap();
    IntegralFamily::new(
        "zero-sector-offshell-bubble",
        vec!["k".into()],
        vec!["p".into()],
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                coefficients.zero(),
                [coefficients.one(), coefficients.zero()],
            ),
            affine(s.clone(), [coefficients.one(), coefficients.integer(2)]),
        ],
        vec![vec![s]],
        vec![
            coefficients.zero(),
            second_shift.unwrap_or_else(|| coefficients.zero()),
        ],
    )
    .unwrap()
}

fn sunset(massive: bool) -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let mass = if massive {
        coefficients.parameter("m2").unwrap()
    } else {
        coefficients.zero()
    };
    IntegralFamily::new(
        if massive {
            "zero-sector-massive-sunset"
        } else {
            "zero-sector-massless-sunset"
        },
        vec!["k0".into(), "k1".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![
            affine(
                mass.clone(),
                [coefficients.one(), coefficients.zero(), coefficients.zero()],
            ),
            affine(
                mass.clone(),
                [coefficients.zero(), coefficients.zero(), coefficients.one()],
            ),
            affine(
                mass,
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

#[test]
fn tadpole_decisions_are_direct_and_certificates_replay() {
    let family = tadpole(true);
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&family, PowerShiftPolicy::FormalGeneric).unwrap();
    let analysis = analyzer.analyze_all().unwrap();
    assert_eq!(analysis.decisions().len(), 2);
    assert_eq!(analysis.distinct_effective_mask_count(), 2);
    assert!(analysis.monotone_zero_closure_verified());

    let certificate = match analysis.decision(&mask("0")).unwrap() {
        ZeroSectorDecision::ProvedZero(certificate) => certificate,
        decision => panic!("expected an empty-face certificate, received {decision:?}"),
    };
    assert_eq!(certificate.raw_sector(), &mask("0"));
    assert_eq!(certificate.effective_sector(), &mask("0"));
    assert_eq!(certificate.primitive_kernel(), &[Integer::one()]);
    certificate.replay(&family).unwrap();

    assert!(matches!(
        analysis.decision(&mask("1")),
        Some(ZeroSectorDecision::NoZeroCertificate(_))
    ));

    let massless = tadpole(false);
    let massless_analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&massless, PowerShiftPolicy::FormalGeneric).unwrap();
    assert!(matches!(
        massless_analyzer.analyze_sector(&mask("1")),
        ZeroSectorDecision::ProvedZero(_)
    ));
}

#[test]
fn external_bubble_has_zero_pinches_but_no_top_zero_certificate() {
    let family = bubble(None);
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&family, PowerShiftPolicy::FormalGeneric).unwrap();
    let analysis = analyzer.analyze_all().unwrap();
    for bits in ["00", "01", "10"] {
        let certificate = match analysis.decision(&mask(bits)).unwrap() {
            ZeroSectorDecision::ProvedZero(certificate) => certificate,
            decision => panic!("expected {bits} to be certified zero, received {decision:?}"),
        };
        assert_eq!(certificate.raw_sector(), &mask(bits));
        certificate.replay(&family).unwrap();
    }
    assert!(matches!(
        analysis.decision(&mask("11")),
        Some(ZeroSectorDecision::NoZeroCertificate(_))
    ));
    assert!(analysis.monotone_zero_closure_verified());

    let pinch = match analysis.decision(&mask("10")).unwrap() {
        ZeroSectorDecision::ProvedZero(certificate) => certificate,
        _ => unreachable!(),
    };
    assert_eq!(
        pinch.primitive_kernel(),
        &[Integer::one(), Integer::from(-1)]
    );
}

#[test]
fn two_loop_sunset_exhausts_all_masks_without_topology_dispatch() {
    let family = sunset(true);
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&family, PowerShiftPolicy::FormalGeneric).unwrap();
    let analysis = analyzer.analyze_all().unwrap();
    assert_eq!(analysis.decisions().len(), 8);
    for (sector, decision) in analysis.decisions() {
        if sector.active_count() <= 1 {
            let certificate = match decision {
                ZeroSectorDecision::ProvedZero(certificate) => certificate,
                other => panic!("expected {sector} to be zero, received {other:?}"),
            };
            assert_eq!(certificate.raw_sector(), sector);
            certificate.replay(&family).unwrap();
        } else {
            assert!(matches!(decision, ZeroSectorDecision::NoZeroCertificate(_)));
        }
    }
    assert!(analysis.monotone_zero_closure_verified());

    let massless = sunset(false);
    let massless_analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&massless, PowerShiftPolicy::FormalGeneric).unwrap();
    assert!(matches!(
        massless_analyzer.analyze_sector(&mask("111")),
        ZeroSectorDecision::ProvedZero(_)
    ));
}

#[test]
fn power_support_changes_the_face_and_carries_a_numerator_guard() {
    let coefficient_context = CoefficientContext::new(["d", "s", "nu"]);
    let nu = coefficient_context.parameter("nu").unwrap();
    let family = bubble(Some(nu.clone()));
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&family, PowerShiftPolicy::FormalGeneric).unwrap();
    assert_eq!(analyzer.power_support(), &mask("01"));
    assert!(analyzer.domain().conditions().iter().any(|condition| {
        condition.polynomial() == &nu.numerator
            && condition
                .sources()
                .contains(&ZeroSectorConditionSource::PowerShiftSupport { denominator: 1 })
    }));

    let witness = match analyzer.analyze_sector(&mask("10")) {
        ZeroSectorDecision::NoZeroCertificate(witness) => witness,
        decision => panic!("shifted pinch should use the top face, received {decision:?}"),
    };
    assert_eq!(witness.raw_sector(), &mask("10"));
    assert_eq!(witness.effective_sector(), &mask("11"));

    let certificate = match analyzer.analyze_sector(&mask("00")) {
        ZeroSectorDecision::ProvedZero(certificate) => certificate,
        decision => panic!("effective 01 face should be zero, received {decision:?}"),
    };
    assert_eq!(certificate.raw_sector(), &mask("00"));
    assert_eq!(certificate.effective_sector(), &mask("01"));
    certificate.replay(&family).unwrap();
}

#[test]
fn formal_shift_policy_rejects_integer_reindexing_and_shifted_cuts() {
    let coefficients = CoefficientContext::new(["d", "s", "nu"]);
    let integer_shift = bubble(Some(coefficients.one()));
    assert!(matches!(
        ZeroSectorAnalyzer::try_unrestricted(&integer_shift, PowerShiftPolicy::FormalGeneric),
        Err(ZeroSectorError::UnsupportedNonzeroIntegerPowerShift { denominator: 1 })
    ));

    let half_shift = bubble(Some(coefficients.rational(ExactRational::new(1, 2))));
    let half_analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&half_shift, PowerShiftPolicy::FormalGeneric).unwrap();
    assert_eq!(half_analyzer.power_support(), &mask("01"));
    assert!(
        !half_analyzer.domain().conditions().iter().any(|condition| {
            condition
                .sources()
                .contains(&ZeroSectorConditionSource::PowerShiftSupport { denominator: 1 })
        })
    );

    let symbolic_shift = bubble(Some(coefficients.parameter("nu").unwrap()));
    let restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(2, [1]).unwrap(),
        SectorPattern::any(2).unwrap(),
    )
    .unwrap();
    assert!(matches!(
        ZeroSectorAnalyzer::try_new(
            &symbolic_shift,
            restrictions,
            PowerShiftPolicy::FormalGeneric
        ),
        Err(ZeroSectorError::UnsupportedShiftedCut { denominator: 1 })
    ));
}

#[test]
fn exclusions_are_not_zero_proofs_and_rank_limits_are_typed() {
    let family = bubble(None);
    let restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(2, [0]).unwrap(),
        SectorPattern::any(2).unwrap(),
    )
    .unwrap();
    let analyzer =
        ZeroSectorAnalyzer::try_new(&family, restrictions, PowerShiftPolicy::FormalGeneric)
            .unwrap();
    assert!(matches!(
        analyzer.analyze_sector(&mask("01")),
        ZeroSectorDecision::Excluded(_)
    ));
    let analysis = analyzer.analyze_all().unwrap();
    assert!(matches!(
        analysis.decision(&mask("00")),
        Some(ZeroSectorDecision::Excluded(_))
    ));
    assert_eq!(analysis.distinct_effective_mask_count(), 2);

    let mut limits = ZeroSectorLimits::default();
    limits.max_rank_rows = 0;
    let limited = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        limits,
    )
    .unwrap();
    match limited.analyze_sector(&mask("11")) {
        ZeroSectorDecision::ResourceLimited(resource) => {
            assert_eq!(resource.resource(), "rank matrix rows");
            assert_eq!(resource.limit(), 0);
        }
        decision => panic!("expected a typed rank limit, received {decision:?}"),
    }

    let mut sector_limits = ZeroSectorLimits::default();
    sector_limits.max_sectors = 3;
    let limited_all = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        sector_limits,
    )
    .unwrap();
    assert!(matches!(
        limited_all.analyze_all(),
        Err(ZeroSectorError::ResourceLimit {
            resource: "raw sectors",
            requested: 4,
            limit: 3,
        })
    ));
}

#[test]
fn effective_cache_and_big_integer_budgets_are_strictly_bounded() {
    let family = bubble(None);

    let mut cache_limits = ZeroSectorLimits::default();
    cache_limits.max_effective_masks = 1;
    let cache_limited = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        cache_limits,
    )
    .unwrap();
    assert!(matches!(
        cache_limited.analyze_all(),
        Err(ZeroSectorError::ResourceLimit {
            resource: "effective mask cache",
            requested: 2,
            limit: 1,
        })
    ));

    let mut certificate_limits = ZeroSectorLimits::default();
    certificate_limits.max_certificate_entries = 0;
    certificate_limits.max_kernel_integer_bits = 0;
    let certificate_limited = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        certificate_limits,
    )
    .unwrap();
    assert!(matches!(
        certificate_limited.analyze_sector(&mask("11")),
        ZeroSectorDecision::NoZeroCertificate(_)
    ));
    match certificate_limited.analyze_sector(&mask("10")) {
        ZeroSectorDecision::ResourceLimited(resource) => {
            assert_eq!(resource.resource(), "certificate kernel entries");
            assert_eq!(resource.requested(), 2);
            assert_eq!(resource.limit(), 0);
        }
        decision => panic!("expected a certificate-size limit, received {decision:?}"),
    }

    let mut rref_limits = ZeroSectorLimits::default();
    rref_limits.max_rref_integer_bits = 0;
    let rref_limited = ZeroSectorAnalyzer::try_unrestricted_with_limits(
        &family,
        PowerShiftPolicy::FormalGeneric,
        rref_limits,
    )
    .unwrap();
    match rref_limited.analyze_sector(&mask("11")) {
        ZeroSectorDecision::ResourceLimited(resource) => {
            assert_eq!(resource.resource(), "RREF integer bits");
            assert!(resource.requested() > 0);
            assert_eq!(resource.limit(), 0);
        }
        decision => panic!("expected an RREF bit limit, received {decision:?}"),
    }
}

#[test]
fn power_shift_pair_diagnostics_have_an_aggregate_budget() {
    let family = bubble(None);
    let mut limits = ZeroSectorLimits::default();
    limits.max_power_shift_pair_checks = 0;
    assert!(matches!(
        ZeroSectorAnalyzer::try_unrestricted_with_limits(
            &family,
            PowerShiftPolicy::FormalGeneric,
            limits,
        ),
        Err(ZeroSectorError::ResourceLimit {
            resource: "power-shift pair checks",
            requested: 1,
            limit: 0,
        })
    ));
}
