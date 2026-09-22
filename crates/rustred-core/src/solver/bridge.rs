//! Runtime-arity access to the existing exact sector solver.
//!
//! This module only adapts ownership and integral keys. Source derivation,
//! parametric discovery, and finite Laporta elimination are the same native
//! algorithms used by [`super::SectorSolver`]. Finite-search residuals are a
//! basis for the returned equations, not a proof of master independence.

use std::collections::{BTreeMap, BTreeSet};

use symbolica::poly::PolyVariable;

use crate::algebra::{Coefficient, CoefficientPolynomial};
use crate::family::IntegralFamily;
use crate::sector::{
    Mask,
    zero::{Analyzer, Decision},
};

use super::{
    CoordinateCase, RuleCandidate, SearchOptions, SectorConfig, SectorSolver, SolverError,
    SourceSystem, extract_exceptions,
};

/// Search radius in the signed L1 seed lattice. Parameters remain exact.
#[derive(Clone, Copy, Debug)]
pub struct DynamicSolveOptions {
    pub max_depth: u32,
    pub include_lorentz: bool,
    /// Maximum distinct concrete integrals searched during RHS closure.
    /// Exceeding this budget is an error, never an incomplete reduction.
    pub max_targets: usize,
}

impl Default for DynamicSolveOptions {
    fn default() -> Self {
        Self {
            max_depth: 2,
            include_lorentz: false,
            max_targets: 1024,
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

#[derive(Clone, Debug, Default)]
pub struct DynamicSolution {
    pub rules: Vec<DynamicRule>,
    /// Unsolved requested integrals and terminal RHS integrals. These depend
    /// on the finite seed budget and are not certified master integrals.
    pub residuals: Vec<Vec<i16>>,
    pub index_variables: Vec<PolyVariable>,
    pub stats: DynamicSolveStats,
}

/// Find one reusable symbolic-index recurrence on an explicit sector/case.
/// Fixed coordinates use absolute powers; `None` coordinates remain symbolic.
/// Exhausting the finite search radius is an error, never a solved rule.
pub fn solve_parametric(
    family: &IntegralFamily,
    sector: &[bool],
    fixed: &[Option<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    dispatch!(
        family.denominator_count(),
        parametric,
        family,
        sector,
        fixed,
        options
    )
}

/// Reduce finite targets by the native shared-seed Laporta search and exact
/// replay. Every newly encountered RHS integral receives the same bounded
/// search, up to `max_targets`. Solved targets are back substituted across
/// sectors. Unresolved RHS integrals remain explicit.
pub fn solve_laporta(
    family: &IntegralFamily,
    targets: &[Vec<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    dispatch!(
        family.denominator_count(),
        laporta,
        family,
        targets,
        options
    )
}

// Keep monomorphization at this API boundary. No topology dispatch occurs.
macro_rules! dispatch {
    ($arity:expr, $function:ident, $($argument:expr),+) => {
        match $arity {
            1 => $function::<1>($($argument),+),
            2 => $function::<2>($($argument),+),
            3 => $function::<3>($($argument),+),
            4 => $function::<4>($($argument),+),
            5 => $function::<5>($($argument),+),
            6 => $function::<6>($($argument),+),
            7 => $function::<7>($($argument),+),
            8 => $function::<8>($($argument),+),
            9 => $function::<9>($($argument),+),
            10 => $function::<10>($($argument),+),
            11 => $function::<11>($($argument),+),
            12 => $function::<12>($($argument),+),
            arity => Err(SolverError::InvalidInput(format!(
                "the runtime bridge supports 1..=12 denominators; received {arity}"
            ))),
        }
    };
}
use dispatch;

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

fn export<const N: usize>(
    candidate: RuleCandidate<N>,
    sources: &SourceSystem<N>,
    sector: &[bool; N],
) -> Result<DynamicRule, SolverError> {
    let exceptions = extract_exceptions(&candidate, sources.index_variables(), sector)
        .map_err(|error| SolverError::ExactReplay(error.to_string()))?
        .branches;
    let powers = |integral: super::Integral<N>| {
        integral
            .powers()
            .iter()
            .map(|power| DynamicPower {
                symbolic: power.is_symbolic(),
                value: power.value(),
            })
            .collect()
    };
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

fn parametric<const N: usize>(
    family: &IntegralFamily,
    sector: &[bool],
    fixed: &[Option<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    let sector = array::<_, N>(sector, "sector")?;
    let case = CoordinateCase::new(array(fixed, "fixed indices")?)?;
    let sources = SourceSystem::<N>::from_family_with_lorentz(family, options.include_lorentz)?;
    let (zero_sectors, zero_conditions) = zero_census(family, &[sector])?;
    if !case.is_in_sector(&sector) {
        return Err(SolverError::InvalidInput(
            "case lies outside its sector".into(),
        ));
    }
    if zero_sectors.contains(&sector) {
        return Ok(DynamicSolution {
            rules: vec![DynamicRule {
                sector: sector.to_vec(),
                target: case
                    .integral()
                    .powers()
                    .iter()
                    .map(|power| DynamicPower {
                        symbolic: power.is_symbolic(),
                        value: power.value(),
                    })
                    .collect(),
                rhs: Vec::new(),
                nonzero_conditions: zero_conditions,
                exceptions: Vec::new(),
            }],
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
        residuals: Vec::new(),
        index_variables: index_variables(&sources),
        stats,
    })
}

fn laporta<const N: usize>(
    family: &IntegralFamily,
    targets: &[Vec<i16>],
    options: DynamicSolveOptions,
) -> Result<DynamicSolution, SolverError> {
    let mut pending = BTreeSet::<[i16; N]>::new();
    for target in targets {
        let powers = array::<_, N>(target, "target")?;
        CoordinateCase::new(powers.map(Some))?;
        pending.insert(powers);
    }
    let sources = SourceSystem::<N>::from_family_with_lorentz(family, options.include_lorentz)?;
    let initial_sectors: Vec<_> = pending
        .iter()
        .map(|powers| powers.map(|power| power > 0))
        .collect();
    let (zero_sectors, zero_conditions) = zero_census(family, &initial_sectors)?;
    let zero_sectors: std::sync::Arc<[[bool; N]]> = zero_sectors.into();
    let mut result = DynamicSolution {
        index_variables: index_variables(&sources),
        ..Default::default()
    };
    let mut visited = BTreeSet::new();
    let mut solvers = BTreeMap::new();
    while !pending.is_empty() {
        if visited.len().saturating_add(pending.len()) > options.max_targets {
            return Err(SolverError::InvalidInput(format!(
                "Laporta RHS closure exceeds max_targets={}; increase the explicit target budget",
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
            if zero_sectors.contains(&sector) {
                for case in cases {
                    result.rules.push(DynamicRule {
                        sector: sector.to_vec(),
                        target: case
                            .integral()
                            .powers()
                            .iter()
                            .map(|power| DynamicPower {
                                symbolic: false,
                                value: power.value(),
                            })
                            .collect(),
                        rhs: Vec::new(),
                        nonzero_conditions: zero_conditions.clone(),
                        exceptions: Vec::new(),
                    });
                }
                continue;
            }
            if !solvers.contains_key(&sector) {
                solvers.insert(
                    sector,
                    SectorSolver::new(
                        &sources,
                        sector,
                        SectorConfig {
                            zero_sectors: zero_sectors.clone(),
                            ..Default::default()
                        },
                    )?,
                );
            }
            let solver = &solvers[&sector];
            let solved = solver.solve_numeric_cases(cases, search(options))?;
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
                let mut rule = export(candidate, &sources, &sector)?;
                extend_conditions(&mut rule.nonzero_conditions, &zero_conditions);
                for term in &rule.rhs {
                    let target = std::array::from_fn(|axis| term.powers[axis].value);
                    if !visited.contains(&target) {
                        pending.insert(target);
                    }
                }
                result.rules.push(rule);
            }
        }
    }
    result.stats.sectors = solvers.len();
    back_substitute(&mut result)?;
    Ok(result)
}

fn extend_conditions(target: &mut Vec<CoefficientPolynomial>, source: &[CoefficientPolynomial]) {
    for condition in source {
        if !target.contains(condition) {
            target.push(condition.clone());
        }
    }
}

/// Analyze the union of the requested sectors' subsectors. Every zero entry
/// comes from the existing exact U+F rank certificate, including missing-loop
/// integrals; no heuristic deletion of numerator or pinched terms occurs.
fn zero_census<const N: usize>(
    family: &IntegralFamily,
    sectors: &[[bool; N]],
) -> Result<(Vec<[bool; N]>, Vec<CoefficientPolynomial>), SolverError> {
    let analyzer = Analyzer::try_unrestricted(family)
        .map_err(|error| SolverError::InvalidInput(error.to_string()))?;
    let mut zero = Vec::new();
    for bits in 0..(1usize << N) {
        let sector = std::array::from_fn(|axis| bits & (1 << axis) != 0);
        if !sectors
            .iter()
            .any(|parent| sector.iter().zip(parent).all(|(a, b)| !a || *b))
        {
            continue;
        }
        let mask =
            Mask::try_new(sector).map_err(|error| SolverError::InvalidInput(error.to_string()))?;
        if matches!(
            analyzer
                .analyze(&mask)
                .map_err(|error| SolverError::InvalidInput(error.to_string()))?,
            Decision::ProvedZero(_)
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
