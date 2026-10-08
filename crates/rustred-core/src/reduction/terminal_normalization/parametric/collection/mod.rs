//! Cross-family orchestration of the existing exact full-U proof.
mod application;
mod model;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod audit_tests;
pub use model::*;

use super::super::{ProductSkipReason as Skip, TerminalAliasError as Error};
use super::{geometry, model::Support, prepare_symanzik, proposal, verify};
use crate::family::{IntegralFamily, IntegralKey};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use symbolica::prelude::PolyVariable;

fn limit(
    resource: &'static str,
    requested: usize,
    maximum: usize,
) -> Result<(), VacuumFamilyAliasError> {
    if requested > maximum {
        return Err(VacuumFamilyAliasError::ResourceLimit {
            resource,
            requested,
            limit: maximum,
        });
    }
    Ok(())
}

impl VacuumFamilyAliasPlan {
    /// Prepare unit scalar/dotted equalities across authenticated families.
    /// All families must use the same loop count, dimension coefficient and
    /// exact ordered coefficient map. Geometry is checked independently on each
    /// active support. Unsupported keys remain distinct, not guessed aliases.
    ///
    /// IntegralFamily does not encode loop-measure normalization: these
    /// equalities use a common formal measure. Callers must retain any external
    /// family-specific factors and all original source-domain conditions.
    pub fn prepare(
        inventories: &[(Arc<IntegralFamily>, BTreeSet<IntegralKey>)],
        limits: VacuumFamilyAliasLimits,
    ) -> Result<Self, VacuumFamilyAliasError> {
        limit("families", inventories.len(), limits.max_families)?;
        let mut families = BTreeMap::new();
        let mut raw = BTreeSet::new();
        let mut inputs = BTreeMap::new();
        let mut count = 0usize;
        for (family, keys) in inventories {
            count = count.saturating_add(keys.len());
            limit("terminals", count, limits.max_terminals)?;
            if let Some((first, _)) = inventories.first() {
                if family.loop_count() != first.loop_count()
                    || !family
                        .coefficient_context()
                        .has_same_variable_map(first.coefficient_context())
                    || family.dimension() != first.dimension()
                {
                    return Err(VacuumFamilyAliasError::IncompatibleFamilies);
                }
            }
            let fingerprint = family.fingerprint_owner();
            if families
                .insert(fingerprint.clone(), family.clone())
                .is_some()
            {
                return Err(VacuumFamilyAliasError::DuplicateFamily);
            }
            for key in keys {
                if key.powers().len() != family.denominator_count() {
                    return Err(Error::WrongArity {
                        expected: family.denominator_count(),
                        actual: key.powers().len(),
                    }
                    .into());
                }
                raw.insert(VacuumIntegralKey {
                    family: fingerprint.clone(),
                    key: key.clone(),
                });
            }
            inputs.insert(fingerprint, keys);
        }
        let mut plan = Self {
            families,
            canonical: raw.clone(),
            raw,
            aliases: BTreeMap::new(),
            statistics: Default::default(),
        };
        plan.statistics.raw_terminals = plan.raw.len();
        let mut representatives: BTreeMap<
            proposal::ProposalGraph,
            (VacuumIntegralKey, Arc<Support>, Vec<usize>),
        > = BTreeMap::new();
        for (fingerprint, keys) in inputs {
            let family = &plan.families[&fingerprint];
            let skip = if family.external_count() != 0 {
                Some(Skip::ExternalMomenta)
            } else if family.power_shifts().iter().any(|s| !s.is_zero()) {
                Some(Skip::AnalyticPowerShifts)
            } else if family.loop_count() == 0 {
                Some(Skip::ActiveLineCount)
            } else if limits.parametric.max_supports == 0
                || limits.parametric.max_canonicalizations == 0
            {
                Some(Skip::ParametricPreparationLimit)
            } else {
                None
            };
            if let Some(skip) = skip {
                *plan.statistics.skipped.entry(skip).or_default() += keys.len();
                continue;
            }
            if keys.is_empty() {
                continue;
            }
            let Some(symanzik) = prepare_symanzik(family, limits.parametric)? else {
                *plan
                    .statistics
                    .skipped
                    .entry(Skip::ParametricPreparationLimit)
                    .or_default() += keys.len();
                continue;
            };
            let variables = Arc::new(
                (0..family.loop_count())
                    .map(PolyVariable::Temporary)
                    .collect(),
            );
            let mut momenta = vec![None; family.denominator_count()];
            let mut supports: BTreeMap<Vec<usize>, Result<Arc<Support>, Skip>> = BTreeMap::new();
            for key in keys {
                if key.powers().iter().any(|&n| n < 0) {
                    plan.statistics.skip(Skip::NumeratorPowers);
                    continue;
                }
                let slots: Vec<_> = key
                    .powers()
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &n)| (n > 0).then_some(i))
                    .collect();
                if slots.len() < family.loop_count() {
                    plan.statistics.skip(Skip::ActiveLineCount);
                    continue;
                }
                if !supports.contains_key(&slots) {
                    if plan.statistics.analyzed_parametric_supports
                        >= limits.parametric.max_supports
                    {
                        plan.statistics.skip(Skip::ParametricPreparationLimit);
                        continue;
                    }
                    plan.statistics.analyzed_parametric_supports += 1;
                    supports.insert(
                        slots.clone(),
                        geometry::prepare(
                            family,
                            &slots,
                            symanzik.u().raw(),
                            &variables,
                            &mut momenta,
                            &mut plan.statistics,
                        )?,
                    );
                }
                let support = match &supports[&slots] {
                    Ok(s) => s.clone(),
                    Err(reason) => {
                        plan.statistics.skip(*reason);
                        continue;
                    }
                };
                plan.statistics.eligible_parametric += 1;
                if plan.statistics.parametric_canonicalizations
                    >= limits.parametric.max_canonicalizations
                {
                    plan.statistics.skip(Skip::ParametricPreparationLimit);
                    continue;
                }
                let Some(proposal) =
                    proposal::canonicalize(family, key, &support, limits.parametric)?
                else {
                    plan.statistics.skip(Skip::ParametricPreparationLimit);
                    continue;
                };
                plan.statistics.parametric_canonicalizations += 1;
                let source = VacuumIntegralKey {
                    family: fingerprint.clone(),
                    key: key.clone(),
                };
                if let Some((representative, target_support, target_parameters)) =
                    representatives.get(&proposal.graph)
                {
                    let permutation = proposal
                        .parameters
                        .iter()
                        .map(|vertex| {
                            target_parameters
                                .iter()
                                .position(|v| v == vertex)
                                .ok_or(Error::InvalidParametricWitness)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    let target_family = &plan.families[&representative.family];
                    let witness = verify::prove_between(
                        family,
                        target_family,
                        key,
                        &representative.key,
                        support,
                        target_support.clone(),
                        permutation,
                    )?;
                    plan.aliases.insert(
                        source.clone(),
                        VerifiedFamilyVacuumAlias {
                            representative: representative.clone(),
                            witness: Arc::new(witness),
                        },
                    );
                    plan.canonical.remove(&source);
                } else {
                    representatives.insert(proposal.graph, (source, support, proposal.parameters));
                }
            }
        }
        plan.statistics.verified_aliases = plan.aliases.len();
        plan.statistics.canonical_terminals = plan.canonical.len();
        Ok(plan)
    }
}
