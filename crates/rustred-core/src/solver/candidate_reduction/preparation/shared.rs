//! Common admission for legacy single-owner and immutable owner-library users.
use super::super::model::{CandidateReductionError, PreparedRule, PreparedTerm};
use crate::algebra::{IndexedCoefficientContext, IndexedPolynomial};
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::SourcePortAudit;
use crate::identity::ParametricIbpGenerator;
use crate::reduction::ReductionLimits;
use crate::sector::zero;
use crate::solver::{Integral, SectorRule, SectorSolution, SourceSystem};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

#[cfg(test)]
std::thread_local! { pub(in crate::solver::candidate_reduction) static PREPARATION_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }

#[derive(Debug)]
pub(in crate::solver::candidate_reduction) struct PreparedFamily<const N: usize> {
    pub context: IndexedCoefficientContext,
    pub index_variables: [usize; N],
    pub source_conditions: Vec<IndexedPolynomial>,
    pub zero_sectors: BTreeSet<[bool; N]>,
    pub zero_certificates: Vec<zero::Certificate>,
    pub sources: Arc<SourceSystem<N>>,
}
pub(in crate::solver::candidate_reduction) fn prepare_family<const N: usize>(
    family: &IntegralFamily,
    zero_certificates: Vec<zero::Certificate>,
    limits: ReductionLimits,
) -> Result<PreparedFamily<N>, CandidateReductionError> {
    if N == 0 || family.denominator_count() != N {
        return Err(CandidateReductionError::InvalidInput(format!(
            "candidate arity {N} does not match family arity {}",
            family.denominator_count()
        )));
    }
    SourcePortAudit::<N>::validate_install_family(family)
        .map_err(|e| CandidateReductionError::UnsupportedFamily(e.to_string()))?;
    let generator = ParametricIbpGenerator::try_new(family)
        .map_err(|e| CandidateReductionError::InvalidInput(e.to_string()))?;
    let context = generator.context().clone();
    let sources = SourceSystem::<N>::from_family(family)
        .map_err(|e| CandidateReductionError::InvalidInput(e.to_string()))?;
    #[cfg(test)]
    PREPARATION_COUNT.with(|count| count.set(count.get() + 1));
    let source_conditions = sources
        .conditions()
        .iter()
        .cloned()
        .map(|p| context.admit_native_polynomial_result_with_limits(p, limits.exact_algebra))
        .collect::<Result<Vec<_>, _>>()?;
    let mut zero_sectors = BTreeSet::new();
    for certificate in &zero_certificates {
        if certificate.family_fingerprint() != family.fingerprint()
            || certificate.raw_sector().arity() != N
        {
            return Err(CandidateReductionError::InvalidInput(
                "zero certificate does not belong to the supplied family".into(),
            ));
        }
        for condition in certificate.domain().conditions() {
            let value = condition.polynomial().clone().into();
            family
                .coefficient_context()
                .validate_with_limits(&value, limits.exact_algebra)?;
            if condition.polynomial().is_zero() {
                return Err(CandidateReductionError::InvalidInput(
                    "zero certificate has an identically zero generic-domain condition".into(),
                ));
            }
        }
        let mask = std::array::from_fn(|i| certificate.raw_sector().active_bits()[i]);
        zero_sectors.insert(mask);
    }

    Ok(PreparedFamily {
        context,
        index_variables: *sources.index_variables(),
        source_conditions,
        zero_sectors,
        zero_certificates,
        sources: Arc::new(sources),
    })
}

pub(in crate::solver::candidate_reduction) struct PreparedRecords<const N: usize> {
    pub rules: BTreeMap<[bool; N], Vec<PreparedRule<N>>>,
    pub terminals: BTreeSet<IntegralKey>,
}
pub(in crate::solver::candidate_reduction) fn prepare_records<const N: usize>(
    shared: &PreparedFamily<N>,
    records: BTreeMap<[bool; N], SectorSolution<N>>,
    limits: ReductionLimits,
) -> Result<PreparedRecords<N>, CandidateReductionError> {
    let mut rules = BTreeMap::new();
    let mut terminals = BTreeSet::new();
    let mut ordinal = 0_usize;
    for (sector, solution) in records {
        let (prepared, finite) = prepare_batch(
            shared,
            sector,
            solution.rules,
            solution.finite_residuals,
            limits,
            &mut ordinal,
        )?;
        rules.insert(sector, prepared);
        terminals.extend(finite);
    }
    Ok(PreparedRecords { rules, terminals })
}

