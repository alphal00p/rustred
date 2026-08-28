//! Exact scalar one-dot closure for factorized four-loop H-family sectors.
//!
//! The native 160-row corner shell reaches a finite lower boundary containing
//! only factorized scalar integrals with one physical dot and no numerator.
//! Their factorization witnesses identify the dotted lower-loop component and
//! its compact reference-line position.  Six fixed, independently certified
//! component formulae then close the term into the existing product basis.
//!
//! This module deliberately does not construct a lower-loop reduction table
//! and does not advertise numerator support.  Inputs outside the exact
//! `D=1,N=0` H-family box are typed errors.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::legacy_oracle_support::coefficient_degree::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use crate::{
    Coefficient, CoefficientContext, FourLoopBoundaryConfig, FourLoopBoundaryError,
    FourLoopBoundaryReducer, FourLoopFactorizationWitness, FourLoopTopology, Integral,
    MassiveVacuumMaster, MasterProduct, MasterProductError, ProductLinearCombination,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

const DENOMINATORS: usize = 10;

/// Frozen batch bounds of the factorized scalar halo in the native 160 rows.
pub const FOUR_LOOP_BOUNDARY_HALO_BLOCKER_OCCURRENCES: usize = 234;
pub const FOUR_LOOP_BOUNDARY_HALO_UNIQUE_WITNESS_PLANS: usize = 28;
pub const FOUR_LOOP_BOUNDARY_HALO_SIGNED_LINE_DISPATCHES: usize = 150;
pub const FOUR_LOOP_BOUNDARY_HALO_FORMULA_DISPATCHES: usize = 234;
pub const FOUR_LOOP_BOUNDARY_HALO_PRODUCT_MULTIPLICATIONS: usize = 420;
pub const FOUR_LOOP_BOUNDARY_HALO_PRECOLLECTION_TERMS: usize = 420;
pub const FOUR_LOOP_BOUNDARY_HALO_OUTPUT_PRODUCTS: usize = 6;

/// Aggregate and per-request resource limits for exact boundary-halo closure.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopBoundaryHaloConfig {
    pub max_blocker_occurrences: usize,
    pub max_unique_witness_plans: usize,
    pub max_signed_line_dispatches: usize,
    pub max_formula_dispatches: usize,
    pub max_product_multiplications: usize,
    pub max_precollection_terms: usize,
    pub max_output_products: usize,
    /// Conservative per-variable Symbolica exponent ceiling.
    pub max_coefficient_degree: usize,
}

impl Default for FourLoopBoundaryHaloConfig {
    fn default() -> Self {
        Self {
            max_blocker_occurrences: FOUR_LOOP_BOUNDARY_HALO_BLOCKER_OCCURRENCES,
            max_unique_witness_plans: FOUR_LOOP_BOUNDARY_HALO_UNIQUE_WITNESS_PLANS,
            max_signed_line_dispatches: FOUR_LOOP_BOUNDARY_HALO_SIGNED_LINE_DISPATCHES,
            max_formula_dispatches: FOUR_LOOP_BOUNDARY_HALO_FORMULA_DISPATCHES,
            max_product_multiplications: FOUR_LOOP_BOUNDARY_HALO_PRODUCT_MULTIPLICATIONS,
            max_precollection_terms: FOUR_LOOP_BOUNDARY_HALO_PRECOLLECTION_TERMS,
            max_output_products: FOUR_LOOP_BOUNDARY_HALO_OUTPUT_PRODUCTS,
            max_coefficient_degree: 4_096,
        }
    }
}

/// Work charged by one request or an aggregate shell batch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopBoundaryHaloStats {
    blocker_occurrences: usize,
    unique_witness_plans: usize,
    signed_line_dispatches: usize,
    formula_dispatches: usize,
    product_multiplications: usize,
    precollection_terms: usize,
    output_products: usize,
}

impl FourLoopBoundaryHaloStats {
    pub const fn new(
        blocker_occurrences: usize,
        unique_witness_plans: usize,
        signed_line_dispatches: usize,
        formula_dispatches: usize,
        product_multiplications: usize,
        precollection_terms: usize,
        output_products: usize,
    ) -> Self {
        Self {
            blocker_occurrences,
            unique_witness_plans,
            signed_line_dispatches,
            formula_dispatches,
            product_multiplications,
            precollection_terms,
            output_products,
        }
    }

    pub const fn blocker_occurrences(self) -> usize {
        self.blocker_occurrences
    }

    pub const fn unique_witness_plans(self) -> usize {
        self.unique_witness_plans
    }

