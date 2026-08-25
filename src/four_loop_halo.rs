//! Exact affine transport for the four-loop genuine-corner IBP halo.
//!
//! A [`FourLoopGenuineWitness`](crate::FourLoopGenuineWitness) maps the active
//! squared routings of one genuine scalar corner to a frozen H/X
//! representative.  Raw corner IBPs also contain one dotted denominator and,
//! in general, one polynomial denominator-basis numerator.  Transporting those
//! terms requires the image of the *complete* ten-entry source basis, including
//! ISPs.  This module constructs and replays that affine map and expands the
//! resulting `(D,N) <= (1,1)` halo terms.
//!
//! The output is a reference-family [`LinearCombination`].  It is not yet a
//! four-loop reduction: factorized numerator sectors still need native tensor
//! closure, and genuine halo columns still need sparse IBP elimination.

use std::{array, fmt};

use crate::four_loop::FourLoopTopology;
use crate::four_loop_genuine::{
    FourLoopGenuineClassifier, FourLoopGenuineCornerType, FourLoopGenuineError,
    FourLoopGenuineWitness,
};
use crate::master_product::MasterProduct;
use crate::{
    Coefficient, Denominator, ExactRational, FamilyError, Integral, LinearCombination,
    MassiveVacuumMaster, VacuumFamily,
};

const LOOPS: usize = 4;
const BASIS: usize = 10;

/// Conservative charge for transforming ten quadratic rows and expanding each
/// transformed row through all ten entries of the frozen denominator basis.
pub const FOUR_LOOP_AFFINE_MAP_OPERATION_BOUND: u128 = 4_000;

/// Resource bounds for one authenticated affine halo map.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopHaloConfig {
    /// Maximum complete source-basis images retained by the map.  A four-loop
    /// scalar-product basis requires exactly ten.
    pub max_affine_basis_images: usize,
    /// Conservative exact-rational/coefficient operation budget charged before
    /// any affine image is constructed.
    pub max_affine_operations: usize,
    /// Maximum terms emitted while expanding one degree-one numerator.  The
    /// exact affine image has at most one constant plus ten basis entries.
    pub max_expanded_terms: usize,
}

impl Default for FourLoopHaloConfig {
    fn default() -> Self {
        Self {
            max_affine_basis_images: BASIS,
            max_affine_operations: FOUR_LOOP_AFFINE_MAP_OPERATION_BOUND as usize,
            max_expanded_terms: BASIS + 1,
        }
    }
}

/// Affine image of one source denominator-basis entry,
///
/// `D_source = constant + sum_j denominator_coefficients[j] D_reference[j]`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopAffineDenominatorImage {
    source_position: usize,
    constant: Coefficient,
    denominator_coefficients: [ExactRational; BASIS],
}

impl FourLoopAffineDenominatorImage {
    pub const fn source_position(&self) -> usize {
        self.source_position
    }

    pub const fn constant(&self) -> &Coefficient {
        &self.constant
    }

    pub const fn denominator_coefficients(&self) -> &[ExactRational; BASIS] {
        &self.denominator_coefficients
    }

    /// Test/certificate tooling may replay a deliberately altered affine
    /// constant without exposing mutable production state.
    #[doc(hidden)]
    pub fn with_constant_for_replay(&self, constant: Coefficient) -> Self {
        let mut image = self.clone();
        image.constant = constant;
        image
    }

    /// Test/certificate tooling counterpart for one denominator coefficient.
    #[doc(hidden)]
    pub fn with_denominator_coefficient_for_replay(
        &self,
        position: usize,
        coefficient: ExactRational,
    ) -> Self {
        let mut image = self.clone();
        if position < BASIS {
            image.denominator_coefficients[position] = coefficient;
        }
        image
    }
}

/// Stable column namespaces for the later inter-family halo normal form.
///
/// `GenuineRepresentative` requires its integral to be expressed in the frozen
/// H/X family selected by `corner_type`.  A factorized entry is used only after
/// its lower-loop scalar/tensor reduction has produced a canonical product.
/// Merely defining these disjoint keys does not claim that the present affine
/// mapper performs either reduction.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FourLoopHaloColumnKey {
    Scaleless,
    Factorized(MasterProduct<MassiveVacuumMaster>),
    GenuineRepresentative {
        corner_type: FourLoopGenuineCornerType,
        integral: Integral,
    },
}

