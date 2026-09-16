use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::Arc;
use std::time::Instant;

use crate::algebra::CoefficientPolynomial;

use super::super::Case;
use super::{
    CaseIntersectionBudget, CaseIntersectionError, CaseIntersectionFailure, CaseIntersectionLimits,
    CaseIntersectionResult, CaseIntersectionStats, native,
};

/// Ancestry is shared by siblings. A repeated state along one refinement path
/// is incomplete, whereas coincident states on different siblings are harmless.
struct Ancestry<const N: usize> {
    parent: Case<N>,
    equations: Arc<[CoefficientPolynomial]>,
    previous: Option<Arc<Self>>,
}

struct WorkItem<const N: usize> {
    parent: Case<N>,
    equations: Arc<[CoefficientPolynomial]>,
    ancestry: Option<Arc<Ancestry<N>>>,
}

struct Engine<'a, const N: usize> {
    original_parent: Case<N>,
    original_conjunction: Arc<[CoefficientPolynomial]>,
    current: WorkItem<N>,
    indices: &'a [usize; N],
    sector: &'a [bool; N],
    limits: CaseIntersectionLimits,
    stats: CaseIntersectionStats,
}

pub(super) fn intersect<const N: usize>(
    parent: &Case<N>,
    conjunction: &[CoefficientPolynomial],
    indices: &[usize; N],
    sector: &[bool; N],
    limits: CaseIntersectionLimits,
) -> Result<CaseIntersectionResult<N>, CaseIntersectionError<N>> {
    let equations: Arc<[CoefficientPolynomial]> = conjunction.to_vec().into();
    let mut engine = Engine {
        original_parent: parent.clone(),
        original_conjunction: equations.clone(),
        current: WorkItem {
            parent: parent.clone(),
            equations,
            ancestry: None,
        },
        indices,
        sector,
        limits,
        stats: CaseIntersectionStats::default(),
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        native::preflight(parent, conjunction, indices)?;
        engine.run()
    }));
    match outcome {
        Ok(Ok(cases)) => Ok(CaseIntersectionResult {
            cases,
            stats: engine.stats,
        }),
        Ok(Err(failure)) => Err(engine.error(failure)),
        Err(_) => Err(engine.error(CaseIntersectionFailure::NativeAlgebra)),
    }
}

impl<const N: usize> Engine<'_, N> {
    fn error(&self, failure: CaseIntersectionFailure) -> CaseIntersectionError<N> {
        CaseIntersectionError {
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
                self.limits.max_work_items,
                CaseIntersectionBudget::WorkItems,
            )?;
            self.check_terms(&self.current.equations.clone())?;
            // Also checks an incoming affine domain against the current sector
            // and rejects a mismatched index map even for an empty conjunction.
            let start = Instant::now();
            let admitted = self
                .current
                .parent
                .intersect(&[], self.indices, self.sector)
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
        let result = canonical_union(resolved);
        self.stats.union_time += start.elapsed();
        result
    }

    fn process(
        &mut self,
        pending: &mut Vec<WorkItem<N>>,
        resolved: &mut Vec<Case<N>>,
    ) -> Result<(), CaseIntersectionFailure> {
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
                .intersect(&self.current.equations, self.indices, self.sector)
                .map_err(CaseIntersectionFailure::Admission);
            self.stats.admission_time += start.elapsed();
            self.stats.affine_admissions += 1;
            if let Some(child) = child? {
                resolved.push(child);
            }
            return Ok(());
        }
        let mut normalized = false;
        loop {
            let start = Instant::now();
            let restricted =
                native::restrict(&self.current.parent, &self.current.equations, self.indices);
            self.stats.restriction_time += start.elapsed();
            self.stats.restrictions += 1;
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
                    .intersect(&affine, self.indices, self.sector)
                    .map_err(CaseIntersectionFailure::Admission);
                self.stats.admission_time += start.elapsed();
                self.stats.affine_admissions += 1;
                let Some(child) = child? else {
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
                continue;
            }

            if !normalized {
                self.remember_state()?;
                if self.current.equations.len() > 1 {
                    spend(
                        &mut self.stats.normalizations,
                        self.limits.max_normalizations,
                        CaseIntersectionBudget::Normalizations,
                    )?;
                    let start = Instant::now();
                    let basis = native::normalize(&self.current.equations);
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

            for position in 0..self.current.equations.len() {
                spend(
                    &mut self.stats.factorizations,
                    self.limits.max_factorizations,
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
                if self
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
                self.stats.factor_children += factors.len();
                for factor in factors.into_iter().rev() {
                    let mut child_equations = self.current.equations.to_vec();
                    child_equations[position] = factor;
                    native::canonicalize(&mut child_equations);
                    pending.push(WorkItem {
                        parent: self.current.parent.clone(),
                        equations: child_equations.into(),
                        ancestry: self.current.ancestry.clone(),
                    });
                }
                return Ok(());
            }
            return Err(CaseIntersectionFailure::UnsupportedGeometry);
        }
    }

    fn check_terms(
        &mut self,
        equations: &[CoefficientPolynomial],
    ) -> Result<(), CaseIntersectionFailure> {
        let count = equations.iter().try_fold(0_usize, |count, equation| {
            count.checked_add(equation.nterms())
        });
        let Some(count) = count.filter(|&count| count <= self.limits.max_terms_per_conjunction)
        else {
            return Err(CaseIntersectionFailure::Budget {
                kind: CaseIntersectionBudget::ConjunctionTerms,
                limit: self.limits.max_terms_per_conjunction,
            });
        };
        self.stats.peak_conjunction_terms = self.stats.peak_conjunction_terms.max(count);
        Ok(())
    }

    fn remember_state(&mut self) -> Result<(), CaseIntersectionFailure> {
        let mut ancestor = self.current.ancestry.as_deref();
        while let Some(previous) = ancestor {
            if previous.parent == self.current.parent
                && previous.equations == self.current.equations
            {
                return Err(CaseIntersectionFailure::RepeatedState);
            }
            ancestor = previous.previous.as_deref();
        }
        self.current.ancestry = Some(Arc::new(Ancestry {
            parent: self.current.parent.clone(),
            equations: self.current.equations.clone(),
            previous: self.current.ancestry.clone(),
        }));
        Ok(())
    }
}

fn spend(
    count: &mut usize,
    limit: usize,
    kind: CaseIntersectionBudget,
) -> Result<(), CaseIntersectionFailure> {
    if *count >= limit {
        return Err(CaseIntersectionFailure::Budget { kind, limit });
    }
    *count += 1;
    Ok(())
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
