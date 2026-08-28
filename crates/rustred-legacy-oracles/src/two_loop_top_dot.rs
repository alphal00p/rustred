//! Complete scalar-dot reduction for the equal-mass two-loop vacuum.
//!
//! For a symmetry-oriented positive target `I(a,b,c)` with `a > 1`, subtract
//! the native rows `d/dk1.k1` and `d/dk1.k2` at the seed `I(a-1,b,c)`:
//!
//! ```text
//! 0 = 3*(a-1)*m2 I(a,b,c)
//!   + (d-3*(a-1)) I(a-1,b,c)
//!   - 2*c I(a-2,b,c+1)
//!   + 2*c I(a-1,b-1,c+1)
//!   + (a-1) I(a,b,c-1)
//!   - (a-1) I(a,b-1,c).
//! ```
//!
//! Every all-positive output has one less total dot.  When a unit power is
//! pinched, the conventional dot count can stay unchanged, but the output has
//! only two active propagators and is therefore still strictly lower in the
//! sector-first Laporta order.  The exact two-line formula in [`crate::two_loop`]
//! closes those branches, including arbitrary inactive numerator powers.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;

use crate::families::equal_mass_two_loop_vacuum;
use crate::two_loop::{
    TwoLoopBoundaryConfig, TwoLoopBoundaryError, TwoLoopBoundaryReducer, pair_sector_work_estimate,
};
use crate::{
    Denominator, FamilyError, IbpGenerationError, IbpGenerator, Integral, LinearCombination,
    VacuumFamily,
};
use rustred::legacy_oracle_support::coefficient_degree::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
};
use rustred::{Coefficient, ExactRational, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT};

/// The native raw-row weights `E00-E01` at the oriented seed `a-e1`.
pub const TWO_LOOP_TOP_DOT_IBP_WEIGHTS: [[i8; 2]; 2] = [[1, -1], [0, 0]];

/// Exact number of derivative contributions constructed by `E00-E01` before
/// collection for an all-positive guarded seed.
pub const TWO_LOOP_TOP_DOT_RAW_TERM_BOUND: usize = 15;

/// Maximum number of distinct terms in the independently expanded equation,
/// including the pivot.  The solved right-hand side has at most five terms.
pub const TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND: usize = 6;

/// Conservative normal-form accumulation operations per possible positive
/// top-sector state: five branches, two masters, and one multiply plus one
/// possible addition per master.
pub const TWO_LOOP_TOP_DOT_ACCUMULATION_OPERATIONS_PER_STATE: usize = 20;

/// Formula, state, and coefficient budgets for all-dot reduction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwoLoopTopDotConfig {
    /// Maximum explicit terms inspected by one recurrence equation.
    pub max_explicit_terms: usize,
    /// Maximum native derivative contributions constructed for provenance.
    pub max_raw_terms: usize,
    /// Maximum memoized normal-form states.  A conservative whole-request
    /// upper bound is checked before coefficient construction, and the actual
    /// counter is checked again while reducing.
    pub max_states: usize,
    /// Maximum coefficient multiplications and additions while composing
    /// memoized normal forms.  Boundary-formula work has its own limit.
    pub max_coefficient_operations: usize,
    /// Maximum conservative exponent of any one Symbolica coefficient
    /// variable.  Symbolica's `u16` hard ceiling is enforced as well.
    pub max_coefficient_degree: u128,
    /// Maximum direct-formula iteration estimate for one two-line boundary.
    pub max_boundary_formula_iterations: usize,
}

impl Default for TwoLoopTopDotConfig {
    fn default() -> Self {
        Self {
            max_explicit_terms: TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND,
            max_raw_terms: TWO_LOOP_TOP_DOT_RAW_TERM_BOUND,
            max_states: 1_000_000,
            max_coefficient_operations: 20_000_000,
            max_coefficient_degree: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            max_boundary_formula_iterations: 1_000_000,
        }
    }
}

/// Fixed provenance of the two-loop recurrence orientation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwoLoopTopDotProvenance {
    seed_lowered_position: usize,
    raw_ibp_weights: [[i8; 2]; 2],
}

impl TwoLoopTopDotProvenance {
    pub const fn seed_lowered_position(self) -> usize {
        self.seed_lowered_position
    }

    pub const fn raw_ibp_weights(self) -> [[i8; 2]; 2] {
        self.raw_ibp_weights
    }
}

const TOP_DOT_PROVENANCE: TwoLoopTopDotProvenance = TwoLoopTopDotProvenance {
    seed_lowered_position: 0,
    raw_ibp_weights: TWO_LOOP_TOP_DOT_IBP_WEIGHTS,
};

/// One symmetry-oriented recurrence step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoLoopTopDotRewrite {
    target: Integral,
    seed: Integral,
    provenance: TwoLoopTopDotProvenance,
    rhs: LinearCombination,
}

impl TwoLoopTopDotRewrite {
    pub fn target(&self) -> &Integral {
        &self.target
    }

