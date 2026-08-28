//! Exact boundary-sector reduction for the equal-mass two-loop vacuum family.
//!
//! This module implements the factorized tadpole and angular-average formula
//! from section 8.5 of `docs/research/litered2_algorithm_report.md`.  It does
//! not use IBP elimination (or FORM): every integral with at most two positive
//! powers is reduced directly.  The two-line sectors reduce to
//!
//! ```text
//! P = I(0, 1, 1),
//! ```
//!
//! while sectors with fewer than two positive powers are scaleless.

use std::fmt;

use rustred::family::PropagatorSign;
use rustred::legacy_oracle_support::coefficient_degree::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use rustred::{
    Coefficient, Integral, LinearCombination, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT, VacuumFamily,
};

/// Resource bound for one direct analytic two-loop boundary reduction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TwoLoopBoundaryConfig {
    /// Maximum conservative iteration estimate for the closed pair-sector
    /// formula.  Its dominant term is cubic in the inactive numerator width.
    pub max_formula_iterations: usize,
}

impl Default for TwoLoopBoundaryConfig {
    fn default() -> Self {
        Self {
            max_formula_iterations: 1_000_000,
        }
    }
}

/// Exact reducer for boundary sectors of the built-in equal-mass two-loop
/// vacuum family.
///
/// The supported denominators are, in any order,
///
/// ```text
/// sigma (k1^2 + m2), sigma (k2^2 + m2),
/// sigma ((k1 + k2)^2 + m2),
/// ```
///
/// where the common `sigma` is either `+1` or `-1`.
///
/// The reducer borrows the family so that its Symbolica coefficient context is
/// also used for every result.  Input permutations are handled analytically;
/// the output master is deliberately the stable convention `I(0,1,1)`, rather
/// than the lexicographic representative selected by `VacuumFamily`.
#[derive(Clone, Debug)]
pub struct TwoLoopBoundaryReducer<'family> {
    family: &'family VacuumFamily,
    config: TwoLoopBoundaryConfig,
    /// The `s` of `D = k^2 - s`.  The built-in Euclidean denominator is
    /// `D = k^2 + m2`, hence `s = -m2`.
    signed_mass_squared: Coefficient,
    propagator_sign: PropagatorSign,
    master: Integral,
}

impl<'family> TwoLoopBoundaryReducer<'family> {
    /// Validate and construct a reducer with safe default work bounds.
    pub fn new(family: &'family VacuumFamily) -> Result<Self, TwoLoopBoundaryError> {
        Self::new_with_config(family, TwoLoopBoundaryConfig::default())
    }

