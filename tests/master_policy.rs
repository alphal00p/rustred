use std::collections::BTreeMap;
use std::convert::Infallible;

use rustred::{
    ConcreteIntegralKey, ConcreteRuleDecision, ConcreteRuleProvider, ConcreteTerminalStatus,
    MasterPolicyError, MasterPolicyLimits, MasterPolicyProvider, MasterPolicyTerminal,
};

#[derive(Default)]
struct UncoveredProvider {
    arity: usize,
    queries: usize,
}

impl ConcreteRuleProvider for UncoveredProvider {
    type Error = Infallible;

    fn index_arity(&self) -> usize {
        self.arity
    }

    fn decision_for(
        &mut self,
        _: &ConcreteIntegralKey,
    ) -> Result<ConcreteRuleDecision, Self::Error> {
        self.queries += 1;
        Ok(ConcreteRuleDecision::Terminal(
            ConcreteTerminalStatus::Uncovered,
        ))
    }
}

fn key(values: impl IntoIterator<Item = i64>) -> ConcreteIntegralKey {
    ConcreteIntegralKey::try_new(values).unwrap()
}

#[test]
fn selected_and_certified_masters_are_explicit_and_bypass_discovery() {
    let selected = key([1]);
    let certified = key([2]);
    let uncovered = key([3]);
    let mut provider = MasterPolicyProvider::try_new(
        UncoveredProvider {
            arity: 1,
            queries: 0,
        },
        [
            (selected.clone(), MasterPolicyTerminal::Selected),
            (
                certified.clone(),
                MasterPolicyTerminal::Certified {
                    certificate_fingerprint: "proof-v1:abc".into(),
                },
            ),
        ],
        MasterPolicyLimits::default(),
    )
    .unwrap();

    assert!(matches!(
        provider.decision_for(&selected).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::SelectedMaster)
    ));
    assert!(matches!(
        provider.decision_for(&certified).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::CertifiedMaster {
            certificate_fingerprint
        }) if certificate_fingerprint.as_ref() == "proof-v1:abc"
    ));
    assert!(matches!(
        provider.decision_for(&uncovered).unwrap(),
        ConcreteRuleDecision::Terminal(ConcreteTerminalStatus::Uncovered)
    ));
    assert_eq!(provider.inner().queries, 1);
    assert_eq!(provider.total_certificate_fingerprint_bytes(), 12);
}

#[test]
fn policy_construction_is_arity_conflict_and_resource_checked() {
    let provider = UncoveredProvider {
        arity: 2,
        queries: 0,
    };
    assert!(matches!(
        MasterPolicyProvider::with_selected(provider, [key([1])]),
        Err(MasterPolicyError::WrongArity {
            expected: 2,
            actual: 1
        })
    ));

    let mut limits = MasterPolicyLimits::default();
    limits.max_certificate_fingerprint_bytes = 2;
    let provider = UncoveredProvider {
        arity: 1,
        queries: 0,
    };
    assert!(matches!(
        MasterPolicyProvider::try_new(
            provider,
            [(
                key([1]),
                MasterPolicyTerminal::Certified {
                    certificate_fingerprint: "abc".into()
                }
            )],
            limits,
        ),
        Err(MasterPolicyError::ResourceLimit {
            resource: "one master certificate fingerprint bytes",
            requested: 3,
            limit: 2,
        })
    ));

    let provider = UncoveredProvider {
        arity: 1,
        queries: 0,
    };
    let duplicate = key([1]);
    assert!(matches!(
        MasterPolicyProvider::try_new(
            provider,
            [
                (duplicate.clone(), MasterPolicyTerminal::Selected),
                (
                    duplicate,
                    MasterPolicyTerminal::Certified {
                        certificate_fingerprint: "proof".into()
                    }
                )
            ],
            MasterPolicyLimits::default(),
        ),
        Err(MasterPolicyError::ConflictingTerminal { .. })
    ));
}

#[test]
fn wrapped_provider_arity_mutation_is_detected() {
    let mut provider = MasterPolicyProvider::with_selected(
        UncoveredProvider {
            arity: 1,
            queries: 0,
        },
        [key([1])],
    )
    .unwrap();
    provider.inner_mut().arity = 2;
    assert!(matches!(
        provider.decision_for(&key([1])),
        Err(MasterPolicyError::ProviderArityChanged {
            expected: 1,
            actual: 2,
        })
    ));

    // Keep this map construction in the regression to ensure the public
    // terminal type remains orderable enough for deterministic callers.
    let _deterministic = BTreeMap::from([(key([1]), MasterPolicyTerminal::Selected)]);
}