/// Shared native conversion for complete records and source-bound partial jobs.
/// This helper confers no completeness or source authority on its caller.
pub(in crate::solver::candidate_reduction) fn prepare_batch<const N: usize>(
    shared: &PreparedFamily<N>,
    sector: [bool; N],
    source_rules: Vec<SectorRule<N>>,
    residuals: Vec<Integral<N>>,
    limits: ReductionLimits,
    ordinal: &mut usize,
) -> Result<(Vec<PreparedRule<N>>, BTreeSet<IntegralKey>), CandidateReductionError> {
    let context = &shared.context;
    let mut terminals = BTreeSet::new();
    if shared.zero_sectors.contains(&sector) {
        return Err(CandidateReductionError::InvalidInput(
            "a solved sector record also has zero-sector evidence".into(),
        ));
    }
    for residual in residuals {
        if residual.powers().iter().any(|p| p.is_symbolic()) {
            return Err(CandidateReductionError::InvalidInput(
                "a symbolic residual is not a finite candidate terminal".into(),
            ));
        }
        let key = IntegralKey::try_new(residual.powers().iter().map(|p| i64::from(p.value())))
            .map_err(crate::reduction::ReductionError::IntegralKey)?;
        if key
            .powers()
            .iter()
            .zip(sector)
            .any(|(&n, active)| (n > 0) != active)
        {
            return Err(CandidateReductionError::InvalidInput(
                "finite candidate terminal does not belong to its sector record".into(),
            ));
        }
        terminals.insert(key);
    }
    let mut prepared = Vec::new();
    for rule in source_rules {
        let candidate = rule.candidate;
        if !candidate.case.is_in_sector(&sector) || candidate.target != candidate.case.integral() {
            return Err(CandidateReductionError::InvalidInput(
                "candidate target is not its canonical case in the supplied sector".into(),
            ));
        }
        let equalities = if let Some(affine) = candidate.case.affine() {
            if affine.index_variables() != &shared.index_variables {
                return Err(CandidateReductionError::InvalidInput(
                    "candidate affine chart has a different index-variable map".into(),
                ));
            }
            affine
                .equations()
                .iter()
                .cloned()
                .map(|p| {
                    context.admit_native_polynomial_result_with_limits(p, limits.exact_algebra)
                })
                .collect::<Result<Vec<_>, _>>()?
        } else {
            Vec::new()
        };
        let exceptions = rule
            .exceptions
            .branches
            .into_iter()
            .map(|branch| {
                branch
                    .into_iter()
                    .map(|p| {
                        context.admit_native_polynomial_result_with_limits(p, limits.exact_algebra)
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut rhs = Vec::new();
        for term in candidate.rhs {
            if term
                .integral
                .powers()
                .iter()
                .zip(candidate.target.powers())
                .any(|(child, target)| child.is_symbolic() != target.is_symbolic())
            {
                return Err(CandidateReductionError::InvalidInput(
                    "candidate RHS changes symbolic versus fixed coordinate layout".into(),
                ));
            }
            let denominator = context.admit_native_polynomial_result_with_limits(
                term.coefficient.denominator.clone(),
                limits.exact_algebra,
            )?;
            let coefficient =
                context.admit_native_result_with_limits(term.coefficient, limits.exact_algebra)?;
            let shift = std::array::from_fn(|i| {
                i64::from(term.integral[i].value()) - i64::from(candidate.target[i].value())
            });
            rhs.push(PreparedTerm {
                shift,
                coefficient,
                denominator,
            });
        }
        prepared.push(PreparedRule {
            ordinal: *ordinal,
            fixed: *candidate.case.fixed(),
            equalities,
            exceptions,
            rhs,
        });
        *ordinal = ordinal.checked_add(1).ok_or_else(|| {
            CandidateReductionError::InvalidInput("candidate rule ordinal overflow".into())
        })?;
    }
    Ok((prepared, terminals))
}
