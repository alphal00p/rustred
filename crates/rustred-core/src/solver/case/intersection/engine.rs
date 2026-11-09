use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Instant;

use crate::algebra::CoefficientPolynomial;

use super::super::{AffineGeometryError, Case};
use super::{
    CaseIntersectionBudget, CaseIntersectionError, CaseIntersectionFailure, CaseIntersectionLimits,
    CaseIntersectionResult, CaseIntersectionStats, CaseRefinement, CaseRefinementStats,
    CaseResolution, CaseResolutionReason, DeferredRegion, GraphDependent, UnsupportedBranchPolicy,
    bilinear_integer, definite_quadratic, linear_resultant, modular, native,
};
use crate::solver::GeometryError;

mod graph;
mod rank;

/// Ancestry is shared by siblings. A repeated state along one refinement path
/// is incomplete, whereas coincident states on different siblings are harmless.
struct Ancestry<const N: usize> {
    parent: Case<N>,
    equations: Arc<[CoefficientPolynomial]>,
    exclusions: Arc<[CoefficientPolynomial]>,
    previous: Option<Arc<Self>>,
}

// Sharing the ancestry between siblings must not create recursive destruction
// when a valid refinement has more states than the optional diagnostic default.
impl<const N: usize> Drop for Ancestry<N> {
    fn drop(&mut self) {
        let mut previous = self.previous.take();
        while let Some(ancestor) = previous {
            match Arc::try_unwrap(ancestor) {
                Ok(mut ancestor) => previous = ancestor.previous.take(),
                Err(_) => break,
            }
        }
    }
}

struct WorkItem<const N: usize> {
    parent: Case<N>,
    equations: Arc<[CoefficientPolynomial]>,
    ancestry: Option<Arc<Ancestry<N>>>,
    /// Opt-in [`CaseRefinement`] only (always empty otherwise): primitive
    /// irreducible polynomials whose zeros inside this item are owned by an
    /// earlier sibling. The item is responsible only for its points at which
    /// every exclusion is nonzero, and may drop the others.
    exclusions: Arc<[CoefficientPolynomial]>,
    /// G-split rounds that replaced equations on the way to this item.
    graph_rounds: u8,
}

struct Engine<'a, const N: usize> {
    original_parent: Case<N>,
    original_conjunction: Arc<[CoefficientPolynomial]>,
    current: WorkItem<N>,
    indices: &'a [usize; N],
    sector: &'a [bool; N],
    limits: CaseIntersectionLimits,
    max_numerator_rank: Option<u32>,
    stats: CaseIntersectionStats,
    resolutions: Vec<CaseResolution<N>>,
    unsupported: UnsupportedBranchPolicy,
    deferred: Vec<DeferredRegion<N>>,
    refinement: CaseRefinement,
    refinement_stats: CaseRefinementStats,
    /// Some G-split remainder replaced equations: resolved cases must then be
    /// checked against the original conjunction (see [`CaseRefinement`]).
    check_original: bool,
}

// CaseIntersectionError carries the stats (S1/S2 counters included) by value.
#[allow(clippy::result_large_err)]
pub(super) fn intersect<const N: usize>(
    parent: &Case<N>,
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; N],
    sector: &[bool; N],
    limits: CaseIntersectionLimits,
    max_numerator_rank: Option<u32>,
    unsupported: UnsupportedBranchPolicy,
    refinement: CaseRefinement,
) -> Result<CaseIntersectionResult<N>, CaseIntersectionError<N>> {
    let equations: Arc<[CoefficientPolynomial]> = conjunction.to_vec().into();
    let mut engine = Engine {
        original_parent: parent.clone(),
        original_conjunction: equations.clone(),
        current: WorkItem {
            parent: parent.clone(),
            equations,
            ancestry: None,
            exclusions: Arc::from([]),
            graph_rounds: 0,
        },
        indices,
        sector,
        limits,
        max_numerator_rank,
        stats: CaseIntersectionStats::default(),
        resolutions: Vec::new(),
        unsupported,
        deferred: Vec::new(),
        refinement,
        refinement_stats: CaseRefinementStats::default(),
        check_original: false,
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        checkpoint()?;
        native::preflight(parent, conjunction, indices)?;
        engine.run()
    }));
    match outcome {
        Ok(Ok(cases)) => Ok(CaseIntersectionResult {
            cases,
            max_numerator_rank,
            stats: engine.stats,
            resolutions: engine.resolutions,
            deferred: engine.deferred,
            refinement: engine.refinement_stats,
        }),
        Ok(Err(failure)) => Err(engine.error(failure)),
        Err(_) => Err(engine.error(CaseIntersectionFailure::NativeAlgebra)),
    }
}

