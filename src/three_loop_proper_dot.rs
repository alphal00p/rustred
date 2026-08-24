//! Guarded scalar-dot descent in the five-line genuine tetrahedron sector.
//!
//! The canonical five-line sector (`F5`, mask 31) has two active-edge orbits.
//! At a central-edge seed `a-e1`, the fixed raw-row weight matrix
//! `[3,0,0; 2,-1,0; 2,0,-1]` isolates a pivot
//! `6*(a1-1)*m2*I(a)`.  At an outer-edge seed `a-e2`, `E21+E22` isolates
//! `3*(a2-1)*m2*I(a)`.  Every other term is strictly lower.
//!
//! No analogous one-seed scalar recurrence exists for the four-cycle (`B4`,
//! mask 43): the exact nine-column constraint matrix has rank nine when both
//! inactive numerators and all same-dot transfers are forbidden.  Undotted B4
//! is a terminal, while dotted B4 inputs return a typed unsupported error.
//!
//! This module is a one-step scalar rewrite, not numerator closure.  Together
//! with the six-line top-dot step it descends all F5 scalar dots.  It does not
//! by itself close arbitrary scalar three-loop reduction because top/F5 steps
//! can still reach dotted B4.

use std::cmp::Ordering;
use std::fmt;

use crate::ibp::IbpGenerator;
use crate::three_loop::{THREE_LOOP_TETRAHEDRON_ROUTINGS, equal_mass_three_loop_tetrahedron};
use crate::{
    Coefficient, Denominator, ExactRational, FamilyError, Integral, LinearCombination, VacuumFamily,
};

pub const THREE_LOOP_F5_MASK: u8 = 31;
pub const THREE_LOOP_B4_MASK: u8 = 43;
/// Exact raw-term reservation for the larger central-orbit formula.
///
/// The five selected native rows emit 38 derivative terms plus three diagonal
/// dimension terms before raw collection. The outer formula emits 14 plus one,
/// so 41 is the exact worst-case coefficient-work preflight for one rewrite.
pub const THREE_LOOP_PROPER_DOT_RAW_TERM_BOUND: usize = 41;

/// Numerators of the central-orbit raw-IBP weights.  Each entry multiplies the
/// native `d/dk_i . k_j` identity at `a-e1` (there is no common denominator).
pub const THREE_LOOP_F5_CENTRAL_IBP_WEIGHTS: [[i8; 3]; 3] = [[3, 0, 0], [2, -1, 0], [2, 0, -1]];

/// Numerators of the outer-orbit raw-IBP weights at `a-e2`: `E21+E22`.
pub const THREE_LOOP_F5_OUTER_IBP_WEIGHTS: [[i8; 3]; 3] = [[0, 0, 0], [1, 1, 0], [0, 0, 0]];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreeLoopProperDotConfig {
    /// Maximum raw identity terms constructed before collection.
    /// Both branches are conservatively charged the exact 41-term maximum.
    pub max_raw_terms: usize,
}

