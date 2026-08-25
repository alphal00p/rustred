use std::cmp::Ordering;
use std::collections::{HashSet, VecDeque};

use symbolica::prelude::AtomCore;

use crate::Integral;
use crate::coefficient::{Coefficient, CoefficientContext};
use crate::exact::{
    ExactRational, invert_matrix, matrix_determinant, matrix_multiply, matrix_rank,
    matrix_transpose,
};

#[derive(Clone, Debug)]
pub struct Denominator {
    quadratic_form: Vec<ExactRational>,
    shift: Coefficient,
    kind: DenominatorKind,
}

#[derive(Clone, Debug)]
enum DenominatorKind {
    Propagator {
        /// Momentum routing after removing the overall denominator sign.
        momentum: Vec<ExactRational>,
        sign: PropagatorSign,
    },
    Auxiliary,
}

/// Overall sign multiplying a physical propagator.
///
/// RustRed stores the *actual* denominator in its quadratic form and shift.
/// Thus `Negative` represents
/// `-((sum_i momentum[i] k_i)^2 + normalized_shift)`, not a change of metric
/// convention hidden from the IBP generator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PropagatorSign {
    Positive,
    Negative,
}

impl PropagatorSign {
    pub const fn normalization(self) -> i8 {
        match self {
            Self::Positive => 1,
            Self::Negative => -1,
        }
    }
}

/// A loop scalar product written as an affine linear form in the family's
/// denominator basis.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScalarProductExpansion {
    constant: Coefficient,
    denominator_coefficients: Vec<ExactRational>,
}

impl ScalarProductExpansion {
    pub fn constant(&self) -> &Coefficient {
        &self.constant
    }

    pub fn denominator_coefficients(&self) -> &[ExactRational] {
        &self.denominator_coefficients
    }
}

impl Denominator {
    /// Construct `(sum_i momentum[i] k_i)^2 + shift`.
    pub fn propagator(momentum: Vec<ExactRational>, shift: Coefficient) -> Self {
        Self::propagator_with_sign(momentum, shift, PropagatorSign::Positive)
    }

    /// Construct `-((sum_i momentum[i] k_i)^2 + shift)`.
    ///
    /// `shift` is the normalized shift before the overall minus sign.  The
    /// stored quadratic form and [`Self::shift`] are both negated, so generic
    /// scalar-product and IBP code sees the actual reversed denominator.
    pub fn reversed_propagator(momentum: Vec<ExactRational>, shift: Coefficient) -> Self {
        Self::propagator_with_sign(momentum, shift, PropagatorSign::Negative)
    }

    /// Construct a signed physical propagator from its normalized routing and
    /// shift.  [`Self::propagator`] remains the positive-sign default.
    pub fn propagator_with_sign(
        momentum: Vec<ExactRational>,
        shift: Coefficient,
        sign: PropagatorSign,
    ) -> Self {
        let normalization: ExactRational = i64::from(sign.normalization()).into();
        let mut quadratic_form = Vec::with_capacity(momentum.len() * (momentum.len() + 1) / 2);
        for left in 0..momentum.len() {
            for right in left..momentum.len() {
                let symmetry_factor: ExactRational =
                    if left == right { 1.into() } else { 2.into() };
                let routed_product = &momentum[left] * &momentum[right];
                let routed_product = &routed_product * &symmetry_factor;
                quadratic_form.push(&routed_product * &normalization);
            }
        }
        Self {
            quadratic_form,
            shift: if sign == PropagatorSign::Positive {
                shift
            } else {
                -shift
            },
            kind: DenominatorKind::Propagator { momentum, sign },
        }
    }

    /// Construct an auxiliary denominator from a scalar-product coefficient row.
    pub fn auxiliary(quadratic_form: Vec<ExactRational>, shift: Coefficient) -> Self {
        Self {
            quadratic_form,
            shift,
            kind: DenominatorKind::Auxiliary,
        }
    }

    pub fn quadratic_form(&self) -> &[ExactRational] {
        &self.quadratic_form
    }

    pub fn shift(&self) -> &Coefficient {
        &self.shift
    }

    /// Whether this basis entry is a physical propagator rather than an ISP.
    pub fn is_propagator(&self) -> bool {
        matches!(&self.kind, DenominatorKind::Propagator { .. })
    }

    /// Overall sign of a physical propagator, or `None` for an auxiliary
    /// scalar-product basis entry.
    pub fn propagator_sign(&self) -> Option<PropagatorSign> {
        match &self.kind {
            DenominatorKind::Propagator { sign, .. } => Some(*sign),
            DenominatorKind::Auxiliary => None,
        }
    }

    /// Numerical normalization (`+1` or `-1`) of a physical propagator.
    /// Auxiliary denominators deliberately have no physical normalization.
    pub fn normalization(&self) -> Option<i8> {
        self.propagator_sign().map(PropagatorSign::normalization)
    }

