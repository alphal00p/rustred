//! Runtime-arity access to the existing exact sector solver.
//!
//! This module only adapts ownership and integral keys. Source derivation,
//! parametric discovery, and finite Laporta elimination are the same native
//! algorithms used by [`super::SectorSolver`]. Finite-search residuals are a
//! basis for the returned equations, not a proof of master independence.
//!
//! Runtime entry points use [`crate::compiled_runtime_arities`] (default 1..=16;
//! configurable at build time with `RUSTRED_RUNTIME_ARITIES=1,2,13,14,15`). The
//! checked `*_for::<N>` entry points do not depend on that registry. This is a
//! compiled capability boundary, not a mathematical maximum denominator count.
//! Downstream direct solver hosts can reuse [`crate::dispatch_arity!`] with the
//! compiled registry or their own explicit arity list.

mod basis;
mod capacity;
mod census;
mod certificate;
mod combination;

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use symbolica::poly::PolyVariable;

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::IntegralFamily;
use crate::sector::{
    CutConstraint, Mask, Pattern, Restrictions,
    zero::{Analyzer, Decision},
};

use super::{
    CoordinateCase, RuleCandidate, SearchOptions, SectorConfig, SectorSolver, SolverError,
    SourceSystem, extract_exceptions,
};

pub use basis::{BasisChange, PreferredMaster, PreferredStatus};
pub use capacity::{
    certify_laporta_with_capacity, solve_laporta_with_capacity, solve_parametric_with_capacity,
};

pub use certificate::{ReductionCertificate, certify_laporta, certify_laporta_for};

/// Number of consecutive deeper searches that must reproduce a residual set
/// before `until_stable` accepts it. One is not enough: some numerator
/// integrals keep the same residuals from depth 0 to 1 and change at depth 2.
pub const STABLE_PATIENCE: u32 = 2;

/// Search radius in the signed L1 seed lattice. Parameters remain exact.
#[derive(Clone, Copy, Debug)]
pub struct DynamicSolveOptions {
    pub max_depth: u32,
    pub include_lorentz: bool,
    /// Maximum distinct concrete integrals searched during RHS closure.
    /// Exceeding this budget is an error, never an incomplete reduction.
    pub max_targets: usize,
    /// Laporta only: search depths `0..=max_depth` and stop once the residual
    /// set has stayed unchanged for [`STABLE_PATIENCE`] deeper searches. This
    /// is a heuristic for search completeness, not a proof.
    pub until_stable: bool,
}

