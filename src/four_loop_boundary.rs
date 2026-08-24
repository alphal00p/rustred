//! Exact scalar-corner factorization for the equal-mass four-loop families.
//!
//! This module certifies the complete `(D,N)=(0,0)` physical-corner boundary
//! of Vakint's `H`, `X`, `BMW`, and `FG` parents.  A rank-deficient routing set
//! is scaleless.  A full-rank set is decomposed into vector-matroid components
//! using fundamental circuits in an explicit unimodular loop basis.  Every
//! proper component is then matched, by an exact canonical `GL(r,Z)` routing
//! signature, to a certified lower-loop scalar terminal.
//!
//! Connected rank-four corners are deliberately returned as genuine and
//! unresolved.  This boundary never promotes them to masters and does not
//! claim a complete four-loop IBP reduction.

use std::array;
use std::fmt;

use crate::exact::{invert_matrix, matrix_determinant, matrix_multiply, matrix_rank};
use crate::four_loop::{FourLoopTopology, equal_mass_four_loop_vacuum};
use crate::master_product::{MasterProduct, MasterProductError, ProductLinearCombination};
use crate::three_loop::THREE_LOOP_TETRAHEDRON_ROUTINGS;
use crate::{Coefficient, Denominator, ExactRational, FamilyError, Integral, VacuumFamily};

const LOOPS: usize = 4;
const DENOMINATORS: usize = 10;

/// Stable semantic identifiers for the lower-loop equal-mass scalar
/// terminals used by the four-loop boundary.
///
/// The three-loop entries are candidate terminals of RustRed's certified
/// finite three-loop box; this enum does not assert unrestricted minimality.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MassiveVacuumMaster {
    /// One-loop tadpole `T1`.
    T1,
    /// Equal-mass two-loop sunset corner `S2`.
    S2,
    /// Four-line, rank-three cycle (`B4`, tetrahedron mask 43).
    B4,
    /// Five-line, rank-three terminal (`F5`, tetrahedron mask 31).
    F5,
    /// Six-line tetrahedron corner (`M6`, tetrahedron mask 63).
    M6,
}

impl MassiveVacuumMaster {
    /// Versioned semantic namespace to use before persisting these identifiers.
    pub const SCHEMA: &'static str = "rustred-equal-mass-euclidean-master-v1";

    pub const fn loops(self) -> usize {
        match self {
            Self::T1 => 1,
            Self::S2 => 2,
            Self::B4 | Self::F5 | Self::M6 => 3,
        }
    }

    pub const fn physical_lines(self) -> usize {
        match self {
            Self::T1 => 1,
            Self::S2 => 3,
            Self::B4 => 4,
            Self::F5 => 5,
            Self::M6 => 6,
        }
    }

    /// Stable, versioned key independent of Rust enum discriminants.
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::T1 => "rustred-equal-mass-euclidean-master-v1:T1",
            Self::S2 => "rustred-equal-mass-euclidean-master-v1:S2",
            Self::B4 => "rustred-equal-mass-euclidean-master-v1:B4",
            Self::F5 => "rustred-equal-mass-euclidean-master-v1:F5",
            Self::M6 => "rustred-equal-mass-euclidean-master-v1:M6",
        }
    }
}

impl fmt::Display for MassiveVacuumMaster {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::T1 => "T1",
            Self::S2 => "S2",
            Self::B4 => "B4",
            Self::F5 => "F5",
            Self::M6 => "M6",
        })
    }
}

/// Resource limits for exact corner classification.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopBoundaryConfig {
    /// Maximum size of the four-row candidate universe admitted while finding
    /// a global unit basis.  This is a conservative preflight cap, not a count
    /// of subsets reached before the first successful basis.
    pub max_global_basis_candidates: usize,
    /// Maximum signed ordered-basis presentations admitted for one lower-loop
    /// component signature.  Axis signs are essential because each squared
    /// physical routing is independently equivalent under `q -> -q`.
    pub max_component_basis_candidates: usize,
}

impl Default for FourLoopBoundaryConfig {
    fn default() -> Self {
        Self {
            max_global_basis_candidates: 10_000,
            max_component_basis_candidates: 10_000,
        }
    }
}

/// Coordinates of one active physical line in the global loop map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopLineCoordinate {
    physical_position: usize,
    coordinates: [ExactRational; LOOPS],
}

/// One physical line matched to a frozen lower-loop reference routing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopSignedLineMatch {
    physical_position: usize,
    reference_position: usize,
    orientation_sign: i8,
}

impl FourLoopSignedLineMatch {
    pub const fn physical_position(&self) -> usize {
        self.physical_position
    }

    pub const fn reference_position(&self) -> usize {
        self.reference_position
    }

    /// `+1` or `-1` in `q_parent * U = sign * q_reference`.
    pub const fn orientation_sign(&self) -> i8 {
        self.orientation_sign
    }
}

impl FourLoopLineCoordinate {
    pub const fn physical_position(&self) -> usize {
        self.physical_position
    }

    pub const fn coordinates(&self) -> &[ExactRational; LOOPS] {
        &self.coordinates
    }
}

