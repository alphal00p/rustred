//! Opt-in two-sided SpIRed eps axes and the injective order (owner: M2-T7b rr-twosided).
//!
//! An eps line carries the power `n + j eps` (power shift `j (4-d)/2`); every
//! integer `n` lies in one support. [`generate_eps_family`] solves every raw
//! chart (`n >= 1` or `n <= 0` per eps axis) of a family's owner support with
//! the opt-in two-sided sources and the default two-sided order program,
//! then enforces the hard false-knock-out guard of plan M2 sec.3.4. The tests
//! pin the 2L regression set (acceptance (a)), strict descent of every rule
//! step and closure of random points onto the residual keys (b), the
//! injective excess `e(n)` and the identity of generation and runtime order,
//! and the guard (c).
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Instant;

use rustred::algebra::{Coefficient, CoefficientContext, CoefficientPolynomial};
use rustred::family::{AffineDenominator, IntegralFamily};
use rustred::input::{Compiler, Limits, LoweringLimits};
use rustred::order::{CompiledOrder, OrderDescriptor, two_sided_excess};
use rustred::sector::{
    ComplexityKey, Mask, MasterCount, MasterCountOptions, MasterCounter, OrderingPolicy, zero,
};
use rustred::solver::{
    Integral, IntegralOrder, SectorConfig, SectorExecutor, SectorSolution, SectorSolveOptions,
    SourceSystem,
};
use symbolica::domains::integer::Integer;
use symbolica::domains::rational::Rational;

/// The eps regression families (rules_study/knockout/families plus the new
/// T005c and T007c), by name.
pub(crate) const EPS_FAMILIES: &[(&str, &str)] = &[
    ("T001c", include_str!("../support/eps_families/T001c.sym")),
    ("T003c", include_str!("../support/eps_families/T003c.sym")),
    ("T005c", include_str!("../support/eps_families/T005c.sym")),
    ("T007c", include_str!("../support/eps_families/T007c.sym")),
    ("T013c", include_str!("../support/eps_families/T013c.sym")),
    ("T070c", include_str!("../support/eps_families/T070c.sym")),
    ("T074c", include_str!("../support/eps_families/T074c.sym")),
    ("T075c", include_str!("../support/eps_families/T075c.sym")),
    ("T083c", include_str!("../support/eps_families/T083c.sym")),
    ("T090c", include_str!("../support/eps_families/T090c.sym")),
    ("T093c", include_str!("../support/eps_families/T093c.sym")),
    ("bubu", include_str!("../support/eps_families/bubu.sym")),
];

pub(crate) fn eps_family_source(name: &str) -> &'static str {
    EPS_FAMILIES
        .iter()
        .find(|(family, _)| *family == name)
        .map(|(_, source)| *source)
        .unwrap_or_else(|| panic!("unknown eps family {name}"))
}

/// Which raw charts to solve: all `2^j` charts of the owner support, or an
/// explicit subset (the guard then refuses the outcome unless it is all), or
/// every nonzero sector of the support's downset (a diagnostic: no guard,
/// exits of the owner become keys of the program).
pub(crate) enum EpsCharts<'a> {
    All,
    Only(&'a [Vec<bool>]),
    Downset,
}

/// One solved raw chart.
#[derive(Clone, Debug)]
pub(crate) struct EpsChart {
    pub chart: Vec<bool>,
    pub rules: usize,
    pub residuals: Vec<Vec<i64>>,
    pub seconds: f64,
}

/// A two-sided generation of one eps family over its owner support.
pub(crate) struct EpsOutcome {
    pub name: String,
    pub family: Arc<IntegralFamily>,
    /// Lines of the owner (target power > 0) and every forced (eps) axis.
    pub support: Vec<bool>,
    /// Forced axes: nonzero power shift.
    pub forced: Vec<bool>,
    /// Two-sided axes of the sources and of the order program.
    pub two_sided: Vec<bool>,
    /// The generation order, persisted as is: the runtime descent order.
    pub policy: OrderingPolicy,
    pub charts: Vec<EpsChart>,
    pub rules: usize,
    pub residual_keys: Vec<Vec<i64>>,
    /// Lee-Pomeransky master count of the owner support with shifts.
    pub lp: u32,
    pub seconds: f64,
    program: EpsProgram,
}

/// Generate the two-sided rules of an eps family (compact `I(...)` input)
/// and enforce the hard guard: two-sided axes cover every forced axis, all
/// `2^j` raw charts are present, and `LP >= 1` implies a residual.
pub(crate) fn generate_eps_family(sym: &str, charts: EpsCharts<'_>) -> Result<EpsOutcome, String> {
    generate_eps_family_with(sym, charts, OrderDescriptor::spired_uncut_two_sided)
}

