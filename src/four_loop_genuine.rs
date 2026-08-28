//! Exact inter-family classification of genuine scalar four-loop corners.
//!
//! The scalar-corner boundary leaves 214 labelled, connected rank-four
//! presentations unresolved.  Quotienting squared routings by signed
//! unimodular loop-basis changes collapses them to exactly ten types.  This
//! module constructs that quotient and emits a replayable witness for every
//! presentation.  It does not yet assert an IBP reduction of those ten types:
//! they are bounded-certificate candidates, not unrestricted masters.

use std::array;
use std::fmt;

use crate::four_loop::{FourLoopTopology, equal_mass_four_loop_vacuum};
use crate::four_loop_boundary::{
    FourLoopBoundaryConfig, FourLoopBoundaryError, FourLoopBoundaryReducer, FourLoopScalarClass,
};
use crate::legacy_oracle_support::exact_matrix::{
    invert_matrix, matrix_determinant, matrix_multiply,
};
use crate::{ExactRational, FamilyError, Integral, VacuumFamily};

const LOOPS: usize = 4;
const SIGN_CHOICES: u128 = 1 << LOOPS;

/// The ten signed-`GL(4,Z)` routing types occurring among all genuine scalar
/// corners of the built-in H, X, BMW, and FG parents.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FourLoopGenuineCornerType {
    FiveLine,
    SixLineA,
    SixLineB,
    SevenLineA,
    SevenLineB,
    SevenLineC,
    EightLineA,
    EightLineB,
    HNineLine,
    XNineLine,
}

impl FourLoopGenuineCornerType {
    pub const ALL: [Self; 10] = [
        Self::FiveLine,
        Self::SixLineA,
        Self::SixLineB,
        Self::SevenLineA,
        Self::SevenLineB,
        Self::SevenLineC,
        Self::EightLineA,
        Self::EightLineB,
        Self::HNineLine,
        Self::XNineLine,
    ];

    pub const SCHEMA: &'static str = "rustred-equal-mass-euclidean-four-loop-corner-v1";

    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::FiveLine => "rustred-equal-mass-euclidean-four-loop-corner-v1:V5",
            Self::SixLineA => "rustred-equal-mass-euclidean-four-loop-corner-v1:V6a",
            Self::SixLineB => "rustred-equal-mass-euclidean-four-loop-corner-v1:V6b",
            Self::SevenLineA => "rustred-equal-mass-euclidean-four-loop-corner-v1:V7a",
            Self::SevenLineB => "rustred-equal-mass-euclidean-four-loop-corner-v1:V7b",
            Self::SevenLineC => "rustred-equal-mass-euclidean-four-loop-corner-v1:V7c",
            Self::EightLineA => "rustred-equal-mass-euclidean-four-loop-corner-v1:V8a",
            Self::EightLineB => "rustred-equal-mass-euclidean-four-loop-corner-v1:V8b",
            Self::HNineLine => "rustred-equal-mass-euclidean-four-loop-corner-v1:H9",
            Self::XNineLine => "rustred-equal-mass-euclidean-four-loop-corner-v1:X9",
        }
    }

    /// A frozen built-in presentation used only as the target of witnesses.
    pub const fn reference_topology(self) -> FourLoopTopology {
        match self {
            Self::XNineLine => FourLoopTopology::X,
            _ => FourLoopTopology::H,
        }
    }

    /// Physical-line mask in [`Self::reference_topology`].
    pub const fn reference_mask(self) -> u16 {
        match self {
            Self::FiveLine => 0x06b,
            Self::SixLineA => 0x06f,
            Self::SixLineB => 0x0cf,
            Self::SevenLineA => 0x13f,
            Self::SevenLineB => 0x07f,
            Self::SevenLineC => 0x0df,
            Self::EightLineA => 0x17f,
            Self::EightLineB => 0x0ff,
            Self::HNineLine | Self::XNineLine => 0x1ff,
        }
    }

    pub const fn physical_lines(self) -> usize {
        self.reference_mask().count_ones() as usize
    }

    /// Number of labelled genuine corners of all four parents belonging to
    /// this type.  The exhaustive test derives these counts independently.
    pub const fn labelled_multiplicity(self) -> usize {
        match self {
            Self::FiveLine => 21,
            Self::SixLineA => 78,
            Self::SixLineB => 13,
            Self::SevenLineA => 4,
            Self::SevenLineB => 44,
            Self::SevenLineC => 32,
            Self::EightLineA => 7,
            Self::EightLineB => 13,
            Self::HNineLine | Self::XNineLine => 1,
        }
    }
}

