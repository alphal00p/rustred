//! Certified finite reduction for the equal-mass five-loop banana.
//!
//! The six physical line momenta may be oriented as
//! `l_i = k_i` for `i < 5` and `l_5 = -(k_0+...+k_4)`.  They obey
//! `l_0+...+l_5 = 0`, so every permutation of the six lines is induced by an
//! integer unit-Jacobian change of the five loop momenta.  This module exposes
//! that `S6` action with an explicit loop map and uses it to certify the
//! complete labelled `(D,N) <= (1,1)` box, with a larger exact factorized
//! boundary where stated:
//!
//! - sectors with at most four active physical lines are scaleless;
//! - every five-line sector with arbitrary resource-bounded positive dots and
//!   at most one numerator reduces by an explicit unimodular
//!   tensor-factorization witness;
//! - the six-line corner is returned as a declared terminal (without a claim
//!   that a complete five-loop reduction has proved master minimality);
//! - every one-dot six-line integral equals
//!   `(12-5*d)/(12*m2)` times that corner.
//! - all top degree-one numerators, including their mixed one-dot shell, close
//!   onto the same corner and five-tadpole product.
//!
//! Positive auxiliaries, total numerator degree above one, and deeper
//! top-sector dots are rejected with typed errors.  In particular, this
//! module never promotes an unsupported integral to a master.

use std::array;
use std::fmt;

use crate::coefficient::{coefficient_product_degree_bound, coefficient_sum_degree_bound};
use crate::exact::invert_matrix;
use crate::five_loop::FIVE_LOOP_BANANA_ROUTINGS;
use crate::{
    Coefficient, Denominator, ExactRational, Integral, LinearCombination,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT, VacuumFamily,
};

pub const FIVE_LOOP_BANANA_PHYSICAL_LINES: usize = 6;
pub const FIVE_LOOP_BANANA_LOOP_MOMENTA: usize = 5;
pub const FIVE_LOOP_BANANA_AUXILIARIES: usize = 9;
pub const FIVE_LOOP_BANANA_DENOMINATORS: usize = 15;
pub const FIVE_LOOP_BANANA_S6_ORDER: usize = 720;

/// Auxiliary-basis positions 6 through 14 are these oriented-line scalar
/// products.  The omitted pair `(3,4)` is supplied by the sixth physical
/// quadratic and therefore is not one of the deterministic auxiliaries.
pub const FIVE_LOOP_BANANA_AUXILIARY_LINE_PAIRS: [[usize; 2]; 9] = [
    [0, 1],
    [0, 2],
    [0, 3],
    [0, 4],
    [1, 2],
    [1, 3],
    [1, 4],
    [2, 3],
    [2, 4],
];

// These are deliberately conservative, deterministic accounting units,
// separate from the explicitly charged tadpole-recurrence iterations.  The
// numerator bound covers a worst-case 5x5 Gauss-Jordan inverse (450 rational
// operations), upper-triangular quadratic transformation (1,145), and the
// subsequent Symbolica coefficient construction with more than 2x headroom.
const SCALAR_ALGEBRA_OPERATION_BOUND: u128 = 64;
const NUMERATOR_ALGEBRA_OPERATION_BOUND: u128 = 4_096;
/// Adjacent transpositions `(0 1)`, ..., `(4 5)`, which generate the full
/// physical `S6` action.  A permutation stores source positions: applying `p`
/// to powers `a` returns `a'[i] = a[p[i]]`.
pub const FIVE_LOOP_BANANA_S6_ADJACENT_TRANSPOSITIONS: [FiveLoopBananaPhysicalPermutation; 5] = [
    FiveLoopBananaPhysicalPermutation::from_sources_unchecked([1, 0, 2, 3, 4, 5]),
    FiveLoopBananaPhysicalPermutation::from_sources_unchecked([0, 2, 1, 3, 4, 5]),
    FiveLoopBananaPhysicalPermutation::from_sources_unchecked([0, 1, 3, 2, 4, 5]),
    FiveLoopBananaPhysicalPermutation::from_sources_unchecked([0, 1, 2, 4, 3, 5]),
    FiveLoopBananaPhysicalPermutation::from_sources_unchecked([0, 1, 2, 3, 5, 4]),
];

/// Resource bounds for one certified five-loop banana reduction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaBoundaryConfig {
    /// Maximum one-loop tadpole recurrence steps in one five-line product.
    /// Values above `u16::MAX` cannot override the hard representability limit
    /// of RustRed's Symbolica coefficient exponent type.
    pub max_tadpole_steps: usize,
    /// Maximum adjacent-transposition word length used to reach the stable
    /// physical-orbit representative.  Every `S6` element needs at most 15.
    pub max_symmetry_steps: usize,
    /// Maximum number of input terms accepted by
    /// [`FiveLoopBananaBoundaryReducer::reduce_combination`].
    pub max_combination_terms: usize,
    /// Maximum aggregate one-loop recurrence steps across one combination.
    pub max_combination_tadpole_steps: usize,
    /// Maximum aggregate adjacent-transposition steps across one combination.
    pub max_combination_symmetry_steps: usize,
    /// Maximum conservative exact-algebra operations charged to one input.
    pub max_algebra_operations: usize,
    /// Maximum aggregate exact-algebra operations across one combination.
    pub max_combination_algebra_operations: usize,
}

impl Default for FiveLoopBananaBoundaryConfig {
    fn default() -> Self {
        Self {
            max_tadpole_steps: u16::MAX as usize,
            max_symmetry_steps: 15,
            max_combination_terms: 1_000_000,
            max_combination_tadpole_steps: 1_000_000,
            max_combination_symmetry_steps: 1_000_000,
            max_algebra_operations: 1_000_000,
            max_combination_algebra_operations: 1_000_000,
        }
    }
}