    /// Validate and construct a reducer for the built-in equal-mass topology
    /// with an explicit direct-formula work bound.
    pub fn new_with_config(
        family: &'family VacuumFamily,
        config: TwoLoopBoundaryConfig,
    ) -> Result<Self, TwoLoopBoundaryError> {
        if family.loops() != 2 {
            return Err(TwoLoopBoundaryError::WrongLoopCount {
                actual: family.loops(),
            });
        }
        if family.denominator_count() != 3 {
            return Err(TwoLoopBoundaryError::WrongDenominatorCount {
                actual: family.denominator_count(),
            });
        }
        if family
            .denominators()
            .iter()
            .any(|denominator| !denominator.is_propagator())
        {
            return Err(TwoLoopBoundaryError::WrongMomentumRouting);
        }

        let propagator_sign = family.denominators()[0]
            .propagator_sign()
            .expect("physical propagators were checked above");
        if family
            .denominators()
            .iter()
            .skip(1)
            .any(|denominator| denominator.propagator_sign() != Some(propagator_sign))
        {
            return Err(TwoLoopBoundaryError::MixedPropagatorSigns);
        }
        let normalization: rustred::ExactRational =
            i64::from(propagator_sign.normalization()).into();
        let normalized_shift = family
            .coefficients()
            .scale_rational(family.denominators()[0].shift(), &normalization);
        if normalized_shift.is_zero() {
            return Err(TwoLoopBoundaryError::MasslessFamily);
        }
        if family
            .denominators()
            .iter()
            .skip(1)
            .map(|denominator| {
                family
                    .coefficients()
                    .scale_rational(denominator.shift(), &normalization)
            })
            .any(|shift| shift != normalized_shift)
        {
            return Err(TwoLoopBoundaryError::UnequalMasses);
        }

        // The analytic numerator formula uses the three edges of the sunset
        // graph.  Accept an arbitrary ordering of those edges, but reject a
        // merely rank-three family for which the formula would not apply.
        let mut actual_basis: Vec<_> = family
            .denominators()
            .iter()
            .map(|denominator| {
                denominator
                    .quadratic_form()
                    .iter()
                    .map(|coefficient| coefficient * &normalization)
                    .collect::<Vec<_>>()
            })
            .collect();
        actual_basis.sort();
        let two = rustred::ExactRational::from(2);
        let mut expected_basis = vec![
            vec![
                rustred::ExactRational::one(),
                rustred::ExactRational::zero(),
                rustred::ExactRational::zero(),
            ],
            vec![
                rustred::ExactRational::zero(),
                rustred::ExactRational::zero(),
                rustred::ExactRational::one(),
            ],
            vec![
                rustred::ExactRational::one(),
                two,
                rustred::ExactRational::one(),
            ],
        ];
        expected_basis.sort();
        if actual_basis != expected_basis {
            return Err(TwoLoopBoundaryError::WrongMomentumRouting);
        }

        // Six elements are the complete permutation group on three objects.
        // Requiring it prevents silently claiming permutation canonicalization
        // for a family configured with only a subgroup.
        if family.symmetries().len() != 6 {
            return Err(TwoLoopBoundaryError::IncompleteSymmetry {
                actual: family.symmetries().len(),
            });
        }

        Ok(Self {
            family,
            config,
            signed_mass_squared: -normalized_shift,
            propagator_sign,
            master: Integral::from([0, 1, 1]),
        })
    }

