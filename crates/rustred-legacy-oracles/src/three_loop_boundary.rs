//! Exact factorized-boundary reduction for the equal-mass three-loop tetrahedron.
//!
//! Connected three-edge tree sectors factor into three one-loop tadpoles.  The
//! four-edge paw sector factors into one bridge tadpole times the equal-mass
//! two-loop sunset.  Arbitrary polynomial numerator powers on inactive lines
//! are expanded and angularly averaged by finite exact algorithms, subject to
//! explicit resource limits.
//!
//! The four-cycle, five-line, and six-line sectors are left to the general
//! three-loop reducer through [`ThreeLoopBoundaryReducer::try_reduce_integral`].

use std::collections::BTreeMap;
use std::fmt;

use crate::three_loop::THREE_LOOP_TETRAHEDRON_ROUTINGS;
use crate::{Denominator, Integral, LinearCombination, VacuumFamily};
use crate::{
    TwoLoopPipelineError, TwoLoopReductionConfig, TwoLoopReductionPipeline, TwoLoopTopDotConfig,
    TwoLoopTopDotError, TwoLoopTopDotReducer,
};
use rustred::legacy_oracle_support::coefficient_degree::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound,
    symbolica_coefficient_degree_is_representable,
};
use rustred::{Coefficient, ExactRational, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT};

/// Resource bounds for the certified scalar three-loop boundary slice.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreeLoopBoundaryConfig {
    /// Maximum total absolute degree on inactive numerator lines.
    pub max_numerator_degree: u32,
    /// Maximum number of distinct monomials in an intermediate polynomial.
    pub max_polynomial_terms: usize,
    /// Maximum sparse affine-multiplication operations in one reduction.
    pub max_polynomial_operations: u128,
    /// Maximum mixed angular-contraction terms visited in one reduction.
    pub max_angular_terms: u128,
    /// Maximum number of one-loop recurrence steps in one input.  Symbolica's
    /// hard coefficient-exponent limit remains in force when this work limit
    /// is larger.
    pub max_tadpole_steps: usize,
    /// Dot coverage retained for the cached finite two-loop compatibility
    /// pipeline.  Paw reduction itself uses the complete positive-dot reducer
    /// and is not capped by this value.
    pub max_two_loop_dots: u32,
    /// Seed-candidate bound for the cached finite pipeline and memoized-state
    /// bound for complete positive-dot paw reduction.
    pub max_two_loop_seed_candidates: usize,
    /// Analytic-boundary work bound shared by both two-loop reducers.
    pub max_two_loop_boundary_terms: usize,
}

impl Default for ThreeLoopBoundaryConfig {
    fn default() -> Self {
        Self {
            max_numerator_degree: 32,
            max_polynomial_terms: 1_000_000,
            max_polynomial_operations: 10_000_000,
            max_angular_terms: 1_000_000,
            max_tadpole_steps: 1_000_000,
            max_two_loop_dots: 9,
            max_two_loop_seed_candidates: 10_000,
            max_two_loop_boundary_terms: 1_000_000,
        }
    }
}

/// An owned, reusable reducer for scalar three-loop boundary integrals.
///
/// The family is owned so this reducer can be stored next to a three-loop
/// reduction table without a self-referential borrow.  Construction retains a
/// reusable finite two-loop pipeline for compatibility and also builds the
/// complete positive-dot reducer used by paw reductions.  Neither service is
/// rebuilt per call.
#[derive(Clone, Debug)]
pub struct ThreeLoopBoundaryReducer {
    family: VacuumFamily,
    config: ThreeLoopBoundaryConfig,
    two_loop: TwoLoopReductionPipeline,
    two_loop_top_dot: TwoLoopTopDotReducer,
    product_master: Integral,
    sunset_times_tadpole_master: Integral,
}

impl ThreeLoopBoundaryReducer {
    /// Validate and take ownership of the built-in positive-Euclidean family.
    pub fn new(
        family: VacuumFamily,
        config: ThreeLoopBoundaryConfig,
    ) -> Result<Self, ThreeLoopBoundaryError> {
        validate_family(&family)?;
        validate_resource_config(config)?;
        let two_loop_family = induced_two_loop_family(&family)?;
        let two_loop = TwoLoopReductionPipeline::build_for_family(
            two_loop_family,
            TwoLoopReductionConfig {
                max_dots: config.max_two_loop_dots,
                max_numerator_degree: config.max_numerator_degree,
                max_seed_candidates: config.max_two_loop_seed_candidates,
                max_boundary_terms: config.max_two_loop_boundary_terms,
            },
        )?;
        let two_loop_top_dot = TwoLoopTopDotReducer::new(
            two_loop.family().clone(),
            TwoLoopTopDotConfig {
                // Reuse the existing public boundary budgets rather than add
                // fields to `ThreeLoopBoundaryConfig` and break struct-literal
                // compatibility.  The complete reducer performs conservative
                // whole-request preflights before coefficient construction.
                max_explicit_terms: config.max_polynomial_terms,
                max_raw_terms: usize::try_from(config.max_polynomial_operations)
                    .unwrap_or(usize::MAX),
                max_states: config.max_two_loop_seed_candidates,
                max_coefficient_operations: usize::try_from(config.max_polynomial_operations)
                    .unwrap_or(usize::MAX),
                max_coefficient_degree: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
                max_boundary_formula_iterations: config.max_two_loop_boundary_terms,
            },
        )?;

        Ok(Self {
            family,
            config,
            two_loop,
            two_loop_top_dot,
            product_master: Integral::from([1, 1, 1, 0, 0, 0]),
            sunset_times_tadpole_master: Integral::from([1, 1, 1, 1, 0, 0]),
        })
    }

