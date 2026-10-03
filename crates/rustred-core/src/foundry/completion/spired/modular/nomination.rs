//! Bounded ordinary-source support nomination, never exact rule authority.
//!
//! This adapter borrows one authenticated exact corpus across modular samples.
//! It delegates evaluation and dependency tracing to the existing native lanes.
//! No sampled coefficient, rank or discarded term is an exact identity claim.
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use symbolica::domains::finite_field::{FiniteFieldCore, ToFiniteField};
use symbolica::prelude::Integer;

use crate::algebra::IndexedCoefficientContext;
use crate::foundry::cell::FixedIndexRestriction;
use crate::identity::{CompletedIbpSourceRows, IndexShift, TranslatedSourceRequest};

use super::super::evaluate::{
    DirectShiftedSourceEvaluator, ShiftedModularSourceBuffer, ValidatedDirectShiftedSources,
};
use super::{SpiredForbiddenTerm, SpiredModularKernel, SpiredModularRow, SpiredValidatedPrime};

pub use super::super::evaluate::DirectShiftedSourceError as EvaluationError;
pub use super::super::evaluate::DirectShiftedSourceLimits as CorpusLimits;
pub use super::SpiredModularError as KernelError;
pub use super::SpiredModularLimits as KernelLimits;

/// Existing native work policies plus a checked preparation-coordinate budget.
/// Native scratch still requires outer time/RSS supervision.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub corpus: CorpusLimits,
    pub kernel: KernelLimits,
    /// Aggregate request, F/target, fixed and full translated-term coordinates
    /// inspected before any sample. This is work/storage, not an RSS bound.
    pub max_plan_coordinate_cells: usize,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            corpus: CorpusLimits::default(),
            kernel: KernelLimits::default(),
            max_plan_coordinate_cells: 67_108_864,
        }
    }
}