    pub fn seed(&self) -> &Integral {
        &self.seed
    }

    pub const fn provenance(&self) -> TwoLoopTopDotProvenance {
        self.provenance
    }

    pub fn rhs(&self) -> &LinearCombination {
        &self.rhs
    }

    pub fn into_rhs(self) -> LinearCombination {
        self.rhs
    }
}

/// Conservative request bounds computed before eager coefficient work.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TwoLoopTopDotPreflight {
    state_upper_bound: u128,
    coefficient_operation_upper_bound: u128,
    coefficient_degree_bound: u128,
    boundary_formula_iterations: u128,
}

impl TwoLoopTopDotPreflight {
    pub const fn state_upper_bound(self) -> u128 {
        self.state_upper_bound
    }

    pub const fn coefficient_operation_upper_bound(self) -> u128 {
        self.coefficient_operation_upper_bound
    }

    pub const fn coefficient_degree_bound(self) -> u128 {
        self.coefficient_degree_bound
    }

    pub const fn boundary_formula_iterations(self) -> u128 {
        self.boundary_formula_iterations
    }
}

/// Actual work performed by one eager normal-form request.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TwoLoopTopDotStats {
    states: usize,
    memo_hits: usize,
    recurrence_steps: usize,
    boundary_calls: usize,
    coefficient_operations: usize,
}

impl TwoLoopTopDotStats {
    pub const fn states(self) -> usize {
        self.states
    }

    pub const fn memo_hits(self) -> usize {
        self.memo_hits
    }

    pub const fn recurrence_steps(self) -> usize {
        self.recurrence_steps
    }

    pub const fn boundary_calls(self) -> usize {
        self.boundary_calls
    }

    pub const fn coefficient_operations(self) -> usize {
        self.coefficient_operations
    }
}

/// A completed normal form together with bounded-work accounting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TwoLoopTopDotNormalForm {
    result: LinearCombination,
    stats: TwoLoopTopDotStats,
}

impl TwoLoopTopDotNormalForm {
    pub fn result(&self) -> &LinearCombination {
        &self.result
    }

    pub const fn stats(&self) -> TwoLoopTopDotStats {
        self.stats
    }

    pub fn into_result(self) -> LinearCombination {
        self.result
    }
}

/// Complete eager scalar reducer for the built-in equal-mass sunset family.
#[derive(Clone, Debug)]
pub struct TwoLoopTopDotReducer {
    family: VacuumFamily,
    config: TwoLoopTopDotConfig,
    dimension: Coefficient,
    mass: Coefficient,
    sunset_master: Integral,
    product_master: Integral,
}

impl TwoLoopTopDotReducer {
    /// Construct the built-in positive-Euclidean equal-mass family.
    pub fn build(config: TwoLoopTopDotConfig) -> Result<Self, TwoLoopTopDotError> {
        Self::new(equal_mass_two_loop_vacuum()?, config)
    }

    /// Authenticate and take ownership of an exactly routed family.
    pub fn new(
        family: VacuumFamily,
        config: TwoLoopTopDotConfig,
    ) -> Result<Self, TwoLoopTopDotError> {
        let (dimension, mass) = validate_family(&family)?;
        Ok(Self {
            family,
            config,
            dimension,
            mass,
            sunset_master: Integral::from([1, 1, 1]),
            product_master: Integral::from([0, 1, 1]),
        })
    }

    pub fn family(&self) -> &VacuumFamily {
        &self.family
    }

    pub const fn config(&self) -> TwoLoopTopDotConfig {
        self.config
    }

    pub fn sunset_master(&self) -> &Integral {
        &self.sunset_master
    }

    pub fn product_master(&self) -> &Integral {
        &self.product_master
    }

    /// Apply one recurrence step to an all-positive scalar integral.
    ///
    /// `Ok(None)` denotes only the undotted sunset corner.  Boundary inputs,
    /// including inactive numerators, belong to [`Self::reduce_integral`].
    pub fn rewrite_once(
        &self,
        integral: &Integral,
    ) -> Result<Option<TwoLoopTopDotRewrite>, TwoLoopTopDotError> {
        let target = self.canonical_scalar_top(integral)?;
        if target == self.sunset_master {
            return Ok(None);
        }
        self.validate_pivot_guard(&target)?;
        self.preflight_explicit_equation()?;
        self.preflight_explicit_shifts(&target)?;
        self.ensure_coefficient_degree(1)?;

        let seed = checked_shift(&target, [-1, 0, 0])?;
        let mut equation = self.expected_equation_for_target(&target)?;
        let pivot =
            equation
                .remove(&target)
                .ok_or_else(|| TwoLoopTopDotError::MissingExpectedPivot {
                    target: target.clone(),
                })?;
        let expected_pivot = self.pivot_coefficient(&target);
        if pivot != expected_pivot {
            return Err(TwoLoopTopDotError::UnexpectedPivotCoefficient {
                target,
                expected: expected_pivot,
                actual: pivot,
            });
        }

        let mut rhs = LinearCombination::new();
        for (output, coefficient) in equation.terms() {
            let Some(canonical) = self.family.try_canonicalize(output)? else {
                return Err(TwoLoopTopDotError::UnexpectedZeroSector {
                    integral: output.clone(),
                });
            };
            rhs.add_term(canonical, -(coefficient / &pivot));
        }

        for output in rhs.terms().keys() {
            if output.numerator_degree() != 0 {
                return Err(TwoLoopTopDotError::UnexpectedNumerator {
                    target: target.clone(),
                    output: output.clone(),
                });
            }
            if compare_integrals_exact(output, &target) != Ordering::Less {
                return Err(TwoLoopTopDotError::NonDescendingTerm {
                    target: target.clone(),
                    output: output.clone(),
                });
            }
        }

        Ok(Some(TwoLoopTopDotRewrite {
            target,
            seed,
            provenance: TOP_DOT_PROVENANCE,
            rhs,
        }))
    }