impl fmt::Display for FourLoopGenuineCornerType {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::FiveLine => "V5",
            Self::SixLineA => "V6a",
            Self::SixLineB => "V6b",
            Self::SevenLineA => "V7a",
            Self::SevenLineB => "V7b",
            Self::SevenLineC => "V7c",
            Self::EightLineA => "V8a",
            Self::EightLineB => "V8b",
            Self::HNineLine => "H9",
            Self::XNineLine => "X9",
        })
    }
}

/// Limits for signed-routing canonicalization.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopGenuineConfig {
    pub boundary: FourLoopBoundaryConfig,
    /// Conservative aggregate signed ordered-basis candidate universe for the
    /// frozen ten-entry catalog.  Its size is 204,288.  Sign choices are only
    /// visited for candidates subsequently proved unimodular, so this charge
    /// deliberately bounds rather than predicts the exact work performed.
    pub max_catalog_signature_candidates: usize,
    /// Conservative signed ordered-basis candidate universe admitted for one
    /// input graph.  A nine-line input is charged 48,384; sign choices are
    /// visited only for the subset of unimodular ordered bases.
    pub max_input_signature_candidates: usize,
    /// Maximum ordered basis index vectors retained in memory for one
    /// signature search.  The largest nine-line input requires 3,024; its 16
    /// axis signs are streamed and do not multiply retained storage.
    pub max_ordered_basis_storage: usize,
}

impl Default for FourLoopGenuineConfig {
    fn default() -> Self {
        Self {
            boundary: FourLoopBoundaryConfig::default(),
            max_catalog_signature_candidates: 250_000,
            max_input_signature_candidates: 50_000,
            max_ordered_basis_storage: 4_000,
        }
    }
}

/// One source line mapped to a frozen reference line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopGenuineLineMatch {
    source_physical_position: usize,
    reference_physical_position: usize,
    orientation_sign: i8,
}

impl FourLoopGenuineLineMatch {
    pub const fn source_physical_position(&self) -> usize {
        self.source_physical_position
    }

    pub const fn reference_physical_position(&self) -> usize {
        self.reference_physical_position
    }

    /// `q_source * U = orientation_sign * q_reference`.
    pub const fn orientation_sign(&self) -> i8 {
        self.orientation_sign
    }
}

/// Determinant-`+/-1` proof that one labelled genuine corner is a frozen type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopGenuineWitness {
    source_topology: FourLoopTopology,
    source_sector_mask: u16,
    corner_type: FourLoopGenuineCornerType,
    source_basis_positions: [usize; LOOPS],
    reference_basis_positions: [usize; LOOPS],
    loop_map: [[ExactRational; LOOPS]; LOOPS],
    determinant_sign: i8,
    signed_line_matches: Vec<FourLoopGenuineLineMatch>,
}

impl FourLoopGenuineWitness {
    pub const fn source_topology(&self) -> FourLoopTopology {
        self.source_topology
    }

    pub const fn source_sector_mask(&self) -> u16 {
        self.source_sector_mask
    }

    pub const fn corner_type(&self) -> FourLoopGenuineCornerType {
        self.corner_type
    }

    pub const fn reference_topology(&self) -> FourLoopTopology {
        self.corner_type.reference_topology()
    }

    pub const fn reference_sector_mask(&self) -> u16 {
        self.corner_type.reference_mask()
    }

    pub const fn source_basis_positions(&self) -> &[usize; LOOPS] {
        &self.source_basis_positions
    }

    pub const fn reference_basis_positions(&self) -> &[usize; LOOPS] {
        &self.reference_basis_positions
    }