impl FourLoopHaloColumnKey {
    pub const SCHEMA: &'static str = "rustred-equal-mass-euclidean-four-loop-halo-v1";

    pub fn stable_key(&self) -> String {
        match self {
            Self::Scaleless => format!("{}:zero", Self::SCHEMA),
            Self::Factorized(product) => {
                let factors = product
                    .factors()
                    .iter()
                    .map(|(master, multiplicity)| format!("{}^{multiplicity}", master.stable_key()))
                    .collect::<Vec<_>>()
                    .join("*");
                format!("{}:product:{factors}", Self::SCHEMA)
            }
            Self::GenuineRepresentative {
                corner_type,
                integral,
            } => {
                let powers = integral
                    .powers()
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                format!(
                    "{}:genuine:{}:[{powers}]",
                    Self::SCHEMA,
                    corner_type.stable_key()
                )
            }
        }
    }
}

/// Authenticated complete-basis transport into one frozen genuine-corner
/// presentation.
#[derive(Clone, Debug)]
pub struct FourLoopHaloMapper {
    config: FourLoopHaloConfig,
    source_topology: FourLoopTopology,
    source_sector_mask: u16,
    corner_type: FourLoopGenuineCornerType,
    reference_family: VacuumFamily,
    signed_line_matches: Vec<(usize, usize)>,
    images: Vec<FourLoopAffineDenominatorImage>,
}

impl FourLoopHaloMapper {
    /// Authenticate `witness` and construct the ten complete-basis affine
    /// images.  Resource bounds are checked before the reference family or any
    /// image coefficient is constructed.
    pub fn from_witness(
        classifier: &FourLoopGenuineClassifier,
        witness: &FourLoopGenuineWitness,
        config: FourLoopHaloConfig,
    ) -> Result<Self, FourLoopHaloError> {
        check_resource(
            "affine denominator-basis images",
            BASIS as u128,
            config.max_affine_basis_images as u128,
        )?;
        check_resource(
            "affine exact operations",
            FOUR_LOOP_AFFINE_MAP_OPERATION_BOUND,
            config.max_affine_operations as u128,
        )?;
        classifier.replay_witness(witness)?;

        let reference_family =
            reference_family_in_context(witness.corner_type(), classifier.family())?;
        let loop_map = witness
            .loop_map()
            .iter()
            .map(|row| row.to_vec())
            .collect::<Vec<_>>();
        let images = classifier
            .family()
            .denominators()
            .iter()
            .enumerate()
            .map(|(source_position, denominator)| {
                affine_image(source_position, denominator, &loop_map, &reference_family)
            })
            .collect::<Result<Vec<_>, FourLoopHaloError>>()?;
        let mapper = Self {
            config,
            source_topology: classifier.topology(),
            source_sector_mask: witness.source_sector_mask(),
            corner_type: witness.corner_type(),
            reference_family,
            signed_line_matches: witness
                .signed_line_matches()
                .iter()
                .map(|line| {
                    (
                        line.source_physical_position(),
                        line.reference_physical_position(),
                    )
                })
                .collect(),
            images,
        };
        mapper.replay_affine_images(classifier, witness)?;
        Ok(mapper)
    }

    pub const fn config(&self) -> FourLoopHaloConfig {
        self.config
    }

    pub const fn source_topology(&self) -> FourLoopTopology {
        self.source_topology
    }

    pub const fn source_sector_mask(&self) -> u16 {
        self.source_sector_mask
    }

    pub const fn corner_type(&self) -> FourLoopGenuineCornerType {
        self.corner_type
    }

    pub fn reference_family(&self) -> &VacuumFamily {
        &self.reference_family
    }

    pub fn images(&self) -> &[FourLoopAffineDenominatorImage] {
        &self.images
    }

    pub fn image(
        &self,
        source_position: usize,
    ) -> Result<&FourLoopAffineDenominatorImage, FourLoopHaloError> {
        self.images
            .get(source_position)
            .filter(|image| image.source_position == source_position)
            .ok_or(FourLoopHaloError::SourcePositionOutOfRange {
                position: source_position,
                denominators: BASIS,
            })
    }