/// [`generate_eps_family`] under another two-sided order program, built
/// from the family's two-sided flags.
pub(crate) fn generate_eps_family_with(
    sym: &str,
    charts: EpsCharts<'_>,
    order: fn(Vec<bool>) -> OrderDescriptor,
) -> Result<EpsOutcome, String> {
    let project = Compiler::new(Limits::default())
        .map_err(|e| e.to_string())?
        .compile_compact(sym, None)
        .map_err(|e| e.to_string())?;
    let name = project.name().to_owned();
    let target: Vec<i64> = project
        .propagators()
        .iter()
        .map(|p| p.target_power())
        .collect();
    let family = Arc::new(
        project
            .into_lowered(LoweringLimits::default())
            .map_err(|e| e.to_string())?
            .into_family(),
    );
    macro_rules! dispatch {
        ($($n:literal),*) => {
            match family.denominator_count() {
                $($n => generate::<$n>(name, family, &target, charts, order),)*
                other => Err(format!("eps family arity {other} is not instantiated in the tests")),
            }
        };
    }
    let downset = matches!(charts, EpsCharts::Downset);
    let outcome = dispatch!(5, 9, 14)?;
    if downset {
        return Ok(outcome);
    }
    check_eps_guard(
        &outcome.forced,
        &outcome.two_sided,
        &outcome.support,
        &outcome
            .charts
            .iter()
            .map(|c| c.chart.clone())
            .collect::<Vec<_>>(),
        outcome.lp,
        outcome.residual_keys.len(),
    )?;
    Ok(outcome)
}

/// The hard false-knock-out guard of plan M2 sec.3.4 on a generation outcome.
pub(crate) fn check_eps_guard(
    forced: &[bool],
    two_sided: &[bool],
    support: &[bool],
    charts: &[Vec<bool>],
    lp: u32,
    residuals: usize,
) -> Result<(), String> {
    if forced.iter().zip(two_sided).any(|(&f, &t)| f && !t) {
        return Err("hard guard: a forced (eps) axis is not two-sided".into());
    }
    let expected: BTreeSet<Vec<bool>> = all_charts(support, forced).into_iter().collect();
    let present: BTreeSet<Vec<bool>> = charts.iter().cloned().collect();
    if present != expected {
        return Err(format!(
            "hard guard: {} of the {} raw charts of the owner support are present",
            present.intersection(&expected).count(),
            expected.len()
        ));
    }
    if lp >= 1 && residuals == 0 {
        return Err(format!(
            "false knock-out guard: LP={lp}, residuals=0 (plan M2 sec.3.4)"
        ));
    }
    Ok(())
}

/// Every raw chart of `support`: its non-forced lines active, each forced
/// axis either `n >= 1` (active) or `n <= 0`.
pub(crate) fn all_charts(support: &[bool], forced: &[bool]) -> Vec<Vec<bool>> {
    let eps: Vec<usize> = (0..support.len()).filter(|&i| forced[i]).collect();
    (0..1u64 << eps.len())
        .map(|bits| {
            let mut chart = support.to_vec();
            for (bit, &axis) in eps.iter().enumerate() {
                chart[axis] = bits >> bit & 1 == 0;
            }
            chart
        })
        .collect()
}

/// Lee-Pomeransky master count of the owner support, with shifts (M2-T7a).
fn lp_with_shifts(family: &IntegralFamily, support: &[bool]) -> Result<u32, String> {
    let mut counter = MasterCounter::try_new_p_shifted(family, MasterCountOptions::default())
        .map_err(|e| e.to_string())?;
    let top = Mask::try_new(support.iter().copied()).map_err(|e| e.to_string())?;
    match counter.count(&top).map_err(|e| e.to_string())? {
        MasterCount::Zero => Ok(0),
        MasterCount::Counted(count) => u32::try_from(count).map_err(|e| e.to_string()),
        other => Err(format!("no LP verdict on the owner support: {other:?}")),
    }
}

