//! Black-box audit of the generic zero-sector provider composition layer.

use std::fmt;

use rustred::{
    AffineDenominator, CertifiedRewriteLimits, CertifiedZeroReductionProof,
    CertifiedZeroSectorRuleProvider, CertifiedZeroSectorRuleProviderError, CoefficientContext,
    ConcreteIntegralKey, ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus,
    CutConstraint, IntegralFamily, MasterPolicyProvider, PowerShiftPolicy, SectorMask,
    SectorPattern, SectorRestrictions,
};

fn family() -> IntegralFamily {
    let coefficients = CoefficientContext::new(["d", "m2"]);
    IntegralFamily::new(
        "zero-sector-provider-massive-tadpole",
        vec!["k".into()],
        Vec::new(),
        coefficients.clone(),
        coefficients.parameter("d").unwrap(),
        vec![AffineDenominator::new(
            coefficients.parse("-m2").unwrap(),
            vec![coefficients.one()],
        )],
        Vec::new(),
        vec![coefficients.zero()],
    )
    .unwrap()
}

fn key(powers: impl IntoIterator<Item = i64>) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(powers).unwrap()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StubError;

impl fmt::Display for StubError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("stub provider error")
    }
}

impl std::error::Error for StubError {}

struct StubProvider {
    arity: usize,
    requests: Vec<ConcreteIntegralKey>,
}

impl StubProvider {
    fn new(arity: usize) -> Self {
        Self {
            arity,
            requests: Vec::new(),
        }
    }
}

impl ConcreteRuleProvider for StubProvider {
    type Error = StubError;

    fn index_arity(&self) -> usize {
        self.arity
    }

    fn decision_for(
        &mut self,
        integral: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.requests.push(integral.clone());
        Ok(ConcreteRuleDecision::Terminal(
            ConcreteTerminalStatus::Uncovered,
        ))
    }
}

#[test]
fn analytic_zero_preempts_inner_while_nonzero_sector_delegates_unchanged() {
    let family = family();
    let mut provider = CertifiedZeroSectorRuleProvider::try_unrestricted(
        &family,
        PowerShiftPolicy::FormalGeneric,
        StubProvider::new(1),
        CertifiedRewriteLimits::default(),
    )
    .unwrap();

    let ConcreteRuleDecision::ProvedZero(zero) = provider.decision_for(&key([0])).unwrap() else {
        panic!("the empty one-loop face must carry an analytic zero proof")
    };
    assert_eq!(zero.source(), &key([0]));
    assert!(matches!(
        zero.proof(),
        CertifiedZeroReductionProof::Analytic(_)
    ));
    let certificate = zero.certificate().unwrap();
    assert_eq!(
        certificate.raw_sector(),
        &SectorMask::try_new([false]).unwrap()
    );
    certificate.replay(&family).unwrap();
    zero.replay(&family).unwrap();
    assert!(provider.inner().requests.is_empty());

    assert!(matches!(
        provider.decision_for(&key([1])).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
    ));
    assert_eq!(provider.inner().requests, [key([1])]);
}

#[test]
fn analytic_zero_preempts_even_an_accidentally_selected_zero_key() {
    let family = family();
    let master_policy =
        MasterPolicyProvider::with_selected(StubProvider::new(1), [key([0]), key([1])]).unwrap();
    let mut provider = CertifiedZeroSectorRuleProvider::try_unrestricted(
        &family,
        PowerShiftPolicy::FormalGeneric,
        master_policy,
        CertifiedRewriteLimits::default(),
    )
    .unwrap();

    let ConcreteRuleDecision::ProvedZero(zero) = provider.decision_for(&key([0])).unwrap() else {
        panic!("analytic zero must take precedence over an accidental master selection")
    };
    zero.replay(&family).unwrap();
    assert!(provider.inner().inner().requests.is_empty());

    assert!(matches!(
        provider.decision_for(&key([1])).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::SelectedMaster)
    ));
    assert!(provider.inner().inner().requests.is_empty());
}