    /// Loop-momentum routing of a physical propagator.
    ///
    /// Auxiliary scalar-product basis entries return `None`.  Exposing the
    /// proved routing lets factorization and topology layers construct exact
    /// loop changes of variables without reverse-engineering a squared
    /// quadratic form.
    pub fn momentum(&self) -> Option<&[ExactRational]> {
        match &self.kind {
            DenominatorKind::Propagator { momentum, .. } => Some(momentum),
            DenominatorKind::Auxiliary => None,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct DenominatorLinearForm {
    pub constant: Coefficient,
    pub denominator_coefficients: Vec<ExactRational>,
}

#[derive(Clone, Debug)]
pub struct VacuumFamily {
    name: String,
    loops: usize,
    coefficients: CoefficientContext,
    dimension: Coefficient,
    denominators: Vec<Denominator>,
    inverse_basis: Vec<Vec<ExactRational>>,
    symmetries: Vec<Vec<usize>>,
    derivative_contractions: Vec<Vec<Vec<DenominatorLinearForm>>>,
    zero_sectors: HashSet<Vec<bool>>,
}

/// Finite work and memory budgets applied while constructing a vacuum family.
///
/// The standard constructors use [`Self::default`].  Callers that deliberately
/// need a larger family can opt in through
/// [`VacuumFamily::new_with_limits`] or
/// [`VacuumFamily::new_with_standard_auxiliaries_and_limits`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FamilyConstructionLimits {
    /// Maximum number of permutations retained in the generated symmetry
    /// group, including the identity.
    pub max_symmetry_permutations: usize,
    /// Maximum aggregate number of basis and sign candidates inspected while
    /// proving that all generated permutations are geometric.
    pub max_geometric_validation_attempts: usize,
    /// Maximum number of physical-sector candidates inspected while deriving
    /// the family's cached zero-sector set.
    pub max_zero_sector_candidates: usize,
}

impl Default for FamilyConstructionLimits {
    fn default() -> Self {
        Self {
            // The built-in two- through five-loop families need at most 24
            // permutations, a few thousand geometric attempts, and 512
            // physical-sector candidates.  Keep substantial headroom without
            // restoring an effectively unbounded construction path.
            max_symmetry_permutations: 65_536,
            max_geometric_validation_attempts: 16_777_216,
            max_zero_sector_candidates: 1_048_576,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FamilyError {
    ScalarProductCountOverflow {
        loops: usize,
    },
    WrongBasisSize {
        expected: usize,
        actual: usize,
    },
    WrongMomentumSize {
        expected: usize,
        actual: usize,
    },
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    DenominatorOutOfRange {
        position: usize,
        denominators: usize,
    },
    ExpectedPropagator {
        position: usize,
    },
    InvalidPermutation(Vec<usize>),
    SingularBasis(String),
    UnknownDimensionParameter(String),
    UnknownCoefficientParameter(String),
    LoopMomentumOutOfRange {
        loop_index: usize,
        loops: usize,
    },
    TooManyPhysicalPropagators {
        actual: usize,
        maximum: usize,
    },
    SymmetryPermutationLimitExceeded {
        requested: usize,
        limit: usize,
    },
    GeometricValidationLimitExceeded {
        requested: usize,
        limit: usize,
    },
    GeometricValidationAttemptCountOverflow {
        accumulated: usize,
    },
    ZeroSectorCandidateLimitExceeded {
        requested: usize,
        limit: usize,
    },
    PhysicalSectorCandidateLimitExceeded {
        requested: usize,
        limit: usize,
    },
}

impl std::fmt::Display for FamilyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ScalarProductCountOverflow { loops } => write!(
                formatter,
                "the number of scalar products for {loops} loops does not fit in usize"
            ),
            Self::WrongBasisSize { expected, actual } => write!(
                formatter,
                "a complete vacuum basis needs {expected} denominators, received {actual}"
            ),
            Self::WrongMomentumSize { expected, actual } => {
                write!(
                    formatter,
                    "loop-momentum vector has size {actual}, expected {expected}"
                )
            }
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "integral has {actual} powers, but this family needs {expected}"
            ),
            Self::DenominatorOutOfRange {
                position,
                denominators,
            } => write!(
                formatter,
                "denominator position {position} is outside a basis of size {denominators}"
            ),
            Self::ExpectedPropagator { position } => write!(
                formatter,
                "basis completion input {position} is auxiliary; only physical propagators may precede generated auxiliaries"
            ),
            Self::InvalidPermutation(permutation) => {
                write!(formatter, "invalid denominator permutation {permutation:?}")
            }
            Self::SingularBasis(message) => formatter.write_str(message),
            Self::UnknownDimensionParameter(parameter) => {
                write!(formatter, "unknown dimension parameter {parameter:?}")
            }
            Self::UnknownCoefficientParameter(parameter) => {
                write!(formatter, "unknown coefficient parameter {parameter:?}")
            }
            Self::LoopMomentumOutOfRange { loop_index, loops } => write!(
                formatter,
                "loop-momentum index {loop_index} is outside a {loops}-loop family"
            ),
            Self::TooManyPhysicalPropagators { actual, maximum } => write!(
                formatter,
                "family has {actual} physical propagators, but sector masks support at most {maximum}"
            ),
            Self::SymmetryPermutationLimitExceeded { requested, limit } => write!(
                formatter,
                "symmetry closure needs at least {requested} permutations, exceeding the construction limit {limit}"
            ),
            Self::GeometricValidationLimitExceeded { requested, limit } => write!(
                formatter,
                "geometric symmetry validation needs at least {requested} attempts, exceeding the construction limit {limit}"
            ),
            Self::GeometricValidationAttemptCountOverflow { accumulated } => write!(
                formatter,
                "geometric symmetry validation attempt count overflowed after {accumulated} attempts"
            ),
            Self::ZeroSectorCandidateLimitExceeded { requested, limit } => write!(
                formatter,
                "zero-sector discovery needs {requested} candidates, exceeding the construction limit {limit}"
            ),
            Self::PhysicalSectorCandidateLimitExceeded { requested, limit } => write!(
                formatter,
                "physical-sector enumeration needs {requested} candidates, exceeding the requested limit {limit}"
            ),
        }
    }
}

impl std::error::Error for FamilyError {}

impl VacuumFamily {
    /// Construct a complete vacuum family by appending deterministic auxiliary
    /// scalar products to an independent list of physical propagators.
    ///
    /// A vacuum family needs `loops*(loops+1)/2` denominator-basis entries,
    /// while graph topologies often contain fewer physical lines.  This helper
    /// scans the standard upper-triangular scalar-product basis and appends the
    /// first rows that increase the exact rank.  Generated auxiliaries have
    /// zero shift and are never treated as physical sectors.
    ///
    /// Symmetry permutations, when supplied, refer to the *completed* basis.
    /// Passing no generators is always safe; a graph symmetry that mixes an
    /// auxiliary with a linear combination of auxiliaries cannot be represented
    /// by RustRed's current permutation-only symmetry layer.
    pub fn new_with_standard_auxiliaries(
        name: impl Into<String>,
        loops: usize,
        coefficients: CoefficientContext,
        dimension_parameter: &str,
        propagators: Vec<Denominator>,
        symmetry_generators: Vec<Vec<usize>>,
    ) -> Result<Self, FamilyError> {
        Self::new_with_standard_auxiliaries_and_limits(
            name,
            loops,
            coefficients,
            dimension_parameter,
            propagators,
            symmetry_generators,
            FamilyConstructionLimits::default(),
        )
    }