/// Exact recognition evidence for one proper routing component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopComponentWitness {
    master: MassiveVacuumMaster,
    global_basis_slots: Vec<usize>,
    physical_positions: Vec<usize>,
    component_basis_positions: Vec<usize>,
    canonical_signature: Vec<Vec<ExactRational>>,
    component_loop_map: Vec<Vec<ExactRational>>,
    determinant_sign: i8,
    signed_line_matches: Vec<FourLoopSignedLineMatch>,
}

impl FourLoopComponentWitness {
    pub const fn master(&self) -> MassiveVacuumMaster {
        self.master
    }

    pub fn global_basis_slots(&self) -> &[usize] {
        &self.global_basis_slots
    }

    pub fn physical_positions(&self) -> &[usize] {
        &self.physical_positions
    }

    pub fn component_basis_positions(&self) -> &[usize] {
        &self.component_basis_positions
    }

    pub fn canonical_signature(&self) -> &[Vec<ExactRational>] {
        &self.canonical_signature
    }

    /// Exact map from this component's global-basis coordinates to the frozen
    /// reference routing of [`Self::master`].
    pub fn component_loop_map(&self) -> &[Vec<ExactRational>] {
        &self.component_loop_map
    }

    pub const fn determinant_sign(&self) -> i8 {
        self.determinant_sign
    }

    pub fn signed_line_matches(&self) -> &[FourLoopSignedLineMatch] {
        &self.signed_line_matches
    }
}

/// Checkable unit-Jacobian block decomposition of one factorized corner.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopFactorizationWitness {
    topology: FourLoopTopology,
    sector_mask: u16,
    global_basis_positions: [usize; LOOPS],
    global_loop_map: [[ExactRational; LOOPS]; LOOPS],
    determinant_sign: i8,
    line_coordinates: Vec<FourLoopLineCoordinate>,
    components: Vec<FourLoopComponentWitness>,
}

impl FourLoopFactorizationWitness {
    pub const fn topology(&self) -> FourLoopTopology {
        self.topology
    }

    pub const fn sector_mask(&self) -> u16 {
        self.sector_mask
    }

    pub const fn global_basis_positions(&self) -> &[usize; LOOPS] {
        &self.global_basis_positions
    }

    /// Rows of `p = B k`; `det(B)=+1` or `-1`.
    pub const fn global_loop_map(&self) -> &[[ExactRational; LOOPS]; LOOPS] {
        &self.global_loop_map
    }

    pub const fn determinant_sign(&self) -> i8 {
        self.determinant_sign
    }

    pub fn line_coordinates(&self) -> &[FourLoopLineCoordinate] {
        &self.line_coordinates
    }

    pub fn components(&self) -> &[FourLoopComponentWitness] {
        &self.components
    }

    pub fn product(&self) -> Result<MasterProduct<MassiveVacuumMaster>, MasterProductError> {
        MasterProduct::try_from_factors(self.components.iter().map(|component| component.master))
    }
}

/// Exact scalar-corner classification.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FourLoopScalarClass {
    Scaleless {
        sector_mask: u16,
        active_lines: usize,
        routing_rank: usize,
    },
    Factorized {
        product: MasterProduct<MassiveVacuumMaster>,
        witness: FourLoopFactorizationWitness,
    },
    GenuineFourLoop {
        sector_mask: u16,
        active_lines: usize,
        global_basis_positions: [usize; LOOPS],
        determinant_sign: i8,
    },
}

/// Owned classifier/reducer for one validated four-loop parent.
#[derive(Clone, Debug)]
pub struct FourLoopBoundaryReducer {
    topology: FourLoopTopology,
    family: VacuumFamily,
    config: FourLoopBoundaryConfig,
}

impl FourLoopBoundaryReducer {
    /// Construct and validate the built-in family.
    pub fn build(
        topology: FourLoopTopology,
        config: FourLoopBoundaryConfig,
    ) -> Result<Self, FourLoopBoundaryError> {
        Self::new(topology, equal_mass_four_loop_vacuum(topology)?, config)
    }

    /// Validate and take ownership of an explicitly supplied built-in parent.
    pub fn new(
        topology: FourLoopTopology,
        family: VacuumFamily,
        config: FourLoopBoundaryConfig,
    ) -> Result<Self, FourLoopBoundaryError> {
        validate_family(topology, &family)?;
        Ok(Self {
            topology,
            family,
            config,
        })
    }

    pub const fn topology(&self) -> FourLoopTopology {
        self.topology
    }

    pub fn family(&self) -> &VacuumFamily {
        &self.family
    }

    pub const fn config(&self) -> FourLoopBoundaryConfig {
        self.config
    }

