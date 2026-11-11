//! Persisted, opt-in rule-choice recipe. Exact eligibility is enforced by the
//! core solver; these descriptors only select bounded trials and quality keys.

use rustred::solver::{
    CaseSeedSchedule, RulePortfolioTrigger, RuleQualityFeature, RuleQualityPriority,
    RuleQualityThreshold, RuleSelectionPolicy, RuleTrialDescriptor, RuleTrialLimits,
    SourceDiscoveryStrategy,
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
        alternatives: Vec<CandidateRuleTrialDescriptor>,
        limits: CandidateRuleTrialLimits,
        quality: Vec<CandidateRuleQualityPriority>,
        trigger: CandidateRulePortfolioTrigger,
        #[serde(default, skip_serializing_if = "is_false")]
        pre_screen: bool,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRuleTrialLimits {
    pub max_depth: u32,
    pub max_rows: usize,
    pub max_exact_trace_rows: usize,
    pub max_exact_trace_terms: usize,
    #[serde(
        default = "default_alternatives",
        skip_serializing_if = "is_default_alternatives"
    )]
    pub max_alternatives: usize,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_case_millis: Option<u64>,
}

fn default_alternatives() -> usize {
    2
}
fn is_default_alternatives(value: &usize) -> bool {
    *value == 2
}
fn is_false(value: &bool) -> bool {
    !*value
}