    /// Generate the selected native equation `E00-E01` without symmetry
    /// canonicalization.
    pub fn raw_ibp(&self, integral: &Integral) -> Result<LinearCombination, TwoLoopTopDotError> {
        let target = self.guarded_target(integral)?;
        let seed = checked_shift(&target, [-1, 0, 0])?;
        self.preflight_raw_ibp(&seed)?;

        let generator = IbpGenerator::new(&self.family);
        let mut weighted = LinearCombination::new();
        for contraction_loop in 0..2 {
            let weight = TWO_LOOP_TOP_DOT_IBP_WEIGHTS[0][contraction_loop];
            if weight == 0 {
                continue;
            }
            let identity = generator.try_generate_raw_identity(&seed, 0, contraction_loop)?;
            weighted.add_scaled(
                &identity.equation,
                &self.family.coefficients().integer(i64::from(weight)),
            );
        }
        Ok(weighted)
    }

    /// Independently expand the equation predicted by the recurrence.
    pub fn expected_raw_ibp(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, TwoLoopTopDotError> {
        let target = self.guarded_target(integral)?;
        self.preflight_explicit_equation()?;
        self.preflight_explicit_shifts(&target)?;
        self.ensure_coefficient_degree(1)?;
        self.expected_equation_for_target(&target)
    }

    /// Authenticate the parametric formula against freshly generated native
    /// total derivatives at the requested asymmetric or symmetric point.
    pub fn validate_raw_ibp_provenance(
        &self,
        integral: &Integral,
    ) -> Result<(), TwoLoopTopDotError> {
        let actual = self.raw_ibp(integral)?;
        let expected = self.expected_raw_ibp(integral)?;
        if actual != expected {
            return Err(TwoLoopTopDotError::RawIbpProvenanceMismatch {
                target: self.guarded_target(integral)?,
                expected,
                actual,
            });
        }
        Ok(())
    }

    /// Compute and enforce conservative request bounds before normal-form
    /// coefficient construction.
    pub fn preflight(
        &self,
        integral: &Integral,
    ) -> Result<TwoLoopTopDotPreflight, TwoLoopTopDotError> {
        self.validate_arity(integral)?;
        let active = integral.denominator_count();
        if active <= 1 {
            return Ok(TwoLoopTopDotPreflight::default());
        }

        let preflight = if active == 2 {
            let (inactive, left, right) = boundary_powers(integral);
            let numerator_degree = u128::from(inactive.unsigned_abs());
            let active_dots = u128::try_from(i64::from(left) - 1)
                .expect("an active boundary power is positive")
                + u128::try_from(i64::from(right) - 1)
                    .expect("an active boundary power is positive");
            TwoLoopTopDotPreflight {
                state_upper_bound: 1,
                coefficient_operation_upper_bound: 0,
                coefficient_degree_bound: numerator_degree.saturating_add(active_dots),
                boundary_formula_iterations: pair_sector_work_estimate(inactive, left, right),
            }
        } else {
            let target = self.canonical_scalar_top(integral)?;
            if target == self.sunset_master {
                TwoLoopTopDotPreflight {
                    state_upper_bound: 1,
                    ..TwoLoopTopDotPreflight::default()
                }
            } else {
                self.preflight_explicit_equation()?;
                let dots = exact_dot_degree(&target);
                let positive_states = positive_state_upper_bound(dots);
                let boundary_states = boundary_state_upper_bound(dots);
                TwoLoopTopDotPreflight {
                    state_upper_bound: positive_states.saturating_add(boundary_states),
                    coefficient_operation_upper_bound: positive_states
                        .saturating_mul(TWO_LOOP_TOP_DOT_ACCUMULATION_OPERATIONS_PER_STATE as u128),
                    // Products reach degree D+1.  A conservative rational
                    // addition may cross-multiply two such denominators.
                    coefficient_degree_bound: u128::from(dots).saturating_add(1).saturating_mul(2),
                    // A generated scalar pinch has inactive power zero and
                    // at most D boundary dots, hence work 1+(D+2)=D+3.
                    boundary_formula_iterations: u128::from(dots).saturating_add(3),
                }
            }
        };
        self.enforce_preflight(preflight)?;
        Ok(preflight)
    }

    /// Eagerly reduce to `S=I(1,1,1)` and `P=I(0,1,1)`.
    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, TwoLoopTopDotError> {
        Ok(self.reduce_integral_with_stats(integral)?.into_result())
    }

    /// Eager normal form with actual memoization/resource statistics.
    pub fn reduce_integral_with_stats(
        &self,
        integral: &Integral,
    ) -> Result<TwoLoopTopDotNormalForm, TwoLoopTopDotError> {
        self.preflight(integral)?;
        if integral.denominator_count() <= 1 {
            return Ok(TwoLoopTopDotNormalForm {
                result: LinearCombination::new(),
                stats: TwoLoopTopDotStats::default(),
            });
        }

        let canonical = self.family.try_canonicalize(integral)?.ok_or_else(|| {
            TwoLoopTopDotError::UnexpectedZeroSector {
                integral: integral.clone(),
            }
        })?;
        let boundary = TwoLoopBoundaryReducer::new_with_config(
            &self.family,
            TwoLoopBoundaryConfig {
                max_formula_iterations: self.config.max_boundary_formula_iterations,
            },
        )?;
        let mut memo = HashMap::new();
        let mut stats = TwoLoopTopDotStats::default();
        let result =
            self.reduce_canonical_iterative(&canonical, &boundary, &mut memo, &mut stats)?;
        debug_assert!(
            result
                .terms()
                .keys()
                .all(|master| { master == &self.sunset_master || master == &self.product_master })
        );
        Ok(TwoLoopTopDotNormalForm { result, stats })
    }

    /// Evaluate the strictly descending dependency DAG with an explicit
    /// stack.  Reduction depth is therefore bounded by the state budget in
    /// heap memory and never consumes one Rust call-stack frame per dot.
    fn reduce_canonical_iterative(
        &self,
        requested: &Integral,
        boundary: &TwoLoopBoundaryReducer<'_>,
        memo: &mut HashMap<Integral, LinearCombination>,
        stats: &mut TwoLoopTopDotStats,
    ) -> Result<LinearCombination, TwoLoopTopDotError> {
        enum Frame {
            Visit(Integral),
            Combine {
                target: Integral,
                rhs: LinearCombination,
            },
        }

        let mut stack = vec![Frame::Visit(requested.clone())];
        while let Some(frame) = stack.pop() {
            match frame {
                Frame::Visit(integral) => {
                    if memo.contains_key(&integral) {
                        charge_counter(&mut stats.memo_hits, usize::MAX, "memo-hit counter")?;
                        continue;
                    }
                    charge_counter(&mut stats.states, self.config.max_states, "memoized states")?;

                    match integral.denominator_count() {
                        0 | 1 => {
                            memo.insert(integral, LinearCombination::new());
                        }
                        2 => {
                            charge_counter(
                                &mut stats.boundary_calls,
                                usize::MAX,
                                "boundary-call counter",
                            )?;
                            let (inactive, left, right) = boundary_powers(&integral);
                            self.ensure_coefficient_degree(boundary_coefficient_degree(
                                inactive, left, right,
                            ))?;
                            let result = boundary.reduce_integral(&integral)?;
                            for coefficient in result.terms().values() {
                                let degree = coefficient_variable_degrees(coefficient)
                                    .into_iter()
                                    .map(|(numerator, denominator)| numerator.max(denominator))
                                    .max()
                                    .unwrap_or(0);
                                self.ensure_coefficient_degree(degree)?;
                            }
                            memo.insert(integral, result);
                        }
                        3 if integral == self.sunset_master => {
                            memo.insert(
                                integral,
                                LinearCombination::from_term(
                                    self.sunset_master.clone(),
                                    self.family.coefficients().one(),
                                ),
                            );
                        }
                        3 => {
                            charge_counter(
                                &mut stats.recurrence_steps,
                                usize::MAX,
                                "recurrence-step counter",
                            )?;
                            let rewrite = self.rewrite_once(&integral)?.ok_or_else(|| {
                                TwoLoopTopDotError::MissingExpectedPivot {
                                    target: integral.clone(),
                                }
                            })?;
                            let rhs = rewrite.into_rhs();
                            stack.push(Frame::Combine {
                                target: integral,
                                rhs: rhs.clone(),
                            });
                            // Reverse BTree order so visits are deterministic
                            // in ascending integral order after the LIFO pop.
                            for dependency in rhs.terms().keys().rev() {
                                if !memo.contains_key(dependency) {
                                    stack.push(Frame::Visit(dependency.clone()));
                                }
                            }
                        }
                        _ => unreachable!(
                            "a three-denominator family has at most three active lines"
                        ),
                    }
                }
                Frame::Combine { target, rhs } => {
                    if memo.contains_key(&target) {
                        continue;
                    }
                    let mut normal = LinearCombination::new();
                    for (dependency, factor) in rhs.terms() {
                        let reduced = memo.get(dependency).ok_or_else(|| {
                            TwoLoopTopDotError::DependencyNotReduced {
                                target: target.clone(),
                                dependency: dependency.clone(),
                            }
                        })?;
                        self.add_scaled_checked(&mut normal, reduced, factor, stats)?;
                    }
                    memo.insert(target, normal);
                }
            }
        }

        memo.get(requested)
            .cloned()
            .ok_or_else(|| TwoLoopTopDotError::DependencyNotReduced {
                target: requested.clone(),
                dependency: requested.clone(),
            })
    }

    fn add_scaled_checked(
        &self,
        output: &mut LinearCombination,
        reduction: &LinearCombination,
        factor: &Coefficient,
        stats: &mut TwoLoopTopDotStats,
    ) -> Result<(), TwoLoopTopDotError> {
        if factor.is_zero() {
            return Ok(());
        }
        for (master, coefficient) in reduction.terms() {
            charge_counter(
                &mut stats.coefficient_operations,
                self.config.max_coefficient_operations,
                "normal-form coefficient operations",
            )?;
            self.ensure_coefficient_degree(coefficient_product_degree_bound(coefficient, factor))?;
            let scaled = coefficient * factor;
            if let Some(current) = output.coefficient(master) {
                charge_counter(
                    &mut stats.coefficient_operations,
                    self.config.max_coefficient_operations,
                    "normal-form coefficient operations",
                )?;
                self.ensure_coefficient_degree(coefficient_sum_degree_bound(current, &scaled))?;
            }
            output.add_term(master.clone(), scaled);
        }
        Ok(())
    }

    fn guarded_target(&self, integral: &Integral) -> Result<Integral, TwoLoopTopDotError> {
        let target = self.canonical_scalar_top(integral)?;
        self.validate_pivot_guard(&target)?;
        Ok(target)
    }

    fn canonical_scalar_top(&self, integral: &Integral) -> Result<Integral, TwoLoopTopDotError> {
        self.validate_arity(integral)?;
        if let Some((position, &power)) = integral
            .powers()
            .iter()
            .enumerate()
            .find(|(_, power)| **power <= 0)
        {
            return Err(TwoLoopTopDotError::OutsideScalarTopSector {
                integral: integral.clone(),
                position,
                power,
            });
        }
        self.family.try_canonicalize(integral)?.ok_or_else(|| {
            TwoLoopTopDotError::UnexpectedZeroSector {
                integral: integral.clone(),
            }
        })
    }

    fn validate_arity(&self, integral: &Integral) -> Result<(), TwoLoopTopDotError> {
        if integral.powers().len() != 3 {
            return Err(TwoLoopTopDotError::WrongIntegralArity {
                expected: 3,
                actual: integral.powers().len(),
            });
        }
        Ok(())
    }

    fn validate_pivot_guard(&self, target: &Integral) -> Result<(), TwoLoopTopDotError> {
        if target.powers()[0] <= 1 {
            return Err(TwoLoopTopDotError::PivotGuardNotSatisfied {
                integral: target.clone(),
                first_power: target.powers()[0],
            });
        }
        Ok(())
    }

    fn preflight_explicit_equation(&self) -> Result<(), TwoLoopTopDotError> {
        if self.config.max_explicit_terms < TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND {
            return Err(TwoLoopTopDotError::ResourceLimit {
                resource: "explicit recurrence terms",
                requested: TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND as u128,
                limit: self.config.max_explicit_terms as u128,
            });
        }
        Ok(())
    }

    fn preflight_explicit_shifts(&self, target: &Integral) -> Result<(), TwoLoopTopDotError> {
        for shift in [[-1, 0, 0], [-2, 0, 1], [-1, -1, 1], [0, 0, -1], [0, -1, 0]] {
            checked_shift(target, shift)?;
        }
        Ok(())
    }

    fn preflight_raw_ibp(&self, seed: &Integral) -> Result<(), TwoLoopTopDotError> {
        let mut raw_terms = 0_usize;
        for contraction_loop in 0..2 {
            if TWO_LOOP_TOP_DOT_IBP_WEIGHTS[0][contraction_loop] == 0 {
                continue;
            }
            raw_terms += usize::from(contraction_loop == 0);
            for (denominator, &power) in seed.powers().iter().enumerate() {
                if power == 0 {
                    continue;
                }
                let contraction =
                    self.family
                        .derivative_contraction(denominator, 0, contraction_loop);
                if !contraction.constant.is_zero() {
                    raw_terms += 1;
                    let mut shift = [0_i32; 3];
                    shift[denominator] = 1;
                    checked_shift(seed, shift)?;
                }
                for (cancelled, _) in contraction
                    .denominator_coefficients
                    .iter()
                    .enumerate()
                    .filter(|(_, coefficient)| !coefficient.is_zero())
                {
                    raw_terms += 1;
                    let mut shift = [0_i32; 3];
                    shift[denominator] += 1;
                    shift[cancelled] -= 1;
                    checked_shift(seed, shift)?;
                }
            }
        }
        debug_assert_eq!(raw_terms, TWO_LOOP_TOP_DOT_RAW_TERM_BOUND);
        if raw_terms > self.config.max_raw_terms {
            return Err(TwoLoopTopDotError::ResourceLimit {
                resource: "native raw derivative terms",
                requested: raw_terms as u128,
                limit: self.config.max_raw_terms as u128,
            });
        }
        self.ensure_coefficient_degree(1)
    }

    fn expected_equation_for_target(
        &self,
        target: &Integral,
    ) -> Result<LinearCombination, TwoLoopTopDotError> {
        let [a, _b, c] = <[i32; 3]>::try_from(target.powers())
            .expect("a guarded two-loop target has three powers");
        let a_minus_one = i64::from(a) - 1;
        let c = i64::from(c);
        let context = self.family.coefficients();
        let mut equation = LinearCombination::new();
        equation.add_term(target.clone(), self.pivot_coefficient(target));
        equation.add_term(
            checked_shift(target, [-1, 0, 0])?,
            &self.dimension - &context.integer(3 * a_minus_one),
        );
        equation.add_term(checked_shift(target, [-2, 0, 1])?, context.integer(-2 * c));
        equation.add_term(checked_shift(target, [-1, -1, 1])?, context.integer(2 * c));
        equation.add_term(
            checked_shift(target, [0, 0, -1])?,
            context.integer(a_minus_one),
        );
        equation.add_term(
            checked_shift(target, [0, -1, 0])?,
            context.integer(-a_minus_one),
        );
        Ok(equation)
    }

    fn pivot_coefficient(&self, target: &Integral) -> Coefficient {
        let multiplier = 3 * (i64::from(target.powers()[0]) - 1);
        &self.family.coefficients().integer(multiplier) * &self.mass
    }

    fn enforce_preflight(
        &self,
        preflight: TwoLoopTopDotPreflight,
    ) -> Result<(), TwoLoopTopDotError> {
        if preflight.state_upper_bound > self.config.max_states as u128 {
            return Err(TwoLoopTopDotError::ResourceLimit {
                resource: "normal-form state upper bound",
                requested: preflight.state_upper_bound,
                limit: self.config.max_states as u128,
            });
        }
        if preflight.coefficient_operation_upper_bound
            > self.config.max_coefficient_operations as u128
        {
            return Err(TwoLoopTopDotError::ResourceLimit {
                resource: "normal-form coefficient-operation upper bound",
                requested: preflight.coefficient_operation_upper_bound,
                limit: self.config.max_coefficient_operations as u128,
            });
        }
        if preflight.boundary_formula_iterations
            > self.config.max_boundary_formula_iterations as u128
        {
            return Err(TwoLoopTopDotError::ResourceLimit {
                resource: "boundary formula iteration estimate",
                requested: preflight.boundary_formula_iterations,
                limit: self.config.max_boundary_formula_iterations as u128,
            });
        }
        self.ensure_coefficient_degree(preflight.coefficient_degree_bound)
    }

    fn ensure_coefficient_degree(&self, requested: u128) -> Result<(), TwoLoopTopDotError> {
        let limit = self
            .config
            .max_coefficient_degree
            .min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT);
        if requested > limit {
            return Err(TwoLoopTopDotError::CoefficientDegreeLimit { requested, limit });
        }
        Ok(())
    }
}

