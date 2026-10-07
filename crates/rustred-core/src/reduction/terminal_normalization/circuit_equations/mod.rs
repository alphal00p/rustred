//! Finite homogeneous equations from already verified circuit symmetries.
//!
//! This reuses the existing unit-circuit generator preparation, not its
//! rank-one projection or its binding to declared positive normal forms.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use symbolica::prelude::PolyVariable;

use crate::family::{IntegralFamily, IntegralKey};
use crate::reduction::terminal_relations::TerminalEquation;
use crate::sector::Mask;
use crate::sector::symmetry::integral_transport::{self, ExpansionError, Prepared};

use super::{
    ProductSkipReason, TerminalAliasError, TerminalAliasStatistics, TerminalNormalizationError,
    TerminalNormalizationLimits, TerminalNormalizationSkipReason, corank_one, weighted,
};

/// Finite-inventory preparation bounds; transport limits apply to every map.
#[derive(Clone, Copy, Debug)]
pub struct TerminalCircuitLimits {
    pub normalization: TerminalNormalizationLimits,
    pub expansion: integral_transport::ExpansionLimits,
    /// Prospective native multiplication work, summed across the inventory.
    pub max_transport_operations: usize,
    /// Prospective endpoints, summed across the inventory before expansion.
    pub max_transport_endpoints: usize,
}

impl Default for TerminalCircuitLimits {
    fn default() -> Self {
        Self {
            normalization: Default::default(),
            expansion: Default::default(),
            max_transport_operations: 64_000_000,
            max_transport_endpoints: 4_000_000,
        }
    }
}

/// Preparation counts are not ranks or independent-master claims.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerminalCircuitStatistics {
    pub input_keys: usize,
    pub analyzed_supports: usize,
    pub verified_generators: usize,
    pub equation_targets: usize,
    pub equations: usize,
    pub output_terms: usize,
    pub trivial_equations: usize,
    pub transport_operations: usize,
    pub transport_endpoints: usize,
    pub skipped: BTreeMap<TerminalNormalizationSkipReason, usize>,
}

/// Immutable same-family equations for an explicitly supplied finite inventory.
///
/// Full-rank unit-mass vacuum `L+1` supports with unit primitive circuits use
/// their existing verified adjacent circuit permutations and bridge flips.
/// Positive dots are transported with the denominator permutation; arbitrary
/// finite numerators are expanded by the shared native Symbolica service.
/// Every nontrivial row is `I - T(I) = 0`, with all offspring retained even if
/// absent from the supplied inventory. Unknown lookup keys return no rows:
/// this object never recursively discovers targets or changes normalization.
#[derive(Debug)]
pub struct TerminalCircuitEquations {
    family_fingerprint: Arc<String>,
    rows: BTreeMap<IntegralKey, Vec<TerminalEquation>>,
    statistics: TerminalCircuitStatistics,
}