impl<const N: usize> Engine<'_, N> {
    fn error(&self, failure: CaseIntersectionFailure) -> CaseIntersectionError<N> {
        CaseIntersectionError {
            max_numerator_rank: self.max_numerator_rank,
            limits: self.limits,
            original_parent: self.original_parent.clone(),
            original_conjunction: self.original_conjunction.clone(),
            unresolved_parent: self.current.parent.clone(),
            unresolved_conjunction: self.current.equations.clone(),
            failure,
            stats: self.stats.clone(),
        }
    }

    fn run(&mut self) -> Result<Vec<Case<N>>, CaseIntersectionFailure> {
        let mut pending = Vec::new();
        let mut resolved = Vec::new();
        loop {
            spend(
                &mut self.stats.work_items,
                self.limits.diagnostic_limit(self.limits.max_work_items),
                CaseIntersectionBudget::WorkItems,
            )?;
            self.check_terms(&self.current.equations.clone())?;
            // Also checks an incoming affine domain against the current sector
            // and rejects a mismatched index map even for an empty conjunction.
            let start = Instant::now();
            let admitted = self
                .current
                .parent
                .intersect_in_rank(&[], self.indices, self.sector, self.max_numerator_rank)
                .map_err(CaseIntersectionFailure::Admission);
            self.stats.admission_time += start.elapsed();
            if let Some(parent) = admitted? {
                self.current.parent = parent;
                self.process(&mut pending, &mut resolved)?;
            }
            match pending.pop() {
                Some(next) => self.current = next,
                None => break,
            }
        }
        let start = Instant::now();
        resolved.retain(|case| rank::in_scope(case, self.max_numerator_rank));
        if self.check_original {
            resolved = self.drop_excluded_cases(resolved)?;
        }
        let result = canonical_union(resolved);
        self.stats.union_time += start.elapsed();
        result
    }

    /// Refine the current branch. A branch that no exact step can refine is
    /// still resolved exactly when S1 or S2 applies (see
    /// [`Self::resolve_stuck_branch`]); otherwise the refusal is unchanged,
    /// or, only under [`UnsupportedBranchPolicy::Defer`], the branch is
    /// deferred (S4, [`Self::defer_stuck_branch`]).
    fn process(
        &mut self,
        pending: &mut Vec<WorkItem<N>>,
        resolved: &mut Vec<Case<N>>,
    ) -> Result<(), CaseIntersectionFailure> {
        match self.refine(pending, resolved) {
            Err(
                failure @ (CaseIntersectionFailure::UnsupportedGeometry
                | CaseIntersectionFailure::RepeatedState),
            ) => self.settle_stuck_branch(failure, pending, resolved),
            other => other,
        }
    }

    /// S1, S2, then (opt-in) G-split, then (only under `Defer`) S4 on the
    /// stuck current branch.
    fn settle_stuck_branch(
        &mut self,
        failure: CaseIntersectionFailure,
        pending: &mut Vec<WorkItem<N>>,
        resolved: &mut Vec<Case<N>>,
    ) -> Result<(), CaseIntersectionFailure> {
        match self.resolve_stuck_branch(failure, resolved) {
            Err(
                refusal @ (CaseIntersectionFailure::UnsupportedGeometry
                | CaseIntersectionFailure::RepeatedState),
            ) => {
                if self.refinement.any()
                    && graph::chart_misses_sector(&self.current.parent, self.sector)
                {
                    self.refinement_stats.sector_empty_branches = self
                        .refinement_stats
                        .sector_empty_branches
                        .saturating_add(1);
                    return Ok(());
                }
                let mut dependents = Vec::new();
                if self.refinement.graph_split {
                    match self.graph_split(pending, true)? {
                        graph::GraphOutcome::Pushed => return Ok(()),
                        graph::GraphOutcome::Unchanged(found) => dependents = found,
                    }
                }
                if self.unsupported == UnsupportedBranchPolicy::Defer {
                    self.defer_stuck_branch(refusal, dependents)
                } else {
                    Err(refusal)
                }
            }
            other => other,
        }
    }

    /// S4: keep the stuck current branch (integer points of chart `parent`
    /// where `equations` vanish) out of the resolved union and record it
    /// exactly. Reached only after S1 and S2 returned the original refusal;
    /// resource errors never get here. An empty conjunction cannot be stuck
    /// (refine resolves it); it would keep its refusal rather than defer the
    /// whole chart.
    fn defer_stuck_branch(
        &mut self,
        reason: CaseIntersectionFailure,
        dependents: Vec<GraphDependent>,
    ) -> Result<(), CaseIntersectionFailure> {
        if self.current.equations.is_empty() {
            return Err(reason);
        }
        if !dependents.is_empty() {
            self.refinement_stats.graph_deferred =
                self.refinement_stats.graph_deferred.saturating_add(1);
        }
        let region = DeferredRegion {
            reason,
            parent: self.original_parent.clone(),
            chart: self.current.parent.clone(),
            conjunction: self.current.equations.clone(),
            exclusions: self.current.exclusions.clone(),
            dependents: dependents.into(),
        };
        if !self.deferred.contains(&region) {
            self.deferred.push(region);
        }
        Ok(())
    }

    /// S1 (lift), then S2 (modular emptiness), on the stuck current branch
    /// `self.current` (chart `parent`, conjunction `equations` restricted to it).
    ///
    /// S1: the chart was derived from the original parent by exact admissions
    /// and splits, so it lies in the original parent (checked again here). If
    /// the ORIGINAL conjunction restricts to zero on the chart, every point of
    /// the chart is in the exceptional locus, and the branch is a subset of the
    /// chart: adding the chart keeps the union exactly the locus.
    /// S2: if the branch has no common zero modulo a small prime on the chart
    /// lattice, it has no integer point and is dropped like any empty branch.
    /// If neither applies, or either test fails internally, the original
    /// refusal is returned unchanged (resource limits still propagate).
    fn resolve_stuck_branch(
        &mut self,
        failure: CaseIntersectionFailure,
        resolved: &mut Vec<Case<N>>,
    ) -> Result<(), CaseIntersectionFailure> {
        let reason = match self.stuck_branch_reason() {
            Ok(Some(reason)) => reason,
            Ok(None) => return Err(failure),
            Err(error @ CaseIntersectionFailure::Resources(_)) => return Err(error),
            Err(_) => return Err(failure),
        };
        match reason {
            CaseResolutionReason::Lifted => {
                self.stats.lifted_branches = self.stats.lifted_branches.saturating_add(1);
                resolved.push(self.current.parent.clone());
            }
            CaseResolutionReason::ModularEmpty { .. } => {
                self.stats.modular_empty_branches =
                    self.stats.modular_empty_branches.saturating_add(1);
            }
        }
        self.resolutions.push(CaseResolution {
            reason,
            replaced: failure,
            chart: self.current.parent.clone(),
            conjunction: self.current.equations.clone(),
        });
        Ok(())
    }

    fn stuck_branch_reason(
        &mut self,
    ) -> Result<Option<CaseResolutionReason>, CaseIntersectionFailure> {
        let start = Instant::now();
        let lifted = original_vanishes_on(
            &self.original_parent,
            &self.original_conjunction,
            &self.current.parent,
            self.indices,
        );
        self.stats.restriction_time += start.elapsed();
        self.stats.restrictions = self.stats.restrictions.saturating_add(1);
        if lifted? {
            return Ok(Some(CaseResolutionReason::Lifted));
        }
        let prime = modular::integer_empty_prime(
            &self.current.parent,
            &self.current.equations,
            self.indices,
        )?;
        Ok(prime.map(|prime| CaseResolutionReason::ModularEmpty { prime }))
    }

    fn refine(
        &mut self,
        pending: &mut Vec<WorkItem<N>>,
        resolved: &mut Vec<Case<N>>,
    ) -> Result<(), CaseIntersectionFailure> {
        if !rank::in_scope(&self.current.parent, self.max_numerator_rank) {
            return Ok(());
        }
        // Public preflight, incoming-domain emptiness and work/term budgets
        // have already been checked. Keep the common coordinate/affine lane
        // on its existing native admission path, without primitive content,
        // sorting, chart-restriction copies, F4 or factorization here.
        if self.current.equations.is_empty() {
            resolved.push(self.current.parent.clone());
            return Ok(());
        }
        if self.current.equations.iter().all(native::is_affine) {
            let start = Instant::now();
            let child = self
                .current
                .parent
                .intersect_in_rank(
                    &self.current.equations,
                    self.indices,
                    self.sector,
                    self.max_numerator_rank,
                )
                .map_err(CaseIntersectionFailure::Admission);
            self.stats.admission_time += start.elapsed();
            self.stats.affine_admissions = self.stats.affine_admissions.saturating_add(1);
            if let Some(child) = child? {
                resolved.push(child);
            }
            return Ok(());
        }
        let mut normalized = false;
        // One elimination-order attempt per work item, including any in-place
        // chart strengthening. Strict queued children share ancestry/budgets
        // but may merit a new attempt on their smaller exact conjunction.
        let mut lex_attempted = false;
        // Opt-in refinement: one factor-first pass and one G-split attempt
        // per state before its normalization (reset whenever it changes).
        let mut factor_checked = false;
        let mut graph_checked = false;
        loop {
            checkpoint()?;
            if !rank::in_scope(&self.current.parent, self.max_numerator_rank) {
                return Ok(());
            }
            let start = Instant::now();
            let restricted =
                native::restrict(&self.current.parent, &self.current.equations, self.indices);
            self.stats.restriction_time += start.elapsed();
            self.stats.restrictions = self.stats.restrictions.saturating_add(1);
            let equations = restricted?;
            self.check_terms(&equations)?;
            self.current.equations = equations.into();
            if self
                .current
                .equations
                .iter()
                .any(|equation| equation.is_constant())
            {
                return Ok(());
            }
            if self.current.equations.is_empty() {
                resolved.push(self.current.parent.clone());
                return Ok(());
            }

            let (affine, nonlinear): (Vec<_>, Vec<_>) = self
                .current
                .equations
                .iter()
                .cloned()
                .partition(native::is_affine);
            if !affine.is_empty() {
                // Never pass mixed nonlinear equations into the existing
                // singleton service: its old coordinate fallback would run a
                // second GB and can discard a useful coupled transformed basis.
                let start = Instant::now();
                let child = self
                    .current
                    .parent
                    .intersect_in_rank(&affine, self.indices, self.sector, self.max_numerator_rank)
                    .map_err(CaseIntersectionFailure::Admission);
                self.stats.admission_time += start.elapsed();
                self.stats.affine_admissions = self.stats.affine_admissions.saturating_add(1);
                let child = match child {
                    Err(
                        error @ CaseIntersectionFailure::Admission(AffineGeometryError::Coordinate(
                            GeometryError::CompactOverflow { .. },
                        )),
                    ) => {
                        // An affine proposal may be incompatible with another
                        // conjunct before its integer fits compact storage.
                        // Only exact inconsistency of the WHOLE restricted AND
                        // can discharge that overflow; never clip the index.
                        if self.compact_overflow_conjunction_is_empty()? {
                            return Ok(());
                        }
                        return Err(error);
                    }
                    other => other?,
                };
                let Some(child) = child else {
                    return Ok(());
                };
                if child == self.current.parent {
                    // Restriction should already have removed any equation
                    // implied by the current chart. Do not loop on a failure
                    // to absorb a nonzero affine equation.
                    return Err(CaseIntersectionFailure::RepeatedState);
                }
                self.current.parent = child;
                self.current.equations = nonlinear.into();
                normalized = false;
                factor_checked = false;
                graph_checked = false;
                continue;
            }

            // When every positive original coordinate is fixed, a small
            // rank simplex can be cheaper than symbolic elimination or an
            // exhaustive signed-divisor pass. This only changes refinement
            // priority: preserve the existing full-conjunction rank splitter.
            if self.prefer_finite_rank_split(pending.len())
                && self.split_rank_coordinate(pending)?
            {
                return Ok(());
            }

            if !normalized && self.refinement.any() {
                if self.refinement.factor_first && !factor_checked {
                    match self.refined_factor_pass(pending)? {
                        FactorStep::Consumed => return Ok(()),
                        FactorStep::Replaced => continue,
                        FactorStep::Unchanged => factor_checked = true,
                    }
                }
                if self.refinement.graph_split && !graph_checked {
                    graph_checked = true;
                    if let graph::GraphOutcome::Pushed = self.graph_split(pending, false)? {
                        return Ok(());
                    }
                }
            }

            if !normalized {
                self.remember_state()?;
                if self.current.equations.len() > 1 {
                    spend(
                        &mut self.stats.normalizations,
                        self.limits.diagnostic_limit(self.limits.max_normalizations),
                        CaseIntersectionBudget::Normalizations,
                    )?;
                    let start = Instant::now();
                    let saturating =
                        self.refinement.saturate && !self.current.exclusions.is_empty();
                    let basis = if saturating {
                        native::saturate(&self.current.equations, &self.current.exclusions)
                    } else {
                        native::normalize(&self.current.equations)
                    };
                    if saturating {
                        self.refinement_stats.saturations =
                            self.refinement_stats.saturations.saturating_add(1);
                        self.refinement_stats.saturation_time += start.elapsed();
                    }
                    self.stats.normalization_time += start.elapsed();
                    let basis = basis?;
                    self.check_terms(&basis)?;
                    self.current.equations = basis.into();
                    normalized = true;
                    // The complete coupled basis is retained and receives
                    // affine admission before any factor/unsupported decision.
                    continue;
                }
            }

            if self.refinement.any() {
                match self.refined_factor_pass(pending)? {
                    FactorStep::Consumed => return Ok(()),
                    FactorStep::Replaced => {
                        normalized = false;
                        factor_checked = false;
                        graph_checked = false;
                        continue;
                    }
                    FactorStep::Unchanged => {}
                }
            }
            for position in 0..self.current.equations.len() {
                if self.refinement.any() {
                    // The exclusion-aware pass above already factored them.
                    break;
                }
                spend(
                    &mut self.stats.factorizations,
                    self.limits.diagnostic_limit(self.limits.max_factorizations),
                    CaseIntersectionBudget::Factorizations,
                )?;
                let start = Instant::now();
                let factors = native::factors(&self.current.equations[position]);
                self.stats.factorization_time += start.elapsed();
                let mut factors = factors?;
                self.check_terms(&factors)?;
                native::retain_possible_integer_factors(&mut factors, self.indices);
                // An empty validated factor list proves this AND branch empty;
                // other pending OR siblings still have to finish successfully.
                if factors.len() == 1 && factors[0] == self.current.equations[position] {
                    continue;
                }
                // Refine exactly ONE equation. Every factor branch keeps all
                // sibling equations and the entire integer-coordinate parent.
                // Check the work budget before retaining a potentially broad
                // union; no Cartesian product is materialized eagerly.
                if self.limits.diagnostics_enabled
                    && self
                        .stats
                        .work_items
                        .checked_add(pending.len())
                        .and_then(|count| count.checked_add(factors.len()))
                        .is_none_or(|count| count > self.limits.max_work_items)
                {
                    return Err(CaseIntersectionFailure::Budget {
                        kind: CaseIntersectionBudget::WorkItems,
                        limit: self.limits.max_work_items,
                    });
                }
                self.stats.factor_children =
                    self.stats.factor_children.saturating_add(factors.len());
                for factor in factors.into_iter().rev() {
                    let mut child_equations = self.current.equations.to_vec();
                    child_equations[position] = factor;
                    native::canonicalize(&mut child_equations);
                    pending.push(WorkItem {
                        parent: self.current.parent.clone(),
                        equations: child_equations.into(),
                        ancestry: self.current.ancestry.clone(),
                        exclusions: self.current.exclusions.clone(),
                        graph_rounds: self.current.graph_rounds,
                    });
                }
                return Ok(());
            }
            // An impossible equation empties only this AND branch. A definite
            // zero minimum instead implies native affine gradient equations.
            // Admit those constraints, then restrict the ENTIRE conjunction
            // again; dropping its other equations would create false cases.
            // Pending OR siblings keep their original parent and equations.
            // The cold proof has no separate stats clock; enclosing sector
            // geometry/solve elapsed time still includes all of its work.
            let mut affine_refinement = None;
            for equation in self.current.equations.iter() {
                checkpoint()?;
                match definite_quadratic::classify_with_policy(
                    &self.current.parent,
                    equation,
                    self.indices,
                    self.sector,
                    self.limits.diagnostics_enabled,
                )? {
                    definite_quadratic::QuadraticProof::Empty => return Ok(()),
                    definite_quadratic::QuadraticProof::EquivalentAffine(gradients) => {
                        affine_refinement = Some(gradients);
                        break;
                    }
                    definite_quadratic::QuadraticProof::Unknown => {}
                }
                checkpoint()?;
            }
            if let Some(gradients) = affine_refinement {
                // This transition consumes a new refinement state even though
                // it stays in the current AND branch. Charge before admission
                // or mutation, just as for a new factor-generated work item.
                spend(
                    &mut self.stats.work_items,
                    self.limits.diagnostic_limit(self.limits.max_work_items),
                    CaseIntersectionBudget::WorkItems,
                )?;
                self.check_terms(&gradients)?;
                let start = Instant::now();
                let child = self
                    .current
                    .parent
                    .intersect_in_rank(
                        &gradients,
                        self.indices,
                        self.sector,
                        self.max_numerator_rank,
                    )
                    .map_err(CaseIntersectionFailure::Admission);
                self.stats.admission_time += start.elapsed();
                self.stats.affine_admissions = self.stats.affine_admissions.saturating_add(1);
                let Some(child) = child? else {
                    return Ok(());
                };
                if child == self.current.parent {
                    // A nonzero restricted definite quadratic must narrow the
                    // chart. Never erase it or loop if admission cannot do so.
                    return Err(CaseIntersectionFailure::RepeatedState);
                }
                self.current.parent = child;
                normalized = false;
                factor_checked = false;
                graph_checked = false;
                continue;
            }
            // Preserve the complete existing bilinear pass before spending
            // any resultant work on a different equation of this conjunction.
            for is_bilinear in [true, false] {
                for equation in self.current.equations.iter() {
                    let branches = if is_bilinear {
                        bilinear_integer::refine(
                            equation,
                            self.indices,
                            self.limits,
                            &mut self.stats,
                            pending.len(),
                        )?
                    } else {
                        linear_resultant::refine(
                            equation,
                            self.indices,
                            self.limits,
                            &mut self.stats,
                            pending.len(),
                        )?
                    };
                    let Some(branches) = branches else {
                        continue;
                    };
                    if self.limits.diagnostics_enabled
                        && self
                            .stats
                            .work_items
                            .checked_add(pending.len())
                            .and_then(|n| n.checked_add(branches.len()))
                            .is_none_or(|n| n > self.limits.max_work_items)
                    {
                        return Err(CaseIntersectionFailure::Budget {
                            kind: CaseIntersectionBudget::WorkItems,
                            limit: self.limits.max_work_items,
                        });
                    }
                    if is_bilinear {
                        self.stats.bilinear_children =
                            self.stats.bilinear_children.saturating_add(branches.len());
                    } else {
                        self.stats.resultant_children =
                            self.stats.resultant_children.saturating_add(branches.len());
                    }
                    for affine in branches.into_iter().rev() {
                        // Keep the selected original equation too: native
                        // restriction will replay it on the admitted child.
                        let mut equations = self.current.equations.to_vec();
                        equations.extend(affine);
                        native::canonicalize(&mut equations);
                        pending.push(WorkItem {
                            parent: self.current.parent.clone(),
                            equations: equations.into(),
                            ancestry: self.current.ancestry.clone(),
                            exclusions: self.current.exclusions.clone(),
                            graph_rounds: self.current.graph_rounds,
                        });
                    }
                    return Ok(());
                }
            }
            if self.split_rank_coordinate(pending)? {
                return Ok(());
            }
            if !lex_attempted
                && native::lex_eligible(&self.current.equations, self.limits.diagnostics_enabled)
            {
                lex_attempted = true;
                spend(
                    &mut self.stats.normalizations,
                    self.limits.diagnostic_limit(self.limits.max_normalizations),
                    CaseIntersectionBudget::Normalizations,
                )?;
                let start = Instant::now();
                let basis = native::normalize_lex(&self.current.equations);
                self.stats.normalization_time += start.elapsed();
                let basis = basis?;
                self.check_terms(&basis)?;
                if basis.as_slice() == self.current.equations.as_ref() {
                    return Err(CaseIntersectionFailure::UnsupportedGeometry);
                }
                self.current.equations = basis.into();
                // Preserve the elimination generators long enough for the
                // existing exact admission/factor services to use them. An
                // immediate Grev normalization would hide them again.
                normalized = true;
                continue;
            }
            return Err(CaseIntersectionFailure::UnsupportedGeometry);
        }
    }

    fn compact_overflow_conjunction_is_empty(&mut self) -> Result<bool, CaseIntersectionFailure> {
        spend(
            &mut self.stats.normalizations,
            self.limits.diagnostic_limit(self.limits.max_normalizations),
            CaseIntersectionBudget::Normalizations,
        )?;
        let start = Instant::now();
        let basis = native::normalize(&self.current.equations);
        self.stats.normalization_time += start.elapsed();
        let basis = basis?;
        self.check_terms(&basis)?;
        Ok(basis
            .iter()
            .any(|equation| equation.is_constant() && !equation.is_zero()))
    }

    fn check_terms(
        &mut self,
        equations: &[CoefficientPolynomial],
    ) -> Result<(), CaseIntersectionFailure> {
        checkpoint()?;
        let count = equations.iter().fold(0_usize, |count, equation| {
            count.saturating_add(equation.nterms())
        });
        if self.limits.diagnostics_enabled && count > self.limits.max_terms_per_conjunction {
            return Err(CaseIntersectionFailure::Budget {
                kind: CaseIntersectionBudget::ConjunctionTerms,
                limit: self.limits.max_terms_per_conjunction,
            });
        }
        self.stats.peak_conjunction_terms = self.stats.peak_conjunction_terms.max(count);
        Ok(())
    }

    fn remember_state(&mut self) -> Result<(), CaseIntersectionFailure> {
        let mut ancestor = self.current.ancestry.as_deref();
        while let Some(previous) = ancestor {
            checkpoint()?;
            if previous.parent == self.current.parent
                && previous.equations == self.current.equations
                && previous.exclusions == self.current.exclusions
            {
                return Err(CaseIntersectionFailure::RepeatedState);
            }
            ancestor = previous.previous.as_deref();
        }
        self.current.ancestry = Some(Arc::new(Ancestry {
            parent: self.current.parent.clone(),
            equations: self.current.equations.clone(),
            exclusions: self.current.exclusions.clone(),
            previous: self.current.ancestry.clone(),
        }));
        Ok(())
    }

    /// Opt-in refinement: factor every equation of the current state, drop
    /// factors that are exclusions of this item (their zeros are owned by an
    /// earlier sibling), and branch on the first equation with two or more
    /// remaining factors. Children are disjoint: the child of factor `f_k`
    /// excludes `f_1..f_{k-1}` (affine factors come first).
    fn refined_factor_pass(
        &mut self,
        pending: &mut Vec<WorkItem<N>>,
    ) -> Result<FactorStep, CaseIntersectionFailure> {
        for position in 0..self.current.equations.len() {
            spend(
                &mut self.stats.factorizations,
                self.limits.diagnostic_limit(self.limits.max_factorizations),
                CaseIntersectionBudget::Factorizations,
            )?;
            let start = Instant::now();
            let factors = native::factors(&self.current.equations[position]);
            self.stats.factorization_time += start.elapsed();
            let mut factors = factors?;
            self.check_terms(&factors)?;
            native::retain_possible_integer_factors(&mut factors, self.indices);
            let found = factors.len();
            factors.retain(|factor| !self.current.exclusions.contains(factor));
            if factors.is_empty() {
                // Either no factor has an integer zero (the established
                // reading), or every zero lies on an excluded hypersurface.
                if found > 0 {
                    self.refinement_stats.excluded_empty_branches = self
                        .refinement_stats
                        .excluded_empty_branches
                        .saturating_add(1);
                }
                return Ok(FactorStep::Consumed);
            }
            if factors.len() == 1 {
                if factors[0] == self.current.equations[position] {
                    continue;
                }
                // A product of excluded factors and ONE remaining factor (or
                // a repeated factor): on this item it is that factor alone.
                self.refinement_stats.excluded_factor_drops = self
                    .refinement_stats
                    .excluded_factor_drops
                    .saturating_add(1);
                let mut equations = self.current.equations.to_vec();
                equations[position] = factors.pop().expect("one factor");
                native::canonicalize(&mut equations);
                self.current.equations = equations.into();
                return Ok(FactorStep::Replaced);
            }
            if self.limits.diagnostics_enabled
                && self
                    .stats
                    .work_items
                    .checked_add(pending.len())
                    .and_then(|count| count.checked_add(factors.len()))
                    .is_none_or(|count| count > self.limits.max_work_items)
            {
                return Err(CaseIntersectionFailure::Budget {
                    kind: CaseIntersectionBudget::WorkItems,
                    limit: self.limits.max_work_items,
                });
            }
            factors.sort_by_key(|factor| (!native::is_affine(factor), factor.nterms()));
            self.stats.factor_children = self.stats.factor_children.saturating_add(factors.len());
            self.refinement_stats.factor_first_splits =
                self.refinement_stats.factor_first_splits.saturating_add(1);
            self.refinement_stats.factor_first_children = self
                .refinement_stats
                .factor_first_children
                .saturating_add(factors.len());
            for (rank, factor) in factors.iter().enumerate().rev() {
                let mut child_equations = self.current.equations.to_vec();
                child_equations[position] = factor.clone();
                native::canonicalize(&mut child_equations);
                let mut exclusions = self.current.exclusions.to_vec();
                exclusions.extend(factors[..rank].iter().cloned());
                native::canonicalize(&mut exclusions);
                pending.push(WorkItem {
                    parent: self.current.parent.clone(),
                    equations: child_equations.into(),
                    ancestry: self.current.ancestry.clone(),
                    exclusions: exclusions.into(),
                    graph_rounds: self.current.graph_rounds,
                });
            }
            return Ok(FactorStep::Consumed);
        }
        Ok(FactorStep::Unchanged)
    }

    /// Keep only resolved cases on which the ORIGINAL conjunction vanishes
    /// identically. Needed only after a G-split remainder replaced equations:
    /// such a remainder implies the original conjunction wherever no
    /// exclusion vanishes, so a case on which the original does not vanish
    /// lies inside an excluded hypersurface, whose points a sibling owns.
    fn drop_excluded_cases(
        &mut self,
        cases: Vec<Case<N>>,
    ) -> Result<Vec<Case<N>>, CaseIntersectionFailure> {
        let mut kept = Vec::with_capacity(cases.len());
        for case in cases {
            checkpoint()?;
            if original_vanishes_on(
                &self.original_parent,
                &self.original_conjunction,
                &case,
                self.indices,
            )? {
                kept.push(case);
            } else {
                self.refinement_stats.excluded_cases_dropped = self
                    .refinement_stats
                    .excluded_cases_dropped
                    .saturating_add(1);
            }
        }
        Ok(kept)
    }
}