fn generate<const N: usize>(
    name: String,
    family: Arc<IntegralFamily>,
    target: &[i64],
    charts: EpsCharts<'_>,
    order: fn(Vec<bool>) -> OrderDescriptor,
) -> Result<EpsOutcome, String> {
    let start = Instant::now();
    let n = family.denominator_count();
    let forced: Vec<bool> = family.power_shifts().iter().map(|s| !s.is_zero()).collect();
    let support: Vec<bool> = (0..n).map(|i| target[i] > 0 || forced[i]).collect();
    let mut sources =
        SourceSystem::<N>::from_family_with_capacity(&family, true).map_err(|e| e.to_string())?;
    let axes = sources
        .set_two_sided_eps_axes(&family)
        .map_err(|e| e.to_string())?;
    let two_sided = axes[..n].to_vec();
    if !two_sided.contains(&true) {
        return Err(format!("{name}: no two-sided eps axis"));
    }
    let program = CompiledOrder::compile(order(two_sided.clone()), Default::default())
        .map_err(|e| e.to_string())?;
    if program.two_sided() != two_sided.as_slice() {
        return Err(format!(
            "{name}: the order program does not carry the eps axes"
        ));
    }
    let policy = OrderingPolicy::try_programmed(program.clone()).map_err(|e| e.to_string())?;
    let zeros = zero_sectors::<N>(&family, &support)?;
    let config = SectorConfig {
        zero_sectors: Arc::from(zeros.clone()),
        integral_order: Some(program),
        ..Default::default()
    };
    let charts = match charts {
        EpsCharts::All => all_charts(&support, &forced),
        EpsCharts::Only(charts) => charts.to_vec(),
        EpsCharts::Downset => {
            let free: Vec<usize> = (0..n).filter(|&i| support[i]).collect();
            (0..1u64 << free.len())
                .map(|bits| {
                    let mut chart = vec![false; n];
                    for (bit, &axis) in free.iter().enumerate() {
                        chart[axis] = bits >> bit & 1 == 1;
                    }
                    chart
                })
                .filter(|chart| !zeros.contains(&std::array::from_fn(|i| i < n && chart[i])))
                .collect()
        }
    };
    let executor = SectorExecutor::new(1).map_err(|e| e.to_string())?;
    let mut solved = Vec::new();
    let mut program_rules = BTreeMap::new();
    for chart in charts {
        let sector: [bool; N] = std::array::from_fn(|i| i < n && chart[i]);
        if zeros.contains(&sector) {
            return Err(format!("{name}: chart {} is a zero sector", mask(&chart)));
        }
        let t0 = Instant::now();
        let mut solutions = executor
            .map(
                &sources,
                &[sector],
                &config,
                SectorSolveOptions::default(),
                |done| Ok::<_, String>(done.solution),
            )
            .map_err(|e| format!("{name} chart {}: {e}", mask(&chart)))?;
        let solution: SectorSolution<N> = solutions.pop().expect("one sector");
        let residuals: Vec<Vec<i64>> = solution
            .finite_residuals
            .iter()
            .map(|key| powers(key, n))
            .collect();
        solved.push(EpsChart {
            chart: chart.clone(),
            rules: solution.rules.len(),
            residuals,
            seconds: t0.elapsed().as_secs_f64(),
        });
        program_rules.insert(chart, dyn_rules(&sources, &solution, n));
    }
    let residual_keys: Vec<Vec<i64>> = solved
        .iter()
        .flat_map(|c| c.residuals.iter().cloned())
        .collect();
    let lp = lp_with_shifts(&family, &support)?;
    Ok(EpsOutcome {
        name,
        support,
        forced,
        two_sided,
        policy,
        rules: solved.iter().map(|c| c.rules).sum(),
        charts: solved,
        residual_keys,
        lp,
        seconds: start.elapsed().as_secs_f64(),
        program: EpsProgram {
            rules: program_rules,
            zeros: zeros.iter().map(|z| z[..n].to_vec()).collect(),
            d_position: d_position(&family, &sources)?,
        },
        family,
    })
}

/// Zero raw masks inside `support` (forced axes are always effective).
fn zero_sectors<const N: usize>(
    family: &IntegralFamily,
    support: &[bool],
) -> Result<Vec<[bool; N]>, String> {
    let analyzer = zero::Analyzer::try_unrestricted(family).map_err(|e| e.to_string())?;
    let n = family.denominator_count();
    let free: Vec<usize> = (0..n).filter(|&i| support[i]).collect();
    let mut zeros = Vec::new();
    for bits in 0u64..1 << free.len() {
        let mut sector = [false; N];
        for (bit, &axis) in free.iter().enumerate() {
            sector[axis] = bits >> bit & 1 == 1;
        }
        let raw = Mask::try_new(sector[..n].iter().copied()).map_err(|e| e.to_string())?;
        if let zero::Decision::ProvedZero(_) = analyzer.analyze(&raw).map_err(|e| e.to_string())? {
            zeros.push(sector);
        }
    }
    Ok(zeros)
}

fn powers<const N: usize>(key: &Integral<N>, n: usize) -> Vec<i64> {
    key.powers()[..n]
        .iter()
        .map(|p| i64::from(p.value()))
        .collect()
}

fn mask(bits: &[bool]) -> String {
    bits.iter().map(|&b| if b { '1' } else { '0' }).collect()
}

/// Position of the dimension `d` in the coefficient variable map.
fn d_position<const N: usize>(
    family: &IntegralFamily,
    sources: &SourceSystem<N>,
) -> Result<usize, String> {
    // Parameters precede the index variables in the sources' variable map
    // (`SourceSystem::from_family_with_capacity`), in the family's order.
    let d = family.dimension();
    let indices = sources.index_variables();
    if d.numerator.nterms() != 1 || !d.denominator.is_one() {
        return Err("the dimension is not a single parameter".into());
    }
    let exponents = d.numerator.exponents(0);
    (0..exponents.len())
        .find(|&position| exponents[position] == 1 && !indices.contains(&position))
        .filter(|&position| {
            sources.coefficient_variables().get(position) == d.numerator.variables().get(position)
        })
        .ok_or_else(|| "dimension variable not found".to_string())
}

// ----- a dynamic applier of generated sector rules (N-free) -----

/// One coordinate of a rule term: an offset of the symbolic power, or a
/// numeric power.
#[derive(Clone, Copy, Debug)]
enum Coordinate {
    Offset(i64),
    Numeric(i64),
}

