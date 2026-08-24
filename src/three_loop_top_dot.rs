//! Guarded scalar-dot descent in the six-line three-loop tetrahedron sector.
//!
//! This module implements one exact recurrence step for an all-positive
//! equal-mass tetrahedron integral.  After `S4` canonicalization, every dotted
//! top-sector integral has `a1 > 1`; a fixed rational combination of the nine
//! native raw IBPs at `a-e1` then rewrites it into strictly lower integrals.
//! Six-line terms have one less total dot, while proper-sector terms are lower
//! by active-propagator count and can still be dotted.
//!
//! This is deliberately **not** a complete three-loop reducer.  In particular,
//! it does not reduce the dotted five-line or four-line genuine sectors which
//! can occur on the right-hand side, and it does not accept numerators.

use std::cmp::Ordering;
use std::fmt;

use crate::ibp::IbpGenerator;
use crate::three_loop::{THREE_LOOP_TETRAHEDRON_ROUTINGS, equal_mass_three_loop_tetrahedron};
use crate::{
    Coefficient, Denominator, ExactRational, FamilyError, Integral, LinearCombination, VacuumFamily,
};

/// Numerators of the fixed raw-IBP weights, with common denominator four.
///
/// Entry `[i][j] / 4` multiplies the native raw identity
/// `d/dk_i . k_j` generated at the seed `a-e1`.
pub const THREE_LOOP_TOP_DOT_IBP_WEIGHT_NUMERATORS: [[i8; 3]; 3] =
    [[3, -4, 2], [6, -1, -4], [0, 2, -1]];

/// Exact resource bounds for one top-dot recurrence step.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThreeLoopTopDotConfig {
    /// Maximum number of explicit formula terms inspected before collection.
    ///
    /// The proved recurrence has exactly seventeen such terms.  This is a
    /// work preflight, not a post-collection bound on distinct integrals.
    pub max_output_terms: usize,
}

impl Default for ThreeLoopTopDotConfig {
    fn default() -> Self {
        Self {
            // The explicit recurrence has seventeen terms before symmetry
            // canonicalization and collection.
            max_output_terms: 17,
        }
    }
}

/// One symmetry-oriented, strictly descending recurrence step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThreeLoopTopDotRewrite {
    target: Integral,
    rhs: LinearCombination,
}

impl ThreeLoopTopDotRewrite {
    /// Canonical all-positive dotted integral rewritten by this step.
    pub fn target(&self) -> &Integral {
        &self.target
    }

    /// Canonical right-hand side, strictly below [`Self::target`].
    pub fn rhs(&self) -> &LinearCombination {
        &self.rhs
    }

    pub fn into_rhs(self) -> LinearCombination {
        self.rhs
    }
}

/// Reusable one-step scalar top-dot rewriter for the built-in tetrahedron.
#[derive(Clone, Debug)]
pub struct ThreeLoopTopDotReducer {
    family: VacuumFamily,
    config: ThreeLoopTopDotConfig,
    dimension: Coefficient,
    mass: Coefficient,
}

impl ThreeLoopTopDotReducer {
    /// Construct the built-in positive-Euclidean equal-mass tetrahedron.
    pub fn build(config: ThreeLoopTopDotConfig) -> Result<Self, ThreeLoopTopDotError> {
        Self::new(equal_mass_three_loop_tetrahedron()?, config)
    }

    /// Authenticate and take ownership of a tetrahedron family.
    pub fn new(
        family: VacuumFamily,
        config: ThreeLoopTopDotConfig,
    ) -> Result<Self, ThreeLoopTopDotError> {
        let (dimension, mass) = validate_family(&family)?;
        Ok(Self {
            family,
            config,
            dimension,
            mass,
        })
    }

    pub fn family(&self) -> &VacuumFamily {
        &self.family
    }

    pub fn config(&self) -> ThreeLoopTopDotConfig {
        self.config
    }