impl Default for ThreeLoopProperDotConfig {
    fn default() -> Self {
        Self {
            max_raw_terms: THREE_LOOP_PROPER_DOT_RAW_TERM_BOUND,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ThreeLoopProperDotSector {
    F5,
    B4,
}

impl ThreeLoopProperDotSector {
    pub const fn mask(self) -> u8 {
        match self {
            Self::F5 => THREE_LOOP_F5_MASK,
            Self::B4 => THREE_LOOP_B4_MASK,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreeLoopProperDotProvenance {
    seed_lowered_position: usize,
    raw_ibp_weights: [[i8; 3]; 3],
}

impl ThreeLoopProperDotProvenance {
    pub const fn seed_lowered_position(self) -> usize {
        self.seed_lowered_position
    }

    pub const fn raw_ibp_weights(self) -> [[i8; 3]; 3] {
        self.raw_ibp_weights
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLoopProperDotRewrite {
    sector: ThreeLoopProperDotSector,
    target: Integral,
    seed: Integral,
    provenance: ThreeLoopProperDotProvenance,
    rhs: LinearCombination,
}

impl ThreeLoopProperDotRewrite {
    pub const fn sector(&self) -> ThreeLoopProperDotSector {
        self.sector
    }

    /// Symmetry-oriented target with its pivot line recorded by provenance.
    pub fn target(&self) -> &Integral {
        &self.target
    }

    pub fn seed(&self) -> &Integral {
        &self.seed
    }

    pub const fn provenance(&self) -> ThreeLoopProperDotProvenance {
        self.provenance
    }

    pub fn rhs(&self) -> &LinearCombination {
        &self.rhs
    }

    pub fn into_rhs(self) -> LinearCombination {
        self.rhs
    }
}

#[derive(Clone, Debug)]
pub struct ThreeLoopProperDotReducer {
    family: VacuumFamily,
    config: ThreeLoopProperDotConfig,
    mass: Coefficient,
}

impl ThreeLoopProperDotReducer {
    pub fn build(config: ThreeLoopProperDotConfig) -> Result<Self, ThreeLoopProperDotError> {
        Self::new(equal_mass_three_loop_tetrahedron()?, config)
    }

    pub fn new(
        family: VacuumFamily,
        config: ThreeLoopProperDotConfig,
    ) -> Result<Self, ThreeLoopProperDotError> {
        let (_, mass) = validate_family(&family)?;
        Ok(Self {
            family,
            config,
            mass,
        })
    }

    pub fn family(&self) -> &VacuumFamily {
        &self.family
    }

    pub const fn config(&self) -> ThreeLoopProperDotConfig {
        self.config
    }

    /// Rewrite one dotted F5/B4 scalar integral.
    ///
    /// `Ok(None)` denotes either undotted genuine proper corner. Inputs in any
    /// other sector, or with inactive numerators, are typed domain errors.
    pub fn rewrite_once(
        &self,
        integral: &Integral,
    ) -> Result<Option<ThreeLoopProperDotRewrite>, ThreeLoopProperDotError> {
        let (sector, target, provenance) = self.oriented_target(integral)?;
        if target.powers().iter().all(|&power| power <= 1) {
            return Ok(None);
        }
        self.validate_pivot_guard(&target, provenance)?;
        if self.config.max_raw_terms < THREE_LOOP_PROPER_DOT_RAW_TERM_BOUND {
            return Err(ThreeLoopProperDotError::ResourceLimit {
                resource: "raw identity terms",
                requested: THREE_LOOP_PROPER_DOT_RAW_TERM_BOUND,
                limit: self.config.max_raw_terms,
            });
        }

        let seed = checked_shift(&target, unit_lowering(provenance.seed_lowered_position))?;
        self.preflight_weighted_raw_ibp(&seed, provenance)?;
        // Generate the fixed finite row set only after every domain, pivot,
        // resource, and lowering guard has passed.  This also makes the native
        // row sum the sole coefficient oracle instead of duplicating a fragile
        // hand-expanded formula.
        let weighted = self.weighted_raw_ibp_at_seed(&seed, provenance)?;
        let pivot = weighted.coefficient(&target).cloned().ok_or_else(|| {
            ThreeLoopProperDotError::MissingExpectedPivot {
                target: target.clone(),
            }
        })?;
        let expected_pivot = self.recurrence_denominator(&target, provenance);
        if pivot != expected_pivot {
            return Err(ThreeLoopProperDotError::UnexpectedPivotCoefficient {
                target: target.clone(),
                expected: expected_pivot,
                actual: pivot,
            });
        }
        let mut rhs = LinearCombination::new();
        for (raw_output, coefficient) in weighted.terms() {
            if raw_output == &target {
                continue;
            }
            let Some(canonical) = self.family.try_canonicalize(raw_output)? else {
                continue;
            };
            rhs.add_term(canonical, -(coefficient / &pivot));
        }

        for output in rhs.terms().keys() {
            if output.numerator_degree() != 0 {
                return Err(ThreeLoopProperDotError::UnexpectedNumerator {
                    target: target.clone(),
                    output: output.clone(),
                });
            }
            if compare_integrals_exact(&self.family, output, &target) != Ordering::Less {
                return Err(ThreeLoopProperDotError::NonDescendingTerm {
                    target: target.clone(),
                    output: output.clone(),
                });
            }
        }

        Ok(Some(ThreeLoopProperDotRewrite {
            sector,
            target,
            seed,
            provenance,
            rhs,
        }))
    }

    /// The selected fixed native raw-row combination at the oriented seed.
    pub fn raw_ibp(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ThreeLoopProperDotError> {
        let (_, target, provenance) = self.guarded_target(integral)?;
        let seed = checked_shift(&target, unit_lowering(provenance.seed_lowered_position))?;
        self.preflight_weighted_raw_ibp(&seed, provenance)?;
        self.weighted_raw_ibp_at_seed(&seed, provenance)
    }

    /// Independently expanded equation predicted by the fixed weight matrix.
    pub fn expected_raw_ibp(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ThreeLoopProperDotError> {
        let (_, target, provenance) = self.guarded_target(integral)?;
        let seed = checked_shift(&target, unit_lowering(provenance.seed_lowered_position))?;
        self.preflight_weighted_raw_ibp(&seed, provenance)?;
        self.expand_expected_raw_ibp(&seed, provenance)
    }

    pub fn validate_raw_ibp_provenance(
        &self,
        integral: &Integral,
    ) -> Result<(), ThreeLoopProperDotError> {
        let actual = self.raw_ibp(integral)?;
        let expected = self.expected_raw_ibp(integral)?;
        if actual != expected {
            return Err(ThreeLoopProperDotError::RawIbpProvenanceMismatch {
                target: self.guarded_target(integral)?.1,
                expected,
                actual,
            });
        }
        Ok(())
    }

    fn weighted_raw_ibp_at_seed(
        &self,
        seed: &Integral,
        provenance: ThreeLoopProperDotProvenance,
    ) -> Result<LinearCombination, ThreeLoopProperDotError> {
        let mut weighted = LinearCombination::new();
        let generator = IbpGenerator::new(&self.family);
        for differentiated_loop in 0..3 {
            for contraction_loop in 0..3 {
                let weight = provenance.raw_ibp_weights[differentiated_loop][contraction_loop];
                if weight == 0 {
                    continue;
                }
                let identity = generator.try_generate_raw_identity(
                    seed,
                    differentiated_loop,
                    contraction_loop,
                )?;
                let coefficient = self.family.coefficients().integer(i64::from(weight));
                weighted.add_scaled(&identity.equation, &coefficient);
            }
        }
        Ok(weighted)
    }

    /// Check every shift used by every selected row before coefficient work.
    fn preflight_weighted_raw_ibp(
        &self,
        seed: &Integral,
        provenance: ThreeLoopProperDotProvenance,
    ) -> Result<(), ThreeLoopProperDotError> {
        for differentiated_loop in 0..3 {
            for contraction_loop in 0..3 {
                if provenance.raw_ibp_weights[differentiated_loop][contraction_loop] == 0 {
                    continue;
                }
                for (denominator, &power) in seed.powers().iter().enumerate() {
                    if power == 0 {
                        continue;
                    }
                    let (has_constant, denominator_support) =
                        self.family.derivative_contraction_support(
                            denominator,
                            differentiated_loop,
                            contraction_loop,
                        );
                    if has_constant {
                        checked_indexed_shift(seed, &[(denominator, 1)])?;
                    }
                    for cancelled in denominator_support {
                        checked_indexed_shift(seed, &[(denominator, 1), (cancelled, -1)])?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Duplicate the small native derivative expansion rather than invoking
    /// `IbpGenerator`, so provenance validation compares independent paths.
    fn expand_expected_raw_ibp(
        &self,
        seed: &Integral,
        provenance: ThreeLoopProperDotProvenance,
    ) -> Result<LinearCombination, ThreeLoopProperDotError> {
        let mut expected = LinearCombination::new();
        for differentiated_loop in 0..3 {
            for contraction_loop in 0..3 {
                let weight = provenance.raw_ibp_weights[differentiated_loop][contraction_loop];
                if weight == 0 {
                    continue;
                }
                let weight = self.family.coefficients().integer(i64::from(weight));
                if differentiated_loop == contraction_loop {
                    expected.add_term(seed.clone(), &weight * self.family.dimension());
                }
                for (denominator, &power) in seed.powers().iter().enumerate() {
                    if power == 0 {
                        continue;
                    }
                    let contraction = self.family.derivative_contraction(
                        denominator,
                        differentiated_loop,
                        contraction_loop,
                    );
                    let derivative = self.family.coefficients().integer(-i64::from(power));
                    let row_factor = &weight * &derivative;
                    if !contraction.constant.is_zero() {
                        expected.add_term(
                            checked_indexed_shift(seed, &[(denominator, 1)])?,
                            &row_factor * &contraction.constant,
                        );
                    }
                    for (cancelled, rational) in contraction
                        .denominator_coefficients
                        .iter()
                        .copied()
                        .enumerate()
                    {
                        if rational.is_zero() {
                            continue;
                        }
                        expected.add_term(
                            checked_indexed_shift(seed, &[(denominator, 1), (cancelled, -1)])?,
                            self.family
                                .coefficients()
                                .scale_rational(&row_factor, rational),
                        );
                    }
                }
            }
        }
        Ok(expected)
    }

    fn guarded_target(
        &self,
        integral: &Integral,
    ) -> Result<
        (
            ThreeLoopProperDotSector,
            Integral,
            ThreeLoopProperDotProvenance,
        ),
        ThreeLoopProperDotError,
    > {
        let (sector, target, provenance) = self.oriented_target(integral)?;
        self.validate_pivot_guard(&target, provenance)?;
        Ok((sector, target, provenance))
    }

    fn oriented_target(
        &self,
        integral: &Integral,
    ) -> Result<
        (
            ThreeLoopProperDotSector,
            Integral,
            ThreeLoopProperDotProvenance,
        ),
        ThreeLoopProperDotError,
    > {
        if integral.powers().len() != 6 {
            return Err(ThreeLoopProperDotError::WrongIntegralArity {
                expected: 6,
                actual: integral.powers().len(),
            });
        }
        if let Some((position, &power)) = integral
            .powers()
            .iter()
            .enumerate()
            .find(|(_, power)| **power < 0)
        {
            return Err(ThreeLoopProperDotError::UnexpectedNumeratorInput {
                integral: integral.clone(),
                position,
                power,
            });
        }
        let sector = match canonical_sector_mask(&self.family, integral) {
            Some(THREE_LOOP_F5_MASK) => ThreeLoopProperDotSector::F5,
            Some(THREE_LOOP_B4_MASK) => ThreeLoopProperDotSector::B4,
            actual => {
                return Err(ThreeLoopProperDotError::OutsideGenuineProperSector {
                    integral: integral.clone(),
                    canonical_mask: actual,
                });
            }
        };

        if sector == ThreeLoopProperDotSector::B4
            && integral.powers().iter().any(|&power| power > 1)
        {
            return Err(ThreeLoopProperDotError::UnsupportedDottedB4 {
                integral: integral.clone(),
            });
        }

        let canonical_mask = sector.mask();
        let line_one = self
            .family
            .symmetries()
            .iter()
            .filter_map(|permutation| {
                let candidate = Integral::new(
                    permutation
                        .iter()
                        .map(|&source| integral.powers()[source])
                        .collect::<Vec<_>>(),
                );
                (sector_mask(&candidate) == canonical_mask && candidate.powers()[0] > 1)
                    .then_some(candidate)
            })
            .max();
        if let Some(oriented) = line_one {
            return Ok((sector, oriented, central_provenance()));
        }

        // F5's central edge (D1) is a singleton stabilizer orbit; its four
        // outer edges form the second orbit. Orient an outer dot to D2 and use
        // the independently derived E21+E22 combination at a-e2.
        if sector == ThreeLoopProperDotSector::F5 {
            let line_two = self
                .family
                .symmetries()
                .iter()
                .filter_map(|permutation| {
                    let candidate = Integral::new(
                        permutation
                            .iter()
                            .map(|&source| integral.powers()[source])
                            .collect::<Vec<_>>(),
                    );
                    (sector_mask(&candidate) == canonical_mask && candidate.powers()[1] > 1)
                        .then_some(candidate)
                })
                .max();
            if let Some(oriented) = line_two {
                return Ok((sector, oriented, outer_provenance()));
            }
        }

        // No dot needs a pivot: retain a deterministic representative inside
        // the canonical sector stabilizer for the terminal corner path.
        let terminal = self
            .family
            .symmetries()
            .iter()
            .map(|permutation| {
                Integral::new(
                    permutation
                        .iter()
                        .map(|&source| integral.powers()[source])
                        .collect::<Vec<_>>(),
                )
            })
            .filter(|candidate| sector_mask(candidate) == canonical_mask)
            .max()
            .expect("the canonical sector has at least one symmetry image");
        Ok((sector, terminal, central_provenance()))
    }

    fn validate_pivot_guard(
        &self,
        target: &Integral,
        provenance: ThreeLoopProperDotProvenance,
    ) -> Result<(), ThreeLoopProperDotError> {
        let position = provenance.seed_lowered_position;
        if target.powers()[position] <= 1 {
            return Err(ThreeLoopProperDotError::PivotGuardNotSatisfied {
                integral: target.clone(),
                position,
                power: target.powers()[position],
            });
        }
        Ok(())
    }

    fn recurrence_denominator(
        &self,
        target: &Integral,
        provenance: ThreeLoopProperDotProvenance,
    ) -> Coefficient {
        let power = i64::from(target.powers()[provenance.seed_lowered_position]);
        let multiplier = match provenance.seed_lowered_position {
            0 => 6,
            1 => 3,
            _ => unreachable!("F5 proper-dot recurrence has two proved branches"),
        };
        &self.mass * &self.family.coefficients().integer(multiplier * (power - 1))
    }
}

const fn central_provenance() -> ThreeLoopProperDotProvenance {
    ThreeLoopProperDotProvenance {
        seed_lowered_position: 0,
        raw_ibp_weights: THREE_LOOP_F5_CENTRAL_IBP_WEIGHTS,
    }
}

const fn outer_provenance() -> ThreeLoopProperDotProvenance {
    ThreeLoopProperDotProvenance {
        seed_lowered_position: 1,
        raw_ibp_weights: THREE_LOOP_F5_OUTER_IBP_WEIGHTS,
    }
}

fn unit_lowering(position: usize) -> [i32; 6] {
    let mut shift = [0_i32; 6];
    shift[position] = -1;
    shift
}

fn sector_mask(integral: &Integral) -> u8 {
    integral
        .powers()
        .iter()
        .enumerate()
        .fold(0_u8, |mask, (position, &power)| {
            mask | (u8::from(power > 0) << position)
        })
}

fn canonical_sector_mask(family: &VacuumFamily, integral: &Integral) -> Option<u8> {
    let boolean = Integral::new(
        integral
            .powers()
            .iter()
            .map(|&power| i32::from(power > 0))
            .collect::<Vec<_>>(),
    );
    family
        .canonicalize(&boolean)
        .map(|value| sector_mask(&value))
}

fn compare_integrals_exact(family: &VacuumFamily, left: &Integral, right: &Integral) -> Ordering {
    fn hardness<'a>(
        family: &VacuumFamily,
        integral: &'a Integral,
    ) -> (usize, u64, u64, u128, &'a [i32]) {
        let mut active = 0_usize;
        let mut sector = 0_u128;
        let mut physical = 0_u32;
        for (position, &power) in integral.powers().iter().enumerate() {
            if !family.is_propagator(position) {
                continue;
            }
            if power > 0 {
                active += 1;
                sector |= 1_u128 << physical;
            }
            physical += 1;
        }
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
        (active, dots + numerators, dots, sector, integral.powers())
    }
    hardness(family, left).cmp(&hardness(family, right))
}

fn checked_shift(
    integral: &Integral,
    shift: [i32; 6],
) -> Result<Integral, ThreeLoopProperDotError> {
    integral
        .checked_shifted(
            &shift
                .into_iter()
                .enumerate()
                .filter(|(_, value)| *value != 0)
                .collect::<Vec<_>>(),
        )
        .ok_or_else(|| ThreeLoopProperDotError::ExponentOverflow {
            integral: integral.clone(),
            shift,
        })
}

fn checked_indexed_shift(
    integral: &Integral,
    shifts: &[(usize, i32)],
) -> Result<Integral, ThreeLoopProperDotError> {
    integral.checked_shifted(shifts).ok_or_else(|| {
        let mut combined = [0_i32; 6];
        for &(position, shift) in shifts {
            combined[position] = combined[position].saturating_add(shift);
        }
        ThreeLoopProperDotError::ExponentOverflow {
            integral: integral.clone(),
            shift: combined,
        }
    })
}

fn validate_family(
    family: &VacuumFamily,
) -> Result<(Coefficient, Coefficient), ThreeLoopProperDotError> {
    if family.loops() != 3 {
        return Err(ThreeLoopProperDotError::WrongLoopCount {
            actual: family.loops(),
        });
    }
    if family.denominator_count() != 6 {
        return Err(ThreeLoopProperDotError::WrongDenominatorCount {
            actual: family.denominator_count(),
        });
    }
    let mass = family
        .coefficients()
        .parameter("m2")
        .ok_or(ThreeLoopProperDotError::MissingParameter { name: "m2" })?;
    let dimension = family
        .coefficients()
        .parameter("d")
        .ok_or(ThreeLoopProperDotError::MissingParameter { name: "d" })?;
    if family.dimension() != &dimension {
        return Err(ThreeLoopProperDotError::WrongMomentumRouting);
    }
    for (position, denominator) in family.denominators().iter().enumerate() {
        if denominator.normalization() != Some(1) {
            return Err(ThreeLoopProperDotError::WrongPropagatorSign { position });
        }
        if denominator.shift() != &mass || denominator.shift().is_zero() {
            return Err(ThreeLoopProperDotError::UnequalMasses);
        }
        let expected = Denominator::propagator(
            THREE_LOOP_TETRAHEDRON_ROUTINGS[position]
                .iter()
                .map(|&component| ExactRational::from(i64::from(component)))
                .collect(),
            mass.clone(),
        );
        if denominator.quadratic_form() != expected.quadratic_form() {
            return Err(ThreeLoopProperDotError::WrongMomentumRouting);
        }
    }
    if family.symmetries().len() != 24 {
        return Err(ThreeLoopProperDotError::IncompleteSymmetry {
            actual: family.symmetries().len(),
        });
    }
    Ok((dimension, mass))
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeLoopProperDotError {
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
    UnexpectedNumeratorInput {
        integral: Integral,
        position: usize,
        power: i32,
    },
    OutsideGenuineProperSector {
        integral: Integral,
        canonical_mask: Option<u8>,
    },
    UnsupportedDottedB4 {
        integral: Integral,
    },
    PivotGuardNotSatisfied {
        integral: Integral,
        position: usize,
        power: i32,
    },
    ExponentOverflow {
        integral: Integral,
        shift: [i32; 6],
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    UnexpectedNumerator {
        target: Integral,
        output: Integral,
    },
    NonDescendingTerm {
        target: Integral,
        output: Integral,
    },
    MissingExpectedPivot {
        target: Integral,
    },
    UnexpectedPivotCoefficient {
        target: Integral,
        expected: Coefficient,
        actual: Coefficient,
    },
    RawIbpProvenanceMismatch {
        target: Integral,
        expected: LinearCombination,
        actual: LinearCombination,
    },
    Ibp(crate::IbpGenerationError),
    Family(FamilyError),
}

impl fmt::Display for ThreeLoopProperDotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLoopCount { actual } => {
                write!(
                    formatter,
                    "proper-dot recurrence needs three loops, received {actual}"
                )
            }
            Self::WrongDenominatorCount { actual } => write!(
                formatter,
                "proper-dot recurrence needs six denominators, received {actual}"
            ),
            Self::WrongMomentumRouting => formatter.write_str(
                "proper-dot recurrence requires the built-in tetrahedron routing and dimension",
            ),
            Self::WrongPropagatorSign { position } => write!(
                formatter,
                "proper-dot denominator {position} is not positive-Euclidean"
            ),
            Self::UnequalMasses => formatter.write_str(
                "proper-dot recurrence requires the common generic nonzero mass parameter m2",
            ),
            Self::MissingParameter { name } => {
                write!(
                    formatter,
                    "proper-dot family does not define parameter {name}"
                )
            }
            Self::IncompleteSymmetry { actual } => write!(
                formatter,
                "proper-dot recurrence needs all 24 tetrahedron symmetries, found {actual}"
            ),
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "proper-dot integral has {actual} powers, expected {expected}"
            ),
            Self::UnexpectedNumeratorInput {
                integral,
                position,
                power,
            } => write!(
                formatter,
                "{integral} is not scalar: inactive power {position} is {power}"
            ),
            Self::OutsideGenuineProperSector {
                integral,
                canonical_mask,
            } => write!(
                formatter,
                "{integral} is outside F5/B4 (canonical mask {canonical_mask:?})"
            ),
            Self::UnsupportedDottedB4 { integral } => write!(
                formatter,
                "dotted B4 integral {integral} has no certified one-seed scalar descent"
            ),
            Self::PivotGuardNotSatisfied {
                integral,
                position,
                power,
            } => write!(
                formatter,
                "proper-dot pivot guard a{}>1 is false for {integral} (a{}={power})",
                position + 1,
                position + 1,
            ),
            Self::ExponentOverflow { integral, shift } => write!(
                formatter,
                "proper-dot shift {shift:?} is outside the i32 exponent range for {integral}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "proper-dot {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::UnexpectedNumerator { target, output } => write!(
                formatter,
                "proper-dot recurrence for {target} unexpectedly produced numerator {output}"
            ),
            Self::NonDescendingTerm { target, output } => write!(
                formatter,
                "proper-dot recurrence for {target} contains non-descending term {output}"
            ),
            Self::MissingExpectedPivot { target } => {
                write!(
                    formatter,
                    "proper-dot raw combination has no pivot for {target}"
                )
            }
            Self::UnexpectedPivotCoefficient {
                target,
                expected,
                actual,
            } => write!(
                formatter,
                "proper-dot pivot for {target} is {actual}, expected {expected}"
            ),
            Self::RawIbpProvenanceMismatch { target, .. } => write!(
                formatter,
                "explicit proper-dot equation for {target} does not equal its raw IBP"
            ),
            Self::Ibp(error) => write!(formatter, "cannot generate proper-dot raw IBP: {error}"),
            Self::Family(error) => write!(formatter, "proper-dot family error: {error}"),
        }
    }
}

impl std::error::Error for ThreeLoopProperDotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ibp(error) => Some(error),
            Self::Family(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::IbpGenerationError> for ThreeLoopProperDotError {
    fn from(value: crate::IbpGenerationError) -> Self {
        Self::Ibp(value)
    }
}

impl From<FamilyError> for ThreeLoopProperDotError {
    fn from(value: FamilyError) -> Self {
        Self::Family(value)
    }
}