    pub fn classify_integral(
        &self,
        integral: &Integral,
    ) -> Result<FourLoopScalarClass, FourLoopBoundaryError> {
        let active = self.validate_corner(integral)?;
        let sector_mask = active
            .iter()
            .fold(0_u16, |mask, &position| mask | (1_u16 << position));
        let rows = active
            .iter()
            .map(|&position| (position, self.physical_routing(position)))
            .collect::<Vec<_>>();
        let rank = matrix_rank(rows.iter().map(|(_, row)| row.clone()).collect());
        if rank < LOOPS {
            return Ok(FourLoopScalarClass::Scaleless {
                sector_mask,
                active_lines: active.len(),
                routing_rank: rank,
            });
        }

        let (basis_positions, basis, basis_inverse, determinant_sign) =
            self.find_global_basis(&rows, sector_mask)?;
        let coordinates = rows
            .iter()
            .map(|(position, row)| FourLoopLineCoordinate {
                physical_position: *position,
                coordinates: row_times_matrix(row, &basis_inverse)
                    .try_into()
                    .expect("a four-loop inverse has four columns"),
            })
            .collect::<Vec<_>>();

        let blocks = coordinate_blocks(&coordinates);
        if blocks.len() == 1 {
            return Ok(FourLoopScalarClass::GenuineFourLoop {
                sector_mask,
                active_lines: active.len(),
                global_basis_positions: basis_positions,
                determinant_sign,
            });
        }

        let mut components = Vec::with_capacity(blocks.len());
        for slots in blocks {
            let block_lines = coordinates
                .iter()
                .filter(|line| {
                    line.coordinates
                        .iter()
                        .enumerate()
                        .any(|(slot, value)| slots.contains(&slot) && !value.is_zero())
                })
                .collect::<Vec<_>>();
            if block_lines.iter().any(|line| {
                line.coordinates
                    .iter()
                    .enumerate()
                    .any(|(slot, value)| !slots.contains(&slot) && !value.is_zero())
            }) {
                return Err(FourLoopBoundaryError::WitnessConstruction(
                    "a routing crosses two computed matroid blocks".to_owned(),
                ));
            }
            let reduced_rows = block_lines
                .iter()
                .map(|line| {
                    (
                        line.physical_position,
                        slots
                            .iter()
                            .map(|&slot| line.coordinates[slot])
                            .collect::<Vec<_>>(),
                    )
                })
                .collect::<Vec<_>>();
            let signature = canonical_signature(
                &reduced_rows,
                slots.len(),
                self.config.max_component_basis_candidates,
            )?;
            let recognition = recognize_signature(slots.len(), reduced_rows.len(), &signature)
                .ok_or_else(|| FourLoopBoundaryError::UnrecognizedComponent {
                    rank: slots.len(),
                    physical_positions: reduced_rows
                        .iter()
                        .map(|(position, _)| *position)
                        .collect(),
                    canonical_signature: signature.values.clone(),
                })?;
            components.push(FourLoopComponentWitness {
                master: recognition.master,
                global_basis_slots: slots,
                physical_positions: reduced_rows.iter().map(|(position, _)| *position).collect(),
                component_basis_positions: signature.basis_positions,
                canonical_signature: signature.values,
                component_loop_map: recognition.component_loop_map,
                determinant_sign: recognition.determinant_sign,
                signed_line_matches: recognition.signed_line_matches,
            });
        }
        components.sort_by_key(|component| component.global_basis_slots[0]);
        let witness = FourLoopFactorizationWitness {
            topology: self.topology,
            sector_mask,
            global_basis_positions: basis_positions,
            global_loop_map: basis
                .into_iter()
                .map(|row| {
                    row.try_into()
                        .expect("a four-loop routing has four entries")
                })
                .collect::<Vec<_>>()
                .try_into()
                .expect("a four-loop basis has four rows"),
            determinant_sign,
            line_coordinates: coordinates,
            components,
        };
        let product = witness.product()?;
        Ok(FourLoopScalarClass::Factorized { product, witness })
    }

    /// Return zero or a one-term lower-loop product for the proved boundary;
    /// return `None` for a genuine connected rank-four corner.
    pub fn try_reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<Option<ProductLinearCombination<MassiveVacuumMaster>>, FourLoopBoundaryError> {
        Ok(match self.classify_integral(integral)? {
            FourLoopScalarClass::Scaleless { .. } => Some(ProductLinearCombination::new()),
            FourLoopScalarClass::Factorized { product, .. } => Some(
                ProductLinearCombination::from_term(product, self.family.coefficients().one()),
            ),
            FourLoopScalarClass::GenuineFourLoop { .. } => None,
        })
    }

    pub fn reduce_integral(
        &self,
        integral: &Integral,
    ) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopBoundaryError> {
        self.try_reduce_integral(integral)?.ok_or_else(|| {
            FourLoopBoundaryError::GenuineFourLoopCorner {
                integral: integral.clone(),
            }
        })
    }