    /// Rows follow `q_source * U = sign * q_reference`.
    pub const fn loop_map(&self) -> &[[ExactRational; LOOPS]; LOOPS] {
        &self.loop_map
    }

    pub const fn determinant_sign(&self) -> i8 {
        self.determinant_sign
    }

    pub fn signed_line_matches(&self) -> &[FourLoopGenuineLineMatch] {
        &self.signed_line_matches
    }

    /// Test/certificate tooling may replay a deliberately altered witness.
    /// The classifier must reject any source basis which is merely an active
    /// subset but is not the ordered basis used by the stored map.
    #[doc(hidden)]
    pub fn with_source_basis_positions_for_replay(
        &self,
        source_basis_positions: [usize; LOOPS],
    ) -> Self {
        let mut witness = self.clone();
        witness.source_basis_positions = source_basis_positions;
        witness
    }

    /// Test/certificate tooling counterpart for the frozen reference basis.
    #[doc(hidden)]
    pub fn with_reference_basis_positions_for_replay(
        &self,
        reference_basis_positions: [usize; LOOPS],
    ) -> Self {
        let mut witness = self.clone();
        witness.reference_basis_positions = reference_basis_positions;
        witness
    }
}

/// A classified genuine corner and its exact inter-family map.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopGenuineClass {
    corner_type: FourLoopGenuineCornerType,
    witness: FourLoopGenuineWitness,
}

impl FourLoopGenuineClass {
    pub const fn corner_type(&self) -> FourLoopGenuineCornerType {
        self.corner_type
    }

    pub const fn witness(&self) -> &FourLoopGenuineWitness {
        &self.witness
    }

    pub fn into_witness(self) -> FourLoopGenuineWitness {
        self.witness
    }
}

#[derive(Clone, Debug)]
struct CatalogEntry {
    corner_type: FourLoopGenuineCornerType,
    signature: CanonicalSignature,
}

/// Owned classifier for one authenticated built-in parent.
#[derive(Clone, Debug)]
pub struct FourLoopGenuineClassifier {
    boundary: FourLoopBoundaryReducer,
    config: FourLoopGenuineConfig,
    catalog: Vec<CatalogEntry>,
}

impl FourLoopGenuineClassifier {
    pub fn build(
        topology: FourLoopTopology,
        config: FourLoopGenuineConfig,
    ) -> Result<Self, FourLoopGenuineError> {
        Self::new(topology, equal_mass_four_loop_vacuum(topology)?, config)
    }

    pub fn new(
        topology: FourLoopTopology,
        family: VacuumFamily,
        config: FourLoopGenuineConfig,
    ) -> Result<Self, FourLoopGenuineError> {
        // Authenticate the topology-specific public surface before reporting
        // catalog work limits for an unrelated or malformed family.
        let boundary = FourLoopBoundaryReducer::new(topology, family, config.boundary)?;
        let requested = FourLoopGenuineCornerType::ALL
            .iter()
            .map(|kind| signature_candidate_count(kind.physical_lines()))
            .fold(0_u128, u128::saturating_add);
        if requested > config.max_catalog_signature_candidates as u128 {
            return Err(FourLoopGenuineError::ResourceLimit {
                resource: "catalog signed ordered-basis presentations",
                requested,
                limit: config.max_catalog_signature_candidates as u128,
            });
        }

        let mut catalog: Vec<CatalogEntry> = Vec::with_capacity(10);
        for corner_type in FourLoopGenuineCornerType::ALL {
            let signature = canonical_signature(
                &reference_rows(corner_type),
                usize::MAX,
                config.max_ordered_basis_storage,
                "catalog signed ordered-basis presentations",
            )?;
            if let Some(previous) = catalog
                .iter()
                .find(|entry| entry.signature.values == signature.values)
            {
                return Err(FourLoopGenuineError::CatalogCollision {
                    left: previous.corner_type,
                    right: corner_type,
                });
            }
            catalog.push(CatalogEntry {
                corner_type,
                signature,
            });
        }
        Ok(Self {
            boundary,
            config,
            catalog,
        })
    }