fn validate_family(
    family: &VacuumFamily,
) -> Result<(Coefficient, Coefficient), TwoLoopTopDotError> {
    if family.loops() != 2 {
        return Err(TwoLoopTopDotError::WrongLoopCount {
            actual: family.loops(),
        });
    }
    if family.denominator_count() != 3 {
        return Err(TwoLoopTopDotError::WrongDenominatorCount {
            actual: family.denominator_count(),
        });
    }
    let mass = family
        .coefficients()
        .parameter("m2")
        .ok_or(TwoLoopTopDotError::MissingParameter { name: "m2" })?;
    let dimension = family
        .coefficients()
        .parameter("d")
        .ok_or(TwoLoopTopDotError::MissingParameter { name: "d" })?;
    if family.dimension() != &dimension {
        return Err(TwoLoopTopDotError::WrongMomentumRouting);
    }
    const ROUTINGS: [[i8; 2]; 3] = [[1, 0], [0, 1], [1, 1]];
    for (position, denominator) in family.denominators().iter().enumerate() {
        if denominator.normalization() != Some(1) {
            return Err(TwoLoopTopDotError::WrongPropagatorSign { position });
        }
        if denominator.shift() != &mass {
            return Err(TwoLoopTopDotError::UnequalMasses);
        }
        let expected = Denominator::propagator(
            ROUTINGS[position]
                .iter()
                .map(|&component| ExactRational::from(i64::from(component)))
                .collect(),
            mass.clone(),
        );
        if denominator.quadratic_form() != expected.quadratic_form() {
            return Err(TwoLoopTopDotError::WrongMomentumRouting);
        }
    }
    if family.symmetries().len() != 6 {
        return Err(TwoLoopTopDotError::IncompleteSymmetry {
            actual: family.symmetries().len(),
        });
    }
    Ok((dimension, mass))
}

