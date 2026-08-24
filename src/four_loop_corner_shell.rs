//! Replayable sparse certificate for the 160 native four-loop corner rows.
//!
//! The ten frozen genuine H/X corners each emit sixteen raw momentum-space
//! IBPs.  This module transports those rows through the authenticated affine
//! halo maps, recursively dispatches proper sectors, removes the common mass
//! dimension, and performs deterministic exact sparse elimination.
//!
//! The scalar `D1/N0` factorized halo is retained first as typed
//! [`UnsupportedBoundaryHalo`] provenance, preflighted as one finite batch,
//! and then closed by fixed lower-component formulae before canonicalization
//! and elimination.  Unsupported terms outside that certified box would still
//! withhold their rows; the default 160-row shell currently has none.

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::coefficient::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use crate::{
    Coefficient, CoefficientContext, Denominator, ExactRational, FamilyError,
    FourLoopBoundaryError, FourLoopBoundaryHaloConfig, FourLoopBoundaryHaloError,
    FourLoopBoundaryHaloPlan, FourLoopBoundaryHaloReducer, FourLoopBoundaryHaloStats,
    FourLoopBoundaryReducer, FourLoopFactorizationWitness, FourLoopGenuineClassifier,
    FourLoopGenuineConfig, FourLoopGenuineCornerType, FourLoopGenuineError, FourLoopHaloColumnKey,
    FourLoopHaloConfig, FourLoopHaloError, FourLoopHaloMapper, FourLoopScalarClass,
    FourLoopTopology, IbpGenerationError, IbpGenerator, Integral, MassiveVacuumMaster,
    MasterProduct, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT, VacuumFamily,
};

const LOOPS: usize = 4;
const BASIS: usize = 10;

/// Exact structural limits derived in section 10 of the reduction plan.
pub const FOUR_LOOP_CORNER_SHELL_RAW_ROWS: usize = 160;
pub const FOUR_LOOP_CORNER_SHELL_GLOBAL_COLUMN_BOUND: usize = 736;
pub const FOUR_LOOP_CORNER_SHELL_RAW_TERM_INCIDENCE_BOUND: usize = 12_712;
pub const FOUR_LOOP_CORNER_SHELL_NORMALIZATION_CONTRIBUTION_BOUND: usize = 139_832;
pub const FOUR_LOOP_CORNER_SHELL_COLLECTED_NONZERO_BOUND: usize = 117_760;
pub const FOUR_LOOP_CORNER_SHELL_ELIMINATION_UPDATE_BOUND: usize = 18_723_840;
pub const FOUR_LOOP_CORNER_SHELL_SOURCE_WEIGHT_BOUND: usize = 25_600;

/// Resource limits for the exact native corner shell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopCornerShellConfig {
    pub genuine: FourLoopGenuineConfig,
    pub halo: FourLoopHaloConfig,
    pub boundary_halo: FourLoopBoundaryHaloConfig,
    pub max_raw_rows: usize,
    pub max_global_columns: usize,
    pub max_raw_term_incidences: usize,
    pub max_normalization_contributions: usize,
    pub max_collected_nonzeros: usize,
    pub max_elimination_updates: usize,
    pub max_source_row_weights: usize,
    pub max_cached_sector_mappers: usize,
    pub max_recursion_depth: usize,
    /// Conservative per-variable numerator/denominator exponent ceiling for
    /// shell-owned Symbolica operations.  Symbolica's hard `u16` ceiling is
    /// checked independently.
    pub max_coefficient_degree: usize,
}

impl Default for FourLoopCornerShellConfig {
    fn default() -> Self {
        Self {
            genuine: FourLoopGenuineConfig::default(),
            halo: FourLoopHaloConfig::default(),
            boundary_halo: FourLoopBoundaryHaloConfig::default(),
            max_raw_rows: FOUR_LOOP_CORNER_SHELL_RAW_ROWS,
            max_global_columns: FOUR_LOOP_CORNER_SHELL_GLOBAL_COLUMN_BOUND,
            max_raw_term_incidences: FOUR_LOOP_CORNER_SHELL_RAW_TERM_INCIDENCE_BOUND,
            max_normalization_contributions:
                FOUR_LOOP_CORNER_SHELL_NORMALIZATION_CONTRIBUTION_BOUND,
            max_collected_nonzeros: FOUR_LOOP_CORNER_SHELL_COLLECTED_NONZERO_BOUND,
            max_elimination_updates: FOUR_LOOP_CORNER_SHELL_ELIMINATION_UPDATE_BOUND,
            max_source_row_weights: FOUR_LOOP_CORNER_SHELL_SOURCE_WEIGHT_BOUND,
            max_cached_sector_mappers: 2 * (1 << 9),
            max_recursion_depth: BASIS,
            max_coefficient_degree: 4_096,
        }
    }
}

/// The two frozen completed-basis families used by the global atlas.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FourLoopReferenceTopology {
    H,
    X,
}

impl FourLoopReferenceTopology {
    pub const fn as_topology(self) -> FourLoopTopology {
        match self {
            Self::H => FourLoopTopology::H,
            Self::X => FourLoopTopology::X,
        }
    }

    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::H => "H",
            Self::X => "X",
        }
    }
}

impl TryFrom<FourLoopTopology> for FourLoopReferenceTopology {
    type Error = FourLoopCornerShellError;

    fn try_from(topology: FourLoopTopology) -> Result<Self, Self::Error> {
        match topology {
            FourLoopTopology::H => Ok(Self::H),
            FourLoopTopology::X => Ok(Self::X),
            FourLoopTopology::Bmw | FourLoopTopology::Fg => {
                Err(FourLoopCornerShellError::NonReferenceTopology { topology })
            }
        }
    }
}

/// Stable provenance label for one of the ten-by-sixteen native raw rows.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FourLoopCornerRawRowId {
    corner_type: FourLoopGenuineCornerType,
    differentiated_loop: u8,
    contraction_loop: u8,
}

impl FourLoopCornerRawRowId {
    pub const SCHEMA: &'static str = "rustred-equal-mass-euclidean-four-loop-corner-raw-row-v1";

    pub const fn new(
        corner_type: FourLoopGenuineCornerType,
        differentiated_loop: u8,
        contraction_loop: u8,
    ) -> Self {
        Self {
            corner_type,
            differentiated_loop,
            contraction_loop,
        }
    }

    pub const fn corner_type(self) -> FourLoopGenuineCornerType {
        self.corner_type
    }

    pub const fn differentiated_loop(self) -> u8 {
        self.differentiated_loop
    }

    pub const fn contraction_loop(self) -> u8 {
        self.contraction_loop
    }

    pub fn stable_key(self) -> String {
        format!(
            "{}:{}:d{}:k{}",
            Self::SCHEMA,
            self.corner_type.stable_key(),
            self.differentiated_loop,
            self.contraction_loop
        )
    }
}

/// Disjoint, versioned global matrix-column identifier.
///
/// There is deliberately no zero variant: proved zero terms are omitted.
/// Genuine powers always contain exactly ten entries in the frozen family of
/// `corner_type`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FourLoopCornerColumnId {
    Product(MasterProduct<MassiveVacuumMaster>),
    Genuine {
        corner_type: FourLoopGenuineCornerType,
        powers: [i32; BASIS],
    },
}

impl FourLoopCornerColumnId {
    pub const SCHEMA: &'static str = FourLoopHaloColumnKey::SCHEMA;

    pub fn stable_key(&self) -> String {
        match self {
            Self::Product(product) => {
                FourLoopHaloColumnKey::Factorized(product.clone()).stable_key()
            }
            Self::Genuine {
                corner_type,
                powers,
            } => FourLoopHaloColumnKey::GenuineRepresentative {
                corner_type: *corner_type,
                integral: Integral::from(*powers),
            }
            .stable_key(),
        }
    }

    /// Mass weight used by the homogeneous `J_b=(m2)^w I_b` basis.
    pub fn mass_weight(&self) -> i64 {
        match self {
            Self::Product(product) => product
                .factors()
                .iter()
                .map(|(master, multiplicity)| {
                    i64::from(*multiplicity)
                        * i64::try_from(master.physical_lines()).expect("small master line count")
                })
                .sum(),
            Self::Genuine { powers, .. } => powers.iter().map(|&power| i64::from(power)).sum(),
        }
    }

    fn order_key(&self) -> FourLoopColumnOrderKey<'_> {
        match self {
            Self::Product(_) => FourLoopColumnOrderKey::Product(self.stable_key()),
            Self::Genuine {
                corner_type,
                powers,
            } => {
                let mask = corner_type.reference_mask();
                let mut dots = 0_u32;
                let mut numerators = 0_u32;
                for (position, &power) in powers.iter().enumerate() {
                    if mask & (1_u16 << position) != 0 {
                        dots = dots
                            .saturating_add(u32::try_from(power.saturating_sub(1).max(0)).unwrap());
                    } else {
                        numerators =
                            numerators.saturating_add(power.saturating_neg().max(0) as u32);
                    }
                }
                FourLoopColumnOrderKey::Genuine {
                    active_lines: corner_type.physical_lines(),
                    degree: dots.saturating_add(numerators),
                    dots,
                    corner_type: corner_type.stable_key(),
                    powers,
                }
            }
        }
    }
}

enum FourLoopColumnOrderKey<'a> {
    Product(String),
    Genuine {
        active_lines: usize,
        degree: u32,
        dots: u32,
        corner_type: &'static str,
        powers: &'a [i32; BASIS],
    },
}

