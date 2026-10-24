//! Reproducible caller resource settings, separate from artifact authority.

use rustred::foundry::artifact::{ArtifactLoadLimits, SourcePortLimits};
use rustred::foundry::parametric::ParametricRuleLimits;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub(super) struct ResourcePolicyOutput {
    // Decimal strings preserve every usize, including values beyond TOML's
    // signed 64-bit integer range. These fields never enter durable bytes.
    #[serde(skip_serializing_if = "Option::is_none")]
    max_domain_bound_endpoint_cells: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_predicate_consistency_work: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_predicate_atoms: Option<String>,
}

impl ResourcePolicyOutput {
    fn from_policy(
        rule: ParametricRuleLimits,
        predicate_diagnostics: bool,
        consistency_work: usize,
        predicate_atoms: usize,
    ) -> Self {
        Self {
            max_domain_bound_endpoint_cells: rule
                .diagnostics_enabled
                .then(|| rule.max_domain_bound_endpoint_cells.to_string()),
            max_predicate_consistency_work: predicate_diagnostics
                .then(|| consistency_work.to_string()),
            max_predicate_atoms: predicate_diagnostics.then(|| predicate_atoms.to_string()),
        }
    }
}

impl From<SourcePortLimits> for ResourcePolicyOutput {
    fn from(limits: SourcePortLimits) -> Self {
        Self::from_policy(
            limits.rule_derivation,
            limits.diagnostics_enabled,
            limits.max_predicate_consistency_work,
            limits.max_predicate_atoms,
        )
    }
}

impl From<ArtifactLoadLimits> for ResourcePolicyOutput {
    fn from(limits: ArtifactLoadLimits) -> Self {
        Self::from_policy(
            limits.rule_derivation,
            limits.diagnostics_enabled,
            limits.max_predicate_consistency_work,
            limits.max_predicate_atoms,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_active_policy_fields_are_reported_for_each_typed_owner() {
        for predicate_diagnostics in [false, true] {
            for rule_diagnostics in [false, true] {
                let mut source = SourcePortLimits::default();
                source.diagnostics_enabled = predicate_diagnostics;
                source.rule_derivation.diagnostics_enabled = rule_diagnostics;
                source.rule_derivation.max_domain_bound_endpoint_cells = usize::MAX;
                source.max_predicate_consistency_work = 0;
                source.max_predicate_atoms = 257;
                let load = ArtifactLoadLimits {
                    diagnostics_enabled: predicate_diagnostics,
                    rule_derivation: source.rule_derivation,
                    max_predicate_consistency_work: 0,
                    max_predicate_atoms: 257,
                    ..Default::default()
                };
                let source = ResourcePolicyOutput::from(source);
                assert_eq!(source, ResourcePolicyOutput::from(load));
                let document: toml::Value =
                    toml::from_str(&toml::to_string(&source).unwrap()).unwrap();
                for (key, enabled, value) in [
                    (
                        "max_domain_bound_endpoint_cells",
                        rule_diagnostics,
                        usize::MAX.to_string(),
                    ),
                    (
                        "max_predicate_consistency_work",
                        predicate_diagnostics,
                        "0".to_owned(),
                    ),
                    (
                        "max_predicate_atoms",
                        predicate_diagnostics,
                        "257".to_owned(),
                    ),
                ] {
                    assert_eq!(
                        document.get(key).and_then(toml::Value::as_str),
                        enabled.then_some(value.as_str())
                    );
                }
            }
        }
    }
}