fn boundary_powers(integral: &Integral) -> (i32, i32, i32) {
    let inactive = integral
        .powers()
        .iter()
        .copied()
        .find(|power| *power <= 0)
        .expect("a two-line boundary has one inactive power");
    let mut active = integral.powers().iter().copied().filter(|power| *power > 0);
    let left = active.next().expect("a two-line boundary has two powers");
    let right = active.next().expect("a two-line boundary has two powers");
    (inactive, left, right)
}

fn boundary_coefficient_degree(inactive: i32, left: i32, right: i32) -> u128 {
    u128::from(inactive.unsigned_abs())
        .saturating_add(u128::try_from(i64::from(left) - 1).expect("positive power"))
        .saturating_add(u128::try_from(i64::from(right) - 1).expect("positive power"))
}

fn exact_dot_degree(integral: &Integral) -> u64 {
    integral
        .powers()
        .iter()
        .map(|&power| u64::try_from(i64::from(power) - 1).expect("positive power"))
        .sum()
}

fn positive_state_upper_bound(dots: u64) -> u128 {
    // Positive triples are nonnegative dot triples with total at most D:
    // C(D+3,3).
    choose_three(u128::from(dots).saturating_add(3))
}

fn boundary_state_upper_bound(dots: u64) -> u128 {
    // Choose the inactive line and two nonnegative dot powers: 3*C(D+2,2).
    3_u128.saturating_mul(choose_two(u128::from(dots).saturating_add(2)))
}

