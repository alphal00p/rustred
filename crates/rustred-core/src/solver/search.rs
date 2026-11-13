use std::cmp::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::{Rng, SeedableRng, rngs::StdRng};
use symbolica::domains::InternalOrdering;
use symbolica::domains::finite_field::{FiniteFieldElement, ToFiniteField, Zp64};
use symbolica::prelude::{Field, Integer, Ring};

use crate::algebra::CoefficientPolynomial;

use super::discovery::{Discovery, DiscoveryStats, exact_materialize_witnessed};
use super::instantiate::{canonical_shifts, canonicalize, instantiate, translate_polynomial};
use super::precondition::{
    PreconditionProvenance, precondition_with_provenance, precondition_with_variable_order,
};
use super::seed::CaseSeedStream;
use super::{
    Case, CaseSeedSchedule, CoefficientVariableOrder, ExactRow, Integral, IntegralOrder,
    PolynomialRow, RuleSelectionPolicy, RuleTrialBudget, RuleTrialLimits, RuleTrialStats, Seed,
    SolverError, SourceDiscoveryStrategy, SourceSystem, SourceVisitOrder, SymbolicExactBackend,
    Term,
};

mod observation;
pub use observation::SearchEvent;
mod study;
pub use study::{StudyRow, TraceStudy};

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
    /// Seed order inside each L1 shell of the symbolic case search (opt-in; default SpIRed chronology).
    pub seed_schedule: super::SeedSchedule,
    /// guard-fix (default ON): carry the source-weight poles of every lift
    /// into [`RuleCandidate::hidden_poles`] (and so into the rule's guards).
    /// A backend that keeps no witness (dense fraction-free, semi-numerical)
    /// is re-lifted with the witnessed target-only factorized backend, whose
    /// row and witness are used. Off = the old
    /// RHS-denominator-only guards (A/B and legacy reproduction only).
    pub witness_guards: bool,
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
            seed_schedule: super::SeedSchedule::default(),
            witness_guards: true,
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
    /// guard-fix (2026-11-12): witness poles of the lift that produced this
    /// candidate, in the candidate's CANONICAL frame (same variable map as the
    /// RHS coefficients): the denominators of the source weights
    /// (`discovery::WitnessPoles`), and for a direct hit the winning row's
    /// pivot. Where one vanishes the retained witness proves nothing, even if
    /// every final RHS coefficient is regular there. Guard extraction treats
    /// their index-containing factors exactly like RHS denominators. Empty for
    /// hand-built candidates and with `SectorConfig::witness_guards` off.
    pub hidden_poles: Vec<CoefficientPolynomial>,
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
    /// Whether rule exceptions include the witness (source-weight) poles of the lift (guard-fix; table
    /// `guard_version` 2 when on, 1 when off).
    pub fn witness_guards(&self) -> bool {
        self.config.witness_guards
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

    /// Node-only pass-through to the existing finite scheduled search. No
    /// mandatory Reference trial, policy change, guard admission or publication.
    pub(crate) fn solve_case_with_schedule_and_observer(
        &self,
        case: impl Into<Case<N>>,
        options: SearchOptions,
        source_order: Option<&[usize]>,
        seed_schedule: &CaseSeedSchedule,
        observe: impl FnMut(SearchEvent<N>),
    ) -> Result<RuleCandidate<N>, SolverError> {
        let case = case.into();
        seed_schedule.validate(N)?;
        let source_order =
            source_order.or_else(|| self.source_visit_order().map(SourceVisitOrder::ordinals));
        if let Some(order) = source_order {
            super::discovery_strategy::validate_visit_order(order, self.basis.len())?;
        }
        self.validate_search(&case, options)?;
        match self
            .search_attempt_with_schedule(
                case,
                options,
                source_order,
                None,
                false,
                seed_schedule,
                observe,
            )
            .0
        {
            Ok(candidate) => Ok(candidate),
            Err(TrialSearchError::Solver(error)) => Err(error),
            Err(TrialSearchError::Limit(_)) => unreachable!("unlimited search has no trial cap"),
        }
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
        options: SearchOptions,
        source_order: Option<&[usize]>,
        limits: Option<RuleTrialLimits>,
        account_trace: bool,
        observe: impl FnMut(SearchEvent<N>),
    ) -> (Result<RuleCandidate<N>, TrialSearchError>, RuleTrialStats) {
        self.search_attempt_with_schedule(
            case,
            options,
            source_order,
            limits,
            account_trace,
            &CaseSeedSchedule::Reference,
            observe,
        )
    }

    /// An opt-in trial can change actual seed availability/chronology without
    /// changing the prepared basis, source ordinals, or exact replay authority.
    pub(super) fn search_attempt_with_schedule(
        &self,
        case: Case<N>,
        mut options: SearchOptions,
        source_order: Option<&[usize]>,
        limits: Option<RuleTrialLimits>,
        account_trace: bool,
        seed_schedule: &CaseSeedSchedule,
        observe: impl FnMut(SearchEvent<N>),
    ) -> (Result<RuleCandidate<N>, TrialSearchError>, RuleTrialStats) {
        if let Some(limits) = limits {
            options.max_depth = Some(
                options
                    .max_depth
                    .map_or(limits.max_depth, |d| d.min(limits.max_depth)),
            );
        }
        if let Some(last_depth) = seed_schedule.last_depth() {
            options.max_depth = Some(options.max_depth.map_or(last_depth, |d| d.min(last_depth)));
        }
        let start = Instant::now();
        let mut work = RuleTrialStats::default();
        let mut result = self.search_attempt_inner(
            case,
            options,
            source_order,
            limits,
            account_trace,
            seed_schedule,
            &mut work,
            observe,
        );
        work.search.elapsed = start.elapsed();
        if let Ok(candidate) = &mut result {
            candidate.stats = work.search;
        }
        (result, work)
    }

    #[allow(clippy::too_many_arguments)]
    fn search_attempt_inner(
        &self,
        case: Case<N>,
        options: SearchOptions,
        source_order: Option<&[usize]>,
        limits: Option<RuleTrialLimits>,
        account_trace: bool,
        seed_schedule: &CaseSeedSchedule,
        work: &mut RuleTrialStats,
        mut observe: impl FnMut(SearchEvent<N>),
    ) -> Result<RuleCandidate<N>, TrialSearchError> {
        let stats = &mut work.search;
        let mut probe = None;
        let mut original_rows = Vec::new();
        let mut original_sources = Vec::new();
        let initial = case.integral();
        // Astra's shifted-shell stream (CaseSeedSchedule) under the opt-in in-shell SeedSchedule (SymbolicFirst).
        let mut seeds = super::ScheduledSeeds::over(
            CaseSeedStream::new(
                initial,
                *self.order.sector(),
                self.seed_frozen(),
                self.order.two_sided_axes(),
                seed_schedule,
            )?
            .with_search_max_depth(options.max_depth),
            initial,
            crate::node::screen::seed_schedule_override().unwrap_or(self.config.seed_schedule),
        );
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
            work.seed_first_depth.get_or_insert(depth);
            work.seed_last_depth = Some(depth);
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
                let row = instantiate(
                    source,
                    &seed,
                    &self.system.indices,
                    self.system.fixed(),
                    &self.order,
                    discovery_zero_sectors,
                    case.affine(),
                )?;
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
                    let hidden_poles = if self.config.witness_guards {
                        // The single row is its own witness (weight 1/pivot): its pivot and
                        // input denominators may cancel from the normalised RHS.
                        let shifts = canonical_shifts(&leading.integral);
                        let mut poles: Vec<_> = row
                            .iter()
                            .map(|term| &term.coefficient.denominator)
                            .chain(std::iter::once(&leading.coefficient.numerator))
                            .filter(|poly| !poly.is_constant())
                            .map(|poly| translate_polynomial(poly, &self.system.indices, &shifts))
                            .collect();
                        poles.sort_unstable_by(InternalOrdering::internal_cmp);
                        poles.dedup();
                        poles
                    } else {
                        Vec::new()
                    };
                    let (target, rhs) = canonicalize(row, &self.system.indices)?;
                    stats.discovery = probe.as_ref().map(|p: &Probe<N>| p.discovery.stats());
                    return Ok(RuleCandidate {
                        case,
                        target,
                        rhs,
                        sources: vec![source],
                        stats: *stats,
                        hidden_poles,
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
                        let sources: Vec<SeedSource<N>> =
                            trace.iter().map(|i| original_sources[*i]).collect();
                        if crate::node::screen::active().is_some() {
                            crate::node::screen::note_trace(
                                sources
                                    .iter()
                                    .map(|s| {
                                        // full seed displacement from the case point (numeric moves included)
                                        let delta = (0..N)
                                            .map(|i| {
                                                i32::from(s.seed.integral[i].value())
                                                    - i32::from(initial[i].value())
                                            })
                                            .collect();
                                        (s.basis_row, delta)
                                    })
                                    .collect(),
                            );
                        }
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
                        let exact = exact_materialize_witnessed(
                            &selected,
                            &self.order,
                            pivot,
                            self.config.symbolic_exact_backend,
                            self.config.coefficient_variable_order,
                            self.system.coefficient_order(),
                            |event| observe(SearchEvent::ExactProgress(event)),
                        );
                        stats.exact_materialization = exact_start.elapsed();
                        let (mut exact, mut witness) = exact.map_err(SolverError::exact_replay)?;
                        if self.config.witness_guards && !witness.recorded {
                            // guard-fix: the diagnostic backends (dense fraction-free,
                            // semi-numerical) keep no source weights. Re-lift the same selected
                            // rows with the witnessed target-only factorized backend and take its
                            // row and witness: the rule is then exactly the one the production
                            // backend proves, guards included. A different row is reported.
                            let (lifted, lifted_witness) = exact_materialize_witnessed(
                                &selected,
                                &self.order,
                                pivot,
                                SymbolicExactBackend::SparseTargetOnlyFactorized,
                                self.config.coefficient_variable_order,
                                self.system.coefficient_order(),
                                |event| observe(SearchEvent::ExactProgress(event)),
                            )
                            .map_err(SolverError::exact_replay)?;
                            if !lifted_witness.recorded {
                                return Err(SolverError::ExactReplay(
                                    "guard-fix: witness re-lift kept no source-weight witness"
                                        .into(),
                                )
                                .into());
                            }
                            if canonicalize(exact.clone(), &self.system.indices)?
                                != canonicalize(lifted.clone(), &self.system.indices)?
                            {
                                eprintln!(
                                    "guard-fix: backend {:?} row differs from the witnessed target-only row; the witnessed row is used",
                                    self.config.symbolic_exact_backend
                                );
                            }
                            exact = lifted;
                            witness = lifted_witness;
                        }
                        let hidden_poles = if self.config.witness_guards {
                            let head = exact.first().ok_or_else(|| {
                                SolverError::ExactReplay("empty winning row".into())
                            })?;
                            let shifts = canonical_shifts(&head.integral);
                            let mut poles: Vec<_> = witness
                                .denominators
                                .iter()
                                .map(|poly| {
                                    translate_polynomial(poly, &self.system.indices, &shifts)
                                })
                                .collect();
                            poles.sort_unstable_by(InternalOrdering::internal_cmp);
                            poles.dedup();
                            poles
                        } else {
                            Vec::new()
                        };
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
                            hidden_poles,
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
        let mut point: Vec<FiniteFieldElement<u64>> = (0..nvars)
            .map(|_| Integer::from(random.random_range(1..options.prime)).to_finite_field(&field))
            .collect();
        // [p-integral round6 native-S1c, diagnostic only] RUSTRED_PROBE_POINT = comma-separated decimal residues, one per
        // coefficient variable (the source system's coefficient order), replaces the random modular discovery point.
        // Unset (the default): the established engine, byte-identical behaviour. The exact lift is unchanged.
        if let Ok(spec) = std::env::var("RUSTRED_PROBE_POINT") {
            let values: Vec<&str> = spec
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .collect();
            assert_eq!(
                values.len(),
                nvars,
                "RUSTRED_PROBE_POINT needs one residue per coefficient variable"
            );
            point = values
                .iter()
                .map(|v| {
                    Integer::from(
                        v.parse::<u64>().expect("RUSTRED_PROBE_POINT residue") % options.prime,
                    )
                    .to_finite_field(&field)
                })
                .collect();
            eprintln!("[probe-point] modular discovery point overridden ({nvars} residues)");
        }
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