struct DynRule {
    fixed: Vec<Option<i64>>,
    affine: Vec<CoefficientPolynomial>,
    exceptions: Vec<Vec<CoefficientPolynomial>>,
    rhs: Vec<(Vec<Coordinate>, Coefficient)>,
    indices: Vec<usize>,
}

struct EpsProgram {
    /// Rules by raw chart.
    rules: BTreeMap<Vec<bool>, Vec<DynRule>>,
    /// Zero raw sectors inside the support: their integrals vanish.
    zeros: BTreeSet<Vec<bool>>,
    d_position: usize,
}

fn dyn_rules<const N: usize>(
    sources: &SourceSystem<N>,
    solution: &SectorSolution<N>,
    n: usize,
) -> Vec<DynRule> {
    let indices = sources.index_variables()[..n].to_vec();
    let mut rules: Vec<DynRule> = solution
        .rules
        .iter()
        .map(|rule| DynRule {
            fixed: rule.candidate.case.fixed()[..n]
                .iter()
                .map(|v| v.map(i64::from))
                .collect(),
            affine: rule
                .candidate
                .case
                .affine()
                .map(|a| a.equations().to_vec())
                .unwrap_or_default(),
            exceptions: rule.exceptions.branches.clone(),
            rhs: rule
                .candidate
                .rhs
                .iter()
                .map(|term| {
                    let coordinates = term.integral.powers()[..n]
                        .iter()
                        .map(|p| {
                            if p.is_symbolic() {
                                Coordinate::Offset(i64::from(p.value()))
                            } else {
                                Coordinate::Numeric(i64::from(p.value()))
                            }
                        })
                        .collect();
                    (coordinates, term.coefficient.clone())
                })
                .collect(),
            indices: indices.clone(),
        })
        .collect();
    // Most specific first (more fixed coordinates, affine cases): every
    // applicable rule is an identity on its domain, this only fixes the pick.
    rules.sort_by_key(|rule| {
        std::cmp::Reverse(
            rule.fixed.iter().filter(|v| v.is_some()).count() + 16 * rule.affine.len(),
        )
    });
    rules
}

/// A polynomial in `d` alone (index variables substituted) at `d = value`.
fn evaluate(polynomial: &CoefficientPolynomial, d_position: usize, value: &Rational) -> Rational {
    let mut sum = Rational::zero();
    for term in 0..polynomial.nterms() {
        let exponent = polynomial.exponents(term)[d_position];
        let mut monomial = Rational::from(polynomial.coefficients[term].clone());
        for _ in 0..exponent {
            monomial = &monomial * value;
        }
        sum = &sum + &monomial;
    }
    sum
}

fn substitute(
    polynomial: &CoefficientPolynomial,
    indices: &[usize],
    key: &[i64],
) -> CoefficientPolynomial {
    let mut value = polynomial.clone();
    for (&position, &power) in indices.iter().zip(key) {
        value = value.replace(position, &Integer::from(power));
    }
    value
}

impl DynRule {
    fn applies(&self, key: &[i64], chart: &[bool]) -> bool {
        for (axis, fixed) in self.fixed.iter().enumerate() {
            match fixed {
                Some(value) if key[axis] != *value => return false,
                None if (key[axis] >= 1) != chart[axis] => return false,
                _ => {}
            }
        }
        if self
            .affine
            .iter()
            .any(|equation| !substitute(equation, &self.indices, key).is_zero())
        {
            return false;
        }
        !self.exceptions.iter().any(|branch| {
            branch
                .iter()
                .all(|equation| substitute(equation, &self.indices, key).is_zero())
        })
    }

    /// RHS keys whose coefficient (exact in `d`) is nonzero at `key`, with
    /// that coefficient's exact value at `d = d_value`.
    fn apply(
        &self,
        key: &[i64],
        d_position: usize,
        d_value: &Rational,
    ) -> Result<Vec<(Vec<i64>, Rational)>, String> {
        let mut out = Vec::new();
        for (coordinates, coefficient) in &self.rhs {
            let numerator = substitute(&coefficient.numerator, &self.indices, key);
            if numerator.is_zero() {
                continue;
            }
            let denominator = substitute(&coefficient.denominator, &self.indices, key);
            if denominator.is_zero() {
                return Err(format!("rule coefficient has a pole at {key:?}"));
            }
            let value = &evaluate(&numerator, d_position, d_value)
                / &evaluate(&denominator, d_position, d_value);
            let child = coordinates
                .iter()
                .zip(key)
                .map(|(c, &x)| match c {
                    Coordinate::Offset(offset) => x + offset,
                    Coordinate::Numeric(value) => *value,
                })
                .collect();
            out.push((child, value));
        }
        Ok(out)
    }
}

/// Reduction of one key at `d = 37/10`: residual keys with their
/// coefficients, the exits (a non-eps owner line at power <= 0: another
/// owner's sector), the number of rule steps and of strict-descent witnesses.
pub(crate) struct EpsReduction {
    pub residuals: BTreeMap<Vec<i64>, Rational>,
    pub exits: BTreeMap<Vec<i64>, Rational>,
    pub steps: usize,
    pub witnesses: usize,
}