/// Outcome of the opt-in factor pass on the current state.
enum FactorStep {
    /// Every equation is irreducible and none has an excluded factor.
    Unchanged,
    /// One equation was replaced in place by its only non-excluded factor.
    Replaced,
    /// The state was split into factor children (or proved empty).
    Consumed,
}

/// Test seam: run only the stuck-branch resolution (S1 then S2) of the
/// engine on a captured branch `chart`/`branch` of `original_conjunction`,
/// exactly as `process` does when refinement returns `failure`.
#[cfg(test)]
pub(super) fn resolve_stuck_branch_for_test<const N: usize>(
    original_parent: &Case<N>,
    original_conjunction: &[CoefficientPolynomial],
    chart: &Case<N>,
    branch: &[CoefficientPolynomial],
    indices: &[usize; N],
    sector: &[bool; N],
    failure: CaseIntersectionFailure,
) -> Result<(Vec<Case<N>>, Vec<CaseResolution<N>>, CaseIntersectionStats), CaseIntersectionFailure>
{
    let equations: Arc<[CoefficientPolynomial]> = native::restrict(chart, branch, indices)?.into();
    let mut engine = Engine {
        original_parent: original_parent.clone(),
        original_conjunction: original_conjunction.to_vec().into(),
        current: WorkItem {
            parent: chart.clone(),
            equations,
            ancestry: None,
            exclusions: Arc::from([]),
            graph_rounds: 0,
        },
        indices,
        sector,
        limits: CaseIntersectionLimits::default(),
        max_numerator_rank: None,
        stats: CaseIntersectionStats::default(),
        resolutions: Vec::new(),
        unsupported: UnsupportedBranchPolicy::Refuse,
        deferred: Vec::new(),
        refinement: CaseRefinement::default(),
        refinement_stats: CaseRefinementStats::default(),
        check_original: false,
    };
    let mut resolved = Vec::new();
    engine.resolve_stuck_branch(failure, &mut resolved)?;
    Ok((resolved, engine.resolutions, engine.stats))
}

