//! Diagnostic access to the discovery loop and the exact lift (p5l-lift-trace, H1).
//!
//! These entry points expose the SAME modular discovery as `solve_case` (same seed stream, same basis visit, same
//! probe point, same pivot test), but record every instantiated row and let the caller supply an explicit seed list
//! and lift an arbitrary row subset. They grant no rule authority: a caller that publishes a rule from a lift must
//! pass it through the ordinary exception/admission path.

use super::*;
use symbolica::domains::finite_field::FiniteFieldCore;

/// One instantiated, nonempty source row seen by a study discovery.
#[derive(Clone, Debug)]
pub struct StudyRow<const N: usize> {
    pub row: ExactRow<N>,
    pub source: SeedSource<N>,
    /// Accepted by the modular reducer (a new pivot) when it was added.
    pub pivot: Option<Integral<N>>,
}

/// Rows seen up to (and including) the first matching pivot, and its engine trace.
#[derive(Clone, Debug)]
pub struct TraceStudy<const N: usize> {
    pub rows: Vec<StudyRow<N>>,
    /// Engine trace (Discovery::trace of the matching row) as indices into `rows`, in pivot-column order.
    pub trace: Vec<usize>,
    /// The matching pivot, or None when the seed list was exhausted without a match.
    pub pivot: Option<Integral<N>>,
    /// The row that produced the matching pivot (index into `rows`).
    pub root: Option<usize>,
    pub seeds: usize,
    pub depth: u32,
    pub discovery: Option<DiscoveryStats>,
    /// The modular probe point (one residue per coefficient variable).
    pub point: Vec<u64>,
    /// A direct hit: the first row's leading integral matched (no trace).
    pub direct_hit: bool,
}

impl<'a, const N: usize> SectorSolver<'a, N> {
    /// Seed for `case` displaced by `shifts` (symbolic axes: displacement; numeric axes: added to the value, which
    /// must keep its sign). None when a numeric axis would change sign or a power is out of range.
    pub fn study_seed(&self, case: &Case<N>, shifts: [i16; N]) -> Option<Seed<N>> {
        let initial = case.integral();
        let mut powers = *initial.powers();
        let mut seed_shifts = [0; N];
        let frozen = self.seed_frozen();
        for i in 0..N {
            if shifts[i] == 0 {
                continue;
            }
            if frozen[i] {
                return None;
            }
            let p = initial[i];
            if !p.is_symbolic() && (i32::from(p.value()) + i32::from(shifts[i]) > 0) != (p.value() > 0) {
                return None;
            }
            powers[i] = powers[i].shifted(shifts[i]).ok()?;
            if powers[i].is_symbolic() {
                seed_shifts[i] = shifts[i];
            }
        }
        Some(Seed { integral: Integral::new(powers), shifts: seed_shifts })
    }

    /// Instantiate one basis row at one seed for `case` (the discovery instantiation).
    pub fn study_instantiate(&self, case: &Case<N>, basis_row: usize, seed: &Seed<N>) -> Result<ExactRow<N>, SolverError> {
        let zero: &[[bool; N]] = if case.affine().is_some() { &[] } else { &self.config.zero_sectors };
        instantiate(&self.basis[basis_row], seed, &self.system.indices, self.system.fixed(), &self.order, zero, case.affine())
    }