    pub fn family(&self) -> &VacuumFamily {
        &self.family
    }

    pub fn config(&self) -> ThreeLoopBoundaryConfig {
        self.config
    }

    /// Stable output representative `P3 = I(1,1,1,0,0,0) = T1^3`.
    pub fn product_master(&self) -> &Integral {
        &self.product_master
    }

    /// Stable output representative `ST = I(1,1,1,1,0,0) = T1*S(1,1,1)`.
    pub fn sunset_times_tadpole_master(&self) -> &Integral {
        &self.sunset_times_tadpole_master
    }

    pub fn two_loop_pipeline(&self) -> &TwoLoopReductionPipeline {
        &self.two_loop
    }

    /// Complete induced sunset service used by actual paw reduction.
    ///
    /// This accessor also exposes exact `E00-E01` provenance replay and typed
    /// normal-form resource preflight for callers auditing the composition.
    pub fn two_loop_top_dot_reducer(&self) -> &TwoLoopTopDotReducer {
        &self.two_loop_top_dot
    }

    /// Reduce a supported boundary, return zero for a scaleless sector, or
    /// return `Ok(None)` for a genuine non-factorized three-loop sector.
    ///
    /// Negative powers are accepted in the three factorized boundary orbits.
    /// The sector is canonicalized before numerator resource work, so a
    /// scaleless input returns zero and a genuine three-loop sector returns
    /// `Ok(None)` without expanding an irrelevant polynomial.
    pub fn try_reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<Option<LinearCombination>, ThreeLoopBoundaryError> {
        validate_integral_arity(integral)?;
        let Some(canonical) = canonicalize_sector_first(&self.family, integral) else {
            return Ok(Some(LinearCombination::new()));
        };
        let mask = sector_mask(&canonical);
        match mask {
            7 | 11 => self.reduce_tree(&canonical).map(Some),
            15 => self.reduce_paw(&canonical).map(Some),
            _ => Ok(None),
        }
    }

    /// Reduce an integral known to belong to the supported scalar boundary.
    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ThreeLoopBoundaryError> {
        self.try_reduce_integral(integral)?.ok_or_else(|| {
            ThreeLoopBoundaryError::UnsupportedSector {
                integral: integral.clone(),
                mask: sector_mask(integral),
            }
        })
    }

    /// Close supported terms in a sum while preserving genuine three-loop terms.
    pub fn reduce_combination(
        &self,
        combination: &LinearCombination,
    ) -> Result<LinearCombination, ThreeLoopBoundaryError> {
        let mut result = LinearCombination::new();
        for (integral, coefficient) in combination.terms() {
            if let Some(reduction) = self.try_reduce_integral(integral)? {
                self.add_scaled_checked(&mut result, &reduction, coefficient)?;
            } else {
                result.add_term(integral.clone(), coefficient.clone());
            }
        }
        Ok(result)
    }