    pub const fn signed_line_dispatches(self) -> usize {
        self.signed_line_dispatches
    }

    pub const fn formula_dispatches(self) -> usize {
        self.formula_dispatches
    }

    pub const fn product_multiplications(self) -> usize {
        self.product_multiplications
    }

    pub const fn precollection_terms(self) -> usize {
        self.precollection_terms
    }

    pub const fn output_products(self) -> usize {
        self.output_products
    }

    pub(crate) fn add_request(&mut self, request: Self) -> Result<(), FourLoopBoundaryHaloError> {
        self.formula_dispatches = checked_stat_add(
            "formula dispatches",
            self.formula_dispatches,
            request.formula_dispatches,
        )?;
        self.product_multiplications = checked_stat_add(
            "product multiplications",
            self.product_multiplications,
            request.product_multiplications,
        )?;
        self.precollection_terms = checked_stat_add(
            "precollection terms",
            self.precollection_terms,
            request.precollection_terms,
        )?;
        self.output_products = self.output_products.max(request.output_products);
        Ok(())
    }

    pub(crate) fn set_batch_shape(
        &mut self,
        blocker_occurrences: usize,
        unique_witness_plans: usize,
        signed_line_dispatches: usize,
        output_products: usize,
    ) {
        self.blocker_occurrences = blocker_occurrences;
        self.unique_witness_plans = unique_witness_plans;
        self.signed_line_dispatches = signed_line_dispatches;
        self.output_products = output_products;
    }

    pub(crate) fn conservative_batch(
        blocker_occurrences: usize,
        unique_witness_plans: usize,
        signed_line_dispatches: usize,
        product_multiplications: usize,
        output_products: usize,
    ) -> Self {
        Self {
            blocker_occurrences,
            unique_witness_plans,
            signed_line_dispatches,
            formula_dispatches: blocker_occurrences,
            product_multiplications,
            precollection_terms: product_multiplications,
            output_products,
        }
    }
}

/// Exact ordinary and mass-normalized forms of one closed halo integral.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopBoundaryHaloReduction {
    dotted_component: MassiveVacuumMaster,
    compact_reference_position: usize,
    ordinary: ProductLinearCombination<MassiveVacuumMaster>,
    mass_normalized: ProductLinearCombination<MassiveVacuumMaster>,
    stats: FourLoopBoundaryHaloStats,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FourLoopBoundaryHaloDispatch {
    component: MassiveVacuumMaster,
    compact_reference_position: usize,
    unaffected_product: MasterProduct<MassiveVacuumMaster>,
}

/// Authenticated, reusable interpretation of one exact factorization witness.
///
/// The shell builds one plan for each distinct sector witness, so exact matrix
/// replay and signed-line authentication are charged once rather than once per
/// blocker occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopBoundaryHaloPlan {
    product: MasterProduct<MassiveVacuumMaster>,
    witness: FourLoopFactorizationWitness,
    dispatches: BTreeMap<usize, FourLoopBoundaryHaloDispatch>,
}

impl FourLoopBoundaryHaloPlan {
    pub const fn product(&self) -> &MasterProduct<MassiveVacuumMaster> {
        &self.product
    }

    pub const fn witness(&self) -> &FourLoopFactorizationWitness {
        &self.witness
    }

    pub fn signed_line_dispatch_count(&self) -> usize {
        self.dispatches.len()
    }
}

impl FourLoopBoundaryHaloReduction {
    pub const fn dotted_component(&self) -> MassiveVacuumMaster {
        self.dotted_component
    }

    pub const fn compact_reference_position(&self) -> usize {
        self.compact_reference_position
    }

    /// Coefficients `r_P(d,m2)` in `I(a) = sum_P r_P P`.
    pub const fn ordinary(&self) -> &ProductLinearCombination<MassiveVacuumMaster> {
        &self.ordinary
    }

    /// Dimensionless coefficients
    /// `r_P * m2^(sum(a)-sum_P physical_lines(P))`.
    pub const fn mass_normalized(&self) -> &ProductLinearCombination<MassiveVacuumMaster> {
        &self.mass_normalized
    }

    pub const fn stats(&self) -> FourLoopBoundaryHaloStats {
        self.stats
    }
}

/// Pure-Rust direct-formula reducer for the proved factorized scalar halo.
#[derive(Clone, Debug)]
pub struct FourLoopBoundaryHaloReducer {
    boundary: FourLoopBoundaryReducer,
    config: FourLoopBoundaryHaloConfig,
    coefficients: CoefficientContext,
    dimension: Coefficient,
    mass: Coefficient,
    mass_position: usize,
}

