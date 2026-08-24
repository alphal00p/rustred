//! Exact scalar `(D,N)=(2,0)` certificate for the five-loop banana.
//!
//! The six-line shell has two `S6` orbits.  The triple-dot orbit descends to
//! the double-double orbit and the existing corner; the double-double orbit is
//! retained as one new bounded candidate terminal.  This module does not
//! claim master minimality or coverage beyond total physical dot degree two.
//! Proper physical subsectors and the `D<=1` top shell are delegated to the
//! existing analytic banana boundary reducer.  No numerator is accepted on
//! the public reduction surface.

use std::cmp::Ordering;
use std::fmt;

use crate::coefficient::SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT;
use crate::five_loop::equal_mass_five_loop_banana;
use crate::five_loop_boundary::{
    FIVE_LOOP_BANANA_AUXILIARY_LINE_PAIRS, FIVE_LOOP_BANANA_DENOMINATORS,
    FIVE_LOOP_BANANA_PHYSICAL_LINES, FiveLoopBananaBoundaryConfig, FiveLoopBananaBoundaryError,
    FiveLoopBananaBoundaryReducer, five_loop_banana_physical_orbit_witness,
};
use crate::{
    Coefficient, ExactRational, FamilyError, IbpGenerationError, IbpGenerator, Integral,
    LinearCombination, VacuumFamily,
};

const EXPLICIT_FORMULA_TERMS: usize = 4;
const PROVENANCE_OPERATION_BOUND: usize = 4_096;
// Every constructed coefficient in the fixed A2/B2/R/Q/X/Y certificate has
// per-variable degree at most two. Multiplication by a native raw-row
// coefficient and a subsequent conservative merge need at most degree four.
const COEFFICIENT_EXPONENT_BOUND: u128 = 4;
const RAW_ONE_DOT_IBP_ROWS: usize = 25;

/// Work limits for the finite `D=2` certificate.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FiveLoopBananaD2Config {
    /// Maximum fixed formula terms inspected before coefficient construction.
    pub max_explicit_formula_terms: usize,
    /// Maximum adjacent-transposition word length used to orient one input.
    pub max_symmetry_steps: usize,
    /// Maximum deterministic operations reserved for replaying all 25 raw
    /// one-dot-seed identities.
    pub max_provenance_operations: usize,
    /// Limits used by the exact `D<=1` and proper-sector boundary service.
    pub boundary: FiveLoopBananaBoundaryConfig,
}

impl Default for FiveLoopBananaD2Config {
    fn default() -> Self {
        Self {
            max_explicit_formula_terms: EXPLICIT_FORMULA_TERMS,
            max_symmetry_steps: 15,
            max_provenance_operations: PROVENANCE_OPERATION_BOUND,
            boundary: FiveLoopBananaBoundaryConfig::default(),
        }
    }
}

/// Finite exact reducer for scalar five-loop banana targets with `D<=2`.
#[derive(Clone, Debug)]
pub struct FiveLoopBananaD2Reducer {
    boundary: FiveLoopBananaBoundaryReducer,
    config: FiveLoopBananaD2Config,
    dimension: Coefficient,
    mass: Coefficient,
    d2_candidate_terminal: Integral,
}

impl FiveLoopBananaD2Reducer {
    pub fn build(config: FiveLoopBananaD2Config) -> Result<Self, FiveLoopBananaD2Error> {
        Self::new(equal_mass_five_loop_banana()?, config)
    }

    pub fn new(
        family: VacuumFamily,
        config: FiveLoopBananaD2Config,
    ) -> Result<Self, FiveLoopBananaD2Error> {
        // Reuse the boundary service's complete routing, sign, mass, dimension,
        // physical-count, and deterministic-auxiliary authentication.
        let boundary = FiveLoopBananaBoundaryReducer::new(family, config.boundary)?;
        let dimension = boundary
            .family()
            .coefficients()
            .parameter("d")
            .ok_or(FiveLoopBananaD2Error::MissingParameter { name: "d" })?;
        let mass = boundary
            .family()
            .coefficients()
            .parameter("m2")
            .ok_or(FiveLoopBananaD2Error::MissingParameter { name: "m2" })?;
        Ok(Self {
            boundary,
            config,
            dimension,
            mass,
            d2_candidate_terminal: Integral::from([2, 2, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]),
        })
    }

    pub fn family(&self) -> &VacuumFamily {
        self.boundary.family()
    }

    pub const fn config(&self) -> FiveLoopBananaD2Config {
        self.config
    }