    fn reduce_tree(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ThreeLoopBoundaryError> {
        let (active_positions, inactive): ([usize; 3], [(usize, [i8; 3]); 3]) =
            match sector_mask(integral) {
                7 => (
                    [0, 1, 2],
                    [(3, [-1, 0, 1]), (4, [1, -1, 0]), (5, [0, 1, -1])],
                ),
                11 => (
                    [0, 1, 3],
                    [(2, [1, 0, 1]), (4, [1, -1, 0]), (5, [-1, 1, -1])],
                ),
                _ => unreachable!("tree reducer received a non-tree sector"),
            };
        let active = active_positions.map(|position| integral.powers()[position]);
        debug_assert!(active.iter().all(|power| *power > 0));

        let numerator_degree = self.check_numerator_degree(integral)?;
        self.check_polynomial_term_bound(numerator_degree, 6)?;
        let tadpole_steps = active.iter().fold(0_u128, |total, power| {
            total.saturating_add((*power as u128).saturating_sub(1))
        });
        self.check_tadpole_steps(tadpole_steps)?;
        self.check_coefficient_degree(tadpole_steps.saturating_add(u128::from(numerator_degree)))?;

        let context = self.family.coefficients();
        let mass = context
            .parameter("m2")
            .expect("family validation requires m2");
        let mut work = BoundaryWork::default();
        let mut polynomial = BTreeMap::from([([0_u32; 6], context.one())]);
        for (position, routing) in inactive {
            let affine = tree_denominator_affine(context, &mass, routing);
            for _ in 0..integral.powers()[position].unsigned_abs() {
                polynomial = multiply_sparse_affine(polynomial, &affine, self.config, &mut work)?;
            }
        }

        let mut radial = RadialCache::new(context, self.family.dimension().clone(), mass);
        let mut angular = AngularCache::new(context, self.family.dimension().clone());
        let mut answer = context.zero();
        for (powers, polynomial_coefficient) in polynomial {
            let [n11, n22, n33, n12, n13, n23] = powers;
            let cross_degree = checked_add_u32(n13, n23)?;
            if cross_degree % 2 != 0 {
                continue;
            }
            let rank = cross_degree / 2;
            // Charge a conservative bound before constructing factorial-sized
            // pairing coefficients or extending dense angular caches.  The
            // limit must bound work, not merely count a Vec after allocation.
            work.add_angular_terms(angular_work_bound(n13, n23), self.config)?;
            let pairings = mixed_pairing_multiplicities(context, n13, n23);
            let mixed_angular = angular.inverse_h(rank);
            for (cross_pairs, multiplicity) in pairings {
                let left_square = (n13 - cross_pairs) / 2;
                let right_square = (n23 - cross_pairs) / 2;
                let remaining_n11 = checked_add_u32(n11, left_square)?;
                let remaining_n22 = checked_add_u32(n22, right_square)?;
                let remaining_n12 = checked_add_u32(n12, cross_pairs)?;
                if remaining_n12 % 2 != 0 {
                    continue;
                }
                let second_rank = remaining_n12 / 2;
                let p3_moment = checked_add_u32(n33, rank)?;
                let p2_moment = checked_add_u32(remaining_n22, second_rank)?;
                let p1_moment = checked_add_u32(remaining_n11, second_rank)?;
                let single_angular = angular.single_vector_factor(second_rank);
                let p3_radial = radial.moment(active[2], p3_moment);
                let p2_radial = radial.moment(active[1], p2_moment);
                let p1_radial = radial.moment(active[0], p1_moment);
                let mut coefficient = &polynomial_coefficient * &multiplicity;
                coefficient = &coefficient * &mixed_angular;
                coefficient = &coefficient * &single_angular;
                coefficient = &coefficient * &p3_radial;
                coefficient = &coefficient * &p2_radial;
                coefficient = &coefficient * &p1_radial;
                answer = &answer + &coefficient;
            }
        }
        Ok(LinearCombination::from_term(
            self.product_master.clone(),
            answer,
        ))
    }

    fn reduce_paw(&self, integral: &Integral) -> Result<LinearCombination, ThreeLoopBoundaryError> {
        debug_assert_eq!(sector_mask(integral), 15);
        let powers = integral.powers();
        debug_assert!(powers[..4].iter().all(|power| *power > 0));
        debug_assert!(powers[4..].iter().all(|power| *power <= 0));

        let numerator_degree = self.check_numerator_degree(integral)?;
        self.check_polynomial_term_bound(numerator_degree, 5)?;

        // D2=k2^2+m2 is the bridge tadpole.  D1,D3,D4 become the standard
        // sunset after q=-k3, so no sign or Jacobian factor is introduced.
        let tadpole_steps = (powers[1] as u128).saturating_sub(1);
        self.check_tadpole_steps(tadpole_steps)?;
        self.check_coefficient_degree(tadpole_steps.saturating_add(u128::from(numerator_degree)))?;

        let context = self.family.coefficients();
        let mass = context
            .parameter("m2")
            .expect("family validation requires m2");
        let mut work = BoundaryWork::default();
        let mut polynomial = BTreeMap::from([([0_u32; 5], context.one())]);
        let d5 = paw_inactive_affine(context, &mass, true);
        let d6 = paw_inactive_affine(context, &mass, false);
        for _ in 0..powers[4].unsigned_abs() {
            polynomial = multiply_sparse_affine(polynomial, &d5, self.config, &mut work)?;
        }
        for _ in 0..powers[5].unsigned_abs() {
            polynomial = multiply_sparse_affine(polynomial, &d6, self.config, &mut work)?;
        }

        let mut radial = RadialCache::new(context, self.family.dimension().clone(), mass.clone());
        let mut angular = AngularCache::new(context, self.family.dimension().clone());
        let d1_minus_mass = denominator_minus_mass_affine(context, &mass, 0);
        let d3_minus_mass = denominator_minus_mass_affine(context, &mass, 1);
        let uv = sunset_scalar_product_affine(context, &mass);
        let mut two_loop_input = LinearCombination::new();

        for (monomial, polynomial_coefficient) in polynomial {
            let [d1_power, d2_power, d3_power, up_power, vp_power] = monomial;
            let cross_degree = checked_add_u32(up_power, vp_power)?;
            if cross_degree % 2 != 0 {
                continue;
            }
            let rank = cross_degree / 2;
            let bridge_power = checked_lower_power(powers[1], d2_power)?;
            let bridge_moment = radial.moment(bridge_power, rank);
            if bridge_moment.is_zero() {
                continue;
            }
            // Preflight all pairing/cache work before constructing it.
            work.add_angular_terms(angular_work_bound(up_power, vp_power), self.config)?;
            let pairings = mixed_pairing_multiplicities(context, up_power, vp_power);
            let inverse_h = angular.inverse_h(rank);
            for (cross_pairs, multiplicity) in pairings {
                let u_square_power = (up_power - cross_pairs) / 2;
                let v_square_power = (vp_power - cross_pairs) / 2;
                let base_coefficient =
                    &(&(&polynomial_coefficient * &multiplicity) * &inverse_h) * &bridge_moment;
                let mut sunset_polynomial =
                    BTreeMap::from([([d1_power, d3_power, 0_u32], base_coefficient)]);
                for _ in 0..u_square_power {
                    sunset_polynomial = multiply_sparse_affine(
                        sunset_polynomial,
                        &d1_minus_mass,
                        self.config,
                        &mut work,
                    )?;
                }
                for _ in 0..v_square_power {
                    sunset_polynomial = multiply_sparse_affine(
                        sunset_polynomial,
                        &d3_minus_mass,
                        self.config,
                        &mut work,
                    )?;
                }
                for _ in 0..cross_pairs {
                    sunset_polynomial =
                        multiply_sparse_affine(sunset_polynomial, &uv, self.config, &mut work)?;
                }

                for (shifts, coefficient) in sunset_polynomial {
                    let shifted = Integral::from([
                        checked_lower_power(powers[0], shifts[0])?,
                        checked_lower_power(powers[2], shifts[1])?,
                        checked_lower_power(powers[3], shifts[2])?,
                    ]);
                    two_loop_input.add_term(shifted, coefficient);
                    if two_loop_input.len() > self.config.max_polynomial_terms {
                        return Err(ThreeLoopBoundaryError::ResourceLimit {
                            resource: "two-loop boundary polynomial terms",
                            requested: two_loop_input.len() as u128,
                            limit: self.config.max_polynomial_terms as u128,
                        });
                    }
                }
            }
        }

        let reduced = self.reduce_two_loop_combination_checked(&two_loop_input)?;
        let mut result = LinearCombination::new();
        for (master, coefficient) in reduced.terms() {
            let embedded = if master == self.two_loop_top_dot.sunset_master() {
                self.sunset_times_tadpole_master.clone()
            } else if master == self.two_loop_top_dot.product_master() {
                self.product_master.clone()
            } else {
                return Err(ThreeLoopBoundaryError::UnexpectedTwoLoopMaster {
                    integral: master.clone(),
                });
            };
            result.add_term(embedded, coefficient.clone());
        }
        Ok(result)
    }

    fn check_numerator_degree(&self, integral: &Integral) -> Result<u32, ThreeLoopBoundaryError> {
        let degree =
            integral
                .checked_numerator_degree()
                .ok_or(ThreeLoopBoundaryError::ResourceLimit {
                    resource: "numerator degree",
                    requested: u128::MAX,
                    limit: u128::from(self.config.max_numerator_degree),
                })?;
        if degree > self.config.max_numerator_degree {
            return Err(ThreeLoopBoundaryError::ResourceLimit {
                resource: "numerator degree",
                requested: u128::from(degree),
                limit: u128::from(self.config.max_numerator_degree),
            });
        }
        Ok(degree)
    }

    fn check_polynomial_term_bound(
        &self,
        numerator_degree: u32,
        variables: u32,
    ) -> Result<(), ThreeLoopBoundaryError> {
        let requested = binomial_saturating(
            u128::from(numerator_degree) + u128::from(variables),
            variables,
        );
        if requested > self.config.max_polynomial_terms as u128 {
            return Err(ThreeLoopBoundaryError::ResourceLimit {
                resource: "polynomial term upper bound",
                requested,
                limit: self.config.max_polynomial_terms as u128,
            });
        }
        Ok(())
    }

    fn check_tadpole_steps(&self, requested: u128) -> Result<(), ThreeLoopBoundaryError> {
        if requested > self.config.max_tadpole_steps as u128 {
            return Err(ThreeLoopBoundaryError::ResourceLimit {
                resource: "tadpole recurrence steps",
                requested,
                limit: self.config.max_tadpole_steps as u128,
            });
        }
        Ok(())
    }

    /// In either supported factorization, every radial tadpole recurrence
    /// contributes at most one power of `d` and one inverse power of `m2`.
    /// A total inactive degree `r` contributes at most `r` further powers:
    /// polynomial mass insertions have degree at most `r`, while the nested
    /// angular denominators have a common denominator of degree at most `r`.
    /// Thus `tadpole_steps + r` bounds every coefficient exponent constructed
    /// before the separately bounded complete two-loop paw subreduction.
    fn check_coefficient_degree(&self, requested: u128) -> Result<(), ThreeLoopBoundaryError> {
        if !symbolica_coefficient_degree_is_representable(requested) {
            return Err(ThreeLoopBoundaryError::ResourceLimit {
                resource: "Symbolica coefficient exponent degree",
                requested,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            });
        }
        Ok(())
    }

    fn add_scaled_checked(
        &self,
        output: &mut LinearCombination,
        reduction: &LinearCombination,
        factor: &Coefficient,
    ) -> Result<(), ThreeLoopBoundaryError> {
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

    /// Compose the paw polynomial with the complete positive-dot two-loop
    /// reducer using the same checked arithmetic as the public combination
    /// API.  The nested reducer preflights its complete dependency DAG and
    /// closes any pinches through the arbitrary-numerator boundary formula.
    fn reduce_two_loop_combination_checked(
        &self,
        combination: &LinearCombination,
    ) -> Result<LinearCombination, ThreeLoopBoundaryError> {
        let mut output = LinearCombination::new();
        for (integral, coefficient) in combination.terms() {
            let reduction = self.two_loop_top_dot.reduce_integral(integral)?;
            self.add_scaled_checked(&mut output, &reduction, coefficient)?;
        }
        Ok(output)
    }
}

type SparsePolynomial<const N: usize> = BTreeMap<[u32; N], Coefficient>;

#[derive(Default)]
struct BoundaryWork {
    polynomial_operations: u128,
    angular_terms: u128,
}

impl BoundaryWork {
    fn add_polynomial_operations(
        &mut self,
        increment: u128,
        config: ThreeLoopBoundaryConfig,
    ) -> Result<(), ThreeLoopBoundaryError> {
        self.polynomial_operations = self.polynomial_operations.saturating_add(increment);
        if self.polynomial_operations > config.max_polynomial_operations {
            return Err(ThreeLoopBoundaryError::ResourceLimit {
                resource: "polynomial expansion operations",
                requested: self.polynomial_operations,
                limit: config.max_polynomial_operations,
            });
        }
        Ok(())
    }

    fn add_angular_terms(
        &mut self,
        increment: u128,
        config: ThreeLoopBoundaryConfig,
    ) -> Result<(), ThreeLoopBoundaryError> {
        self.angular_terms = self.angular_terms.saturating_add(increment);
        if self.angular_terms > config.max_angular_terms {
            return Err(ThreeLoopBoundaryError::ResourceLimit {
                resource: "angular contraction terms",
                requested: self.angular_terms,
                limit: config.max_angular_terms,
            });
        }
        Ok(())
    }
}

fn multiply_sparse_affine<const N: usize>(
    polynomial: SparsePolynomial<N>,
    affine: &[([u32; N], Coefficient)],
    config: ThreeLoopBoundaryConfig,
    work: &mut BoundaryWork,
) -> Result<SparsePolynomial<N>, ThreeLoopBoundaryError> {
    let attempted = (polynomial.len() as u128).saturating_mul(affine.len() as u128);
    work.add_polynomial_operations(attempted, config)?;

    let mut output = BTreeMap::new();
    for (powers, coefficient) in polynomial {
        for (increment, affine_coefficient) in affine {
            let mut shifted = powers;
            for position in 0..N {
                shifted[position] = shifted[position]
                    .checked_add(increment[position])
                    .ok_or(ThreeLoopBoundaryError::ExponentOverflow)?;
            }
            add_sparse_term(&mut output, shifted, &coefficient * affine_coefficient);
            if output.len() > config.max_polynomial_terms {
                return Err(ThreeLoopBoundaryError::ResourceLimit {
                    resource: "intermediate polynomial terms",
                    requested: output.len() as u128,
                    limit: config.max_polynomial_terms as u128,
                });
            }
        }
    }
    Ok(output)
}

fn add_sparse_term<const N: usize>(
    polynomial: &mut SparsePolynomial<N>,
    powers: [u32; N],
    coefficient: Coefficient,
) {
    if coefficient.is_zero() {
        return;
    }
    if let Some(current) = polynomial.get_mut(&powers) {
        let sum = &*current + &coefficient;
        if sum.is_zero() {
            polynomial.remove(&powers);
        } else {
            *current = sum;
        }
    } else {
        polynomial.insert(powers, coefficient);
    }
}

fn unit_power<const N: usize>(position: usize) -> [u32; N] {
    let mut powers = [0_u32; N];
    powers[position] = 1;
    powers
}

fn tree_denominator_affine(
    context: &rustred::CoefficientContext,
    mass: &Coefficient,
    routing: [i8; 3],
) -> Vec<([u32; 6], Coefficient)> {
    let mut output = vec![([0_u32; 6], mass.clone())];
    for (position, component) in routing.iter().copied().enumerate() {
        let coefficient = i64::from(component) * i64::from(component);
        if coefficient != 0 {
            output.push((unit_power(position), context.integer(coefficient)));
        }
    }
    for (left, right, position) in [(0, 1, 3), (0, 2, 4), (1, 2, 5)] {
        let coefficient = 2_i64 * i64::from(routing[left]) * i64::from(routing[right]);
        if coefficient != 0 {
            output.push((unit_power(position), context.integer(coefficient)));
        }
    }
    output
}

fn paw_inactive_affine(
    context: &rustred::CoefficientContext,
    mass: &Coefficient,
    d5: bool,
) -> Vec<([u32; 5], Coefficient)> {
    let (outer_denominator, scalar_product) = if d5 { (0, 3) } else { (2, 4) };
    vec![
        ([0_u32; 5], -mass.clone()),
        (unit_power(outer_denominator), context.one()),
        (unit_power(1), context.one()),
        (unit_power(scalar_product), context.integer(-2)),
    ]
}

fn denominator_minus_mass_affine(
    context: &rustred::CoefficientContext,
    mass: &Coefficient,
    denominator: usize,
) -> Vec<([u32; 3], Coefficient)> {
    vec![
        ([0_u32; 3], -mass.clone()),
        (unit_power(denominator), context.one()),
    ]
}

fn sunset_scalar_product_affine(
    context: &rustred::CoefficientContext,
    mass: &Coefficient,
) -> Vec<([u32; 3], Coefficient)> {
    let half = context.rational(ExactRational::new(1, 2));
    vec![
        ([0_u32; 3], -(&half * mass)),
        (unit_power(0), half.clone()),
        (unit_power(1), half.clone()),
        (unit_power(2), -half),
    ]
}

struct RadialCache<'context> {
    context: &'context rustred::CoefficientContext,
    dimension: Coefficient,
    mass: Coefficient,
    tadpole_ratios: Vec<Coefficient>,
    moments: BTreeMap<(i32, u32), Coefficient>,
}

impl<'context> RadialCache<'context> {
    fn new(
        context: &'context rustred::CoefficientContext,
        dimension: Coefficient,
        mass: Coefficient,
    ) -> Self {
        Self {
            context,
            dimension,
            mass,
            tadpole_ratios: vec![context.zero(), context.one()],
            moments: BTreeMap::new(),
        }
    }