    /// Bounded counterpart of [`Self::new_with_standard_auxiliaries`].
    pub fn new_with_standard_auxiliaries_and_limits(
        name: impl Into<String>,
        loops: usize,
        coefficients: CoefficientContext,
        dimension_parameter: &str,
        propagators: Vec<Denominator>,
        symmetry_generators: Vec<Vec<usize>>,
        limits: FamilyConstructionLimits,
    ) -> Result<Self, FamilyError> {
        let scalar_products = checked_scalar_product_count(loops)?;
        if propagators.len() > scalar_products {
            return Err(FamilyError::WrongBasisSize {
                expected: scalar_products,
                actual: propagators.len(),
            });
        }
        for (position, denominator) in propagators.iter().enumerate() {
            if !denominator.is_propagator() {
                return Err(FamilyError::ExpectedPropagator { position });
            }
            if denominator.quadratic_form.len() != scalar_products {
                return Err(FamilyError::WrongBasisSize {
                    expected: scalar_products,
                    actual: denominator.quadratic_form.len(),
                });
            }
            if denominator
                .momentum()
                .is_some_and(|momentum| momentum.len() != loops)
            {
                return Err(FamilyError::WrongMomentumSize {
                    expected: loops,
                    actual: denominator.momentum().map_or(0, |momentum| momentum.len()),
                });
            }
        }

        // Reject an impossible or over-budget physical-sector enumeration
        // before exact ranks and a completed basis are constructed.  Every
        // input above has been proved physical, so its count is final.
        preflight_zero_sector_candidates(propagators.len(), limits.max_zero_sector_candidates)?;

        let mut denominators = propagators;
        let mut rows: Vec<_> = denominators
            .iter()
            .map(|denominator| denominator.quadratic_form.clone())
            .collect();
        let mut rank = matrix_rank(rows.clone()).map_err(FamilyError::SingularBasis)?;
        if rank != rows.len() {
            return Err(FamilyError::SingularBasis(
                "physical propagator quadratic forms are linearly dependent".to_owned(),
            ));
        }

        for scalar_product in 0..scalar_products {
            if denominators.len() == scalar_products {
                break;
            }
            let mut row = vec![ExactRational::zero(); scalar_products];
            row[scalar_product] = ExactRational::one();
            let mut trial = rows.clone();
            trial.push(row.clone());
            let trial_rank = matrix_rank(trial).map_err(FamilyError::SingularBasis)?;
            if trial_rank > rank {
                denominators.push(Denominator::auxiliary(row.clone(), coefficients.zero()));
                rows.push(row);
                rank = trial_rank;
            }
        }
        if denominators.len() != scalar_products || rank != scalar_products {
            return Err(FamilyError::SingularBasis(
                "physical propagators could not be completed to a scalar-product basis".to_owned(),
            ));
        }

        Self::new_with_limits(
            name,
            loops,
            coefficients,
            dimension_parameter,
            denominators,
            symmetry_generators,
            limits,
        )
    }

    pub fn new(
        name: impl Into<String>,
        loops: usize,
        coefficients: CoefficientContext,
        dimension_parameter: &str,
        denominators: Vec<Denominator>,
        symmetry_generators: Vec<Vec<usize>>,
    ) -> Result<Self, FamilyError> {
        Self::new_with_limits(
            name,
            loops,
            coefficients,
            dimension_parameter,
            denominators,
            symmetry_generators,
            FamilyConstructionLimits::default(),
        )
    }