fn choose_two(n: u128) -> u128 {
    if n < 2 {
        return 0;
    }
    let (left, right) = if n % 2 == 0 {
        (n / 2, n - 1)
    } else {
        (n, (n - 1) / 2)
    };
    left.saturating_mul(right)
}

fn choose_three(n: u128) -> u128 {
    if n < 3 {
        return 0;
    }
    let mut factors = [n, n - 1, n - 2];
    if let Some(value) = factors.iter_mut().find(|value| **value % 2 == 0) {
        *value /= 2;
    }
    if let Some(value) = factors.iter_mut().find(|value| **value % 3 == 0) {
        *value /= 3;
    }
    factors.into_iter().fold(1_u128, u128::saturating_mul)
}

fn compare_integrals_exact(left: &Integral, right: &Integral) -> Ordering {
    fn hardness(integral: &Integral) -> (usize, u64, u64, &[i32]) {
        let active = integral.denominator_count();
        let dots = integral
            .powers()
            .iter()
            .map(|&power| u64::from(power.saturating_sub(1).max(0) as u32))
            .sum::<u64>();
        let numerators = integral
            .powers()
            .iter()
            .map(|&power| {
                if power <= 0 {
                    u64::from(power.unsigned_abs())
                } else {
                    0
                }
            })
            .sum::<u64>();
        (
            active,
            dots.saturating_add(numerators),
            dots,
            integral.powers(),
        )
    }
    hardness(left).cmp(&hardness(right))
}