    fn tadpole_ratio(&mut self, power: i32) -> Coefficient {
        debug_assert!(power > 0);
        let power = usize::try_from(power).expect("a positive i32 fits in usize");
        while self.tadpole_ratios.len() <= power {
            let n = i64::try_from(self.tadpole_ratios.len() - 1)
                .expect("an i32 tadpole power fits in i64");
            let two_n = self.context.integer(2 * n);
            let next = &(&self.tadpole_ratios[n as usize] * &(&two_n - &self.dimension))
                / &(&two_n * &self.mass);
            self.tadpole_ratios.push(next);
        }
        self.tadpole_ratios[power].clone()
    }

    fn moment(&mut self, power: i32, moment: u32) -> Coefficient {
        if let Some(value) = self.moments.get(&(power, moment)) {
            return value.clone();
        }
        if power <= 0 {
            return self.context.zero();
        }

        let upper = moment.min((power - 1) as u32);
        let negative_mass = -self.mass.clone();
        let mut choose = self.context.one();
        let mut value = self.context.zero();
        for j in 0..=upper {
            let mass_power = negative_mass.pow(u64::from(moment - j));
            let shifted_power = power - i32::try_from(j).expect("j is bounded by an i32 power");
            let term = &(&choose * &mass_power) * &self.tadpole_ratio(shifted_power);
            value = &value + &term;
            if j != upper {
                choose = &(&choose * &self.context.integer(i64::from(moment - j)))
                    / &self.context.integer(i64::from(j + 1));
            }
        }
        self.moments.insert((power, moment), value.clone());
        value
    }
}

struct AngularCache<'context> {
    context: &'context rustred::CoefficientContext,
    dimension: Coefficient,
    inverse_h: Vec<Coefficient>,
    single_vector: Vec<Coefficient>,
}

