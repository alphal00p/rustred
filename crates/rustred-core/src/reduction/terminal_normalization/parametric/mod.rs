//! Exact positive-power vacuum equalities by Schwinger-parameter relabeling.
//!
//! A native colored graph proposes a permutation. It is not proof: the full
//! restricted native U polynomial, including its scale, is replayed exactly.
//! For integer momentum squares, unit masses, no external momenta and zero
//! analytic shifts, equal U and matched positive powers identify the complete
//! convergent Euclidean parameter integrands, hence their analytic continuations.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use symbolica::prelude::PolyVariable;

use crate::algebra::ExactAlgebraError;
use crate::family::symanzik::{FeynmanPolynomialError, SymanzikPolynomials};
use crate::family::{IntegralFamily, IntegralKey};
use crate::sector::OrderingPolicy;

use super::{
    ProductSkipReason as Skip, TerminalAliasError as Error, TerminalAliasPlan,
    TerminalAliasStatistics, TerminalAliasWitness, VerifiedTerminalAlias,
};

#[cfg(test)]
mod audit_tests;
mod geometry;
mod model;
mod proposal;
#[cfg(test)]
mod tests;
mod verify;

pub use model::{VacuumParametricLimits, VerifiedVacuumParameterMap};

impl TerminalAliasPlan {
    /// Prepare a new opt-in plan for all eligible positive-power vacuum keys.
    /// Existing momentum-routing factories and reducer defaults are unchanged.
    ///
    /// Every alias points directly to an already declared, strictly smaller
    /// key. Equality of the exact full restricted U polynomial under a
    /// power-preserving parameter bijection proves a unit-coefficient identity;
    /// no integer loop-momentum map or master-minimality claim is implied.
    /// Unsupported geometry or exhausted explicit bounds retains raw keys.
    pub fn vacuum_parametric_equivalences(
        family: &IntegralFamily,
        raw: &BTreeSet<IntegralKey>,
        ordering: OrderingPolicy,
        limits: VacuumParametricLimits,
    ) -> Result<Self, Error> {
        ordering
            .require_arity(family.denominator_count())
            .map_err(|e| Error::Ordering(e.to_string()))?;
        for key in raw {
            if key.powers().len() != family.denominator_count() {
                return Err(Error::WrongArity {
                    expected: family.denominator_count(),
                    actual: key.powers().len(),
                });
            }
        }
        let mut plan = Self {
            family_fingerprint: family.fingerprint_owner(),
            arity: family.denominator_count(),
            ordering,
            raw: raw.clone(),
            canonical: raw.clone(),
            aliases: BTreeMap::new(),
            statistics: TerminalAliasStatistics {
                raw_terminals: raw.len(),
                canonical_terminals: raw.len(),
                ..Default::default()
            },
        };
        if family
            .power_shifts()
            .iter()
            .any(|shift| !family.coefficient_context().contains(shift))
        {
            return Err(Error::InvalidCoefficientContext);
        }
        let skip = if family.external_count() != 0 {
            Some(Skip::ExternalMomenta)
        } else if family.power_shifts().iter().any(|s| !s.is_zero()) {
            Some(Skip::AnalyticPowerShifts)
        } else if family.loop_count() == 0 {
            Some(Skip::ActiveLineCount)
        } else {
            None
        };
        if let Some(skip) = skip {
            plan.statistics.skipped.insert(skip, raw.len());
            return Ok(plan);
        }
        if raw.is_empty() {
            return Ok(plan);
        }
        if limits.max_supports == 0 || limits.max_canonicalizations == 0 {
            plan.statistics
                .skipped
                .insert(Skip::ParametricPreparationLimit, raw.len());
            return Ok(plan);
        }
        let symanzik =
            match SymanzikPolynomials::try_from_family_with_limits(family, limits.symanzik) {
                Ok(value) => value,
                Err(
                    FeynmanPolynomialError::ResourceLimit { .. }
                    | FeynmanPolynomialError::ResourceCountOverflow { .. }
                    | FeynmanPolynomialError::AllocationFailure { .. }
                    | FeynmanPolynomialError::ParameterExponentOverflow { .. }
                    | FeynmanPolynomialError::ExactAlgebra(
                        ExactAlgebraError::ResourceLimit { .. }
                        | ExactAlgebraError::ResourceCountOverflow { .. }
                        | ExactAlgebraError::ExponentLimit { .. }
                        | ExactAlgebraError::ExponentArithmeticOverflow { .. },
                    ),
                ) => {
                    plan.statistics
                        .skipped
                        .insert(Skip::ParametricPreparationLimit, raw.len());
                    return Ok(plan);
                }
                Err(error) => return Err(Error::ExactAlgebra(error.to_string())),
            };
        let variables = Arc::new(
            (0..family.loop_count())
                .map(PolyVariable::Temporary)
                .collect(),
        );
        let mut momenta = vec![None; family.denominator_count()];
        let mut supports: BTreeMap<Vec<usize>, Result<Arc<model::Support>, Skip>> = BTreeMap::new();
        let mut representatives: BTreeMap<
            proposal::ProposalGraph,
            (IntegralKey, Arc<model::Support>, Vec<usize>),
        > = BTreeMap::new();
        let mut ordered = raw
            .iter()
            .map(|key| {
                ordering
                    .complexity_key(key.powers())
                    .map(|complexity| (complexity, key))
                    .map_err(|e| Error::Ordering(e.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        ordered.sort_by(|a, b| a.0.cmp(&b.0));
        for (_, key) in ordered {
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
                if supports.len() >= limits.max_supports {
                    plan.statistics.skip(Skip::ParametricPreparationLimit);
                    continue;
                }
                plan.statistics.analyzed_parametric_supports += 1;
                let support = geometry::prepare(
                    family,
                    &slots,
                    symanzik.u().raw(),
                    &variables,
                    &mut momenta,
                    &mut plan.statistics,
                )?;
                supports.insert(slots.clone(), support);
            }
            let support = match supports.get(&slots).expect("prepared support") {
                Ok(support) => support.clone(),
                Err(reason) => {
                    plan.statistics.skip(*reason);
                    continue;
                }
            };
            plan.statistics.eligible_parametric += 1;
            if plan.statistics.parametric_canonicalizations >= limits.max_canonicalizations {
                plan.statistics.skip(Skip::ParametricPreparationLimit);
                continue;
            }
            let Some(proposal) = proposal::canonicalize(family, key, &support, limits)? else {
                plan.statistics.skip(Skip::ParametricPreparationLimit);
                continue;
            };
            plan.statistics.parametric_canonicalizations += 1;
            if let Some((representative_key, representative, parameters)) =
                representatives.get(&proposal.graph)
            {
                let permutation = proposal
                    .parameters
                    .iter()
                    .map(|vertex| {
                        parameters
                            .iter()
                            .position(|v| v == vertex)
                            .ok_or(Error::InvalidParametricWitness)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if !ordering
                    .compare(representative_key.powers(), key.powers())
                    .map_err(|e| Error::Ordering(e.to_string()))?
                    .is_lt()
                {
                    return Err(Error::InvalidParametricWitness);
                }
                let witness = verify::prove(
                    family,
                    key,
                    representative_key,
                    support,
                    representative.clone(),
                    permutation,
                )?;
                plan.aliases.insert(
                    key.clone(),
                    VerifiedTerminalAlias {
                        representative: representative_key.clone(),
                        witness: TerminalAliasWitness::Parametric(Arc::new(witness)),
                    },
                );
                plan.canonical.remove(key);
            } else {
                representatives.insert(proposal.graph, (key.clone(), support, proposal.parameters));
            }
        }
        plan.statistics.verified_aliases = plan.aliases.len();
        plan.statistics.canonical_terminals = plan.canonical.len();
        Ok(plan)
    }
}