impl Default for DynamicSolveOptions {
    fn default() -> Self {
        Self {
            max_depth: 2,
            include_lorentz: false,
            max_targets: 1024,
            until_stable: false,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct DynamicPower {
    /// A symbolic power is `n_i + value`; otherwise it is the integer `value`.
    pub symbolic: bool,
    pub value: i16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DynamicTerm {
    pub powers: Vec<DynamicPower>,
    pub coefficient: Coefficient,
}

#[derive(Clone, Debug)]
pub struct DynamicRule {
    pub sector: Vec<bool>,
    pub target: Vec<DynamicPower>,
    pub rhs: Vec<DynamicTerm>,
    /// All inherited source conditions and explicit coefficient poles. Every
    /// polynomial must remain nonzero under an intended specialization.
    pub nonzero_conditions: Vec<CoefficientPolynomial>,
    /// Index exceptions: outer OR, inner AND. A true branch forbids use of
    /// this rule, including inactive-coordinate activation boundaries.
    pub exceptions: Vec<Vec<CoefficientPolynomial>>,
}

#[derive(Clone, Debug, Default)]
pub struct DynamicSolveStats {
    pub sectors: usize,
    pub seeds: usize,
    pub rows: usize,
    pub exact_trace_rows: usize,
}

/// Where a Laporta rule, before back substitution, comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleOrigin {
    /// A combination of the family's original identities instantiated at these
    /// concrete seed integrals.
    Identities { seeds: Vec<Vec<i16>> },
    /// The target's sector is scaleless by an exact zero-sector proof.
    ProvedZero,
    /// The target has a nonpositive power on a cut denominator.
    OutsideCut,
}

/// A Laporta rule before back substitution, kept so that a certificate can
/// derive it again from the original identities.
#[derive(Clone, Debug)]
pub struct DerivedRule {
    pub rule: DynamicRule,
    pub origin: RuleOrigin,
}

#[derive(Clone, Debug, Default)]
pub struct DynamicSolution {
    pub rules: Vec<DynamicRule>,
    /// Unsolved requested integrals and terminal RHS integrals. These depend
    /// on the finite seed budget and are not certified master integrals.
    /// After a preferred-master basis change, every preferred integral is one
    /// of them, whether or not a target needs it.
    pub residuals: Vec<Vec<i16>>,
    pub index_variables: Vec<PolyVariable>,
    pub stats: DynamicSolveStats,
    /// Laporta only: the rules before back substitution and any basis change.
    pub derivation: Vec<DerivedRule>,
    /// Laporta only: the requested targets, then any preferred masters.
    pub requested: Vec<Vec<i16>>,
    /// Laporta only: the preferred-master basis change, if one was requested.
    pub basis_change: Option<BasisChange>,
    /// Laporta only: the seed depth of the returned search.
    pub depth: u32,
    /// With `until_stable`, the first depth whose residuals the next
    /// [`STABLE_PATIENCE`] depths reproduced; `None` if none did.
    pub stable_depth: Option<u32>,
}

/// Find one reusable symbolic-index recurrence on an explicit sector/case.
/// Fixed coordinates use absolute powers; `None` coordinates remain symbolic.
/// Exhausting the finite search radius is an error, never a solved rule.
///
/// `cuts` selects reverse-unitarity (`CutDs`) denominators; integrals with a
/// nonpositive power on one of them vanish by definition. Discovery drops terms
/// that leave the cut through a fixed cut index. Terms that leave it through a
/// symbolic cut index remain on the right-hand side and vanish once the index
/// is specialized; with every cut index symbolic, the rule is the uncut rule.
/// `CutConstraint::none` is the ordinary family.
pub fn solve_parametric(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    sector: &[bool],
    fixed: &[Option<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    dispatch_solver_capacity!(
        family.denominator_count(),
        solve_parametric_with_capacity(family, cuts, sector, fixed, options),
        arity => Err(unsupported_runtime_arity(arity))
    )
}

/// Reduce finite targets by the native shared-seed Laporta search and exact
/// replay. Every newly encountered RHS integral receives the same bounded
/// search, up to `max_targets`. Solved targets are back substituted across
/// sectors. Unresolved RHS integrals remain explicit.
///
/// Integrals outside the support of `cuts` vanish, as in [`solve_parametric`].
/// The integral order is that of the uncut family, so a cut reduction equals
/// the uncut one with every integral outside the cut support removed.
/// Requested targets outside the support receive zero rules without a search.
///
/// `preferred` integrals are searched like targets and then become residuals
/// through an exact basis change; see [`BasisChange`]. A preferred integral
/// outside the cut, in a zero sector, reducing to zero or to lower sectors, or
/// dependent on other preferred integrals of its sector is an error.
pub fn solve_laporta(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    targets: &[Vec<i16>],
    preferred: &[Vec<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    dispatch_solver_capacity!(
        family.denominator_count(),
        solve_laporta_with_capacity(family, cuts, targets, preferred, options),
        arity => Err(unsupported_runtime_arity(arity))
    )
}

/// Use the exact parametric solver at an explicitly compiled arity.
///
/// This bypasses only runtime dispatch, not family, cut, case, or solver checks.
/// `N` must be positive and equal the family's denominator count. Search options
/// and all mathematical semantics are identical to [`solve_parametric`].
pub fn solve_parametric_for<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    sector: &[bool],
    fixed: &[Option<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    validate_family_arity::<N>(family)?;
    parametric::<N>(family, cuts, sector, fixed, options)
}

/// Use finite Laporta reduction at an explicitly compiled arity.
///
/// All cuts, preferred-master changes, search budgets, and optional residual
/// stabilization have the same semantics as [`solve_laporta`]. No membership in
/// [`crate::compiled_runtime_arities`] is required.
pub fn solve_laporta_for<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    targets: &[Vec<i16>],
    preferred: &[Vec<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    validate_family_arity::<N>(family)?;
    laporta::<N>(family, cuts, targets, preferred, options)
}

fn validate_family_arity<const N: usize>(family: &IntegralFamily) -> Result<(), SolverError> {
    if N == 0 || N != family.denominator_count() {
        return Err(SolverError::InvalidInput(format!(
            "const-generic arity {N} must be positive and equal the family denominator count {}",
            family.denominator_count()
        )));
    }
    Ok(())
}

fn unsupported_runtime_arity(arity: usize) -> SolverError {
    SolverError::InvalidInput(format!(
        "runtime bridge was compiled for arities {:?}; received {arity}; use a checked *_for::<N> entry point or rebuild with RUSTRED_RUNTIME_ARITIES",
        crate::compiled_runtime_arities()
    ))
}

fn search(options: DynamicSolveOptions) -> SearchOptions {
    SearchOptions {
        max_depth: Some(options.max_depth),
        ..Default::default()
    }
}

fn array<T: Copy, const N: usize>(values: &[T], label: &str) -> Result<[T; N], SolverError> {
    values.try_into().map_err(|_| {
        SolverError::InvalidInput(format!(
            "{label} has {} coordinates; expected {N}",
            values.len()
        ))
    })
}

fn index_variables<const N: usize>(sources: &SourceSystem<N>) -> Vec<PolyVariable> {
    let variables = sources.coefficient_variables();
    sources
        .index_variables()
        .iter()
        .map(|index| variables[*index].clone())
        .collect()
}

fn dynamic_powers<const N: usize>(integral: super::Integral<N>) -> Vec<DynamicPower> {
    integral
        .powers()
        .iter()
        .map(|power| DynamicPower {
            symbolic: power.is_symbolic(),
            value: power.value(),
        })
        .collect()
}

fn export<const N: usize>(
    candidate: RuleCandidate<N>,
    sources: &SourceSystem<N>,
    sector: &[bool; N],
) -> Result<DynamicRule, SolverError> {
    let exceptions = extract_exceptions(&candidate, sources.index_variables(), sector)
        .map_err(|error| SolverError::ExactReplay(error.to_string()))?
        .branches;
    let powers = dynamic_powers::<N>;
    let mut nonzero_conditions = sources.conditions().to_vec();
    for term in &candidate.rhs {
        let denominator = &term.coefficient.denominator;
        if !denominator.is_constant() && !nonzero_conditions.contains(denominator) {
            nonzero_conditions.push(denominator.clone());
        }
    }
    Ok(DynamicRule {
        sector: sector.to_vec(),
        target: powers(candidate.target),
        rhs: candidate
            .rhs
            .into_iter()
            .map(|term| DynamicTerm {
                powers: powers(term.integral),
                coefficient: term.coefficient,
            })
            .collect(),
        nonzero_conditions,
        exceptions,
    })
}

/// A rule for a vanishing target: either by a zero proof, valid under the
/// family's domain `conditions`, or by lying outside the cut, which needs none.
fn zero_rule<const N: usize>(
    case: &CoordinateCase<N>,
    sector: &[bool; N],
    conditions: Vec<CoefficientPolynomial>,
) -> DynamicRule {
    DynamicRule {
        sector: sector.to_vec(),
        target: dynamic_powers(case.integral()),
        rhs: Vec::new(),
        nonzero_conditions: conditions,
        exceptions: Vec::new(),
    }
}

fn parametric<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    sector: &[bool],
    fixed: &[Option<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    let sector = array::<_, N>(sector, "sector")?;
    let case = CoordinateCase::new(array(fixed, "fixed indices")?)?;
    let sources = SourceSystem::<N>::from_family_with_capacity(family, options.include_lorentz)?;
    let restrictions = cut_restrictions(family, cuts)?;
    if !case.is_in_sector(&sector) {
        return Err(SolverError::InvalidInput(
            "case lies outside its sector".into(),
        ));
    }
    if is_excluded(&restrictions, sector)? {
        return Ok(DynamicSolution {
            rules: vec![zero_rule(&case, &sector, Vec::new())],
            index_variables: index_variables(&sources),
            ..Default::default()
        });
    }
    let (zero_sectors, zero_conditions) = zero_census(family, &restrictions, &[sector])?;
    if zero_sectors.contains(&sector) {
        return Ok(DynamicSolution {
            rules: vec![zero_rule(&case, &sector, zero_conditions)],
            index_variables: index_variables(&sources),
            ..Default::default()
        });
    }
    let solver = SectorSolver::new(
        &sources,
        sector,
        SectorConfig {
            zero_sectors: zero_sectors.into(),
            ..Default::default()
        },
    )?;
    let candidate = solver.solve_case(case, search(options))?;
    let stats = DynamicSolveStats {
        sectors: 1,
        seeds: candidate.stats.seeds,
        rows: candidate.stats.rows,
        exact_trace_rows: candidate.stats.exact_trace_rows,
    };
    let mut rule = export(candidate, &sources, &sector)?;
    extend_conditions(&mut rule.nonzero_conditions, &zero_conditions);
    Ok(DynamicSolution {
        rules: vec![rule],
        index_variables: index_variables(&sources),
        stats,
        ..Default::default()
    })
}

/// The concrete seed integrals whose original identities a rule combines.
fn seed_integrals<const N: usize>(candidate: &RuleCandidate<N>) -> Vec<Vec<i16>> {
    let seeds: BTreeSet<Vec<i16>> = candidate
        .sources
        .iter()
        .map(|source| {
            source
                .seed
                .integral
                .powers()
                .iter()
                .map(|power| power.value())
                .collect()
        })
        .collect();
    seeds.into_iter().collect()
}

fn laporta<const N: usize>(
    family: &IntegralFamily,
    cuts: &CutConstraint,
    targets: &[Vec<i16>],
    preferred: &[Vec<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    let restrictions = cut_restrictions(family, cuts)?;
    let mut requested = BTreeSet::<[i16; N]>::new();
    // Outside the cut support no search is needed, so these targets neither
    // count against `max_targets` nor widen the zero census.
    let mut excluded = BTreeMap::<[i16; N], CoordinateCase<N>>::new();
    for target in targets {
        let powers = array::<_, N>(target, "target")?;
        let case = CoordinateCase::new(powers.map(Some))?;
        if is_excluded(&restrictions, powers.map(|power| power > 0))? {
            excluded.insert(powers, case);
        } else {
            requested.insert(powers);
        }
    }
    let preferred = basis::validate::<N>(preferred, &restrictions)?;
    requested.extend(preferred.iter().copied());
    let sources = SourceSystem::<N>::from_family_with_capacity(family, options.include_lorentz)?;
    let initial_sectors: Vec<_> = requested
        .iter()
        .map(|powers| powers.map(|power| power > 0))
        .collect();
    let (zero_sectors, zero_conditions) = zero_census(family, &restrictions, &initial_sectors)?;
    if let Some(master) = preferred
        .iter()
        .find(|master| zero_sectors.contains(&master.map(|power| power > 0)))
    {
        return Err(SolverError::InvalidInput(format!(
            "preferred master {master:?} lies in a zero sector; it vanishes"
        )));
    }
    let mut closure = Closure {
        sources: &sources,
        zero_sectors: zero_sectors.into(),
        zero_conditions,
        solvers: BTreeMap::new(),
    };
    let mut result = if options.until_stable {
        closure.until_stable(&requested, options)?
    } else {
        closure.run(&requested, options, options.max_depth)?
    };
    for (powers, case) in excluded {
        let rule = zero_rule(&case, &powers.map(|power| power > 0), Vec::new());
        result.derivation.push(DerivedRule {
            rule: rule.clone(),
            origin: RuleOrigin::OutsideCut,
        });
        result.rules.push(rule);
    }
    result.index_variables = index_variables(&sources);
    result.requested = targets
        .iter()
        .cloned()
        .chain(preferred.iter().map(|master| master.to_vec()))
        .collect();
    if !preferred.is_empty() {
        basis::prefer(&mut result, &preferred)?;
    }
    Ok(result)
}

/// One Laporta closure over a fixed family, census and set of sector solvers,
/// so that deeper searches reuse each sector's preconditioned identities.
struct Closure<'s, const N: usize> {
    sources: &'s SourceSystem<N>,
    zero_sectors: Arc<[[bool; N]]>,
    zero_conditions: Vec<CoefficientPolynomial>,
    solvers: BTreeMap<[bool; N], SectorSolver<'s, N>>,
}

impl<'s, const N: usize> Closure<'s, N> {
    fn run(
        &mut self,
        requested: &BTreeSet<[i16; N]>,
        options: DynamicSolveOptions,
        depth: u32,
    ) -> Result<DynamicSolution, SolverError> {
        let search = search(DynamicSolveOptions {
            max_depth: depth,
            ..options
        });
        let mut pending = requested.clone();
        let mut result = DynamicSolution {
            depth,
            ..Default::default()
        };
        let mut visited = BTreeSet::new();
        while !pending.is_empty() {
            if visited.len().saturating_add(pending.len()) > options.max_targets {
                return Err(SolverError::InvalidInput(format!(
                    "Laporta RHS closure exceeds max_targets={}; increase the explicit target budget, which also counts preferred masters",
                    options.max_targets
                )));
            }
            let mut sectors = BTreeMap::<[bool; N], Vec<CoordinateCase<N>>>::new();
            for target in std::mem::take(&mut pending) {
                visited.insert(target);
                sectors
                    .entry(target.map(|power| power > 0))
                    .or_default()
                    .push(CoordinateCase::new(target.map(Some))?);
            }
            for (sector, cases) in sectors {
                if self.zero_sectors.contains(&sector) {
                    for case in cases {
                        let rule = zero_rule(&case, &sector, self.zero_conditions.clone());
                        result.derivation.push(DerivedRule {
                            rule: rule.clone(),
                            origin: RuleOrigin::ProvedZero,
                        });
                        result.rules.push(rule);
                    }
                    continue;
                }
                if !self.solvers.contains_key(&sector) {
                    let solver = SectorSolver::new(
                        self.sources,
                        sector,
                        SectorConfig {
                            zero_sectors: self.zero_sectors.clone(),
                            ..Default::default()
                        },
                    )?;
                    self.solvers.insert(sector, solver);
                }
                let solved = self.solvers[&sector].solve_numeric_cases(cases, search)?;
                result.stats.seeds += solved.stats.seeds;
                result.stats.rows += solved.stats.rows;
                result.stats.exact_trace_rows += solved.stats.exact_trace_rows;
                result.residuals.extend(solved.residuals.iter().map(|case| {
                    case.integral()
                        .powers()
                        .iter()
                        .map(|power| power.value())
                        .collect()
                }));
                for candidate in solved.rules {
                    let seeds = seed_integrals(&candidate);
                    let mut rule = export(candidate, self.sources, &sector)?;
                    extend_conditions(&mut rule.nonzero_conditions, &self.zero_conditions);
                    for term in &rule.rhs {
                        let target = std::array::from_fn(|axis| term.powers[axis].value);
                        if !visited.contains(&target) {
                            pending.insert(target);
                        }
                    }
                    result.derivation.push(DerivedRule {
                        rule: rule.clone(),
                        origin: RuleOrigin::Identities { seeds },
                    });
                    result.rules.push(rule);
                }
            }
        }
        result.stats.sectors = self.solvers.len();
        back_substitute(&mut result)?;
        Ok(result)
    }

    /// Deepen the search until [`STABLE_PATIENCE`] deeper searches reproduce
    /// the residual set, or `max_depth` is reached. Returns the deepest search.
    fn until_stable(
        &mut self,
        requested: &BTreeSet<[i16; N]>,
        options: DynamicSolveOptions,
    ) -> Result<DynamicSolution, SolverError> {
        let mut plateau = 0;
        let mut previous: Option<Vec<Vec<i16>>> = None;
        for depth in 0..=options.max_depth {
            let mut solution = self.run(requested, options, depth)?;
            if previous.as_ref() != Some(&solution.residuals) {
                plateau = depth;
                previous = Some(solution.residuals.clone());
            }
            if depth == options.max_depth || depth - plateau >= STABLE_PATIENCE {
                solution.stable_depth = (depth - plateau >= STABLE_PATIENCE).then_some(plateau);
                return Ok(solution);
            }
        }
        unreachable!("the depth loop returns at max_depth")
    }
}

fn extend_conditions(target: &mut Vec<CoefficientPolynomial>, source: &[CoefficientPolynomial]) {
    for condition in source {
        if !target.contains(condition) {
            target.push(condition.clone());
        }
    }
}

fn invalid_input(error: impl std::fmt::Display) -> SolverError {
    SolverError::InvalidInput(error.to_string())
}

/// The cut support as sector restrictions, without a sector pattern.
///
/// Like the solver's own cut preparation, cuts are not combined with power
/// shifts: a shifted uncut index could activate a sector outside the census
/// while lowering a cut index, and that integral would survive as a residual.
fn cut_restrictions(
    family: &IntegralFamily,
    cuts: &CutConstraint,
) -> Result<Restrictions, SolverError> {
    if cuts.arity() != family.denominator_count() {
        return Err(SolverError::InvalidInput(
            "cut constraint arity differs from the physical family".into(),
        ));
    }
    if cuts.required_active().active_count() > 0
        && family.power_shifts().iter().any(|shift| !shift.is_zero())
    {
        return Err(SolverError::InvalidInput(
            "reverse-unitarity cuts are not supported with power shifts".into(),
        ));
    }
    Restrictions::try_new(
        cuts.clone(),
        Pattern::any(cuts.arity()).map_err(invalid_input)?,
    )
    .map_err(invalid_input)
}

/// Whether `sector` misses a cut denominator, so all its integrals vanish.
fn is_excluded<const N: usize>(
    restrictions: &Restrictions,
    sector: [bool; N],
) -> Result<bool, SolverError> {
    let arity = restrictions.cuts().arity();
    if N < arity || sector[arity..].iter().any(|active| *active) {
        return Err(SolverError::InvalidInput("invalid capacity sector".into()));
    }
    let mask = Mask::try_new(sector[..arity].iter().copied()).map_err(invalid_input)?;
    Ok(restrictions
        .exclusion(&mask)
        .map_err(invalid_input)?
        .is_some())
}

/// Analyze the union of the requested sectors' subsectors. A zero entry either
/// comes from the existing exact U+F rank certificate, including missing-loop
/// integrals, or misses a cut denominator and so vanishes by the definition of
/// a cut integral. No heuristic deletion of numerator or pinched terms occurs.
fn zero_census<const N: usize>(
    family: &IntegralFamily,
    restrictions: &Restrictions,
    sectors: &[[bool; N]],
) -> Result<(Vec<[bool; N]>, Vec<CoefficientPolynomial>), SolverError> {
    let arity = family.denominator_count();
    let sector_count = census::sector_count_for(arity)?;
    let analyzer = Analyzer::try_new(family, restrictions.clone()).map_err(invalid_input)?;
    let mut zero = Vec::new();
    for bits in 0..sector_count {
        let sector = std::array::from_fn(|axis| axis < arity && bits & (1 << axis) != 0);
        if !sectors
            .iter()
            .any(|parent| sector.iter().zip(parent).all(|(a, b)| !a || *b))
        {
            continue;
        }
        let mask = Mask::try_new(sector[..arity].iter().copied()).map_err(invalid_input)?;
        if matches!(
            analyzer.analyze(&mask).map_err(invalid_input)?,
            Decision::ProvedZero(_) | Decision::Excluded(_)
        ) {
            zero.push(sector);
        }
    }
    let conditions = analyzer
        .domain()
        .conditions()
        .iter()
        .map(|condition| condition.polynomial().clone())
        .collect();
    Ok((zero, conditions))
}

fn back_substitute(result: &mut DynamicSolution) -> Result<(), SolverError> {
    let keys: BTreeMap<_, _> = result
        .rules
        .iter()
        .enumerate()
        .map(|(index, rule)| (rule.target.clone(), index))
        .collect();
    let mut done = BTreeSet::new();
    let mut visiting = BTreeSet::new();
    for index in 0..result.rules.len() {
        substitute(index, &keys, &mut result.rules, &mut done, &mut visiting)?;
    }
    let mut residuals: BTreeSet<_> = std::mem::take(&mut result.residuals).into_iter().collect();
    for term in result.rules.iter().flat_map(|rule| &rule.rhs) {
        residuals.insert(term.powers.iter().map(|power| power.value).collect());
    }
    result.residuals = residuals.into_iter().collect();
    Ok(())
}

fn substitute(
    index: usize,
    keys: &BTreeMap<Vec<DynamicPower>, usize>,
    rules: &mut [DynamicRule],
    done: &mut BTreeSet<usize>,
    visiting: &mut BTreeSet<usize>,
) -> Result<(), SolverError> {
    if done.contains(&index) {
        return Ok(());
    }
    if !visiting.insert(index) {
        return Err(SolverError::ExactReplay(
            "cyclic Laporta target rules".into(),
        ));
    }
    let mut terms = BTreeMap::<Vec<DynamicPower>, Coefficient>::new();
    for term in std::mem::take(&mut rules[index].rhs) {
        if let Some(&next) = keys.get(&term.powers) {
            substitute(next, keys, rules, done, visiting)?;
            for condition in rules[next].nonzero_conditions.clone() {
                if !rules[index].nonzero_conditions.contains(&condition) {
                    rules[index].nonzero_conditions.push(condition);
                }
            }
            for child in &rules[next].rhs {
                let coefficient = &term.coefficient * &child.coefficient;
                terms
                    .entry(child.powers.clone())
                    .and_modify(|old| *old = &*old + &coefficient)
                    .or_insert(coefficient);
            }
        } else {
            terms
                .entry(term.powers)
                .and_modify(|old| *old = &*old + &term.coefficient)
                .or_insert(term.coefficient);
        }
    }
    rules[index].rhs = terms
        .into_iter()
        .filter(|(_, coefficient)| !coefficient.is_zero())
        .map(|(powers, coefficient)| DynamicTerm {
            powers,
            coefficient,
        })
        .collect();
    visiting.remove(&index);
    done.insert(index);
    Ok(())
}
