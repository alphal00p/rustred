use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use crate::algebra::IndexedCoefficientContext;
use crate::family::{IntegralFamily, IntegralKey};
use crate::foundry::artifact::SourcePortAudit;
use crate::identity::ParametricIbpGenerator;
use crate::reduction::{CacheWeight, ReductionLimits, ReductionStatistics, SharedCacheBudget};
use crate::sector::{OrderingPolicy, zero};
use crate::solver::{SectorSolution, SourceSystem};

use super::model::{CandidateReductionError, PreparedRule, PreparedTerm};
use super::reducer::CandidateReducer;

impl<const N: usize> CandidateReducer<N> {
    /// Prepare an explicitly experimental, owned formula applier. The family
    /// admission is the same unit-mass vacuum shape check as publication, but
    /// this constructor does NOT perform original-source replay or cover proof.
    /// `finite_residuals` must contain only concrete fixed integral keys.
    pub fn try_new(
        family: &IntegralFamily,
        root_sector: [bool; N],
        ordering: OrderingPolicy,
        sectors: impl IntoIterator<Item = ([bool; N], SectorSolution<N>)>,
        zero_certificates: Vec<zero::Certificate>,
        limits: ReductionLimits,
    ) -> Result<Self, CandidateReductionError> {
        if N == 0 || family.denominator_count() != N {
            return Err(CandidateReductionError::InvalidInput(format!(
                "candidate arity {N} does not match family arity {}",
                family.denominator_count()
            )));
        }
        SourcePortAudit::<N>::validate_install_family(family)
            .map_err(|e| CandidateReductionError::UnsupportedFamily(e.to_string()))?;
        // Validate even an ordering never exercised by a zero/terminal request.
        ordering
            .compare(&[0; N], &[0; N])
            .map_err(crate::reduction::ReductionError::Ordering)?;
        let generator = ParametricIbpGenerator::try_new(family)
            .map_err(|e| CandidateReductionError::InvalidInput(e.to_string()))?;
        let context = generator.context().clone();
        let sources = SourceSystem::<N>::from_family(family)
            .map_err(|e| CandidateReductionError::InvalidInput(e.to_string()))?;
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
        let mut records = BTreeMap::new();
        for (sector, solution) in sectors {
            if sector.iter().zip(root_sector).any(|(&s, root)| s && !root) {
                return Err(CandidateReductionError::InvalidInput(
                    "candidate sector lies outside the supplied root".into(),
                ));
            }
            if records.insert(sector, solution).is_some() {
                return Err(CandidateReductionError::InvalidInput(
                    "duplicate candidate sector record".into(),
                ));
            }
        }
        let mut rules = BTreeMap::new();
        let mut terminals = BTreeSet::new();
        let mut ordinal = 0_usize;
        for (sector, solution) in records {
            if zero_sectors.contains(&sector) {
                return Err(CandidateReductionError::InvalidInput(
                    "a solved sector record also has zero-sector evidence".into(),
                ));
            }
            for residual in solution.finite_residuals {
                if residual.powers().iter().any(|p| p.is_symbolic()) {
                    return Err(CandidateReductionError::InvalidInput(
                        "a symbolic residual is not a finite candidate terminal".into(),
                    ));
                }
                let key =
                    IntegralKey::try_new(residual.powers().iter().map(|p| i64::from(p.value())))
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
            for rule in solution.rules {
                let candidate = rule.candidate;
                if !candidate.case.is_in_sector(&sector)
                    || candidate.target != candidate.case.integral()
                {
                    return Err(CandidateReductionError::InvalidInput(
                        "candidate target is not its canonical case in the supplied sector".into(),
                    ));
                }
                let equalities = if let Some(affine) = candidate.case.affine() {
                    if affine.index_variables() != sources.index_variables() {
                        return Err(CandidateReductionError::InvalidInput(
                            "candidate affine chart has a different index-variable map".into(),
                        ));
                    }
                    affine
                        .equations()
                        .iter()
                        .cloned()
                        .map(|p| {
                            context
                                .admit_native_polynomial_result_with_limits(p, limits.exact_algebra)
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
                                context.admit_native_polynomial_result_with_limits(
                                    p,
                                    limits.exact_algebra,
                                )
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
                    let coefficient = context
                        .admit_native_result_with_limits(term.coefficient, limits.exact_algebra)?;
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
                    ordinal,
                    fixed: *candidate.case.fixed(),
                    equalities,
                    exceptions,
                    rhs,
                });
                ordinal = ordinal.checked_add(1).ok_or_else(|| {
                    CandidateReductionError::InvalidInput("candidate rule ordinal overflow".into())
                })?;
            }
            rules.insert(sector, prepared);
        }
        Ok(Self {
            family_fingerprint: Arc::new(family.fingerprint().to_owned()),
            context,
            root_sector,
            ordering,
            rules,
            terminals,
            terminal_aliases: None,
            zero_sectors,
            _zero_certificates: zero_certificates,
            source_conditions,
            limits,
            cache: BTreeMap::new(),
            cache_weight: CacheWeight::default(),
            cache_budget: SharedCacheBudget::default(),
            statistics: ReductionStatistics::default(),
        })
    }

    /// The exact family-owned indexed coefficient context, useful for
    /// diagnostics and pure-Rust adapters. It has no closure authority.
    pub fn coefficient_context(&self) -> &IndexedCoefficientContext {
        &self.context
    }
}