    /// Independently replay the stored exact maps and signed line bijections.
    ///
    /// This does not rerun canonical-signature search and is independent of
    /// the reducer's classification resource limits.
    pub fn replay_witness(
        &self,
        witness: &FourLoopFactorizationWitness,
    ) -> Result<MasterProduct<MassiveVacuumMaster>, FourLoopBoundaryError> {
        if witness.topology != self.topology {
            return Err(FourLoopBoundaryError::WitnessMismatch);
        }
        let physical_count = self.family.propagator_count();
        if witness.sector_mask >> physical_count != 0
            || witness.determinant_sign != determinant_sign_4(&witness.global_loop_map)?
        {
            return Err(FourLoopBoundaryError::WitnessMismatch);
        }
        for (slot, &position) in witness.global_basis_positions.iter().enumerate() {
            if position >= physical_count
                || witness.sector_mask & (1_u16 << position) == 0
                || self.physical_routing(position).as_slice() != witness.global_loop_map[slot]
            {
                return Err(FourLoopBoundaryError::WitnessMismatch);
            }
            if witness.global_basis_positions[..slot].contains(&position) {
                return Err(FourLoopBoundaryError::WitnessMismatch);
            }
        }

        let expected_active = (0..physical_count)
            .filter(|&position| witness.sector_mask & (1_u16 << position) != 0)
            .collect::<Vec<_>>();
        let mut actual_active = witness
            .line_coordinates
            .iter()
            .map(|line| line.physical_position)
            .collect::<Vec<_>>();
        actual_active.sort_unstable();
        actual_active.dedup();
        if actual_active != expected_active || actual_active.len() != witness.line_coordinates.len()
        {
            return Err(FourLoopBoundaryError::WitnessMismatch);
        }
        for line in &witness.line_coordinates {
            let reconstructed = row_times_matrix(
                &line.coordinates,
                &witness
                    .global_loop_map
                    .iter()
                    .map(|row| row.to_vec())
                    .collect::<Vec<_>>(),
            );
            if reconstructed != self.physical_routing(line.physical_position) {
                return Err(FourLoopBoundaryError::WitnessMismatch);
            }
        }

        let mut seen_slots = Vec::new();
        let mut seen_lines = Vec::new();
        for component in &witness.components {
            let rank = component.global_basis_slots.len();
            if rank != component.master.loops()
                || component.physical_positions.len() != component.master.physical_lines()
                || component.component_basis_positions.len() != rank
                || component.component_basis_positions.iter().enumerate().any(
                    |(index, position)| {
                        !component.physical_positions.contains(position)
                            || component.component_basis_positions[..index].contains(position)
                    },
                )
                || component.component_loop_map.len() != rank
                || component
                    .component_loop_map
                    .iter()
                    .any(|row| row.len() != rank)
                || determinant_sign(&component.component_loop_map)? != component.determinant_sign
            {
                return Err(FourLoopBoundaryError::WitnessMismatch);
            }
            for &slot in &component.global_basis_slots {
                if slot >= LOOPS || seen_slots.contains(&slot) {
                    return Err(FourLoopBoundaryError::WitnessMismatch);
                }
                seen_slots.push(slot);
            }
            let mut reference_seen = Vec::new();
            for line_match in &component.signed_line_matches {
                if !component
                    .physical_positions
                    .contains(&line_match.physical_position)
                    || seen_lines.contains(&line_match.physical_position)
                    || reference_seen.contains(&line_match.reference_position)
                    || !matches!(line_match.orientation_sign, -1 | 1)
                {
                    return Err(FourLoopBoundaryError::WitnessMismatch);
                }
                let Some(line) = witness
                    .line_coordinates
                    .iter()
                    .find(|line| line.physical_position == line_match.physical_position)
                else {
                    return Err(FourLoopBoundaryError::WitnessMismatch);
                };
                if line.coordinates.iter().enumerate().any(|(slot, value)| {
                    !component.global_basis_slots.contains(&slot) && !value.is_zero()
                }) {
                    return Err(FourLoopBoundaryError::WitnessMismatch);
                }
                let local = component
                    .global_basis_slots
                    .iter()
                    .map(|&slot| line.coordinates[slot])
                    .collect::<Vec<_>>();
                let mapped = row_times_matrix(&local, &component.component_loop_map);
                let reference = reference_rows(component.master);
                let Some((_, reference_row)) = reference
                    .iter()
                    .find(|(position, _)| *position == line_match.reference_position)
                else {
                    return Err(FourLoopBoundaryError::WitnessMismatch);
                };
                let expected = reference_row
                    .iter()
                    .map(|&value| {
                        value * ExactRational::from(i64::from(line_match.orientation_sign))
                    })
                    .collect::<Vec<_>>();
                if mapped != expected {
                    return Err(FourLoopBoundaryError::WitnessMismatch);
                }
                seen_lines.push(line_match.physical_position);
                reference_seen.push(line_match.reference_position);
            }
            if reference_seen.len() != component.master.physical_lines()
                || component.signed_line_matches.len() != component.physical_positions.len()
            {
                return Err(FourLoopBoundaryError::WitnessMismatch);
            }
        }
        seen_slots.sort_unstable();
        seen_lines.sort_unstable();
        if seen_slots != [0, 1, 2, 3] || seen_lines != expected_active {
            return Err(FourLoopBoundaryError::WitnessMismatch);
        }
        witness.product().map_err(Into::into)
    }