impl<'context> AngularCache<'context> {
    fn new(context: &'context rustred::CoefficientContext, dimension: Coefficient) -> Self {
        Self {
            context,
            dimension,
            inverse_h: vec![context.one()],
            single_vector: vec![context.one()],
        }
    }

    fn inverse_h(&mut self, rank: u32) -> Coefficient {
        let rank = usize::try_from(rank).expect("a u32 rank fits in usize");
        while self.inverse_h.len() <= rank {
            let previous = self.inverse_h.len() - 1;
            let denominator = &self.dimension
                + &self
                    .context
                    .integer(2 * i64::try_from(previous).expect("a u32 rank fits in i64"));
            let next = &self.inverse_h[previous] / &denominator;
            self.inverse_h.push(next);
        }
        self.inverse_h[rank].clone()
    }

    fn single_vector_factor(&mut self, rank: u32) -> Coefficient {
        let rank = usize::try_from(rank).expect("a u32 rank fits in usize");
        while self.single_vector.len() <= rank {
            let previous = self.single_vector.len() - 1;
            let numerator = self
                .context
                .integer(2 * i64::try_from(previous).expect("a u32 rank fits in i64") + 1);
            let denominator = &self.dimension
                + &self
                    .context
                    .integer(2 * i64::try_from(previous).expect("a u32 rank fits in i64"));
            let next = &(&self.single_vector[previous] * &numerator) / &denominator;
            self.single_vector.push(next);
        }
        self.single_vector[rank].clone()
    }
}