impl Ord for FourLoopCornerColumnId {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.order_key(), other.order_key()) {
            (FourLoopColumnOrderKey::Product(left), FourLoopColumnOrderKey::Product(right)) => {
                left.cmp(&right)
            }
            (FourLoopColumnOrderKey::Product(_), FourLoopColumnOrderKey::Genuine { .. }) => {
                Ordering::Less
            }
            (FourLoopColumnOrderKey::Genuine { .. }, FourLoopColumnOrderKey::Product(_)) => {
                Ordering::Greater
            }
            (
                FourLoopColumnOrderKey::Genuine {
                    active_lines: left_active,
                    degree: left_degree,
                    dots: left_dots,
                    corner_type: left_type,
                    powers: left_powers,
                },
                FourLoopColumnOrderKey::Genuine {
                    active_lines: right_active,
                    degree: right_degree,
                    dots: right_dots,
                    corner_type: right_type,
                    powers: right_powers,
                },
            ) => (left_active, left_degree, left_dots, left_type, left_powers).cmp(&(
                right_active,
                right_degree,
                right_dots,
                right_type,
                right_powers,
            )),
        }
    }
}

impl PartialOrd for FourLoopCornerColumnId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Replaceable lower-sector service input for an unsupported D1/N1 branch.
///
/// The factorization witness is retained so a later native lower-loop closure
/// service can consume this record directly rather than reclassifying or
/// changing the persisted schema.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsupportedBoundaryHalo {
    topology: FourLoopReferenceTopology,
    integral: Integral,
    product: MasterProduct<MassiveVacuumMaster>,
    witness: FourLoopFactorizationWitness,
    coefficient: Coefficient,
}

/// Replayable substitution of one normalized factorized halo occurrence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopBoundaryHaloClosure {
    raw_id: FourLoopCornerRawRowId,
    blocker: UnsupportedBoundaryHalo,
    dotted_component: MassiveVacuumMaster,
    compact_reference_position: usize,
    mass_normalized_output: BTreeMap<MasterProduct<MassiveVacuumMaster>, Coefficient>,
}

impl FourLoopBoundaryHaloClosure {
    pub const fn raw_id(&self) -> FourLoopCornerRawRowId {
        self.raw_id
    }

    pub const fn blocker(&self) -> &UnsupportedBoundaryHalo {
        &self.blocker
    }

    pub const fn dotted_component(&self) -> MassiveVacuumMaster {
        self.dotted_component
    }

    pub const fn compact_reference_position(&self) -> usize {
        self.compact_reference_position
    }

    pub const fn mass_normalized_output(
        &self,
    ) -> &BTreeMap<MasterProduct<MassiveVacuumMaster>, Coefficient> {
        &self.mass_normalized_output
    }
}

impl UnsupportedBoundaryHalo {
    pub const SCHEMA: &'static str =
        "rustred-equal-mass-euclidean-four-loop-unsupported-boundary-halo-v1";

    pub const fn topology(&self) -> FourLoopReferenceTopology {
        self.topology
    }

    pub const fn integral(&self) -> &Integral {
        &self.integral
    }

    pub const fn product(&self) -> &MasterProduct<MassiveVacuumMaster> {
        &self.product
    }

    pub const fn witness(&self) -> &FourLoopFactorizationWitness {
        &self.witness
    }

    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }

    pub fn stable_key(&self) -> String {
        let powers = self
            .integral
            .powers()
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(",");
        let product = FourLoopHaloColumnKey::Factorized(self.product.clone()).stable_key();
        format!(
            "{}:{}:{}:[{}]",
            Self::SCHEMA,
            self.topology.stable_key(),
            product,
            powers
        )
    }

    pub fn dot_degree(&self) -> u32 {
        self.integral
            .powers()
            .iter()
            .enumerate()
            .filter(|(position, _)| self.witness.sector_mask() & (1_u16 << position) != 0)
            .map(|(_, &power)| u32::try_from(power.saturating_sub(1).max(0)).unwrap())
            .sum()
    }

    pub fn numerator_degree(&self) -> u32 {
        self.integral
            .powers()
            .iter()
            .map(|&power| power.saturating_neg().max(0) as u32)
            .sum()
    }
}

/// Deterministic grouping key for the exact blocker census.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FourLoopBoundaryHaloCensusKey {
    topology: FourLoopReferenceTopology,
    sector_mask: u16,
    dot_degree: u32,
    numerator_degree: u32,
    product: MasterProduct<MassiveVacuumMaster>,
}

impl FourLoopBoundaryHaloCensusKey {
    pub fn from_blocker(blocker: &UnsupportedBoundaryHalo) -> Self {
        Self {
            topology: blocker.topology,
            sector_mask: blocker.witness.sector_mask(),
            dot_degree: blocker.dot_degree(),
            numerator_degree: blocker.numerator_degree(),
            product: blocker.product.clone(),
        }
    }

    pub const fn topology(&self) -> FourLoopReferenceTopology {
        self.topology
    }

    pub const fn sector_mask(&self) -> u16 {
        self.sector_mask
    }

    pub const fn dot_degree(&self) -> u32 {
        self.dot_degree
    }

    pub const fn numerator_degree(&self) -> u32 {
        self.numerator_degree
    }

    pub const fn product(&self) -> &MasterProduct<MassiveVacuumMaster> {
        &self.product
    }
}

/// One complete, mass-normalized raw row admitted to sparse elimination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopCornerNormalizedRow {
    raw_id: FourLoopCornerRawRowId,
    seed_mass_weight: i64,
    /// Coefficient of the hardest column before canonical row division.
    /// The stored entries equal the mass-normalized row divided by this value.
    row_scale: Coefficient,
    entries: BTreeMap<FourLoopCornerColumnId, Coefficient>,
}

impl FourLoopCornerNormalizedRow {
    pub const fn raw_id(&self) -> FourLoopCornerRawRowId {
        self.raw_id
    }

    pub const fn seed_mass_weight(&self) -> i64 {
        self.seed_mass_weight
    }

    pub const fn row_scale(&self) -> &Coefficient {
        &self.row_scale
    }

    pub const fn entries(&self) -> &BTreeMap<FourLoopCornerColumnId, Coefficient> {
        &self.entries
    }
}

/// Exact partially normalized row withheld from elimination.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopCornerBlockedRow {
    raw_id: FourLoopCornerRawRowId,
    seed_mass_weight: i64,
    supported_entries: BTreeMap<FourLoopCornerColumnId, Coefficient>,
    unsupported_boundary_halo: Vec<UnsupportedBoundaryHalo>,
}

impl FourLoopCornerBlockedRow {
    pub const fn raw_id(&self) -> FourLoopCornerRawRowId {
        self.raw_id
    }

    pub const fn seed_mass_weight(&self) -> i64 {
        self.seed_mass_weight
    }

    /// Supported portion only; this is not an equation without the blocker
    /// terms returned by [`Self::unsupported_boundary_halo`].
    pub const fn supported_entries(&self) -> &BTreeMap<FourLoopCornerColumnId, Coefficient> {
        &self.supported_entries
    }

    pub fn unsupported_boundary_halo(&self) -> &[UnsupportedBoundaryHalo] {
        &self.unsupported_boundary_halo
    }
}

/// Deterministic triangular rule `pivot = rhs` with exact input-row proof.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopCornerPivotRule {
    pivot: FourLoopCornerColumnId,
    rhs: BTreeMap<FourLoopCornerColumnId, Coefficient>,
    source_row_weights: BTreeMap<FourLoopCornerRawRowId, Coefficient>,
}

impl FourLoopCornerPivotRule {
    pub const fn pivot(&self) -> &FourLoopCornerColumnId {
        &self.pivot
    }

    pub const fn rhs(&self) -> &BTreeMap<FourLoopCornerColumnId, Coefficient> {
        &self.rhs
    }

    /// Exact sparse combination of stored normalized input rows which equals
    /// `pivot - rhs`.
    pub const fn source_row_weights(&self) -> &BTreeMap<FourLoopCornerRawRowId, Coefficient> {
        &self.source_row_weights
    }
}

/// Whether every raw row reached the global product/genuine column space.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopCornerShellStatus {
    Complete,
    PartialUnsupportedBoundaryHalo,
}

/// Maximal replayable certificate for the fixed native corner-row shell.
#[derive(Clone, Debug)]
pub struct FourLoopCornerShellCertificate {
    config: FourLoopCornerShellConfig,
    raw_row_ids: Vec<FourLoopCornerRawRowId>,
    normalized_rows: Vec<FourLoopCornerNormalizedRow>,
    blocked_rows: Vec<FourLoopCornerBlockedRow>,
    blocker_census: BTreeMap<FourLoopBoundaryHaloCensusKey, usize>,
    /// Census retained before scalar `D1/N0` blockers were closed.
    preclosure_blocker_census: BTreeMap<FourLoopBoundaryHaloCensusKey, usize>,
    preclosure_blocked_rows: Vec<FourLoopCornerBlockedRow>,
    preclosure_blocker_rows: usize,
    preclosure_blocker_terms: usize,
    boundary_halo_stats: FourLoopBoundaryHaloStats,
    boundary_halo_closures: Vec<FourLoopBoundaryHaloClosure>,
    pivots: Vec<FourLoopCornerPivotRule>,
    free_columns: Vec<FourLoopCornerColumnId>,
    normalization_contributions: usize,
    elimination_updates: usize,
}

impl FourLoopCornerShellCertificate {
    pub const SCHEMA: &'static str =
        "rustred-equal-mass-euclidean-four-loop-corner-shell-certificate-v1";

    pub fn build(config: FourLoopCornerShellConfig) -> Result<Self, FourLoopCornerShellError> {
        preflight_config(config)?;
        CornerShellBuilder::new(config)?.build(true)
    }

    pub const fn config(&self) -> FourLoopCornerShellConfig {
        self.config
    }