    /// The discovery loop of `solve_case` (default visit order) with an optional explicit seed list in place of the
    /// engine seed stream. Every nonempty instantiated row is recorded. Stops at the first matching pivot.
    pub fn study_discovery(
        &self,
        case: &Case<N>,
        options: SearchOptions,
        seeds: Option<&[Seed<N>]>,
        max_seeds: Option<usize>,
        stop_at_match: bool,
    ) -> Result<TraceStudy<N>, SolverError> {
        self.validate_search(case, options)?;
        let initial = case.integral();
        let mut stream = super::super::ScheduledSeeds::new(
            Seeds::new_two_sided(initial, *self.order.sector(), self.seed_frozen(), self.order.two_sided_axes()),
            self.config.seed_schedule,
        );
        let probe = Probe::new(self.order.clone(), self.system.variable_count, options);
        let point: Vec<u64> = probe.point.iter().map(|x| probe.field.from_element(x)).collect();
        let mut probe = Some(probe);
        let mut out = TraceStudy {
            rows: Vec::new(),
            trace: Vec::new(),
            pivot: None,
            root: None,
            seeds: 0,
            depth: 0,
            discovery: None,
            point,
            direct_hit: false,
        };
        let mut accepted: Vec<usize> = Vec::new();
        let mut explicit = seeds.map(|s| s.iter());
        loop {
            let seed = match explicit.as_mut() {
                Some(it) => match it.next() {
                    Some(s) => *s,
                    None => break,
                },
                None => match stream.next() {
                    Some(s) => {
                        out.depth = stream.depth();
                        if options.max_depth.is_some_and(|limit| stream.depth() > limit) {
                            break;
                        }
                        s?
                    }
                    None => break,
                },
            };
            if max_seeds.is_some_and(|m| out.seeds >= m) {
                break;
            }
            out.seeds += 1;
            for ordinal in 0..self.basis.len() {
                let row = self.study_instantiate(case, ordinal, &seed)?;
                let Some(leading) = row.first().map(|t| t.integral) else { continue };
                let source = SeedSource { basis_row: ordinal, seed };
                if case.matches(&leading) && stop_at_match {
                    out.direct_hit = true;
                    out.pivot = Some(leading);
                    let index = out.rows.len();
                    out.rows.push(StudyRow { row, source, pivot: Some(leading) });
                    out.root = Some(index);
                    out.trace = vec![index];
                    return Ok(out);
                }
                if case.is_numerical() && self.order.compare(&initial, &leading) == Ordering::Less {
                    continue;
                }
                let active = probe.as_mut().unwrap();
                let modular = active.evaluate(&row)?;
                let pivot = active.discovery.add_row(&modular);
                let index = out.rows.len();
                out.rows.push(StudyRow { row, source, pivot });
                if let Some(pivot) = pivot {
                    accepted.push(index);
                    if case.matches(&pivot) && out.pivot.is_none() {
                        let trace = active.discovery.trace(accepted.len() - 1);
                        out.trace = trace.iter().map(|&k| accepted[k]).collect();
                        out.pivot = Some(pivot);
                        out.root = Some(index);
                        out.discovery = Some(active.discovery.stats());
                        if stop_at_match {
                            return Ok(out);
                        }
                    }
                }
            }
        }
        out.discovery = probe.as_ref().map(|p| p.discovery.stats());
        Ok(out)
    }

    /// Exact lift of an arbitrary row subset (rows harder-first is best; any order is valid): the first normalized
    /// row whose pivot is `target`, as in the engine's exact replay, with an explicit backend.
    pub fn study_exact_lift(
        &self,
        rows: &[ExactRow<N>],
        target: Integral<N>,
        backend: SymbolicExactBackend,
        observe: impl FnMut(super::super::MaterializationEvent<N>),
    ) -> Result<ExactRow<N>, SolverError> {
        exact_materialize_using_with_observer(
            rows,
            &self.order,
            target,
            backend,
            self.config.coefficient_variable_order,
            self.system.coefficient_order(),
            observe,
        )
        .map_err(SolverError::exact_replay)
    }

    /// Canonical (target, rhs) of an exact winning row, as the engine publishes candidates.
    pub fn study_canonicalize(&self, row: ExactRow<N>) -> Result<(Integral<N>, ExactRow<N>), SolverError> {
        canonicalize(row, &self.system.indices)
    }

    /// Index-variable positions in the coefficient ring (n_i -> variable).
    pub fn study_index_variables(&self) -> [usize; N] {
        self.system.indices
    }
}
