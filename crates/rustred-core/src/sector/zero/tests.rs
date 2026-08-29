use std::collections::BTreeSet;

use crate::algebra::CoefficientContext;
use crate::family::{AffineDenominator, CoefficientLocation, IntegralFamily};
use crate::sector::Mask;

use super::{Analyzer, ConditionSource, Decision, Error, Limits};

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
    let analyzer = Analyzer::try_unrestricted(&family).unwrap();

    let inactive = Mask::try_new([false]).unwrap();
    let Decision::ProvedZero(certificate) = analyzer.analyze(&inactive).unwrap() else {
        panic!("the inactive one-denominator sector must have a zero certificate");
    };
    assert_eq!(certificate.raw_sector(), &inactive);
    assert!(
        certificate
            .primitive_kernel()
            .iter()
            .any(|entry| !entry.is_zero())
    );

    let active = Mask::try_new([true]).unwrap();
    assert!(matches!(
        analyzer.analyze(&active).unwrap(),
        Decision::Inconclusive(_)
    ));
}

#[test]
fn domain_evidence_uses_only_zero_sector_sources() {
    let family = family_and_power_support_conditions();
    let analyzer = Analyzer::try_unrestricted(&family).unwrap();
    let coefficients = family.coefficient_context();

    let family_condition = analyzer
        .domain()
        .conditions()
        .iter()
        .find(|condition| condition.polynomial() == &coefficients.parameter("s").unwrap().numerator)
        .unwrap();
    assert_eq!(
        family_condition.sources(),
        &BTreeSet::from([ConditionSource::Family(CoefficientLocation::Dimension,)])
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
        &BTreeSet::from([ConditionSource::PowerShiftSupport { denominator: 0 }])
    );
}

#[test]
fn resource_exhaustion_is_an_error_not_a_sector_decision() {
    let family = one_denominator_massive_family();
    let mut limits = Limits::default();
    limits.max_rank_operations = 0;
    let analyzer = Analyzer::try_unrestricted_with_limits(&family, limits).unwrap();
    let active = Mask::try_new([true]).unwrap();

    assert!(matches!(
        analyzer.analyze(&active),
        Err(Error::ResourceLimit {
            resource: "rank operations",
            ..
        })
    ));
}

#[test]
fn malformed_sector_arity_remains_a_typed_error() {
    let family = one_denominator_massive_family();
    let analyzer = Analyzer::try_unrestricted(&family).unwrap();
    let malformed = Mask::try_new([true, false]).unwrap();

    assert!(matches!(
        analyzer.analyze(&malformed),
        Err(Error::Sector(_))
    ));
}