    /// Apply one guarded recurrence step.
    ///
    /// `Ok(None)` is returned only for the undotted six-line corner `M6`.
    /// Every other accepted input is canonicalized and rewritten.  Inputs
    /// outside the all-positive scalar top sector are typed domain errors.
    pub fn rewrite_once(
        &self,
        integral: &Integral,
    ) -> Result<Option<ThreeLoopTopDotRewrite>, ThreeLoopTopDotError> {
        let target = self.canonical_scalar_top(integral)?;
        if target.powers().iter().all(|&power| power == 1) {
            return Ok(None);
        }
        self.validate_pivot_guard(&target)?;
        if self.config.max_output_terms < 17 {
            return Err(ThreeLoopTopDotError::ResourceLimit {
                resource: "explicit recurrence terms",
                requested: 17,
                limit: self.config.max_output_terms,
            });
        }

        let denominator = self.recurrence_denominator(&target);
        let mut rhs = LinearCombination::new();
        for (shifts, numerator) in self.explicit_rhs_numerators(&target) {
            let shifted = checked_shift(&target, shifts)?;
            let Some(canonical) = self.family.try_canonicalize(&shifted)? else {
                return Err(ThreeLoopTopDotError::UnexpectedZeroSector { integral: shifted });
            };
            rhs.add_term(canonical, &numerator / &denominator);
        }

        for output in rhs.terms().keys() {
            if output.numerator_degree() != 0 {
                return Err(ThreeLoopTopDotError::UnexpectedNumerator {
                    target: target.clone(),
                    output: output.clone(),
                });
            }
            if compare_integrals_exact(&self.family, output, &target) != Ordering::Less {
                return Err(ThreeLoopTopDotError::NonDescendingTerm {
                    target: target.clone(),
                    output: output.clone(),
                });
            }
        }

        Ok(Some(ThreeLoopTopDotRewrite { target, rhs }))
    }

