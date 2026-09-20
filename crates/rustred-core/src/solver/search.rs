use std::cmp::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::finite_field::{FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::prelude::{Field, Integer, Ring};

use super::discovery::{Discovery, DiscoveryStats, exact_materialize_using_with_observer};
use super::instantiate::{canonicalize, instantiate};
use super::precondition::{
    PreconditionProvenance, precondition_with_provenance, precondition_with_variable_order,
};
use super::{
    Case, CoefficientVariableOrder, ExactRow, Integral, IntegralOrder, PolynomialRow, Seed, Seeds,
    SolverError, SourceSystem, SymbolicExactBackend, Term,
};

mod observation;
pub use observation::SearchEvent;

#[derive(Clone, Debug)]
pub struct SectorConfig<const N: usize> {
    pub deltas: [bool; N],
    pub removed_deltas: [bool; N],
    /// Coordinate priority for the final lexicographic tie-breaks only.
    /// Sector and cut priority and aggregate degrees remain unchanged.
    pub permutation: Option<[usize; N]>,
    /// Immutable family-wide zero-sector census, shared by sector workers.
    pub zero_sectors: Arc<[[bool; N]]>,
    /// Single-target symbolic exact lifting. The independent numerical policy
    /// controls shared finite-corner replay; both defaults preserve sparse GPLU.
    pub symbolic_exact_backend: SymbolicExactBackend,
    /// Native exact coefficient field for the shared finite-corner trace.
    pub numerical_exact_backend: super::NumericalExactBackend,
    /// Native polynomial representation during single-target exact lifting;
    /// independent of the physical integral permutation and modular discovery.
    pub coefficient_variable_order: CoefficientVariableOrder,
}

impl<const N: usize> Default for SectorConfig<N> {
    fn default() -> Self {
        Self {
            deltas: [false; N],
            removed_deltas: [false; N],
            permutation: None,
            zero_sectors: Arc::from([]),
            symbolic_exact_backend: SymbolicExactBackend::Sparse,
            numerical_exact_backend: super::NumericalExactBackend::Sparse,
            coefficient_variable_order: CoefficientVariableOrder::Original,
        }
    }
}

/// Search controls, not a statement of coverage when the search is exhausted.
#[derive(Clone, Copy, Debug)]
pub struct SearchOptions {
    pub max_depth: Option<u32>,
    pub prime: u64,
    pub sample_seed: u64,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_depth: None,
            prime: (1_u64 << 61) - 1,
            sample_seed: 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct SearchStats {
    pub seeds: usize,
    pub rows: usize,
    pub independent_rows: usize,
    pub exact_trace_rows: usize,
    pub direct_hit: bool,
    pub elapsed: Duration,
    pub exact_materialization: Duration,
    pub discovery: Option<DiscoveryStats>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SeedSource<const N: usize> {
    /// Ordinal in [`SectorSolver::basis`], after sector-local polynomial
    /// elimination. This is NOT an ordinal in the original source corpus.
    pub basis_row: usize,
    pub seed: Seed<N>,
}

/// An exact case-solving equation, before exceptional-domain analysis.
///
/// In particular, a candidate is *not* an unconditional rewrite rule. Its
/// normalized RHS denominators and sector-activation boundaries still need
/// to be processed by the sector case queue. Source conditions are owned by
/// the immutable [`SourceSystem`]. This type cannot publish a closing artifact.
#[derive(Debug)]
pub struct RuleCandidate<const N: usize> {
    pub case: Case<N>,
    pub target: Integral<N>,
    pub rhs: ExactRow<N>,
    pub sources: Vec<SeedSource<N>>,
    pub stats: SearchStats,
}

/// One sector-local preconditioned source basis. Construct once, then reuse
/// across all its exceptional cases, exactly as SpIRed's `solveSector` does.
pub struct SectorSolver<'a, const N: usize> {
    pub(super) system: &'a SourceSystem<N>,
    pub(super) basis: Vec<PolynomialRow<N>>,
    pub(super) order: IntegralOrder<N>,
    pub(super) config: SectorConfig<N>,
}

impl<'a, const N: usize> SectorSolver<'a, N> {
    pub fn new(
        system: &'a SourceSystem<N>,
        sector: [bool; N],
        config: SectorConfig<N>,
    ) -> Result<Self, SolverError> {
        let (order, rows) = Self::prepare(system, sector, &config)?;
        let basis = precondition_with_variable_order(rows, &order, system.coefficient_order());
        Ok(Self {
            system,
            basis,
            order,
            config,
        })
    }

    /// Cold replay may retain the forward polynomial derivation. Discovery's
    /// ordinary constructor never constructs this optional trace.
    pub(crate) fn new_with_provenance(
        system: &'a SourceSystem<N>,
        sector: [bool; N],
        config: SectorConfig<N>,
    ) -> Result<(Self, PreconditionProvenance), SolverError> {
        let (order, rows) = Self::prepare(system, sector, &config)?;
        let (basis, trace) = precondition_with_provenance(rows, &order, system.coefficient_order());
        Ok((
            Self {
                system,
                basis,
                order,
                config,
            },
            trace,
        ))
    }

    fn prepare(
        system: &'a SourceSystem<N>,
        sector: [bool; N],
        config: &SectorConfig<N>,
    ) -> Result<(IntegralOrder<N>, Vec<PolynomialRow<N>>), SolverError> {
        for i in 0..N {
            if config.deltas[i] && !sector[i] || config.removed_deltas[i] && !config.deltas[i] {
                return Err(SolverError::InvalidInput(
                    "invalid delta sector/configuration".into(),
                ));
            }
            if system.fixed()[i].is_some_and(|value| value != 1 || !config.removed_deltas[i]) {
                return Err(SolverError::InvalidInput(
                    "prepared fixed sources require the matching removed delta at power one".into(),
                ));
            }
        }
        let mut order = IntegralOrder::new(sector, config.deltas);
        if let Some(permutation) = config.permutation {
            order = order.with_permutation(permutation)?;
        }
        let mut rows = system.rows.clone();
        for row in &mut rows {
            row.retain(|term| !term.coefficient.is_zero());
            row.sort_unstable_by(|a, b| order.compare(&a.integral, &b.integral));
            let mut canonical: PolynomialRow<N> = Vec::with_capacity(row.len());
            for term in row.drain(..) {
                if let Some(last) = canonical
                    .last_mut()
                    .filter(|last| last.integral == term.integral)
                {
                    last.coefficient = &last.coefficient + &term.coefficient;
                } else {
                    canonical.push(term);
                }
            }
            canonical.retain(|term| !term.coefficient.is_zero());
            *row = canonical;
        }
        Ok((order, rows))
    }

    pub fn basis(&self) -> &[PolynomialRow<N>] {
        &self.basis
    }
    pub fn ordering(&self) -> &IntegralOrder<N> {
        &self.order
    }

    pub fn solve_case(
        &self,
        case: impl Into<Case<N>>,
        options: SearchOptions,
    ) -> Result<RuleCandidate<N>, SolverError> {
        self.solve_case_with_observer(case, options, |_| {})
    }

    /// Observe coarse discovery/exact phase boundaries without retaining
    /// additional rows or changing seed, pivot, or exact-replay chronology.
    pub fn solve_case_with_observer(
        &self,
        case: impl Into<Case<N>>,
        options: SearchOptions,
        mut observe: impl FnMut(SearchEvent<N>),
    ) -> Result<RuleCandidate<N>, SolverError> {
        let case = case.into();
        if !case.is_in_sector(self.order.sector()) {
            return Err(SolverError::InvalidInput(
                "case lies outside its sector".into(),
            ));
        }
        for (i, removed) in self.config.removed_deltas.iter().enumerate() {
            if *removed && case.fixed()[i] != Some(1) {
                return Err(SolverError::InvalidInput(
                    "removed linear deltas must be fixed to one".into(),
                ));
            }
        }
        if let Some(affine) = case.affine() {
            if affine.index_variables() != self.system.index_variables()
                || self
                    .system
                    .rows()
                    .iter()
                    .flatten()
                    .next()
                    .is_some_and(|term| {
                        term.coefficient.variables() != affine.equations()[0].variables()
                    })
            {
                return Err(SolverError::InvalidInput(
                    "affine case and source system use different coefficient/index maps".into(),
                ));
            }
        }
        if options.prime < 3 || !Integer::from(options.prime).is_prime(0) {
            return Err(SolverError::InvalidInput(
                "modular probe requires an odd prime".into(),
            ));
        }
        let start = Instant::now();
        let mut stats = SearchStats::default();
        let mut probe = None;
        let mut original_rows = Vec::new();
        let mut original_sources = Vec::new();
        let initial = case.integral();
        let mut seeds = Seeds::new(initial, *self.order.sector(), self.config.removed_deltas);
        // A coupled affine chart can determine the sign of a symbolic power
        // only after its equations are solved together with the sector.  The
        // rectangular zero-sector census cannot make that inference: pruning
        // a term from an affine row using the parent orthant can therefore
        // change the source span before GPLU sees it.  Keep every physical
        // column for affine discovery and let the exact chart/replay gates
        // decide whether it is removable.  Coordinate cases retain the
        // cheap authenticated zero-sector projection.
        let discovery_zero_sectors: &[[bool; N]] = if case.affine().is_some() {
            &[]
        } else {
            &self.config.zero_sectors
        };
        let mut observed_depth = None;
        while let Some(seed) = seeds.next() {
            let depth = seeds.depth();
            if options.max_depth.is_some_and(|limit| depth > limit) {
                return Err(SolverError::SearchExhausted {
                    depth: depth - 1,
                    rows: stats.rows,
                });
            }
            let seed = seed?;
            stats.seeds += 1;
            if observed_depth != Some(depth) || stats.seeds.is_power_of_two() {
                observed_depth = Some(depth);
                observe(SearchEvent::DiscoveryProgress {
                    depth,
                    seeds: stats.seeds,
                    rows: stats.rows,
                    discovery: probe.as_ref().map(|p: &Probe<N>| p.discovery.stats()),
                });
            }
            for (ordinal, source) in self.basis.iter().enumerate() {
                let row = instantiate(
                    source,
                    &seed,
                    &self.system.indices,
                    self.system.fixed(),
                    &self.order,
                    discovery_zero_sectors,
                    case.affine(),
                )?;
                stats.rows += 1;
                let Some(leading) = row.first() else { continue };
                let source = SeedSource {
                    basis_row: ordinal,
                    seed,
                };
                if case.matches(&leading.integral) {
                    stats.direct_hit = true;
                    stats.exact_trace_rows = 1;
                    observe(SearchEvent::CanonicalizationStarted {
                        terms: row.len(),
                        direct_hit: true,
                    });
                    let (target, rhs) = canonicalize(row, &self.system.indices)?;
                    stats.elapsed = start.elapsed();
                    stats.discovery = probe.as_ref().map(|p: &Probe<N>| p.discovery.stats());
                    return Ok(RuleCandidate {
                        case,
                        target,
                        rhs,
                        sources: vec![source],
                        stats,
                    });
                }
                if case.is_numerical()
                    && self.order.compare(&initial, &leading.integral) == Ordering::Less
                {
                    continue;
                }
                let probe = probe.get_or_insert_with(|| {
                    Probe::new(self.order, self.system.variable_count, options)
                });
                let modular_row = probe.evaluate(&row)?;
                if let Some(pivot) = probe.discovery.add_row(&modular_row) {
                    original_rows.push(row);
                    original_sources.push(source);
                    stats.independent_rows += 1;
                    if case.matches(&pivot) {
                        let trace = probe.discovery.trace(original_rows.len() - 1);
                        stats.exact_trace_rows = trace.len();
                        let selected = trace
                            .iter()
                            .map(|i| std::mem::take(&mut original_rows[*i]))
                            .collect::<Vec<_>>();
                        // This case ends at the first matching pivot. Release
                        // nonwinning exact rows before the expensive native solve.
                        drop(original_rows);
                        observe(SearchEvent::ExactStarted {
                            pivot,
                            trace_rows: trace.len(),
                            discovery: probe.discovery.stats(),
                        });
                        let exact_start = Instant::now();
                        let exact = exact_materialize_using_with_observer(
                            &selected,
                            &self.order,
                            pivot,
                            self.config.symbolic_exact_backend,
                            self.config.coefficient_variable_order,
                            self.system.coefficient_order(),
                            |event| observe(SearchEvent::ExactProgress(event)),
                        )
                        .map_err(|error| SolverError::ExactReplay(error.to_string()))?;
                        observe(SearchEvent::CanonicalizationStarted {
                            terms: exact.len(),
                            direct_hit: false,
                        });
                        let (target, rhs) = canonicalize(exact, &self.system.indices)?;
                        stats.exact_materialization = exact_start.elapsed();
                        stats.elapsed = start.elapsed();
                        stats.discovery = Some(probe.discovery.stats());
                        return Ok(RuleCandidate {
                            case,
                            target,
                            rhs,
                            sources: trace.into_iter().map(|i| original_sources[i]).collect(),
                            stats,
                        });
                    }
                }
            }
        }
        Err(SolverError::SearchExhausted {
            depth: seeds.depth() as u32,
            rows: stats.rows,
        })
    }
}

pub(super) struct Probe<const N: usize> {
    field: Zp64,
    point: Vec<FiniteFieldElement<u64>>,
    pub(super) discovery: Discovery<N>,
}

impl<const N: usize> Probe<N> {
    pub(super) fn new(order: IntegralOrder<N>, nvars: usize, options: SearchOptions) -> Self {
        let field = Zp64::new(options.prime);
        let mut random = StdRng::seed_from_u64(options.sample_seed);
        let point = (0..nvars)
            .map(|_| Integer::from(random.random_range(1..options.prime)).to_finite_field(&field))
            .collect();
        Self {
            discovery: Discovery::new(order, field.clone()),
            field,
            point,
        }
    }

    pub(super) fn evaluate(
        &self,
        row: &ExactRow<N>,
    ) -> Result<Vec<Term<N, FiniteFieldElement<u64>>>, SolverError> {
        row.iter()
            .map(|term| {
                let numerator = term.coefficient.numerator.evaluate_with_coeff_map(
                    |value| value.to_finite_field(&self.field),
                    &self.point,
                    &self.field,
                );
                let denominator = term.coefficient.denominator.evaluate_with_coeff_map(
                    |value| value.to_finite_field(&self.field),
                    &self.point,
                    &self.field,
                );
                if self.field.is_zero(&denominator) {
                    return Err(SolverError::UnluckySample);
                }
                Ok(Term {
                    integral: term.integral,
                    coefficient: self.field.div(&numerator, &denominator),
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod affine_tests;

#[cfg(test)]
mod observation_tests;