    pub fn family(&self) -> &'family VacuumFamily {
        self.family
    }

    pub fn config(&self) -> TwoLoopBoundaryConfig {
        self.config
    }

    /// The fixed pair-sector master `P = I(0,1,1)` used in returned results.
    pub fn master(&self) -> &Integral {
        &self.master
    }

    /// The mass variable in the convention `D = k^2 - s`.
    ///
    /// For the built-in Euclidean family this is `-m2`.
    pub fn signed_mass_squared(&self) -> &Coefficient {
        &self.signed_mass_squared
    }

    /// Common overall sign multiplying all three physical denominators.
    pub fn propagator_sign(&self) -> PropagatorSign {
        self.propagator_sign
    }

    /// Reduce a boundary integral, or return `Ok(None)` for a top-sector
    /// integral which this specialized reducer intentionally does not handle.
    ///
    /// Every numerator power on the inactive line whose closed formula fits
    /// both the configured work bound and Symbolica's coefficient exponent
    /// representation is supported.  A pair sector returns one term
    /// proportional to [`Self::master`]; an empty or single-line sector
    /// returns `Some(0)` without consuming the formula budget.
    pub fn try_reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<Option<LinearCombination>, TwoLoopBoundaryError> {
        if integral.powers().len() != 3 {
            return Err(TwoLoopBoundaryError::WrongIntegralArity {
                actual: integral.powers().len(),
            });
        }

        let active: Vec<_> = integral
            .powers()
            .iter()
            .copied()
            .filter(|power| *power > 0)
            .collect();
        match active.len() {
            0 | 1 => Ok(Some(LinearCombination::new())),
            2 => {
                let inactive_power = integral
                    .powers()
                    .iter()
                    .copied()
                    .find(|power| *power <= 0)
                    .expect("a pair sector has one inactive denominator");
                self.check_pair_coefficient_degree(inactive_power, active[0], active[1])?;
                self.check_pair_work(inactive_power, active[0], active[1])?;
                let mut coefficient =
                    self.pair_sector_coefficient(inactive_power, active[0], active[1]);
                // I_sigma(a) = sigma^(-sum(a)) I_+(a), while the pair
                // master has total exponent two and is therefore unchanged.
                // For sigma=-1 only parity matters, including negative
                // numerator powers on the inactive line.
                if self.propagator_sign == PropagatorSign::Negative
                    && integral
                        .powers()
                        .iter()
                        .filter(|power| power.rem_euclid(2) != 0)
                        .count()
                        % 2
                        != 0
                {
                    coefficient = -coefficient;
                }
                Ok(Some(LinearCombination::from_term(
                    self.master.clone(),
                    coefficient,
                )))
            }
            3 => Ok(None),
            _ => unreachable!("a three-denominator family has at most three active lines"),
        }
    }

    /// Reduce an integral known to be in a boundary sector.
    ///
    /// Use [`Self::try_reduce_integral`] when top-sector inputs are an expected
    /// part of a larger reduction pipeline.
    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, TwoLoopBoundaryError> {
        self.try_reduce_integral(integral)?
            .ok_or_else(|| TwoLoopBoundaryError::TopSector(integral.clone()))
    }

    /// Reduce all boundary terms of a linear combination and leave top-sector
    /// terms unchanged.
    ///
    /// This is the convenient composition point for a general IBP table: its
    /// top-sector output can be passed here to close every factorized or
    /// scaleless boundary without changing the sunset-master terms.
    pub fn reduce_combination(
        &self,
        combination: &LinearCombination,
    ) -> Result<LinearCombination, TwoLoopBoundaryError> {
        let mut output = LinearCombination::new();
        for (integral, coefficient) in combination.terms() {
            if let Some(reduction) = self.try_reduce_integral(integral)? {
                self.add_scaled_checked(&mut output, &reduction, coefficient)?;
            } else {
                output.add_term(integral.clone(), coefficient.clone());
            }
        }
        Ok(output)
    }

    /// Preflight all Symbolica exponents used by the closed pair-sector
    /// formula.  Write `s=N_s/D_s`, `d=N_d/D_d`, `r=-a`,
    /// `q=(b-1)+(c-1)`, and `t=floor(r/2)`.  After fully expanding the finite
    /// sums, every summand has a denominator dividing
    ///
    /// `D_s^r (D_d N_s)^q H_t(d)`.
    ///
    /// The second bound below is the degree after lifting every numerator to
    /// that common denominator.  It also dominates the mass-power, tadpole,
    /// and angular-cache intermediates constructed before the final sum.
    fn check_pair_coefficient_degree(
        &self,
        inactive_power: i32,
        left_power: i32,
        right_power: i32,
    ) -> Result<(), TwoLoopBoundaryError> {
        let numerator_degree = u128::from(inactive_power.unsigned_abs());
        let tadpole_steps = u128::try_from(i64::from(left_power) - 1)
            .expect("an active power is positive")
            .saturating_add(
                u128::try_from(i64::from(right_power) - 1).expect("an active power is positive"),
            );
        let angular_rank = numerator_degree / 2;
        let requested = coefficient_variable_degrees(&self.signed_mass_squared)
            .into_iter()
            .zip(coefficient_variable_degrees(self.family.dimension()))
            .map(
                |(
                    (mass_numerator, mass_denominator),
                    (dimension_numerator, dimension_denominator),
                )| {
                    let shifted_dimension = dimension_numerator.max(dimension_denominator);
                    let common_denominator =
                        numerator_degree
                            .saturating_mul(mass_denominator)
                            .saturating_add(tadpole_steps.saturating_mul(
                                dimension_denominator.saturating_add(mass_numerator),
                            ))
                            .saturating_add(angular_rank.saturating_mul(shifted_dimension));
                    let lifted_numerator = numerator_degree
                        .saturating_mul(mass_numerator.saturating_add(mass_denominator))
                        .saturating_add(
                            tadpole_steps.saturating_mul(
                                shifted_dimension
                                    .saturating_add(mass_denominator)
                                    .saturating_add(dimension_denominator)
                                    .saturating_add(mass_numerator),
                            ),
                        )
                        .saturating_add(angular_rank.saturating_mul(
                            dimension_denominator.saturating_add(shifted_dimension),
                        ));
                    common_denominator.max(lifted_numerator)
                },
            )
            .max()
            .unwrap_or(0);
        self.check_coefficient_degree(requested)?;
        Ok(())
    }

    fn check_coefficient_degree(&self, requested: u128) -> Result<(), TwoLoopBoundaryError> {
        if !symbolica_coefficient_degree_is_representable(requested) {
            return Err(TwoLoopBoundaryError::CoefficientExponentLimit {
                requested,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            });
        }
        Ok(())
    }

    fn check_pair_work(
        &self,
        inactive_power: i32,
        left_power: i32,
        right_power: i32,
    ) -> Result<(), TwoLoopBoundaryError> {
        let requested = pair_sector_work_estimate(inactive_power, left_power, right_power);
        if requested > self.config.max_formula_iterations as u128 {
            return Err(TwoLoopBoundaryError::ResourceLimit {
                resource: "boundary formula iteration estimate",
                requested,
                limit: self.config.max_formula_iterations as u128,
            });
        }
        Ok(())
    }

    fn add_scaled_checked(
        &self,
        output: &mut LinearCombination,
        reduction: &LinearCombination,
        factor: &Coefficient,
    ) -> Result<(), TwoLoopBoundaryError> {
        if factor.is_zero() {
            return Ok(());
        }
        for (integral, coefficient) in reduction.terms() {
            self.check_coefficient_degree(coefficient_product_degree_bound(coefficient, factor))?;
            let scaled = coefficient * factor;
            if let Some(current) = output.coefficient(integral) {
                self.check_coefficient_degree(coefficient_sum_degree_bound(current, &scaled))?;
            }
            output.add_term(integral.clone(), scaled);
        }
        Ok(())
    }

    fn pair_sector_coefficient(
        &self,
        inactive_power: i32,
        left_power: i32,
        right_power: i32,
    ) -> Coefficient {
        debug_assert!(inactive_power <= 0);
        debug_assert!(left_power > 0 && right_power > 0);

        let context = self.family.coefficients();
        let dimension = self.family.dimension();
        let s = &self.signed_mass_squared;
        let numerator_degree = inactive_power.unsigned_abs() as usize;

        // Cache s^n once.  Formula (B) never needs a power above r.
        let mut mass_powers = Vec::with_capacity(numerator_degree.saturating_add(1));
        mass_powers.push(context.one());
        for exponent in 1..=numerator_degree {
            let next = &mass_powers[exponent - 1] * s;
            mass_powers.push(next);
        }

        // Only a width-r window of each tadpole sequence is referenced.  This
        // avoids retaining all lower powers when b or c is much larger than r.
        let left_tadpoles = TadpoleWindow::new(
            context,
            dimension,
            s,
            i64::from(left_power),
            numerator_degree,
        );
        let right_tadpoles = TadpoleWindow::new(
            context,
            dimension,
            s,
            i64::from(right_power),
            numerator_degree,
        );

        let mut answer = context.zero();
        let mut choose_r_2t = context.one();
        // This stores 4^t (1/2)_t / (d/2)_t directly.
        let mut angular_factor = context.one();

        for t in 0..=numerator_degree / 2 {
            if t != 0 {
                let previous_t = t - 1;
                choose_r_2t = multiply_integer_ratio(
                    context,
                    &choose_r_2t,
                    &[
                        numerator_degree - 2 * previous_t,
                        numerator_degree - 2 * previous_t - 1,
                    ],
                    &[2 * previous_t + 1, 2 * previous_t + 2],
                );

                let angular_numerator = context.integer(
                    i64::try_from(4 * (2 * previous_t + 1))
                        .expect("the i32 integral power fits in i64"),
                );
                let angular_denominator = dimension
                    + &context.integer(
                        i64::try_from(2 * previous_t).expect("the i32 integral power fits in i64"),
                    );
                angular_factor = &(&angular_factor * &angular_numerator) / &angular_denominator;
            }

            let remaining_degree = numerator_degree - 2 * t;
            let binomial_t = binomial_row(context, t);

            // Expanding (D_p+s)^t and (D_q+s)^t factorizes the i,j
            // sums in (B).  Precomputing them lowers the direct formula from
            // five nested summations to O(r^3) coefficient operations.
            let mut left_moments = Vec::with_capacity(remaining_degree + 1);
            let mut right_moments = Vec::with_capacity(remaining_degree + 1);
            for denominator_power in 0..=remaining_degree {
                let mut left = context.zero();
                let mut right = context.zero();
                for angular_power in 0..=t {
                    let factor = &binomial_t[angular_power] * &mass_powers[t - angular_power];
                    let left_index = i64::from(left_power)
                        - i64::try_from(denominator_power + angular_power)
                            .expect("the i32 integral power fits in i64");
                    let right_index = i64::from(right_power)
                        - i64::try_from(denominator_power + angular_power)
                            .expect("the i32 integral power fits in i64");
                    if left_index > 0 {
                        left = &left + &(&factor * left_tadpoles.ratio(left_index));
                    }
                    if right_index > 0 {
                        right = &right + &(&factor * right_tadpoles.ratio(right_index));
                    }
                }
                left_moments.push(left);
                right_moments.push(right);
            }

            // Multinomial(n;u,v,w) is advanced by exact neighboring ratios,
            // avoiding factorial overflow and keeping arbitrary-size integers
            // inside Symbolica's coefficient domain.
            let mut middle_sum = context.zero();
            let mut choose_n_u = context.one();
            for u in 0..=remaining_degree {
                let mut multinomial = choose_n_u.clone();
                let maximum_v = remaining_degree - u;
                for v in 0..=maximum_v {
                    let w = maximum_v - v;
                    if !left_moments[u].is_zero() && !right_moments[v].is_zero() {
                        let term = &(&(&multinomial * &mass_powers[w]) * &left_moments[u])
                            * &right_moments[v];
                        middle_sum = &middle_sum + &term;
                    }
                    if v != maximum_v {
                        multinomial = multiply_integer_ratio(context, &multinomial, &[w], &[v + 1]);
                    }
                }
                if u != remaining_degree {
                    choose_n_u = multiply_integer_ratio(
                        context,
                        &choose_n_u,
                        &[remaining_degree - u],
                        &[u + 1],
                    );
                }
            }

            let contribution = &(&choose_r_2t * &angular_factor) * &middle_sum;
            answer = &answer + &contribution;
        }

        answer
    }
}