#[derive(Debug)]
pub enum Error {
    InvalidInput(&'static str),
    Evaluation(EvaluationError),
    Kernel(KernelError),
}
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(s) => f.write_str(s),
            Self::Evaluation(e) => e.fmt(f),
            Self::Kernel(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {}
impl From<EvaluationError> for Error {
    fn from(e: EvaluationError) -> Self {
        Self::Evaluation(e)
    }
}
impl From<KernelError> for Error {
    fn from(e: KernelError) -> Self {
        Self::Kernel(e)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stats {
    pub rows_evaluated: usize,
    pub structural_terms_evaluated: usize,
    pub registered_forbidden_columns: usize,
    pub forbidden_rank: usize,
    pub augmented_rank: usize,
}

/// A source shortlist only. It carries no weights, guard waiver or proof token.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Support {
    /// Dependency-topological order; the target-producing root is last.
    pub requests: Box<[TranslatedSourceRequest]>,
    /// Exact positions in the immutable caller-supplied source visitation.
    pub input_ordinals: Box<[usize]>,
    pub trace_edges: usize,
    pub stats: Stats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    TargetSupport(Support),
    /// An unlucky rank specialization can miss a genuine exact target.
    SampledMiss(Stats),
    UnluckySample {
        input_ordinal: usize,
        cause: EvaluationError,
        stats: Stats,
    },
}

/// Immutable source expressions are borrowed, not cloned or retranslated per
/// prime. The complete exact corpus and every requested structural shift are
/// authenticated once. Each sample owns only native modular scratch/state.
///
/// Fixed restrictions bind the specialized face, not the full application
/// domain. The caller retains its unchanged chart/order, checks any claimed
/// in-chart sample, and independently proves selected sources against COMPLETE
/// F and the full original identity. Caller/family assumptions are not waived.
#[derive(Debug)]
pub struct PreparedOrdinaryNomination<'context, 'sources, 'plan> {
    corpus: ValidatedDirectShiftedSources<'context, 'sources>,
    requests: &'plan [TranslatedSourceRequest],
    input_ordinals: BTreeMap<&'plan TranslatedSourceRequest, usize>,
    forbidden: &'plan BTreeSet<IndexShift>,
    forbidden_by_values: BTreeMap<&'plan [i64], &'plan IndexShift>,
    target: &'plan IndexShift,
    fixed: &'plan [FixedIndexRestriction],
    limits: Limits,
}

impl<'context, 'sources, 'plan> PreparedOrdinaryNomination<'context, 'sources, 'plan> {
    pub fn try_new(
        context: &'context IndexedCoefficientContext,
        sources: &'sources CompletedIbpSourceRows,
        requests: &'plan [TranslatedSourceRequest],
        forbidden: &'plan BTreeSet<IndexShift>,
        target: &'plan IndexShift,
        fixed: &'plan [FixedIndexRestriction],
        limits: Limits,
    ) -> Result<Self, Error> {
        let arity = context.index_count();
        ensure(
            !requests.is_empty() && requests.len() <= limits.kernel.max_rows,
            "empty or oversized nomination bank",
        )?;
        ensure(
            forbidden.len() <= limits.kernel.max_forbidden_columns,
            "nomination F exceeds column limit",
        )?;
        ensure(
            target.values().len() == arity && !forbidden.contains(target),
            "nomination target arity or target in F",
        )?;
        ensure(
            forbidden.iter().all(|s| s.values().len() == arity),
            "nomination F arity differs",
        )?;
        ensure(
            fixed.len() <= arity
                && fixed.windows(2).all(|w| w[0].position() < w[1].position())
                && fixed.iter().all(|f| f.position() < arity),
            "nomination fixed restrictions invalid",
        )?;
        let mut coordinates = 0usize;
        charge(&mut coordinates, requests.len(), arity, limits)?;
        charge(&mut coordinates, forbidden.len(), arity, limits)?;
        charge(&mut coordinates, 1, arity, limits)?;
        charge(&mut coordinates, fixed.len(), 2, limits)?;
        let corpus = ValidatedDirectShiftedSources::try_new(context, sources, limits.corpus)?;
        // Charge/check the ENTIRE bank before creating maps or sampling, even
        // requests later than the eventual hit. No polynomial expression copy.
        for request in requests {
            ensure(
                request.offset().len() == arity,
                "nomination offset arity differs",
            )?;
            let source =
                sources
                    .source_relation(request.source_ordinal())
                    .ok_or(Error::InvalidInput(
                        "nomination ordinary source ordinal out of range",
                    ))?;
            charge(&mut coordinates, source.terms().len(), arity, limits)?;
            for shift in source.terms().keys() {
                for (&a, &b) in request.offset().values().iter().zip(shift.values()) {
                    ensure(
                        a.checked_add(b).is_some(),
                        "nomination structural shift overflow",
                    )?;
                }
            }
        }
        let mut input_ordinals = BTreeMap::new();
        for (ordinal, request) in requests.iter().enumerate() {
            ensure(
                input_ordinals.insert(request, ordinal).is_none(),
                "duplicate nomination source request",
            )?;
        }
        Ok(Self {
            corpus,
            requests,
            input_ordinals,
            forbidden,
            forbidden_by_values: forbidden.iter().map(|s| (s.values(), s)).collect(),
            target,
            fixed,
            limits,
        })
    }

    /// One explicit sample, with no automatic retry or exact fallback. Original
    /// conditions/denominators are evaluated before any row terms. Singular
    /// samples return `UnluckySample`, not an exact miss or source deletion.
    pub fn sample(
        &self,
        modulus: u64,
        base_parameter_residues: &[u64],
        physical_indices: &[i64],
    ) -> Result<Outcome, Error> {
        ensure(
            physical_indices.len() == self.corpus.context().index_count(),
            "nomination sample index arity differs",
        )?;
        ensure(
            self.fixed
                .iter()
                .all(|f| physical_indices[f.position()] == f.value()),
            "nomination sample differs from fixed face",
        )?;
        ensure(
            self.corpus
                .context()
                .base()
                .parameter_names()
                .len()
                .checked_add(physical_indices.len())
                .is_some_and(|count| count <= self.limits.corpus.max_point_coordinates),
            "nomination point coordinate budget exceeded",
        )?;
        let prime = SpiredValidatedPrime::try_new(modulus)?;
        let residues: Vec<_> = physical_indices
            .iter()
            .map(|&n| {
                prime
                    .field()
                    .from_element(&Integer::from(n).to_finite_field(prime.field()))
            })
            .collect();
        let mut evaluator = DirectShiftedSourceEvaluator::try_new_from_validated(
            &self.corpus,
            modulus,
            base_parameter_residues,
            &residues,
        )?;
        let mut kernel =
            SpiredModularKernel::try_new_with_validated_prime(prime, self.limits.kernel)?;
        // No prior rows exist, so historical structural zero is vacuous. Full
        // F remains registered even if every sampled coefficient is zero.
        kernel.try_preregister_historical_zero_columns(
            &self.forbidden.iter().cloned().collect::<Vec<_>>(),
        )?;
        let mut stats = Stats {
            registered_forbidden_columns: self.forbidden.len(),
            ..Stats::default()
        };
        let mut buffer = ShiftedModularSourceBuffer::default();
        for (input_ordinal, request) in self.requests.iter().enumerate() {
            stats.rows_evaluated = input_ordinal + 1;
            if let Err(cause) = evaluator.try_evaluate_request(request, &mut buffer) {
                return match cause {
                    EvaluationError::ConditionZero { .. }
                    | EvaluationError::TermDenominatorZero { .. } => Ok(Outcome::UnluckySample {
                        input_ordinal,
                        cause,
                        stats,
                    }),
                    other => Err(other.into()),
                };
            }
            stats.structural_terms_evaluated = stats
                .structural_terms_evaluated
                .checked_add(buffer.len())
                .ok_or(Error::InvalidInput(
                    "nomination evaluated-term count overflow",
                ))?;
            let forbidden_count = buffer
                .terms()
                .filter(|term| {
                    self.forbidden_by_values
                        .contains_key(term.structural_shift())
                })
                .count();
            ensure(
                forbidden_count <= self.limits.kernel.max_structural_terms_per_row,
                "nomination row F-term budget exceeded",
            )?;
            let mut terms = Vec::new();
            terms.try_reserve_exact(forbidden_count).map_err(|_| {
                Error::Kernel(KernelError::AllocationFailure {
                    resource: "nomination row F terms",
                    requested: forbidden_count,
                })
            })?;
            let mut target_residue = 0;
            for term in buffer.terms() {
                if term.structural_shift() == self.target.values() {
                    target_residue = term.residue();
                } else if let Some(shift) = self.forbidden_by_values.get(term.structural_shift()) {
                    terms.push(SpiredForbiddenTerm::new((*shift).clone(), term.residue()));
                }
            }
            let hit = kernel.try_push_row(SpiredModularRow::new(
                request.clone(),
                terms,
                target_residue,
            ))?;
            stats.forbidden_rank = kernel.forbidden_rank();
            stats.augmented_rank = kernel.augmented_rank();
            if let Some(hit) = hit {
                let ordinals = hit
                    .dependency_order
                    .iter()
                    .map(|request| {
                        self.input_ordinals
                            .get(request)
                            .copied()
                            .ok_or(Error::InvalidInput(
                                "native trace escaped frozen source bank",
                            ))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                return Ok(Outcome::TargetSupport(Support {
                    requests: hit.dependency_order,
                    input_ordinals: ordinals.into_boxed_slice(),
                    trace_edges: hit.dependency_trace.edge_count(),
                    stats,
                }));
            }
        }
        Ok(Outcome::SampledMiss(stats))
    }
}

fn ensure(condition: bool, message: &'static str) -> Result<(), Error> {
    if condition {
        Ok(())
    } else {
        Err(Error::InvalidInput(message))
    }
}
fn charge(total: &mut usize, count: usize, width: usize, limits: Limits) -> Result<(), Error> {
    *total = count
        .checked_mul(width)
        .and_then(|n| total.checked_add(n))
        .filter(|&n| n <= limits.max_plan_coordinate_cells)
        .ok_or(Error::InvalidInput(
            "nomination preparation coordinate budget exceeded",
        ))?;
    Ok(())
}

#[cfg(test)]
mod tests;