#[test]
fn cut_exclusion_is_a_replayable_zero_but_pattern_exclusion_is_not() {
    let family = family();
    let cut_restrictions = SectorRestrictions::try_new(
        CutConstraint::try_from_positions(1, [0]).unwrap(),
        SectorPattern::any(1).unwrap(),
    )
    .unwrap();
    let mut cut = CertifiedZeroSectorRuleProvider::try_new(
        &family,
        cut_restrictions,
        PowerShiftPolicy::FormalGeneric,
        StubProvider::new(1),
        CertifiedRewriteLimits::default(),
    )
    .unwrap();
    let ConcreteRuleDecision::ProvedZero(zero) = cut.decision_for(&key([0])).unwrap() else {
        panic!("violating a required cut must produce a cut-zero proof")
    };
    assert!(matches!(
        zero.proof(),
        CertifiedZeroReductionProof::Cut { .. }
    ));
    assert!(zero.certificate().is_none());
    zero.replay(&family).unwrap();
    assert!(cut.inner().requests.is_empty());

    let pattern_restrictions = SectorRestrictions::try_new(
        CutConstraint::none(1).unwrap(),
        SectorPattern::try_from_string("1").unwrap(),
    )
    .unwrap();
    let mut pattern = CertifiedZeroSectorRuleProvider::try_new(
        &family,
        pattern_restrictions,
        PowerShiftPolicy::FormalGeneric,
        StubProvider::new(1),
        CertifiedRewriteLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        pattern.decision_for(&key([0])),
        Err(CertifiedZeroSectorRuleProviderError::PatternExcludedSector {
            source,
            exclusion,
        }) if source == key([0])
            && !exclusion.violates_cut()
            && exclusion.violates_pattern()
    ));
    assert!(pattern.inner().requests.is_empty());
}

#[test]
fn construction_request_and_mutated_inner_arities_fail_typed_before_delegation() {
    let family = family();
    assert!(matches!(
        CertifiedZeroSectorRuleProvider::try_new(
            &family,
            SectorRestrictions::unrestricted(2).unwrap(),
            PowerShiftPolicy::FormalGeneric,
            StubProvider::new(1),
            CertifiedRewriteLimits::default(),
        ),
        Err(
            CertifiedZeroSectorRuleProviderError::WrongRestrictionsArity {
                expected: 1,
                actual: 2,
            }
        )
    ));
    assert!(matches!(
        CertifiedZeroSectorRuleProvider::try_unrestricted(
            &family,
            PowerShiftPolicy::FormalGeneric,
            StubProvider::new(2),
            CertifiedRewriteLimits::default(),
        ),
        Err(CertifiedZeroSectorRuleProviderError::WrongProviderArity {
            expected: 1,
            actual: 2,
        })
    ));

    let mut wrong_request = CertifiedZeroSectorRuleProvider::try_unrestricted(
        &family,
        PowerShiftPolicy::FormalGeneric,
        StubProvider::new(1),
        CertifiedRewriteLimits::default(),
    )
    .unwrap();
    assert!(matches!(
        wrong_request.decision_for(&key([1, 1])),
        Err(CertifiedZeroSectorRuleProviderError::WrongArity {
            expected: 1,
            actual: 2,
        })
    ));
    assert!(wrong_request.inner().requests.is_empty());

    let mut changed = CertifiedZeroSectorRuleProvider::try_unrestricted(
        &family,
        PowerShiftPolicy::FormalGeneric,
        StubProvider::new(1),
        CertifiedRewriteLimits::default(),
    )
    .unwrap();
    changed.inner_mut().arity = 2;
    assert!(matches!(
        changed.decision_for(&key([1])),
        Err(CertifiedZeroSectorRuleProviderError::ProviderArityChanged {
            expected: 1,
            actual: 2,
        })
    ));
    assert!(changed.inner().requests.is_empty());
}

#[test]
fn zero_analysis_resource_failure_is_typed_and_never_falls_through() {
    let family = family();
    let mut limits = CertifiedRewriteLimits::default();
    limits.zero_sector.max_rank_columns = 0;
    let mut provider = CertifiedZeroSectorRuleProvider::try_unrestricted(
        &family,
        PowerShiftPolicy::FormalGeneric,
        StubProvider::new(1),
        limits,
    )
    .unwrap();
    assert!(matches!(
        provider.decision_for(&key([0])),
        Err(CertifiedZeroSectorRuleProviderError::ZeroResource {
            resource: "rank matrix columns",
            requested: 1,
            limit: 0,
        })
    ));
    assert!(provider.inner().requests.is_empty());
}