    pub fn status(&self) -> FourLoopCornerShellStatus {
        if self.blocked_rows.is_empty() {
            FourLoopCornerShellStatus::Complete
        } else {
            FourLoopCornerShellStatus::PartialUnsupportedBoundaryHalo
        }
    }

    pub fn is_complete(&self) -> bool {
        self.status() == FourLoopCornerShellStatus::Complete
    }

    pub fn raw_row_ids(&self) -> &[FourLoopCornerRawRowId] {
        &self.raw_row_ids
    }

    pub fn normalized_rows(&self) -> &[FourLoopCornerNormalizedRow] {
        &self.normalized_rows
    }

    pub fn blocked_rows(&self) -> &[FourLoopCornerBlockedRow] {
        &self.blocked_rows
    }

    pub fn blocker_census(&self) -> &BTreeMap<FourLoopBoundaryHaloCensusKey, usize> {
        &self.blocker_census
    }

    pub fn blocker_term_count(&self) -> usize {
        self.blocker_census.values().sum()
    }

    pub fn preclosure_blocker_census(&self) -> &BTreeMap<FourLoopBoundaryHaloCensusKey, usize> {
        &self.preclosure_blocker_census
    }

    /// Immutable normalized blocker provenance collected before closure.
    pub fn preclosure_blocked_rows(&self) -> &[FourLoopCornerBlockedRow] {
        &self.preclosure_blocked_rows
    }

    pub const fn preclosure_blocker_row_count(&self) -> usize {
        self.preclosure_blocker_rows
    }

    pub const fn preclosure_blocker_term_count(&self) -> usize {
        self.preclosure_blocker_terms
    }

    pub const fn boundary_halo_stats(&self) -> FourLoopBoundaryHaloStats {
        self.boundary_halo_stats
    }

    pub fn boundary_halo_closures(&self) -> &[FourLoopBoundaryHaloClosure] {
        &self.boundary_halo_closures
    }

    pub fn pivots(&self) -> &[FourLoopCornerPivotRule] {
        &self.pivots
    }

    pub fn rank(&self) -> usize {
        self.pivots.len()
    }

    /// Columns not pivoted by the fully normalized subset.  These are free in
    /// this finite shell only and are intentionally not called masters.
    pub fn free_unresolved_columns(&self) -> &[FourLoopCornerColumnId] {
        &self.free_columns
    }

    pub const fn normalization_contributions(&self) -> usize {
        self.normalization_contributions
    }

    pub const fn elimination_updates(&self) -> usize {
        self.elimination_updates
    }