/// A physical-line permutation together with its exact unimodular action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FiveLoopBananaPhysicalPermutation {
    sources: [usize; FIVE_LOOP_BANANA_PHYSICAL_LINES],
}

impl FiveLoopBananaPhysicalPermutation {
    pub const fn identity() -> Self {
        Self::from_sources_unchecked([0, 1, 2, 3, 4, 5])
    }

    pub fn try_new(
        sources: [usize; FIVE_LOOP_BANANA_PHYSICAL_LINES],
    ) -> Result<Self, FiveLoopBananaPermutationError> {
        let mut seen = [false; FIVE_LOOP_BANANA_PHYSICAL_LINES];
        for (position, &source) in sources.iter().enumerate() {
            if source >= FIVE_LOOP_BANANA_PHYSICAL_LINES {
                return Err(FiveLoopBananaPermutationError::SourceOutOfRange { position, source });
            }
            if seen[source] {
                return Err(FiveLoopBananaPermutationError::DuplicateSource { source });
            }
            seen[source] = true;
        }
        Ok(Self { sources })
    }

    const fn from_sources_unchecked(sources: [usize; FIVE_LOOP_BANANA_PHYSICAL_LINES]) -> Self {
        Self { sources }
    }

    pub const fn sources(&self) -> &[usize; FIVE_LOOP_BANANA_PHYSICAL_LINES] {
        &self.sources
    }

    /// Apply this physical action without pretending that the nine auxiliary
    /// basis entries transform by a denominator permutation.
    pub fn apply_physical_powers(
        self,
        powers: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
    ) -> [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES] {
        array::from_fn(|target| powers[self.sources[target]])
    }

    /// Apply `self` and then `next`.
    pub fn followed_by(self, next: Self) -> Self {
        Self::from_sources_unchecked(array::from_fn(|target| self.sources[next.sources[target]]))
    }

    pub fn inverse(self) -> Self {
        let mut inverse = [0; FIVE_LOOP_BANANA_PHYSICAL_LINES];
        for (target, source) in self.sources.into_iter().enumerate() {
            inverse[source] = target;
        }
        Self::from_sources_unchecked(inverse)
    }

    /// A word in [`FIVE_LOOP_BANANA_S6_ADJACENT_TRANSPOSITIONS`] that builds
    /// this permutation from the identity.  Its length is the inversion count
    /// and is therefore at most 15.
    pub fn adjacent_generator_word(self) -> Vec<usize> {
        let mut current = [0, 1, 2, 3, 4, 5];
        let mut word = Vec::new();
        for target in 0..FIVE_LOOP_BANANA_PHYSICAL_LINES {
            let mut position = current
                .iter()
                .position(|&source| source == self.sources[target])
                .expect("a validated permutation contains every source");
            while position > target {
                current.swap(position - 1, position);
                word.push(position - 1);
                position -= 1;
            }
        }
        debug_assert_eq!(current, self.sources);
        word
    }

    /// The integer map `new_k_i = l_sources[i](old_k)` for `i=0,...,4`,
    /// where `l_5=-(k_0+...+k_4)`.  Its determinant is exactly `+1` or `-1`.
    pub fn unimodular_loop_map(self) -> [[i8; FIVE_LOOP_BANANA_LOOP_MOMENTA]; 5] {
        array::from_fn(|target| {
            five_loop_banana_oriented_line_routing(self.sources[target])
                .expect("a validated permutation has in-range sources")
        })
    }

    /// Determinant of [`Self::unimodular_loop_map`].  This is the sign of the
    /// permutation in the five-dimensional standard representation of `S6`.
    pub fn determinant_sign(self) -> i8 {
        let inversions = (0..FIVE_LOOP_BANANA_PHYSICAL_LINES)
            .flat_map(|left| {
                (left + 1..FIVE_LOOP_BANANA_PHYSICAL_LINES).map(move |right| (left, right))
            })
            .filter(|&(left, right)| self.sources[left] > self.sources[right])
            .count();
        if inversions % 2 == 0 { 1 } else { -1 }
    }
}

/// Oriented routing used to make the physical `S6` action linear.  The sixth
/// routing differs by a harmless sign from `FIVE_LOOP_BANANA_ROUTINGS[5]`.
pub fn five_loop_banana_oriented_line_routing(
    line: usize,
) -> Option<[i8; FIVE_LOOP_BANANA_LOOP_MOMENTA]> {
    match line {
        0..=4 => Some(array::from_fn(|column| if column == line { 1 } else { 0 })),
        5 => Some([-1; FIVE_LOOP_BANANA_LOOP_MOMENTA]),
        _ => None,
    }
}

/// Checkable evidence that one physical exponent vector belongs to a stable
/// `S6` representative.  The canonical powers are sorted non-increasingly;
/// ties retain physical-line order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaOrbitWitness {
    original: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
    canonical: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
    permutation: FiveLoopBananaPhysicalPermutation,
    adjacent_generators: Vec<usize>,
}

impl FiveLoopBananaOrbitWitness {
    pub const fn original(&self) -> &[i32; FIVE_LOOP_BANANA_PHYSICAL_LINES] {
        &self.original
    }

    pub const fn canonical(&self) -> &[i32; FIVE_LOOP_BANANA_PHYSICAL_LINES] {
        &self.canonical
    }

    pub const fn permutation(&self) -> FiveLoopBananaPhysicalPermutation {
        self.permutation
    }

    pub fn adjacent_generators(&self) -> &[usize] {
        &self.adjacent_generators
    }

    pub fn unimodular_loop_map(&self) -> [[i8; FIVE_LOOP_BANANA_LOOP_MOMENTA]; 5] {
        self.permutation.unimodular_loop_map()
    }

    pub fn determinant_sign(&self) -> i8 {
        self.permutation.determinant_sign()
    }
}