fn mixed_pairing_multiplicities(
    context: &rustred::CoefficientContext,
    left: u32,
    right: u32,
) -> Vec<(u32, Coefficient)> {
    let Some(total) = left.checked_add(right) else {
        return Vec::new();
    };
    if total % 2 != 0 {
        return Vec::new();
    }
    let mut cross_pairs = left % 2;
    let maximum = left.min(right);
    let mut multiplicity = context.one();
    for occurrences in [left, right] {
        let maximum_factor = if cross_pairs == 0 {
            occurrences.saturating_sub(1)
        } else {
            occurrences
        };
        let mut factor = 1_u32;
        while factor <= maximum_factor {
            multiplicity = &multiplicity * &context.integer(i64::from(factor));
            factor = factor.saturating_add(2);
        }
    }

    let mut output = Vec::with_capacity((maximum / 2 + 1) as usize);
    loop {
        output.push((cross_pairs, multiplicity.clone()));
        let Some(next) = cross_pairs.checked_add(2) else {
            break;
        };
        if next > maximum {
            break;
        }
        let numerator_left = context.integer(i64::from(left - cross_pairs));
        let numerator_right = context.integer(i64::from(right - cross_pairs));
        let denominator_left = context.integer(i64::from(cross_pairs + 1));
        let denominator_right = context.integer(i64::from(cross_pairs + 2));
        multiplicity = &(&(&multiplicity * &numerator_left) * &numerator_right)
            / &(&denominator_left * &denominator_right);
        cross_pairs = next;
    }
    output
}