    fn validate_corner(&self, integral: &Integral) -> Result<Vec<usize>, FourLoopBoundaryError> {
        if integral.powers().len() != DENOMINATORS {
            return Err(FourLoopBoundaryError::WrongIntegralArity {
                expected: DENOMINATORS,
                actual: integral.powers().len(),
            });
        }
        let mut active = Vec::new();
        for (position, (&power, denominator)) in integral
            .powers()
            .iter()
            .zip(self.family.denominators())
            .enumerate()
        {
            if denominator.is_propagator() {
                match power {
                    0 => {}
                    1 => active.push(position),
                    power if power < 0 => {
                        return Err(FourLoopBoundaryError::PhysicalNumerator { position, power });
                    }
                    power => return Err(FourLoopBoundaryError::PhysicalDot { position, power }),
                }
            } else if power != 0 {
                return Err(FourLoopBoundaryError::NonzeroAuxiliary { position, power });
            }
        }
        Ok(active)
    }

    fn physical_routing(&self, position: usize) -> Vec<ExactRational> {
        self.family.denominators()[position]
            .momentum()
            .expect("validated physical position")
            .to_vec()
    }

    fn find_global_basis(
        &self,
        rows: &[(usize, Vec<ExactRational>)],
        sector_mask: u16,
    ) -> Result<
        (
            [usize; LOOPS],
            Vec<Vec<ExactRational>>,
            Vec<Vec<ExactRational>>,
            i8,
        ),
        FourLoopBoundaryError,
    > {
        let requested = binomial_saturating(rows.len(), LOOPS);
        if requested > self.config.max_global_basis_candidates as u128 {
            return Err(FourLoopBoundaryError::ResourceLimit {
                resource: "global unimodular basis candidates",
                requested,
                limit: self.config.max_global_basis_candidates as u128,
            });
        }
        for indices in combinations(rows.len(), LOOPS) {
            let basis = indices
                .iter()
                .map(|&index| rows[index].1.clone())
                .collect::<Vec<_>>();
            let determinant =
                matrix_determinant(&basis).map_err(FourLoopBoundaryError::LinearAlgebra)?;
            if determinant != ExactRational::ONE && determinant != -ExactRational::ONE {
                continue;
            }
            let positions = indices
                .iter()
                .map(|&index| rows[index].0)
                .collect::<Vec<_>>()
                .try_into()
                .expect("selected four basis positions");
            let inverse = invert_matrix(&basis).map_err(FourLoopBoundaryError::LinearAlgebra)?;
            return Ok((
                positions,
                basis,
                inverse,
                if determinant == ExactRational::ONE {
                    1
                } else {
                    -1
                },
            ));
        }
        Err(FourLoopBoundaryError::NoUnimodularGlobalBasis { sector_mask })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalSignature {
    basis_positions: Vec<usize>,
    basis_matrix: Vec<Vec<ExactRational>>,
    axis_signs: Vec<i8>,
    lines: Vec<CanonicalLine>,
    values: Vec<Vec<ExactRational>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CanonicalLine {
    element_position: usize,
    normalized: Vec<ExactRational>,
    orientation_sign: i8,
}

fn canonical_signature(
    rows: &[(usize, Vec<ExactRational>)],
    rank: usize,
    max_candidates: usize,
) -> Result<CanonicalSignature, FourLoopBoundaryError> {
    let axis_signs = 1_u128.checked_shl(rank as u32).unwrap_or(u128::MAX);
    let requested = falling_factorial_saturating(rows.len(), rank).saturating_mul(axis_signs);
    if requested > max_candidates as u128 {
        return Err(FourLoopBoundaryError::ResourceLimit {
            resource: "component ordered-basis candidates",
            requested,
            limit: max_candidates as u128,
        });
    }
    let mut best: Option<CanonicalSignature> = None;
    for indices in ordered_selections(rows.len(), rank) {
        let basis = indices
            .iter()
            .map(|&index| rows[index].1.clone())
            .collect::<Vec<_>>();
        let determinant =
            matrix_determinant(&basis).map_err(FourLoopBoundaryError::LinearAlgebra)?;
        if determinant != ExactRational::ONE && determinant != -ExactRational::ONE {
            continue;
        }
        let inverse = invert_matrix(&basis).map_err(FourLoopBoundaryError::LinearAlgebra)?;
        let coordinates = rows
            .iter()
            .map(|(_, row)| row_times_matrix(row, &inverse))
            .collect::<Vec<_>>();
        for signs in 0..(1_usize << rank) {
            let mut lines = coordinates
                .iter()
                .enumerate()
                .map(|(line, row)| {
                    let signed = row
                        .iter()
                        .enumerate()
                        .map(|(axis, &value)| {
                            if signs & (1 << axis) == 0 {
                                value
                            } else {
                                -value
                            }
                        })
                        .collect::<Vec<_>>();
                    let (normalized, orientation_sign) = normalize_squared_routing(signed);
                    CanonicalLine {
                        element_position: rows[line].0,
                        normalized,
                        orientation_sign,
                    }
                })
                .collect::<Vec<_>>();
            lines.sort_by(|left, right| {
                left.normalized
                    .cmp(&right.normalized)
                    .then_with(|| left.element_position.cmp(&right.element_position))
            });
            let values = lines.iter().map(|line| line.normalized.clone()).collect();
            let candidate = CanonicalSignature {
                basis_positions: indices.iter().map(|&index| rows[index].0).collect(),
                basis_matrix: basis.clone(),
                axis_signs: (0..rank)
                    .map(|axis| if signs & (1 << axis) == 0 { 1 } else { -1 })
                    .collect(),
                lines,
                values,
            };
            if best
                .as_ref()
                .is_none_or(|current| candidate.values < current.values)
            {
                best = Some(candidate);
            }
        }
    }
    best.ok_or_else(|| FourLoopBoundaryError::NoUnimodularComponentBasis {
        physical_positions: rows.iter().map(|(position, _)| *position).collect(),
    })
}

fn recognize_signature(
    rank: usize,
    lines: usize,
    signature: &CanonicalSignature,
) -> Option<ComponentRecognition> {
    let candidate = match (rank, lines) {
        (1, 1) => MassiveVacuumMaster::T1,
        (2, 3) => MassiveVacuumMaster::S2,
        (3, 4) => MassiveVacuumMaster::B4,
        (3, 5) => MassiveVacuumMaster::F5,
        (3, 6) => MassiveVacuumMaster::M6,
        _ => return None,
    };
    let reference = reference_rows(candidate);
    let canonical = canonical_signature(&reference, rank, usize::MAX).ok()?;
    if canonical.values != signature.values {
        return None;
    }
    let parent_inverse = invert_matrix(&signature.basis_matrix).ok()?;
    let signed_axes = diagonal_sign_product(&signature.axis_signs, &canonical.axis_signs);
    let map = matrix_multiply(
        &matrix_multiply(&parent_inverse, &signed_axes).ok()?,
        &canonical.basis_matrix,
    )
    .ok()?;
    let determinant_sign = determinant_sign(&map).ok()?;
    let signed_line_matches = signature
        .lines
        .iter()
        .zip(&canonical.lines)
        .map(|(parent, reference)| FourLoopSignedLineMatch {
            physical_position: parent.element_position,
            reference_position: reference.element_position,
            orientation_sign: parent.orientation_sign * reference.orientation_sign,
        })
        .collect();
    Some(ComponentRecognition {
        master: candidate,
        component_loop_map: map,
        determinant_sign,
        signed_line_matches,
    })
}

struct ComponentRecognition {
    master: MassiveVacuumMaster,
    component_loop_map: Vec<Vec<ExactRational>>,
    determinant_sign: i8,
    signed_line_matches: Vec<FourLoopSignedLineMatch>,
}

fn reference_rows(master: MassiveVacuumMaster) -> Vec<(usize, Vec<ExactRational>)> {
    let rows: Vec<Vec<i8>> = match master {
        MassiveVacuumMaster::T1 => vec![vec![1]],
        MassiveVacuumMaster::S2 => vec![vec![1, 0], vec![0, 1], vec![1, 1]],
        MassiveVacuumMaster::B4 => [0, 1, 3, 5]
            .into_iter()
            .map(|position| THREE_LOOP_TETRAHEDRON_ROUTINGS[position].to_vec())
            .collect(),
        MassiveVacuumMaster::F5 => THREE_LOOP_TETRAHEDRON_ROUTINGS[..5]
            .iter()
            .map(|row| row.to_vec())
            .collect(),
        MassiveVacuumMaster::M6 => THREE_LOOP_TETRAHEDRON_ROUTINGS
            .iter()
            .map(|row| row.to_vec())
            .collect(),
    };
    rows.into_iter()
        .enumerate()
        .map(|(position, row)| {
            (
                position,
                row.into_iter()
                    .map(|value| ExactRational::from(i64::from(value)))
                    .collect(),
            )
        })
        .collect()
}

fn coordinate_blocks(lines: &[FourLoopLineCoordinate]) -> Vec<Vec<usize>> {
    let mut parent: [usize; LOOPS] = array::from_fn(|index| index);
    for line in lines {
        let support = line
            .coordinates
            .iter()
            .enumerate()
            .filter_map(|(slot, value)| (!value.is_zero()).then_some(slot))
            .collect::<Vec<_>>();
        if let Some((&first, rest)) = support.split_first() {
            for &other in rest {
                union(&mut parent, first, other);
            }
        }
    }
    let mut blocks = Vec::<Vec<usize>>::new();
    for slot in 0..LOOPS {
        let root = find(&mut parent, slot);
        if let Some(block) = blocks
            .iter_mut()
            .find(|block| find(&mut parent, block[0]) == root)
        {
            block.push(slot);
        } else {
            blocks.push(vec![slot]);
        }
    }
    blocks.sort_by_key(|block| block[0]);
    blocks
}

fn find(parent: &mut [usize; LOOPS], slot: usize) -> usize {
    if parent[slot] != slot {
        parent[slot] = find(parent, parent[slot]);
    }
    parent[slot]
}

fn union(parent: &mut [usize; LOOPS], left: usize, right: usize) {
    let left = find(parent, left);
    let right = find(parent, right);
    if left != right {
        parent[right] = left;
    }
}

fn row_times_matrix(row: &[ExactRational], matrix: &[Vec<ExactRational>]) -> Vec<ExactRational> {
    (0..matrix[0].len())
        .map(|column| {
            row.iter()
                .zip(matrix)
                .map(|(&left, right)| left * right[column])
                .fold(ExactRational::ZERO, |sum, value| sum + value)
        })
        .collect()
}

fn normalize_squared_routing(mut row: Vec<ExactRational>) -> (Vec<ExactRational>, i8) {
    let mut orientation_sign = 1;
    if row
        .iter()
        .find(|value| !value.is_zero())
        .is_some_and(|value| value.numerator() < 0)
    {
        orientation_sign = -1;
        for value in &mut row {
            *value = -*value;
        }
    }
    (row, orientation_sign)
}

fn diagonal_sign_product(left: &[i8], right: &[i8]) -> Vec<Vec<ExactRational>> {
    (0..left.len())
        .map(|row| {
            (0..left.len())
                .map(|column| {
                    if row == column {
                        ExactRational::from(i64::from(left[row] * right[row]))
                    } else {
                        ExactRational::ZERO
                    }
                })
                .collect()
        })
        .collect()
}

fn determinant_sign(matrix: &[Vec<ExactRational>]) -> Result<i8, FourLoopBoundaryError> {
    let determinant = matrix_determinant(matrix).map_err(FourLoopBoundaryError::LinearAlgebra)?;
    match determinant {
        ExactRational::ONE => Ok(1),
        value if value == -ExactRational::ONE => Ok(-1),
        _ => Err(FourLoopBoundaryError::WitnessConstruction(
            "component loop map is not unimodular".to_owned(),
        )),
    }
}

fn determinant_sign_4(
    matrix: &[[ExactRational; LOOPS]; LOOPS],
) -> Result<i8, FourLoopBoundaryError> {
    determinant_sign(&matrix.iter().map(|row| row.to_vec()).collect::<Vec<_>>())
}

fn combinations(length: usize, choose: usize) -> Vec<Vec<usize>> {
    fn visit(
        length: usize,
        choose: usize,
        start: usize,
        current: &mut Vec<usize>,
        output: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == choose {
            output.push(current.clone());
            return;
        }
        let needed = choose - current.len();
        for value in start..=length.saturating_sub(needed) {
            current.push(value);
            visit(length, choose, value + 1, current, output);
            current.pop();
        }
    }
    if choose > length {
        return Vec::new();
    }
    let mut output = Vec::new();
    visit(length, choose, 0, &mut Vec::new(), &mut output);
    output
}

fn ordered_selections(length: usize, choose: usize) -> Vec<Vec<usize>> {
    fn visit(
        length: usize,
        choose: usize,
        used: &mut [bool],
        current: &mut Vec<usize>,
        output: &mut Vec<Vec<usize>>,
    ) {
        if current.len() == choose {
            output.push(current.clone());
            return;
        }
        for value in 0..length {
            if !used[value] {
                used[value] = true;
                current.push(value);
                visit(length, choose, used, current, output);
                current.pop();
                used[value] = false;
            }
        }
    }
    if choose > length {
        return Vec::new();
    }
    let mut output = Vec::new();
    visit(
        length,
        choose,
        &mut vec![false; length],
        &mut Vec::new(),
        &mut output,
    );
    output
}

fn binomial_saturating(n: usize, k: usize) -> u128 {
    if k > n {
        return 0;
    }
    let k = k.min(n - k);
    let mut value = 1_u128;
    for index in 0..k {
        value = value
            .saturating_mul((n - index) as u128)
            .checked_div((index + 1) as u128)
            .unwrap_or(u128::MAX);
    }
    value
}

fn falling_factorial_saturating(n: usize, k: usize) -> u128 {
    if k > n {
        return 0;
    }
    (0..k).fold(1_u128, |value, index| {
        value.saturating_mul((n - index) as u128)
    })
}

fn validate_family(
    topology: FourLoopTopology,
    family: &VacuumFamily,
) -> Result<Coefficient, FourLoopBoundaryError> {
    if family.loops() != LOOPS {
        return Err(FourLoopBoundaryError::WrongLoopCount {
            actual: family.loops(),
        });
    }
    if family.denominator_count() != DENOMINATORS {
        return Err(FourLoopBoundaryError::WrongDenominatorCount {
            actual: family.denominator_count(),
        });
    }
    if family.propagator_count() != topology.routings().len() {
        return Err(FourLoopBoundaryError::WrongPhysicalCount {
            expected: topology.routings().len(),
            actual: family.propagator_count(),
        });
    }
    let mass = family
        .coefficients()
        .parameter("m2")
        .ok_or(FourLoopBoundaryError::MissingParameter { name: "m2" })?;
    let dimension = family
        .coefficients()
        .parameter("d")
        .ok_or(FourLoopBoundaryError::MissingParameter { name: "d" })?;
    if family.dimension() != &dimension {
        return Err(FourLoopBoundaryError::WrongDimensionParameter);
    }
    for (position, expected_routing) in topology.routings().iter().enumerate() {
        let denominator = &family.denominators()[position];
        if denominator.normalization() != Some(1) {
            return Err(FourLoopBoundaryError::WrongPropagatorSign { position });
        }
        if denominator.shift() != &mass || denominator.shift().is_zero() {
            return Err(FourLoopBoundaryError::UnequalOrMasslessPropagators { position });
        }
        let expected = Denominator::propagator(
            expected_routing
                .iter()
                .map(|&value| ExactRational::from(i64::from(value)))
                .collect(),
            mass.clone(),
        );
        if denominator.quadratic_form() != expected.quadratic_form() {
            return Err(FourLoopBoundaryError::WrongMomentumRouting { position });
        }
    }
    for position in topology.routings().len()..DENOMINATORS {
        if family.denominators()[position].is_propagator() {
            return Err(FourLoopBoundaryError::ExpectedAuxiliary { position });
        }
    }
    Ok(mass)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FourLoopBoundaryError {
    Family(FamilyError),
    WrongLoopCount {
        actual: usize,
    },
    WrongDenominatorCount {
        actual: usize,
    },
    WrongPhysicalCount {
        expected: usize,
        actual: usize,
    },
    MissingParameter {
        name: &'static str,
    },
    WrongDimensionParameter,
    WrongPropagatorSign {
        position: usize,
    },
    UnequalOrMasslessPropagators {
        position: usize,
    },
    WrongMomentumRouting {
        position: usize,
    },
    ExpectedAuxiliary {
        position: usize,
    },
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    PhysicalNumerator {
        position: usize,
        power: i32,
    },
    PhysicalDot {
        position: usize,
        power: i32,
    },
    NonzeroAuxiliary {
        position: usize,
        power: i32,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    NoUnimodularGlobalBasis {
        sector_mask: u16,
    },
    NoUnimodularComponentBasis {
        physical_positions: Vec<usize>,
    },
    UnrecognizedComponent {
        rank: usize,
        physical_positions: Vec<usize>,
        canonical_signature: Vec<Vec<ExactRational>>,
    },
    LinearAlgebra(String),
    WitnessConstruction(String),
    WitnessMismatch,
    MasterProduct(MasterProductError),
    GenuineFourLoopCorner {
        integral: Integral,
    },
}

impl From<FamilyError> for FourLoopBoundaryError {
    fn from(error: FamilyError) -> Self {
        Self::Family(error)
    }
}

impl From<MasterProductError> for FourLoopBoundaryError {
    fn from(error: MasterProductError) -> Self {
        Self::MasterProduct(error)
    }
}

impl fmt::Display for FourLoopBoundaryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => error.fmt(formatter),
            Self::WrongLoopCount { actual } => {
                write!(
                    formatter,
                    "four-loop boundary received a {actual}-loop family"
                )
            }
            Self::WrongDenominatorCount { actual } => write!(
                formatter,
                "four-loop boundary needs ten denominator-basis entries, received {actual}"
            ),
            Self::WrongPhysicalCount { expected, actual } => write!(
                formatter,
                "four-loop topology needs {expected} physical lines, received {actual}"
            ),
            Self::MissingParameter { name } => write!(formatter, "missing parameter {name}"),
            Self::WrongDimensionParameter => {
                formatter.write_str("family dimension parameter is not d")
            }
            Self::WrongPropagatorSign { position } => {
                write!(
                    formatter,
                    "physical line {position} does not have positive normalization"
                )
            }
            Self::UnequalOrMasslessPropagators { position } => write!(
                formatter,
                "physical line {position} does not carry the common nonzero mass m2"
            ),
            Self::WrongMomentumRouting { position } => {
                write!(formatter, "physical line {position} has the wrong routing")
            }
            Self::ExpectedAuxiliary { position } => {
                write!(formatter, "basis entry {position} must be auxiliary")
            }
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "integral has {actual} powers, expected {expected}"
            ),
            Self::PhysicalNumerator { position, power } => write!(
                formatter,
                "physical line {position} has numerator power {power}; scalar corners require 0 or 1"
            ),
            Self::PhysicalDot { position, power } => write!(
                formatter,
                "physical line {position} has dotted power {power}; scalar corners require 0 or 1"
            ),
            Self::NonzeroAuxiliary { position, power } => write!(
                formatter,
                "auxiliary basis entry {position} has nonzero power {power}"
            ),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "four-loop {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::NoUnimodularGlobalBasis { sector_mask } => write!(
                formatter,
                "full-rank corner mask {sector_mask:#x} has no unit-Jacobian routing basis"
            ),
            Self::NoUnimodularComponentBasis { physical_positions } => write!(
                formatter,
                "routing component {physical_positions:?} has no unit-Jacobian basis"
            ),
            Self::UnrecognizedComponent {
                rank,
                physical_positions,
                ..
            } => write!(
                formatter,
                "rank-{rank} routing component {physical_positions:?} is not a certified lower-loop terminal"
            ),
            Self::LinearAlgebra(message) | Self::WitnessConstruction(message) => {
                formatter.write_str(message)
            }
            Self::WitnessMismatch => formatter.write_str("factorization witness replay mismatch"),
            Self::MasterProduct(error) => error.fmt(formatter),
            Self::GenuineFourLoopCorner { integral } => write!(
                formatter,
                "{integral} is a genuine connected four-loop corner outside the factorized boundary"
            ),
        }
    }
}

impl std::error::Error for FourLoopBoundaryError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Family(error) => Some(error),
            Self::MasterProduct(error) => Some(error),
            _ => None,
        }
    }
}
