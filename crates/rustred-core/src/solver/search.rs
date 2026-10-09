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
    Case, CoefficientVariableOrder, ExactRow, Integral, IntegralOrder, PolynomialRow,
    RuleSelectionPolicy, RuleTrialBudget, RuleTrialLimits, RuleTrialStats, Seed, Seeds,
    SolverError, SourceDiscoveryStrategy, SourceSystem, SourceVisitOrder, SymbolicExactBackend,
    Term,
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
    /// Full persisted uncut integral order; mutually exclusive with legacy
    /// permutation/cut controls. Independent of source-row discovery priority.
    pub integral_order: Option<rustred_order::CompiledOrder>,
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
    /// Finite row visiting only; original basis IDs and proof order stay fixed.
    pub source_discovery: SourceDiscoveryStrategy,
    /// Optional exact candidate selection; the default retains first-valid search.
    pub rule_selection: RuleSelectionPolicy,
}

impl<const N: usize> Default for SectorConfig<N> {
    fn default() -> Self {
        Self {
            deltas: [false; N],
            removed_deltas: [false; N],
            permutation: None,
            integral_order: None,
            zero_sectors: Arc::from([]),
            symbolic_exact_backend: SymbolicExactBackend::Sparse,
            numerical_exact_backend: super::NumericalExactBackend::Sparse,
            coefficient_variable_order: CoefficientVariableOrder::Original,
            source_discovery: SourceDiscoveryStrategy::default(),
            rule_selection: RuleSelectionPolicy::default(),
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
    /// Guarded source rows retain their original ordinals. Mixing them in the
    /// unconditional polynomial preconditioner would erase their domains.
    pub(super) fn new_guarded(
        system: &'a SourceSystem<N>,
        sector: [bool; N],
        roles: [super::guarded::IndexRole; N],
    ) -> Result<Self, SolverError> {
        let config = SectorConfig {
            deltas: roles.map(|role| role == super::guarded::IndexRole::RequiredCut),
            ..Default::default()
        };
        let (order, mut basis) = Self::prepare(system, sector, &config)?;
        let order = order.with_roles(roles)?;
        for row in &mut basis {
            row.sort_unstable_by(|a, b| order.compare(&a.integral, &b.integral));
        }
        Ok(Self {
            system,
            basis,
            order,
            config,
        })
    }

    pub(super) fn solve_guarded_case(
        &self,
        case: Case<N>,
        options: SearchOptions,
        scope: &super::guarded::GuardedSearchScope<'_, N>,
    ) -> Result<RuleCandidate<N>, SolverError> {
        self.validate_search(&case, options)?;
        let start = Instant::now();
        let mut work = RuleTrialStats::default();
        let result = self.search_attempt_inner(
            case,
            options,
            None,
            None,
            false,
            &mut work,
            |_| {},
            Some(scope),
        );
        match result {
            Ok(mut candidate) => {
                work.search.elapsed = start.elapsed();
                candidate.stats = work.search;
                Ok(candidate)
            }
            Err(TrialSearchError::Solver(error)) => Err(error),
            Err(TrialSearchError::Limit(_)) => unreachable!("no rule portfolio limits"),
        }
    }

    pub fn new(
        system: &'a SourceSystem<N>,
        sector: [bool; N],
        mut config: SectorConfig<N>,
    ) -> Result<Self, SolverError> {
        let (order, rows) = Self::prepare(system, sector, &config)?;
        let basis = precondition_with_variable_order(rows, &order, system.coefficient_order());
        if let Some(plan) = config.source_discovery.materialize(&basis)? {
            config.source_discovery = SourceDiscoveryStrategy::Materialized(plan);
        }
        config.rule_selection.materialize(&basis)?;
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
        mut config: SectorConfig<N>,
    ) -> Result<(Self, PreconditionProvenance), SolverError> {
        let (order, rows) = Self::prepare(system, sector, &config)?;
        let (basis, trace) = precondition_with_provenance(rows, &order, system.coefficient_order());
        if let Some(plan) = config.source_discovery.materialize(&basis)? {
            config.source_discovery = SourceDiscoveryStrategy::Materialized(plan);
        }
        config.rule_selection.materialize(&basis)?;
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

    pub(super) fn seed_frozen(&self) -> [bool; N] {
        std::array::from_fn(|axis| {
            axis >= self.system.active_arity() || self.config.removed_deltas[axis]
        })
    }

    fn prepare(
        system: &'a SourceSystem<N>,
        sector: [bool; N],
        config: &SectorConfig<N>,
    ) -> Result<(IntegralOrder<N>, Vec<PolynomialRow<N>>), SolverError> {
        config.source_discovery.validate(N)?;
        config.rule_selection.validate(N)?;
        if config.integral_order.is_some()
            && (config.deltas.iter().any(|&cut| cut)
                || config.removed_deltas.iter().any(|&cut| cut)
                || config.permutation.is_some())
        {
            return Err(SolverError::InvalidInput("programmable integral order does not support cuts, removed cuts, or a legacy tie permutation".into()));
        }
        for i in 0..N {
            if config.deltas[i] && !sector[i] || config.removed_deltas[i] && !config.deltas[i] {
                return Err(SolverError::InvalidInput(
                    "invalid delta sector/configuration".into(),
                ));
            }
            if i >= system.active_arity() {
                if sector[i]
                    || config.deltas[i]
                    || config.removed_deltas[i]
                    || system.fixed()[i] != Some(0)
                {
                    return Err(SolverError::InvalidInput(
                        "padding coordinates must remain inactive numeric zero".into(),
                    ));
                }
                continue;
            }
            if system.fixed()[i].is_some_and(|value| value != 1 || !config.removed_deltas[i]) {
                return Err(SolverError::InvalidInput(
                    "prepared fixed sources require the matching removed delta at power one".into(),
                ));
            }
        }
        let mut order =
            IntegralOrder::new(sector, config.deltas).with_physical_arity(system.active_arity())?;
        if let Some(permutation) = config.permutation {
            order = order.with_permutation(permutation)?;
        }
        if let Some(program) = &config.integral_order {
            order = order.with_program(program.clone())?;
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

    /// The once-prepared plan, when explicitly requested. Default traversal
    /// retains no plan allocation. This is schedule metadata, not authority.
    pub fn source_visit_order(&self) -> Option<&SourceVisitOrder> {
        match &self.config.source_discovery {
            SourceDiscoveryStrategy::Materialized(plan) => Some(plan),
            _ => None,
        }
    }

    /// Install an already materialized callback result without preparing the
    /// basis twice. This validates before any subsequent solve; it does not
    /// change the basis or confer a portable identity on it.
    pub fn with_source_visit_order(mut self, plan: SourceVisitOrder) -> Result<Self, SolverError> {
        super::discovery_strategy::validate_visit_order(plan.ordinals(), self.basis.len())?;
        self.config.source_discovery = SourceDiscoveryStrategy::Materialized(plan);
        Ok(self)
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
        observe: impl FnMut(SearchEvent<N>),
    ) -> Result<RuleCandidate<N>, SolverError> {
        self.solve_case_with_visit_order(
            case.into(),
            options,
            self.source_visit_order().map(SourceVisitOrder::ordinals),
            observe,
        )
    }

    /// Diagnose a different first-hit search by visiting the stored
    /// preconditioned basis in `source_order` at every seed.
    ///
    /// The slice must be a permutation of `0..self.basis().len()`. Validation
    /// precedes all search work and observer events. Basis storage, source
    /// provenance, integral ordering, and seed enumeration are unchanged;
    /// returned [`SeedSource::basis_row`] values remain stored basis indices,
    /// not positions in this slice. Every row is visited once per seed unless
    /// the ordinary first successful hit ends the search.
    ///
    /// This isolated-case diagnostic does not change sector solving defaults,
    /// retry another schedule, analyze exceptional cases, or publish a rule.
    pub fn solve_case_with_source_order_and_observer(
        &self,
        case: impl Into<Case<N>>,
        options: SearchOptions,
        source_order: &[usize],
        observe: impl FnMut(SearchEvent<N>),
    ) -> Result<RuleCandidate<N>, SolverError> {
        super::discovery_strategy::validate_visit_order(source_order, self.basis.len())?;
        self.solve_case_with_visit_order(case.into(), options, Some(source_order), observe)
    }

    fn solve_case_with_visit_order(
        &self,
        case: Case<N>,
        options: SearchOptions,
        source_order: Option<&[usize]>,
        mut observe: impl FnMut(SearchEvent<N>),
    ) -> Result<RuleCandidate<N>, SolverError> {
        self.validate_search(&case, options)?;
        self.search_validated_case(case, options, source_order, &mut observe)
    }

    pub(super) fn validate_search(
        &self,
        case: &Case<N>,
        options: SearchOptions,
    ) -> Result<(), SolverError> {
        self.validate_case(case)?;
        if options.prime < 3 || !Integer::from(options.prime).is_prime(0) {
            return Err(SolverError::InvalidInput(
                "modular probe requires an odd prime".into(),
            ));
        }
        Ok(())
    }

    /// Bind a queued case to this source system before any work or publication.
    pub(super) fn validate_case(&self, case: &Case<N>) -> Result<(), SolverError> {
        if !case.is_in_sector(self.order.sector()) {
            return Err(SolverError::InvalidInput(
                "case lies outside its sector".into(),
            ));
        }
        if case.fixed()[self.system.active_arity()..]
            .iter()
            .any(|value| *value != Some(0))
        {
            return Err(SolverError::InvalidInput(
                "padding coordinates must remain fixed to zero".into(),
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
        Ok(())
    }

    fn search_validated_case(
        &self,
        case: Case<N>,
        options: SearchOptions,
        source_order: Option<&[usize]>,
        observe: impl FnMut(SearchEvent<N>),
    ) -> Result<RuleCandidate<N>, SolverError> {
        match self
            .search_attempt(case, options, source_order, None, false, observe)
            .0
        {
            Ok(candidate) => Ok(candidate),
            Err(TrialSearchError::Solver(error)) => Err(error),
            Err(TrialSearchError::Limit(_)) => unreachable!("unlimited search has no trial cap"),
        }
    }

    /// Already-validated queued case. The caller validates the same case/prime
    /// before the baseline, so optional attempts cannot bypass those checks.
    pub(super) fn search_attempt(
        &self,
        case: Case<N>,
        mut options: SearchOptions,
        source_order: Option<&[usize]>,
        limits: Option<RuleTrialLimits>,
        account_trace: bool,
        observe: impl FnMut(SearchEvent<N>),
    ) -> (Result<RuleCandidate<N>, TrialSearchError>, RuleTrialStats) {
        if let Some(limits) = limits {
            options.max_depth = Some(
                options
                    .max_depth
                    .map_or(limits.max_depth, |d| d.min(limits.max_depth)),
            );
        }
        let start = Instant::now();
        let mut work = RuleTrialStats::default();
        let mut result = self.search_attempt_inner(
            case,
            options,
            source_order,
            limits,
            account_trace,
            &mut work,
            observe,
            None,
        );
        work.search.elapsed = start.elapsed();
        if let Ok(candidate) = &mut result {
            candidate.stats = work.search;
        }
        (result, work)
    }

    fn search_attempt_inner(
        &self,
        case: Case<N>,
        options: SearchOptions,
        source_order: Option<&[usize]>,
        limits: Option<RuleTrialLimits>,
        account_trace: bool,
        work: &mut RuleTrialStats,
        mut observe: impl FnMut(SearchEvent<N>),
        guarded: Option<&super::guarded::GuardedSearchScope<'_, N>>,
    ) -> Result<RuleCandidate<N>, TrialSearchError> {
        let stats = &mut work.search;
        let mut probe = None;
        let mut original_rows = Vec::new();
        let mut original_sources = Vec::new();
        let initial = case.integral();
        let mut seeds = Seeds::new(initial, *self.order.sector(), self.seed_frozen());
        if let Some(scope) = guarded {
            seeds = seeds.with_crossing(
                scope
                    .problem
                    .roles
                    .map(|role| role == super::guarded::IndexRole::Occupation),
            );
        }
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
                }
                .into());
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
            for position in 0..self.basis.len() {
                if limits.is_some_and(|limit| stats.rows >= limit.max_rows) {
                    return Err(TrialSearchError::Limit(RuleTrialBudget::SourceRows));
                }
                let ordinal = source_order.map_or(position, |order| order[position]);
                let source = &self.basis[ordinal];
                if account_trace {
                    stats.rows += 1;
                }
                let row = if let Some(scope) = guarded {
                    let Some(row) = scope.instantiate(ordinal, source, &seed, &self.order)? else {
                        continue;
                    };
                    row
                } else {
                    instantiate(
                        source,
                        &seed,
                        &self.system.indices,
                        self.system.fixed(),
                        &self.order,
                        discovery_zero_sectors,
                        case.affine(),
                    )?
                };
                if !account_trace {
                    stats.rows += 1;
                }
                let Some(leading) = row.first() else { continue };
                let source = SeedSource {
                    basis_row: ordinal,
                    seed,
                };
                if case.matches(&leading.integral) {
                    stats.direct_hit = true;
                    stats.exact_trace_rows = 1;
                    work.exact_trace_terms = row.len();
                    if limits.is_some_and(|limit| row.len() > limit.max_exact_trace_terms) {
                        return Err(TrialSearchError::Limit(RuleTrialBudget::ExactTraceTerms));
                    }
                    observe(SearchEvent::CanonicalizationStarted {
                        terms: row.len(),
                        direct_hit: true,
                    });
                    let (target, rhs) = canonicalize(row, &self.system.indices)?;
                    stats.discovery = probe.as_ref().map(|p: &Probe<N>| p.discovery.stats());
                    return Ok(RuleCandidate {
                        case,
                        target,
                        rhs,
                        sources: vec![source],
                        stats: *stats,
                    });
                }
                if case.is_numerical()
                    && self.order.compare(&initial, &leading.integral) == Ordering::Less
                {
                    continue;
                }
                let active_probe = probe.get_or_insert_with(|| {
                    Probe::new(self.order.clone(), self.system.variable_count, options)
                });
                let modular_row = active_probe.evaluate(&row)?;
                if let Some(pivot) = active_probe.discovery.add_row(&modular_row) {
                    original_rows.push(row);
                    original_sources.push(source);
                    stats.independent_rows += 1;
                    if case.matches(&pivot) {
                        let trace = if let Some(limit) = limits {
                            active_probe
                                .discovery
                                .trace_bounded(
                                    original_rows.len() - 1,
                                    limit.max_exact_trace_rows,
                                    limit.max_exact_trace_terms,
                                    |row| {
                                        let terms = original_rows[row].len();
                                        stats.exact_trace_rows += 1;
                                        work.exact_trace_terms =
                                            work.exact_trace_terms.saturating_add(terms);
                                        terms
                                    },
                                )
                                .map_err(TrialSearchError::Limit)?
                        } else {
                            active_probe.discovery.trace(original_rows.len() - 1)
                        };
                        let discovery = active_probe.discovery.stats();
                        stats.discovery = Some(discovery);
                        stats.exact_trace_rows = trace.len();
                        if account_trace && limits.is_none() {
                            work.exact_trace_terms =
                                trace.iter().map(|&row| original_rows[row].len()).sum();
                        }
                        let selected = trace
                            .iter()
                            .map(|i| std::mem::take(&mut original_rows[*i]))
                            .collect::<Vec<_>>();
                        let sources = trace.iter().map(|i| original_sources[*i]).collect();
                        // This case ends at the first matching pivot. Release
                        // nonwinning exact rows before the expensive native solve.
                        drop(original_rows);
                        // The first winning trace is final: exact lifting uses
                        // only selected rows, not the modular reducer. Release
                        // its owning Option, not merely the borrowed probe.
                        drop(probe.take());
                        observe(SearchEvent::ExactStarted {
                            pivot,
                            trace_rows: trace.len(),
                            discovery,
                        });
                        let exact_start = Instant::now();
                        work.exact_lifts += 1;
                        let exact = exact_materialize_using_with_observer(
                            &selected,
                            &self.order,
                            pivot,
                            self.config.symbolic_exact_backend,
                            self.config.coefficient_variable_order,
                            self.system.coefficient_order(),
                            |event| observe(SearchEvent::ExactProgress(event)),
                        );
                        stats.exact_materialization = exact_start.elapsed();
                        let exact =
                            exact.map_err(|error| SolverError::ExactReplay(error.to_string()))?;
                        observe(SearchEvent::CanonicalizationStarted {
                            terms: exact.len(),
                            direct_hit: false,
                        });
                        let (target, rhs) = canonicalize(exact, &self.system.indices)?;
                        stats.exact_materialization = exact_start.elapsed();
                        stats.discovery = Some(discovery);
                        return Ok(RuleCandidate {
                            case,
                            target,
                            rhs,
                            sources,
                            stats: *stats,
                        });
                    }
                }
            }
        }
        Err(SolverError::SearchExhausted {
            depth: seeds.depth() as u32,
            rows: stats.rows,
        }
        .into())
    }
}

pub(super) enum TrialSearchError {
    Solver(SolverError),
    Limit(RuleTrialBudget),
}

impl From<SolverError> for TrialSearchError {
    fn from(error: SolverError) -> Self {
        Self::Solver(error)
    }
}

impl From<super::PowerError> for TrialSearchError {
    fn from(error: super::PowerError) -> Self {
        Self::Solver(error.into())
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

#[cfg(test)]
mod source_order_tests;
