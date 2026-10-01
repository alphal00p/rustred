//! Persisted, opt-in rule-choice recipe. Exact eligibility is enforced by the
//! core solver; these descriptors only select bounded trials and quality keys.

use rustred::solver::{
    RulePortfolioTrigger, RuleQualityFeature, RuleQualityPriority, RuleQualityThreshold,
    RuleSelectionPolicy, RuleTrialLimits, SourceDiscoveryStrategy,
};
use serde::{Deserialize, Serialize};

use super::{CandidateSourcePriority, strategy_error};
use crate::AppError;

/// The ordered alternatives, quality keys, trigger and budgets all participate
/// in generation checkpoint identity. Quality version 1 is explicit so a later
/// interpretation cannot silently reuse an older generation checkpoint.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CandidateRulePortfolio {
    BoundedPortfolio {
        version: u32,
        alternatives: Vec<CandidateSourcePriority>,
        limits: CandidateRuleTrialLimits,
        quality: Vec<CandidateRuleQualityPriority>,
        trigger: CandidateRulePortfolioTrigger,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRuleTrialLimits {
    pub max_depth: u32,
    pub max_rows: usize,
    pub max_exact_trace_rows: usize,
    pub max_exact_trace_terms: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRuleQualityPriority {
    pub feature: CandidateRuleQualityFeature,
    pub descending: bool,
}

/// The shift-excursion keys are displacement heuristics, not physical rank
/// bounds. No score or trigger can override exact rule/exception admission.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CandidateRuleQualityFeature {
    MaxNumeratorShiftExcursion,
    TotalNumeratorShiftExcursion,
    MaxPositiveShiftExcursion,
    ExceptionalCases,
    AffineExceptionalCases,
    GuardBranches,
    GuardPredicates,
    RhsTerms,
    CoefficientMonomials,
    SourceRows,
    SearchRows,
    TotalPositiveShiftExcursion,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CandidateRulePortfolioTrigger {
    Always,
    AnyAtLeast {
        thresholds: Vec<CandidateRuleQualityThreshold>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRuleQualityThreshold {
    pub feature: CandidateRuleQualityFeature,
    pub minimum: u64,
}

impl CandidateRulePortfolio {
    pub(super) fn validate(
        &self,
        arity: usize,
        sectors: &[Vec<bool>],
        limit: usize,
        entries: &mut usize,
    ) -> Result<(), AppError> {
        let Self::BoundedPortfolio {
            version,
            alternatives,
            ..
        } = self;
        if *version != 1 {
            return Err(AppError::input(
                "unsupported rule portfolio quality version",
            ));
        }
        // Bound descriptor work before iterating materialized inventories. Core
        // validates the policy again before preparing any actual sector basis.
        if !(1..=2).contains(&alternatives.len()) {
            return Err(AppError::input(
                "rule portfolio requires one or two alternatives",
            ));
        }
        for alternative in alternatives {
            alternative.validate(arity, sectors, limit, entries)?;
        }
        // Materialized ordinals were checked above against the complete sector
        // inventory. Their actual basis sizes are checked by the core when
        // solving. A zero-sector inventory is valid and has no plan to select.
        let sources = alternatives
            .iter()
            .map(|alternative| match alternative {
                CandidateSourcePriority::Materialized { .. } => {
                    Ok(SourceDiscoveryStrategy::InputOrder)
                }
                _ => alternative.source_for(&[]),
            })
            .collect::<Result<Vec<_>, _>>()?;
        self.with_sources(sources)
            .validate(arity)
            .map_err(strategy_error)
    }

    pub(super) fn native_for(&self, sector: &[bool]) -> Result<RuleSelectionPolicy, AppError> {
        let Self::BoundedPortfolio { alternatives, .. } = self;
        Ok(self.with_sources(
            alternatives
                .iter()
                .map(|alternative| alternative.source_for(sector))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn with_sources(&self, alternatives: Vec<SourceDiscoveryStrategy>) -> RuleSelectionPolicy {
        let Self::BoundedPortfolio {
            limits,
            quality,
            trigger,
            ..
        } = self;
        RuleSelectionPolicy::BoundedPortfolio {
            alternatives,
            limits: RuleTrialLimits {
                max_depth: limits.max_depth,
                max_rows: limits.max_rows,
                max_exact_trace_rows: limits.max_exact_trace_rows,
                max_exact_trace_terms: limits.max_exact_trace_terms,
            },
            quality: quality
                .iter()
                .map(|priority| RuleQualityPriority {
                    feature: priority.feature.native(),
                    descending: priority.descending,
                })
                .collect(),
            trigger: match trigger {
                CandidateRulePortfolioTrigger::Always => RulePortfolioTrigger::Always,
                CandidateRulePortfolioTrigger::AnyAtLeast { thresholds } => {
                    RulePortfolioTrigger::AnyAtLeast(
                        thresholds
                            .iter()
                            .map(|threshold| RuleQualityThreshold {
                                feature: threshold.feature.native(),
                                minimum: threshold.minimum,
                            })
                            .collect(),
                    )
                }
            },
        }
    }
}

impl CandidateRuleQualityFeature {
    fn native(self) -> RuleQualityFeature {
        match self {
            Self::MaxNumeratorShiftExcursion => RuleQualityFeature::MaxNumeratorShiftExcursion,
            Self::TotalNumeratorShiftExcursion => RuleQualityFeature::TotalNumeratorShiftExcursion,
            Self::MaxPositiveShiftExcursion => RuleQualityFeature::MaxPositiveShiftExcursion,
            Self::ExceptionalCases => RuleQualityFeature::ExceptionalCases,
            Self::AffineExceptionalCases => RuleQualityFeature::AffineExceptionalCases,
            Self::GuardBranches => RuleQualityFeature::GuardBranches,
            Self::GuardPredicates => RuleQualityFeature::GuardPredicates,
            Self::RhsTerms => RuleQualityFeature::RhsTerms,
            Self::CoefficientMonomials => RuleQualityFeature::CoefficientMonomials,
            Self::SourceRows => RuleQualityFeature::SourceRows,
            Self::SearchRows => RuleQualityFeature::SearchRows,
            Self::TotalPositiveShiftExcursion => RuleQualityFeature::TotalPositiveShiftExcursion,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CandidateDiscoveryStrategy, CandidateSourceVisitPlan};

    fn descriptor() -> CandidateDiscoveryStrategy {
        CandidateDiscoveryStrategy::from_json(r#"{
            "version":2,"sectors":{"kind":"active-first"},"rows":{"kind":"input-order"},
            "rule_selection":{"kind":"bounded-portfolio","version":1,
                "alternatives":[{"kind":"input-order"}],
                "limits":{"max_depth":0,"max_rows":4,"max_exact_trace_rows":4,"max_exact_trace_terms":16},
                "quality":[{"feature":"rhs-terms","descending":false}],
                "trigger":{"kind":"any-at-least","thresholds":[{"feature":"source-rows","minimum":3}]}}
        }"#).unwrap()
    }

    #[test]
    fn portfolio_descriptor_roundtrips_and_preserves_v1_default() {
        let value = descriptor();
        value.validate(2, &[vec![true, true]], 100).unwrap();
        let text = serde_json::to_string(&value).unwrap();
        assert_eq!(CandidateDiscoveryStrategy::from_json(&text).unwrap(), value);
        let toml = toml::to_string(&value).unwrap();
        assert_eq!(
            toml::from_str::<CandidateDiscoveryStrategy>(&toml).unwrap(),
            value
        );
        assert_eq!(
            serde_json::to_string(&CandidateDiscoveryStrategy::default()).unwrap(),
            r#"{"version":1,"sectors":{"kind":"active-first"},"rows":{"kind":"input-order"}}"#
        );
        assert!(matches!(
            CandidateDiscoveryStrategy::default()
                .rule_selection_for(&[true])
                .unwrap(),
            RuleSelectionPolicy::FirstValid
        ));
        let RuleSelectionPolicy::BoundedPortfolio {
            limits,
            trigger,
            quality,
            ..
        } = value.rule_selection_for(&[true, true]).unwrap()
        else {
            panic!("missing opt-in")
        };
        assert_eq!(limits.max_depth, 0);
        assert!(
            matches!(trigger, RulePortfolioTrigger::AnyAtLeast(ref values) if values.len() == 1 && values[0].minimum == 3)
        );
        assert!(matches!(quality[0].feature, RuleQualityFeature::RhsTerms));
    }

    #[test]
    fn portfolio_total_positive_roundtrips_and_projects_quality_and_trigger() {
        let mut encoded = serde_json::to_value(descriptor()).unwrap();
        encoded["rule_selection"]["quality"][0]["feature"] =
            "total-positive-shift-excursion".into();
        encoded["rule_selection"]["trigger"]["thresholds"][0]["feature"] =
            "total-positive-shift-excursion".into();
        let value = CandidateDiscoveryStrategy::from_json(&encoded.to_string()).unwrap();
        value.validate(2, &[vec![true, true]], 100).unwrap();
        assert_eq!(serde_json::to_value(&value).unwrap(), encoded);
        assert_eq!(
            toml::from_str::<CandidateDiscoveryStrategy>(&toml::to_string(&value).unwrap())
                .unwrap(),
            value
        );
        let RuleSelectionPolicy::BoundedPortfolio {
            quality, trigger, ..
        } = value.rule_selection_for(&[true, true]).unwrap()
        else {
            panic!("missing opt-in")
        };
        assert_eq!(
            quality[0].feature,
            RuleQualityFeature::TotalPositiveShiftExcursion
        );
        assert!(
            matches!(trigger, RulePortfolioTrigger::AnyAtLeast(ref values)
            if values[0].feature == RuleQualityFeature::TotalPositiveShiftExcursion)
        );
        encoded["rule_selection"]["quality"][0]["feature"] =
            "total-positive-shift-excursion-unknown".into();
        assert!(CandidateDiscoveryStrategy::from_json(&encoded.to_string()).is_err());
    }

    #[test]
    fn portfolio_rejects_ambiguous_versions_unknown_fields_and_invalid_core_policy() {
        let original = serde_json::to_value(descriptor()).unwrap();
        let paths: &[(&[&str], serde_json::Value)] = &[
            (&["version"], 1.into()),
            (&["rule_selection"], serde_json::Value::Null),
            (&["rule_selection", "version"], 2.into()),
            (&["rule_selection", "limits", "max_rows"], 0.into()),
            (&["rule_selection", "alternatives"], serde_json::json!([])),
            (&["rule_selection", "quality"], serde_json::json!([])),
            (
                &["rule_selection", "trigger", "thresholds"],
                serde_json::json!([]),
            ),
            (&["rule_selection", "unknown"], true.into()),
        ];
        for (path, replacement) in paths {
            let mut value = original.clone();
            let mut cursor = &mut value;
            for component in &path[..path.len() - 1] {
                cursor = &mut cursor[*component];
            }
            cursor[path[path.len() - 1]] = replacement.clone();
            let rejected = CandidateDiscoveryStrategy::from_json(&value.to_string())
                .and_then(|value| value.validate(2, &[vec![true, true]], 100));
            assert!(rejected.is_err(), "accepted invalid path {path:?}");
        }
        for field in ["quality", "trigger"] {
            let mut value = original.clone();
            let values = if field == "quality" {
                &mut value["rule_selection"][field]
            } else {
                &mut value["rule_selection"][field]["thresholds"]
            };
            let first = values[0].clone();
            values.as_array_mut().unwrap().push(first);
            assert!(
                CandidateDiscoveryStrategy::from_json(&value.to_string())
                    .unwrap()
                    .validate(2, &[vec![true, true]], 100)
                    .is_err()
            );
        }
    }

    #[test]
    fn portfolio_materialized_plans_share_one_collection_budget_and_exact_inventory() {
        let mut value = descriptor();
        let plan = CandidateSourcePriority::Materialized {
            sectors: vec![CandidateSourceVisitPlan {
                sector: vec![true, true],
                ordinals: vec![1, 0],
            }],
        };
        value.rows = plan.clone();
        let CandidateRulePortfolio::BoundedPortfolio { alternatives, .. } =
            value.rule_selection.as_mut().unwrap();
        *alternatives = vec![plan.clone(), plan];
        value.validate(2, &[vec![true, true]], 12).unwrap();
        assert!(value.validate(2, &[vec![true, true]], 11).is_err());
        assert!(value.validate(2, &[vec![false, true]], 12).is_err());
        let RuleSelectionPolicy::BoundedPortfolio { alternatives, .. } =
            value.rule_selection_for(&[true, true]).unwrap()
        else {
            panic!("missing portfolio")
        };
        assert!(
            alternatives
                .iter()
                .all(|s| matches!(s, SourceDiscoveryStrategy::Materialized(_)))
        );
        assert!(value.rule_selection_for(&[false, true]).is_err());
    }
}