impl FourLoopBoundaryHaloReducer {
    /// Preflight the fixed formula table before any witness authentication.
    /// The F5 rows require per-variable coefficient degree two.
    pub fn preflight_formula_table(&self) -> Result<(), FourLoopBoundaryHaloError> {
        self.check_degree(2)
    }

    /// Build the built-in H-family service in a fresh `Q(d,m2)` context.
    pub fn build(config: FourLoopBoundaryHaloConfig) -> Result<Self, FourLoopBoundaryHaloError> {
        Self::new(
            FourLoopBoundaryReducer::build(FourLoopTopology::H, FourLoopBoundaryConfig::default())?,
            config,
        )
    }

    /// Authenticate and reuse a boundary reducer, preserving its coefficient
    /// context.  The current certified domain is deliberately H-only.
    pub fn new(
        boundary: FourLoopBoundaryReducer,
        config: FourLoopBoundaryHaloConfig,
    ) -> Result<Self, FourLoopBoundaryHaloError> {
        if boundary.topology() != FourLoopTopology::H {
            return Err(FourLoopBoundaryHaloError::WrongTopology {
                expected: FourLoopTopology::H,
                actual: boundary.topology(),
            });
        }
        if config.max_coefficient_degree as u128 > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
            return Err(FourLoopBoundaryHaloError::ResourceLimit {
                resource: "configured coefficient exponent degree",
                requested: config.max_coefficient_degree as u128,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            });
        }
        let coefficients = boundary.family().coefficients().clone();
        let dimension = coefficients
            .parameter("d")
            .ok_or(FourLoopBoundaryHaloError::MissingParameter { name: "d" })?;
        let mass_position = coefficients
            .parameter_names()
            .iter()
            .position(|name| name == "m2")
            .ok_or(FourLoopBoundaryHaloError::MissingParameter { name: "m2" })?;
        let mass = coefficients
            .parameter("m2")
            .ok_or(FourLoopBoundaryHaloError::MissingParameter { name: "m2" })?;
        Ok(Self {
            boundary,
            config,
            coefficients,
            dimension,
            mass,
            mass_position,
        })
    }

    pub fn boundary(&self) -> &FourLoopBoundaryReducer {
        &self.boundary
    }

    pub const fn config(&self) -> FourLoopBoundaryHaloConfig {
        self.config
    }

    /// Preflight a complete aggregate request before witness or coefficient
    /// work.  The shell uses a conservative three-term reservation for every
    /// `T1*F5` blocker.
    pub fn preflight_stats(
        &self,
        requested: FourLoopBoundaryHaloStats,
    ) -> Result<(), FourLoopBoundaryHaloError> {
        for (resource, request, limit) in [
            (
                "blocker occurrences",
                requested.blocker_occurrences,
                self.config.max_blocker_occurrences,
            ),
            (
                "unique witness plans",
                requested.unique_witness_plans,
                self.config.max_unique_witness_plans,
            ),
            (
                "signed-line dispatches",
                requested.signed_line_dispatches,
                self.config.max_signed_line_dispatches,
            ),
            (
                "formula dispatches",
                requested.formula_dispatches,
                self.config.max_formula_dispatches,
            ),
            (
                "product multiplications",
                requested.product_multiplications,
                self.config.max_product_multiplications,
            ),
            (
                "precollection terms",
                requested.precollection_terms,
                self.config.max_precollection_terms,
            ),
            (
                "output products",
                requested.output_products,
                self.config.max_output_products,
            ),
        ] {
            check_resource(resource, request as u128, limit as u128)?;
        }
        Ok(())
    }

    /// Replay and compile one exact factorization witness into a reusable
    /// signed-line dispatch plan.  Resource checks precede witness replay.
    pub fn prepare_plan(
        &self,
        advertised_product: &MasterProduct<MassiveVacuumMaster>,
        witness: &FourLoopFactorizationWitness,
    ) -> Result<FourLoopBoundaryHaloPlan, FourLoopBoundaryHaloError> {
        if witness.topology() != FourLoopTopology::H {
            return Err(FourLoopBoundaryHaloError::WrongTopology {
                expected: FourLoopTopology::H,
                actual: witness.topology(),
            });
        }
        if !is_allowed_product(advertised_product) {
            return Err(FourLoopBoundaryHaloError::ProductOutsideClosure {
                product: advertised_product.clone(),
            });
        }
        let requested = FourLoopBoundaryHaloStats {
            blocker_occurrences: 0,
            unique_witness_plans: 1,
            signed_line_dispatches: witness.sector_mask().count_ones() as usize,
            formula_dispatches: 0,
            product_multiplications: 0,
            precollection_terms: 0,
            output_products: 0,
        };
        self.preflight_stats(requested)?;
        let witness_product = witness.product()?;
        if &witness_product != advertised_product {
            return Err(FourLoopBoundaryHaloError::ProductMismatch {
                expected: witness_product,
                actual: advertised_product.clone(),
            });
        }
        let replayed = self.boundary.replay_witness(witness)?;
        if replayed != *advertised_product {
            return Err(FourLoopBoundaryHaloError::ProductMismatch {
                expected: replayed,
                actual: advertised_product.clone(),
            });
        }

        let mut dispatches = BTreeMap::new();
        for (component_index, component) in witness.components().iter().enumerate() {
            let unaffected_product = MasterProduct::try_from_factors(
                witness
                    .components()
                    .iter()
                    .enumerate()
                    .filter_map(|(index, other)| {
                        (index != component_index).then_some(other.master())
                    }),
            )?;
            for line_match in component.signed_line_matches() {
                validate_compact_position(component.master(), line_match.reference_position())?;
                let dispatch = FourLoopBoundaryHaloDispatch {
                    component: component.master(),
                    compact_reference_position: line_match.reference_position(),
                    unaffected_product: unaffected_product.clone(),
                };
                if dispatches
                    .insert(line_match.physical_position(), dispatch)
                    .is_some()
                {
                    return Err(FourLoopBoundaryHaloError::AmbiguousDottedLine {
                        physical_position: line_match.physical_position(),
                    });
                }
            }
        }
        if dispatches.len() != witness.sector_mask().count_ones() as usize
            || dispatches
                .keys()
                .any(|position| witness.sector_mask() & (1_u16 << position) == 0)
        {
            return Err(FourLoopBoundaryHaloError::IncompleteSignedLinePlan {
                sector_mask: witness.sector_mask(),
                dispatches: dispatches.len(),
            });
        }
        Ok(FourLoopBoundaryHaloPlan {
            product: advertised_product.clone(),
            witness: witness.clone(),
            dispatches,
        })
    }

    /// Close one exact H-family factorized `D1/N0` integral.
    pub fn reduce_integral(
        &self,
        integral: &Integral,
        advertised_product: &MasterProduct<MassiveVacuumMaster>,
        witness: &FourLoopFactorizationWitness,
    ) -> Result<FourLoopBoundaryHaloReduction, FourLoopBoundaryHaloError> {
        self.preflight_formula_table()?;
        let plan = self.prepare_plan(advertised_product, witness)?;
        let mut reduction = self.reduce_with_plan(integral, &plan)?;
        reduction.stats.unique_witness_plans = 1;
        reduction.stats.signed_line_dispatches = plan.signed_line_dispatch_count();
        Ok(reduction)
    }

    /// Close one occurrence using an already authenticated witness plan.
    pub fn reduce_with_plan(
        &self,
        integral: &Integral,
        plan: &FourLoopBoundaryHaloPlan,
    ) -> Result<FourLoopBoundaryHaloReduction, FourLoopBoundaryHaloError> {
        let dot_position = self.validate_domain(integral, &plan.witness)?;
        let dispatch = plan.dispatches.get(&dot_position).ok_or(
            FourLoopBoundaryHaloError::UnmatchedDottedLine {
                physical_position: dot_position,
            },
        )?;
        let dotted_component = dispatch.component;
        let compact_reference_position = dispatch.compact_reference_position;
        let formula_terms = formula_term_count(dotted_component, compact_reference_position)?;
        let request_stats = FourLoopBoundaryHaloStats {
            blocker_occurrences: 1,
            unique_witness_plans: 0,
            signed_line_dispatches: 0,
            formula_dispatches: 1,
            product_multiplications: formula_terms,
            precollection_terms: formula_terms,
            output_products: formula_terms,
        };
        self.preflight_stats(request_stats)?;
        self.check_degree(formula_mass_degree(
            dotted_component,
            compact_reference_position,
        )?)?;
        let local = self.component_formula(dotted_component, compact_reference_position)?;
        if local.len() != formula_terms {
            return Err(FourLoopBoundaryHaloError::FormulaInvariant {
                master: dotted_component,
                compact_reference_position,
            });
        }
        let mut ordinary_terms = BTreeMap::new();
        for (local_product, coefficient) in local {
            let product = dispatch
                .unaffected_product
                .checked_multiply(&local_product)?;
            if !is_allowed_product(&product) {
                return Err(FourLoopBoundaryHaloError::ProductOutsideClosure { product });
            }
            add_checked(self, &mut ordinary_terms, product, coefficient)?;
        }
        check_resource(
            "output products",
            ordinary_terms.len() as u128,
            self.config.max_output_products as u128,
        )?;
        let ordinary = product_combination_from_map(ordinary_terms.clone());

        let integral_weight = integral
            .powers()
            .iter()
            .map(|&power| i64::from(power))
            .sum::<i64>();
        let mut normalized_terms = BTreeMap::new();
        for (product, coefficient) in ordinary_terms {
            let exponent = integral_weight - product_mass_weight(&product);
            let normalized = self.multiply_mass_power(&coefficient, exponent)?;
            let degrees = coefficient_variable_degrees(&normalized);
            let (numerator_degree, denominator_degree) =
                degrees
                    .get(self.mass_position)
                    .copied()
                    .ok_or(FourLoopBoundaryHaloError::MissingParameter { name: "m2" })?;
            if numerator_degree != 0 || denominator_degree != 0 {
                return Err(FourLoopBoundaryHaloError::ResidualMassDependence {
                    product,
                    numerator_degree,
                    denominator_degree,
                });
            }
            add_checked(self, &mut normalized_terms, product, normalized)?;
        }
        let mass_normalized = product_combination_from_map(normalized_terms);

        Ok(FourLoopBoundaryHaloReduction {
            dotted_component,
            compact_reference_position,
            ordinary,
            mass_normalized,
            stats: request_stats,
        })
    }

    fn validate_domain(
        &self,
        integral: &Integral,
        witness: &FourLoopFactorizationWitness,
    ) -> Result<usize, FourLoopBoundaryHaloError> {
        if integral.powers().len() != DENOMINATORS {
            return Err(FourLoopBoundaryHaloError::WrongIntegralArity {
                expected: DENOMINATORS,
                actual: integral.powers().len(),
            });
        }
        if witness.topology() != FourLoopTopology::H {
            return Err(FourLoopBoundaryHaloError::WrongTopology {
                expected: FourLoopTopology::H,
                actual: witness.topology(),
            });
        }
        let actual_sector = integral
            .powers()
            .iter()
            .zip(self.boundary.family().denominators())
            .enumerate()
            .filter_map(|(position, (&power, denominator))| {
                (power > 0 && denominator.is_propagator()).then_some(position)
            })
            .fold(0_u16, |mask, position| mask | (1_u16 << position));
        if actual_sector != witness.sector_mask() {
            return Err(FourLoopBoundaryHaloError::SectorMismatch {
                expected: witness.sector_mask(),
                actual: actual_sector,
            });
        }

        let mut dots = 0_u128;
        let mut dotted_position = None;
        for (position, (&power, denominator)) in integral
            .powers()
            .iter()
            .zip(self.boundary.family().denominators())
            .enumerate()
        {
            let active = witness.sector_mask() & (1_u16 << position) != 0;
            if !denominator.is_propagator() || !active {
                if power != 0 {
                    return Err(FourLoopBoundaryHaloError::UnexpectedPower {
                        position,
                        power,
                        expected: "zero on every inactive or auxiliary denominator",
                    });
                }
                continue;
            }
            if power < 1 {
                return Err(FourLoopBoundaryHaloError::UnexpectedPower {
                    position,
                    power,
                    expected: "one or two on every active physical denominator",
                });
            }
            let local_dots = u128::from(power.saturating_sub(1) as u32);
            dots = dots.saturating_add(local_dots);
            if power == 2 {
                dotted_position = Some(position);
            }
        }
        if dots != 1 {
            return Err(FourLoopBoundaryHaloError::OutsideD1N0 {
                dots,
                numerator_degree: 0,
            });
        }
        dotted_position.ok_or(FourLoopBoundaryHaloError::OutsideD1N0 {
            dots,
            numerator_degree: 0,
        })
    }

    fn component_formula(
        &self,
        master: MassiveVacuumMaster,
        position: usize,
    ) -> Result<Vec<(MasterProduct<MassiveVacuumMaster>, Coefficient)>, FourLoopBoundaryHaloError>
    {
        validate_compact_position(master, position)?;
        let factor = |master| MasterProduct::from_factor(master);
        let product = |masters: &[MassiveVacuumMaster]| {
            MasterProduct::try_from_factors(masters.iter().copied())
        };
        Ok(match (master, position) {
            (MassiveVacuumMaster::T1, 0) => {
                vec![(factor(MassiveVacuumMaster::T1), self.ratio(2, -1, 2, 1)?)]
            }
            (MassiveVacuumMaster::S2, 0..=2) => {
                vec![(factor(MassiveVacuumMaster::S2), self.ratio(3, -1, 3, 1)?)]
            }
            (MassiveVacuumMaster::B4, 0..=3) => {
                vec![(factor(MassiveVacuumMaster::B4), self.ratio(8, -3, 8, 1)?)]
            }
            (MassiveVacuumMaster::F5, 0) => vec![
                (factor(MassiveVacuumMaster::B4), self.ratio(8, -3, 6, 2)?),
                (
                    product(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::S2])?,
                    self.ratio(-4, 2, 3, 2)?,
                ),
                (factor(MassiveVacuumMaster::F5), self.ratio(6, -1, 6, 1)?),
            ],
            (MassiveVacuumMaster::F5, 1..=4) => vec![
                (factor(MassiveVacuumMaster::B4), self.ratio(-8, 3, 24, 2)?),
                (
                    product(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::S2])?,
                    self.ratio(2, -1, 6, 2)?,
                ),
                (factor(MassiveVacuumMaster::F5), self.ratio(3, -1, 3, 1)?),
            ],
            (MassiveVacuumMaster::M6, 0..=5) => {
                vec![(factor(MassiveVacuumMaster::M6), self.ratio(4, -1, 4, 1)?)]
            }
            _ => {
                return Err(FourLoopBoundaryHaloError::InvalidCompactReferencePosition {
                    master,
                    position,
                });
            }
        })
    }

    /// Construct `(constant + d_factor*d)/(denominator*m2^mass_power)`.
    fn ratio(
        &self,
        constant: i64,
        dimension_factor: i64,
        denominator: i64,
        mass_power: usize,
    ) -> Result<Coefficient, FourLoopBoundaryHaloError> {
        let scaled_dimension = self.checked_mul(
            &self.coefficients.integer(dimension_factor),
            &self.dimension,
        )?;
        let numerator =
            self.checked_add(&self.coefficients.integer(constant), &scaled_dimension)?;
        let mut divisor = self.coefficients.integer(denominator);
        for _ in 0..mass_power {
            divisor = self.checked_mul(&divisor, &self.mass)?;
        }
        self.checked_div(&numerator, &divisor)
    }

    fn multiply_mass_power(
        &self,
        coefficient: &Coefficient,
        exponent: i64,
    ) -> Result<Coefficient, FourLoopBoundaryHaloError> {
        let mut output = coefficient.clone();
        if exponent >= 0 {
            for _ in 0..u64::try_from(exponent).expect("a nonnegative i64 fits u64") {
                output = self.checked_mul(&output, &self.mass)?;
            }
        } else {
            for _ in 0..exponent.unsigned_abs() {
                output = self.checked_div(&output, &self.mass)?;
            }
        }
        Ok(output)
    }

    fn check_degree(&self, requested: u128) -> Result<(), FourLoopBoundaryHaloError> {
        if !symbolica_coefficient_degree_is_representable(requested) {
            return Err(FourLoopBoundaryHaloError::ResourceLimit {
                resource: "Symbolica coefficient exponent degree",
                requested,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            });
        }
        check_resource(
            "configured coefficient exponent degree",
            requested,
            self.config.max_coefficient_degree as u128,
        )
    }

    fn checked_mul(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopBoundaryHaloError> {
        self.check_degree(coefficient_product_degree_bound(left, right))?;
        Ok(left * right)
    }

    fn checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopBoundaryHaloError> {
        self.check_degree(coefficient_sum_degree_bound(left, right))?;
        Ok(left + right)
    }

    fn checked_div(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopBoundaryHaloError> {
        if right.is_zero() {
            return Err(FourLoopBoundaryHaloError::ZeroFormulaDivisor);
        }
        self.check_degree(coefficient_quotient_degree_bound(left, right))?;
        Ok(left / right)
    }
}

fn formula_term_count(
    master: MassiveVacuumMaster,
    position: usize,
) -> Result<usize, FourLoopBoundaryHaloError> {
    validate_compact_position(master, position)?;
    Ok(if master == MassiveVacuumMaster::F5 {
        3
    } else {
        1
    })
}

fn formula_mass_degree(
    master: MassiveVacuumMaster,
    position: usize,
) -> Result<u128, FourLoopBoundaryHaloError> {
    validate_compact_position(master, position)?;
    Ok(if master == MassiveVacuumMaster::F5 {
        2
    } else {
        1
    })
}

fn validate_compact_position(
    master: MassiveVacuumMaster,
    position: usize,
) -> Result<(), FourLoopBoundaryHaloError> {
    let valid = match master {
        MassiveVacuumMaster::T1 => position == 0,
        MassiveVacuumMaster::S2 => position < 3,
        // These are compact component positions.  B4 positions 0,1,2,3
        // lift to tetrahedron-family positions 0,1,3,5 in oracle tests.
        MassiveVacuumMaster::B4 => position < 4,
        MassiveVacuumMaster::F5 => position < 5,
        MassiveVacuumMaster::M6 => position < 6,
    };
    if valid {
        Ok(())
    } else {
        Err(FourLoopBoundaryHaloError::InvalidCompactReferencePosition { master, position })
    }
}

fn is_allowed_product(product: &MasterProduct<MassiveVacuumMaster>) -> bool {
    let multiplicity = |master| product.multiplicity(&master);
    let only = |allowed: &[MassiveVacuumMaster]| {
        product
            .factors()
            .keys()
            .all(|master| allowed.contains(master))
    };
    (only(&[MassiveVacuumMaster::T1]) && multiplicity(MassiveVacuumMaster::T1) == 4)
        || (only(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::S2])
            && multiplicity(MassiveVacuumMaster::T1) == 2
            && multiplicity(MassiveVacuumMaster::S2) == 1)
        || (only(&[MassiveVacuumMaster::S2]) && multiplicity(MassiveVacuumMaster::S2) == 2)
        || [
            MassiveVacuumMaster::B4,
            MassiveVacuumMaster::F5,
            MassiveVacuumMaster::M6,
        ]
        .into_iter()
        .any(|three_loop| {
            only(&[MassiveVacuumMaster::T1, three_loop])
                && multiplicity(MassiveVacuumMaster::T1) == 1
                && multiplicity(three_loop) == 1
        })
}