    pub fn boundary(&self) -> &FiveLoopBananaBoundaryReducer {
        &self.boundary
    }

    /// Stable candidate terminal for the double-double `D=2` orbit.
    ///
    /// The projected one-dot seed layer leaves this orbit as one free column.
    /// That does not prove that deeper IBPs cannot reduce it and does not claim
    /// unrestricted master minimality.
    pub fn d2_candidate_terminal(&self) -> &Integral {
        &self.d2_candidate_terminal
    }

    /// Reduce one scalar target with no auxiliary numerator and total physical
    /// dot degree at most two.
    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        let analysis = self.analyze_public_input(integral)?;
        self.preflight_input(analysis.symmetry_steps, analysis.dot_degree)?;

        if analysis.active_lines < FIVE_LOOP_BANANA_PHYSICAL_LINES || analysis.dot_degree <= 1 {
            return Ok(self.boundary.reduce_integral(integral)?);
        }

        let canonical = self.canonical_scalar(analysis.physical);
        match canonical.powers()[..FIVE_LOOP_BANANA_PHYSICAL_LINES] {
            [3, 1, 1, 1, 1, 1] => self.reduce_triple_dot(),
            [2, 2, 1, 1, 1, 1] => Ok(LinearCombination::from_term(
                self.d2_candidate_terminal.clone(),
                self.family().coefficients().one(),
            )),
            _ => Err(FiveLoopBananaD2Error::UnexpectedD2Orbit {
                integral: canonical,
            }),
        }
    }

    /// Replay all 25 native raw IBPs at the canonical one-dot seed.
    ///
    /// A separate orbit projection shows that, together with oriented-line
    /// momentum conservation, the `(0,0)` and `(1,1)` rows give the two
    /// independent equations for the three classes `{A2,B2,R}`.  This method
    /// replays rather than re-computes that rank argument: all 25 generated rows
    /// must reduce to literal zero under the resulting parameterization.
    pub fn validate_raw_ibp_provenance(&self) -> Result<(), FiveLoopBananaD2Error> {
        self.preflight_provenance()?;
        let seed = Integral::from([2, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
        let identities = IbpGenerator::new(self.family()).try_generate_raw(&seed)?;
        if identities.len() != RAW_ONE_DOT_IBP_ROWS {
            return Err(FiveLoopBananaD2Error::RawIbpCount {
                expected: RAW_ONE_DOT_IBP_ROWS,
                actual: identities.len(),
            });
        }
        for identity in identities {
            let remainder = self.reduce_halo_combination(&identity.equation)?;
            if !remainder.is_zero() {
                return Err(FiveLoopBananaD2Error::RawIbpRemainder {
                    differentiated_loop: identity.differentiated_loop,
                    contraction_loop: identity.contraction_loop,
                    remainder,
                });
            }
        }
        Ok(())
    }

    fn reduce_triple_dot(&self) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        self.preflight_formula()?;
        let context = self.family().coefficients();
        let mut result = LinearCombination::new();
        result.add_term(
            self.d2_candidate_terminal.clone(),
            context.rational(ExactRational::new(-5, 2)),
        );
        let polynomial = &(&context.scale_integer(&(&self.dimension * &self.dimension), 25)
            - &context.scale_integer(&self.dimension, 130))
            + &context.integer(168);
        let denominator = &context.integer(48) * &(&self.mass * &self.mass);
        result.add_term(
            self.boundary.top_master().clone(),
            &polynomial / &denominator,
        );
        debug_assert!(result.terms().keys().all(|output| {
            exact_hardness(output).cmp(&exact_hardness(&Integral::from([
                3, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]))) == Ordering::Less
        }));
        Ok(result)
    }

    fn one_dot_reduction(&self) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        Ok(self.boundary.reduce_integral(&Integral::from([
            2, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))?)
    }

    fn product_two_dot_reduction(&self) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        Ok(self.boundary.reduce_integral(&Integral::from([
            2, 2, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ]))?)
    }

    fn reduce_halo_combination(
        &self,
        combination: &LinearCombination,
    ) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        let mut result = LinearCombination::new();
        for (integral, coefficient) in combination.terms() {
            let reduction = self.reduce_halo_integral(integral)?;
            result.add_scaled(&reduction, coefficient);
        }
        Ok(result)
    }

    fn reduce_halo_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        if integral.powers().len() != FIVE_LOOP_BANANA_DENOMINATORS {
            return Err(FiveLoopBananaD2Error::WrongIntegralArity {
                expected: FIVE_LOOP_BANANA_DENOMINATORS,
                actual: integral.powers().len(),
            });
        }
        let negative_auxiliaries = integral.powers()[FIVE_LOOP_BANANA_PHYSICAL_LINES..]
            .iter()
            .enumerate()
            .filter(|(_, power)| **power < 0)
            .collect::<Vec<_>>();
        if negative_auxiliaries.is_empty() {
            return self.reduce_integral(integral);
        }
        if negative_auxiliaries.len() != 1 || *negative_auxiliaries[0].1 != -1 {
            return Err(FiveLoopBananaD2Error::UnsupportedInternalHalo {
                integral: integral.clone(),
            });
        }
        if integral.powers()[FIVE_LOOP_BANANA_PHYSICAL_LINES..]
            .iter()
            .any(|power| *power > 0)
        {
            return Err(FiveLoopBananaD2Error::UnsupportedInternalHalo {
                integral: integral.clone(),
            });
        }
        let physical: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES] = integral.powers()
            [..FIVE_LOOP_BANANA_PHYSICAL_LINES]
            .try_into()
            .expect("integral arity was checked");
        if physical.iter().any(|power| *power <= 0) {
            return Ok(self.boundary.reduce_integral(integral)?);
        }
        let numerator_position = negative_auxiliaries[0].0 + FIVE_LOOP_BANANA_PHYSICAL_LINES;
        let edge = FIVE_LOOP_BANANA_AUXILIARY_LINE_PAIRS[numerator_position - 6];
        let dot_degree = exact_dot_degree(&physical);
        match dot_degree {
            2 if physical.iter().any(|power| *power == 3) => {
                let triple = physical.iter().position(|power| *power == 3).unwrap();
                if !edge.contains(&triple) {
                    return Err(FiveLoopBananaD2Error::UnsupportedInternalHalo {
                        integral: integral.clone(),
                    });
                }
                self.reduce_triple_dot_moment()
            }
            2 => {
                let dotted = physical
                    .iter()
                    .enumerate()
                    .filter_map(|(line, power)| (*power == 2).then_some(line))
                    .collect::<Vec<_>>();
                let incidence = edge.iter().filter(|line| dotted.contains(line)).count();
                match incidence {
                    2 => self.reduce_double_dot_moment(),
                    1 => self.reduce_dot_undotted_moment(),
                    0 => self.reduce_undotted_moment(),
                    _ => unreachable!(),
                }
            }
            _ => Err(FiveLoopBananaD2Error::UnsupportedInternalHalo {
                integral: integral.clone(),
            }),
        }
    }

    /// `R = 5*m2*B2 + (-10*d^2+49*d-60)/(12*m2) M`.
    fn reduce_double_dot_moment(&self) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        self.preflight_formula()?;
        let context = self.family().coefficients();
        let mut result = LinearCombination::new();
        result.add_term(
            self.d2_candidate_terminal.clone(),
            context.scale_integer(&self.mass, 5),
        );
        let polynomial = &(&context.scale_integer(&(&self.dimension * &self.dimension), -10)
            + &context.scale_integer(&self.dimension, 49))
            - &context.integer(60);
        result.add_term(
            self.boundary.top_master().clone(),
            &polynomial / &(&context.integer(12) * &self.mass),
        );
        Ok(result)
    }

    /// `Q = -(A-m2*A2)/5` for the triple-dot/undotted pair moment.
    fn reduce_triple_dot_moment(&self) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        let context = self.family().coefficients();
        let mut result = self
            .one_dot_reduction()?
            .scaled(&context.rational(ExactRational::new(-1, 5)));
        result.add_scaled(
            &self.reduce_triple_dot()?,
            &context.scale_rational(&self.mass, ExactRational::new(1, 5)),
        );
        Ok(result)
    }

    /// `X = -(A-m2*B2+R)/4` for a dotted/undotted pair moment.
    fn reduce_dot_undotted_moment(&self) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        let context = self.family().coefficients();
        let minus_quarter = context.rational(ExactRational::new(-1, 4));
        let mut inside = self.one_dot_reduction()?;
        inside.add_term(self.d2_candidate_terminal.clone(), -self.mass.clone());
        inside.add_scaled(&self.reduce_double_dot_moment()?, &context.one());
        Ok(inside.scaled(&minus_quarter))
    }

    /// `Y = -(F-m2*B2+2*X)/3` for an undotted/undotted pair moment.
    fn reduce_undotted_moment(&self) -> Result<LinearCombination, FiveLoopBananaD2Error> {
        let context = self.family().coefficients();
        let mut inside = self.product_two_dot_reduction()?;
        inside.add_term(self.d2_candidate_terminal.clone(), -self.mass.clone());
        inside.add_scaled(&self.reduce_dot_undotted_moment()?, &context.integer(2));
        Ok(inside.scaled(&context.rational(ExactRational::new(-1, 3))))
    }

    fn analyze_public_input(
        &self,
        integral: &Integral,
    ) -> Result<PublicAnalysis, FiveLoopBananaD2Error> {
        if integral.powers().len() != FIVE_LOOP_BANANA_DENOMINATORS {
            return Err(FiveLoopBananaD2Error::WrongIntegralArity {
                expected: FIVE_LOOP_BANANA_DENOMINATORS,
                actual: integral.powers().len(),
            });
        }
        for (offset, power) in integral.powers()[FIVE_LOOP_BANANA_PHYSICAL_LINES..]
            .iter()
            .enumerate()
        {
            if *power != 0 {
                return Err(FiveLoopBananaD2Error::NumeratorOrPositiveAuxiliary {
                    position: offset + FIVE_LOOP_BANANA_PHYSICAL_LINES,
                    power: *power,
                });
            }
        }
        if let Some((position, power)) = integral.powers()[..FIVE_LOOP_BANANA_PHYSICAL_LINES]
            .iter()
            .enumerate()
            .find(|(_, power)| **power < 0)
        {
            return Err(FiveLoopBananaD2Error::PhysicalNumerator {
                position,
                power: *power,
            });
        }
        let physical: [i32; FIVE_LOOP_BANANA_PHYSICAL_LINES] = integral.powers()
            [..FIVE_LOOP_BANANA_PHYSICAL_LINES]
            .try_into()
            .expect("integral arity was checked");
        let orbit = five_loop_banana_physical_orbit_witness(physical);
        Ok(PublicAnalysis {
            active_lines: physical.iter().filter(|power| **power > 0).count(),
            dot_degree: exact_dot_degree(&physical),
            symmetry_steps: orbit.adjacent_generators().len(),
            physical,
        })
    }

    fn canonical_scalar(&self, physical: [i32; 6]) -> Integral {
        let canonical = *five_loop_banana_physical_orbit_witness(physical).canonical();
        let mut powers = vec![0; FIVE_LOOP_BANANA_DENOMINATORS];
        powers[..FIVE_LOOP_BANANA_PHYSICAL_LINES].copy_from_slice(&canonical);
        Integral::new(powers)
    }

    fn preflight_input(
        &self,
        symmetry_steps: usize,
        dot_degree: u64,
    ) -> Result<(), FiveLoopBananaD2Error> {
        if dot_degree > 2 {
            return Err(FiveLoopBananaD2Error::OutOfCoverage {
                dot_degree,
                maximum: 2,
            });
        }
        self.check_resource(
            "adjacent symmetry steps",
            symmetry_steps,
            self.config.max_symmetry_steps,
        )?;
        Ok(())
    }

    fn preflight_formula(&self) -> Result<(), FiveLoopBananaD2Error> {
        self.check_resource(
            "explicit formula terms",
            EXPLICIT_FORMULA_TERMS,
            self.config.max_explicit_formula_terms,
        )?;
        if COEFFICIENT_EXPONENT_BOUND > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
            return Err(FiveLoopBananaD2Error::CoefficientExponentLimit {
                requested: COEFFICIENT_EXPONENT_BOUND,
                limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
            });
        }
        Ok(())
    }

    fn preflight_provenance(&self) -> Result<(), FiveLoopBananaD2Error> {
        self.check_resource(
            "raw-IBP provenance operations",
            PROVENANCE_OPERATION_BOUND,
            self.config.max_provenance_operations,
        )?;
        self.preflight_formula()
    }

    fn check_resource(
        &self,
        resource: &'static str,
        requested: usize,
        limit: usize,
    ) -> Result<(), FiveLoopBananaD2Error> {
        if requested > limit {
            return Err(FiveLoopBananaD2Error::ResourceLimit {
                resource,
                requested,
                limit,
            });
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug)]
struct PublicAnalysis {
    active_lines: usize,
    dot_degree: u64,
    symmetry_steps: usize,
    physical: [i32; 6],
}