    /// Replay the fixed `C`-weighted sum of all nine native raw IBPs.
    ///
    /// The input is symmetry-canonicalized first.  Unlike [`Self::rewrite_once`],
    /// the `M6` corner is a pivot-guard error because no seed `a-e1` can solve
    /// for the corner through this recurrence.
    pub fn weighted_raw_ibp(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ThreeLoopTopDotError> {
        let target = self.guarded_target(integral)?;
        let seed = checked_shift(&target, [-1, 0, 0, 0, 0, 0])?;
        let identities = IbpGenerator::new(&self.family).try_generate_raw(&seed)?;
        let mut weighted = LinearCombination::new();
        for identity in identities {
            let numerator = i64::from(
                THREE_LOOP_TOP_DOT_IBP_WEIGHT_NUMERATORS[identity.differentiated_loop]
                    [identity.contraction_loop],
            );
            let weight = self
                .family
                .coefficients()
                .rational(ExactRational::new(numerator, 4));
            weighted.add_scaled(&identity.equation, &weight);
        }
        Ok(weighted)
    }

    /// Build the explicit uncanonicalized equation predicted by the recurrence.
    ///
    /// It is normalized as
    /// `(a1-1)*m2*I(a) - 1/2*sum_delta b_delta I(a+delta) = 0`.
    pub fn expected_weighted_raw_ibp(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, ThreeLoopTopDotError> {
        let target = self.guarded_target(integral)?;
        let mut expected =
            LinearCombination::from_term(target.clone(), self.pivot_coefficient(&target));
        for (shifts, numerator) in self.explicit_rhs_numerators(&target) {
            expected.add_term(
                checked_shift(&target, shifts)?,
                self.family
                    .coefficients()
                    .scale_rational(&numerator, ExactRational::new(-1, 2)),
            );
        }
        Ok(expected)
    }

    /// Authenticate the explicit formula against independently generated IBPs.
    pub fn validate_raw_ibp_provenance(
        &self,
        integral: &Integral,
    ) -> Result<(), ThreeLoopTopDotError> {
        let actual = self.weighted_raw_ibp(integral)?;
        let expected = self.expected_weighted_raw_ibp(integral)?;
        if actual != expected {
            return Err(ThreeLoopTopDotError::RawIbpProvenanceMismatch {
                target: self.guarded_target(integral)?,
                expected,
                actual,
            });
        }
        Ok(())
    }

    fn guarded_target(&self, integral: &Integral) -> Result<Integral, ThreeLoopTopDotError> {
        let target = self.canonical_scalar_top(integral)?;
        self.validate_pivot_guard(&target)?;
        Ok(target)
    }

    fn canonical_scalar_top(&self, integral: &Integral) -> Result<Integral, ThreeLoopTopDotError> {
        if integral.powers().len() != 6 {
            return Err(ThreeLoopTopDotError::WrongIntegralArity {
                expected: 6,
                actual: integral.powers().len(),
            });
        }
        if let Some((position, &power)) = integral
            .powers()
            .iter()
            .enumerate()
            .find(|(_, power)| **power <= 0)
        {
            return Err(ThreeLoopTopDotError::OutsideScalarTopSector {
                integral: integral.clone(),
                position,
                power,
            });
        }
        self.family.try_canonicalize(integral)?.ok_or_else(|| {
            ThreeLoopTopDotError::UnexpectedZeroSector {
                integral: integral.clone(),
            }
        })
    }

    fn validate_pivot_guard(&self, target: &Integral) -> Result<(), ThreeLoopTopDotError> {
        if target.powers()[0] <= 1 {
            return Err(ThreeLoopTopDotError::PivotGuardNotSatisfied {
                integral: target.clone(),
                first_power: target.powers()[0],
            });
        }
        Ok(())
    }

    fn pivot_coefficient(&self, target: &Integral) -> Coefficient {
        self.family
            .coefficients()
            .scale_integer(&self.mass, target.powers()[0] - 1)
    }

    fn recurrence_denominator(&self, target: &Integral) -> Coefficient {
        self.family
            .coefficients()
            .scale_integer(&self.pivot_coefficient(target), 2)
    }

    fn explicit_rhs_numerators(&self, target: &Integral) -> Vec<([i32; 6], Coefficient)> {
        let context = self.family.coefficients();
        let [a1, a2, a3, a4, a5, a6] = <[i32; 6]>::try_from(target.powers())
            .expect("a guarded tetrahedron target has six powers");
        let integer = |value: i64| context.integer(value);
        let scaled = |factor: i64, power: i32| integer(factor * i64::from(power));
        let a1_minus_one = i64::from(a1) - 1;
        let dimension_term = &integer(2 * a1_minus_one)
            - &context.scale_rational(&self.dimension, ExactRational::new(1, 2));

        vec![
            ([-2, 1, 0, 0, 0, 0], scaled(3, a2)),
            ([-1, -1, 1, 0, 0, 0], scaled(1, a3)),
            ([-1, 1, -1, 0, 0, 0], scaled(-2, a2)),
            ([-1, 1, 0, 0, -1, 0], scaled(-3, a2)),
            ([-1, 1, 0, 0, 0, -1], scaled(2, a2)),
            ([-1, 0, 1, 0, 0, -1], scaled(-1, a3)),
            ([-1, 0, 0, 0, -1, 1], scaled(-3, a6)),
            ([-1, 0, 0, 1, -1, 0], scaled(3, a4)),
            ([-1, 0, 0, 0, 1, -1], scaled(3, a5)),
            ([-1, 0, 0, -1, 1, 0], scaled(-3, a5)),
            ([-1, 0, 0, 1, 0, -1], scaled(-3, a4)),
            ([-1, 0, 0, -1, 0, 1], scaled(3, a6)),
            ([-1, 0, 0, 0, 0, 0], dimension_term),
            ([0, -1, 0, 0, 0, 0], integer(-2 * a1_minus_one)),
            ([0, 0, -1, 0, 0, 0], integer(a1_minus_one)),
            ([0, 0, 0, 0, -1, 0], integer(2 * a1_minus_one)),
            ([0, 0, 0, -1, 0, 0], integer(-a1_minus_one)),
        ]
    }
}

/// RustRed's public comparator deliberately saturates aggregate degrees at
/// `u32::MAX` for its legacy total-order API.  This recurrence accepts the
/// full positive `i32` index domain, where six summed dot powers still fit
/// exactly in `u64`; use the same ordering tuple without saturation when
/// certifying descent.
fn compare_integrals_exact(family: &VacuumFamily, left: &Integral, right: &Integral) -> Ordering {
    fn hardness<'a>(
        family: &VacuumFamily,
        integral: &'a Integral,
    ) -> (usize, u64, u64, u128, &'a [i32]) {
        let mut active_propagators = 0;
        let mut sector = 0_u128;
        let mut physical_index = 0_u32;
        for (position, &power) in integral.powers().iter().enumerate() {
            if !family.is_propagator(position) {
                continue;
            }
            if power > 0 {
                active_propagators += 1;
                sector |= 1_u128 << physical_index;
            }
            physical_index += 1;
        }
        let dot_degree = integral
            .powers()
            .iter()
            .map(|&power| u64::from(power.saturating_sub(1).max(0) as u32))
            .sum::<u64>();
        let numerator_degree = integral
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
            active_propagators,
            dot_degree + numerator_degree,
            dot_degree,
            sector,
            integral.powers(),
        )
    }

    hardness(family, left).cmp(&hardness(family, right))
}

fn checked_shift(integral: &Integral, shift: [i32; 6]) -> Result<Integral, ThreeLoopTopDotError> {
    let indexed = shift
        .into_iter()
        .enumerate()
        .filter(|(_, value)| *value != 0)
        .collect::<Vec<_>>();
    integral
        .checked_shifted(&indexed)
        .ok_or_else(|| ThreeLoopTopDotError::ExponentOverflow {
            integral: integral.clone(),
            shift,
        })
}