    pub const fn topology(&self) -> FourLoopTopology {
        self.boundary.topology()
    }

    pub fn family(&self) -> &VacuumFamily {
        self.boundary.family()
    }

    pub const fn config(&self) -> FourLoopGenuineConfig {
        self.config
    }

    /// Return `None` for scaleless or factorized scalar corners.  Domain and
    /// resource failures remain typed errors.
    pub fn try_classify_integral(
        &self,
        integral: &Integral,
    ) -> Result<Option<FourLoopGenuineClass>, FourLoopGenuineError> {
        let FourLoopScalarClass::GenuineFourLoop { sector_mask, .. } =
            self.boundary.classify_integral(integral)?
        else {
            return Ok(None);
        };

        let rows = active_source_rows(self.family(), sector_mask);
        let source = canonical_signature(
            &rows,
            self.config.max_input_signature_candidates,
            self.config.max_ordered_basis_storage,
            "input signed ordered-basis presentations",
        )?;
        let reference = self
            .catalog
            .iter()
            .find(|entry| entry.signature.values == source.values)
            .ok_or(FourLoopGenuineError::UnknownGenuineSignature {
                topology: self.topology(),
                sector_mask,
            })?;

        let source_inverse =
            invert_matrix(&source.basis_matrix).map_err(FourLoopGenuineError::LinearAlgebra)?;
        let signed_axes =
            diagonal_sign_product(&source.axis_signs, &reference.signature.axis_signs);
        let loop_map = matrix_multiply(
            &matrix_multiply(&source_inverse, &signed_axes)
                .map_err(FourLoopGenuineError::LinearAlgebra)?,
            &reference.signature.basis_matrix,
        )
        .map_err(FourLoopGenuineError::LinearAlgebra)?;
        let determinant_sign = determinant_sign(&loop_map)?;
        let signed_line_matches = source
            .lines
            .iter()
            .zip(&reference.signature.lines)
            .map(|(source, reference)| FourLoopGenuineLineMatch {
                source_physical_position: source.element_position,
                reference_physical_position: reference.element_position,
                orientation_sign: source.orientation_sign * reference.orientation_sign,
            })
            .collect();

        let witness = FourLoopGenuineWitness {
            source_topology: self.topology(),
            source_sector_mask: sector_mask,
            corner_type: reference.corner_type,
            source_basis_positions: source
                .basis_positions
                .try_into()
                .expect("a four-loop signature has four basis positions"),
            reference_basis_positions: reference
                .signature
                .basis_positions
                .clone()
                .try_into()
                .expect("a four-loop signature has four basis positions"),
            loop_map: array::from_fn(|row| array::from_fn(|column| loop_map[row][column].clone())),
            determinant_sign,
            signed_line_matches,
        };
        self.replay_witness(&witness)?;
        Ok(Some(FourLoopGenuineClass {
            corner_type: reference.corner_type,
            witness,
        }))
    }

    pub fn classify_integral(
        &self,
        integral: &Integral,
    ) -> Result<FourLoopGenuineClass, FourLoopGenuineError> {
        self.try_classify_integral(integral)?.ok_or_else(|| {
            FourLoopGenuineError::NotGenuineFourLoopCorner {
                integral: integral.clone(),
            }
        })
    }