fn checked_shift(integral: &Integral, shift: [i32; 3]) -> Result<Integral, TwoLoopTopDotError> {
    let indexed = shift
        .into_iter()
        .enumerate()
        .filter(|(_, value)| *value != 0)
        .collect::<Vec<_>>();
    integral
        .checked_shifted(&indexed)
        .ok_or_else(|| TwoLoopTopDotError::ExponentOverflow {
            integral: integral.clone(),
            shift,
        })
}

fn charge_counter(
    counter: &mut usize,
    limit: usize,
    resource: &'static str,
) -> Result<(), TwoLoopTopDotError> {
    let requested = counter
        .checked_add(1)
        .ok_or(TwoLoopTopDotError::ResourceLimit {
            resource,
            requested: u128::MAX,
            limit: limit as u128,
        })?;
    if requested > limit {
        return Err(TwoLoopTopDotError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        });
    }
    *counter = requested;
    Ok(())
}

/// Typed topology, guard, overflow, resource, and provenance failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TwoLoopTopDotError {
    WrongLoopCount {
        actual: usize,
    },
    WrongDenominatorCount {
        actual: usize,
    },
    WrongMomentumRouting,
    WrongPropagatorSign {
        position: usize,
    },
    UnequalMasses,
    MissingParameter {
        name: &'static str,
    },
    IncompleteSymmetry {
        actual: usize,
    },
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    OutsideScalarTopSector {
        integral: Integral,
        position: usize,
        power: i32,
    },
    PivotGuardNotSatisfied {
        integral: Integral,
        first_power: i32,
    },
    MissingExpectedPivot {
        target: Integral,
    },
    UnexpectedPivotCoefficient {
        target: Integral,
        expected: Coefficient,
        actual: Coefficient,
    },
    ExponentOverflow {
        integral: Integral,
        shift: [i32; 3],
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    CoefficientDegreeLimit {
        requested: u128,
        limit: u128,
    },
    UnexpectedZeroSector {
        integral: Integral,
    },
    UnexpectedNumerator {
        target: Integral,
        output: Integral,
    },
    NonDescendingTerm {
        target: Integral,
        output: Integral,
    },
    DependencyNotReduced {
        target: Integral,
        dependency: Integral,
    },
    RawIbpProvenanceMismatch {
        target: Integral,
        expected: LinearCombination,
        actual: LinearCombination,
    },
    Boundary(TwoLoopBoundaryError),
    Ibp(IbpGenerationError),
    Family(FamilyError),
}

