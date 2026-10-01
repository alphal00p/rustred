//! Discovery recipe only. Candidate source IDs and mathematical policy do not change.

use rustred::solver::{
    RuleSelectionPolicy, SectorVisitOrder, SourceDiscoveryStrategy, SourceRowFeature,
    SourceRowPriority, SourceVisitOrder,
};
use serde::{Deserialize, Serialize};

use crate::AppError;

mod portfolio;
pub use portfolio::{
    CandidateRulePortfolio, CandidateRulePortfolioTrigger, CandidateRuleQualityFeature,
    CandidateRuleQualityPriority, CandidateRuleQualityThreshold, CandidateRuleTrialLimits,
};

/// Runtime interpreter input. Typed descriptor identity (including materialized
/// callback output) belongs in the generation manifest, not `solver_policy`.
/// JSON whitespace/key ordering is not identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateDiscoveryStrategy {
    pub version: u32,
    pub sectors: CandidateSectorPriority,
    pub rows: CandidateSourcePriority,
    /// Version 2 only; absence preserves the version-1 first-valid representation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_selection: Option<CandidateRulePortfolio>,
}

impl Default for CandidateDiscoveryStrategy {
    fn default() -> Self {
        Self {
            version: 1,
            sectors: CandidateSectorPriority::ActiveFirst,
            rows: CandidateSourcePriority::InputOrder,
            rule_selection: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CandidateSectorPriority {
    ActiveFirst,
    InputOrder,
    WeightedSupport {
        weights: Vec<u32>,
        descending: bool,
    },
    /// Original canonical sector ordinals, not a mask's binary integer value.
    Materialized {
        ordinals: Vec<usize>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CandidateSourcePriority {
    InputOrder,
    Features {
        priorities: Vec<CandidateRowPriority>,
    },
    /// Exactly one plan for every canonical prepared sector. An opaque Rust
    /// callback is run by the caller once, never serialized or invoked on resume.
    Materialized {
        sectors: Vec<CandidateSourceVisitPlan>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateSourceVisitPlan {
    pub sector: Vec<bool>,
    pub ordinals: Vec<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateRowPriority {
    pub feature: CandidateRowFeature,
    pub descending: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CandidateRowFeature {
    Terms,
    CoefficientMonomials,
    AbsoluteShifts { weights: Vec<u32> },
    PositiveShifts { weights: Vec<u32> },
    NegativeShifts { weights: Vec<u32> },
}

impl CandidateDiscoveryStrategy {
    /// Bounded JSON front end shared by Python-generated files and Rust callers.
    /// Inventory-dependent validation occurs before sector search. Materialized
    /// row counts are checked against each prepared basis before that sector's
    /// solve; independent sectors may already have completed and checkpointed.
    pub fn from_json(text: &str) -> Result<Self, AppError> {
        if text.len() > crate::MAX_INPUT_BYTES {
            return Err(AppError::limit(
                "discovery strategy exceeds input byte limit",
            ));
        }
        serde_json::from_str(text)
            .map_err(|e| AppError::input(format!("invalid discovery strategy: {e}")))
    }

    pub(super) fn validate(
        &self,
        arity: usize,
        sectors: &[Vec<bool>],
        limit: usize,
    ) -> Result<(), AppError> {
        if !matches!(
            (self.version, self.rule_selection.is_some()),
            (1, false) | (2, true)
        ) {
            return Err(AppError::input(
                "discovery version 1 requires no rule portfolio; version 2 requires one",
            ));
        }
        match &self.sectors {
            CandidateSectorPriority::WeightedSupport { weights, .. } => {
                validate_weights(weights, arity)?
            }
            CandidateSectorPriority::Materialized { ordinals } => {
                if ordinals.len() > limit {
                    return Err(AppError::limit("sector plan exceeds collection limit"));
                }
                SectorVisitOrder::new(ordinals.clone(), sectors.len()).map_err(strategy_error)?;
            }
            _ => (),
        }
        let mut entries = 0;
        self.rows.validate(arity, sectors, limit, &mut entries)?;
        if let Some(portfolio) = &self.rule_selection {
            portfolio.validate(arity, sectors, limit, &mut entries)?;
        }
        Ok(())
    }

    pub(super) fn source_for(&self, sector: &[bool]) -> Result<SourceDiscoveryStrategy, AppError> {
        self.rows.source_for(sector)
    }

    pub(super) fn rule_selection_for(
        &self,
        sector: &[bool],
    ) -> Result<RuleSelectionPolicy, AppError> {
        self.rule_selection
            .as_ref()
            .map_or(Ok(RuleSelectionPolicy::FirstValid), |p| {
                p.native_for(sector)
            })
    }

    /// Restrict a full plan to pending work without changing original sector
    /// identities or forgetting completed shards on resume.
    pub(super) fn pending_order<const N: usize>(
        &self,
        pending: &[(usize, [bool; N])],
    ) -> Option<SectorVisitOrder> {
        match &self.sectors {
            CandidateSectorPriority::ActiveFirst => None,
            CandidateSectorPriority::InputOrder => Some(SectorVisitOrder::by_key(
                &pending.iter().map(|(_, s)| *s).collect::<Vec<_>>(),
                |i, _| pending[i].0,
            )),
            CandidateSectorPriority::WeightedSupport {
                weights,
                descending,
            } => {
                let sectors: Vec<_> = pending.iter().map(|(_, s)| *s).collect();
                Some(SectorVisitOrder::by_key(&sectors, |i, sector| {
                    let score: u64 = sector
                        .iter()
                        .zip(weights)
                        .filter(|(active, _)| **active)
                        .map(|(_, w)| u64::from(*w))
                        .sum();
                    (
                        if *descending { u64::MAX - score } else { score },
                        pending[i].0,
                    )
                }))
            }
            CandidateSectorPriority::Materialized { ordinals } => {
                let mut ranks = vec![0usize; ordinals.len()];
                for (rank, &ordinal) in ordinals.iter().enumerate() {
                    ranks[ordinal] = rank;
                }
                let sectors: Vec<_> = pending.iter().map(|(_, s)| *s).collect();
                Some(SectorVisitOrder::by_key(&sectors, |i, _| {
                    ranks[pending[i].0]
                }))
            }
        }
    }
}

impl CandidateSourcePriority {
    fn validate(
        &self,
        arity: usize,
        sectors: &[Vec<bool>],
        limit: usize,
        entries: &mut usize,
    ) -> Result<(), AppError> {
        match self {
            CandidateSourcePriority::InputOrder => (),
            CandidateSourcePriority::Features { .. } => self
                .source_for(&[])?
                .validate(arity)
                .map_err(strategy_error)?,
            CandidateSourcePriority::Materialized { sectors: plans } => {
                if plans.len() != sectors.len()
                    || plans
                        .iter()
                        .zip(sectors)
                        .any(|(plan, sector)| &plan.sector != sector)
                {
                    return Err(AppError::input(
                        "materialized row plans must match every canonical prepared sector in order",
                    ));
                }
                for plan in plans {
                    *entries = entries
                        .checked_add(plan.sector.len())
                        .and_then(|n| n.checked_add(plan.ordinals.len()))
                        .ok_or_else(|| AppError::limit("source plan count overflow"))?;
                    if *entries > limit {
                        return Err(AppError::limit("source plans exceed collection limit"));
                    }
                    // Exact row count is checked against the newly prepared
                    // basis before solving; here reject malformed permutations.
                    SourceVisitOrder::new(plan.ordinals.clone(), plan.ordinals.len())
                        .map_err(strategy_error)?;
                }
            }
        }
        Ok(())
    }

    fn source_for(&self, sector: &[bool]) -> Result<SourceDiscoveryStrategy, AppError> {
        Ok(match self {
            CandidateSourcePriority::InputOrder => SourceDiscoveryStrategy::InputOrder,
            CandidateSourcePriority::Features { priorities } => SourceDiscoveryStrategy::Features(
                priorities
                    .iter()
                    .map(|p| SourceRowPriority {
                        descending: p.descending,
                        feature: match &p.feature {
                            CandidateRowFeature::Terms => SourceRowFeature::Terms,
                            CandidateRowFeature::CoefficientMonomials => {
                                SourceRowFeature::CoefficientMonomials
                            }
                            CandidateRowFeature::AbsoluteShifts { weights } => {
                                SourceRowFeature::AbsoluteShifts(weights.clone())
                            }
                            CandidateRowFeature::PositiveShifts { weights } => {
                                SourceRowFeature::PositiveShifts(weights.clone())
                            }
                            CandidateRowFeature::NegativeShifts { weights } => {
                                SourceRowFeature::NegativeShifts(weights.clone())
                            }
                        },
                    })
                    .collect(),
            ),
            CandidateSourcePriority::Materialized { sectors } => {
                let i = sectors
                    .binary_search_by(|p| p.sector.as_slice().cmp(sector))
                    .map_err(|_| AppError::input("missing materialized source plan"))?;
                SourceDiscoveryStrategy::Materialized(
                    SourceVisitOrder::new(sectors[i].ordinals.clone(), sectors[i].ordinals.len())
                        .map_err(strategy_error)?,
                )
            }
        })
    }
}

fn validate_weights(weights: &[u32], arity: usize) -> Result<(), AppError> {
    if weights.len() != arity
        || weights.iter().all(|&w| w == 0)
        || weights
            .iter()
            .any(|&w| w > SourceDiscoveryStrategy::MAX_WEIGHT)
    {
        return Err(AppError::input(
            "sector weights require exact arity, a nonzero weight, and weights at most 1000000",
        ));
    }
    Ok(())
}

fn strategy_error(error: rustred::solver::SolverError) -> AppError {
    AppError::input(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptor_roundtrips_and_rejects_unknown_fields_and_versions() {
        let strategy = CandidateDiscoveryStrategy {
            sectors: CandidateSectorPriority::WeightedSupport {
                weights: vec![1, 3],
                descending: true,
            },
            rows: CandidateSourcePriority::Features {
                priorities: vec![CandidateRowPriority {
                    feature: CandidateRowFeature::Terms,
                    descending: false,
                }],
            },
            ..Default::default()
        };
        let sectors = vec![vec![false, true], vec![true, false]];
        strategy.validate(2, &sectors, 100).unwrap();
        let json = serde_json::to_string(&strategy).unwrap();
        assert_eq!(
            CandidateDiscoveryStrategy::from_json(&json).unwrap(),
            strategy
        );
        let toml = toml::to_string(&strategy).unwrap();
        assert_eq!(
            toml::from_str::<CandidateDiscoveryStrategy>(&toml).unwrap(),
            strategy
        );
        assert!(
            CandidateDiscoveryStrategy::from_json(&json.replacen(
                "\"version\":1",
                "\"unknown\":1,\"version\":1",
                1
            ))
            .is_err()
        );
        let mut wrong = strategy;
        wrong.version = 99;
        assert!(wrong.validate(2, &sectors, 100).is_err());
    }

    #[test]
    fn resume_restricts_full_sector_order_without_losing_pending_obligations() {
        let strategy = CandidateDiscoveryStrategy {
            sectors: CandidateSectorPriority::Materialized {
                ordinals: vec![3, 1, 0, 2],
            },
            ..Default::default()
        };
        let sectors = vec![
            vec![false, false],
            vec![false, true],
            vec![true, false],
            vec![true, true],
        ];
        strategy.validate(2, &sectors, 100).unwrap();
        // Full IDs 1 and 2 were saved; pending positions are now 0 and 1.
        let pending = [(0, [false, false]), (3, [true, true])];
        assert_eq!(strategy.pending_order(&pending).unwrap().ordinals(), [1, 0]);
    }

    #[test]
    fn materialized_source_plans_require_exact_inventory_and_finite_bijections() {
        let sectors = vec![vec![false, true], vec![true, false]];
        let mut strategy = CandidateDiscoveryStrategy {
            rows: CandidateSourcePriority::Materialized {
                sectors: sectors
                    .iter()
                    .map(|sector| CandidateSourceVisitPlan {
                        sector: sector.clone(),
                        ordinals: vec![1, 0],
                    })
                    .collect(),
            },
            ..Default::default()
        };
        strategy.validate(2, &sectors, 100).unwrap();
        assert!(strategy.validate(2, &sectors, 3).is_err());
        let CandidateSourcePriority::Materialized { sectors: plans } = &mut strategy.rows else {
            unreachable!()
        };
        plans[0].ordinals = vec![0, 0];
        assert!(strategy.validate(2, &sectors, 100).is_err());
        assert!(strategy.validate(2, &sectors[..1], 100).is_err());
    }
}