fn validate_family(
    family: &VacuumFamily,
) -> Result<(Coefficient, Coefficient), ThreeLoopTopDotError> {
    if family.loops() != 3 {
        return Err(ThreeLoopTopDotError::WrongLoopCount {
            actual: family.loops(),
        });
    }
    if family.denominator_count() != 6 {
        return Err(ThreeLoopTopDotError::WrongDenominatorCount {
            actual: family.denominator_count(),
        });
    }
    let mass = family
        .coefficients()
        .parameter("m2")
        .ok_or(ThreeLoopTopDotError::MissingParameter { name: "m2" })?;
    let dimension = family
        .coefficients()
        .parameter("d")
        .ok_or(ThreeLoopTopDotError::MissingParameter { name: "d" })?;
    if family.dimension() != &dimension {
        return Err(ThreeLoopTopDotError::WrongMomentumRouting);
    }
    for (position, denominator) in family.denominators().iter().enumerate() {
        if denominator.normalization() != Some(1) {
            return Err(ThreeLoopTopDotError::WrongPropagatorSign { position });
        }
        if denominator.shift() != &mass {
            return Err(ThreeLoopTopDotError::UnequalMasses);
        }
        let expected = Denominator::propagator(
            THREE_LOOP_TETRAHEDRON_ROUTINGS[position]
                .iter()
                .map(|&component| ExactRational::from(i64::from(component)))
                .collect(),
            mass.clone(),
        );
        if denominator.quadratic_form() != expected.quadratic_form() {
            return Err(ThreeLoopTopDotError::WrongMomentumRouting);
        }
    }
    if family.symmetries().len() != 24 {
        return Err(ThreeLoopTopDotError::IncompleteSymmetry {
            actual: family.symmetries().len(),
        });
    }
    Ok((dimension, mass))
}

/// Typed topology, domain, guard, resource, and exponent failures.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ThreeLoopTopDotError {
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
    ExponentOverflow {
        integral: Integral,
        shift: [i32; 6],
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
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
    RawIbpProvenanceMismatch {
        target: Integral,
        expected: LinearCombination,
        actual: LinearCombination,
    },
    Ibp(crate::IbpGenerationError),
    Family(FamilyError),
}

impl fmt::Display for ThreeLoopTopDotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLoopCount { actual } => {
                write!(
                    formatter,
                    "top-dot recurrence needs three loops, received {actual}"
                )
            }
            Self::WrongDenominatorCount { actual } => write!(
                formatter,
                "top-dot recurrence needs six denominators, received {actual}"
            ),
            Self::WrongMomentumRouting => formatter.write_str(
                "top-dot recurrence requires the built-in tetrahedron routing and dimension",
            ),
            Self::WrongPropagatorSign { position } => write!(
                formatter,
                "top-dot denominator {position} is not positive-Euclidean"
            ),
            Self::UnequalMasses => formatter.write_str(
                "top-dot recurrence requires the common generic nonzero mass parameter m2",
            ),
            Self::MissingParameter { name } => {
                write!(formatter, "top-dot family does not define parameter {name}")
            }
            Self::IncompleteSymmetry { actual } => write!(
                formatter,
                "top-dot recurrence needs all 24 tetrahedron symmetries, found {actual}"
            ),
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "top-dot integral has {actual} powers, expected {expected}"
            ),
            Self::OutsideScalarTopSector {
                integral,
                position,
                power,
            } => write!(
                formatter,
                "{integral} is outside the all-positive scalar top sector: power {position} is {power}"
            ),
            Self::PivotGuardNotSatisfied {
                integral,
                first_power,
            } => write!(
                formatter,
                "top-dot pivot guard a1>1 is false for {integral} (a1={first_power})"
            ),
            Self::ExponentOverflow { integral, shift } => write!(
                formatter,
                "top-dot shift {shift:?} is outside the i32 exponent range for {integral}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "top-dot {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::UnexpectedZeroSector { integral } => write!(
                formatter,
                "top-dot recurrence unexpectedly produced scaleless {integral}"
            ),
            Self::UnexpectedNumerator { target, output } => write!(
                formatter,
                "top-dot recurrence for {target} unexpectedly produced numerator {output}"
            ),
            Self::NonDescendingTerm { target, output } => write!(
                formatter,
                "top-dot recurrence for {target} contains non-descending term {output}"
            ),
            Self::RawIbpProvenanceMismatch { target, .. } => write!(
                formatter,
                "explicit top-dot equation for {target} does not equal its weighted raw IBPs"
            ),
            Self::Ibp(error) => write!(formatter, "cannot generate top-dot raw IBPs: {error}"),
            Self::Family(error) => write!(formatter, "top-dot family error: {error}"),
        }
    }
}

impl std::error::Error for ThreeLoopTopDotError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Ibp(error) => Some(error),
            Self::Family(error) => Some(error),
            _ => None,
        }
    }
}

impl From<crate::IbpGenerationError> for ThreeLoopTopDotError {
    fn from(value: crate::IbpGenerationError) -> Self {
        Self::Ibp(value)
    }
}

impl From<FamilyError> for ThreeLoopTopDotError {
    fn from(value: FamilyError) -> Self {
        Self::Family(value)
    }
}