impl fmt::Display for TwoLoopTopDotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLoopCount { actual } => write!(
                formatter,
                "two-loop top-dot recurrence received {actual} loops"
            ),
            Self::WrongDenominatorCount { actual } => write!(
                formatter,
                "two-loop top-dot recurrence needs three denominators, received {actual}"
            ),
            Self::WrongMomentumRouting => formatter.write_str(
                "two-loop top-dot recurrence requires the built-in ordered sunset routing",
            ),
            Self::WrongPropagatorSign { position } => write!(
                formatter,
                "two-loop top-dot denominator {position} is not positive-Euclidean"
            ),
            Self::UnequalMasses => formatter
                .write_str("two-loop top-dot recurrence requires the common mass parameter m2"),
            Self::MissingParameter { name } => write!(
                formatter,
                "two-loop top-dot family does not define parameter {name}"
            ),
            Self::IncompleteSymmetry { actual } => write!(
                formatter,
                "two-loop top-dot recurrence needs all six S3 symmetries, found {actual}"
            ),
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "two-loop top-dot integral has {actual} powers, expected {expected}"
            ),
            Self::OutsideScalarTopSector {
                integral,
                position,
                power,
            } => write!(
                formatter,
                "{integral} is outside the all-positive two-loop top sector: power {position} is {power}"
            ),
            Self::PivotGuardNotSatisfied {
                integral,
                first_power,
            } => write!(
                formatter,
                "two-loop top-dot pivot guard a>1 is false for {integral} (a={first_power})"
            ),
            Self::MissingExpectedPivot { target } => {
                write!(formatter, "two-loop recurrence has no pivot for {target}")
            }
            Self::UnexpectedPivotCoefficient { target, .. } => write!(
                formatter,
                "two-loop recurrence has the wrong pivot coefficient for {target}"
            ),
            Self::ExponentOverflow { integral, shift } => write!(
                formatter,
                "two-loop top-dot shift {shift:?} is outside the i32 exponent range for {integral}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "two-loop top-dot {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::CoefficientDegreeLimit { requested, limit } => write!(
                formatter,
                "two-loop top-dot coefficient degree {requested} exceeds limit {limit}"
            ),
            Self::UnexpectedZeroSector { integral } => write!(
                formatter,
                "two-loop top-dot recurrence unexpectedly produced scaleless {integral}"
            ),
            Self::UnexpectedNumerator { target, output } => write!(
                formatter,
                "two-loop top-dot recurrence for {target} unexpectedly produced numerator {output}"
            ),
            Self::NonDescendingTerm { target, output } => write!(
                formatter,
                "two-loop top-dot recurrence for {target} contains non-descending term {output}"
            ),
            Self::DependencyNotReduced { target, dependency } => write!(
                formatter,
                "two-loop normal form for {target} reached unevaluated dependency {dependency}"
            ),
            Self::RawIbpProvenanceMismatch { target, .. } => write!(
                formatter,
                "explicit two-loop recurrence for {target} does not equal E00-E01"
            ),
            Self::Boundary(error) => write!(formatter, "two-loop boundary closure failed: {error}"),
            Self::Ibp(error) => write!(formatter, "cannot generate two-loop raw IBPs: {error}"),
            Self::Family(error) => write!(formatter, "two-loop top-dot family error: {error}"),
        }
    }
}

impl std::error::Error for TwoLoopTopDotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Boundary(error) => Some(error),
            Self::Ibp(error) => Some(error),
            Self::Family(error) => Some(error),
            _ => None,
        }
    }
}

impl From<TwoLoopBoundaryError> for TwoLoopTopDotError {
    fn from(value: TwoLoopBoundaryError) -> Self {
        Self::Boundary(value)
    }
}

impl From<IbpGenerationError> for TwoLoopTopDotError {
    fn from(value: IbpGenerationError) -> Self {
        Self::Ibp(value)
    }
}

impl From<FamilyError> for TwoLoopTopDotError {
    fn from(value: FamilyError) -> Self {
        Self::Family(value)
    }
}