impl TerminalCircuitEquations {
    pub fn prepare(
        family: Arc<IntegralFamily>,
        keys: &BTreeSet<IntegralKey>,
        limits: TerminalCircuitLimits,
    ) -> Result<Self, TerminalNormalizationError> {
        check(
            "circuit input keys",
            keys.len(),
            limits.normalization.max_terminals,
        )?;
        for key in keys {
            if key.powers().len() != family.denominator_count() {
                return Err(TerminalAliasError::WrongArity {
                    expected: family.denominator_count(),
                    actual: key.powers().len(),
                }
                .into());
            }
        }
        let mut plan = Self {
            family_fingerprint: family.fingerprint_owner(),
            rows: BTreeMap::new(),
            statistics: TerminalCircuitStatistics {
                input_keys: keys.len(),
                ..Default::default()
            },
        };
        let family_skip = if family.external_count() != 0 {
            Some(ProductSkipReason::ExternalMomenta)
        } else if family.power_shifts().iter().any(|s| !s.is_zero()) {
            Some(ProductSkipReason::AnalyticPowerShifts)
        } else {
            None
        };
        if let Some(reason) = family_skip {
            plan.statistics.skipped.insert(
                TerminalNormalizationSkipReason::UnsupportedGeometry(reason),
                keys.len(),
            );
            return Ok(plan);
        }
        let active_count = family.loop_count().checked_add(1).ok_or(
            TerminalNormalizationError::InvalidWitness("circuit active count overflow"),
        )?;
        let variables = Arc::new(
            (0..family.loop_count())
                .map(PolyVariable::Temporary)
                .collect(),
        );
        let mut momenta = vec![None; family.denominator_count()];
        let mut geometry_statistics = TerminalAliasStatistics::default();
        let mut supports: BTreeMap<
            Vec<usize>,
            Result<Vec<Prepared>, TerminalNormalizationSkipReason>,
        > = BTreeMap::new();
        for key in keys {
            let slots: Vec<_> = key
                .powers()
                .iter()
                .enumerate()
                .filter_map(|(i, &p)| (p > 0).then_some(i))
                .collect();
            if slots.len() != active_count {
                plan.skip(TerminalNormalizationSkipReason::UnsupportedGeometry(
                    ProductSkipReason::ActiveLineCount,
                ));
                continue;
            }
            if !supports.contains_key(&slots) {
                check(
                    "circuit supports",
                    supports.len().saturating_add(1),
                    limits.normalization.max_supports,
                )?;
                check(
                    "circuit momentum matrix cells",
                    active_count.saturating_mul(family.loop_count()),
                    limits.normalization.max_matrix_cells,
                )?;
                let support = corank_one::proposal::support(
                    &family,
                    &slots,
                    &variables,
                    &mut momenta,
                    &mut geometry_statistics,
                )?;
                let generators = match support {
                    Ok(support) => {
                        weighted::projection::prepare(&family, &support, limits.normalization)?
                    }
                    Err(reason) => {
                        Err(TerminalNormalizationSkipReason::UnsupportedGeometry(reason))
                    }
                };
                let prepared = match generators {
                    Ok(generators) => {
                        let root = Mask::try_from_indices(key.powers()).map_err(algebra)?;
                        let transports = generators
                            .generators()
                            .iter()
                            .map(|map| {
                                integral_transport::compile(
                                    &family,
                                    Arc::clone(&family),
                                    Arc::clone(map),
                                    root.clone(),
                                    root.clone(),
                                    limits.expansion,
                                )
                                .map_err(TerminalNormalizationError::from)
                            })
                            .collect::<Result<Vec<_>, _>>()?;
                        plan.statistics.verified_generators += transports.len();
                        Ok(transports)
                    }
                    Err(reason) => Err(reason),
                };
                plan.statistics.analyzed_supports += 1;
                supports.insert(slots.clone(), prepared);
            }
            let transports = match &supports[&slots] {
                Ok(transports) => transports,
                Err(reason) => {
                    plan.skip(*reason);
                    continue;
                }
            };
            let context = family.coefficient_context();
            let mut equations = Vec::new();
            for transport in transports {
                let mapped = transport
                    .transport_with_usage(key, limits.expansion, |usage| {
                        let operations = reserve(
                            "aggregate circuit transport operations",
                            plan.statistics.transport_operations,
                            usage.operations,
                            limits.max_transport_operations,
                        )?;
                        let endpoints = reserve(
                            "aggregate circuit transport endpoints",
                            plan.statistics.transport_endpoints,
                            usage.endpoints,
                            limits.max_transport_endpoints,
                        )?;
                        // Account for the source term before allocating the
                        // complete weighted expansion or homogeneous row.
                        reserve(
                            "circuit equation output terms",
                            plan.statistics.output_terms,
                            usage.endpoints.saturating_add(1),
                            limits.normalization.max_output_terms,
                        )?;
                        plan.statistics.transport_operations = operations;
                        plan.statistics.transport_endpoints = endpoints;
                        Ok(())
                    })
                    .map_err(TerminalNormalizationError::from)?;
                let mut terms = BTreeMap::from([(key.clone(), context.one())]);
                for endpoint in mapped.terms() {
                    let value = if let Some(old) = terms.get(endpoint.key()) {
                        context.try_sub(old, endpoint.coefficient(), limits.expansion.exact_algebra)
                    } else {
                        context.try_neg(endpoint.coefficient(), limits.expansion.exact_algebra)
                    }
                    .map_err(algebra)?;
                    if value.is_zero() {
                        terms.remove(endpoint.key());
                    } else {
                        terms.insert(endpoint.key().clone(), value);
                    }
                }
                if terms.is_empty() {
                    plan.statistics.trivial_equations += 1;
                } else {
                    plan.statistics.output_terms += terms.len();
                    plan.statistics.equations += 1;
                    // compile admits only rational-constant affine rows,
                    // unconditional unit Jacobians and nonzero constant guards.
                    equations.push(TerminalEquation {
                        terms,
                        nonzero_conditions: Vec::new(),
                    });
                }
            }
            if !equations.is_empty() {
                plan.statistics.equation_targets += 1;
                plan.rows.insert(key.clone(), equations);
            }
        }
        Ok(plan)
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    /// Only keys explicitly present during preparation can supply equations.
    pub fn equations(&self, key: &IntegralKey) -> &[TerminalEquation] {
        self.rows.get(key).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn statistics(&self) -> &TerminalCircuitStatistics {
        &self.statistics
    }

    fn skip(&mut self, reason: TerminalNormalizationSkipReason) {
        *self.statistics.skipped.entry(reason).or_default() += 1;
    }
}

fn reserve(
    resource: &'static str,
    previous: usize,
    next: usize,
    limit: usize,
) -> Result<usize, ExpansionError> {
    let requested = previous
        .checked_add(next)
        .ok_or(ExpansionError::ResourceLimit {
            resource,
            requested: usize::MAX,
            limit,
        })?;
    if requested > limit {
        Err(ExpansionError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(requested)
    }
}

fn check(
    resource: &'static str,
    requested: usize,
    limit: usize,
) -> Result<(), TerminalNormalizationError> {
    if requested > limit {
        Err(TerminalNormalizationError::Limit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn algebra(error: impl std::fmt::Display) -> TerminalNormalizationError {
    TerminalNormalizationError::ExactAlgebra(error.to_string())
}

#[cfg(test)]
mod tests;