    /// Construct a complete vacuum family subject to explicit finite budgets.
    ///
    /// Limits are checked before the corresponding closure or sector result is
    /// retained.  Geometric-validation attempts are charged to one aggregate
    /// counter shared by every permutation in the generated closure.
    pub fn new_with_limits(
        name: impl Into<String>,
        loops: usize,
        coefficients: CoefficientContext,
        dimension_parameter: &str,
        denominators: Vec<Denominator>,
        symmetry_generators: Vec<Vec<usize>>,
        limits: FamilyConstructionLimits,
    ) -> Result<Self, FamilyError> {
        let scalar_products = checked_scalar_product_count(loops)?;
        if denominators.len() != scalar_products {
            return Err(FamilyError::WrongBasisSize {
                expected: scalar_products,
                actual: denominators.len(),
            });
        }
        for denominator in &denominators {
            if denominator.quadratic_form.len() != scalar_products {
                return Err(FamilyError::WrongBasisSize {
                    expected: scalar_products,
                    actual: denominator.quadratic_form.len(),
                });
            }
            if let Some(momentum) = denominator.momentum()
                && momentum.len() != loops
            {
                return Err(FamilyError::WrongMomentumSize {
                    expected: loops,
                    actual: momentum.len(),
                });
            }
        }

        let dimension = coefficients.parameter(dimension_parameter).ok_or_else(|| {
            FamilyError::UnknownDimensionParameter(dimension_parameter.to_owned())
        })?;
        let propagator_count = denominators
            .iter()
            .filter(|denominator| denominator.is_propagator())
            .count();
        preflight_zero_sector_candidates(propagator_count, limits.max_zero_sector_candidates)?;

        let basis: Vec<Vec<ExactRational>> = denominators
            .iter()
            .map(|denominator| denominator.quadratic_form.clone())
            .collect();
        let inverse_basis = invert_matrix(&basis).map_err(FamilyError::SingularBasis)?;
        let symmetries = permutation_closure(
            denominators.len(),
            symmetry_generators,
            limits.max_symmetry_permutations,
        )?;
        let mut geometric_validation_attempts = 0;
        for permutation in &symmetries {
            if !symmetry_is_geometric(
                loops,
                &denominators,
                permutation,
                &mut geometric_validation_attempts,
                limits.max_geometric_validation_attempts,
            )? {
                return Err(FamilyError::InvalidPermutation(permutation.clone()));
            }
        }

        let mut family = Self {
            name: name.into(),
            loops,
            coefficients,
            dimension,
            denominators,
            inverse_basis,
            symmetries,
            derivative_contractions: Vec::new(),
            zero_sectors: HashSet::new(),
        };
        family.derivative_contractions = family.build_derivative_contractions();
        family.zero_sectors =
            family.discover_zero_sectors_with_limit(limits.max_zero_sector_candidates)?;
        Ok(family)
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    /// Stable identity of the algebraic family, symmetries, and ordering.
    pub fn fingerprint(&self) -> String {
        let mut output = format!(
            "rustred-family-v2|order-v1|{}:{}|{}|{}",
            self.name.len(),
            self.name,
            self.loops,
            self.denominators.len()
        );
        output.push_str(&format!(
            "|parameters:{}",
            self.coefficients.parameter_names().len()
        ));
        for parameter in self.coefficients.parameter_names() {
            output.push('|');
            output.push_str(&format!("{}:{parameter}", parameter.len()));
        }
        let dimension = self.dimension.to_expression().to_canonical_string();
        output.push_str(&format!("|dimension:{}:{dimension}", dimension.len()));
        for denominator in &self.denominators {
            output.push('|');
            output.push(if denominator.is_propagator() {
                'P'
            } else {
                'A'
            });
            if let Some(normalization) = denominator.normalization() {
                output.push(if normalization > 0 { '+' } else { '-' });
            }
            for coefficient in &denominator.quadratic_form {
                output.push('|');
                output.push_str(&coefficient.to_string());
            }
            let shift = denominator.shift.to_expression().to_canonical_string();
            output.push('|');
            output.push_str(&format!("{}:{shift}", shift.len()));
        }
        output.push_str(&format!("|sym:{}", self.symmetries.len()));
        for permutation in &self.symmetries {
            output.push('|');
            output.push_str(
                &permutation
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            );
        }
        output
    }

    pub fn loops(&self) -> usize {
        self.loops
    }

    pub fn denominator_count(&self) -> usize {
        self.denominators.len()
    }

    pub fn propagator_count(&self) -> usize {
        self.denominators
            .iter()
            .filter(|denominator| denominator.is_propagator())
            .count()
    }

    /// Whether `position` is a physical propagator.
    ///
    /// An out-of-range position returns `false` for compatibility with this
    /// predicate's historical Boolean API.  Call [`Self::try_is_propagator`]
    /// when an invalid position must remain distinguishable from an auxiliary.
    pub fn is_propagator(&self, position: usize) -> bool {
        self.denominators
            .get(position)
            .is_some_and(Denominator::is_propagator)
    }

    /// Checked counterpart of [`Self::is_propagator`].
    pub fn try_is_propagator(&self, position: usize) -> Result<bool, FamilyError> {
        self.denominators
            .get(position)
            .map(Denominator::is_propagator)
            .ok_or(FamilyError::DenominatorOutOfRange {
                position,
                denominators: self.denominator_count(),
            })
    }

    pub fn coefficients(&self) -> &CoefficientContext {
        &self.coefficients
    }

    pub fn dimension(&self) -> &Coefficient {
        &self.dimension
    }

    pub fn denominators(&self) -> &[Denominator] {
        &self.denominators
    }

    pub fn symmetries(&self) -> &[Vec<usize>] {
        &self.symmetries
    }

    /// Sector masks proved scaleless by the current conservative criteria.
    pub fn zero_sectors(&self) -> &HashSet<Vec<bool>> {
        &self.zero_sectors
    }

    /// Enumerate every physical-propagator sector, in denominator order.
    /// Auxiliary denominators are fixed inactive and do not create sectors.
    pub fn physical_sectors(&self) -> Vec<Vec<bool>> {
        self.try_physical_sectors_with_limit(usize::MAX)
            .expect("a constructed family has a machine-sized physical-sector mask")
    }

    /// Enumerate physical sectors only when their exact count fits `limit`.
    ///
    /// The bound is checked before any sector vectors are allocated.  This is
    /// the preferred entry point for input-dependent families; the legacy
    /// [`Self::physical_sectors`] method retains its historical unbounded-call
    /// surface for source compatibility.
    pub fn try_physical_sectors_with_limit(
        &self,
        limit: usize,
    ) -> Result<Vec<Vec<bool>>, FamilyError> {
        let (propagator_positions, candidate_count) = self.physical_sector_shape()?;
        if candidate_count > limit {
            return Err(FamilyError::PhysicalSectorCandidateLimitExceeded {
                requested: candidate_count,
                limit,
            });
        }
        Ok((0..candidate_count)
            .map(|mask| {
                let mut sector = vec![false; self.denominator_count()];
                for (bit, &position) in propagator_positions.iter().enumerate() {
                    sector[position] = mask & (1 << bit) != 0;
                }
                sector
            })
            .collect())
    }

    /// Express `k_left . k_right` in the complete denominator basis.
    pub fn scalar_product_expansion(
        &self,
        left: usize,
        right: usize,
    ) -> Result<ScalarProductExpansion, FamilyError> {
        for loop_index in [left, right] {
            if loop_index >= self.loops {
                return Err(FamilyError::LoopMomentumOutOfRange {
                    loop_index,
                    loops: self.loops,
                });
            }
        }
        let row = scalar_product_index(self.loops, left, right);
        let denominator_coefficients = self.inverse_basis[row].clone();
        let mut constant = self.coefficients.zero();
        for (coefficient, denominator) in denominator_coefficients.iter().zip(&self.denominators) {
            if !coefficient.is_zero() {
                constant = &constant
                    + &self
                        .coefficients
                        .scale_rational(&denominator.shift, -coefficient);
            }
        }
        Ok(ScalarProductExpansion {
            constant,
            denominator_coefficients,
        })
    }

    /// Canonicalize a correctly sized integral, returning `None` for either a
    /// scaleless sector or a legacy wrong-arity input.
    ///
    /// Use [`Self::try_canonicalize`] to distinguish those cases without a
    /// panic or an ambiguous `None`.
    pub fn canonicalize(&self, integral: &Integral) -> Option<Integral> {
        self.try_canonicalize(integral).ok().flatten()
    }

    /// Checked canonicalization that reports a denominator-basis mismatch.
    pub fn try_canonicalize(&self, integral: &Integral) -> Result<Option<Integral>, FamilyError> {
        self.validate_integral_arity(integral)?;
        if self.is_scaleless_unchecked(integral) {
            return Ok(None);
        }

        Ok(self
            .symmetries
            .iter()
            .map(|permutation| {
                Integral::new(
                    permutation
                        .iter()
                        .map(|&position| integral.powers()[position])
                        .collect::<Vec<_>>(),
                )
            })
            .max())
    }

    /// Conservatively test scalelessness.
    ///
    /// A wrong-arity integral returns `false`: it is never silently discarded
    /// as a zero sector.  [`Self::try_is_scaleless`] exposes the typed error.
    pub fn is_scaleless(&self, integral: &Integral) -> bool {
        self.try_is_scaleless(integral).unwrap_or(false)
    }

    /// Checked scalelessness test that rejects a denominator-basis mismatch.
    pub fn try_is_scaleless(&self, integral: &Integral) -> Result<bool, FamilyError> {
        self.validate_integral_arity(integral)?;
        Ok(self.is_scaleless_unchecked(integral))
    }

    fn is_scaleless_unchecked(&self, integral: &Integral) -> bool {
        debug_assert_eq!(integral.powers().len(), self.denominator_count());
        let mut active_momenta = Vec::new();
        let mut has_scale = false;
        let mut has_auxiliary_denominator = false;
        let mut parent: Vec<usize> = (0..self.loops).collect();
        let mut active_physical = Vec::new();

        for (power, denominator) in integral.powers().iter().zip(&self.denominators) {
            if *power <= 0 {
                continue;
            }
            has_scale |= !denominator.shift.is_zero();
            if let Some(momentum) = denominator.momentum() {
                active_momenta.push(momentum.to_vec());
                let support: Vec<usize> = momentum
                    .iter()
                    .enumerate()
                    .filter_map(|(index, coefficient)| (!coefficient.is_zero()).then_some(index))
                    .collect();
                if let Some((&first, rest)) = support.split_first() {
                    for &other in rest {
                        union_components(&mut parent, first, other);
                    }
                    active_physical.push((first, !denominator.shift.is_zero()));
                }
            } else {
                has_auxiliary_denominator = true;
            }
        }

        if !has_scale {
            return true;
        }
        // Auxiliary positive powers require the full Lee zero-sector criterion.
        // Be conservative until that criterion is implemented: never discard
        // such a sector merely from the propagator rank test.
        if has_auxiliary_denominator {
            return false;
        }
        let Ok(active_rank) = matrix_rank(active_momenta) else {
            // Every physical routing was authenticated against `self.loops`
            // during construction. Be conservative if that invariant is ever
            // violated: a malformed rank input is not a zero-sector proof.
            return false;
        };
        if active_rank < self.loops {
            return true;
        }

        let mut component_has_scale = vec![false; self.loops];
        for (loop_index, scaled) in active_physical {
            let root = find_component(&mut parent, loop_index);
            component_has_scale[root] |= scaled;
        }
        for loop_index in 0..self.loops {
            if find_component(&mut parent, loop_index) == loop_index
                && !component_has_scale[loop_index]
            {
                return true;
            }
        }
        false
    }

    fn physical_sector_shape(&self) -> Result<(Vec<usize>, usize), FamilyError> {
        let propagator_positions = self
            .denominators
            .iter()
            .enumerate()
            .filter_map(|(position, denominator)| denominator.is_propagator().then_some(position))
            .collect::<Vec<_>>();
        let candidate_count = physical_sector_candidate_count(propagator_positions.len())?;
        Ok((propagator_positions, candidate_count))
    }

    fn discover_zero_sectors_with_limit(
        &self,
        limit: usize,
    ) -> Result<HashSet<Vec<bool>>, FamilyError> {
        let (propagator_positions, candidate_count) = self.physical_sector_shape()?;
        if candidate_count > limit {
            return Err(FamilyError::ZeroSectorCandidateLimitExceeded {
                requested: candidate_count,
                limit,
            });
        }
        let mut zero_sectors = HashSet::new();
        for mask in 0..candidate_count {
            let mut powers = vec![0; self.denominator_count()];
            for (bit, &position) in propagator_positions.iter().enumerate() {
                powers[position] = i32::from(mask & (1 << bit) != 0);
            }
            let integral = Integral::new(powers);
            if self.is_scaleless(&integral) {
                zero_sectors.insert(integral.powers().iter().map(|power| *power > 0).collect());
            }
        }
        Ok(zero_sectors)
    }

    /// Compare integrals using RustRed's deterministic hardness ordering.
    ///
    /// This legacy comparator remains total even for a wrong-arity value.  In
    /// that case excess positions are treated as non-propagator entries.  Use
    /// [`Self::try_compare_integrals`] when family membership must be checked.
    pub fn compare_integrals(&self, left: &Integral, right: &Integral) -> Ordering {
        fn hardness<'a>(
            family: &VacuumFamily,
            integral: &'a Integral,
        ) -> (usize, u32, u32, u128, &'a [i32]) {
            let mut active_propagators = 0;
            let mut sector = 0_u128;
            let mut physical_index = 0;
            for (index, &power) in integral.powers().iter().enumerate() {
                if !family.is_propagator(index) {
                    continue;
                }
                if power > 0 {
                    active_propagators += 1;
                    sector |= 1_u128.checked_shl(physical_index).unwrap_or(u128::MAX);
                }
                physical_index += 1;
            }
            (
                active_propagators,
                integral
                    .dot_degree()
                    .saturating_add(integral.numerator_degree()),
                integral.dot_degree(),
                sector,
                integral.powers(),
            )
        }
        hardness(self, left).cmp(&hardness(self, right))
    }