/// Conservative operation/allocation estimate for one mixed angular average.
///
/// Pairing construction initializes two double factorials, advances through
/// every allowed cross-pair count, and may extend both angular caches up to
/// ranks bounded by the total degree.  Deliberately over-counting repeated
/// cache extensions makes the public cap deterministic and ensures it is
/// checked before any work proportional to a caller-controlled numerator.
fn angular_work_bound(left: u32, right: u32) -> u128 {
    let left = u128::from(left);
    let right = u128::from(right);
    let minimum = left.min(right);
    let parity = left % 2;
    let pairings = if minimum < parity {
        0
    } else {
        (minimum - parity) / 2 + 1
    };
    1_u128
        .saturating_add(left)
        .saturating_add(right)
        .saturating_add(pairings)
        .saturating_add(left.saturating_add(right))
}

fn checked_add_u32(left: u32, right: u32) -> Result<u32, ThreeLoopBoundaryError> {
    left.checked_add(right)
        .ok_or(ThreeLoopBoundaryError::ExponentOverflow)
}

fn checked_lower_power(power: i32, shift: u32) -> Result<i32, ThreeLoopBoundaryError> {
    let shifted = i64::from(power) - i64::from(shift);
    i32::try_from(shifted).map_err(|_| ThreeLoopBoundaryError::ExponentOverflow)
}

fn binomial_saturating(n: u128, k: u32) -> u128 {
    let k = u128::from(k).min(n.saturating_sub(u128::from(k)));
    let mut result = 1_u128;
    for index in 0..k {
        let Some(product) = result.checked_mul(n - index) else {
            return u128::MAX;
        };
        result = product / (index + 1);
    }
    result
}

/// Typed construction, domain, and resource errors for the scalar boundary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeLoopBoundaryError {
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
        actual: usize,
    },
    UnsupportedSector {
        integral: Integral,
        mask: u8,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    ExponentOverflow,
    UnexpectedTwoLoopMaster {
        integral: Integral,
    },
    TwoLoop(TwoLoopPipelineError),
    TwoLoopTopDot(TwoLoopTopDotError),
}

impl fmt::Display for ThreeLoopBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLoopCount { actual } => write!(
                formatter,
                "the three-loop boundary reducer received {actual} loops"
            ),
            Self::WrongDenominatorCount { actual } => write!(
                formatter,
                "the three-loop tetrahedron needs six denominators, received {actual}"
            ),
            Self::WrongMomentumRouting => formatter.write_str(
                "the denominator order/routing is not the built-in three-loop tetrahedron",
            ),
            Self::WrongPropagatorSign { position } => write!(
                formatter,
                "denominator {position} is not a positive-Euclidean physical propagator"
            ),
            Self::UnequalMasses => formatter.write_str(
                "the scalar three-loop boundary formula requires one common nonzero mass",
            ),
            Self::MissingParameter { name } => {
                write!(
                    formatter,
                    "the three-loop family does not define parameter {name}"
                )
            }
            Self::IncompleteSymmetry { actual } => write!(
                formatter,
                "the tetrahedron boundary reducer needs all 24 S4 symmetries, found {actual}"
            ),
            Self::WrongIntegralArity { actual } => write!(
                formatter,
                "a three-loop tetrahedron integral needs six powers, received {actual}"
            ),
            Self::UnsupportedSector { integral, mask } => write!(
                formatter,
                "{integral} belongs to unsupported non-factorized sector mask {mask}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "three-loop boundary {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::ExponentOverflow => formatter.write_str(
                "three-loop boundary polynomial powers exceed RustRed's integer exponent range",
            ),
            Self::UnexpectedTwoLoopMaster { integral } => write!(
                formatter,
                "the complete two-loop paw reducer returned unexpected master {integral}"
            ),
            Self::TwoLoop(error) => write!(
                formatter,
                "finite two-loop compatibility pipeline failed: {error}"
            ),
            Self::TwoLoopTopDot(error) => {
                write!(formatter, "complete two-loop paw reduction failed: {error}")
            }
        }
    }
}

impl std::error::Error for ThreeLoopBoundaryError {}

impl From<TwoLoopPipelineError> for ThreeLoopBoundaryError {
    fn from(value: TwoLoopPipelineError) -> Self {
        Self::TwoLoop(value)
    }
}

impl From<TwoLoopTopDotError> for ThreeLoopBoundaryError {
    fn from(value: TwoLoopTopDotError) -> Self {
        Self::TwoLoopTopDot(value)
    }
}