    /// Replay every fully normalized original row and every pivot's exact
    /// source-row combination.  This performs no new signature search.
    pub fn replay(&self) -> Result<(), FourLoopCornerShellError> {
        replay_certificate(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct BoundaryHaloKey {
    topology: FourLoopReferenceTopology,
    integral: Integral,
    product: MasterProduct<MassiveVacuumMaster>,
}

#[derive(Clone)]
struct BoundaryHaloValue {
    witness: FourLoopFactorizationWitness,
    coefficient: Coefficient,
}

#[derive(Default)]
struct NormalizationOutcome {
    supported: BTreeMap<FourLoopCornerColumnId, Coefficient>,
    unsupported: BTreeMap<BoundaryHaloKey, BoundaryHaloValue>,
}

struct CollectedCornerRow {
    raw_id: FourLoopCornerRawRowId,
    seed_mass_weight: i64,
    outcome: NormalizationOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct SectorMapperKey {
    topology: FourLoopReferenceTopology,
    sector_mask: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BoundaryHaloPlanKey {
    topology: FourLoopReferenceTopology,
    sector_mask: u16,
    product: MasterProduct<MassiveVacuumMaster>,
    witness: FourLoopFactorizationWitness,
}

struct CornerShellBuilder {
    config: FourLoopCornerShellConfig,
    coefficients: CoefficientContext,
    mass: Coefficient,
    mass_position: usize,
    h_classifier: FourLoopGenuineClassifier,
    x_classifier: FourLoopGenuineClassifier,
    h_boundary: FourLoopBoundaryReducer,
    x_boundary: FourLoopBoundaryReducer,
    boundary_halo: FourLoopBoundaryHaloReducer,
    mapper_cache: BTreeMap<SectorMapperKey, FourLoopHaloMapper>,
    normalization_contributions: usize,
}

impl CornerShellBuilder {
    fn new(config: FourLoopCornerShellConfig) -> Result<Self, FourLoopCornerShellError> {
        let coefficients = CoefficientContext::new(["d", "m2"]);
        let mass_position = coefficients
            .parameter_names()
            .iter()
            .position(|name| name == "m2")
            .ok_or(FourLoopCornerShellError::MissingMassParameter)?;
        let mass = coefficients
            .parameter("m2")
            .ok_or(FourLoopCornerShellError::MissingMassParameter)?;
        let h_family =
            reference_family_in_context(FourLoopReferenceTopology::H, coefficients.clone(), &mass)?;
        let x_family =
            reference_family_in_context(FourLoopReferenceTopology::X, coefficients.clone(), &mass)?;
        let h_boundary = FourLoopBoundaryReducer::new(
            FourLoopTopology::H,
            h_family.clone(),
            config.genuine.boundary,
        )?;
        let x_boundary = FourLoopBoundaryReducer::new(
            FourLoopTopology::X,
            x_family.clone(),
            config.genuine.boundary,
        )?;
        let boundary_halo =
            FourLoopBoundaryHaloReducer::new(h_boundary.clone(), config.boundary_halo)?;
        let h_classifier =
            FourLoopGenuineClassifier::new(FourLoopTopology::H, h_family, config.genuine)?;
        let x_classifier =
            FourLoopGenuineClassifier::new(FourLoopTopology::X, x_family, config.genuine)?;
        Ok(Self {
            config,
            coefficients,
            mass,
            mass_position,
            h_classifier,
            x_classifier,
            h_boundary,
            x_boundary,
            boundary_halo,
            mapper_cache: BTreeMap::new(),
            normalization_contributions: 0,
        })
    }

    fn build(
        mut self,
        replay_after_build: bool,
    ) -> Result<FourLoopCornerShellCertificate, FourLoopCornerShellError> {
        let mut raw_row_ids = Vec::with_capacity(FOUR_LOOP_CORNER_SHELL_RAW_ROWS);
        let mut collected_rows = Vec::with_capacity(FOUR_LOOP_CORNER_SHELL_RAW_ROWS);

        for corner_type in FourLoopGenuineCornerType::ALL {
            let topology = FourLoopReferenceTopology::try_from(corner_type.reference_topology())?;
            let seed = corner_seed(corner_type);
            let identities =
                IbpGenerator::new(self.classifier(topology).family()).try_generate_raw(&seed)?;
            if identities.len() != LOOPS * LOOPS {
                return Err(FourLoopCornerShellError::RawRowCountMismatch {
                    expected: LOOPS * LOOPS,
                    actual: identities.len(),
                });
            }
            let source_mapper = self.mapper_for(topology, &seed)?;
            for identity in identities {
                let differentiated_loop = u8::try_from(identity.differentiated_loop)
                    .map_err(|_| FourLoopCornerShellError::RawRowLabelOutOfRange)?;
                let contraction_loop = u8::try_from(identity.contraction_loop)
                    .map_err(|_| FourLoopCornerShellError::RawRowLabelOutOfRange)?;
                let raw_id =
                    FourLoopCornerRawRowId::new(corner_type, differentiated_loop, contraction_loop);
                if identity.seed != seed
                    || usize::from(differentiated_loop) >= LOOPS
                    || usize::from(contraction_loop) >= LOOPS
                {
                    return Err(FourLoopCornerShellError::RawRowProvenanceMismatch { raw_id });
                }
                raw_row_ids.push(raw_id);
                let mut outcome =
                    self.normalize_raw_equation(topology, &source_mapper, &identity.equation)?;
                let seed_weight = i64::try_from(corner_type.physical_lines())
                    .expect("a corner has at most nine physical lines");
                self.mass_normalize_supported(raw_id, seed_weight, &mut outcome.supported)?;
                self.mass_normalize_unsupported(raw_id, seed_weight, &mut outcome.unsupported)?;

                collected_rows.push(CollectedCornerRow {
                    raw_id,
                    seed_mass_weight: seed_weight,
                    outcome,
                });
            }
        }

        if raw_row_ids.len() != FOUR_LOOP_CORNER_SHELL_RAW_ROWS {
            return Err(FourLoopCornerShellError::RawRowCountMismatch {
                expected: FOUR_LOOP_CORNER_SHELL_RAW_ROWS,
                actual: raw_row_ids.len(),
            });
        }
        let unique = raw_row_ids.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != raw_row_ids.len() {
            return Err(FourLoopCornerShellError::DuplicateRawRowId);
        }

        let preclosure_blocked_rows = build_preclosure_blocked_rows(&collected_rows);
        let preclosure_blocker_census = build_blocker_census(&preclosure_blocked_rows);
        let preclosure_blocker_rows = preclosure_blocked_rows.len();
        let preclosure_blocker_terms = preclosure_blocked_rows
            .iter()
            .map(|row| row.unsupported_boundary_halo.len())
            .sum();
        let (boundary_halo_reservation, boundary_halo_plans) =
            self.prepare_boundary_halo_batch(&collected_rows)?;

        let mut normalized_rows = Vec::with_capacity(FOUR_LOOP_CORNER_SHELL_RAW_ROWS);
        let mut blocked_rows = Vec::new();
        let mut boundary_halo_closures = Vec::with_capacity(preclosure_blocker_terms);
        let mut actual_boundary_halo_stats = FourLoopBoundaryHaloStats::default();
        for row in collected_rows {
            let (mut supported, unsupported) = self.close_boundary_halo_row(
                row.raw_id,
                row.outcome,
                &boundary_halo_plans,
                &mut actual_boundary_halo_stats,
                &mut boundary_halo_closures,
            )?;
            if unsupported.is_empty() {
                let row_scale = self.canonicalize_row(&mut supported)?;
                normalized_rows.push(FourLoopCornerNormalizedRow {
                    raw_id: row.raw_id,
                    seed_mass_weight: row.seed_mass_weight,
                    row_scale,
                    entries: supported,
                });
            } else {
                let unsupported_boundary_halo = unsupported
                    .into_iter()
                    .map(|(key, value)| UnsupportedBoundaryHalo {
                        topology: key.topology,
                        integral: key.integral,
                        product: key.product,
                        witness: value.witness,
                        coefficient: value.coefficient,
                    })
                    .collect();
                blocked_rows.push(FourLoopCornerBlockedRow {
                    raw_id: row.raw_id,
                    seed_mass_weight: row.seed_mass_weight,
                    supported_entries: supported,
                    unsupported_boundary_halo,
                });
            }
        }
        actual_boundary_halo_stats.set_batch_shape(
            boundary_halo_reservation.blocker_occurrences(),
            boundary_halo_reservation.unique_witness_plans(),
            boundary_halo_reservation.signed_line_dispatches(),
            boundary_halo_reservation.output_products(),
        );
        self.boundary_halo
            .preflight_stats(actual_boundary_halo_stats)?;
        let boundary_halo_stats = actual_boundary_halo_stats;

        let collected_nonzeros = normalized_rows
            .iter()
            .map(|row| row.entries.len())
            .sum::<usize>();
        check_resource(
            "collected normalized nonzeros",
            collected_nonzeros as u128,
            self.config.max_collected_nonzeros as u128,
        )?;
        let global_columns = normalized_rows
            .iter()
            .flat_map(|row| row.entries.keys().cloned())
            .collect::<BTreeSet<_>>();
        check_resource(
            "global columns",
            global_columns.len() as u128,
            self.config.max_global_columns as u128,
        )?;

        let blocker_census = build_blocker_census(&blocked_rows);
        let (pivots, free_columns, elimination_updates) = eliminate_rows(&self, &normalized_rows)?;
        let certificate = FourLoopCornerShellCertificate {
            config: self.config,
            raw_row_ids,
            normalized_rows,
            blocked_rows,
            blocker_census,
            preclosure_blocker_census,
            preclosure_blocked_rows,
            preclosure_blocker_rows,
            preclosure_blocker_terms,
            boundary_halo_stats,
            boundary_halo_closures,
            pivots,
            free_columns,
            normalization_contributions: self.normalization_contributions,
            elimination_updates,
        };
        if replay_after_build {
            certificate.replay()?;
        }
        Ok(certificate)
    }

    fn classifier(&self, topology: FourLoopReferenceTopology) -> &FourLoopGenuineClassifier {
        match topology {
            FourLoopReferenceTopology::H => &self.h_classifier,
            FourLoopReferenceTopology::X => &self.x_classifier,
        }
    }

    fn boundary(&self, topology: FourLoopReferenceTopology) -> &FourLoopBoundaryReducer {
        match topology {
            FourLoopReferenceTopology::H => &self.h_boundary,
            FourLoopReferenceTopology::X => &self.x_boundary,
        }
    }

    fn prepare_boundary_halo_batch(
        &self,
        rows: &[CollectedCornerRow],
    ) -> Result<
        (
            FourLoopBoundaryHaloStats,
            Vec<(BoundaryHaloPlanKey, FourLoopBoundaryHaloPlan)>,
        ),
        FourLoopCornerShellError,
    > {
        let mut blocker_occurrences = 0_usize;
        let mut witness_plan_keys = Vec::<BoundaryHaloPlanKey>::new();
        let mut signed_line_dispatches = 0_usize;
        let mut conservative_product_multiplications = 0_usize;

        for (key, value) in rows.iter().flat_map(|row| row.outcome.unsupported.iter()) {
            blocker_occurrences = blocker_occurrences.checked_add(1).ok_or(
                FourLoopCornerShellError::ResourceLimit {
                    resource: "factorized boundary-halo blocker occurrences",
                    requested: u128::MAX,
                    limit: self.config.boundary_halo.max_blocker_occurrences as u128,
                },
            )?;
            conservative_product_multiplications = conservative_product_multiplications
                .checked_add(if key.product.multiplicity(&MassiveVacuumMaster::F5) == 1 {
                    3
                } else {
                    1
                })
                .ok_or(FourLoopCornerShellError::ResourceLimit {
                    resource: "factorized boundary-halo product multiplications",
                    requested: u128::MAX,
                    limit: self.config.boundary_halo.max_product_multiplications as u128,
                })?;
            let plan_key = BoundaryHaloPlanKey {
                topology: key.topology,
                sector_mask: value.witness.sector_mask(),
                product: key.product.clone(),
                witness: value.witness.clone(),
            };
            if !witness_plan_keys.contains(&plan_key) {
                signed_line_dispatches = signed_line_dispatches
                    .checked_add(value.witness.sector_mask().count_ones() as usize)
                    .ok_or(FourLoopCornerShellError::ResourceLimit {
                        resource: "factorized boundary-halo signed-line dispatches",
                        requested: u128::MAX,
                        limit: self.config.boundary_halo.max_signed_line_dispatches as u128,
                    })?;
                witness_plan_keys.push(plan_key);
            }
        }

        let requested = FourLoopBoundaryHaloStats::conservative_batch(
            blocker_occurrences,
            witness_plan_keys.len(),
            signed_line_dispatches,
            conservative_product_multiplications,
            if blocker_occurrences == 0 { 0 } else { 6 },
        );
        self.boundary_halo.preflight_stats(requested)?;
        self.boundary_halo.preflight_formula_table()?;
        let mut plans = Vec::with_capacity(witness_plan_keys.len());
        for key in witness_plan_keys {
            if key.topology != FourLoopReferenceTopology::H {
                continue;
            }
            let plan = self
                .boundary_halo
                .prepare_plan(&key.product, &key.witness)?;
            plans.push((key, plan));
        }
        Ok((requested, plans))
    }

    fn close_boundary_halo_row(
        &self,
        raw_id: FourLoopCornerRawRowId,
        outcome: NormalizationOutcome,
        plans: &[(BoundaryHaloPlanKey, FourLoopBoundaryHaloPlan)],
        stats: &mut FourLoopBoundaryHaloStats,
        closures: &mut Vec<FourLoopBoundaryHaloClosure>,
    ) -> Result<
        (
            BTreeMap<FourLoopCornerColumnId, Coefficient>,
            BTreeMap<BoundaryHaloKey, BoundaryHaloValue>,
        ),
        FourLoopCornerShellError,
    > {
        let mut supported = outcome.supported;
        let mut unsupported = BTreeMap::new();
        for (key, value) in outcome.unsupported {
            if key.topology != FourLoopReferenceTopology::H
                || blocker_dot_degree(&key, &value.witness) != 1
                || blocker_numerator_degree(&key) != 0
            {
                unsupported.insert(key, value);
                continue;
            }
            let plan_key = BoundaryHaloPlanKey {
                topology: key.topology,
                sector_mask: value.witness.sector_mask(),
                product: key.product.clone(),
                witness: value.witness.clone(),
            };
            let plan = plans
                .iter()
                .find_map(|(candidate, plan)| (candidate == &plan_key).then_some(plan))
                .ok_or_else(|| FourLoopCornerShellError::MissingBoundaryHaloPlan {
                    integral: key.integral.clone(),
                })?;
            let reduction = self.boundary_halo.reduce_with_plan(&key.integral, plan)?;
            stats.add_request(reduction.stats())?;
            let blocker = UnsupportedBoundaryHalo {
                topology: key.topology,
                integral: key.integral.clone(),
                product: key.product.clone(),
                witness: value.witness.clone(),
                coefficient: value.coefficient.clone(),
            };
            closures.push(FourLoopBoundaryHaloClosure {
                raw_id,
                blocker,
                dotted_component: reduction.dotted_component(),
                compact_reference_position: reduction.compact_reference_position(),
                mass_normalized_output: reduction.mass_normalized().terms().clone(),
            });
            for (product, ratio) in reduction.mass_normalized().terms() {
                let coefficient = self.checked_mul(&value.coefficient, ratio)?;
                self.add_supported(
                    &mut supported,
                    FourLoopCornerColumnId::Product(product.clone()),
                    coefficient,
                )?;
            }
        }
        Ok((supported, unsupported))
    }

    fn mapper_for(
        &mut self,
        topology: FourLoopReferenceTopology,
        corner: &Integral,
    ) -> Result<FourLoopHaloMapper, FourLoopCornerShellError> {
        let sector_mask = physical_mask(self.classifier(topology).family(), corner);
        let key = SectorMapperKey {
            topology,
            sector_mask,
        };
        if let Some(mapper) = self.mapper_cache.get(&key) {
            return Ok(mapper.clone());
        }
        check_resource(
            "cached genuine-sector halo mappers",
            (self.mapper_cache.len() + 1) as u128,
            self.config.max_cached_sector_mappers as u128,
        )?;
        let mapper = {
            let classifier = self.classifier(topology);
            let class = classifier.classify_integral(corner)?;
            let witness = class.into_witness();
            FourLoopHaloMapper::from_witness(classifier, &witness, self.config.halo)?
        };
        self.mapper_cache.insert(key, mapper.clone());
        Ok(mapper)
    }

    fn normalize_raw_equation(
        &mut self,
        source_topology: FourLoopReferenceTopology,
        source_mapper: &FourLoopHaloMapper,
        equation: &crate::LinearCombination,
    ) -> Result<NormalizationOutcome, FourLoopCornerShellError> {
        let mut output = NormalizationOutcome::default();
        for (integral, raw_coefficient) in equation.terms() {
            let mapped = source_mapper.map_raw_halo_integral(integral)?;
            self.charge_normalization(mapped.len())?;
            for (mapped_integral, map_coefficient) in mapped.terms() {
                let factor = self.checked_mul(raw_coefficient, map_coefficient)?;
                let branch =
                    self.normalize_reference_integral(source_topology, mapped_integral, 0)?;
                self.merge_outcome(&mut output, branch, &factor)?;
            }
        }
        Ok(output)
    }

    fn normalize_reference_integral(
        &mut self,
        topology: FourLoopReferenceTopology,
        integral: &Integral,
        depth: usize,
    ) -> Result<NormalizationOutcome, FourLoopCornerShellError> {
        if depth > self.config.max_recursion_depth {
            return Err(FourLoopCornerShellError::NormalizationRecursionLimit {
                depth,
                limit: self.config.max_recursion_depth,
                integral: integral.clone(),
            });
        }
        let family = self.classifier(topology).family();
        if family.try_is_scaleless(integral)? {
            return Ok(NormalizationOutcome::default());
        }
        let corner = scalar_corner(family, integral);
        let class = self.boundary(topology).classify_integral(&corner)?;
        match class {
            FourLoopScalarClass::Scaleless { .. } => Ok(NormalizationOutcome::default()),
            FourLoopScalarClass::Factorized { product, witness } => {
                if integral == &corner {
                    let reduction = self
                        .boundary(topology)
                        .try_reduce_integral(&corner)?
                        .ok_or_else(|| FourLoopCornerShellError::BoundaryClosureMismatch {
                            integral: corner.clone(),
                        })?;
                    let mut outcome = NormalizationOutcome::default();
                    for (closed_product, coefficient) in reduction.terms() {
                        self.add_supported(
                            &mut outcome.supported,
                            FourLoopCornerColumnId::Product(closed_product.clone()),
                            coefficient.clone(),
                        )?;
                    }
                    Ok(outcome)
                } else {
                    let key = BoundaryHaloKey {
                        topology,
                        integral: integral.clone(),
                        product,
                    };
                    Ok(NormalizationOutcome {
                        supported: BTreeMap::new(),
                        unsupported: BTreeMap::from([(
                            key,
                            BoundaryHaloValue {
                                witness,
                                coefficient: self.coefficients.one(),
                            },
                        )]),
                    })
                }
            }
            FourLoopScalarClass::GenuineFourLoop { sector_mask, .. } => {
                let mapper = self.mapper_for(topology, &corner)?;
                let target_type = mapper.corner_type();
                let mapped = mapper.map_raw_halo_integral(integral)?;
                self.charge_normalization(mapped.len())?;
                let target_topology =
                    FourLoopReferenceTopology::try_from(target_type.reference_topology())?;
                let mut output = NormalizationOutcome::default();
                for (mapped_integral, coefficient) in mapped.terms() {
                    let mapped_mask = physical_mask(mapper.reference_family(), mapped_integral);
                    if mapped_mask == target_type.reference_mask() {
                        let powers: [i32; BASIS] =
                            mapped_integral.powers().try_into().map_err(|_| {
                                FourLoopCornerShellError::WrongIntegralArity {
                                    expected: BASIS,
                                    actual: mapped_integral.powers().len(),
                                }
                            })?;
                        self.add_supported(
                            &mut output.supported,
                            FourLoopCornerColumnId::Genuine {
                                corner_type: target_type,
                                powers,
                            },
                            coefficient.clone(),
                        )?;
                    } else {
                        if mapped_mask.count_ones() >= sector_mask.count_ones() {
                            return Err(FourLoopCornerShellError::NonDecreasingNormalization {
                                source_sector_mask: sector_mask,
                                mapped_sector_mask: mapped_mask,
                                integral: mapped_integral.clone(),
                            });
                        }
                        let branch = self.normalize_reference_integral(
                            target_topology,
                            mapped_integral,
                            depth + 1,
                        )?;
                        self.merge_outcome(&mut output, branch, coefficient)?;
                    }
                }
                Ok(output)
            }
        }
    }

    fn merge_outcome(
        &self,
        target: &mut NormalizationOutcome,
        source: NormalizationOutcome,
        factor: &Coefficient,
    ) -> Result<(), FourLoopCornerShellError> {
        for (column, coefficient) in source.supported {
            let scaled = self.checked_mul(&coefficient, factor)?;
            self.add_supported(&mut target.supported, column, scaled)?;
        }
        for (key, value) in source.unsupported {
            let scaled = self.checked_mul(&value.coefficient, factor)?;
            self.add_unsupported(target, key, value.witness, scaled)?;
        }
        Ok(())
    }

    fn add_supported(
        &self,
        entries: &mut BTreeMap<FourLoopCornerColumnId, Coefficient>,
        column: FourLoopCornerColumnId,
        coefficient: Coefficient,
    ) -> Result<(), FourLoopCornerShellError> {
        add_sparse_checked(self, entries, column, coefficient)
    }

    fn add_unsupported(
        &self,
        outcome: &mut NormalizationOutcome,
        key: BoundaryHaloKey,
        witness: FourLoopFactorizationWitness,
        coefficient: Coefficient,
    ) -> Result<(), FourLoopCornerShellError> {
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = outcome.unsupported.get_mut(&key) {
            if current.witness != witness {
                return Err(FourLoopCornerShellError::BoundaryWitnessMismatch {
                    integral: key.integral,
                });
            }
            let sum = self.checked_add(&current.coefficient, &coefficient)?;
            if sum.is_zero() {
                outcome.unsupported.remove(&key);
            } else {
                current.coefficient = sum;
            }
        } else {
            outcome.unsupported.insert(
                key,
                BoundaryHaloValue {
                    witness,
                    coefficient,
                },
            );
        }
        Ok(())
    }

    fn mass_normalize_supported(
        &self,
        raw_id: FourLoopCornerRawRowId,
        seed_weight: i64,
        entries: &mut BTreeMap<FourLoopCornerColumnId, Coefficient>,
    ) -> Result<(), FourLoopCornerShellError> {
        for (column, coefficient) in entries.iter_mut() {
            *coefficient = self.mass_normalize_coefficient(
                raw_id,
                column.stable_key(),
                coefficient,
                seed_weight - column.mass_weight(),
            )?;
        }
        entries.retain(|_, coefficient| !coefficient.is_zero());
        Ok(())
    }

    fn mass_normalize_unsupported(
        &self,
        raw_id: FourLoopCornerRawRowId,
        seed_weight: i64,
        entries: &mut BTreeMap<BoundaryHaloKey, BoundaryHaloValue>,
    ) -> Result<(), FourLoopCornerShellError> {
        for (key, value) in entries.iter_mut() {
            let weight = key
                .integral
                .powers()
                .iter()
                .map(|&power| i64::from(power))
                .sum::<i64>();
            let stable_key = unsupported_stable_key(key);
            value.coefficient = self.mass_normalize_coefficient(
                raw_id,
                stable_key,
                &value.coefficient,
                seed_weight - weight,
            )?;
        }
        entries.retain(|_, value| !value.coefficient.is_zero());
        Ok(())
    }

    fn mass_normalize_coefficient(
        &self,
        raw_id: FourLoopCornerRawRowId,
        column_key: String,
        coefficient: &Coefficient,
        exponent: i64,
    ) -> Result<Coefficient, FourLoopCornerShellError> {
        let mut normalized = coefficient.clone();
        if exponent >= 0 {
            for _ in 0..u64::try_from(exponent).unwrap() {
                normalized = self.checked_mul(&normalized, &self.mass)?;
            }
        } else {
            for _ in 0..exponent.unsigned_abs() {
                normalized = self.checked_div(&normalized, &self.mass)?;
            }
        }
        let degrees = coefficient_variable_degrees(&normalized);
        let (numerator, denominator) = degrees
            .get(self.mass_position)
            .copied()
            .ok_or(FourLoopCornerShellError::MissingMassParameter)?;
        if numerator != 0 || denominator != 0 {
            return Err(FourLoopCornerShellError::ResidualMassDependence {
                raw_id,
                column_key,
                numerator_degree: numerator,
                denominator_degree: denominator,
            });
        }
        Ok(normalized)
    }

    fn canonicalize_row(
        &self,
        entries: &mut BTreeMap<FourLoopCornerColumnId, Coefficient>,
    ) -> Result<Coefficient, FourLoopCornerShellError> {
        let Some(scale) = entries.last_key_value().map(|(_, value)| value.clone()) else {
            return Ok(self.coefficients.one());
        };
        for coefficient in entries.values_mut() {
            *coefficient = self.checked_div(coefficient, &scale)?;
        }
        Ok(scale)
    }

    fn charge_normalization(&mut self, amount: usize) -> Result<(), FourLoopCornerShellError> {
        self.normalization_contributions = self
            .normalization_contributions
            .checked_add(amount)
            .ok_or(FourLoopCornerShellError::ResourceLimit {
                resource: "normalization contributions",
                requested: u128::MAX,
                limit: self.config.max_normalization_contributions as u128,
            })?;
        check_resource(
            "normalization contributions",
            self.normalization_contributions as u128,
            self.config.max_normalization_contributions as u128,
        )
    }

    fn check_degree(&self, requested: u128) -> Result<(), FourLoopCornerShellError> {
        if !symbolica_coefficient_degree_is_representable(requested) {
            return Err(FourLoopCornerShellError::ResourceLimit {
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
    ) -> Result<Coefficient, FourLoopCornerShellError> {
        self.check_degree(coefficient_product_degree_bound(left, right))?;
        Ok(left * right)
    }

    fn checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopCornerShellError> {
        self.check_degree(coefficient_sum_degree_bound(left, right))?;
        Ok(left + right)
    }

    fn checked_div(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopCornerShellError> {
        if right.is_zero() {
            return Err(FourLoopCornerShellError::ZeroPivotCoefficient);
        }
        self.check_degree(coefficient_quotient_degree_bound(left, right))?;
        Ok(left / right)
    }
}

fn preflight_config(config: FourLoopCornerShellConfig) -> Result<(), FourLoopCornerShellError> {
    if config.max_coefficient_degree as u128 > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(FourLoopCornerShellError::ResourceLimit {
            resource: "configured coefficient exponent degree",
            requested: config.max_coefficient_degree as u128,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }
    if config.boundary_halo.max_coefficient_degree > config.max_coefficient_degree {
        return Err(FourLoopCornerShellError::ResourceLimit {
            resource: "factorized halo coefficient exponent degree",
            requested: config.boundary_halo.max_coefficient_degree as u128,
            limit: config.max_coefficient_degree as u128,
        });
    }
    for (resource, requested, limit) in [
        (
            "native raw rows",
            FOUR_LOOP_CORNER_SHELL_RAW_ROWS,
            config.max_raw_rows,
        ),
        (
            "structural global columns",
            FOUR_LOOP_CORNER_SHELL_GLOBAL_COLUMN_BOUND,
            config.max_global_columns,
        ),
        (
            "raw term incidences",
            FOUR_LOOP_CORNER_SHELL_RAW_TERM_INCIDENCE_BOUND,
            config.max_raw_term_incidences,
        ),
        (
            "normalization contributions",
            FOUR_LOOP_CORNER_SHELL_NORMALIZATION_CONTRIBUTION_BOUND,
            config.max_normalization_contributions,
        ),
        (
            "collected normalized nonzeros",
            FOUR_LOOP_CORNER_SHELL_COLLECTED_NONZERO_BOUND,
            config.max_collected_nonzeros,
        ),
        (
            "elimination coefficient updates",
            FOUR_LOOP_CORNER_SHELL_ELIMINATION_UPDATE_BOUND,
            config.max_elimination_updates,
        ),
        (
            "source-row provenance weights",
            FOUR_LOOP_CORNER_SHELL_SOURCE_WEIGHT_BOUND,
            config.max_source_row_weights,
        ),
    ] {
        check_resource(resource, requested as u128, limit as u128)?;
    }
    check_resource(
        "normalization recursion depth",
        BASIS as u128,
        config.max_recursion_depth as u128,
    )?;
    check_resource(
        "frozen genuine-sector halo mappers",
        FourLoopGenuineCornerType::ALL.len() as u128,
        config.max_cached_sector_mappers as u128,
    )?;
    Ok(())
}

fn reference_family_in_context(
    topology: FourLoopReferenceTopology,
    coefficients: CoefficientContext,
    mass: &Coefficient,
) -> Result<VacuumFamily, FamilyError> {
    let topology = topology.as_topology();
    let propagators = topology
        .routings()
        .iter()
        .map(|routing| {
            Denominator::propagator(
                routing
                    .iter()
                    .map(|&component| ExactRational::from(i64::from(component)))
                    .collect(),
                mass.clone(),
            )
        })
        .collect();
    VacuumFamily::new_with_standard_auxiliaries(
        format!("{}_corner_shell_reference", topology.name()),
        LOOPS,
        coefficients,
        "d",
        propagators,
        Vec::new(),
    )
}

/// Frozen scalar seed for one genuine reference type.
pub fn four_loop_corner_seed(corner_type: FourLoopGenuineCornerType) -> Integral {
    corner_seed(corner_type)
}

fn corner_seed(corner_type: FourLoopGenuineCornerType) -> Integral {
    Integral::from(std::array::from_fn::<_, BASIS, _>(|position| {
        i32::from(corner_type.reference_mask() & (1_u16 << position) != 0)
    }))
}

fn scalar_corner(family: &VacuumFamily, integral: &Integral) -> Integral {
    Integral::new(
        integral
            .powers()
            .iter()
            .zip(family.denominators())
            .map(|(&power, denominator)| i32::from(power > 0 && denominator.is_propagator()))
            .collect::<Vec<_>>(),
    )
}

fn physical_mask(family: &VacuumFamily, integral: &Integral) -> u16 {
    integral
        .powers()
        .iter()
        .zip(family.denominators())
        .enumerate()
        .filter_map(|(position, (&power, denominator))| {
            (power > 0 && denominator.is_propagator()).then_some(position)
        })
        .fold(0_u16, |mask, position| mask | (1_u16 << position))
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

fn add_sparse_checked<K: Ord>(
    builder: &CornerShellBuilder,
    entries: &mut BTreeMap<K, Coefficient>,
    key: K,
    coefficient: Coefficient,
) -> Result<(), FourLoopCornerShellError> {
    if coefficient.is_zero() {
        return Ok(());
    }
    if let Some(current) = entries.get_mut(&key) {
        let sum = builder.checked_add(current, &coefficient)?;
        if sum.is_zero() {
            entries.remove(&key);
        } else {
            *current = sum;
        }
    } else {
        entries.insert(key, coefficient);
    }
    Ok(())
}

fn unsupported_stable_key(key: &BoundaryHaloKey) -> String {
    let powers = key
        .integral
        .powers()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{}:{}:{}:[{}]",
        UnsupportedBoundaryHalo::SCHEMA,
        key.topology.stable_key(),
        FourLoopHaloColumnKey::Factorized(key.product.clone()).stable_key(),
        powers
    )
}

fn build_blocker_census(
    rows: &[FourLoopCornerBlockedRow],
) -> BTreeMap<FourLoopBoundaryHaloCensusKey, usize> {
    let mut census = BTreeMap::new();
    for blocker in rows
        .iter()
        .flat_map(|row| row.unsupported_boundary_halo.iter())
    {
        let key = FourLoopBoundaryHaloCensusKey::from_blocker(blocker);
        *census.entry(key).or_insert(0) += 1;
    }
    census
}

fn build_preclosure_blocked_rows(rows: &[CollectedCornerRow]) -> Vec<FourLoopCornerBlockedRow> {
    let mut blocked_rows = Vec::new();
    for row in rows {
        if row.outcome.unsupported.is_empty() {
            continue;
        }
        let unsupported_boundary_halo = row
            .outcome
            .unsupported
            .iter()
            .map(|(key, value)| UnsupportedBoundaryHalo {
                topology: key.topology,
                integral: key.integral.clone(),
                product: key.product.clone(),
                witness: value.witness.clone(),
                coefficient: value.coefficient.clone(),
            })
            .collect();
        blocked_rows.push(FourLoopCornerBlockedRow {
            raw_id: row.raw_id,
            seed_mass_weight: row.seed_mass_weight,
            supported_entries: row.outcome.supported.clone(),
            unsupported_boundary_halo,
        });
    }
    blocked_rows
}

fn blocker_dot_degree(key: &BoundaryHaloKey, witness: &FourLoopFactorizationWitness) -> u32 {
    key.integral
        .powers()
        .iter()
        .enumerate()
        .filter(|(position, _)| witness.sector_mask() & (1_u16 << position) != 0)
        .map(|(_, &power)| u32::try_from(power.saturating_sub(1).max(0)).unwrap())
        .sum()
}

fn blocker_numerator_degree(key: &BoundaryHaloKey) -> u32 {
    key.integral.numerator_degree()
}

#[derive(Clone)]
struct EliminationWorkRow {
    entries: BTreeMap<FourLoopCornerColumnId, Coefficient>,
    source_weights: BTreeMap<FourLoopCornerRawRowId, Coefficient>,
}

fn eliminate_rows(
    builder: &CornerShellBuilder,
    rows: &[FourLoopCornerNormalizedRow],
) -> Result<
    (
        Vec<FourLoopCornerPivotRule>,
        Vec<FourLoopCornerColumnId>,
        usize,
    ),
    FourLoopCornerShellError,
> {
    let all_columns = rows
        .iter()
        .flat_map(|row| row.entries.keys().cloned())
        .collect::<BTreeSet<_>>();
    let mut pivots = BTreeMap::<FourLoopCornerColumnId, EliminationWorkRow>::new();
    let mut updates = 0_usize;

    for row in rows {
        let mut work = EliminationWorkRow {
            entries: row.entries.clone(),
            source_weights: BTreeMap::from([(row.raw_id, builder.coefficients.one())]),
        };
        loop {
            let Some(hardest) = work
                .entries
                .last_key_value()
                .map(|(column, _)| column.clone())
            else {
                break;
            };
            let Some(pivot_row) = pivots.get(&hardest) else {
                break;
            };
            let factor = work
                .entries
                .get(&hardest)
                .expect("the selected hardest coefficient exists")
                .clone();
            add_scaled_work_row(builder, &mut work, pivot_row, &(-factor), &mut updates)?;
        }
        if work.entries.is_empty() {
            continue;
        }
        let pivot = work
            .entries
            .last_key_value()
            .map(|(column, _)| column.clone())
            .expect("a nonzero work row has a hardest column");
        let pivot_coefficient = work
            .entries
            .get(&pivot)
            .expect("the pivot coefficient exists")
            .clone();
        divide_work_row(builder, &mut work, &pivot_coefficient, &mut updates)?;
        if work.source_weights.len() > builder.config.max_raw_rows {
            return Err(FourLoopCornerShellError::ResourceLimit {
                resource: "source weights in one pivot",
                requested: work.source_weights.len() as u128,
                limit: builder.config.max_raw_rows as u128,
            });
        }
        pivots.insert(pivot, work);
    }

    let stored_weights = pivots
        .values()
        .map(|row| row.source_weights.len())
        .sum::<usize>();
    check_resource(
        "source-row provenance weights",
        stored_weights as u128,
        builder.config.max_source_row_weights as u128,
    )?;
    check_resource(
        "elimination coefficient updates",
        updates as u128,
        builder.config.max_elimination_updates as u128,
    )?;

    let pivot_columns = pivots.keys().cloned().collect::<BTreeSet<_>>();
    let free_columns = all_columns
        .difference(&pivot_columns)
        .cloned()
        .collect::<Vec<_>>();
    let rules = pivots
        .into_iter()
        .rev()
        .map(|(pivot, row)| {
            let mut equation = row.entries;
            let removed = equation
                .remove(&pivot)
                .expect("a stored pivot row retains its pivot");
            debug_assert_eq!(removed, builder.coefficients.one());
            let rhs = equation
                .into_iter()
                .map(|(column, coefficient)| (column, -coefficient))
                .collect();
            FourLoopCornerPivotRule {
                pivot,
                rhs,
                source_row_weights: row.source_weights,
            }
        })
        .collect();
    Ok((rules, free_columns, updates))
}

fn add_scaled_work_row(
    builder: &CornerShellBuilder,
    target: &mut EliminationWorkRow,
    source: &EliminationWorkRow,
    factor: &Coefficient,
    updates: &mut usize,
) -> Result<(), FourLoopCornerShellError> {
    for (column, coefficient) in &source.entries {
        charge_elimination_update(builder, updates)?;
        let scaled = builder.checked_mul(coefficient, factor)?;
        add_sparse_checked(builder, &mut target.entries, column.clone(), scaled)?;
    }
    for (row_id, coefficient) in &source.source_weights {
        charge_elimination_update(builder, updates)?;
        let scaled = builder.checked_mul(coefficient, factor)?;
        add_sparse_checked(builder, &mut target.source_weights, *row_id, scaled)?;
    }
    Ok(())
}

fn divide_work_row(
    builder: &CornerShellBuilder,
    row: &mut EliminationWorkRow,
    divisor: &Coefficient,
    updates: &mut usize,
) -> Result<(), FourLoopCornerShellError> {
    for coefficient in row.entries.values_mut() {
        charge_elimination_update(builder, updates)?;
        *coefficient = builder.checked_div(coefficient, divisor)?;
    }
    for coefficient in row.source_weights.values_mut() {
        charge_elimination_update(builder, updates)?;
        *coefficient = builder.checked_div(coefficient, divisor)?;
    }
    Ok(())
}

fn charge_elimination_update(
    builder: &CornerShellBuilder,
    updates: &mut usize,
) -> Result<(), FourLoopCornerShellError> {
    *updates = updates
        .checked_add(1)
        .ok_or(FourLoopCornerShellError::ResourceLimit {
            resource: "elimination coefficient updates",
            requested: u128::MAX,
            limit: builder.config.max_elimination_updates as u128,
        })?;
    check_resource(
        "elimination coefficient updates",
        *updates as u128,
        builder.config.max_elimination_updates as u128,
    )
}

fn replay_certificate(
    certificate: &FourLoopCornerShellCertificate,
) -> Result<(), FourLoopCornerShellError> {
    // Deterministically regenerate the native IBPs and every affine/boundary
    // normalization before trusting any stored normalized row. The nested
    // build deliberately suppresses its own replay to avoid recursion.
    let rebuilt = CornerShellBuilder::new(certificate.config)?.build(false)?;
    if rebuilt.raw_row_ids != certificate.raw_row_ids
        || rebuilt.normalized_rows != certificate.normalized_rows
        || rebuilt.blocked_rows != certificate.blocked_rows
        || rebuilt.preclosure_blocked_rows != certificate.preclosure_blocked_rows
        || rebuilt.boundary_halo_closures != certificate.boundary_halo_closures
        || rebuilt.boundary_halo_stats != certificate.boundary_halo_stats
    {
        return Err(FourLoopCornerShellError::RawInputRebuildMismatch);
    }

    let coefficients = CoefficientContext::new(["d", "m2"]);
    let replay_builder = ReplayArithmetic {
        config: certificate.config,
    };
    let rules = certificate
        .pivots
        .iter()
        .map(|rule| (rule.pivot.clone(), rule))
        .collect::<BTreeMap<_, _>>();

    let h_family = reference_family_in_context(
        FourLoopReferenceTopology::H,
        coefficients.clone(),
        &coefficients
            .parameter("m2")
            .ok_or(FourLoopCornerShellError::MissingMassParameter)?,
    )?;
    let h_boundary = FourLoopBoundaryReducer::new(
        FourLoopTopology::H,
        h_family,
        certificate.config.genuine.boundary,
    )?;
    let halo = FourLoopBoundaryHaloReducer::new(h_boundary, certificate.config.boundary_halo)?;
    halo.preflight_formula_table()?;
    let mut replay_plans = Vec::<(
        MasterProduct<MassiveVacuumMaster>,
        FourLoopFactorizationWitness,
        FourLoopBoundaryHaloPlan,
    )>::new();
    for closure in &certificate.boundary_halo_closures {
        let plan = if let Some((_, _, plan)) = replay_plans.iter().find(|(product, witness, _)| {
            product == closure.blocker.product() && witness == closure.blocker.witness()
        }) {
            plan
        } else {
            let plan = halo.prepare_plan(closure.blocker.product(), closure.blocker.witness())?;
            replay_plans.push((
                closure.blocker.product().clone(),
                closure.blocker.witness().clone(),
                plan,
            ));
            &replay_plans.last().expect("a plan was inserted").2
        };
        let reduction = halo.reduce_with_plan(closure.blocker.integral(), plan)?;
        if reduction.dotted_component() != closure.dotted_component
            || reduction.compact_reference_position() != closure.compact_reference_position
            || reduction.mass_normalized().terms() != &closure.mass_normalized_output
        {
            return Err(
                FourLoopCornerShellError::BoundaryHaloClosureReplayMismatch {
                    raw_id: closure.raw_id,
                    integral: closure.blocker.integral().clone(),
                },
            );
        }
    }

    // Reconstruct every closed, mass-normalized input row from the retained
    // preclosure supported terms plus the replayed blocker substitutions.
    // This catches a dropped factor, sign, or row-scale error between closure
    // and elimination; checking the stored post-closure rows alone cannot.
    let normalized_by_id = certificate
        .normalized_rows
        .iter()
        .map(|row| (row.raw_id, row))
        .collect::<BTreeMap<_, _>>();
    for preclosure in &certificate.preclosure_blocked_rows {
        let mut reconstructed = preclosure.supported_entries.clone();
        for closure in certificate
            .boundary_halo_closures
            .iter()
            .filter(|closure| closure.raw_id == preclosure.raw_id)
        {
            for (product, ratio) in &closure.mass_normalized_output {
                let coefficient =
                    replay_builder.checked_mul(closure.blocker.coefficient(), ratio)?;
                replay_builder.add_sparse(
                    &mut reconstructed,
                    FourLoopCornerColumnId::Product(product.clone()),
                    coefficient,
                )?;
            }
        }
        let normalized = normalized_by_id.get(&preclosure.raw_id).ok_or(
            FourLoopCornerShellError::UnknownSourceRow {
                raw_id: preclosure.raw_id,
            },
        )?;
        for coefficient in reconstructed.values_mut() {
            *coefficient = replay_builder.checked_div(coefficient, normalized.row_scale())?;
        }
        reconstructed.retain(|_, coefficient| !coefficient.is_zero());
        if reconstructed != *normalized.entries() {
            return Err(FourLoopCornerShellError::ClosedRowReplayMismatch {
                raw_id: preclosure.raw_id,
            });
        }
    }

    for row in &certificate.normalized_rows {
        let remainder = reduce_by_rules(&replay_builder, row.entries.clone(), &rules)?;
        if !remainder.is_empty() {
            return Err(FourLoopCornerShellError::NormalizedRowReplayMismatch {
                raw_id: row.raw_id,
            });
        }
    }

    let rows = certificate
        .normalized_rows
        .iter()
        .map(|row| (row.raw_id, row))
        .collect::<BTreeMap<_, _>>();
    for rule in &certificate.pivots {
        let mut combination = BTreeMap::new();
        for (raw_id, weight) in &rule.source_row_weights {
            let row = rows
                .get(raw_id)
                .ok_or(FourLoopCornerShellError::UnknownSourceRow { raw_id: *raw_id })?;
            for (column, coefficient) in &row.entries {
                let scaled = replay_builder.checked_mul(coefficient, weight)?;
                replay_builder.add_sparse(&mut combination, column.clone(), scaled)?;
            }
        }
        let mut expected = BTreeMap::from([(rule.pivot.clone(), coefficients.one())]);
        for (column, coefficient) in &rule.rhs {
            replay_builder.add_sparse(&mut expected, column.clone(), -coefficient.clone())?;
        }
        if combination != expected {
            return Err(FourLoopCornerShellError::PivotProvenanceReplayMismatch {
                pivot: rule.pivot.clone(),
            });
        }
        if rule.rhs.keys().any(|column| column >= &rule.pivot) {
            return Err(FourLoopCornerShellError::NonTriangularPivot {
                pivot: rule.pivot.clone(),
            });
        }
    }
    Ok(())
}

struct ReplayArithmetic {
    config: FourLoopCornerShellConfig,
}

impl ReplayArithmetic {
    fn check_degree(&self, requested: u128) -> Result<(), FourLoopCornerShellError> {
        if !symbolica_coefficient_degree_is_representable(requested) {
            return Err(FourLoopCornerShellError::ResourceLimit {
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
    ) -> Result<Coefficient, FourLoopCornerShellError> {
        self.check_degree(coefficient_product_degree_bound(left, right))?;
        Ok(left * right)
    }

    fn checked_add(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopCornerShellError> {
        self.check_degree(coefficient_sum_degree_bound(left, right))?;
        Ok(left + right)
    }

    fn checked_div(
        &self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopCornerShellError> {
        if right.is_zero() {
            return Err(FourLoopCornerShellError::ZeroPivotCoefficient);
        }
        self.check_degree(coefficient_quotient_degree_bound(left, right))?;
        Ok(left / right)
    }

    fn add_sparse<K: Ord>(
        &self,
        entries: &mut BTreeMap<K, Coefficient>,
        key: K,
        coefficient: Coefficient,
    ) -> Result<(), FourLoopCornerShellError> {
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = entries.get_mut(&key) {
            let sum = self.checked_add(current, &coefficient)?;
            if sum.is_zero() {
                entries.remove(&key);
            } else {
                *current = sum;
            }
        } else {
            entries.insert(key, coefficient);
        }
        Ok(())
    }
}

fn reduce_by_rules(
    arithmetic: &ReplayArithmetic,
    mut entries: BTreeMap<FourLoopCornerColumnId, Coefficient>,
    rules: &BTreeMap<FourLoopCornerColumnId, &FourLoopCornerPivotRule>,
) -> Result<BTreeMap<FourLoopCornerColumnId, Coefficient>, FourLoopCornerShellError> {
    loop {
        let Some((pivot, rule)) = entries
            .keys()
            .rev()
            .find_map(|column| rules.get(column).map(|rule| (column.clone(), *rule)))
        else {
            break;
        };
        let factor = entries
            .remove(&pivot)
            .expect("the selected reducible pivot is present");
        for (column, coefficient) in &rule.rhs {
            let scaled = arithmetic.checked_mul(coefficient, &factor)?;
            arithmetic.add_sparse(&mut entries, column.clone(), scaled)?;
        }
    }
    Ok(entries)
}

fn check_resource(
    resource: &'static str,
    requested: u128,
    limit: u128,
) -> Result<(), FourLoopCornerShellError> {
    if requested > limit {
        Err(FourLoopCornerShellError::ResourceLimit {
            resource,
            requested,
            limit,
        })
    } else {
        Ok(())
    }
}

#[derive(Debug)]
pub enum FourLoopCornerShellError {
    Family(FamilyError),
    Boundary(FourLoopBoundaryError),
    Genuine(FourLoopGenuineError),
    Halo(FourLoopHaloError),
    Ibp(IbpGenerationError),
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    NonReferenceTopology {
        topology: FourLoopTopology,
    },
    MissingMassParameter,
    WrongIntegralArity {
        expected: usize,
        actual: usize,
    },
    RawRowCountMismatch {
        expected: usize,
        actual: usize,
    },
    RawRowLabelOutOfRange,
    RawRowProvenanceMismatch {
        raw_id: FourLoopCornerRawRowId,
    },
    RawInputRebuildMismatch,
    DuplicateRawRowId,
    BoundaryClosureMismatch {
        integral: Integral,
    },
    BoundaryWitnessMismatch {
        integral: Integral,
    },
    MissingBoundaryHaloPlan {
        integral: Integral,
    },
    NormalizationRecursionLimit {
        depth: usize,
        limit: usize,
        integral: Integral,
    },
    BoundaryHalo(FourLoopBoundaryHaloError),
    NonDecreasingNormalization {
        source_sector_mask: u16,
        mapped_sector_mask: u16,
        integral: Integral,
    },
    ResidualMassDependence {
        raw_id: FourLoopCornerRawRowId,
        column_key: String,
        numerator_degree: u128,
        denominator_degree: u128,
    },
    ZeroPivotCoefficient,
    NormalizedRowReplayMismatch {
        raw_id: FourLoopCornerRawRowId,
    },
    BoundaryHaloClosureReplayMismatch {
        raw_id: FourLoopCornerRawRowId,
        integral: Integral,
    },
    ClosedRowReplayMismatch {
        raw_id: FourLoopCornerRawRowId,
    },
    UnknownSourceRow {
        raw_id: FourLoopCornerRawRowId,
    },
    PivotProvenanceReplayMismatch {
        pivot: FourLoopCornerColumnId,
    },
    NonTriangularPivot {
        pivot: FourLoopCornerColumnId,
    },
}

impl From<FamilyError> for FourLoopCornerShellError {
    fn from(error: FamilyError) -> Self {
        Self::Family(error)
    }
}

impl From<FourLoopBoundaryError> for FourLoopCornerShellError {
    fn from(error: FourLoopBoundaryError) -> Self {
        Self::Boundary(error)
    }
}

impl From<FourLoopGenuineError> for FourLoopCornerShellError {
    fn from(error: FourLoopGenuineError) -> Self {
        Self::Genuine(error)
    }
}

impl From<FourLoopHaloError> for FourLoopCornerShellError {
    fn from(error: FourLoopHaloError) -> Self {
        Self::Halo(error)
    }
}

impl From<FourLoopBoundaryHaloError> for FourLoopCornerShellError {
    fn from(error: FourLoopBoundaryHaloError) -> Self {
        Self::BoundaryHalo(error)
    }
}

impl From<IbpGenerationError> for FourLoopCornerShellError {
    fn from(error: IbpGenerationError) -> Self {
        Self::Ibp(error)
    }
}

impl fmt::Display for FourLoopCornerShellError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Family(error) => write!(formatter, "four-loop corner-shell family: {error}"),
            Self::Boundary(error) => write!(formatter, "four-loop corner-shell boundary: {error}"),
            Self::Genuine(error) => write!(formatter, "four-loop corner-shell atlas: {error}"),
            Self::Halo(error) => write!(formatter, "four-loop corner-shell halo: {error}"),
            Self::BoundaryHalo(error) => {
                write!(formatter, "four-loop corner-shell factorized halo: {error}")
            }
            Self::Ibp(error) => write!(formatter, "four-loop corner-shell IBP: {error}"),
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "four-loop corner-shell {resource} requires {requested}, exceeding limit {limit}"
            ),
            Self::NonReferenceTopology { topology } => write!(
                formatter,
                "{topology:?} is not one of the frozen H/X corner-shell reference families"
            ),
            Self::MissingMassParameter => {
                formatter.write_str("four-loop corner shell requires coefficient parameter m2")
            }
            Self::WrongIntegralArity { expected, actual } => write!(
                formatter,
                "four-loop corner-shell integral has {actual} powers; expected {expected}"
            ),
            Self::RawRowCountMismatch { expected, actual } => write!(
                formatter,
                "four-loop corner shell generated {actual} raw rows; expected {expected}"
            ),
            Self::RawRowLabelOutOfRange => {
                formatter.write_str("four-loop raw-row derivative label does not fit u8")
            }
            Self::RawRowProvenanceMismatch { raw_id } => write!(
                formatter,
                "generated raw row does not replay provenance {}",
                raw_id.stable_key()
            ),
            Self::RawInputRebuildMismatch => formatter.write_str(
                "regenerated native IBPs and boundary normalizations do not match the stored corner-shell inputs",
            ),
            Self::DuplicateRawRowId => {
                formatter.write_str("four-loop corner shell contains duplicate raw-row IDs")
            }
            Self::BoundaryClosureMismatch { integral } => write!(
                formatter,
                "scalar boundary classified {integral} as factorized but did not close it"
            ),
            Self::BoundaryWitnessMismatch { integral } => write!(
                formatter,
                "two incompatible factorization witnesses were collected for {integral}"
            ),
            Self::MissingBoundaryHaloPlan { integral } => write!(
                formatter,
                "no authenticated factorized boundary-halo plan was retained for {integral}"
            ),
            Self::NormalizationRecursionLimit {
                depth,
                limit,
                integral,
            } => write!(
                formatter,
                "normalizing {integral} reached recursion depth {depth}, exceeding {limit}"
            ),
            Self::NonDecreasingNormalization {
                source_sector_mask,
                mapped_sector_mask,
                integral,
            } => write!(
                formatter,
                "normalization branch {integral} did not lower the active sector ({source_sector_mask:#x} -> {mapped_sector_mask:#x})"
            ),
            Self::ResidualMassDependence {
                raw_id,
                column_key,
                numerator_degree,
                denominator_degree,
            } => write!(
                formatter,
                "mass-normalized row {} column {column_key} retains m2 degrees ({numerator_degree},{denominator_degree})",
                raw_id.stable_key()
            ),
            Self::ZeroPivotCoefficient => {
                formatter.write_str("attempted to divide a sparse row by zero")
            }
            Self::NormalizedRowReplayMismatch { raw_id } => write!(
                formatter,
                "normalized input row {} does not replay through the pivot rules",
                raw_id.stable_key()
            ),
            Self::BoundaryHaloClosureReplayMismatch { raw_id, integral } => write!(
                formatter,
                "factorized halo substitution for {integral} in {} did not replay",
                raw_id.stable_key()
            ),
            Self::ClosedRowReplayMismatch { raw_id } => write!(
                formatter,
                "closed preclosure row {} does not reconstruct its normalized input row",
                raw_id.stable_key()
            ),
            Self::UnknownSourceRow { raw_id } => write!(
                formatter,
                "pivot provenance references unknown normalized row {}",
                raw_id.stable_key()
            ),
            Self::PivotProvenanceReplayMismatch { pivot } => write!(
                formatter,
                "source-row weights do not replay pivot {}",
                pivot.stable_key()
            ),
            Self::NonTriangularPivot { pivot } => write!(
                formatter,
                "pivot {} has a right-hand-side column that is not strictly easier",
                pivot.stable_key()
            ),
        }
    }
}

impl Error for FourLoopCornerShellError {}