    /// Replay a witness without canonical search or dependence on signature
    /// resource limits.
    pub fn replay_witness(
        &self,
        witness: &FourLoopGenuineWitness,
    ) -> Result<FourLoopGenuineCornerType, FourLoopGenuineError> {
        if witness.source_topology != self.topology()
            || witness.source_sector_mask == 0
            || witness.source_sector_mask & !((1_u16 << self.family().propagator_count()) - 1) != 0
            || witness.corner_type.physical_lines() != witness.signed_line_matches.len()
        {
            return Err(FourLoopGenuineError::WitnessMismatch);
        }
        let source_active =
            positions_in_mask(witness.source_sector_mask, self.family().propagator_count());
        let reference_active = positions_in_mask(
            witness.corner_type.reference_mask(),
            witness.corner_type.reference_topology().routings().len(),
        );
        if source_active.len() != reference_active.len()
            || !is_basis_subset(&witness.source_basis_positions, &source_active)
            || !is_basis_subset(&witness.reference_basis_positions, &reference_active)
        {
            return Err(FourLoopGenuineError::WitnessMismatch);
        }

        let loop_map = witness
            .loop_map
            .iter()
            .map(|row| row.to_vec())
            .collect::<Vec<_>>();
        if determinant_sign(&loop_map)? != witness.determinant_sign {
            return Err(FourLoopGenuineError::WitnessMismatch);
        }

        // Authenticate both ordered basis arrays against the stored map.  It
        // is insufficient that they are four distinct active positions: a
        // tampered active subset must not be accepted as certificate metadata.
        // In particular, paired singular subsets could otherwise satisfy all
        // line-map checks while falsely claiming to be loop bases.
        let source_basis = witness
            .source_basis_positions
            .iter()
            .map(|&position| {
                self.family().denominators()[position]
                    .momentum()
                    .ok_or(FourLoopGenuineError::WitnessMismatch)
                    .map(<[ExactRational]>::to_vec)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let reference_basis = witness
            .reference_basis_positions
            .iter()
            .map(|&position| {
                Ok(
                    witness.corner_type.reference_topology().routings()[position]
                        .iter()
                        .map(|&value| ExactRational::from(i64::from(value)))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Result<Vec<_>, FourLoopGenuineError>>()?;
        for basis in [&source_basis, &reference_basis] {
            let determinant =
                matrix_determinant(basis).map_err(FourLoopGenuineError::LinearAlgebra)?;
            if determinant != ExactRational::one() && determinant != -ExactRational::one() {
                return Err(FourLoopGenuineError::WitnessMismatch);
            }
        }
        for slot in 0..LOOPS {
            let source_position = witness.source_basis_positions[slot];
            let reference_position = witness.reference_basis_positions[slot];
            let source = self.family().denominators()[source_position]
                .momentum()
                .ok_or(FourLoopGenuineError::WitnessMismatch)?;
            let mapped = row_times_matrix(source, &loop_map);
            let reference = witness.corner_type.reference_topology().routings()[reference_position]
                .iter()
                .map(|&value| ExactRational::from(i64::from(value)))
                .collect::<Vec<_>>();
            let line_match = witness
                .signed_line_matches
                .iter()
                .find(|line| {
                    line.source_physical_position == source_position
                        && line.reference_physical_position == reference_position
                })
                .ok_or(FourLoopGenuineError::WitnessMismatch)?;
            let expected = reference
                .iter()
                .map(|value| value * &ExactRational::from(i64::from(line_match.orientation_sign)))
                .collect::<Vec<_>>();
            if mapped != expected {
                return Err(FourLoopGenuineError::WitnessMismatch);
            }
        }

        let mut seen_source = Vec::with_capacity(source_active.len());
        let mut seen_reference = Vec::with_capacity(reference_active.len());
        for line_match in &witness.signed_line_matches {
            if line_match.orientation_sign.unsigned_abs() != 1
                || !source_active.contains(&line_match.source_physical_position)
                || !reference_active.contains(&line_match.reference_physical_position)
                || seen_source.contains(&line_match.source_physical_position)
                || seen_reference.contains(&line_match.reference_physical_position)
            {
                return Err(FourLoopGenuineError::WitnessMismatch);
            }
            seen_source.push(line_match.source_physical_position);
            seen_reference.push(line_match.reference_physical_position);

            let source = self.family().denominators()[line_match.source_physical_position]
                .momentum()
                .ok_or(FourLoopGenuineError::WitnessMismatch)?;
            let mapped = row_times_matrix(source, &loop_map);
            let expected = witness.corner_type.reference_topology().routings()
                [line_match.reference_physical_position]
                .iter()
                .map(|&value| {
                    ExactRational::from(i64::from(value) * i64::from(line_match.orientation_sign))
                })
                .collect::<Vec<_>>();
            if mapped != expected {
                return Err(FourLoopGenuineError::WitnessMismatch);
            }
        }
        seen_source.sort_unstable();
        seen_reference.sort_unstable();
        if seen_source != source_active || seen_reference != reference_active {
            return Err(FourLoopGenuineError::WitnessMismatch);
        }
        Ok(witness.corner_type)
    }
}

fn active_source_rows(family: &VacuumFamily, sector_mask: u16) -> Vec<(usize, Vec<ExactRational>)> {
    positions_in_mask(sector_mask, family.propagator_count())
        .into_iter()
        .map(|position| {
            (
                position,
                family.denominators()[position]
                    .momentum()
                    .expect("an authenticated physical line has a momentum")
                    .to_vec(),
            )
        })
        .collect()
}

fn reference_rows(kind: FourLoopGenuineCornerType) -> Vec<(usize, Vec<ExactRational>)> {
    let topology = kind.reference_topology();
    positions_in_mask(kind.reference_mask(), topology.routings().len())
        .into_iter()
        .map(|position| {
            (
                position,
                topology.routings()[position]
                    .iter()
                    .map(|&value| ExactRational::from(i64::from(value)))
                    .collect(),
            )
        })
        .collect()
}

fn positions_in_mask(mask: u16, positions: usize) -> Vec<usize> {
    (0..positions)
        .filter(|&position| mask & (1_u16 << position) != 0)
        .collect()
}

fn is_basis_subset(basis: &[usize; LOOPS], active: &[usize]) -> bool {
    basis.iter().all(|position| active.contains(position))
        && (0..LOOPS).all(|left| (left + 1..LOOPS).all(|right| basis[left] != basis[right]))
}

#[derive(Clone, Debug)]
struct CanonicalSignature {
    basis_positions: Vec<usize>,
    basis_matrix: Vec<Vec<ExactRational>>,
    axis_signs: Vec<i8>,
    lines: Vec<CanonicalLine>,
    values: Vec<Vec<ExactRational>>,
}

#[derive(Clone, Debug)]
struct CanonicalLine {
    element_position: usize,
    normalized: Vec<ExactRational>,
    orientation_sign: i8,
}

fn canonical_signature(
    rows: &[(usize, Vec<ExactRational>)],
    max_candidates: usize,
    max_ordered_basis_storage: usize,
    resource: &'static str,
) -> Result<CanonicalSignature, FourLoopGenuineError> {
    let requested = signature_candidate_count(rows.len());
    if requested > max_candidates as u128 {
        return Err(FourLoopGenuineError::ResourceLimit {
            resource,
            requested,
            limit: max_candidates as u128,
        });
    }
    let ordered_bases = ordered_basis_candidate_count(rows.len());
    if ordered_bases > max_ordered_basis_storage as u128 {
        return Err(FourLoopGenuineError::ResourceLimit {
            resource: "ordered-basis candidate storage",
            requested: ordered_bases,
            limit: max_ordered_basis_storage as u128,
        });
    }
    let mut best: Option<CanonicalSignature> = None;
    for indices in ordered_selections(rows.len(), LOOPS) {
        let basis = indices
            .iter()
            .map(|&index| rows[index].1.clone())
            .collect::<Vec<_>>();
        let determinant =
            matrix_determinant(&basis).map_err(FourLoopGenuineError::LinearAlgebra)?;
        if determinant != ExactRational::one() && determinant != -ExactRational::one() {
            continue;
        }
        let inverse = invert_matrix(&basis).map_err(FourLoopGenuineError::LinearAlgebra)?;
        let coordinates = rows
            .iter()
            .map(|(_, row)| row_times_matrix(row, &inverse))
            .collect::<Vec<_>>();
        for signs in 0..(1_usize << LOOPS) {
            let mut lines = coordinates
                .iter()
                .enumerate()
                .map(|(line, row)| {
                    let signed = row
                        .iter()
                        .enumerate()
                        .map(|(axis, value)| {
                            if signs & (1 << axis) == 0 {
                                value.clone()
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
                axis_signs: (0..LOOPS)
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
    best.ok_or(FourLoopGenuineError::NoUnimodularBasis)
}

fn signature_candidate_count(lines: usize) -> u128 {
    ordered_basis_candidate_count(lines).saturating_mul(SIGN_CHOICES)
}

fn ordered_basis_candidate_count(lines: usize) -> u128 {
    (0..LOOPS).fold(1_u128, |value, offset| {
        value.saturating_mul(lines.saturating_sub(offset) as u128)
    })
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
    let mut output = Vec::new();
    if choose <= length {
        visit(
            length,
            choose,
            &mut vec![false; length],
            &mut Vec::new(),
            &mut output,
        );
    }
    output
}

fn row_times_matrix(row: &[ExactRational], matrix: &[Vec<ExactRational>]) -> Vec<ExactRational> {
    (0..matrix[0].len())
        .map(|column| {
            row.iter()
                .zip(matrix)
                .map(|(left, right)| left * &right[column])
                .fold(ExactRational::zero(), |sum, value| sum + value)
        })
        .collect()
}

fn normalize_squared_routing(mut row: Vec<ExactRational>) -> (Vec<ExactRational>, i8) {
    let mut orientation_sign = 1;
    if row
        .iter()
        .find(|value| !value.is_zero())
        .is_some_and(|value| value.is_negative())
    {
        orientation_sign = -1;
        for value in &mut row {
            *value = -&*value;
        }
    }
    (row, orientation_sign)
}

fn diagonal_sign_product(left: &[i8], right: &[i8]) -> Vec<Vec<ExactRational>> {
    (0..LOOPS)
        .map(|row| {
            (0..LOOPS)
                .map(|column| {
                    if row == column {
                        ExactRational::from(i64::from(left[row] * right[row]))
                    } else {
                        ExactRational::zero()
                    }
                })
                .collect()
        })
        .collect()
}

fn determinant_sign(matrix: &[Vec<ExactRational>]) -> Result<i8, FourLoopGenuineError> {
    let determinant = matrix_determinant(matrix).map_err(FourLoopGenuineError::LinearAlgebra)?;
    if determinant.is_one() {
        Ok(1)
    } else if determinant == -ExactRational::one() {
        Ok(-1)
    } else {
        Err(FourLoopGenuineError::WitnessMismatch)
    }
}

#[derive(Debug)]
pub enum FourLoopGenuineError {
    Family(FamilyError),
    Boundary(FourLoopBoundaryError),
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    CatalogCollision {
        left: FourLoopGenuineCornerType,
        right: FourLoopGenuineCornerType,
    },
    NoUnimodularBasis,
    UnknownGenuineSignature {
        topology: FourLoopTopology,
        sector_mask: u16,
    },
    NotGenuineFourLoopCorner {
        integral: Integral,
    },
    LinearAlgebra(String),
    WitnessMismatch,
}

impl From<FamilyError> for FourLoopGenuineError {
    fn from(error: FamilyError) -> Self {
        Self::Family(error)
    }
}

impl From<FourLoopBoundaryError> for FourLoopGenuineError {
    fn from(error: FourLoopBoundaryError) -> Self {
        Self::Boundary(error)
    }
}

impl fmt::Display for FourLoopGenuineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => write!(formatter, "{error}"),
            Self::Boundary(error) => write!(formatter, "{error}"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "four-loop genuine-corner {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::CatalogCollision { left, right } => {
                write!(
                    formatter,
                    "frozen four-loop types {left} and {right} have the same signature"
                )
            }
            Self::NoUnimodularBasis => {
                formatter.write_str("genuine four-loop routing has no unimodular loop basis")
            }
            Self::UnknownGenuineSignature {
                topology,
                sector_mask,
            } => write!(
                formatter,
                "genuine {topology:?} corner {sector_mask:#x} is absent from the frozen catalog"
            ),
            Self::NotGenuineFourLoopCorner { integral } => {
                write!(
                    formatter,
                    "{integral} is scaleless or factorized, not a genuine four-loop corner"
                )
            }
            Self::LinearAlgebra(error) => write!(formatter, "exact linear algebra failed: {error}"),
            Self::WitnessMismatch => {
                formatter.write_str("four-loop genuine-corner witness does not replay")
            }
        }
    }
}

impl std::error::Error for FourLoopGenuineError {}