    /// Independently replay every stored affine identity and every exact active
    /// physical-line image.  This performs no canonical-signature search beyond
    /// authenticating the supplied genuine-corner witness.
    pub fn replay_affine_images(
        &self,
        classifier: &FourLoopGenuineClassifier,
        witness: &FourLoopGenuineWitness,
    ) -> Result<(), FourLoopHaloError> {
        if self.source_topology != classifier.topology()
            || self.source_topology != witness.source_topology()
            || self.source_sector_mask != witness.source_sector_mask()
            || self.corner_type != witness.corner_type()
            || self.images.len() != BASIS
        {
            return Err(FourLoopHaloError::WitnessMismatch);
        }
        classifier.replay_witness(witness)?;
        let loop_map = witness
            .loop_map()
            .iter()
            .map(|row| row.to_vec())
            .collect::<Vec<_>>();

        for (position, (source, image)) in classifier
            .family()
            .denominators()
            .iter()
            .zip(&self.images)
            .enumerate()
        {
            if image.source_position != position {
                return Err(FourLoopHaloError::AffineReplayMismatch { position });
            }
            let transformed = transform_quadratic_form(source.quadratic_form(), &loop_map);
            let mut reconstructed: [ExactRational; BASIS] =
                array::from_fn(|_| ExactRational::zero());
            let mut reconstructed_shift = image.constant.clone();
            for (coefficient, reference) in image
                .denominator_coefficients
                .iter()
                .zip(self.reference_family.denominators())
            {
                if coefficient.is_zero() {
                    continue;
                }
                for (target, value) in reconstructed.iter_mut().zip(reference.quadratic_form()) {
                    let contribution = coefficient * value;
                    *target = &*target + &contribution;
                }
                reconstructed_shift = &reconstructed_shift
                    + &self
                        .reference_family
                        .coefficients()
                        .scale_rational(reference.shift(), coefficient);
            }
            if reconstructed.as_slice() != transformed.as_slice()
                || &reconstructed_shift != source.shift()
            {
                return Err(FourLoopHaloError::AffineReplayMismatch { position });
            }
        }

        for &(source, reference) in &self.signed_line_matches {
            let image = self.image(source)?;
            if !image.constant.is_zero()
                || image.denominator_coefficients.iter().enumerate().any(
                    |(position, coefficient)| {
                        let expected = if position == reference {
                            ExactRational::one()
                        } else {
                            ExactRational::zero()
                        };
                        coefficient != &expected
                    },
                )
            {
                return Err(FourLoopHaloError::ActiveLineImageMismatch {
                    source_position: source,
                    reference_position: reference,
                });
            }
        }
        Ok(())
    }