/// Construction or domain error from [`TwoLoopBoundaryReducer`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TwoLoopBoundaryError {
    WrongLoopCount {
        actual: usize,
    },
    WrongDenominatorCount {
        actual: usize,
    },
    WrongMomentumRouting,
    UnequalMasses,
    MixedPropagatorSigns,
    MasslessFamily,
    IncompleteSymmetry {
        actual: usize,
    },
    WrongIntegralArity {
        actual: usize,
    },
    CoefficientExponentLimit {
        requested: u128,
        limit: u128,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    TopSector(Integral),
}

impl fmt::Display for TwoLoopBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLoopCount { actual } => {
                write!(
                    formatter,
                    "the two-loop boundary reducer received {actual} loops"
                )
            }
            Self::WrongDenominatorCount { actual } => write!(
                formatter,
                "the two-loop boundary reducer needs three denominators, received {actual}"
            ),
            Self::WrongMomentumRouting => formatter.write_str(
                "the denominator basis is not the built-in equal-mass two-loop sunset routing",
            ),
            Self::UnequalMasses => formatter
                .write_str("the two-loop boundary formula requires equal denominator shifts"),
            Self::MixedPropagatorSigns => formatter.write_str(
                "the two-loop boundary formula requires one common overall propagator sign",
            ),
            Self::MasslessFamily => formatter.write_str(
                "the equal-mass boundary master is scaleless when the common shift is zero",
            ),
            Self::IncompleteSymmetry { actual } => write!(
                formatter,
                "the equal-mass family needs all six denominator permutations, found {actual}"
            ),
            Self::WrongIntegralArity { actual } => write!(
                formatter,
                "a two-loop vacuum integral needs three powers, received {actual}"
            ),
            Self::CoefficientExponentLimit { requested, limit } => write!(
                formatter,
                "two-loop boundary Symbolica coefficient degree {requested} exceeds exponent limit {limit}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "two-loop boundary {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::TopSector(integral) => write!(
                formatter,
                "{integral} is a top-sector integral, not a boundary integral"
            ),
        }
    }
}