fn exact_dot_degree(physical: &[i32; 6]) -> u64 {
    physical
        .iter()
        .map(|power| u64::try_from(power.saturating_sub(1).max(0)).unwrap())
        .sum()
}

fn exact_hardness(integral: &Integral) -> (usize, u64, u64, &[i32]) {
    let active = integral.powers()[..FIVE_LOOP_BANANA_PHYSICAL_LINES]
        .iter()
        .filter(|power| **power > 0)
        .count();
    let dots = integral.powers()[..FIVE_LOOP_BANANA_PHYSICAL_LINES]
        .iter()
        .map(|power| u64::try_from(power.saturating_sub(1).max(0)).unwrap())
        .sum::<u64>();
    let numerators = integral.powers()[FIVE_LOOP_BANANA_PHYSICAL_LINES..]
        .iter()
        .map(|power| u64::from(power.unsigned_abs()))
        .sum::<u64>();
    (active, dots + numerators, dots, integral.powers())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FiveLoopBananaD2Error {
    MissingParameter {
        name: &'static str,
    },
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    NumeratorOrPositiveAuxiliary {
        position: usize,
        power: i32,
    },
    PhysicalNumerator {
        position: usize,
        power: i32,
    },
    OutOfCoverage {
        dot_degree: u64,
        maximum: u64,
    },
    UnexpectedD2Orbit {
        integral: Integral,
    },
    UnsupportedInternalHalo {
        integral: Integral,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    CoefficientExponentLimit {
        requested: u128,
        limit: u128,
    },
    RawIbpRemainder {
        differentiated_loop: usize,
        contraction_loop: usize,
        remainder: LinearCombination,
    },
    RawIbpCount {
        expected: usize,
        actual: usize,
    },
    Boundary(FiveLoopBananaBoundaryError),
    Family(FamilyError),
    Ibp(IbpGenerationError),
}

impl fmt::Display for FiveLoopBananaD2Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingParameter { name } => write!(formatter, "D=2 family misses {name}"),
            Self::WrongIntegralArity { expected, actual } => {
                write!(
                    formatter,
                    "D=2 input has {actual} powers, expected {expected}"
                )
            }
            Self::NumeratorOrPositiveAuxiliary { position, power } => write!(
                formatter,
                "D=2 scalar input requires auxiliary power zero at {position}, received {power}"
            ),
            Self::PhysicalNumerator { position, power } => write!(
                formatter,
                "D=2 scalar input does not accept physical numerator {position}^{power}"
            ),
            Self::OutOfCoverage {
                dot_degree,
                maximum,
            } => write!(
                formatter,
                "five-loop banana dot degree {dot_degree} exceeds D=2 coverage {maximum}"
            ),
            Self::UnexpectedD2Orbit { integral } => {
                write!(formatter, "unexpected scalar D=2 orbit {integral}")
            }
            Self::UnsupportedInternalHalo { integral } => {
                write!(
                    formatter,
                    "unsupported D=2 provenance halo integral {integral}"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "five-loop D=2 {resource} requires {requested}, exceeding {limit}"
            ),
            Self::CoefficientExponentLimit { requested, limit } => write!(
                formatter,
                "five-loop D=2 coefficient exponent {requested} exceeds {limit}"
            ),
            Self::RawIbpRemainder {
                differentiated_loop,
                contraction_loop,
                ..
            } => write!(
                formatter,
                "five-loop D=2 raw IBP ({differentiated_loop},{contraction_loop}) has nonzero remainder"
            ),
            Self::RawIbpCount { expected, actual } => write!(
                formatter,
                "five-loop D=2 provenance generated {actual} raw IBPs, expected {expected}"
            ),
            Self::Boundary(error) => write!(formatter, "five-loop D=2 boundary error: {error}"),
            Self::Family(error) => write!(formatter, "five-loop D=2 family error: {error}"),
            Self::Ibp(error) => write!(formatter, "five-loop D=2 IBP error: {error}"),
        }
    }
}

impl std::error::Error for FiveLoopBananaD2Error {}

impl From<FiveLoopBananaBoundaryError> for FiveLoopBananaD2Error {
    fn from(value: FiveLoopBananaBoundaryError) -> Self {
        Self::Boundary(value)
    }
}

impl From<FamilyError> for FiveLoopBananaD2Error {
    fn from(value: FamilyError) -> Self {
        Self::Family(value)
    }
}

impl From<IbpGenerationError> for FiveLoopBananaD2Error {
    fn from(value: IbpGenerationError) -> Self {
        Self::Ibp(value)
    }
}