fn product_mass_weight(product: &MasterProduct<MassiveVacuumMaster>) -> i64 {
    product
        .factors()
        .iter()
        .map(|(master, multiplicity)| {
            i64::from(*multiplicity)
                * i64::try_from(master.physical_lines()).expect("small master line count")
        })
        .sum()
}

fn product_combination_from_map(
    terms: BTreeMap<MasterProduct<MassiveVacuumMaster>, Coefficient>,
) -> ProductLinearCombination<MassiveVacuumMaster> {
    let mut output = ProductLinearCombination::new();
    for (product, coefficient) in terms {
        output.add_term(product, coefficient);
    }
    output
}

fn add_checked(
    reducer: &FourLoopBoundaryHaloReducer,
    terms: &mut BTreeMap<MasterProduct<MassiveVacuumMaster>, Coefficient>,
    product: MasterProduct<MassiveVacuumMaster>,
    coefficient: Coefficient,
) -> Result<(), FourLoopBoundaryHaloError> {
    if coefficient.is_zero() {
        return Ok(());
    }
    if let Some(current) = terms.get_mut(&product) {
        let sum = reducer.checked_add(current, &coefficient)?;
        if sum.is_zero() {
            terms.remove(&product);
        } else {
            *current = sum;
        }
    } else {
        terms.insert(product, coefficient);
    }
    Ok(())
}