    /// Expand one raw-corner halo integral into the frozen reference basis.
    ///
    /// Accepted inputs have positive powers only on the witnessed active
    /// physical lines, total dot degree at most one, and at most one degree-one
    /// polynomial numerator on an inactive source-basis entry.  In addition to
    /// those `(D,N)` bounds, the active sector must be the corner itself or
    /// differ from it by the single pinch that can accompany a raw-corner dot.
    /// A numerator can only accompany a dot and cannot accompany a pinch.
    pub fn map_raw_halo_integral(
        &self,
        integral: &Integral,
    ) -> Result<LinearCombination, FourLoopHaloError> {
        if integral.powers().len() != BASIS {
            return Err(FourLoopHaloError::WrongIntegralArity {
                expected: BASIS,
                actual: integral.powers().len(),
            });
        }

        let mut reference_powers = [0_i32; BASIS];
        let mut dot_degree = 0_u32;
        let mut active_pinches = 0_u32;
        let mut numerator_position = None;
        for (position, &power) in integral.powers().iter().enumerate() {
            let source_active = self.source_sector_mask & (1_u16 << position) != 0;
            if power > 0 {
                if !source_active || power > 2 {
                    return Err(FourLoopHaloError::OutsideRawCornerHalo {
                        integral: integral.clone(),
                    });
                }
                dot_degree += u32::try_from(power - 1).expect("a positive i32 fits u32");
                let reference = self
                    .signed_line_matches
                    .iter()
                    .find_map(|&(source, reference)| (source == position).then_some(reference))
                    .ok_or(FourLoopHaloError::WitnessMismatch)?;
                reference_powers[reference] = power;
            } else if power < 0 {
                if source_active || power != -1 || numerator_position.replace(position).is_some() {
                    return Err(FourLoopHaloError::OutsideRawCornerHalo {
                        integral: integral.clone(),
                    });
                }
            } else if source_active {
                active_pinches += 1;
            }
        }
        if dot_degree > 1 {
            return Err(FourLoopHaloError::OutsideRawCornerHalo {
                integral: integral.clone(),
            });
        }

        // A raw identity at the scalar corner emits the corner itself, one
        // dot, one dot paired with one active-line pinch, or one dot paired
        // with one inactive-basis numerator.  `(D,N)` alone does not bound the
        // number of pinches, so authenticate that adjacency explicitly.
        let direct_raw_shape = match (dot_degree, numerator_position.is_some()) {
            (0, false) => active_pinches == 0,
            (1, false) => active_pinches <= 1,
            (1, true) => active_pinches == 0,
            _ => false,
        };
        if !direct_raw_shape {
            return Err(FourLoopHaloError::OutsideRawCornerHalo {
                integral: integral.clone(),
            });
        }

        let Some(numerator_position) = numerator_position else {
            check_resource(
                "expanded halo terms",
                1,
                self.config.max_expanded_terms as u128,
            )?;
            return Ok(LinearCombination::from_term(
                Integral::from(reference_powers),
                self.reference_family.coefficients().one(),
            ));
        };

        let image = self.image(numerator_position)?;
        let requested = u128::from(!image.constant.is_zero() as u8)
            + image
                .denominator_coefficients
                .iter()
                .filter(|coefficient| !coefficient.is_zero())
                .count() as u128;
        check_resource(
            "expanded halo terms",
            requested,
            self.config.max_expanded_terms as u128,
        )?;

        let mut output = LinearCombination::new();
        if !image.constant.is_zero() {
            output.add_term(Integral::from(reference_powers), image.constant.clone());
        }
        for (position, coefficient) in image.denominator_coefficients.iter().enumerate() {
            if coefficient.is_zero() {
                continue;
            }
            let mut powers = reference_powers;
            powers[position] -= 1;
            output.add_term(
                Integral::from(powers),
                self.reference_family.coefficients().rational(coefficient),
            );
        }
        Ok(output)
    }

    /// Test/certificate tooling may replay a deliberately altered image.
    #[doc(hidden)]
    pub fn with_affine_image_for_replay(
        &self,
        source_position: usize,
        image: FourLoopAffineDenominatorImage,
    ) -> Self {
        let mut mapper = self.clone();
        if source_position < mapper.images.len() {
            mapper.images[source_position] = image;
        }
        mapper
    }
}

fn affine_image(
    source_position: usize,
    source: &Denominator,
    loop_map: &[Vec<ExactRational>],
    reference: &VacuumFamily,
) -> Result<FourLoopAffineDenominatorImage, FourLoopHaloError> {
    let transformed = transform_quadratic_form(source.quadratic_form(), loop_map);
    let mut constant = source.shift().clone();
    let mut denominator_coefficients = array::from_fn(|_| ExactRational::zero());
    for (scalar_product, coefficient) in transformed.iter().enumerate() {
        if coefficient.is_zero() {
            continue;
        }
        let (left, right) = scalar_product_pair(scalar_product);
        let expansion = reference.scalar_product_expansion(left, right)?;
        constant = &constant
            + &reference
                .coefficients()
                .scale_rational(expansion.constant(), coefficient);
        for (target, basis_coefficient) in denominator_coefficients
            .iter_mut()
            .zip(expansion.denominator_coefficients())
        {
            let contribution = coefficient * basis_coefficient;
            *target = &*target + &contribution;
        }
    }
    Ok(FourLoopAffineDenominatorImage {
        source_position,
        constant,
        denominator_coefficients,
    })
}

/// Transform the flattened upper-triangular scalar-product coefficients under
/// `k_source = loop_map * k_reference`.  Off-diagonal entries in
/// `Denominator::quadratic_form` already multiply `k_i.k_j` directly, so both
/// ordered cross contributions are accumulated explicitly.
fn transform_quadratic_form(
    source: &[ExactRational],
    loop_map: &[Vec<ExactRational>],
) -> [ExactRational; BASIS] {
    let mut output = array::from_fn(|_| ExactRational::zero());
    for source_left in 0..LOOPS {
        for source_right in source_left..LOOPS {
            let coefficient = &source[scalar_product_index(source_left, source_right)];
            if coefficient.is_zero() {
                continue;
            }
            for reference_left in 0..LOOPS {
                for reference_right in reference_left..LOOPS {
                    let transformed = if reference_left == reference_right {
                        &loop_map[source_left][reference_left]
                            * &loop_map[source_right][reference_right]
                    } else {
                        &loop_map[source_left][reference_left]
                            * &loop_map[source_right][reference_right]
                            + &loop_map[source_left][reference_right]
                                * &loop_map[source_right][reference_left]
                    };
                    let target = scalar_product_index(reference_left, reference_right);
                    let contribution = coefficient * &transformed;
                    output[target] = &output[target] + &contribution;
                }
            }
        }
    }
    output
}