impl EpsOutcome {
    /// Reduce `key` (owner support powers, other slots at their key values)
    /// with the generated rules, hardest key first, proving strict descent in
    /// the generation order for every RHS term.
    pub(crate) fn reduce(&self, key: &[i64]) -> Result<EpsReduction, String> {
        let residual_set: BTreeSet<&Vec<i64>> = self.residual_keys.iter().collect();
        let complexity = |k: &[i64]| -> Result<ComplexityKey, String> {
            self.policy.complexity_key(k).map_err(|e| e.to_string())
        };
        let d_value = Rational::from((Integer::from(37), Integer::from(10)));
        let mut frontier: BTreeMap<ComplexityKey, (Vec<i64>, Rational)> = BTreeMap::new();
        frontier.insert(complexity(key)?, (key.to_vec(), Rational::one()));
        let mut out = EpsReduction {
            residuals: BTreeMap::new(),
            exits: BTreeMap::new(),
            steps: 0,
            witnesses: 0,
        };
        while let Some((_, (key, coefficient))) = frontier.pop_last() {
            if coefficient.is_zero() {
                continue;
            }
            if residual_set.contains(&key) {
                out.residuals.insert(key, coefficient);
                continue;
            }
            // A chart without rules lies outside the generated program: an
            // exit (for an owner: a non-eps line at power <= 0).
            let chart: Vec<bool> = key.iter().map(|&x| x >= 1).collect();
            if self.program.zeros.contains(&chart) {
                continue;
            }
            let Some(rules) = self.program.rules.get(&chart) else {
                // Every chart of the owner support is solved: a key there
                // without rules would be a hole, never an exit.
                let owner = (0..key.len()).all(|i| {
                    (!self.support[i] || self.forced[i] || key[i] >= 1)
                        && (self.support[i] || key[i] <= 0)
                });
                if owner {
                    return Err(format!("{}: {key:?} is in no solved chart", self.name));
                }
                out.exits.insert(key, coefficient);
                continue;
            };
            let rule = rules
                .iter()
                .find(|rule| rule.applies(&key, &chart))
                .ok_or_else(|| format!("{}: no rule covers {key:?}", self.name))?;
            out.steps += 1;
            if out.steps > 200_000 {
                return Err(format!("{}: reduction did not terminate", self.name));
            }
            for (child, c) in rule.apply(&key, self.program.d_position, &d_value)? {
                let witness = self
                    .policy
                    .prove_strict_descent(&key, &child)
                    .map_err(|e| format!("{}: {key:?} -> {child:?}: {e}", self.name))?;
                if !witness.verify() {
                    return Err(format!("{}: witness {key:?} -> {child:?} fails", self.name));
                }
                out.witnesses += 1;
                let entry = frontier
                    .entry(complexity(&child)?)
                    .or_insert_with(|| (child.clone(), Rational::zero()));
                entry.1 = &entry.1 + &(&coefficient * &c);
            }
        }
        Ok(out)
    }
}

// ----- deterministic random points -----

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        self.0 >> 33
    }
    fn range(&mut self, low: i64, high: i64) -> i64 {
        low + (self.next() % (high - low + 1) as u64) as i64
    }
}

/// A random owner key: eps powers in `eps`, non-eps lines at 1 + dots with at
/// most `max_dots` dots in total, other slots at 0.
fn random_key(outcome: &EpsOutcome, rng: &mut Lcg, eps: (i64, i64), max_dots: i64) -> Vec<i64> {
    let mut key = vec![0; outcome.support.len()];
    let mut dots = rng.range(0, max_dots);
    let lines: Vec<usize> = (0..key.len())
        .filter(|&i| outcome.support[i] && !outcome.forced[i])
        .collect();
    for &i in &lines {
        key[i] = 1;
    }
    while dots > 0 {
        key[lines[rng.range(0, lines.len() as i64 - 1) as usize]] += 1;
        dots -= 1;
    }
    for (power, &forced) in key.iter_mut().zip(&outcome.forced) {
        if forced {
            *power = rng.range(eps.0, eps.1);
        }
    }
    key
}

fn generated(name: &str) -> EpsOutcome {
    generate_eps_family(eps_family_source(name), EpsCharts::All)
        .unwrap_or_else(|error| panic!("{name}: {error}"))
}

// ----- acceptance (a): the 2L regression set -----