fn coefficient_quotient_degree_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    if left.get_variables() != right.get_variables() {
        return u128::MAX;
    }
    coefficient_variable_degrees(left)
        .into_iter()
        .zip(coefficient_variable_degrees(right))
        .map(
            |((left_numerator, left_denominator), (right_numerator, right_denominator))| {
                left_numerator
                    .saturating_add(right_denominator)
                    .max(left_denominator.saturating_add(right_numerator))
            },
        )
        .max()
        .unwrap_or(0)
}

fn check_resource(
    resource: &'static str,
    requested: u128,
    limit: u128,
) -> Result<(), FourLoopBoundaryHaloError> {
    if requested > limit {
        Err(FourLoopBoundaryHaloError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

fn checked_stat_add(
    resource: &'static str,
    left: usize,
    right: usize,
) -> Result<usize, FourLoopBoundaryHaloError> {
    left.checked_add(right)
        .ok_or(FourLoopBoundaryHaloError::ResourceLimit {
            resource,
            requested: u128::MAX,
            limit: usize::MAX as u128,
        })
}

/// Typed domain, witness, product, resource, and homogeneity failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FourLoopBoundaryHaloError {
    WrongTopology {
        expected: FourLoopTopology,
        actual: FourLoopTopology,
    },
    MissingParameter {
        name: &'static str,
    },
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    SectorMismatch {
        expected: u16,
        actual: u16,
    },
    UnexpectedPower {
        position: usize,
        power: i32,
        expected: &'static str,
    },
    OutsideD1N0 {
        dots: u128,
        numerator_degree: u128,
    },
    ProductMismatch {
        expected: MasterProduct<MassiveVacuumMaster>,
        actual: MasterProduct<MassiveVacuumMaster>,
    },
    ProductOutsideClosure {
        product: MasterProduct<MassiveVacuumMaster>,
    },
    UnmatchedDottedLine {
        physical_position: usize,
    },
    AmbiguousDottedLine {
        physical_position: usize,
    },
    IncompleteSignedLinePlan {
        sector_mask: u16,
        dispatches: usize,
    },
    InvalidCompactReferencePosition {
        master: MassiveVacuumMaster,
        position: usize,
    },
    FormulaInvariant {
        master: MassiveVacuumMaster,
        compact_reference_position: usize,
    },
    ResidualMassDependence {
        product: MasterProduct<MassiveVacuumMaster>,
        numerator_degree: u128,
        denominator_degree: u128,
    },
    ZeroFormulaDivisor,
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    Boundary(FourLoopBoundaryError),
    MasterProduct(MasterProductError),
}

impl From<FourLoopBoundaryError> for FourLoopBoundaryHaloError {
    fn from(error: FourLoopBoundaryError) -> Self {
        Self::Boundary(error)
    }
}

impl From<MasterProductError> for FourLoopBoundaryHaloError {
    fn from(error: MasterProductError) -> Self {
        Self::MasterProduct(error)
    }
}

impl fmt::Display for FourLoopBoundaryHaloError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongTopology { expected, actual } => write!(
                formatter,
                "factorized boundary-halo closure requires {expected:?}, received {actual:?}"
            ),
            Self::MissingParameter { name } => {
                write!(
                    formatter,
                    "factorized boundary-halo context is missing {name}"
                )
            }
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "factorized boundary-halo integral has {actual} powers; expected {expected}"
            ),
            Self::SectorMismatch { expected, actual } => write!(
                formatter,
                "factorized boundary-halo sector {actual:#x} does not match witness {expected:#x}"
            ),
            Self::UnexpectedPower {
                position,
                power,
                expected,
            } => write!(
                formatter,
                "factorized boundary-halo power {position} is {power}; expected {expected}"
            ),
            Self::OutsideD1N0 {
                dots,
                numerator_degree,
            } => write!(
                formatter,
                "factorized boundary halo is outside D1/N0 (D={dots}, N={numerator_degree})"
            ),
            Self::ProductMismatch { expected, actual } => write!(
                formatter,
                "factorized boundary-halo product {actual} does not match witness {expected}"
            ),
            Self::ProductOutsideClosure { product } => write!(
                formatter,
                "factorized boundary-halo product {product} is outside the six-key closure"
            ),
            Self::UnmatchedDottedLine { physical_position } => write!(
                formatter,
                "dotted parent line {physical_position} has no signed witness match"
            ),
            Self::AmbiguousDottedLine { physical_position } => write!(
                formatter,
                "dotted parent line {physical_position} has multiple signed witness matches"
            ),
            Self::IncompleteSignedLinePlan {
                sector_mask,
                dispatches,
            } => write!(
                formatter,
                "factorized sector {sector_mask:#x} has {dispatches} signed-line dispatches"
            ),
            Self::InvalidCompactReferencePosition { master, position } => write!(
                formatter,
                "compact reference position {position} is invalid for {master}"
            ),
            Self::FormulaInvariant {
                master,
                compact_reference_position,
            } => write!(
                formatter,
                "fixed {master} formula at compact position {compact_reference_position} violated its term-count invariant"
            ),
            Self::ResidualMassDependence {
                product,
                numerator_degree,
                denominator_degree,
            } => write!(
                formatter,
                "mass-normalized {product} coefficient retains m2 degrees ({numerator_degree},{denominator_degree})"
            ),
            Self::ZeroFormulaDivisor => {
                formatter.write_str("factorized boundary-halo formula attempted division by zero")
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "factorized boundary-halo {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::Boundary(error) => write!(formatter, "factorized boundary witness: {error}"),
            Self::MasterProduct(error) => error.fmt(formatter),
        }
    }
}

impl Error for FourLoopBoundaryHaloError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Boundary(error) => Some(error),
            Self::MasterProduct(error) => Some(error),
            _ => None,
        }
    }
}