fn reference_family_in_context(
    corner_type: FourLoopGenuineCornerType,
    source: &VacuumFamily,
) -> Result<VacuumFamily, FourLoopHaloError> {
    let topology = corner_type.reference_topology();
    let mass = source
        .coefficients()
        .parameter("m2")
        .ok_or(FourLoopHaloError::MissingParameter { name: "m2" })?;
    let propagators = topology
        .routings()
        .iter()
        .map(|routing| {
            Denominator::propagator(
                routing
                    .iter()
                    .map(|&value| ExactRational::from(i64::from(value)))
                    .collect(),
                mass.clone(),
            )
        })
        .collect();
    Ok(VacuumFamily::new_with_standard_auxiliaries(
        format!("{}_halo_reference", topology.name()),
        LOOPS,
        source.coefficients().clone(),
        "d",
        propagators,
        Vec::new(),
    )?)
}

fn scalar_product_index(left: usize, right: usize) -> usize {
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    (0..left).map(|row| LOOPS - row).sum::<usize>() + right - left
}

fn scalar_product_pair(index: usize) -> (usize, usize) {
    (0..LOOPS)
        .flat_map(|left| (left..LOOPS).map(move |right| (left, right)))
        .nth(index)
        .expect("a four-loop scalar-product index is in range")
}

fn check_resource(
    resource: &'static str,
    requested: u128,
    limit: u128,
) -> Result<(), FourLoopHaloError> {
    if requested > limit {
        return Err(FourLoopHaloError::ResourceLimit {
            resource,
            requested,
            limit,
        });
    }
    Ok(())
}

#[derive(Debug)]
pub enum FourLoopHaloError {
    Family(FamilyError),
    Genuine(FourLoopGenuineError),
    MissingParameter {
        name: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    SourcePositionOutOfRange {
        position: usize,
        denominators: usize,
    },
    OutsideRawCornerHalo {
        integral: Integral,
    },
    AffineReplayMismatch {
        position: usize,
    },
    ActiveLineImageMismatch {
        source_position: usize,
        reference_position: usize,
    },
    WitnessMismatch,
}

impl From<FamilyError> for FourLoopHaloError {
    fn from(error: FamilyError) -> Self {
        Self::Family(error)
    }
}

impl From<FourLoopGenuineError> for FourLoopHaloError {
    fn from(error: FourLoopGenuineError) -> Self {
        Self::Genuine(error)
    }
}

impl fmt::Display for FourLoopHaloError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => write!(formatter, "cannot build four-loop halo family: {error}"),
            Self::Genuine(error) => write!(formatter, "genuine-corner witness failed: {error}"),
            Self::MissingParameter { name } => write!(formatter, "missing parameter {name}"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "four-loop halo {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "four-loop halo integral has {actual} powers, expected {expected}"
            ),
            Self::SourcePositionOutOfRange {
                position,
                denominators,
            } => write!(
                formatter,
                "source denominator position {position} is outside 0..{denominators}"
            ),
            Self::OutsideRawCornerHalo { integral } => write!(
                formatter,
                "{integral} is outside the raw scalar-corner (D,N) <= (1,1) halo"
            ),
            Self::AffineReplayMismatch { position } => write!(
                formatter,
                "affine image of source denominator {position} does not replay"
            ),
            Self::ActiveLineImageMismatch {
                source_position,
                reference_position,
            } => write!(
                formatter,
                "active source line {source_position} does not map exactly to reference line {reference_position}"
            ),
            Self::WitnessMismatch => {
                formatter.write_str("four-loop halo mapper and genuine witness do not match")
            }
        }
    }
}

impl std::error::Error for FourLoopHaloError {}