/// Exact change-of-variables certificate for a degree-one numerator in a
/// five-line product sector.
///
/// The transformed row uses the standard upper-triangular scalar-product
/// order `(p0.p0,p0.p1,...,p4.p4)`.  It represents
/// `mass_coefficient*m2 + sum row[rs] p_r.p_s`, where the five `p_r` are the
/// independent active oriented lines selected by [`Self::loop_map`].  Mixed
/// products integrate to zero by independent tadpole parity; the five
/// diagonal entries are retained explicitly rather than hidden in a trace.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaProductNumeratorWitness {
    numerator_position: usize,
    missing_line: usize,
    active_lines: [usize; FIVE_LOOP_BANANA_LOOP_MOMENTA],
    loop_map: [[i8; FIVE_LOOP_BANANA_LOOP_MOMENTA]; FIVE_LOOP_BANANA_LOOP_MOMENTA],
    mass_coefficient: ExactRational,
    transformed_quadratic_form: [ExactRational; FIVE_LOOP_BANANA_DENOMINATORS],
}

impl FiveLoopBananaProductNumeratorWitness {
    pub const fn numerator_position(&self) -> usize {
        self.numerator_position
    }

    pub const fn missing_line(&self) -> usize {
        self.missing_line
    }

    /// Original oriented physical line represented by each transformed loop
    /// variable.  Diagonal coefficients must be paired with powers in this
    /// order; the sorted `S6` orbit is intentionally not used here.
    pub const fn active_lines(&self) -> &[usize; FIVE_LOOP_BANANA_LOOP_MOMENTA] {
        &self.active_lines
    }

    pub const fn loop_map(
        &self,
    ) -> &[[i8; FIVE_LOOP_BANANA_LOOP_MOMENTA]; FIVE_LOOP_BANANA_LOOP_MOMENTA] {
        &self.loop_map
    }

    pub const fn mass_coefficient(&self) -> ExactRational {
        self.mass_coefficient
    }

    pub const fn transformed_quadratic_form(
        &self,
    ) -> &[ExactRational; FIVE_LOOP_BANANA_DENOMINATORS] {
        &self.transformed_quadratic_form
    }

    pub fn diagonal_coefficients(&self) -> [ExactRational; FIVE_LOOP_BANANA_LOOP_MOMENTA] {
        array::from_fn(|loop_index| {
            self.transformed_quadratic_form[scalar_product_index(loop_index, loop_index)]
        })
    }
}

pub fn five_loop_banana_physical_orbit_witness(
    physical_powers: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
) -> FiveLoopBananaOrbitWitness {
    let mut sources = [0, 1, 2, 3, 4, 5];
    sources.sort_by(|&left, &right| {
        physical_powers[right]
            .cmp(&physical_powers[left])
            .then_with(|| left.cmp(&right))
    });
    let permutation = FiveLoopBananaPhysicalPermutation::from_sources_unchecked(sources);
    FiveLoopBananaOrbitWitness {
        original: physical_powers,
        canonical: permutation.apply_physical_powers(physical_powers),
        adjacent_generators: permutation.adjacent_generator_word(),
        permutation,
    }
}