#[test]
fn two_loop_eps_families_close_with_their_lp_residuals_at_n_equal_one() {
    // (family, residual keys) at the default two-sided order; targets.tsv
    // masters_lp_nosym: T001 1, T013 2, T007 1, T003 0, T005 0.
    let expected: [(&str, u32, &[[i64; 5]]); 5] = [
        ("T001c", 1, &[[1, 1, 1, 1, 1]]),
        ("T013c", 2, &[[1, 1, 1, 1, 1], [1, 2, 1, 1, 1]]),
        ("T007c", 1, &[[1, 1, 1, 1, 1]]),
        ("T003c", 0, &[]),
        ("T005c", 0, &[]),
    ];
    for (name, lp, keys) in expected {
        let outcome = generated(name);
        let found: BTreeSet<Vec<i64>> = outcome.residual_keys.iter().cloned().collect();
        let keys: BTreeSet<Vec<i64>> = keys.iter().map(|k| k.to_vec()).collect();
        assert_eq!(found, keys, "{name} residual keys");
        assert_eq!(outcome.lp, lp, "{name}: LP with shifts = targets.tsv");
        assert_eq!(
            outcome.residual_keys.len(),
            outcome.lp as usize,
            "{name}: residuals = LP"
        );
        // Residuals only at eps n = 1, and only in the top chart.
        for key in &outcome.residual_keys {
            for (axis, &forced) in outcome.forced.iter().enumerate() {
                assert!(
                    !forced || key[axis] == 1,
                    "{name}: residual {key:?} off n = 1"
                );
            }
        }
        // Every chart closed (a failed chart is an error of the generation).
        assert_eq!(
            outcome.charts.len(),
            1 << outcome.forced.iter().filter(|&&f| f).count(),
            "{name}"
        );
        assert_eq!(outcome.two_sided, outcome.forced, "{name}: eps axes");
    }
}

// ----- acceptance (b): descent and closure -----

#[test]
fn random_points_reduce_onto_the_residual_keys_with_strict_descent_witnesses() {
    for name in ["T001c", "T013c", "T007c", "T003c", "T005c"] {
        let outcome = generated(name);
        let mut rng = Lcg(0x7b5d_0001 ^ name.len() as u64);
        let (mut steps, mut witnesses) = (0, 0);
        for _ in 0..50 {
            let key = random_key(&outcome, &mut rng, (-3, 4), 3);
            let reduction = outcome
                .reduce(&key)
                .unwrap_or_else(|error| panic!("{name} {key:?}: {error}"));
            for residual in reduction.residuals.keys() {
                assert!(outcome.residual_keys.contains(residual), "{name}");
            }
            steps += reduction.steps;
            witnesses += reduction.witnesses;
        }
        assert!(steps > 0 && witnesses >= steps, "{name}: {steps} steps");
    }
}

#[test]
fn every_rule_term_descends_at_its_own_chart_points() {
    // Each rule applied at each covered point of a box in its chart: every
    // RHS term carries a strict-descent witness of the generation order.
    for name in ["T001c", "T013c", "T007c", "T003c", "T005c"] {
        let outcome = generated(name);
        let mut checked = 0usize;
        for (chart, rules) in &outcome.program.rules {
            for point in chart_box(chart, &outcome.forced, 3) {
                for rule in rules.iter().filter(|rule| rule.applies(&point, chart)) {
                    let d = Rational::from((Integer::from(37), Integer::from(10)));
                    for (child, _) in rule.apply(&point, outcome.program.d_position, &d).unwrap() {
                        let witness = outcome
                            .policy
                            .prove_strict_descent(&point, &child)
                            .unwrap_or_else(|e| panic!("{name} {point:?} -> {child:?}: {e}"));
                        assert!(witness.verify());
                        checked += 1;
                    }
                }
            }
        }
        assert!(checked > 0, "{name}");
    }
}

/// Points of `chart` with active non-eps powers 1..=1+r, eps powers in
/// -r..=0 or 1..=1+r by chart.
fn chart_box(chart: &[bool], forced: &[bool], r: i64) -> Vec<Vec<i64>> {
    let ranges: Vec<(i64, i64)> = chart
        .iter()
        .zip(forced)
        .map(|(&active, &eps)| match (active, eps) {
            (true, _) => (1, 1 + r),
            (false, true) => (-r, 0),
            (false, false) => (0, 0),
        })
        .collect();
    let mut points = vec![vec![]];
    for (low, high) in ranges {
        points = points
            .into_iter()
            .flat_map(|p: Vec<i64>| {
                (low..=high).map(move |v| {
                    let mut q = p.clone();
                    q.push(v);
                    q
                })
            })
            .collect();
    }
    points
}

#[test]
fn two_sided_excess_is_injective_with_its_minimum_at_one() {
    let values: Vec<u64> = (-6..=6).map(|n| two_sided_excess(n).unwrap()).collect();
    let distinct: BTreeSet<u64> = values.iter().copied().collect();
    assert_eq!(distinct.len(), values.len());
    assert_eq!(two_sided_excess(1), Some(0));
    assert_eq!(two_sided_excess(0), Some(1));
    assert_eq!(two_sided_excess(2), Some(2));
    assert_eq!(two_sided_excess(-1), Some(3));
    // Per axis of a two-sided program: the complexity key is injective in
    // the eps power with every other power fixed.
    let outcome = generated("T001c");
    for (axis, _) in outcome.forced.iter().enumerate().filter(|(_, f)| **f) {
        let keys: BTreeSet<ComplexityKey> = (-6..=6)
            .map(|n| {
                let mut key = vec![1; 5];
                key[axis] = n;
                outcome.policy.complexity_key(&key).unwrap()
            })
            .collect();
        assert_eq!(keys.len(), 13, "axis {axis}");
    }
}