impl std::error::Error for TwoLoopBoundaryError {}

/// Conservative direct-formula iteration estimate.  `r+1` bounds every dense
/// numerator-indexed cache dimension, and the finite angular/multinomial sum
/// is cubic in that width.  Advancing the two tadpole windows costs no more
/// than the sum of their positive powers.
pub(crate) fn pair_sector_work_estimate(
    inactive_power: i32,
    left_power: i32,
    right_power: i32,
) -> u128 {
    let numerator_width = u128::from(inactive_power.unsigned_abs()).saturating_add(1);
    numerator_width
        .saturating_mul(numerator_width)
        .saturating_mul(numerator_width)
        .saturating_add(u128::try_from(left_power).unwrap_or(u128::MAX))
        .saturating_add(u128::try_from(right_power).unwrap_or(u128::MAX))
}

/// A contiguous set of `T_n / T_1` values for `1 <= n <= maximum`.
struct TadpoleWindow {
    minimum: i64,
    values: Vec<Coefficient>,
}

impl TadpoleWindow {
    fn new(
        context: &rustred::CoefficientContext,
        dimension: &Coefficient,
        signed_mass_squared: &Coefficient,
        maximum: i64,
        width: usize,
    ) -> Self {
        debug_assert!(maximum > 0);
        let width = i64::try_from(width).expect("the i32 integral power fits in i64");
        let minimum = 1_i64.max(maximum - width);

        let mut current = context.one();
        for n in 1..minimum {
            current = next_tadpole_ratio(context, dimension, signed_mass_squared, n, &current);
        }

        let mut values = Vec::with_capacity(
            usize::try_from(maximum - minimum + 1).expect("the i32 integral power fits in usize"),
        );
        values.push(current.clone());
        for n in minimum..maximum {
            current = next_tadpole_ratio(context, dimension, signed_mass_squared, n, &current);
            values.push(current.clone());
        }
        Self { minimum, values }
    }