fn validate_integral_arity(integral: &Integral) -> Result<(), ThreeLoopBoundaryError> {
    if integral.powers().len() != 6 {
        return Err(ThreeLoopBoundaryError::WrongIntegralArity {
            actual: integral.powers().len(),
        });
    }
    Ok(())
}

fn sector_mask(integral: &Integral) -> u8 {
    integral
        .powers()
        .iter()
        .enumerate()
        .fold(0_u8, |mask, (position, power)| {
            mask | (u8::from(*power > 0) << position)
        })
}

fn validate_resource_config(config: ThreeLoopBoundaryConfig) -> Result<(), ThreeLoopBoundaryError> {
    for (resource, requested, limit) in [
        (
            "intermediate polynomial terms",
            1_u128,
            config.max_polynomial_terms as u128,
        ),
        (
            "polynomial expansion operations",
            1_u128,
            config.max_polynomial_operations,
        ),
        (
            "angular contraction terms",
            1_u128,
            config.max_angular_terms,
        ),
    ] {
        if limit == 0 {
            return Err(ThreeLoopBoundaryError::ResourceLimit {
                resource,
                requested,
                limit,
            });
        }
    }
    Ok(())
}

/// Choose the lexicographically greatest sector representative first, then
/// use exponent ordering only to break ties inside that sector stabilizer.
///
/// `VacuumFamily::canonicalize` maximizes the complete exponent vector.  With
/// unequal dots that can select a different labelled tree or paw mask from the
/// Boolean sector representative on which the factorization formula is
/// defined.  Every image is still an equal integral, but dispatching by that
/// incidental mask would miss valid boundaries.
fn canonicalize_sector_first(family: &VacuumFamily, integral: &Integral) -> Option<Integral> {
    if family.is_scaleless(integral) {
        return None;
    }

    let sector_image = |permutation: &[usize]| {
        permutation
            .iter()
            .map(|&source| i32::from(integral.powers()[source] > 0))
            .collect::<Vec<_>>()
    };
    let representative = family
        .symmetries()
        .iter()
        .map(|permutation| sector_image(permutation))
        .max()
        .expect("a validated family symmetry group contains the identity");

    family
        .symmetries()
        .iter()
        .filter(|permutation| sector_image(permutation) == representative)
        .map(|permutation| {
            Integral::new(
                permutation
                    .iter()
                    .map(|&source| integral.powers()[source])
                    .collect::<Vec<_>>(),
            )
        })
        .max()
}

fn validate_family(family: &VacuumFamily) -> Result<(), ThreeLoopBoundaryError> {
    if family.loops() != 3 {
        return Err(ThreeLoopBoundaryError::WrongLoopCount {
            actual: family.loops(),
        });
    }
    if family.denominator_count() != 6 {
        return Err(ThreeLoopBoundaryError::WrongDenominatorCount {
            actual: family.denominator_count(),
        });
    }
    let mass = family
        .coefficients()
        .parameter("m2")
        .ok_or(ThreeLoopBoundaryError::MissingParameter { name: "m2" })?;
    let dimension = family
        .coefficients()
        .parameter("d")
        .ok_or(ThreeLoopBoundaryError::MissingParameter { name: "d" })?;
    if family.dimension() != &dimension {
        return Err(ThreeLoopBoundaryError::WrongMomentumRouting);
    }
    for (position, denominator) in family.denominators().iter().enumerate() {
        if denominator.normalization() != Some(1) {
            return Err(ThreeLoopBoundaryError::WrongPropagatorSign { position });
        }
        if denominator.shift() != &mass || denominator.shift().is_zero() {
            return Err(ThreeLoopBoundaryError::UnequalMasses);
        }
        let expected = Denominator::propagator(
            THREE_LOOP_TETRAHEDRON_ROUTINGS[position]
                .iter()
                .map(|component| ExactRational::from(i64::from(*component)))
                .collect(),
            mass.clone(),
        );
        if denominator.quadratic_form() != expected.quadratic_form() {
            return Err(ThreeLoopBoundaryError::WrongMomentumRouting);
        }
    }
    if family.symmetries().len() != 24 {
        return Err(ThreeLoopBoundaryError::IncompleteSymmetry {
            actual: family.symmetries().len(),
        });
    }
    Ok(())
}

fn induced_two_loop_family(family: &VacuumFamily) -> Result<VacuumFamily, ThreeLoopBoundaryError> {
    let coefficients = family.coefficients().clone();
    let mass = coefficients
        .parameter("m2")
        .ok_or(ThreeLoopBoundaryError::MissingParameter { name: "m2" })?;
    let routing =
        |left: i64, right: i64| vec![ExactRational::from(left), ExactRational::from(right)];
    let denominators = vec![
        Denominator::propagator(routing(1, 0), mass.clone()),
        Denominator::propagator(routing(0, 1), mass.clone()),
        Denominator::propagator(routing(1, 1), mass),
    ];
    Ok(VacuumFamily::new(
        "equal_mass_two_loop_paw_subgraph",
        2,
        coefficients,
        "d",
        denominators,
        vec![vec![1, 0, 2], vec![1, 2, 0]],
    )
    .map_err(TwoLoopPipelineError::from)?)
}