#[test]
fn generation_order_is_the_runtime_descent_order() {
    // The solver's comparator (symbolic and numeric) and the persisted
    // policy's keys and concrete comparison agree on a box of two-sided keys.
    let outcome = generated("T013c");
    let n = 5;
    let box_points: Vec<[i16; 5]> = (0..5u32.pow(5))
        .map(|mut code| {
            std::array::from_fn(|_| {
                let v = (code % 5) as i16 - 1;
                code /= 5;
                v
            })
        })
        .collect();
    for chart_bits in 0..1u32 << n {
        let chart: [bool; 5] = std::array::from_fn(|i| chart_bits >> i & 1 == 1);
        let order = IntegralOrder::from_persisted_policy(chart, &outcome.policy).unwrap();
        assert_eq!(order.two_sided_axes().to_vec(), outcome.two_sided);
        for a in box_points.iter().step_by(37) {
            for b in box_points.iter().step_by(41) {
                let (la, lb) = (a.map(i64::from), b.map(i64::from));
                let expected = outcome.policy.compare(&la, &lb).unwrap();
                assert_eq!(
                    outcome
                        .policy
                        .complexity_key(&la)
                        .unwrap()
                        .cmp(&outcome.policy.complexity_key(&lb).unwrap()),
                    expected
                );
                assert_eq!(
                    order
                        .compare(
                            &Integral::numeric(*a).unwrap(),
                            &Integral::numeric(*b).unwrap()
                        )
                        .reverse(),
                    expected
                );
            }
        }
        // Symbolic offsets of the chart agree with translated concrete points
        // that stay in the chart (n -> 5 on n >= 1, n -> -5 on n <= 0).
        for a in box_points.iter().step_by(53) {
            for b in box_points.iter().step_by(59) {
                let translate = |shift: &[i16; 5]| -> Vec<i64> {
                    (0..n)
                        .map(|i| i64::from(shift[i]) + if chart[i] { 5 } else { -5 })
                        .collect()
                };
                assert_eq!(
                    order
                        .compare(
                            &Integral::symbolic(*a).unwrap(),
                            &Integral::symbolic(*b).unwrap()
                        )
                        .reverse(),
                    outcome
                        .policy
                        .compare(&translate(a), &translate(b))
                        .unwrap(),
                    "chart {chart:?} {a:?} {b:?}"
                );
            }
        }
    }
}

// ----- acceptance (c): the hard guard -----

#[test]
fn hard_guard_rejects_a_zero_residual_outcome_with_lp_one() {
    let forced = [true, false, false, false, false];
    let support = [true; 5];
    let charts = all_charts(&support, &forced);
    assert!(check_eps_guard(&forced, &forced, &support, &charts, 1, 1).is_ok());
    assert!(check_eps_guard(&forced, &forced, &support, &charts, 0, 0).is_ok());
    let error = check_eps_guard(&forced, &forced, &support, &charts, 1, 0).unwrap_err();
    assert!(error.contains("false knock-out guard"), "{error}");
    // Two-sided data must cover every forced axis, all 2^j charts present.
    assert!(check_eps_guard(&forced, &[false; 5], &support, &charts, 1, 1).is_err());
    assert!(check_eps_guard(&forced, &forced, &support, &charts[..1], 1, 1).is_err());
    // The one-sided S1 failure mode: solving only the top chart of T001c.
    let top = vec![vec![true; 5]];
    let error = generate_eps_family(eps_family_source("T001c"), EpsCharts::Only(&top))
        .err()
        .expect("a single chart cannot pass the guard");
    assert!(error.contains("raw charts"), "{error}");
}

#[test]
fn the_option_refuses_shifts_that_are_not_eps_lines() {
    // An integer power shift is not j(4-d)/2.
    let source = eps_family_source("T007c").replace("power_shift(D1,2-d/2)", "power_shift(D1,1)");
    let project = Compiler::new(Limits::default())
        .unwrap()
        .compile_compact(&source, None)
        .unwrap();
    let family = project
        .into_lowered(LoweringLimits::default())
        .unwrap()
        .into_family();
    let mut sources = SourceSystem::<5>::from_family_with_capacity(&family, true).unwrap();
    let error = sources.set_two_sided_eps_axes(&family).unwrap_err();
    assert!(error.to_string().contains("j(4-d)/2"), "{error}");
    assert_eq!(sources.two_sided_axes(), [false; 5]);
    // A half-integer number of eps units is refused too.
    let source =
        eps_family_source("T007c").replace("power_shift(D1,2-d/2)", "power_shift(D1,1-d/4)");
    let family = Compiler::new(Limits::default())
        .unwrap()
        .compile_compact(&source, None)
        .unwrap()
        .into_lowered(LoweringLimits::default())
        .unwrap()
        .into_family();
    let mut sources = SourceSystem::<5>::from_family_with_capacity(&family, true).unwrap();
    assert!(sources.set_two_sided_eps_axes(&family).is_err());
}