/// Exact classification returned before coefficient construction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FiveLoopBananaScalarClass {
    ScalelessPinch {
        sector_mask: u8,
        active_lines: usize,
    },
    UnimodularProduct {
        missing_line: usize,
        tadpole_steps: u128,
        orbit: FiveLoopBananaOrbitWitness,
        numerator: Option<FiveLoopBananaProductNumeratorWitness>,
        physical_powers: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
        algebra_operations: u128,
    },
    TopCorner,
    TopOneDot {
        dotted_line: usize,
        orbit: FiveLoopBananaOrbitWitness,
    },
    TopNumerator {
        numerator_position: usize,
    },
    TopOneDotNumerator {
        dotted_line: usize,
        numerator_position: usize,
        incident_to_dot: bool,
        orbit: FiveLoopBananaOrbitWitness,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct FiveLoopBananaResourceRequest {
    tadpole_steps: u128,
    symmetry_steps: u128,
    algebra_operations: u128,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FiveLoopBananaIntegralAnalysis {
    numerator_position: Option<usize>,
    physical: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
    sector_mask: u8,
    active_lines: usize,
    resources: FiveLoopBananaResourceRequest,
}

/// Owned reducer for the certified scalar slice described in the module docs.
#[derive(Clone, Debug)]
pub struct FiveLoopBananaBoundaryReducer {
    family: VacuumFamily,
    config: FiveLoopBananaBoundaryConfig,
    mass: Coefficient,
    product_master: Integral,
    top_master: Integral,
}

impl FiveLoopBananaBoundaryReducer {
    pub fn new(
        family: VacuumFamily,
        config: FiveLoopBananaBoundaryConfig,
    ) -> Result<Self, FiveLoopBananaBoundaryError> {
        let mass = validate_family(&family)?;
        Ok(Self {
            family,
            config,
            mass,
            product_master: Integral::from([1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
            top_master: Integral::from([1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        })
    }

    pub fn family(&self) -> &VacuumFamily {
        &self.family
    }

    pub const fn config(&self) -> FiveLoopBananaBoundaryConfig {
        self.config
    }

    /// Stable representative `I(1,1,1,1,1,0;0,...,0) = T1^5`.
    pub fn product_master(&self) -> &Integral {
        &self.product_master
    }

    /// Stable declared six-line banana terminal
    /// `I(1,1,1,1,1,1;0,...,0)`.  This bounded reducer does not claim that a
    /// complete top-sector analysis has proved master minimality.
    pub fn top_master(&self) -> &Integral {
        &self.top_master
    }

    pub fn classify_integral(
        &self,
        integral: &Integral,
    ) -> Result<FiveLoopBananaScalarClass, FiveLoopBananaBoundaryError> {
        let analysis = self.analyze_integral(integral)?;
        self.check_integral_resources(analysis.resources)?;
        self.classify_analyzed_integral(integral, analysis)
    }

    fn classify_analyzed_integral(
        &self,
        integral: &Integral,
        analysis: FiveLoopBananaIntegralAnalysis,
    ) -> Result<FiveLoopBananaScalarClass, FiveLoopBananaBoundaryError> {
        let FiveLoopBananaIntegralAnalysis {
            numerator_position,
            physical,
            sector_mask,
            active_lines,
            resources,
        } = analysis;
        if active_lines <= 4 {
            // A polynomial numerator cannot restore the missing radial scale:
            // rank below five leaves at least one unconstrained loop
            // integration, so the integral is scaleless term by term.
            return Ok(FiveLoopBananaScalarClass::ScalelessPinch {
                sector_mask,
                active_lines,
            });
        }

        if active_lines == 5 {
            let missing_line = physical
                .iter()
                .position(|&power| power <= 0)
                .expect("five active lines leave one nonpositive physical entry");
            let tadpole_steps = physical
                .iter()
                .filter(|&&power| power > 0)
                .map(|&power| u128::try_from(power - 1).expect("active powers are positive"))
                .sum();
            debug_assert_eq!(tadpole_steps, resources.tadpole_steps);
            let orbit_powers = physical.map(|power| power.max(0));
            let orbit = five_loop_banana_physical_orbit_witness(orbit_powers);
            debug_assert_eq!(
                orbit.adjacent_generators().len() as u128,
                resources.symmetry_steps
            );
            let algebra_operations = resources.algebra_operations;
            let numerator = numerator_position
                .map(|position| self.product_numerator_witness(missing_line, position))
                .transpose()?;
            return Ok(FiveLoopBananaScalarClass::UnimodularProduct {
                missing_line,
                tadpole_steps,
                orbit,
                numerator,
                physical_powers: physical,
                algebra_operations,
            });
        }

        let dot_degree = physical
            .iter()
            .map(|&power| u128::try_from(power - 1).expect("top-sector powers are positive"))
            .sum::<u128>();
        match (dot_degree, numerator_position) {
            (0, None) => Ok(FiveLoopBananaScalarClass::TopCorner),
            (0, Some(numerator_position)) => {
                Ok(FiveLoopBananaScalarClass::TopNumerator { numerator_position })
            }
            (1, numerator_position) => {
                let dotted_line = physical
                    .iter()
                    .position(|&power| power == 2)
                    .expect("one total dot identifies one line");
                let orbit =
                    five_loop_banana_physical_orbit_witness(physical.map(|power| power.max(0)));
                debug_assert_eq!(
                    orbit.adjacent_generators().len() as u128,
                    resources.symmetry_steps
                );
                if let Some(numerator_position) = numerator_position {
                    Ok(FiveLoopBananaScalarClass::TopOneDotNumerator {
                        dotted_line,
                        numerator_position,
                        incident_to_dot: auxiliary_pair(numerator_position)
                            .is_some_and(|pair| pair.contains(&dotted_line)),
                        orbit,
                    })
                } else {
                    Ok(FiveLoopBananaScalarClass::TopOneDot { dotted_line, orbit })
                }
            }
            _ => Err(FiveLoopBananaBoundaryError::UnsupportedTopDots {
                integral: integral.clone(),
                dot_degree,
            }),
        }
    }

    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, FiveLoopBananaBoundaryError> {
        self.reduce_class(self.classify_integral(integral)?)
    }

    fn reduce_class(
        &self,
        class: FiveLoopBananaScalarClass,
    ) -> Result<LinearCombination, FiveLoopBananaBoundaryError> {
        match class {
            FiveLoopBananaScalarClass::ScalelessPinch { .. } => Ok(LinearCombination::new()),
            FiveLoopBananaScalarClass::UnimodularProduct {
                orbit,
                numerator,
                physical_powers,
                ..
            } => {
                let coefficient = if let Some(witness) = numerator {
                    self.reduce_product_numerator(&physical_powers, &witness)
                } else {
                    orbit.canonical()[..FIVE_LOOP_BANANA_LOOP_MOMENTA]
                        .iter()
                        .fold(self.family.coefficients().one(), |coefficient, &power| {
                            &coefficient * &self.tadpole_ratio(power)
                        })
                };
                Ok(LinearCombination::from_term(
                    self.product_master.clone(),
                    coefficient,
                ))
            }
            FiveLoopBananaScalarClass::TopCorner => Ok(LinearCombination::from_term(
                self.top_master.clone(),
                self.family.coefficients().one(),
            )),
            FiveLoopBananaScalarClass::TopOneDot { .. } => Ok(LinearCombination::from_term(
                self.top_master.clone(),
                self.top_one_dot_ratio(),
            )),
            FiveLoopBananaScalarClass::TopNumerator { .. } => {
                let context = self.family.coefficients();
                let mut result = LinearCombination::new();
                result.add_term(
                    self.top_master.clone(),
                    &self.mass * &context.rational(ExactRational::new(1, 5)),
                );
                result.add_term(
                    self.product_master.clone(),
                    context.rational(ExactRational::new(-1, 5)),
                );
                Ok(result)
            }
            FiveLoopBananaScalarClass::TopOneDotNumerator {
                incident_to_dot, ..
            } => {
                let context = self.family.coefficients();
                let mut result = LinearCombination::new();
                if incident_to_dot {
                    result.add_term(
                        self.top_master.clone(),
                        context.scale_rational(self.family.dimension(), ExactRational::new(-1, 12)),
                    );
                } else {
                    let top_coefficient = &context.rational(ExactRational::new(1, 4))
                        + &context
                            .scale_rational(self.family.dimension(), ExactRational::new(-1, 12));
                    result.add_term(self.top_master.clone(), top_coefficient);
                    let product_numerator = &context
                        .scale_rational(self.family.dimension(), ExactRational::new(1, 8))
                        - &context.rational(ExactRational::new(1, 4));
                    result.add_term(self.product_master.clone(), &product_numerator / &self.mass);
                }
                Ok(result)
            }
        }
    }

    /// Reduce every term or return the first typed domain/resource error.  No
    /// unsupported term is copied through as an implicit master.
    pub fn reduce_combination(
        &self,
        combination: &LinearCombination,
    ) -> Result<LinearCombination, FiveLoopBananaBoundaryError> {
        self.check_resource(
            "input combination terms",
            combination.len() as u128,
            self.config.max_combination_terms as u128,
        )?;

        // Preflight the complete combination before constructing any exact
        // quadratic-change witness or symbolic reduction coefficient.  This
        // makes an aggregate cap a bound on work, not merely a retrospective
        // diagnostic after the expensive part has already happened.
        let mut aggregate_tadpole_steps = 0_u128;
        let mut aggregate_symmetry_steps = 0_u128;
        let mut aggregate_algebra_operations = 0_u128;
        for integral in combination.terms().keys() {
            let resources = self.analyze_integral(integral)?.resources;
            self.check_integral_resources(resources)?;
            // All public limits are `usize`, so a saturated `u128::MAX` is
            // still a truthful over-limit request on every supported target.
            aggregate_tadpole_steps =
                aggregate_tadpole_steps.saturating_add(resources.tadpole_steps);
            aggregate_symmetry_steps =
                aggregate_symmetry_steps.saturating_add(resources.symmetry_steps);
            aggregate_algebra_operations =
                aggregate_algebra_operations.saturating_add(resources.algebra_operations);
            self.check_resource(
                "combination tadpole recurrence steps",
                aggregate_tadpole_steps,
                self.config.max_combination_tadpole_steps as u128,
            )?;
            self.check_resource(
                "combination adjacent symmetry steps",
                aggregate_symmetry_steps,
                self.config.max_combination_symmetry_steps as u128,
            )?;
            self.check_resource(
                "combination exact algebra operations",
                aggregate_algebra_operations,
                self.config.max_combination_algebra_operations as u128,
            )?;
        }

        let mut output = LinearCombination::new();
        for (integral, coefficient) in combination.terms() {
            let class = self.classify_integral(integral)?;
            self.add_scaled_checked(&mut output, &self.reduce_class(class)?, coefficient)?;
        }
        Ok(output)
    }

    /// Scale and merge only after conservatively bounding every per-variable
    /// Symbolica exponent.  This also rejects coefficients built over a
    /// different variable map before `RationalPolynomial` tries to unify it.
    fn add_scaled_checked(
        &self,
        output: &mut LinearCombination,
        reduction: &LinearCombination,
        factor: &Coefficient,
    ) -> Result<(), FiveLoopBananaBoundaryError> {
        if factor.is_zero() {
            return Ok(());
        }
        for (integral, coefficient) in reduction.terms() {
            self.check_resource(
                "Symbolica coefficient exponent degree",
                coefficient_product_degree_bound(coefficient, factor),
                SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            )?;
            let scaled = coefficient * factor;
            if let Some(current) = output.coefficient(integral) {
                self.check_resource(
                    "Symbolica coefficient exponent degree",
                    coefficient_sum_degree_bound(current, &scaled),
                    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
                )?;
            }
            output.add_term(integral.clone(), scaled);
        }
        Ok(())
    }

    fn analyze_integral(
        &self,
        integral: &Integral,
    ) -> Result<FiveLoopBananaIntegralAnalysis, FiveLoopBananaBoundaryError> {
        let numerator_position = self.validate_integral(integral)?;
        let physical: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES] = integral.powers()
            [..FIVE_LOOP_BANANA_PHYSICAL_LINES]
            .try_into()
            .expect("integral arity was validated");
        let sector_mask = physical
            .iter()
            .enumerate()
            .fold(0_u8, |mask, (position, &power)| {
                mask | (u8::from(power > 0) << position)
            });
        let active_lines = sector_mask.count_ones() as usize;
        let mut resources = FiveLoopBananaResourceRequest::default();

        if active_lines == 5 {
            resources.tadpole_steps = physical
                .iter()
                .filter(|&&power| power > 0)
                .map(|&power| u128::try_from(power - 1).expect("active powers are positive"))
                .sum();
            resources.symmetry_steps =
                five_loop_banana_physical_orbit_witness(physical.map(|power| power.max(0)))
                    .adjacent_generators()
                    .len() as u128;
            resources.algebra_operations = if numerator_position.is_some() {
                NUMERATOR_ALGEBRA_OPERATION_BOUND
            } else {
                SCALAR_ALGEBRA_OPERATION_BOUND
            };
        } else if active_lines == FIVE_LOOP_BANANA_PHYSICAL_LINES {
            let dot_degree = physical
                .iter()
                .map(|&power| u128::try_from(power - 1).expect("top-sector powers are positive"))
                .sum::<u128>();
            if dot_degree > 1 {
                return Err(FiveLoopBananaBoundaryError::UnsupportedTopDots {
                    integral: integral.clone(),
                    dot_degree,
                });
            }
            if dot_degree == 1 {
                resources.symmetry_steps = five_loop_banana_physical_orbit_witness(physical)
                    .adjacent_generators()
                    .len() as u128;
                resources.algebra_operations = SCALAR_ALGEBRA_OPERATION_BOUND;
            }
            if numerator_position.is_some() {
                resources.algebra_operations = NUMERATOR_ALGEBRA_OPERATION_BOUND;
            }
        }

        Ok(FiveLoopBananaIntegralAnalysis {
            numerator_position,
            physical,
            sector_mask,
            active_lines,
            resources,
        })
    }

    fn check_integral_resources(
        &self,
        resources: FiveLoopBananaResourceRequest,
    ) -> Result<(), FiveLoopBananaBoundaryError> {
        self.check_resource(
            "tadpole recurrence steps",
            resources.tadpole_steps,
            self.config.max_tadpole_steps as u128,
        )?;
        // This representability ceiling remains in force even if a caller
        // deliberately configures a larger work budget.
        self.check_resource(
            "Symbolica coefficient exponent degree",
            resources.tadpole_steps,
            SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        )?;
        self.check_resource(
            "adjacent symmetry steps",
            resources.symmetry_steps,
            self.config.max_symmetry_steps as u128,
        )?;
        self.check_resource(
            "exact algebra operations",
            resources.algebra_operations,
            self.config.max_algebra_operations as u128,
        )
    }

    fn validate_integral(
        &self,
        integral: &Integral,
    ) -> Result<Option<usize>, FiveLoopBananaBoundaryError> {
        if integral.powers().len() != FIVE_LOOP_BANANA_DENOMINATORS {
            return Err(FiveLoopBananaBoundaryError::WrongIntegralArity {
                expected: FIVE_LOOP_BANANA_DENOMINATORS,
                actual: integral.powers().len(),
            });
        }
        for (offset, &power) in integral.powers()[FIVE_LOOP_BANANA_PHYSICAL_LINES..]
            .iter()
            .enumerate()
        {
            if power > 0 {
                let position = offset + FIVE_LOOP_BANANA_PHYSICAL_LINES;
                return Err(FiveLoopBananaBoundaryError::PositiveAuxiliary { position, power });
            }
        }

        let numerator_degree = integral
            .powers()
            .iter()
            .zip(self.family.denominators())
            .fold(0_u128, |degree, (&power, denominator)| {
                degree.saturating_add(
                    if power < 0 || (power == 0 && denominator.is_propagator()) {
                        u128::from(power.unsigned_abs())
                    } else {
                        0
                    },
                )
            });
        if numerator_degree > 1 {
            return Err(FiveLoopBananaBoundaryError::UnsupportedNumeratorDegree {
                integral: integral.clone(),
                numerator_degree,
            });
        }
        let numerator_position = integral.powers().iter().position(|&power| power < 0);
        if let Some(position) = numerator_position {
            let power = integral.powers()[position];
            if position < FIVE_LOOP_BANANA_PHYSICAL_LINES {
                // A physical denominator can occur polynomially only when its
                // line is inactive.  This is automatic for a negative power.
                debug_assert_eq!(power, -1);
            }
        }
        Ok(numerator_position)
    }

    fn check_resource(
        &self,
        resource: &'static str,
        requested: u128,
        limit: u128,
    ) -> Result<(), FiveLoopBananaBoundaryError> {
        if requested > limit {
            return Err(FiveLoopBananaBoundaryError::ResourceLimit {
                resource,
                requested,
                limit,
            });
        }
        Ok(())
    }

    fn product_numerator_witness(
        &self,
        missing_line: usize,
        numerator_position: usize,
    ) -> Result<FiveLoopBananaProductNumeratorWitness, FiveLoopBananaBoundaryError> {
        let active_lines: [usize; FIVE_LOOP_BANANA_LOOP_MOMENTA] = (0
            ..FIVE_LOOP_BANANA_PHYSICAL_LINES)
            .filter(|&line| line != missing_line)
            .collect::<Vec<_>>()
            .try_into()
            .expect("one of six physical lines is missing");
        let loop_map = active_lines.map(|line| {
            five_loop_banana_oriented_line_routing(line)
                .expect("an active physical line is in range")
        });
        let loop_map_exact: Vec<Vec<_>> = loop_map
            .iter()
            .map(|row| {
                row.iter()
                    .map(|&entry| ExactRational::from(i64::from(entry)))
                    .collect()
            })
            .collect();
        let inverse = invert_matrix(&loop_map_exact).map_err(|message| {
            FiveLoopBananaBoundaryError::InvalidProductLoopMap {
                missing_line,
                message,
            }
        })?;

        let denominator = &self.family.denominators()[numerator_position];
        let old_quadratic = denominator.quadratic_form();
        let mut transformed = [ExactRational::ZERO; FIVE_LOOP_BANANA_DENOMINATORS];
        // If p=A*k, then k=A^{-1}p.  Convert the stored upper-triangular row
        // to a symmetric matrix Q (halving its off-diagonal entries), compute
        // A^{-T} Q A^{-1}, then convert back to the stored row convention.
        let mut old_matrix = [[ExactRational::ZERO; 5]; 5];
        for left in 0..5 {
            for right in left..5 {
                let coefficient = old_quadratic[scalar_product_index(left, right)];
                if left == right {
                    old_matrix[left][right] = coefficient;
                } else {
                    let half = coefficient / ExactRational::from(2);
                    old_matrix[left][right] = half;
                    old_matrix[right][left] = half;
                }
            }
        }
        for new_left in 0..5 {
            for new_right in new_left..5 {
                let matrix_entry = (0..5)
                    .flat_map(|old_left| (0..5).map(move |old_right| (old_left, old_right)))
                    .map(|(old_left, old_right)| {
                        inverse[old_left][new_left]
                            * old_matrix[old_left][old_right]
                            * inverse[old_right][new_right]
                    })
                    .fold(ExactRational::ZERO, std::ops::Add::add);
                transformed[scalar_product_index(new_left, new_right)] = if new_left == new_right {
                    matrix_entry
                } else {
                    matrix_entry * ExactRational::from(2)
                };
            }
        }
        let mass_coefficient = if numerator_position < FIVE_LOOP_BANANA_PHYSICAL_LINES {
            ExactRational::ONE
        } else {
            ExactRational::ZERO
        };
        Ok(FiveLoopBananaProductNumeratorWitness {
            numerator_position,
            missing_line,
            active_lines,
            loop_map,
            mass_coefficient,
            transformed_quadratic_form: transformed,
        })
    }

    fn reduce_product_numerator(
        &self,
        physical_powers: &[i32; FIVE_LOOP_BANANA_PHYSICAL_LINES],
        witness: &FiveLoopBananaProductNumeratorWitness,
    ) -> Coefficient {
        let context = self.family.coefficients();
        // Compute R_a and R_(a-1) in one charged recurrence traversal.  In
        // particular, diagonal numerator insertions must not silently repeat
        // another `a-2` recurrence steps after the resource preflight.
        let ratio_pairs: [(Coefficient, Coefficient); FIVE_LOOP_BANANA_LOOP_MOMENTA] =
            array::from_fn(|position| {
                self.tadpole_ratio_with_lowered(physical_powers[witness.active_lines[position]])
            });
        let ratios: [Coefficient; FIVE_LOOP_BANANA_LOOP_MOMENTA] =
            array::from_fn(|position| ratio_pairs[position].0.clone());
        let product = ratios
            .iter()
            .fold(context.one(), |coefficient, ratio| &coefficient * ratio);
        let mut coefficient =
            &context.scale_rational(&self.mass, witness.mass_coefficient) * &product;
        for transformed_loop in 0..FIVE_LOOP_BANANA_LOOP_MOMENTA {
            let diagonal = witness.transformed_quadratic_form
                [scalar_product_index(transformed_loop, transformed_loop)];
            if diagonal.is_zero() {
                continue;
            }
            let radial_insertion =
                &ratio_pairs[transformed_loop].1 - &(&self.mass * &ratios[transformed_loop]);
            let other_product = ratios
                .iter()
                .enumerate()
                .filter(|(position, _)| *position != transformed_loop)
                .fold(context.one(), |product, (_, ratio)| &product * ratio);
            coefficient = &coefficient
                + &context.scale_rational(&(&radial_insertion * &other_product), diagonal);
        }
        coefficient
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

    fn tadpole_ratio_with_lowered(&self, power: i32) -> (Coefficient, Coefficient) {
        debug_assert!(power > 0);
        let context = self.family.coefficients();
        let mut ratio = context.one();
        let mut lowered = context.zero();
        for n in 1..i64::from(power) {
            lowered = ratio.clone();
            let two_n = context.integer(2 * n);
            ratio = &(&ratio * &(&two_n - self.family.dimension())) / &(&two_n * &self.mass);
        }
        (ratio, lowered)
    }

    fn top_one_dot_ratio(&self) -> Coefficient {
        let context = self.family.coefficients();
        let numerator = &context.integer(12) - &context.scale_integer(self.family.dimension(), 5);
        let denominator = &context.integer(12) * &self.mass;
        &numerator / &denominator
    }

    /// Public proof surface for a five-line degree-one numerator.  This is
    /// useful to verify the exact quadratic change of variables independently
    /// of coefficient construction.
    pub fn product_numerator_witness_for(
        &self,
        integral: &Integral,
    ) -> Result<Option<FiveLoopBananaProductNumeratorWitness>, FiveLoopBananaBoundaryError> {
        match self.classify_integral(integral)? {
            FiveLoopBananaScalarClass::UnimodularProduct { numerator, .. } => Ok(numerator),
            _ => Ok(None),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FiveLoopBananaPermutationError {
    SourceOutOfRange { position: usize, source: usize },
    DuplicateSource { source: usize },
}

impl fmt::Display for FiveLoopBananaPermutationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SourceOutOfRange { position, source } => write!(
                formatter,
                "physical permutation source {source} at position {position} is outside 0..6"
            ),
            Self::DuplicateSource { source } => {
                write!(formatter, "physical permutation repeats source {source}")
            }
        }
    }
}

impl std::error::Error for FiveLoopBananaPermutationError {}

fn scalar_product_index(left: usize, right: usize) -> usize {
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    (0..left)
        .map(|row| FIVE_LOOP_BANANA_LOOP_MOMENTA - row)
        .sum::<usize>()
        + right
        - left
}

fn auxiliary_pair(position: usize) -> Option<[usize; 2]> {
    position
        .checked_sub(FIVE_LOOP_BANANA_PHYSICAL_LINES)
        .and_then(|offset| FIVE_LOOP_BANANA_AUXILIARY_LINE_PAIRS.get(offset).copied())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FiveLoopBananaBoundaryError {
    WrongLoopCount {
        actual: usize,
    },
    WrongDenominatorCount {
        actual: usize,
    },
    WrongPhysicalCount {
        actual: usize,
    },
    MissingParameter {
        name: &'static str,
    },
    WrongDimensionParameter,
    WrongPropagatorSign {
        position: usize,
    },
    UnequalMasses {
        position: usize,
    },
    WrongMomentumRouting {
        position: usize,
    },
    WrongAuxiliaryLayout {
        position: usize,
    },
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    /// Retained for source compatibility; degree-one auxiliary numerators are
    /// now part of the certified box and no longer produce this error.
    AuxiliaryNumerator {
        position: usize,
        power: i32,
    },
    PositiveAuxiliary {
        position: usize,
        power: i32,
    },
    /// Retained for source compatibility; an inactive degree-one physical
    /// numerator is now reduced by the product-sector witness.
    PhysicalNumerator {
        position: usize,
        power: i32,
    },
    UnsupportedNumeratorDegree {
        integral: Integral,
        numerator_degree: u128,
    },
    UnsupportedTopDots {
        integral: Integral,
        dot_degree: u128,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    InvalidProductLoopMap {
        missing_line: usize,
        message: String,
    },
}

impl fmt::Display for FiveLoopBananaBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLoopCount { actual } => {
                write!(
                    formatter,
                    "the five-loop banana reducer received {actual} loops"
                )
            }
            Self::WrongDenominatorCount { actual } => write!(
                formatter,
                "the five-loop banana needs 15 basis denominators, received {actual}"
            ),
            Self::WrongPhysicalCount { actual } => write!(
                formatter,
                "the five-loop banana needs six physical lines, received {actual}"
            ),
            Self::MissingParameter { name } => {
                write!(
                    formatter,
                    "the five-loop banana family does not define {name}"
                )
            }
            Self::WrongDimensionParameter => formatter
                .write_str("the five-loop banana family does not use parameter d as its dimension"),
            Self::WrongPropagatorSign { position } => write!(
                formatter,
                "denominator {position} is not a positive-Euclidean physical propagator"
            ),
            Self::UnequalMasses { position } => write!(
                formatter,
                "denominator {position} does not carry the common nonzero mass m2"
            ),
            Self::WrongMomentumRouting { position } => write!(
                formatter,
                "physical denominator {position} has the wrong banana routing"
            ),
            Self::WrongAuxiliaryLayout { position } => write!(
                formatter,
                "basis denominator {position} is not the deterministic banana auxiliary"
            ),
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "a five-loop banana integral needs {expected} powers, received {actual}"
            ),
            Self::AuxiliaryNumerator { position, power } => write!(
                formatter,
                "auxiliary {position} has unsupported numerator power {power}"
            ),
            Self::PositiveAuxiliary { position, power } => write!(
                formatter,
                "auxiliary {position} has positive denominator power {power}, outside the certified scalar slice"
            ),
            Self::PhysicalNumerator { position, power } => write!(
                formatter,
                "physical line {position} has unsupported numerator power {power}"
            ),
            Self::UnsupportedNumeratorDegree {
                integral,
                numerator_degree,
            } => write!(
                formatter,
                "five-loop banana integral {integral} has numerator degree {numerator_degree}; the certified box permits total degree at most one"
            ),
            Self::UnsupportedTopDots {
                integral,
                dot_degree,
            } => write!(
                formatter,
                "six-line top integral {integral} has dot degree {dot_degree}; only the corner and one-dot orbit are certified"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "five-loop banana {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::InvalidProductLoopMap {
                missing_line,
                message,
            } => write!(
                formatter,
                "five-loop product sector missing line {missing_line} has an invalid loop map: {message}"
            ),
        }
    }
}

impl std::error::Error for FiveLoopBananaBoundaryError {}

fn validate_family(family: &VacuumFamily) -> Result<Coefficient, FiveLoopBananaBoundaryError> {
    if family.loops() != FIVE_LOOP_BANANA_LOOP_MOMENTA {
        return Err(FiveLoopBananaBoundaryError::WrongLoopCount {
            actual: family.loops(),
        });
    }
    if family.denominator_count() != FIVE_LOOP_BANANA_DENOMINATORS {
        return Err(FiveLoopBananaBoundaryError::WrongDenominatorCount {
            actual: family.denominator_count(),
        });
    }
    if family.propagator_count() != FIVE_LOOP_BANANA_PHYSICAL_LINES {
        return Err(FiveLoopBananaBoundaryError::WrongPhysicalCount {
            actual: family.propagator_count(),
        });
    }
    let mass = family
        .coefficients()
        .parameter("m2")
        .ok_or(FiveLoopBananaBoundaryError::MissingParameter { name: "m2" })?;
    let dimension = family
        .coefficients()
        .parameter("d")
        .ok_or(FiveLoopBananaBoundaryError::MissingParameter { name: "d" })?;
    if family.dimension() != &dimension {
        return Err(FiveLoopBananaBoundaryError::WrongDimensionParameter);
    }
    for (position, expected_routing) in FIVE_LOOP_BANANA_ROUTINGS.iter().enumerate() {
        let denominator = &family.denominators()[position];
        if denominator.normalization() != Some(1) {
            return Err(FiveLoopBananaBoundaryError::WrongPropagatorSign { position });
        }
        if mass.is_zero() || denominator.shift() != &mass {
            return Err(FiveLoopBananaBoundaryError::UnequalMasses { position });
        }
        let expected = Denominator::propagator(
            expected_routing
                .iter()
                .map(|&component| ExactRational::from(i64::from(component)))
                .collect(),
            mass.clone(),
        );
        if denominator.quadratic_form() != expected.quadratic_form() {
            return Err(FiveLoopBananaBoundaryError::WrongMomentumRouting { position });
        }
    }

    // Upper-triangular scalar-product positions s12, s13, s14, s15, s23,
    // s24, s25, s34, s35 complete the six independent physical quadratics.
    const AUXILIARY_SCALAR_PRODUCTS: [usize; FIVE_LOOP_BANANA_AUXILIARIES] =
        [1, 2, 3, 4, 6, 7, 8, 10, 11];
    for (offset, &scalar_product) in AUXILIARY_SCALAR_PRODUCTS.iter().enumerate() {
        let position = FIVE_LOOP_BANANA_PHYSICAL_LINES + offset;
        let denominator = &family.denominators()[position];
        let expected: Vec<_> = (0..FIVE_LOOP_BANANA_DENOMINATORS)
            .map(|candidate| {
                if candidate == scalar_product {
                    ExactRational::ONE
                } else {
                    ExactRational::ZERO
                }
            })
            .collect();
        if denominator.is_propagator()
            || !denominator.shift().is_zero()
            || denominator.quadratic_form() != expected.as_slice()
        {
            return Err(FiveLoopBananaBoundaryError::WrongAuxiliaryLayout { position });
        }
    }
    Ok(mass)
}