/// Legacy source-only JSON is preserved byte-for-byte on serialization. An
/// explicit scheduled descriptor adds actual shifted-seed trials.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CandidateRuleTrialDescriptor {
    Scheduled(CandidateScheduledRuleTrial),
    Legacy(CandidateSourcePriority),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateScheduledRuleTrial {
    pub source_discovery: CandidateSourcePriority,
    pub seed_schedule: CandidateCaseSeedSchedule,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CandidateCaseSeedSchedule {
    Reference,
    ShiftShells {
        first_depth: u32,
        last_depth: u32,
        axis_priority: Vec<usize>,
        positive_first: bool,
    },
}

impl From<CandidateSourcePriority> for CandidateRuleTrialDescriptor {
    fn from(source: CandidateSourcePriority) -> Self {
        Self::Legacy(source)
    }
}
impl CandidateRuleTrialDescriptor {
    fn source(&self) -> &CandidateSourcePriority {
        match self {
            Self::Legacy(source) => source,
            Self::Scheduled(trial) => &trial.source_discovery,
        }
    }
    fn schedule(&self) -> CaseSeedSchedule {
        match self {
            Self::Legacy(_) => CaseSeedSchedule::Reference,
            Self::Scheduled(trial) => match &trial.seed_schedule {
                CandidateCaseSeedSchedule::Reference => CaseSeedSchedule::Reference,
                CandidateCaseSeedSchedule::ShiftShells {
                    first_depth,
                    last_depth,
                    axis_priority,
                    positive_first,
                } => CaseSeedSchedule::ShiftShells {
                    first_depth: *first_depth,
                    last_depth: *last_depth,
                    axis_priority: axis_priority.clone(),
                    positive_first: *positive_first,
                },
            },
        }
    }
    fn native(&self, sector: &[bool]) -> Result<RuleTrialDescriptor, AppError> {
        Ok(RuleTrialDescriptor {
            source_discovery: self.source().source_for(sector)?,
            seed_schedule: self.schedule(),
        })
    }
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
    DistinctRaisedSupportLines,
    PositiveMaxNetInactiveDegreeShift,
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
        limit: Option<usize>,
        entries: &mut usize,
    ) -> Result<(), AppError> {
        let Self::BoundedPortfolio {
            version,
            alternatives,
            limits,
            ..
        } = self;
        if *version != 1 {
            return Err(AppError::input(
                "unsupported rule portfolio quality version",
            ));
        }
        // Bound descriptor work before iterating materialized inventories. Core
        // validates the policy again before preparing any actual sector basis.
        if ![2, 8, 32].contains(&limits.max_alternatives)
            || alternatives.is_empty()
            || alternatives.len() > limits.max_alternatives
        {
            return Err(AppError::input(
                "rule portfolio alternatives must fit an explicit 2, 8 or 32 trial budget",
            ));
        }
        for alternative in alternatives {
            alternative
                .source()
                .validate(arity, sectors, limit, entries)?;
            alternative
                .schedule()
                .validate(arity)
                .map_err(strategy_error)?;
        }
        // Materialized ordinals were checked above against the complete sector
        // inventory. Their actual basis sizes are checked by the core when
        // solving. A zero-sector inventory is valid and has no plan to select.
        let sources = alternatives
            .iter()
            .map(|alternative| {
                let source_discovery = match alternative.source() {
                    CandidateSourcePriority::Materialized { .. } => {
                        Ok(SourceDiscoveryStrategy::InputOrder)
                    }
                    source => source.source_for(&[]),
                }?;
                Ok::<_, AppError>(RuleTrialDescriptor {
                    source_discovery,
                    seed_schedule: alternative.schedule(),
                })
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
                .map(|alternative| alternative.native(sector))
                .collect::<Result<Vec<_>, _>>()?,
        ))
    }

    fn with_sources(&self, alternatives: Vec<RuleTrialDescriptor>) -> RuleSelectionPolicy {
        let Self::BoundedPortfolio {
            limits,
            quality,
            trigger,
            pre_screen,
            ..
        } = self;
        RuleSelectionPolicy::BoundedPortfolio {
            alternatives,
            pre_screen: *pre_screen,
            limits: RuleTrialLimits {
                max_depth: limits.max_depth,
                max_rows: limits.max_rows,
                max_exact_trace_rows: limits.max_exact_trace_rows,
                max_exact_trace_terms: limits.max_exact_trace_terms,
                max_alternatives: limits.max_alternatives,
                max_case_elapsed: limits.max_case_millis.map(std::time::Duration::from_millis),
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
            Self::DistinctRaisedSupportLines => RuleQualityFeature::DistinctRaisedSupportLines,
            Self::PositiveMaxNetInactiveDegreeShift => {
                RuleQualityFeature::PositiveMaxNetInactiveDegreeShift
            }
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
        value.validate(2, &[vec![true, true]], Some(100)).unwrap();
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
        value.validate(2, &[vec![true, true]], Some(100)).unwrap();
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
                .and_then(|value| value.validate(2, &[vec![true, true]], Some(100)));
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
                    .validate(2, &[vec![true, true]], Some(100))
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
        *alternatives = vec![plan.clone().into(), plan.into()];
        value.validate(2, &[vec![true, true]], Some(12)).unwrap();
        assert!(value.validate(2, &[vec![true, true]], Some(11)).is_err());
        assert!(value.validate(2, &[vec![false, true]], Some(12)).is_err());
        let RuleSelectionPolicy::BoundedPortfolio { alternatives, .. } =
            value.rule_selection_for(&[true, true]).unwrap()
        else {
            panic!("missing portfolio")
        };
        assert!(
            alternatives
                .iter()
                .all(|s| matches!(s.source_discovery, SourceDiscoveryStrategy::Materialized(_)))
        );
        assert!(value.rule_selection_for(&[false, true]).is_err());
    }

    #[test]
    fn scheduled_trials_and_explicit_budgets_preserve_legacy_wire_defaults() {
        let old = descriptor();
        let legacy = serde_json::to_value(&old).unwrap();
        assert!(
            legacy["rule_selection"]["limits"]
                .get("max_alternatives")
                .is_none()
        );
        assert!(
            legacy["rule_selection"]["limits"]
                .get("max_case_millis")
                .is_none()
        );
        for count in [2, 8, 32] {
            let mut encoded = legacy.clone();
            encoded["rule_selection"]["limits"]["max_alternatives"] = count.into();
            encoded["rule_selection"]["limits"]["max_case_millis"] = 250.into();
            encoded["rule_selection"]["limits"]["max_depth"] = 2.into();
            encoded["rule_selection"]["alternatives"] = serde_json::json!(vec![
                serde_json::json!({
                    "source_discovery":{"kind":"input-order"},
                    "seed_schedule":{"kind":"shift-shells","first_depth":1,"last_depth":2,
                        "axis_priority":[1,0],"positive_first":false}
                });
                count
            ]);
            encoded["rule_selection"]["quality"] = serde_json::json!([
                {"feature":"rhs-terms","descending":false},
                {"feature":"distinct-raised-support-lines","descending":false},
                {"feature":"positive-max-net-inactive-degree-shift","descending":false}
            ]);
            let parsed = CandidateDiscoveryStrategy::from_json(&encoded.to_string()).unwrap();
            parsed.validate(2, &[vec![true, true]], Some(1000)).unwrap();
            let roundtrip = serde_json::to_value(&parsed).unwrap();
            // Explicit default2 is canonically omitted; no field affects old JSON.
            if count == 2 {
                encoded["rule_selection"]["limits"]
                    .as_object_mut()
                    .unwrap()
                    .remove("max_alternatives");
            }
            assert_eq!(roundtrip, encoded);
            let RuleSelectionPolicy::BoundedPortfolio {
                alternatives,
                limits,
                quality,
                ..
            } = parsed.rule_selection_for(&[true, true]).unwrap()
            else {
                panic!("missing scheduled portfolio")
            };
            assert_eq!(alternatives.len(), count);
            assert_eq!(limits.max_alternatives, count);
            assert_eq!(
                limits.max_case_elapsed,
                Some(std::time::Duration::from_millis(250))
            );
            assert!(matches!(
                alternatives[0].seed_schedule,
                CaseSeedSchedule::ShiftShells {
                    first_depth: 1,
                    last_depth: 2,
                    positive_first: false,
                    ..
                }
            ));
            assert_eq!(
                quality[1].feature,
                RuleQualityFeature::DistinctRaisedSupportLines
            );
            assert_eq!(
                quality[2].feature,
                RuleQualityFeature::PositiveMaxNetInactiveDegreeShift
            );
        }
        let mut invalid = legacy;
        invalid["rule_selection"]["limits"]["max_alternatives"] = 3.into();
        assert!(
            CandidateDiscoveryStrategy::from_json(&invalid.to_string())
                .unwrap()
                .validate(2, &[vec![true, true]], Some(1000))
                .is_err()
        );
    }
}