#[test]
fn the_option_refuses_a_shifted_family_without_one_external_momentum() {
    let base = CoefficientContext::try_new(["d"]).unwrap();
    let d = base.parameter("d").unwrap();
    let limits = Default::default();
    let unit = base
        .try_div(
            &base.try_sub(&base.integer(4), &d, limits).unwrap(),
            &base.integer(2),
            limits,
        )
        .unwrap();
    let family = IntegralFamily::new(
        "vacuum-eps",
        vec!["k".into()],
        vec![],
        base.clone(),
        d,
        vec![AffineDenominator::new(base.integer(-1), vec![base.one()])],
        vec![],
        vec![unit],
    )
    .unwrap();
    let mut sources = SourceSystem::<1>::from_family_with_capacity(&family, true).unwrap();
    let error = sources.set_two_sided_eps_axes(&family).unwrap_err();
    assert!(
        error.to_string().contains("one external momentum"),
        "{error}"
    );
}

#[test]
fn sources_and_order_program_must_carry_the_same_two_sided_axes() {
    let family = Arc::new(
        Compiler::new(Limits::default())
            .unwrap()
            .compile_compact(eps_family_source("T007c"), None)
            .unwrap()
            .into_lowered(LoweringLimits::default())
            .unwrap()
            .into_family(),
    );
    let two_sided_program = CompiledOrder::compile(
        OrderDescriptor::spired_uncut_two_sided(vec![true, false, false, false, false]),
        Default::default(),
    )
    .unwrap();
    let mut enabled = SourceSystem::<5>::from_family_with_capacity(&family, true).unwrap();
    enabled.set_two_sided_eps_axes(&family).unwrap();
    let disabled = SourceSystem::<5>::from_family_with_capacity(&family, true).unwrap();
    assert_eq!(disabled.two_sided_axes(), [false; 5]);
    let executor = SectorExecutor::new(1).unwrap();
    let chart = [true; 5];
    let solve = |sources: &SourceSystem<5>, program: Option<CompiledOrder>| {
        executor
            .map(
                sources,
                &[chart],
                &SectorConfig {
                    integral_order: program,
                    ..Default::default()
                },
                SectorSolveOptions::default(),
                |done| Ok::<_, String>(done.solution.rules.len()),
            )
            .map(|rules| rules[0])
            .map_err(|error| error.to_string())
    };
    // The option without the program, and the program without the option.
    let error = solve(&enabled, None).unwrap_err();
    assert!(error.contains("two-sided eps axes"), "{error}");
    let error = solve(&disabled, Some(two_sided_program.clone())).unwrap_err();
    assert!(error.contains("two-sided eps axes"), "{error}");
    assert!(solve(&enabled, Some(two_sided_program)).unwrap() > 0);
}

// ----- value check: two independently ordered generations agree -----

/// The two-sided SpIRed order led by the eps excess: another rule set.
fn eps_first_two_sided(two_sided: Vec<bool>) -> OrderDescriptor {
    let mut descriptor = OrderDescriptor::spired_uncut(two_sided.len());
    descriptor.degree_rows.insert(
        0,
        rustred::order::DegreeRow {
            active: two_sided.iter().map(|&flag| u64::from(flag)).collect(),
            inactive: vec![0; two_sided.len()],
        },
    );
    descriptor.two_sided = two_sided;
    descriptor
}

#[test]
fn two_independently_ordered_downsets_reduce_to_the_same_values() {
    // The default and the eps-first orders derive different rules (different
    // chart crossings and branches). Over the full downset of the owner
    // support both reduce every key onto residual keys; where the residual
    // sets coincide the coefficients must agree exactly (at d = 37/10).
    let mut compared = Vec::new();
    for name in ["T001c", "T007c", "T003c", "T005c", "T013c"] {
        let first = generate_eps_family(eps_family_source(name), EpsCharts::Downset)
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        let other = generate_eps_family_with(
            eps_family_source(name),
            EpsCharts::Downset,
            eps_first_two_sided,
        )
        .unwrap_or_else(|error| panic!("{name}: {error}"));
        let (a, b): (BTreeSet<_>, BTreeSet<_>) = (
            first.residual_keys.iter().collect(),
            other.residual_keys.iter().collect(),
        );
        if a != b {
            continue;
        }
        let mut rng = Lcg(0x7b5d_0002 ^ name.len() as u64);
        for _ in 0..12 {
            let key = random_key(&first, &mut rng, (-2, 3), 2);
            let x = first.reduce(&key).unwrap();
            let y = other.reduce(&key).unwrap();
            assert!(
                x.exits.is_empty() && y.exits.is_empty(),
                "{name} {key:?}: exits {:?} {:?}",
                x.exits.keys().collect::<Vec<_>>(),
                y.exits.keys().collect::<Vec<_>>()
            );
            assert_eq!(x.residuals, y.residuals, "{name} {key:?}");
        }
        compared.push(name);
    }
    assert!(
        compared.len() >= 4,
        "residual sets coincide only in {compared:?}"
    );
}