    /// Checked counterpart of [`Self::compare_integrals`].
    pub fn try_compare_integrals(
        &self,
        left: &Integral,
        right: &Integral,
    ) -> Result<Ordering, FamilyError> {
        self.validate_integral_arity(left)?;
        self.validate_integral_arity(right)?;
        Ok(self.compare_integrals(left, right))
    }

    fn validate_integral_arity(&self, integral: &Integral) -> Result<(), FamilyError> {
        let expected = self.denominator_count();
        let actual = integral.powers().len();
        if actual != expected {
            return Err(FamilyError::WrongIntegralArity { expected, actual });
        }
        Ok(())
    }

    pub(crate) fn derivative_contraction(
        &self,
        denominator: usize,
        differentiated_loop: usize,
        contraction_loop: usize,
    ) -> &DenominatorLinearForm {
        &self.derivative_contractions[denominator][differentiated_loop][contraction_loop]
    }

    /// Whether a native derivative contraction contains a nonzero constant
    /// or denominator-basis coefficient.
    ///
    /// This coefficient-free Boolean view lets bounded analytic recurrences
    /// preflight every index shift before constructing Symbolica coefficients.
    pub(crate) fn derivative_contraction_support(
        &self,
        denominator: usize,
        differentiated_loop: usize,
        contraction_loop: usize,
    ) -> (bool, Vec<usize>) {
        let contraction =
            &self.derivative_contractions[denominator][differentiated_loop][contraction_loop];
        (
            !contraction.constant.is_zero(),
            contraction
                .denominator_coefficients
                .iter()
                .enumerate()
                .filter_map(|(position, coefficient)| (!coefficient.is_zero()).then_some(position))
                .collect(),
        )
    }