/// Test seam: [`resolve_stuck_branch_for_test`] followed by S4 under
/// `unsupported`, exactly as `process` settles a stuck branch.
#[cfg(test)]
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub(super) fn settle_stuck_branch_for_test<const N: usize>(
    original_parent: &Case<N>,
    original_conjunction: &[CoefficientPolynomial],
    chart: &Case<N>,
    branch: &[CoefficientPolynomial],
    indices: &[usize; N],
    sector: &[bool; N],
    unsupported: UnsupportedBranchPolicy,
    failure: CaseIntersectionFailure,
) -> Result<(Vec<Case<N>>, Vec<CaseResolution<N>>, Vec<DeferredRegion<N>>), CaseIntersectionFailure>
{
    let equations: Arc<[CoefficientPolynomial]> = native::restrict(chart, branch, indices)?.into();
    let mut engine = Engine {
        original_parent: original_parent.clone(),
        original_conjunction: original_conjunction.to_vec().into(),
        current: WorkItem {
            parent: chart.clone(),
            equations,
            ancestry: None,
            exclusions: Arc::from([]),
            graph_rounds: 0,
        },
        indices,
        sector,
        limits: CaseIntersectionLimits::default(),
        max_numerator_rank: None,
        stats: CaseIntersectionStats::default(),
        resolutions: Vec::new(),
        unsupported,
        deferred: Vec::new(),
        refinement: CaseRefinement::default(),
        refinement_stats: CaseRefinementStats::default(),
        check_original: false,
    };
    let mut resolved = Vec::new();
    let mut pending = Vec::new();
    engine.settle_stuck_branch(failure, &mut pending, &mut resolved)?;
    debug_assert!(pending.is_empty(), "G-split is off in this seam");
    Ok((resolved, engine.resolutions, engine.deferred))
}

