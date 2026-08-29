use std::collections::BTreeSet;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, CoefficientLocation, IntegralFamily};
use crate::sector::Mask;

use super::{PowerShiftPolicy, ZeroSectorAnalyzer, ZeroSectorConditionSource, ZeroSectorDecision};

fn one_denominator_massive_family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    let zero = coefficients.zero();
    IntegralFamily::new(
        "zero-sector-on-demand-sentinel",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.coefficient_fixture("-m2"),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![zero],
    )
    .unwrap()
}

fn family_and_power_support_conditions() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "s", "nu"]);
    IntegralFamily::new(
        "zero-sector-condition-sources",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.coefficient_fixture("d/s"),
        vec![AffineDenominator::new(
            coefficients.zero(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.parameter("nu").unwrap()],
    )
    .unwrap()
}

#[test]
fn sentinel_zero_sector_decisions_are_explicit_and_on_demand() {
    let family = one_denominator_massive_family();
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&family, PowerShiftPolicy::FormalGeneric).unwrap();

    let inactive = Mask::try_new([false]).unwrap();
    let ZeroSectorDecision::ProvedZero(certificate) = analyzer.analyze_sector(&inactive) else {
        panic!("the inactive one-denominator sector must have a zero certificate");
    };
    assert_eq!(certificate.raw_sector(), &inactive);
    certificate.replay(&family).unwrap();

    let active = Mask::try_new([true]).unwrap();
    assert!(matches!(
        analyzer.analyze_sector(&active),
        ZeroSectorDecision::NoZeroCertificate(_)
    ));
}

#[test]
fn domain_evidence_uses_only_zero_sector_sources() {
    let family = family_and_power_support_conditions();
    let analyzer =
        ZeroSectorAnalyzer::try_unrestricted(&family, PowerShiftPolicy::FormalGeneric).unwrap();
    let coefficients = family.coefficient_context();

    let family_condition = analyzer
        .domain()
        .conditions()
        .iter()
        .find(|condition| condition.polynomial() == &coefficients.parameter("s").unwrap().numerator)
        .unwrap();
    assert_eq!(
        family_condition.sources(),
        &BTreeSet::from([ZeroSectorConditionSource::Family(
            CoefficientLocation::Dimension,
        )])
    );

    let power_support = analyzer
        .domain()
        .conditions()
        .iter()
        .find(|condition| {
            condition.polynomial() == &coefficients.parameter("nu").unwrap().numerator
        })
        .unwrap();
    assert_eq!(
        power_support.sources(),
        &BTreeSet::from([ZeroSectorConditionSource::PowerShiftSupport { denominator: 0 }])
    );
}