    fn build_derivative_contractions(&self) -> Vec<Vec<Vec<DenominatorLinearForm>>> {
        (0..self.denominator_count())
            .map(|denominator| {
                (0..self.loops)
                    .map(|differentiated_loop| {
                        (0..self.loops)
                            .map(|contraction_loop| {
                                self.build_derivative_contraction(
                                    denominator,
                                    differentiated_loop,
                                    contraction_loop,
                                )
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect()
    }

    fn build_derivative_contraction(
        &self,
        denominator: usize,
        differentiated_loop: usize,
        contraction_loop: usize,
    ) -> DenominatorLinearForm {
        let scalar_products = self.denominator_count();
        let mut scalar_coefficients = vec![ExactRational::zero(); scalar_products];

        for left in 0..self.loops {
            for right in left..self.loops {
                let coefficient = &self.denominators[denominator].quadratic_form
                    [scalar_product_index(self.loops, left, right)];
                if coefficient.is_zero() {
                    continue;
                }
                if differentiated_loop == left {
                    let multiplicity: ExactRational = if left == right { 2 } else { 1 }.into();
                    let index = scalar_product_index(self.loops, right, contraction_loop);
                    let contribution = coefficient * &multiplicity;
                    scalar_coefficients[index] = &scalar_coefficients[index] + &contribution;
                }
                if left != right && differentiated_loop == right {
                    let index = scalar_product_index(self.loops, left, contraction_loop);
                    scalar_coefficients[index] = &scalar_coefficients[index] + coefficient;
                }
            }
        }

        let mut denominator_coefficients = vec![ExactRational::zero(); scalar_products];
        for scalar_product in 0..scalar_products {
            for target_denominator in 0..scalar_products {
                let contribution = &scalar_coefficients[scalar_product]
                    * &self.inverse_basis[scalar_product][target_denominator];
                denominator_coefficients[target_denominator] =
                    &denominator_coefficients[target_denominator] + &contribution;
            }
        }

        let mut constant = self.coefficients.zero();
        for (coefficient, target_denominator) in
            denominator_coefficients.iter().zip(&self.denominators)
        {
            if !coefficient.is_zero() {
                let contribution = self
                    .coefficients
                    .scale_rational(&target_denominator.shift, -coefficient);
                constant = &constant + &contribution;
            }
        }

        DenominatorLinearForm {
            constant,
            denominator_coefficients,
        }
    }
}

fn find_component(parent: &mut [usize], index: usize) -> usize {
    if parent[index] != index {
        parent[index] = find_component(parent, parent[index]);
    }
    parent[index]
}

fn union_components(parent: &mut [usize], left: usize, right: usize) {
    let left = find_component(parent, left);
    let right = find_component(parent, right);
    if left != right {
        parent[right] = left;
    }
}

fn checked_scalar_product_count(loops: usize) -> Result<usize, FamilyError> {
    let successor = loops
        .checked_add(1)
        .ok_or(FamilyError::ScalarProductCountOverflow { loops })?;
    let (left, right) = if loops % 2 == 0 {
        (loops / 2, successor)
    } else {
        (loops, successor / 2)
    };
    left.checked_mul(right)
        .ok_or(FamilyError::ScalarProductCountOverflow { loops })
}

fn physical_sector_candidate_count(propagator_count: usize) -> Result<usize, FamilyError> {
    if propagator_count >= usize::BITS as usize {
        return Err(FamilyError::TooManyPhysicalPropagators {
            actual: propagator_count,
            maximum: usize::BITS as usize - 1,
        });
    }
    // The range check above proves this conversion and shift are representable.
    Ok(1_usize << propagator_count)
}

fn preflight_zero_sector_candidates(
    propagator_count: usize,
    limit: usize,
) -> Result<usize, FamilyError> {
    let requested = physical_sector_candidate_count(propagator_count)?;
    if requested > limit {
        return Err(FamilyError::ZeroSectorCandidateLimitExceeded { requested, limit });
    }
    Ok(requested)
}

fn scalar_product_index(loops: usize, left: usize, right: usize) -> usize {
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    left * loops - left * left.saturating_sub(1) / 2 + (right - left)
}

fn permutation_closure(
    size: usize,
    generators: Vec<Vec<usize>>,
    limit: usize,
) -> Result<Vec<Vec<usize>>, FamilyError> {
    let identity: Vec<usize> = (0..size).collect();
    let mut checked_generators = Vec::with_capacity(generators.len() + 1);
    checked_generators.push(identity.clone());
    for generator in generators {
        let values: HashSet<usize> = generator.iter().copied().collect();
        if generator.len() != size || values.len() != size || values.iter().any(|&x| x >= size) {
            return Err(FamilyError::InvalidPermutation(generator));
        }
        checked_generators.push(generator);
    }

    if limit == 0 {
        return Err(FamilyError::SymmetryPermutationLimitExceeded {
            requested: 1,
            limit,
        });
    }

    let mut seen = HashSet::from([identity.clone()]);
    let mut queue = VecDeque::from([identity]);
    while let Some(current) = queue.pop_front() {
        for generator in &checked_generators {
            let composed: Vec<usize> = generator.iter().map(|&index| current[index]).collect();
            if !seen.contains(&composed) {
                let requested = seen.len().saturating_add(1);
                if requested > limit {
                    return Err(FamilyError::SymmetryPermutationLimitExceeded { requested, limit });
                }
                seen.insert(composed.clone());
                queue.push_back(composed);
            }
        }
    }
    let mut closure: Vec<Vec<usize>> = seen.into_iter().collect();
    closure.sort();
    Ok(closure)
}

fn symmetry_is_geometric(
    loops: usize,
    denominators: &[Denominator],
    permutation: &[usize],
    attempts: &mut usize,
    limit: usize,
) -> Result<bool, FamilyError> {
    let identity = permutation.iter().copied().eq(0..denominators.len());
    if identity {
        return Ok(true);
    }

    for (target, &source) in permutation.iter().enumerate() {
        if denominators[target].is_propagator() != denominators[source].is_propagator()
            || denominators[target].shift != denominators[source].shift
        {
            return Ok(false);
        }
    }

    let physical_targets: Vec<usize> = denominators
        .iter()
        .enumerate()
        .filter_map(|(index, denominator)| denominator.is_propagator().then_some(index))
        .collect();
    for basis_positions in FixedCombinations::new(&physical_targets, loops) {
        charge_geometric_validation_attempt(attempts, limit)?;
        let source_matrix: Vec<Vec<ExactRational>> = basis_positions
            .iter()
            .map(|&target| {
                denominators[permutation[target]]
                    .momentum()
                    .expect("kind preservation was checked")
                    .to_vec()
            })
            .collect();
        let Ok(source_inverse) = invert_matrix(&source_matrix) else {
            continue;
        };
        let target_matrix: Vec<Vec<ExactRational>> = basis_positions
            .iter()
            .map(|&target| {
                denominators[target]
                    .momentum()
                    .expect("selected a physical denominator")
                    .to_vec()
            })
            .collect();

        for signs in SignAssignments::new(loops) {
            charge_geometric_validation_attempt(attempts, limit)?;
            let signed_target: Vec<Vec<ExactRational>> = target_matrix
                .iter()
                .enumerate()
                .map(|(row, values)| {
                    let sign: ExactRational = if signs[row] { -1 } else { 1 }.into();
                    values.iter().map(|value| &sign * value).collect()
                })
                .collect();
            let Ok(transformation) = matrix_multiply(&source_inverse, &signed_target) else {
                continue;
            };
            let Ok(determinant) = matrix_determinant(&transformation) else {
                continue;
            };
            if determinant != ExactRational::one() && determinant != -ExactRational::one() {
                continue;
            }
            if permutation.iter().enumerate().all(|(target, &source)| {
                transformed_quadratic_form(
                    loops,
                    &denominators[source].quadratic_form,
                    &transformation,
                ) == denominators[target].quadratic_form
            }) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn charge_geometric_validation_attempt(
    attempts: &mut usize,
    limit: usize,
) -> Result<(), FamilyError> {
    let requested =
        attempts
            .checked_add(1)
            .ok_or(FamilyError::GeometricValidationAttemptCountOverflow {
                accumulated: *attempts,
            })?;
    if requested > limit {
        return Err(FamilyError::GeometricValidationLimitExceeded { requested, limit });
    }
    *attempts = requested;
    Ok(())
}

fn transformed_quadratic_form(
    loops: usize,
    form: &[ExactRational],
    transformation: &[Vec<ExactRational>],
) -> Vec<ExactRational> {
    let mut matrix = vec![vec![ExactRational::zero(); loops]; loops];
    for left in 0..loops {
        for right in left..loops {
            let coefficient = &form[scalar_product_index(loops, left, right)];
            if left == right {
                matrix[left][right] = coefficient.clone();
            } else {
                let half = coefficient * ExactRational::new(1, 2);
                matrix[left][right] = half.clone();
                matrix[right][left] = half;
            }
        }
    }
    let transpose =
        matrix_transpose(transformation).expect("square transformation has compatible dimensions");
    let left_product = matrix_multiply(&transpose, &matrix)
        .expect("square transformation has compatible dimensions");
    let transformed = matrix_multiply(&left_product, transformation)
        .expect("square transformation has compatible dimensions");
    let mut result = Vec::with_capacity(form.len());
    for left in 0..loops {
        for right in left..loops {
            result.push(if left == right {
                transformed[left][right].clone()
            } else {
                &transformed[left][right] * &ExactRational::from(2)
            });
        }
    }
    result
}

/// Lazily enumerate fixed-size subsets without materializing the binomial
/// candidate set before geometric-validation limits can be enforced.
struct FixedCombinations<'a> {
    values: &'a [usize],
    indices: Option<Vec<usize>>,
}

impl<'a> FixedCombinations<'a> {
    fn new(values: &'a [usize], choose: usize) -> Self {
        Self {
            values,
            indices: (choose <= values.len()).then(|| (0..choose).collect()),
        }
    }
}

impl Iterator for FixedCombinations<'_> {
    type Item = Vec<usize>;

    fn next(&mut self) -> Option<Self::Item> {
        let indices = self.indices.as_ref()?.clone();
        let output = indices.iter().map(|&index| self.values[index]).collect();
        let choose = indices.len();
        if choose == 0 {
            self.indices = None;
            return Some(output);
        }

        let mut next_indices = indices;
        let mut pivot = choose;
        while pivot > 0 {
            pivot -= 1;
            if next_indices[pivot] < self.values.len() - choose + pivot {
                next_indices[pivot] += 1;
                for index in pivot + 1..choose {
                    next_indices[index] = next_indices[index - 1] + 1;
                }
                self.indices = Some(next_indices);
                return Some(output);
            }
        }
        self.indices = None;
        Some(output)
    }
}

/// Lazily enumerate all row-sign choices with row zero as the least
/// significant bit, matching the historical integer-mask order.
struct SignAssignments {
    current: Option<Vec<bool>>,
}

impl SignAssignments {
    fn new(width: usize) -> Self {
        Self {
            current: Some(vec![false; width]),
        }
    }
}

impl Iterator for SignAssignments {
    type Item = Vec<bool>;

    fn next(&mut self) -> Option<Self::Item> {
        let output = self.current.as_ref()?.clone();
        let mut next = output.clone();
        let mut bit = 0;
        while bit < next.len() && next[bit] {
            next[bit] = false;
            bit += 1;
        }
        self.current = if bit == next.len() {
            None
        } else {
            next[bit] = true;
            Some(next)
        };
        Some(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_product_layout_is_upper_triangular() {
        assert_eq!(scalar_product_index(3, 0, 0), 0);
        assert_eq!(scalar_product_index(3, 0, 2), 2);
        assert_eq!(scalar_product_index(3, 1, 1), 3);
        assert_eq!(scalar_product_index(3, 2, 1), 4);
        assert_eq!(scalar_product_index(3, 2, 2), 5);
    }

    #[test]
    fn completes_sparse_physical_topology_with_auxiliaries() {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let mass = coefficients.parameter("m2").unwrap();
        let route = |entries: [i64; 4]| {
            entries
                .into_iter()
                .map(ExactRational::from)
                .collect::<Vec<_>>()
        };
        let propagators = vec![
            Denominator::propagator(route([1, 0, 0, 0]), mass.clone()),
            Denominator::propagator(route([0, 1, 0, 0]), mass.clone()),
            Denominator::propagator(route([0, 0, 1, 0]), mass.clone()),
            Denominator::propagator(route([0, 0, 0, 1]), mass.clone()),
            Denominator::propagator(route([1, 1, 1, 1]), mass),
        ];
        let family = VacuumFamily::new_with_standard_auxiliaries(
            "four_loop_banana_test",
            4,
            coefficients,
            "d",
            propagators,
            Vec::new(),
        )
        .unwrap();
        assert_eq!(family.denominator_count(), 10);
        assert_eq!(family.propagator_count(), 5);
        assert_eq!(family.symmetries(), &[vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]]);
        for left in 0..4 {
            for right in left..4 {
                let expansion = family.scalar_product_expansion(left, right).unwrap();
                assert_eq!(expansion.denominator_coefficients().len(), 10);
            }
        }

        let duplicate = vec![
            Denominator::propagator(route([1, 0, 0, 0]), family.coefficients().one()),
            Denominator::propagator(route([1, 0, 0, 0]), family.coefficients().one()),
        ];
        assert!(matches!(
            VacuumFamily::new_with_standard_auxiliaries(
                "dependent",
                4,
                family.coefficients().clone(),
                "d",
                duplicate,
                Vec::new(),
            ),
            Err(FamilyError::SingularBasis(_))
        ));
    }

    fn symmetric_two_loop_input() -> (CoefficientContext, Vec<Denominator>, Vec<Vec<usize>>) {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let mass = coefficients.parameter("m2").unwrap();
        let denominators = vec![
            Denominator::propagator(vec![1.into(), 0.into()], mass.clone()),
            Denominator::propagator(vec![0.into(), 1.into()], mass.clone()),
            Denominator::propagator(vec![1.into(), 1.into()], mass),
        ];
        (
            coefficients,
            denominators,
            vec![vec![1, 0, 2], vec![1, 2, 0]],
        )
    }

    fn build_bounded_two_loop(
        limits: FamilyConstructionLimits,
    ) -> Result<VacuumFamily, FamilyError> {
        let (coefficients, denominators, generators) = symmetric_two_loop_input();
        VacuumFamily::new_with_limits(
            "bounded_two_loop",
            2,
            coefficients,
            "d",
            denominators,
            generators,
            limits,
        )
    }

    #[test]
    fn symmetry_closure_limit_is_checked_before_retention() {
        let error = build_bounded_two_loop(FamilyConstructionLimits {
            max_symmetry_permutations: 5,
            ..FamilyConstructionLimits::default()
        })
        .unwrap_err();
        assert_eq!(
            error,
            FamilyError::SymmetryPermutationLimitExceeded {
                requested: 6,
                limit: 5,
            }
        );
    }

    #[test]
    fn geometric_validation_limit_is_aggregate_across_symmetries() {
        let (coefficients, denominators, generators) = symmetric_two_loop_input();
        let closure = permutation_closure(denominators.len(), generators.clone(), 6).unwrap();
        let mut first_permutation_attempts = 0;
        assert!(
            symmetry_is_geometric(
                2,
                &denominators,
                &closure[1],
                &mut first_permutation_attempts,
                usize::MAX,
            )
            .unwrap()
        );
        assert!(first_permutation_attempts > 0);

        let error = VacuumFamily::new_with_limits(
            "aggregate_geometric_budget",
            2,
            coefficients,
            "d",
            denominators,
            generators,
            FamilyConstructionLimits {
                max_geometric_validation_attempts: first_permutation_attempts,
                ..FamilyConstructionLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            FamilyError::GeometricValidationLimitExceeded {
                requested: first_permutation_attempts + 1,
                limit: first_permutation_attempts,
            }
        );
    }

    #[test]
    fn sector_limits_preflight_construction_and_public_enumeration() {
        let error = build_bounded_two_loop(FamilyConstructionLimits {
            max_zero_sector_candidates: 7,
            ..FamilyConstructionLimits::default()
        })
        .unwrap_err();
        assert_eq!(
            error,
            FamilyError::ZeroSectorCandidateLimitExceeded {
                requested: 8,
                limit: 7,
            }
        );

        let family = build_bounded_two_loop(FamilyConstructionLimits::default()).unwrap();
        assert_eq!(
            family.try_physical_sectors_with_limit(7),
            Err(FamilyError::PhysicalSectorCandidateLimitExceeded {
                requested: 8,
                limit: 7,
            })
        );
        assert_eq!(family.try_physical_sectors_with_limit(8).unwrap().len(), 8);
        assert_eq!(family.physical_sectors().len(), 8);
    }

    #[test]
    fn standard_auxiliary_constructor_forwards_limits() {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let mass = coefficients.parameter("m2").unwrap();
        let propagators = vec![
            Denominator::propagator(vec![1.into(), 0.into()], mass.clone()),
            Denominator::propagator(vec![0.into(), 1.into()], mass),
        ];
        let error = VacuumFamily::new_with_standard_auxiliaries_and_limits(
            "bounded_standard_completion",
            2,
            coefficients,
            "d",
            propagators,
            Vec::new(),
            FamilyConstructionLimits {
                max_zero_sector_candidates: 3,
                ..FamilyConstructionLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            FamilyError::ZeroSectorCandidateLimitExceeded {
                requested: 4,
                limit: 3,
            }
        );
    }

    #[test]
    fn scalar_product_count_overflow_is_typed_in_both_constructor_paths() {
        let coefficients = CoefficientContext::new(["d"]);
        let expected = FamilyError::ScalarProductCountOverflow { loops: usize::MAX };

        let complete_error = VacuumFamily::new_with_limits(
            "overflowing_complete_basis",
            usize::MAX,
            coefficients.clone(),
            "d",
            Vec::new(),
            Vec::new(),
            FamilyConstructionLimits::default(),
        )
        .unwrap_err();
        assert_eq!(complete_error, expected.clone());
        let standard_error = VacuumFamily::new_with_standard_auxiliaries_and_limits(
            "overflowing_standard_completion",
            usize::MAX,
            coefficients,
            "d",
            Vec::new(),
            Vec::new(),
            FamilyConstructionLimits::default(),
        )
        .unwrap_err();
        assert_eq!(standard_error, expected);
    }

    #[test]
    fn standard_auxiliary_sector_budget_precedes_rank_completion() {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let mass = coefficients.parameter("m2").unwrap();
        let duplicate = Denominator::propagator(vec![1.into(), 0.into()], mass);
        let error = VacuumFamily::new_with_standard_auxiliaries_and_limits(
            "early_sector_preflight",
            2,
            coefficients,
            "d",
            vec![duplicate.clone(), duplicate],
            Vec::new(),
            FamilyConstructionLimits {
                max_zero_sector_candidates: 3,
                ..FamilyConstructionLimits::default()
            },
        )
        .unwrap_err();
        assert_eq!(
            error,
            FamilyError::ZeroSectorCandidateLimitExceeded {
                requested: 4,
                limit: 3,
            }
        );
    }

    #[test]
    fn standard_auxiliary_sector_shift_is_preflighted_before_rank_completion() {
        let loops = if usize::BITS == 64 { 16 } else { 8 };
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let mass = coefficients.parameter("m2").unwrap();
        let mut momentum = vec![ExactRational::zero(); loops];
        momentum[0] = ExactRational::one();
        let duplicate = Denominator::propagator(momentum, mass);
        let propagators = vec![duplicate; usize::BITS as usize];

        let error = VacuumFamily::new_with_standard_auxiliaries_and_limits(
            "early_shift_preflight",
            loops,
            coefficients,
            "d",
            propagators,
            Vec::new(),
            FamilyConstructionLimits::default(),
        )
        .unwrap_err();
        assert_eq!(
            error,
            FamilyError::TooManyPhysicalPropagators {
                actual: usize::BITS as usize,
                maximum: usize::BITS as usize - 1,
            }
        );
    }

    #[test]
    fn geometric_validation_counter_overflow_has_a_distinct_error() {
        let mut attempts = usize::MAX;
        assert_eq!(
            charge_geometric_validation_attempt(&mut attempts, usize::MAX),
            Err(FamilyError::GeometricValidationAttemptCountOverflow {
                accumulated: usize::MAX,
            })
        );
        assert_eq!(attempts, usize::MAX);
    }
}