/// S1 test: `chart` lies in `original_parent` and the ORIGINAL conjunction
/// restricts to zero on it (exact native restriction, no sampling).
pub(super) fn original_vanishes_on<const N: usize>(
    original_parent: &Case<N>,
    original_conjunction: &[CoefficientPolynomial],
    chart: &Case<N>,
    indices: &[usize; N],
) -> Result<bool, CaseIntersectionFailure> {
    if !original_parent
        .contains(chart)
        .map_err(CaseIntersectionFailure::Admission)?
    {
        return Ok(false);
    }
    Ok(native::restrict(chart, original_conjunction, indices)?.is_empty())
}

pub(super) fn spend(
    count: &mut usize,
    limit: Option<usize>,
    kind: CaseIntersectionBudget,
) -> Result<(), CaseIntersectionFailure> {
    checkpoint()?;
    if let Some(limit) = limit
        && *count >= limit
    {
        return Err(CaseIntersectionFailure::Budget { kind, limit });
    }
    *count = count.saturating_add(1);
    Ok(())
}

pub(super) fn checkpoint() -> Result<(), CaseIntersectionFailure> {
    symbolica::utils::resources::checkpoint().map_err(CaseIntersectionFailure::Resources)
}

/// Conservative equality-domain implication is sufficient for pruning a
/// covered component. No sector inequality is guessed from sampled points.
fn canonical_union<const N: usize>(
    mut cases: Vec<Case<N>>,
) -> Result<Vec<Case<N>>, CaseIntersectionFailure> {
    cases.sort_by(Case::queue_cmp);
    cases.dedup();
    let mut retained = Vec::<Case<N>>::new();
    for candidate in cases {
        checkpoint()?;
        let mut covered = false;
        for container in &retained {
            if container
                .contains(&candidate)
                .map_err(CaseIntersectionFailure::Admission)?
            {
                covered = true;
                break;
            }
        }
        if covered {
            continue;
        }
        let mut previous = Vec::with_capacity(retained.len() + 1);
        for contained in retained {
            if !candidate
                .contains(&contained)
                .map_err(CaseIntersectionFailure::Admission)?
            {
                previous.push(contained);
            }
        }
        previous.push(candidate);
        retained = previous;
    }
    retained.sort_by(Case::queue_cmp);
    Ok(retained)
}
