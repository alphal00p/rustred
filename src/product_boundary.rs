//! Exact product-of-tadpoles closure for equal-mass vacuum sectors.
//!
//! If exactly `L` physical propagators of an `L`-loop family are active and
//! their momentum routings form a unimodular basis, an exact unit-Jacobian loop
//! change of variables turns the integral into `L` independent one-loop
//! tadpoles.  This module detects that situation and applies the one-loop IBP
//! recurrence for resource- and coefficient-representability-bounded positive
//! active powers.  Polynomial powers on inactive lines are rejected
//! explicitly: their angular factorization is a separate tensor problem, not
//! a scalar product formula.

use std::fmt;

use crate::coefficient::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use crate::exact::matrix_determinant;
use crate::{
    Coefficient, ExactRational, Integral, LinearCombination, PropagatorSign,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT, VacuumFamily,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProductBoundaryConfig {
    /// Maximum number of fixed-cardinality physical sectors inspected while
    /// choosing the stable product master.
    pub max_sector_candidates: usize,
    /// Maximum total number of one-loop recurrence steps in one input.
    /// Symbolica's hard coefficient-exponent limit remains in force when this
    /// configured work limit is larger.
    pub max_tadpole_steps: usize,
    /// Maximum number of terms accepted by [`ProductBoundaryReducer::reduce_combination`].
    pub max_combination_terms: usize,
    /// Maximum aggregate tadpole steps across one combination reduction.
    pub max_combination_tadpole_steps: usize,
}

impl Default for ProductBoundaryConfig {
    fn default() -> Self {
        Self {
            max_sector_candidates: 1_000_000,
            max_tadpole_steps: 1_000_000,
            max_combination_terms: 1_000_000,
            max_combination_tadpole_steps: 1_000_000,
        }
    }
}

/// Reusable exact reducer for unimodular product sectors of one family.
#[derive(Clone, Debug)]
pub struct ProductBoundaryReducer {
    family: VacuumFamily,
    config: ProductBoundaryConfig,
    mass: Coefficient,
    sign: PropagatorSign,
    product_master: Integral,
}

enum ProductClassification {
    Scaleless,
    NotProduct,
    Product {
        active_physical: Vec<usize>,
        tadpole_steps: u128,
    },
}

impl ProductBoundaryReducer {
    pub fn new(
        family: VacuumFamily,
        config: ProductBoundaryConfig,
    ) -> Result<Self, ProductBoundaryError> {
        let physical: Vec<_> = family
            .denominators()
            .iter()
            .enumerate()
            .filter(|(_, denominator)| denominator.is_propagator())
            .collect();
        let Some((_, first)) = physical.first().copied() else {
            return Err(ProductBoundaryError::NoPhysicalPropagators);
        };
        let sign = first
            .propagator_sign()
            .expect("a physical denominator has a sign");
        let sign_rational = ExactRational::from(i64::from(sign.normalization()));
        let mass = family
            .coefficients()
            .scale_rational(first.shift(), &sign_rational);
        if mass.is_zero() {
            return Err(ProductBoundaryError::MasslessFamily);
        }
        for (position, denominator) in physical.iter().copied() {
            if denominator.propagator_sign() != Some(sign) {
                return Err(ProductBoundaryError::MixedPropagatorSigns { position });
            }
            let normalized = family
                .coefficients()
                .scale_rational(denominator.shift(), &sign_rational);
            if normalized != mass {
                return Err(ProductBoundaryError::UnequalMasses { position });
            }
            if denominator
                .momentum()
                .is_none_or(|routing| routing.len() != family.loops())
            {
                return Err(ProductBoundaryError::WrongMomentumSize { position });
            }
        }

        let candidate_count = binomial_saturating(physical.len(), family.loops());
        if candidate_count > config.max_sector_candidates as u128 {
            return Err(ProductBoundaryError::ResourceLimit {
                resource: "unimodular sector candidates",
                requested: candidate_count,
                limit: config.max_sector_candidates as u128,
            });
        }
        let physical_positions = physical
            .iter()
            .map(|(position, _)| *position)
            .collect::<Vec<_>>();
        let product_master = FixedCombinations::new(physical_positions, family.loops())
            .filter_map(|positions| {
                let mut sector = vec![false; family.denominator_count()];
                for position in positions {
                    sector[position] = true;
                }
                sector_is_unimodular(&family, &sector).then_some(sector)
            })
            .filter_map(|sector| {
                let candidate =
                    Integral::new(sector.into_iter().map(i32::from).collect::<Vec<_>>());
                family.canonicalize(&candidate)
            })
            .max()
            .ok_or(ProductBoundaryError::NoUnimodularProductSector)?;

        Ok(Self {
            family,
            config,
            mass,
            sign,
            product_master,
        })
    }

    pub fn family(&self) -> &VacuumFamily {
        &self.family
    }

    pub fn config(&self) -> ProductBoundaryConfig {
        self.config
    }

    /// Stable family-local representative of `T1^L`.
    pub fn product_master(&self) -> &Integral {
        &self.product_master
    }

    /// Reduce a unimodular product sector, return a proved scaleless zero, or
    /// return `None` when the input is not a product sector.
    pub fn try_reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<Option<LinearCombination>, ProductBoundaryError> {
        Ok(match self.classify_integral(integral)? {
            ProductClassification::Scaleless => Some(LinearCombination::new()),
            ProductClassification::NotProduct => None,
            ProductClassification::Product {
                active_physical, ..
            } => Some(self.reduce_classified_product(integral, &active_physical)),
        })
    }

    fn reduce_classified_product(
        &self,
        integral: &Integral,
        active_physical: &[usize],
    ) -> LinearCombination {
        let mut coefficient = self.family.coefficients().one();
        let mut total_power = 0_i64;
        for position in active_physical {
            let power = integral.powers()[*position];
            total_power += i64::from(power);
            coefficient = &coefficient * &self.tadpole_ratio(power);
        }
        if self.sign == PropagatorSign::Negative
            && (total_power - i64::try_from(self.family.loops()).unwrap()).rem_euclid(2) != 0
        {
            coefficient = -coefficient;
        }
        LinearCombination::from_term(self.product_master.clone(), coefficient)
    }

    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ProductBoundaryError> {
        self.try_reduce_integral(integral)?
            .ok_or_else(|| ProductBoundaryError::NotProductSector {
                integral: integral.clone(),
            })
    }

    pub fn reduce_combination(
        &self,
        combination: &LinearCombination,
    ) -> Result<LinearCombination, ProductBoundaryError> {
        if combination.len() > self.config.max_combination_terms {
            return Err(ProductBoundaryError::ResourceLimit {
                resource: "input combination terms",
                requested: combination.len() as u128,
                limit: self.config.max_combination_terms as u128,
            });
        }
        let mut aggregate_steps = 0_u128;
        let mut classified = Vec::with_capacity(combination.len());
        for (integral, coefficient) in combination.terms() {
            let classification = self.classify_integral(integral)?;
            if let ProductClassification::Product { tadpole_steps, .. } = &classification {
                aggregate_steps = aggregate_steps.saturating_add(*tadpole_steps);
                if aggregate_steps > self.config.max_combination_tadpole_steps as u128 {
                    return Err(ProductBoundaryError::ResourceLimit {
                        resource: "combination tadpole recurrence steps",
                        requested: aggregate_steps,
                        limit: self.config.max_combination_tadpole_steps as u128,
                    });
                }
            }
            classified.push((integral, coefficient, classification));
        }

        // Only construct recurrence coefficients after the complete aggregate
        // work request is known to fit.  A late over-limit term must not make
        // the reducer perform expensive work for otherwise valid early terms.
        let mut output = LinearCombination::new();
        for (integral, coefficient, classification) in classified {
            match classification {
                ProductClassification::Scaleless => {}
                ProductClassification::NotProduct => {
                    output.add_term(integral.clone(), coefficient.clone());
                }
                ProductClassification::Product {
                    active_physical, ..
                } => {
                    let reduction = self.reduce_classified_product(integral, &active_physical);
                    self.add_scaled_checked(&mut output, &reduction, coefficient)?;
                }
            }
        }
        Ok(output)
    }

    fn classify_integral(
        &self,
        integral: &Integral,
    ) -> Result<ProductClassification, ProductBoundaryError> {
        self.validate_arity(integral)?;
        if self.family.is_scaleless(integral) {
            return Ok(ProductClassification::Scaleless);
        }

        let mut active_physical = Vec::new();
        let mut has_positive_auxiliary = false;
        for (position, (&power, denominator)) in integral
            .powers()
            .iter()
            .zip(self.family.denominators())
            .enumerate()
        {
            if power > 0 {
                if denominator.is_propagator() {
                    active_physical.push(position);
                } else {
                    has_positive_auxiliary = true;
                }
            }
        }
        if has_positive_auxiliary || active_physical.len() != self.family.loops() {
            return Ok(ProductClassification::NotProduct);
        }

        let sector: Vec<_> = integral.powers().iter().map(|power| *power > 0).collect();
        if !sector_is_unimodular(&self.family, &sector) {
            return Ok(ProductClassification::NotProduct);
        }
        if integral.powers().iter().any(|power| *power < 0) {
            return Err(ProductBoundaryError::UnsupportedNumerator {
                integral: integral.clone(),
            });
        }

        let tadpole_steps = active_physical.iter().fold(0_u128, |total, &position| {
            total.saturating_add(
                u128::try_from(integral.powers()[position] - 1)
                    .expect("an active power is positive"),
            )
        });
        if tadpole_steps > self.config.max_tadpole_steps as u128 {
            return Err(ProductBoundaryError::ResourceLimit {
                resource: "tadpole recurrence steps",
                requested: tadpole_steps,
                limit: self.config.max_tadpole_steps as u128,
            });
        }
        let coefficient_degree = self.tadpole_coefficient_degree_bound(tadpole_steps);
        self.check_coefficient_degree(coefficient_degree)?;
        Ok(ProductClassification::Product {
            active_physical,
            tadpole_steps,
        })
    }

    fn validate_arity(&self, integral: &Integral) -> Result<(), ProductBoundaryError> {
        if integral.powers().len() != self.family.denominator_count() {
            return Err(ProductBoundaryError::WrongIntegralArity {
                expected: self.family.denominator_count(),
                actual: integral.powers().len(),
            });
        }
        Ok(())
    }

    fn check_coefficient_degree(&self, requested: u128) -> Result<(), ProductBoundaryError> {
        if !symbolica_coefficient_degree_is_representable(requested) {
            return Err(ProductBoundaryError::ResourceLimit {
                resource: "Symbolica coefficient exponent degree",
                requested,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            });
        }
        Ok(())
    }

    /// Multiply and merge a classified reduction only after both Symbolica
    /// operations have passed conservative per-variable exponent preflights.
    fn add_scaled_checked(
        &self,
        output: &mut LinearCombination,
        reduction: &LinearCombination,
        factor: &Coefficient,
    ) -> Result<(), ProductBoundaryError> {
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

    /// Bound every per-variable exponent in
    /// `product_n (2*n-d)/(2*n*mass)` before constructing the first factor.
    ///
    /// For a rational `d=N_d/D_d` and `mass=N_m/D_m`, one recurrence factor
    /// has numerator degree at most
    /// `max(deg N_d, deg D_d) + deg D_m` and denominator degree at most
    /// `deg D_d + deg N_m`.  Multiplying all active tadpole ratios uses exactly
    /// `steps` such factors.
    fn tadpole_coefficient_degree_bound(&self, steps: u128) -> u128 {
        coefficient_variable_degrees(self.family.dimension())
            .into_iter()
            .zip(coefficient_variable_degrees(&self.mass))
            .map(
                |(
                    (dimension_numerator, dimension_denominator),
                    (mass_numerator, mass_denominator),
                )| {
                    let factor_numerator = dimension_numerator
                        .max(dimension_denominator)
                        .saturating_add(mass_denominator);
                    let factor_denominator = dimension_denominator.saturating_add(mass_numerator);
                    steps.saturating_mul(factor_numerator.max(factor_denominator))
                },
            )
            .max()
            .unwrap_or(0)
    }

    fn tadpole_ratio(&self, power: i32) -> Coefficient {
        debug_assert!(power > 0);
        let context = self.family.coefficients();
        let mut ratio = context.one();
        for n in 1..i64::from(power) {
            let two_n = context.integer(2 * n);
            ratio = &(&ratio * &(&two_n - self.family.dimension())) / &(&two_n * &self.mass);
        }
        ratio
    }
}

fn sector_is_unimodular(family: &VacuumFamily, sector: &[bool]) -> bool {
    if sector.len() != family.denominator_count() {
        return false;
    }
    let routings: Vec<Vec<ExactRational>> = sector
        .iter()
        .zip(family.denominators())
        .filter_map(|(active, denominator)| {
            if *active {
                denominator.momentum().map(ToOwned::to_owned)
            } else {
                None
            }
        })
        .collect();
    if routings.len() != family.loops() {
        return false;
    }
    let one = ExactRational::one();
    matches!(
        matrix_determinant(&routings),
        Ok(determinant) if determinant == one || determinant == -&one
    )
}

/// Lazy lexicographic fixed-cardinality combinations.  Product-master
/// discovery needs only `C(P,L)` sectors and must not materialize all `2^P`
/// physical masks merely to discard sectors of other cardinalities.
struct FixedCombinations {
    values: Vec<usize>,
    indices: Option<Vec<usize>>,
}

impl FixedCombinations {
    fn new(values: Vec<usize>, choose: usize) -> Self {
        let indices = (choose <= values.len()).then(|| (0..choose).collect());
        Self { values, indices }
    }
}

impl Iterator for FixedCombinations {
    type Item = Vec<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        let indices = self.indices.as_mut()?;
        let output = indices.iter().map(|&index| self.values[index]).collect();

        if indices.is_empty() {
            self.indices = None;
            return Some(output);
        }
        let choose = indices.len();
        let mut pivot = choose;
        while pivot > 0 {
            pivot -= 1;
            let maximum = self.values.len() - choose + pivot;
            if indices[pivot] < maximum {
                indices[pivot] += 1;
                for index in pivot + 1..choose {
                    indices[index] = indices[index - 1] + 1;
                }
                return Some(output);
            }
        }
        self.indices = None;
        Some(output)
    }
}

fn binomial_saturating(n: usize, k: usize) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n.saturating_sub(k));
    let mut result = 1_u128;
    for index in 0..k {
        let Some(product) = result.checked_mul((n - index) as u128) else {
            return u128::MAX;
        };
        result = product / (index + 1) as u128;
    }
    result
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProductBoundaryError {
    NoPhysicalPropagators,
    MasslessFamily,
    MixedPropagatorSigns {
        position: usize,
    },
    UnequalMasses {
        position: usize,
    },
    WrongMomentumSize {
        position: usize,
    },
    NoUnimodularProductSector,
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    UnsupportedNumerator {
        integral: Integral,
    },
    NotProductSector {
        integral: Integral,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
}

impl fmt::Display for ProductBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoPhysicalPropagators => {
                formatter.write_str("the family has no physical propagators")
            }
            Self::MasslessFamily => formatter.write_str("a massless tadpole product is scaleless"),
            Self::MixedPropagatorSigns { position } => write!(
                formatter,
                "physical propagator {position} has a different overall sign"
            ),
            Self::UnequalMasses { position } => write!(
                formatter,
                "physical propagator {position} has a different normalized mass"
            ),
            Self::WrongMomentumSize { position } => write!(
                formatter,
                "physical propagator {position} has a routing with the wrong loop dimension"
            ),
            Self::NoUnimodularProductSector => formatter
                .write_str("the family has no L-line sector forming a unit-Jacobian loop basis"),
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "product-boundary integral has {actual} powers, expected {expected}"
            ),
            Self::UnsupportedNumerator { integral } => write!(
                formatter,
                "{integral} has an inactive polynomial numerator requiring tensor factorization"
            ),
            Self::NotProductSector { integral } => {
                write!(formatter, "{integral} is not a unimodular product sector")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "product boundary {resource} requires {requested}, exceeding limit {limit}"
            ),
        }
    }
}

impl std::error::Error for ProductBoundaryError {}