    fn ratio(&self, power: i64) -> &Coefficient {
        let offset = usize::try_from(power - self.minimum)
            .expect("the requested tadpole belongs to the cached positive window");
        &self.values[offset]
    }
}

fn next_tadpole_ratio(
    context: &rustred::CoefficientContext,
    dimension: &Coefficient,
    signed_mass_squared: &Coefficient,
    n: i64,
    current: &Coefficient,
) -> Coefficient {
    let numerator = dimension - &context.integer(2 * n);
    let denominator = &context.integer(2 * n) * signed_mass_squared;
    &(current * &numerator) / &denominator
}

fn binomial_row(context: &rustred::CoefficientContext, n: usize) -> Vec<Coefficient> {
    let mut row = Vec::with_capacity(n.saturating_add(1));
    let mut value = context.one();
    row.push(value.clone());
    for k in 0..n {
        value = multiply_integer_ratio(context, &value, &[n - k], &[k + 1]);
        row.push(value.clone());
    }
    row
}

/// Multiply by a product of nonnegative machine integers without ever
/// materializing their potentially overflowing product in a machine type.
fn multiply_integer_ratio(
    context: &rustred::CoefficientContext,
    value: &Coefficient,
    numerators: &[usize],
    denominators: &[usize],
) -> Coefficient {
    let mut result = value.clone();
    for &numerator in numerators {
        result = &result
            * &context
                .integer(i64::try_from(numerator).expect("the i32 integral power fits in i64"));
    }
    for &denominator in denominators {
        result = &result
            / &context
                .integer(i64::try_from(denominator).expect("the i32 integral power fits in i64"));
    }
    result
}
