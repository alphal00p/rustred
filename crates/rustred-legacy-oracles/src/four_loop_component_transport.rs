//! Exact component transport for every factorized boundary in the frozen
//! four-loop next-shell inventory.
//!
//! A factorization witness only identifies the scalar corner.  This module
//! additionally transports the retained dots and the optional single
//! numerator into complete lower-loop component bases.  Cross-component
//! scalar products are certified to vanish by two separately owned odd-rank
//! vacuum projectors.  The output remains a transport certificate: it does
//! not perform lower-loop closure, mass normalization, rank computation, or
//! row elimination.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;

use crate::{
    FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM, FourLoopBoundaryError, FourLoopNextBoundaryKey,
    FourLoopNextInventory, FourLoopNextInventoryError, FourLoopNextInventoryStatus,
    MassiveVacuumMaster, THREE_LOOP_TETRAHEDRON_ROUTINGS,
};
use rustred::legacy_oracle_support::exact_matrix::{
    invert_matrix, matrix_determinant, matrix_multiply,
};
use rustred::{
    Coefficient, ExactRational, IndexedVector, LoopVector, LorentzIndex, TensorError,
    TensorMonomial, VacuumFamily, VacuumTensorProjector,
};

const LOOPS: usize = 4;
const BASIS: usize = 10;
// Fixed dimensions bound one 10x10 basis inverse/determinant, 4x4 routing
// algebra, signed-line images, and eleven affine probes.  This deliberately
// gross power-of-two reservation is separate from Symbolica coefficient ops.
const RATIONAL_OPERATION_RESERVATION_PER_PLAN: usize = 65_536;

/// Conservative batch bounds for the exact frozen inventory.
pub const FOUR_LOOP_COMPONENT_TRANSPORT_PLANS: usize = 1_066;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES: usize = 4_230;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENTS: usize = 4_264;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENT_MAP_ENTRIES: usize = 10_660;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_SIGNED_LINE_REPLAYS: usize = 7_462;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_SLOTS: usize = 7_462;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_LOOP_MAP_ENTRIES: usize = 17_056;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_TRANSFORMED_COEFFICIENTS: usize = 10_660;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_AFFINE_CONSTANTS: usize = 1_066;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_COEFFICIENTS: usize = 7_462;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_CROSS_COEFFICIENTS: usize = 6_396;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_PARITY_PROJECTIONS: usize = 12_792;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_SCALAR_BRANCHES: usize = 8_528;
pub const FOUR_LOOP_COMPONENT_TRANSPORT_RATIONAL_OPERATIONS: usize =
    FOUR_LOOP_COMPONENT_TRANSPORT_PLANS * RATIONAL_OPERATION_RESERVATION_PER_PLAN;

/// Gross preflight limits.  Defaults reserve the whole authenticated census,
/// so every controlled allocation and exact-algebra stage has a bound before
/// the first plan is constructed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopComponentTransportConfig {
    pub max_plans: usize,
    pub max_occurrences: usize,
    pub max_components: usize,
    pub max_component_map_entries: usize,
    pub max_signed_line_replays: usize,
    pub max_local_slots: usize,
    pub max_loop_map_entries: usize,
    pub max_transformed_coefficients: usize,
    pub max_affine_constants: usize,
    pub max_local_coefficients: usize,
    pub max_cross_coefficients: usize,
    pub max_parity_projections: usize,
    pub max_scalar_branches: usize,
    pub max_rational_operations: usize,
}

impl Default for FourLoopComponentTransportConfig {
    fn default() -> Self {
        Self {
            max_plans: FOUR_LOOP_COMPONENT_TRANSPORT_PLANS,
            max_occurrences: FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES,
            max_components: FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENTS,
            max_component_map_entries: FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENT_MAP_ENTRIES,
            max_signed_line_replays: FOUR_LOOP_COMPONENT_TRANSPORT_SIGNED_LINE_REPLAYS,
            max_local_slots: FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_SLOTS,
            max_loop_map_entries: FOUR_LOOP_COMPONENT_TRANSPORT_LOOP_MAP_ENTRIES,
            max_transformed_coefficients: FOUR_LOOP_COMPONENT_TRANSPORT_TRANSFORMED_COEFFICIENTS,
            max_affine_constants: FOUR_LOOP_COMPONENT_TRANSPORT_AFFINE_CONSTANTS,
            max_local_coefficients: FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_COEFFICIENTS,
            max_cross_coefficients: FOUR_LOOP_COMPONENT_TRANSPORT_CROSS_COEFFICIENTS,
            max_parity_projections: FOUR_LOOP_COMPONENT_TRANSPORT_PARITY_PROJECTIONS,
            max_scalar_branches: FOUR_LOOP_COMPONENT_TRANSPORT_SCALAR_BRANCHES,
            max_rational_operations: FOUR_LOOP_COMPONENT_TRANSPORT_RATIONAL_OPERATIONS,
        }
    }
}

/// Work retained by the exact transport batch.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopComponentTransportStats {
    plans: usize,
    occurrences: usize,
    components: usize,
    component_map_entries: usize,
    signed_line_replays: usize,
    local_slots: usize,
    loop_map_entries: usize,
    transformed_coefficients: usize,
    affine_constants: usize,
    local_coefficients: usize,
    cross_coefficients: usize,
    parity_projections: usize,
    scalar_branches: usize,
    rational_operations: usize,
    n0_plans: usize,
    n1_plans: usize,
}

impl FourLoopComponentTransportStats {
    pub const fn plans(self) -> usize {
        self.plans
    }
    pub const fn occurrences(self) -> usize {
        self.occurrences
    }
    pub const fn components(self) -> usize {
        self.components
    }
    pub const fn component_map_entries(self) -> usize {
        self.component_map_entries
    }
    pub const fn signed_line_replays(self) -> usize {
        self.signed_line_replays
    }
    pub const fn local_slots(self) -> usize {
        self.local_slots
    }
    pub const fn loop_map_entries(self) -> usize {
        self.loop_map_entries
    }
    pub const fn transformed_coefficients(self) -> usize {
        self.transformed_coefficients
    }
    pub const fn affine_constants(self) -> usize {
        self.affine_constants
    }
    pub const fn local_coefficients(self) -> usize {
        self.local_coefficients
    }
    pub const fn cross_coefficients(self) -> usize {
        self.cross_coefficients
    }
    pub const fn parity_projections(self) -> usize {
        self.parity_projections
    }
    pub const fn scalar_branches(self) -> usize {
        self.scalar_branches
    }
    /// A conservative charged envelope, not a hardware-operation counter.
    pub const fn rational_operations(self) -> usize {
        self.rational_operations
    }
    pub const fn n0_plans(self) -> usize {
        self.n0_plans
    }
    pub const fn n1_plans(self) -> usize {
        self.n1_plans
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopComponentTransportStatus {
    ExactComponentTransport,
}

/// Stable ownership identity for one witness component.  The witness index is
/// essential when a commutative master product contains repeated factors.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopTransportComponent {
    witness_index: usize,
    master: MassiveVacuumMaster,
    reference_loop_offset: usize,
    global_basis_slots: Vec<usize>,
    local_powers: Vec<i32>,
    line_assignments: Vec<FourLoopTransportLineAssignment>,
}

impl FourLoopTransportComponent {
    pub const fn witness_index(&self) -> usize {
        self.witness_index
    }
    pub const fn master(&self) -> MassiveVacuumMaster {
        self.master
    }
    pub const fn reference_loop_offset(&self) -> usize {
        self.reference_loop_offset
    }
    pub fn global_basis_slots(&self) -> &[usize] {
        &self.global_basis_slots
    }
    /// Powers in the complete local basis: 1 slot for T1, 3 for S2, and 6
    /// for B4/F5/M6.
    pub fn local_powers(&self) -> &[i32] {
        &self.local_powers
    }
    pub fn line_assignments(&self) -> &[FourLoopTransportLineAssignment] {
        &self.line_assignments
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopTransportLineAssignment {
    parent_position: usize,
    compact_reference_position: usize,
    local_position: usize,
    orientation_sign: i8,
}

impl FourLoopTransportLineAssignment {
    pub const fn parent_position(self) -> usize {
        self.parent_position
    }
    pub const fn compact_reference_position(self) -> usize {
        self.compact_reference_position
    }
    pub const fn local_position(self) -> usize {
        self.local_position
    }
    pub const fn orientation_sign(self) -> i8 {
        self.orientation_sign
    }
}

/// Ordered column of the complete factorized scalar-product basis.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopComponentBasisColumn {
    Local {
        component_index: usize,
        local_position: usize,
    },
    Cross {
        left_component: usize,
        left_axis: usize,
        right_component: usize,
        right_axis: usize,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopComponentAffineImage {
    source_position: usize,
    constant: Coefficient,
    coefficients: [ExactRational; BASIS],
}

impl FourLoopComponentAffineImage {
    pub const fn source_position(&self) -> usize {
        self.source_position
    }
    pub const fn constant(&self) -> &Coefficient {
        &self.constant
    }
    pub const fn coefficients(&self) -> &[ExactRational; BASIS] {
        &self.coefficients
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopComponentScalarBranchKind {
    Base,
    Constant,
    Local {
        component_index: usize,
        local_position: usize,
    },
}

/// One nonzero affine scalar target after numerator transport.  A later
/// lower-loop closure may still prove the target scaleless (for example a
/// pinched one-loop tadpole).  Local branches retain the complete lowered
/// power vector of their owning component.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopComponentScalarBranch {
    kind: FourLoopComponentScalarBranchKind,
    coefficient: Coefficient,
    lowered_component_powers: Option<Vec<i32>>,
}

impl FourLoopComponentScalarBranch {
    pub const fn kind(&self) -> FourLoopComponentScalarBranchKind {
        self.kind
    }
    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }
    pub fn lowered_component_powers(&self) -> Option<&[i32]> {
        self.lowered_component_powers.as_deref()
    }
}

/// A nonzero cross-component coefficient accompanied by two independent
/// rank-one odd projector results.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopComponentParityWitness {
    basis_position: usize,
    coefficient: ExactRational,
    left_component: usize,
    left_axis: usize,
    right_component: usize,
    right_axis: usize,
    left_rank_one_zero: bool,
    right_rank_one_zero: bool,
}

impl FourLoopComponentParityWitness {
    pub const fn basis_position(&self) -> usize {
        self.basis_position
    }
    pub fn coefficient(&self) -> ExactRational {
        self.coefficient.clone()
    }
    pub const fn left_component(&self) -> usize {
        self.left_component
    }
    pub const fn left_axis(&self) -> usize {
        self.left_axis
    }
    pub const fn right_component(&self) -> usize {
        self.right_component
    }
    pub const fn right_axis(&self) -> usize {
        self.right_axis
    }
    pub const fn left_rank_one_zero(&self) -> bool {
        self.left_rank_one_zero
    }
    pub const fn right_rank_one_zero(&self) -> bool {
        self.right_rank_one_zero
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopComponentTransportPlan {
    leaf_id: u32,
    key: FourLoopNextBoundaryKey,
    /// `k_parent = T * ell_component`.
    loop_transform: [[ExactRational; LOOPS]; LOOPS],
    components: Vec<FourLoopTransportComponent>,
    basis_columns: Vec<FourLoopComponentBasisColumn>,
    affine_image: Option<FourLoopComponentAffineImage>,
    scalar_branches: Vec<FourLoopComponentScalarBranch>,
    parity_witnesses: Vec<FourLoopComponentParityWitness>,
}

impl FourLoopComponentTransportPlan {
    pub const fn leaf_id(&self) -> u32 {
        self.leaf_id
    }
    pub const fn key(&self) -> &FourLoopNextBoundaryKey {
        &self.key
    }
    pub const fn loop_transform(&self) -> &[[ExactRational; LOOPS]; LOOPS] {
        &self.loop_transform
    }
    pub fn components(&self) -> &[FourLoopTransportComponent] {
        &self.components
    }
    pub fn basis_columns(&self) -> &[FourLoopComponentBasisColumn] {
        &self.basis_columns
    }
    pub const fn affine_image(&self) -> Option<&FourLoopComponentAffineImage> {
        self.affine_image.as_ref()
    }
    pub fn scalar_branches(&self) -> &[FourLoopComponentScalarBranch] {
        &self.scalar_branches
    }
    pub fn parity_witnesses(&self) -> &[FourLoopComponentParityWitness] {
        &self.parity_witnesses
    }

    #[doc(hidden)]
    pub fn with_loop_transform_entry_for_replay(
        &self,
        row: usize,
        column: usize,
        value: ExactRational,
    ) -> Self {
        let mut candidate = self.clone();
        if row < LOOPS && column < LOOPS {
            candidate.loop_transform[row][column] = value;
        }
        candidate
    }

    #[doc(hidden)]
    pub fn with_affine_coefficient_for_replay(
        &self,
        position: usize,
        value: ExactRational,
    ) -> Self {
        let mut candidate = self.clone();
        if let Some(image) = &mut candidate.affine_image
            && position < BASIS
        {
            image.coefficients[position] = value;
        }
        candidate
    }

    #[doc(hidden)]
    pub fn with_affine_constant_for_replay(&self, value: Coefficient) -> Self {
        let mut candidate = self.clone();
        if let Some(image) = &mut candidate.affine_image {
            image.constant = value;
        }
        candidate
    }

    #[doc(hidden)]
    pub fn with_component_local_power_for_replay(
        &self,
        component_index: usize,
        local_position: usize,
        value: i32,
    ) -> Self {
        let mut candidate = self.clone();
        if let Some(component) = candidate.components.get_mut(component_index)
            && let Some(power) = component.local_powers.get_mut(local_position)
        {
            *power = value;
        }
        candidate
    }

    #[doc(hidden)]
    pub fn with_line_local_position_for_replay(
        &self,
        component_index: usize,
        assignment_index: usize,
        local_position: usize,
    ) -> Self {
        let mut candidate = self.clone();
        if let Some(component) = candidate.components.get_mut(component_index)
            && let Some(assignment) = component.line_assignments.get_mut(assignment_index)
        {
            assignment.local_position = local_position;
        }
        candidate
    }

    #[doc(hidden)]
    pub fn with_basis_column_for_replay(
        &self,
        position: usize,
        column: FourLoopComponentBasisColumn,
    ) -> Self {
        let mut candidate = self.clone();
        if let Some(target) = candidate.basis_columns.get_mut(position) {
            *target = column;
        }
        candidate
    }

    #[doc(hidden)]
    pub fn with_parity_flag_for_replay(
        &self,
        position: usize,
        left_rank_one_zero: bool,
        right_rank_one_zero: bool,
    ) -> Self {
        let mut candidate = self.clone();
        if let Some(witness) = candidate.parity_witnesses.get_mut(position) {
            witness.left_rank_one_zero = left_rank_one_zero;
            witness.right_rank_one_zero = right_rank_one_zero;
        }
        candidate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopComponentTransportOccurrence {
    row_index: u16,
    path_index: u32,
    leaf_id: u32,
    plan_index: u32,
}

impl FourLoopComponentTransportOccurrence {
    pub const fn row_index(self) -> u16 {
        self.row_index
    }
    pub const fn path_index(self) -> u32 {
        self.path_index
    }
    pub const fn leaf_id(self) -> u32 {
        self.leaf_id
    }
    pub const fn plan_index(self) -> u32 {
        self.plan_index
    }
}

/// Replayable transport certificate borrowing the exact inventory whose
/// private family contexts authenticate every plan.
pub struct FourLoopComponentTransport<'inventory> {
    inventory: &'inventory FourLoopNextInventory,
    config: FourLoopComponentTransportConfig,
    source_schema: &'static str,
    source_seed_checksum: u64,
    plans: Vec<FourLoopComponentTransportPlan>,
    occurrences: Vec<FourLoopComponentTransportOccurrence>,
    stats: FourLoopComponentTransportStats,
}

impl<'inventory> FourLoopComponentTransport<'inventory> {
    pub const SCHEMA: &'static str =
        "rustred-equal-mass-euclidean-four-loop-component-transport-v1";

    pub const fn config(&self) -> FourLoopComponentTransportConfig {
        self.config
    }
    pub const fn status(&self) -> FourLoopComponentTransportStatus {
        FourLoopComponentTransportStatus::ExactComponentTransport
    }
    pub const fn stats(&self) -> FourLoopComponentTransportStats {
        self.stats
    }
    pub fn plans(&self) -> &[FourLoopComponentTransportPlan] {
        &self.plans
    }
    pub fn occurrences(&self) -> &[FourLoopComponentTransportOccurrence] {
        &self.occurrences
    }
    pub const fn source_schema(&self) -> &'static str {
        self.source_schema
    }
    pub const fn source_seed_checksum(&self) -> u64 {
        self.source_seed_checksum
    }

    /// Authenticated inventory borrowed by this transport certificate.
    pub(crate) const fn inventory(&self) -> &'inventory FourLoopNextInventory {
        self.inventory
    }

    /// Crate-internal forwarding access to the authenticated parent family
    /// and exact coefficient context of one retained plan.
    ///
    /// Lower-loop closure must construct its induced services in this precise
    /// Symbolica variable map.  Keeping the forwarding surface private avoids
    /// exposing inventory implementation ownership or allowing callers to
    /// substitute a same-named but incompatible context.
    pub(crate) fn authenticated_source_context(
        &self,
        leaf_id: u32,
    ) -> Result<(&FourLoopNextBoundaryKey, &VacuumFamily), FourLoopComponentTransportError> {
        let (key, family, _) = self.inventory.authenticated_boundary_context(leaf_id)?;
        Ok((key, family))
    }

    /// Test/certificate tooling can replay one bounded altered plan without
    /// cloning the full 1,066-plan batch.
    #[doc(hidden)]
    pub fn replay_plan_candidate(
        &self,
        candidate: &FourLoopComponentTransportPlan,
    ) -> Result<(), FourLoopComponentTransportError> {
        replay_retained_plan(self.inventory, candidate)
    }

    /// Validate the gross frozen-census reservations without touching an
    /// inventory or performing exact algebra.
    pub fn preflight_config(
        config: FourLoopComponentTransportConfig,
    ) -> Result<(), FourLoopComponentTransportError> {
        preflight_config(config)
    }

    pub fn build(
        inventory: &'inventory FourLoopNextInventory,
        config: FourLoopComponentTransportConfig,
    ) -> Result<Self, FourLoopComponentTransportError> {
        preflight_config(config)?;
        authenticate_source(inventory)?;
        if inventory.boundary_target_summaries().len() != FOUR_LOOP_COMPONENT_TRANSPORT_PLANS {
            return Err(FourLoopComponentTransportError::CensusMismatch {
                resource: "boundary transport plans",
                expected: FOUR_LOOP_COMPONENT_TRANSPORT_PLANS,
                actual: inventory.boundary_target_summaries().len(),
            });
        }
        if inventory.boundary_occurrences().len() != FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES {
            return Err(FourLoopComponentTransportError::CensusMismatch {
                resource: "boundary transport occurrences",
                expected: FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES,
                actual: inventory.boundary_occurrences().len(),
            });
        }

        let mut plans = Vec::new();
        plans
            .try_reserve_exact(FOUR_LOOP_COMPONENT_TRANSPORT_PLANS)
            .map_err(|_| FourLoopComponentTransportError::AllocationFailed {
                resource: "component transport plans",
                requested: FOUR_LOOP_COMPONENT_TRANSPORT_PLANS,
            })?;
        let mut stats = FourLoopComponentTransportStats::default();
        for summary in inventory.boundary_target_summaries() {
            let plan = build_plan(inventory, summary.leaf_id())?;
            accumulate_plan_stats(&mut stats, &plan)?;
            plans.push(plan);
        }
        stats.plans = plans.len();
        stats.rational_operations = stats
            .plans
            .checked_mul(RATIONAL_OPERATION_RESERVATION_PER_PLAN)
            .ok_or(FourLoopComponentTransportError::ArithmeticOverflow {
                resource: "rational-operation reservation",
            })?;

        let mut plan_by_leaf = BTreeMap::new();
        for (plan_index, plan) in plans.iter().enumerate() {
            if plan_by_leaf.insert(plan.leaf_id, plan_index).is_some() {
                return Err(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id: plan.leaf_id,
                    stage: "duplicate plan leaf",
                });
            }
        }
        let mut occurrences = Vec::new();
        occurrences
            .try_reserve_exact(FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES)
            .map_err(|_| FourLoopComponentTransportError::AllocationFailed {
                resource: "component transport occurrences",
                requested: FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES,
            })?;
        for occurrence in inventory.boundary_occurrences() {
            let row_index = usize::from(occurrence.row_index());
            let path_index = occurrence.path_index() as usize;
            let path = inventory
                .rows()
                .get(row_index)
                .and_then(|row| row.paths().get(path_index))
                .ok_or(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id: u32::MAX,
                    stage: "occurrence coordinate",
                })?;
            let leaf_id = path.leaf_id();
            let &plan_index = plan_by_leaf.get(&leaf_id).ok_or(
                FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "occurrence plan lookup",
                },
            )?;
            if inventory.boundary_key(leaf_id)? != &plans[plan_index].key {
                return Err(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "occurrence full key",
                });
            }
            occurrences.push(FourLoopComponentTransportOccurrence {
                row_index: occurrence.row_index(),
                path_index: occurrence.path_index(),
                leaf_id,
                plan_index: u32::try_from(plan_index).map_err(|_| {
                    FourLoopComponentTransportError::ArithmeticOverflow {
                        resource: "occurrence plan index",
                    }
                })?,
            });
        }
        stats.occurrences = occurrences.len();
        check_stats(config, stats)?;

        Ok(Self {
            inventory,
            config,
            source_schema: FourLoopNextInventory::SCHEMA,
            source_seed_checksum: inventory.manifest().seed_checksum(),
            plans,
            occurrences,
            stats,
        })
    }

    /// Replay from primitive inventory contexts, matrices, local bases, exact
    /// probes, and two separate odd-rank projectors for every cross term.
    pub fn replay(&self) -> Result<(), FourLoopComponentTransportError> {
        authenticate_source(self.inventory)?;
        if self.source_schema != FourLoopNextInventory::SCHEMA {
            return Err(FourLoopComponentTransportError::SourceSchemaMismatch);
        }
        if self.source_seed_checksum != self.inventory.manifest().seed_checksum() {
            return Err(FourLoopComponentTransportError::SourceChecksumMismatch {
                expected: self.source_seed_checksum,
                actual: self.inventory.manifest().seed_checksum(),
            });
        }
        let mut rebuilt_stats = FourLoopComponentTransportStats::default();
        if self.plans.len() != self.inventory.boundary_target_summaries().len() {
            return Err(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id: u32::MAX,
                stage: "plan census",
            });
        }
        for (retained, summary) in self
            .plans
            .iter()
            .zip(self.inventory.boundary_target_summaries())
        {
            if retained.leaf_id != summary.leaf_id() {
                return Err(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id: retained.leaf_id,
                    stage: "plan summary order",
                });
            }
            replay_retained_plan(self.inventory, retained)?;
            accumulate_plan_stats(&mut rebuilt_stats, retained)?;
        }
        rebuilt_stats.plans = self.plans.len();
        rebuilt_stats.occurrences = self.occurrences.len();
        rebuilt_stats.rational_operations = rebuilt_stats
            .plans
            .checked_mul(RATIONAL_OPERATION_RESERVATION_PER_PLAN)
            .ok_or(FourLoopComponentTransportError::ArithmeticOverflow {
                resource: "replay rational-operation reservation",
            })?;
        let plan_by_leaf = self
            .plans
            .iter()
            .enumerate()
            .map(|(index, plan)| (plan.leaf_id, index))
            .collect::<BTreeMap<_, _>>();
        if self.occurrences.len() != self.inventory.boundary_occurrences().len() {
            return Err(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id: u32::MAX,
                stage: "occurrence census",
            });
        }
        for (retained, source) in self
            .occurrences
            .iter()
            .zip(self.inventory.boundary_occurrences())
        {
            let row_index = usize::from(source.row_index());
            let path_index = source.path_index() as usize;
            let leaf_id = self
                .inventory
                .rows()
                .get(row_index)
                .and_then(|row| row.paths().get(path_index))
                .map(|path| path.leaf_id())
                .ok_or(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id: retained.leaf_id,
                    stage: "occurrence replay coordinate",
                })?;
            let plan_index = *plan_by_leaf.get(&leaf_id).ok_or(
                FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "occurrence replay plan",
                },
            )?;
            let expected = FourLoopComponentTransportOccurrence {
                row_index: source.row_index(),
                path_index: source.path_index(),
                leaf_id,
                plan_index: u32::try_from(plan_index).map_err(|_| {
                    FourLoopComponentTransportError::ArithmeticOverflow {
                        resource: "replayed occurrence plan index",
                    }
                })?,
            };
            if retained != &expected
                || self.inventory.boundary_key(leaf_id)? != self.plans[plan_index].key()
            {
                return Err(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "occurrence replay identity",
                });
            }
        }
        if rebuilt_stats != self.stats {
            return Err(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id: u32::MAX,
                stage: "transport stats",
            });
        }
        check_stats(self.config, rebuilt_stats)
    }
}

#[derive(Debug)]
pub enum FourLoopComponentTransportError {
    Inventory(FourLoopNextInventoryError),
    Boundary(FourLoopBoundaryError),
    Tensor(TensorError),
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    AllocationFailed {
        resource: &'static str,
        requested: usize,
    },
    CensusMismatch {
        resource: &'static str,
        expected: usize,
        actual: usize,
    },
    SourceSchemaMismatch,
    SourceChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    UnsupportedDomain {
        leaf_id: u32,
        reason: &'static str,
    },
    LinearAlgebra(String),
    ArithmeticOverflow {
        resource: &'static str,
    },
    ReplayMismatch {
        leaf_id: u32,
        stage: &'static str,
    },
}

impl From<FourLoopNextInventoryError> for FourLoopComponentTransportError {
    fn from(error: FourLoopNextInventoryError) -> Self {
        Self::Inventory(error)
    }
}

impl From<FourLoopBoundaryError> for FourLoopComponentTransportError {
    fn from(error: FourLoopBoundaryError) -> Self {
        Self::Boundary(error)
    }
}

impl From<TensorError> for FourLoopComponentTransportError {
    fn from(error: TensorError) -> Self {
        Self::Tensor(error)
    }
}

impl fmt::Display for FourLoopComponentTransportError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "four-loop component transport: {self:?}")
    }
}

impl Error for FourLoopComponentTransportError {}

#[derive(Clone)]
struct InternalBasisEntry {
    column: FourLoopComponentBasisColumn,
    quadratic_form: [ExactRational; BASIS],
    shift: Coefficient,
}

fn preflight_config(
    config: FourLoopComponentTransportConfig,
) -> Result<(), FourLoopComponentTransportError> {
    let minima = [
        (
            "component transport plans",
            config.max_plans,
            FOUR_LOOP_COMPONENT_TRANSPORT_PLANS,
        ),
        (
            "component transport occurrences",
            config.max_occurrences,
            FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES,
        ),
        (
            "component transport components",
            config.max_components,
            FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENTS,
        ),
        (
            "component map entries",
            config.max_component_map_entries,
            FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENT_MAP_ENTRIES,
        ),
        (
            "signed line replays",
            config.max_signed_line_replays,
            FOUR_LOOP_COMPONENT_TRANSPORT_SIGNED_LINE_REPLAYS,
        ),
        (
            "complete local slots",
            config.max_local_slots,
            FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_SLOTS,
        ),
        (
            "retained loop-map entries",
            config.max_loop_map_entries,
            FOUR_LOOP_COMPONENT_TRANSPORT_LOOP_MAP_ENTRIES,
        ),
        (
            "transformed coefficients",
            config.max_transformed_coefficients,
            FOUR_LOOP_COMPONENT_TRANSPORT_TRANSFORMED_COEFFICIENTS,
        ),
        (
            "affine constants",
            config.max_affine_constants,
            FOUR_LOOP_COMPONENT_TRANSPORT_AFFINE_CONSTANTS,
        ),
        (
            "local coefficient inspections",
            config.max_local_coefficients,
            FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_COEFFICIENTS,
        ),
        (
            "cross coefficient inspections",
            config.max_cross_coefficients,
            FOUR_LOOP_COMPONENT_TRANSPORT_CROSS_COEFFICIENTS,
        ),
        (
            "rank-one parity projections",
            config.max_parity_projections,
            FOUR_LOOP_COMPONENT_TRANSPORT_PARITY_PROJECTIONS,
        ),
        (
            "scalar transport branches",
            config.max_scalar_branches,
            FOUR_LOOP_COMPONENT_TRANSPORT_SCALAR_BRANCHES,
        ),
        (
            "exact rational operations",
            config.max_rational_operations,
            FOUR_LOOP_COMPONENT_TRANSPORT_RATIONAL_OPERATIONS,
        ),
    ];
    for (resource, actual, minimum) in minima {
        if actual < minimum {
            return Err(FourLoopComponentTransportError::ResourceLimit {
                resource,
                requested: minimum as u128,
                limit: actual as u128,
            });
        }
    }
    Ok(())
}

fn authenticate_source(
    inventory: &FourLoopNextInventory,
) -> Result<(), FourLoopComponentTransportError> {
    if inventory.status() != FourLoopNextInventoryStatus::ExactPreclosureInventory {
        return Err(FourLoopComponentTransportError::SourceSchemaMismatch);
    }
    let actual = inventory.manifest().seed_checksum();
    if actual != FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM {
        return Err(FourLoopComponentTransportError::SourceChecksumMismatch {
            expected: FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM,
            actual,
        });
    }
    Ok(())
}

fn checked_add(
    target: &mut usize,
    value: usize,
    resource: &'static str,
) -> Result<(), FourLoopComponentTransportError> {
    *target = target
        .checked_add(value)
        .ok_or(FourLoopComponentTransportError::ArithmeticOverflow { resource })?;
    Ok(())
}

fn accumulate_plan_stats(
    stats: &mut FourLoopComponentTransportStats,
    plan: &FourLoopComponentTransportPlan,
) -> Result<(), FourLoopComponentTransportError> {
    checked_add(
        &mut stats.components,
        plan.components.len(),
        "component count",
    )?;
    checked_add(
        &mut stats.component_map_entries,
        plan.components
            .iter()
            .map(|component| component.global_basis_slots.len().pow(2))
            .sum(),
        "component map entries",
    )?;
    checked_add(
        &mut stats.signed_line_replays,
        plan.components
            .iter()
            .map(|component| component.line_assignments.len())
            .sum(),
        "signed line replays",
    )?;
    checked_add(
        &mut stats.local_slots,
        plan.components
            .iter()
            .map(|component| component.local_powers.len())
            .sum(),
        "complete local slots",
    )?;
    checked_add(
        &mut stats.loop_map_entries,
        LOOPS * LOOPS,
        "retained loop maps",
    )?;
    if plan.affine_image.is_some() {
        checked_add(&mut stats.n1_plans, 1, "N1 plans")?;
        checked_add(
            &mut stats.transformed_coefficients,
            BASIS,
            "transformed coefficients",
        )?;
        checked_add(&mut stats.affine_constants, 1, "affine constants")?;
        let local = plan
            .basis_columns
            .iter()
            .filter(|column| matches!(column, FourLoopComponentBasisColumn::Local { .. }))
            .count();
        checked_add(
            &mut stats.local_coefficients,
            local,
            "local coefficient inspections",
        )?;
        checked_add(
            &mut stats.cross_coefficients,
            BASIS - local,
            "cross coefficient inspections",
        )?;
    } else {
        checked_add(&mut stats.n0_plans, 1, "N0 plans")?;
    }
    checked_add(
        &mut stats.parity_projections,
        plan.parity_witnesses.len().checked_mul(2).ok_or(
            FourLoopComponentTransportError::ArithmeticOverflow {
                resource: "rank-one parity projections",
            },
        )?,
        "rank-one parity projections",
    )?;
    checked_add(
        &mut stats.scalar_branches,
        plan.scalar_branches.len(),
        "scalar branches",
    )?;
    Ok(())
}

fn check_stats(
    config: FourLoopComponentTransportConfig,
    stats: FourLoopComponentTransportStats,
) -> Result<(), FourLoopComponentTransportError> {
    let fields = [
        ("component transport plans", stats.plans, config.max_plans),
        (
            "component transport occurrences",
            stats.occurrences,
            config.max_occurrences,
        ),
        (
            "component transport components",
            stats.components,
            config.max_components,
        ),
        (
            "component map entries",
            stats.component_map_entries,
            config.max_component_map_entries,
        ),
        (
            "signed line replays",
            stats.signed_line_replays,
            config.max_signed_line_replays,
        ),
        (
            "complete local slots",
            stats.local_slots,
            config.max_local_slots,
        ),
        (
            "retained loop-map entries",
            stats.loop_map_entries,
            config.max_loop_map_entries,
        ),
        (
            "transformed coefficients",
            stats.transformed_coefficients,
            config.max_transformed_coefficients,
        ),
        (
            "affine constants",
            stats.affine_constants,
            config.max_affine_constants,
        ),
        (
            "local coefficient inspections",
            stats.local_coefficients,
            config.max_local_coefficients,
        ),
        (
            "cross coefficient inspections",
            stats.cross_coefficients,
            config.max_cross_coefficients,
        ),
        (
            "rank-one parity projections",
            stats.parity_projections,
            config.max_parity_projections,
        ),
        (
            "scalar transport branches",
            stats.scalar_branches,
            config.max_scalar_branches,
        ),
        (
            "exact rational operations",
            stats.rational_operations,
            config.max_rational_operations,
        ),
    ];
    for (resource, requested, limit) in fields {
        if requested > limit {
            return Err(FourLoopComponentTransportError::ResourceLimit {
                resource,
                requested: requested as u128,
                limit: limit as u128,
            });
        }
    }
    Ok(())
}

fn validate_key_power_mask(
    leaf_id: u32,
    key: &FourLoopNextBoundaryKey,
    family: &VacuumFamily,
) -> Result<(), FourLoopComponentTransportError> {
    let mut mask = 0_u16;
    for (position, (&power, denominator)) in
        key.powers().iter().zip(family.denominators()).enumerate()
    {
        if power > 0 {
            if !denominator.is_propagator() {
                return Err(FourLoopComponentTransportError::UnsupportedDomain {
                    leaf_id,
                    reason: "positive auxiliary power in boundary key",
                });
            }
            mask |= 1_u16 << position;
        }
    }
    if mask != key.sector_mask() {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "powers-derived physical mask",
        });
    }
    Ok(())
}

fn build_plan(
    inventory: &FourLoopNextInventory,
    leaf_id: u32,
) -> Result<FourLoopComponentTransportPlan, FourLoopComponentTransportError> {
    let (key, family, boundary) = inventory.authenticated_boundary_context(leaf_id)?;
    if family.loops() != LOOPS || family.denominator_count() != BASIS {
        return Err(FourLoopComponentTransportError::UnsupportedDomain {
            leaf_id,
            reason: "source family is not a complete four-loop basis",
        });
    }
    validate_key_power_mask(leaf_id, key, family)?;
    if boundary.replay_witness(key.witness())? != *key.product()
        || key.witness().topology() != key.topology()
        || key.witness().sector_mask() != key.sector_mask()
    {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "boundary witness authentication",
        });
    }

    let numerator_positions = key
        .powers()
        .iter()
        .enumerate()
        .filter_map(|(position, &power)| (power < 0).then_some((position, power)))
        .collect::<Vec<_>>();
    if numerator_positions.len() > 1
        || numerator_positions
            .first()
            .is_some_and(|(_, power)| *power != -1)
    {
        return Err(FourLoopComponentTransportError::UnsupportedDomain {
            leaf_id,
            reason: "transport supports N0 or one power-minus-one numerator",
        });
    }

    let witness = key.witness();
    let basis_matrix = witness
        .global_loop_map()
        .iter()
        .map(|row| row.to_vec())
        .collect::<Vec<_>>();
    let basis_inverse =
        invert_matrix(&basis_matrix).map_err(FourLoopComponentTransportError::LinearAlgebra)?;
    let mut scatter = vec![vec![ExactRational::zero(); LOOPS]; LOOPS];
    let mut reference_offset = 0_usize;
    for component in witness.components() {
        let rank = component.master().loops();
        if component.global_basis_slots().len() != rank
            || component.component_loop_map().len() != rank
            || component
                .component_loop_map()
                .iter()
                .any(|row| row.len() != rank)
            || reference_offset + rank > LOOPS
        {
            return Err(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id,
                stage: "component scatter dimensions",
            });
        }
        for (local_row, &global_slot) in component.global_basis_slots().iter().enumerate() {
            if global_slot >= LOOPS {
                return Err(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "component global slot",
                });
            }
            for local_column in 0..rank {
                scatter[global_slot][reference_offset + local_column] =
                    component.component_loop_map()[local_row][local_column].clone();
            }
        }
        reference_offset += rank;
    }
    if reference_offset != LOOPS
        || matrix_determinant(&scatter)
            .map_err(FourLoopComponentTransportError::LinearAlgebra)?
            .is_zero()
    {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "component scatter completeness",
        });
    }
    let loop_map = matrix_multiply(&basis_inverse, &scatter)
        .map_err(FourLoopComponentTransportError::LinearAlgebra)?;
    let determinant =
        matrix_determinant(&loop_map).map_err(FourLoopComponentTransportError::LinearAlgebra)?;
    if determinant != ExactRational::one() && determinant != -ExactRational::one() {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "component loop transform determinant",
        });
    }
    let loop_transform: [[ExactRational; LOOPS]; LOOPS] = loop_map
        .clone()
        .into_iter()
        .map(|row| {
            row.try_into()
                .map_err(|_| FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "component loop transform row",
                })
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "component loop transform",
        })?;

    let mut components = Vec::new();
    components
        .try_reserve_exact(witness.components().len())
        .map_err(|_| FourLoopComponentTransportError::AllocationFailed {
            resource: "transport plan components",
            requested: witness.components().len(),
        })?;
    let mut seen_parent_lines = Vec::new();
    reference_offset = 0;
    for (witness_index, component) in witness.components().iter().enumerate() {
        let mut local_powers = vec![0_i32; complete_local_slot_count(component.master())];
        let mut assignments = Vec::new();
        assignments
            .try_reserve_exact(component.signed_line_matches().len())
            .map_err(|_| FourLoopComponentTransportError::AllocationFailed {
                resource: "transport signed-line assignments",
                requested: component.signed_line_matches().len(),
            })?;
        for line_match in component.signed_line_matches() {
            let parent_position = line_match.physical_position();
            if parent_position >= family.propagator_count()
                || key.sector_mask() & (1_u16 << parent_position) == 0
                || seen_parent_lines.contains(&parent_position)
            {
                return Err(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "signed line partition",
                });
            }
            let local_position =
                compact_to_complete_position(component.master(), line_match.reference_position())
                    .ok_or(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "compact local line lift",
                })?;
            local_powers[local_position] = key.powers()[parent_position];
            verify_signed_line(
                leaf_id,
                family,
                &loop_map,
                component.master(),
                reference_offset,
                line_match.physical_position(),
                line_match.reference_position(),
                line_match.orientation_sign(),
            )?;
            seen_parent_lines.push(parent_position);
            assignments.push(FourLoopTransportLineAssignment {
                parent_position,
                compact_reference_position: line_match.reference_position(),
                local_position,
                orientation_sign: line_match.orientation_sign(),
            });
        }
        components.push(FourLoopTransportComponent {
            witness_index,
            master: component.master(),
            reference_loop_offset: reference_offset,
            global_basis_slots: component.global_basis_slots().to_vec(),
            local_powers,
            line_assignments: assignments,
        });
        reference_offset += component.master().loops();
    }
    seen_parent_lines.sort_unstable();
    let expected_parent_lines = (0..family.propagator_count())
        .filter(|position| key.sector_mask() & (1_u16 << position) != 0)
        .collect::<Vec<_>>();
    if seen_parent_lines != expected_parent_lines {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "active parent line coverage",
        });
    }

    let basis_entries = build_factorized_basis(family, &components, leaf_id)?;
    let basis_columns = basis_entries
        .iter()
        .map(|entry| entry.column)
        .collect::<Vec<_>>();
    let (affine_image, scalar_branches, parity_witnesses) =
        if let Some(&(source_position, _)) = numerator_positions.first() {
            build_n1_transport(
                leaf_id,
                family,
                &components,
                &basis_entries,
                &loop_map,
                source_position,
            )?
        } else {
            (
                None,
                vec![FourLoopComponentScalarBranch {
                    kind: FourLoopComponentScalarBranchKind::Base,
                    coefficient: family.coefficients().one(),
                    lowered_component_powers: None,
                }],
                Vec::new(),
            )
        };

    Ok(FourLoopComponentTransportPlan {
        leaf_id,
        key: key.clone(),
        loop_transform,
        components,
        basis_columns,
        affine_image,
        scalar_branches,
        parity_witnesses,
    })
}

fn complete_local_slot_count(master: MassiveVacuumMaster) -> usize {
    match master {
        MassiveVacuumMaster::T1 => 1,
        MassiveVacuumMaster::S2 => 3,
        MassiveVacuumMaster::B4 | MassiveVacuumMaster::F5 | MassiveVacuumMaster::M6 => 6,
    }
}

fn compact_to_complete_position(
    master: MassiveVacuumMaster,
    compact_position: usize,
) -> Option<usize> {
    match master {
        MassiveVacuumMaster::T1 => (compact_position == 0).then_some(0),
        MassiveVacuumMaster::S2 => (compact_position < 3).then_some(compact_position),
        MassiveVacuumMaster::B4 => [0, 1, 3, 5].get(compact_position).copied(),
        MassiveVacuumMaster::F5 => (compact_position < 5).then_some(compact_position),
        MassiveVacuumMaster::M6 => (compact_position < 6).then_some(compact_position),
    }
}

fn complete_reference_routings(master: MassiveVacuumMaster) -> Vec<Vec<ExactRational>> {
    let integer_rows: Vec<Vec<i8>> = match master {
        MassiveVacuumMaster::T1 => vec![vec![1]],
        MassiveVacuumMaster::S2 => vec![vec![1, 0], vec![0, 1], vec![1, 1]],
        MassiveVacuumMaster::B4 | MassiveVacuumMaster::F5 | MassiveVacuumMaster::M6 => {
            THREE_LOOP_TETRAHEDRON_ROUTINGS
                .iter()
                .map(|row| row.to_vec())
                .collect()
        }
    };
    integer_rows
        .into_iter()
        .map(|row| {
            row.into_iter()
                .map(|value| ExactRational::from(i64::from(value)))
                .collect()
        })
        .collect()
}

fn active_reference_routing(
    master: MassiveVacuumMaster,
    compact_position: usize,
) -> Option<Vec<ExactRational>> {
    let full_position = compact_to_complete_position(master, compact_position)?;
    complete_reference_routings(master)
        .get(full_position)
        .cloned()
}

fn verify_signed_line(
    leaf_id: u32,
    family: &VacuumFamily,
    loop_map: &[Vec<ExactRational>],
    master: MassiveVacuumMaster,
    reference_offset: usize,
    parent_position: usize,
    compact_reference_position: usize,
    orientation_sign: i8,
) -> Result<(), FourLoopComponentTransportError> {
    if !matches!(orientation_sign, -1 | 1) {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "signed line orientation",
        });
    }
    let parent = family
        .denominators()
        .get(parent_position)
        .and_then(|denominator| denominator.momentum())
        .ok_or(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "signed line parent routing",
        })?;
    let mapped = row_times_matrix(parent, loop_map)?;
    let local = active_reference_routing(master, compact_reference_position).ok_or(
        FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "signed line reference routing",
        },
    )?;
    let sign = ExactRational::from(i64::from(orientation_sign));
    let mut expected = vec![ExactRational::zero(); LOOPS];
    for (axis, value) in local.into_iter().enumerate() {
        let Some(target) = expected.get_mut(reference_offset + axis) else {
            return Err(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id,
                stage: "signed line reference offset",
            });
        };
        *target = &sign * &value;
    }
    if mapped != expected {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "signed line image",
        });
    }
    Ok(())
}

fn row_times_matrix(
    row: &[ExactRational],
    matrix: &[Vec<ExactRational>],
) -> Result<Vec<ExactRational>, FourLoopComponentTransportError> {
    if row.len() != matrix.len()
        || matrix.is_empty()
        || matrix
            .iter()
            .any(|matrix_row| matrix_row.len() != matrix[0].len())
    {
        return Err(FourLoopComponentTransportError::LinearAlgebra(
            "row/matrix dimensions do not match".to_owned(),
        ));
    }
    Ok((0..matrix[0].len())
        .map(|column| {
            row.iter()
                .zip(matrix)
                .map(|(coefficient, matrix_row)| coefficient * &matrix_row[column])
                .fold(ExactRational::zero(), |sum, term| sum + term)
        })
        .collect())
}

fn build_factorized_basis(
    family: &VacuumFamily,
    components: &[FourLoopTransportComponent],
    leaf_id: u32,
) -> Result<Vec<InternalBasisEntry>, FourLoopComponentTransportError> {
    let mass = family.coefficients().parameter("m2").ok_or(
        FourLoopComponentTransportError::UnsupportedDomain {
            leaf_id,
            reason: "source coefficient context has no m2 parameter",
        },
    )?;
    let mut entries = Vec::new();
    entries.try_reserve_exact(BASIS).map_err(|_| {
        FourLoopComponentTransportError::AllocationFailed {
            resource: "factorized basis entries",
            requested: BASIS,
        }
    })?;
    for (component_index, component) in components.iter().enumerate() {
        for (local_position, routing) in complete_reference_routings(component.master)
            .into_iter()
            .enumerate()
        {
            let mut embedded = vec![ExactRational::zero(); LOOPS];
            for (axis, value) in routing.into_iter().enumerate() {
                embedded[component.reference_loop_offset + axis] = value;
            }
            entries.push(InternalBasisEntry {
                column: FourLoopComponentBasisColumn::Local {
                    component_index,
                    local_position,
                },
                quadratic_form: routing_quadratic_form(&embedded),
                shift: mass.clone(),
            });
        }
    }
    for left_component in 0..components.len() {
        let left = &components[left_component];
        for right_component in left_component + 1..components.len() {
            let right = &components[right_component];
            for left_axis in 0..left.master.loops() {
                for right_axis in 0..right.master.loops() {
                    let left_global = left.reference_loop_offset + left_axis;
                    let right_global = right.reference_loop_offset + right_axis;
                    let mut quadratic_form = std::array::from_fn(|_| ExactRational::zero());
                    quadratic_form[scalar_product_index(left_global, right_global)] =
                        ExactRational::one();
                    entries.push(InternalBasisEntry {
                        column: FourLoopComponentBasisColumn::Cross {
                            left_component,
                            left_axis,
                            right_component,
                            right_axis,
                        },
                        quadratic_form,
                        shift: family.coefficients().zero(),
                    });
                }
            }
        }
    }
    if entries.len() != BASIS {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "complete factorized basis size",
        });
    }
    let determinant = matrix_determinant(
        &entries
            .iter()
            .map(|entry| entry.quadratic_form.to_vec())
            .collect::<Vec<_>>(),
    )
    .map_err(FourLoopComponentTransportError::LinearAlgebra)?;
    // Complete propagator bases are rational scalar-product bases, not loop
    // changes of variables.  Their off-diagonal qform entries carry the
    // conventional factor two, so nonsingularity (not unimodularity) is the
    // correct invariant here.
    if determinant.is_zero() {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "factorized scalar-product basis determinant",
        });
    }
    Ok(entries)
}

fn routing_quadratic_form(routing: &[ExactRational]) -> [ExactRational; BASIS] {
    let mut result = std::array::from_fn(|_| ExactRational::zero());
    for left in 0..LOOPS {
        for right in left..LOOPS {
            let factor = if left == right {
                ExactRational::one()
            } else {
                ExactRational::from(2)
            };
            result[scalar_product_index(left, right)] = &routing[left] * &routing[right] * factor;
        }
    }
    result
}

fn scalar_product_index(left: usize, right: usize) -> usize {
    let (left, right) = if left <= right {
        (left, right)
    } else {
        (right, left)
    };
    (0..left).map(|row| LOOPS - row).sum::<usize>() + right - left
}

fn build_n1_transport(
    leaf_id: u32,
    family: &VacuumFamily,
    components: &[FourLoopTransportComponent],
    basis_entries: &[InternalBasisEntry],
    loop_map: &[Vec<ExactRational>],
    source_position: usize,
) -> Result<
    (
        Option<FourLoopComponentAffineImage>,
        Vec<FourLoopComponentScalarBranch>,
        Vec<FourLoopComponentParityWitness>,
    ),
    FourLoopComponentTransportError,
> {
    let source = family.denominators().get(source_position).ok_or(
        FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "numerator source denominator",
        },
    )?;
    let transformed = transform_quadratic_form(source.quadratic_form(), loop_map);
    let basis_matrix = basis_entries
        .iter()
        .map(|entry| entry.quadratic_form.to_vec())
        .collect::<Vec<_>>();
    let basis_inverse =
        invert_matrix(&basis_matrix).map_err(FourLoopComponentTransportError::LinearAlgebra)?;
    let mut coefficients = std::array::from_fn(|_| ExactRational::zero());
    for target in 0..BASIS {
        coefficients[target] = (0..BASIS)
            .map(|scalar_product| {
                &transformed[scalar_product] * &basis_inverse[scalar_product][target]
            })
            .fold(ExactRational::zero(), |sum, term| sum + term);
    }
    let mut constant = source.shift().clone();
    for (coefficient, basis_entry) in coefficients.iter().zip(basis_entries) {
        if !coefficient.is_zero() {
            constant = &constant
                - &family
                    .coefficients()
                    .scale_rational(&basis_entry.shift, coefficient);
        }
    }
    let affine_image = FourLoopComponentAffineImage {
        source_position,
        constant: constant.clone(),
        coefficients: coefficients.clone(),
    };
    verify_affine_probes(
        leaf_id,
        family,
        source,
        loop_map,
        basis_entries,
        &affine_image,
    )?;

    let mut branches = Vec::new();
    branches.try_reserve(BASIS + 1).map_err(|_| {
        FourLoopComponentTransportError::AllocationFailed {
            resource: "component scalar branches",
            requested: BASIS + 1,
        }
    })?;
    if !constant.is_zero() {
        branches.push(FourLoopComponentScalarBranch {
            kind: FourLoopComponentScalarBranchKind::Constant,
            coefficient: constant,
            lowered_component_powers: None,
        });
    }
    let mut parity_witnesses = Vec::new();
    parity_witnesses.try_reserve(BASIS).map_err(|_| {
        FourLoopComponentTransportError::AllocationFailed {
            resource: "cross-component parity witnesses",
            requested: BASIS,
        }
    })?;
    let mut projector = VacuumTensorProjector::new(family.coefficients(), "d")?;
    for (basis_position, (basis_entry, coefficient)) in
        basis_entries.iter().zip(&coefficients).enumerate()
    {
        if coefficient.is_zero() {
            continue;
        }
        match basis_entry.column {
            FourLoopComponentBasisColumn::Local {
                component_index,
                local_position,
            } => {
                let component = components.get(component_index).ok_or(
                    FourLoopComponentTransportError::ReplayMismatch {
                        leaf_id,
                        stage: "local coefficient owner",
                    },
                )?;
                let mut lowered = component.local_powers.clone();
                let power = lowered.get_mut(local_position).ok_or(
                    FourLoopComponentTransportError::ReplayMismatch {
                        leaf_id,
                        stage: "local coefficient position",
                    },
                )?;
                *power = power.checked_sub(1).ok_or(
                    FourLoopComponentTransportError::ArithmeticOverflow {
                        resource: "lowered local power",
                    },
                )?;
                branches.push(FourLoopComponentScalarBranch {
                    kind: FourLoopComponentScalarBranchKind::Local {
                        component_index,
                        local_position,
                    },
                    coefficient: family.coefficients().rational(coefficient),
                    lowered_component_powers: Some(lowered),
                });
            }
            FourLoopComponentBasisColumn::Cross {
                left_component,
                left_axis,
                right_component,
                right_axis,
            } => {
                let left_vector =
                    reference_loop_vector(components, left_component, left_axis, leaf_id)?;
                let right_vector =
                    reference_loop_vector(components, right_component, right_axis, leaf_id)?;
                let left_zero = projector
                    .reduce(&TensorMonomial::new([IndexedVector::new(
                        left_vector,
                        LorentzIndex::new(0),
                    )]))?
                    .is_zero();
                let right_zero = projector
                    .reduce(&TensorMonomial::new([IndexedVector::new(
                        right_vector,
                        LorentzIndex::new(1),
                    )]))?
                    .is_zero();
                if !left_zero || !right_zero {
                    return Err(FourLoopComponentTransportError::ReplayMismatch {
                        leaf_id,
                        stage: "rank-one component parity",
                    });
                }
                parity_witnesses.push(FourLoopComponentParityWitness {
                    basis_position,
                    coefficient: coefficient.clone(),
                    left_component,
                    left_axis,
                    right_component,
                    right_axis,
                    left_rank_one_zero: left_zero,
                    right_rank_one_zero: right_zero,
                });
            }
        }
    }
    Ok((Some(affine_image), branches, parity_witnesses))
}

fn reference_loop_vector(
    components: &[FourLoopTransportComponent],
    component_index: usize,
    axis: usize,
    leaf_id: u32,
) -> Result<LoopVector, FourLoopComponentTransportError> {
    let component =
        components
            .get(component_index)
            .ok_or(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id,
                stage: "parity component ownership",
            })?;
    if axis >= component.master.loops() {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "parity component axis",
        });
    }
    let id = u16::try_from(component.reference_loop_offset + axis).map_err(|_| {
        FourLoopComponentTransportError::ArithmeticOverflow {
            resource: "parity loop-vector id",
        }
    })?;
    Ok(LoopVector::new(id))
}

fn transform_quadratic_form(
    source: &[ExactRational],
    loop_map: &[Vec<ExactRational>],
) -> [ExactRational; BASIS] {
    let mut output = std::array::from_fn(|_| ExactRational::zero());
    for source_left in 0..LOOPS {
        for source_right in source_left..LOOPS {
            let coefficient = &source[scalar_product_index(source_left, source_right)];
            if coefficient.is_zero() {
                continue;
            }
            for target_left in 0..LOOPS {
                for target_right in target_left..LOOPS {
                    let transformed = if target_left == target_right {
                        &loop_map[source_left][target_left] * &loop_map[source_right][target_right]
                    } else {
                        &loop_map[source_left][target_left] * &loop_map[source_right][target_right]
                            + &loop_map[source_left][target_right]
                                * &loop_map[source_right][target_left]
                    };
                    let target = scalar_product_index(target_left, target_right);
                    let contribution = coefficient * &transformed;
                    output[target] = &output[target] + &contribution;
                }
            }
        }
    }
    output
}

fn affine_probe_points() -> [[ExactRational; LOOPS]; 11] {
    let mut points = std::array::from_fn(|_| std::array::from_fn(|_| ExactRational::zero()));
    for axis in 0..LOOPS {
        points[1 + axis][axis] = ExactRational::one();
    }
    let mut position = 1 + LOOPS;
    for left in 0..LOOPS {
        for right in left + 1..LOOPS {
            points[position][left] = ExactRational::one();
            points[position][right] = ExactRational::one();
            position += 1;
        }
    }
    points
}

fn evaluate_quadratic_form(
    quadratic_form: &[ExactRational],
    point: &[ExactRational; LOOPS],
) -> ExactRational {
    let mut value = ExactRational::zero();
    for left in 0..LOOPS {
        for right in left..LOOPS {
            let term =
                &quadratic_form[scalar_product_index(left, right)] * &point[left] * &point[right];
            value = &value + &term;
        }
    }
    value
}

fn verify_affine_probes(
    leaf_id: u32,
    family: &VacuumFamily,
    source: &rustred::Denominator,
    loop_map: &[Vec<ExactRational>],
    basis_entries: &[InternalBasisEntry],
    image: &FourLoopComponentAffineImage,
) -> Result<(), FourLoopComponentTransportError> {
    for point in affine_probe_points() {
        let source_point = (0..LOOPS)
            .map(|row| {
                (0..LOOPS)
                    .map(|column| &loop_map[row][column] * &point[column])
                    .fold(ExactRational::zero(), |sum, term| sum + term)
            })
            .collect::<Vec<_>>();
        let source_point: [ExactRational; LOOPS] = source_point.try_into().map_err(|_| {
            FourLoopComponentTransportError::ReplayMismatch {
                leaf_id,
                stage: "affine source probe shape",
            }
        })?;
        let mut left = source.shift().clone();
        left = &left
            + &family.coefficients().rational(evaluate_quadratic_form(
                source.quadratic_form(),
                &source_point,
            ));

        let mut right = image.constant.clone();
        for (coefficient, basis_entry) in image.coefficients.iter().zip(basis_entries) {
            if coefficient.is_zero() {
                continue;
            }
            let basis_value = &basis_entry.shift
                + &family
                    .coefficients()
                    .rational(evaluate_quadratic_form(&basis_entry.quadratic_form, &point));
            right = &right
                + &family
                    .coefficients()
                    .scale_rational(&basis_value, coefficient);
        }
        if left != right {
            return Err(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id,
                stage: "eleven-point affine identity",
            });
        }
    }
    Ok(())
}

fn replay_retained_plan(
    inventory: &FourLoopNextInventory,
    plan: &FourLoopComponentTransportPlan,
) -> Result<(), FourLoopComponentTransportError> {
    let leaf_id = plan.leaf_id;
    let (key, family, boundary) = inventory.authenticated_boundary_context(leaf_id)?;
    if key != &plan.key
        || boundary.replay_witness(key.witness())? != *key.product()
        || key.witness().topology() != key.topology()
        || key.witness().sector_mask() != key.sector_mask()
    {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "retained source and witness",
        });
    }
    validate_key_power_mask(leaf_id, key, family)?;

    let witness = key.witness();
    let mut scatter = vec![vec![ExactRational::zero(); LOOPS]; LOOPS];
    let mut reference_offset = 0_usize;
    for component in witness.components() {
        let rank = component.master().loops();
        if component.global_basis_slots().len() != rank
            || component.component_loop_map().len() != rank
            || component
                .component_loop_map()
                .iter()
                .any(|row| row.len() != rank)
            || reference_offset + rank > LOOPS
        {
            return Err(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id,
                stage: "replay scatter dimensions",
            });
        }
        for (local_row, &global_slot) in component.global_basis_slots().iter().enumerate() {
            let scatter_row = scatter.get_mut(global_slot).ok_or(
                FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "replay scatter global slot",
                },
            )?;
            for local_column in 0..rank {
                scatter_row[reference_offset + local_column] =
                    component.component_loop_map()[local_row][local_column].clone();
            }
        }
        reference_offset += rank;
    }
    if reference_offset != LOOPS {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "replay scatter coverage",
        });
    }
    let basis_matrix = witness
        .global_loop_map()
        .iter()
        .map(|row| row.to_vec())
        .collect::<Vec<_>>();
    let retained_transform = plan
        .loop_transform
        .iter()
        .map(|row| row.to_vec())
        .collect::<Vec<_>>();
    if matrix_multiply(&basis_matrix, &retained_transform)
        .map_err(FourLoopComponentTransportError::LinearAlgebra)?
        != scatter
    {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "B times T equals scattered component map",
        });
    }
    let determinant = matrix_determinant(&retained_transform)
        .map_err(FourLoopComponentTransportError::LinearAlgebra)?;
    if determinant != ExactRational::one() && determinant != -ExactRational::one() {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "retained loop-transform determinant",
        });
    }

    if plan.components.len() != witness.components().len() {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "retained component census",
        });
    }
    let mut expected_active = Vec::new();
    reference_offset = 0;
    for (witness_index, (retained, source_component)) in
        plan.components.iter().zip(witness.components()).enumerate()
    {
        if retained.witness_index != witness_index
            || retained.master != source_component.master()
            || retained.reference_loop_offset != reference_offset
            || retained.global_basis_slots != source_component.global_basis_slots()
            || retained.local_powers.len() != complete_local_slot_count(retained.master)
            || retained.line_assignments.len() != source_component.signed_line_matches().len()
        {
            return Err(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id,
                stage: "retained component identity",
            });
        }
        let mut expected_powers = vec![0_i32; complete_local_slot_count(retained.master)];
        for (assignment, line_match) in retained
            .line_assignments
            .iter()
            .zip(source_component.signed_line_matches())
        {
            let local_position = compact_to_complete_position(
                source_component.master(),
                line_match.reference_position(),
            )
            .ok_or(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id,
                stage: "replay compact local line lift",
            })?;
            let expected_assignment = FourLoopTransportLineAssignment {
                parent_position: line_match.physical_position(),
                compact_reference_position: line_match.reference_position(),
                local_position,
                orientation_sign: line_match.orientation_sign(),
            };
            if assignment != &expected_assignment
                || expected_active.contains(&assignment.parent_position)
                || assignment.parent_position >= family.propagator_count()
                || key.sector_mask() & (1_u16 << assignment.parent_position) == 0
            {
                return Err(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "retained signed-line assignment",
                });
            }
            verify_signed_line(
                leaf_id,
                family,
                &retained_transform,
                retained.master,
                retained.reference_loop_offset,
                assignment.parent_position,
                assignment.compact_reference_position,
                assignment.orientation_sign,
            )?;
            expected_powers[assignment.local_position] = key.powers()[assignment.parent_position];
            expected_active.push(assignment.parent_position);
        }
        if retained.local_powers != expected_powers {
            return Err(FourLoopComponentTransportError::ReplayMismatch {
                leaf_id,
                stage: "retained complete local powers",
            });
        }
        reference_offset += retained.master.loops();
    }
    expected_active.sort_unstable();
    let active_from_mask = (0..family.propagator_count())
        .filter(|position| key.sector_mask() & (1_u16 << position) != 0)
        .collect::<Vec<_>>();
    if reference_offset != LOOPS || expected_active != active_from_mask {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "retained active-line coverage",
        });
    }

    let basis_entries = build_factorized_basis(family, &plan.components, leaf_id)?;
    let expected_columns = basis_entries
        .iter()
        .map(|entry| entry.column)
        .collect::<Vec<_>>();
    if plan.basis_columns != expected_columns {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "retained factorized-basis ordering",
        });
    }

    let numerator_positions = key
        .powers()
        .iter()
        .enumerate()
        .filter_map(|(position, &power)| (power < 0).then_some((position, power)))
        .collect::<Vec<_>>();
    match numerator_positions.as_slice() {
        [] => {
            let expected_branches = vec![FourLoopComponentScalarBranch {
                kind: FourLoopComponentScalarBranchKind::Base,
                coefficient: family.coefficients().one(),
                lowered_component_powers: None,
            }];
            if plan.affine_image.is_some()
                || plan.scalar_branches != expected_branches
                || !plan.parity_witnesses.is_empty()
            {
                return Err(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "retained N0 transport",
                });
            }
        }
        [(source_position, -1)] => {
            let image = plan.affine_image.as_ref().ok_or(
                FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "retained N1 affine image",
                },
            )?;
            if image.source_position != *source_position {
                return Err(FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "retained N1 source position",
                });
            }
            let source = family.denominators().get(*source_position).ok_or(
                FourLoopComponentTransportError::ReplayMismatch {
                    leaf_id,
                    stage: "retained N1 source denominator",
                },
            )?;
            verify_affine_probes(
                leaf_id,
                family,
                source,
                &retained_transform,
                &basis_entries,
                image,
            )?;
            replay_scalar_and_parity_branches(
                leaf_id,
                family,
                &plan.components,
                &basis_entries,
                image,
                &plan.scalar_branches,
                &plan.parity_witnesses,
            )?;
        }
        _ => {
            return Err(FourLoopComponentTransportError::UnsupportedDomain {
                leaf_id,
                reason: "replay supports N0 or one power-minus-one numerator",
            });
        }
    }
    Ok(())
}

fn replay_scalar_and_parity_branches(
    leaf_id: u32,
    family: &VacuumFamily,
    components: &[FourLoopTransportComponent],
    basis_entries: &[InternalBasisEntry],
    image: &FourLoopComponentAffineImage,
    retained_branches: &[FourLoopComponentScalarBranch],
    retained_parity: &[FourLoopComponentParityWitness],
) -> Result<(), FourLoopComponentTransportError> {
    let mut expected_branches = Vec::new();
    expected_branches.try_reserve(BASIS + 1).map_err(|_| {
        FourLoopComponentTransportError::AllocationFailed {
            resource: "replayed scalar branches",
            requested: BASIS + 1,
        }
    })?;
    if !image.constant.is_zero() {
        expected_branches.push(FourLoopComponentScalarBranch {
            kind: FourLoopComponentScalarBranchKind::Constant,
            coefficient: image.constant.clone(),
            lowered_component_powers: None,
        });
    }
    let mut expected_parity = Vec::new();
    expected_parity.try_reserve(BASIS).map_err(|_| {
        FourLoopComponentTransportError::AllocationFailed {
            resource: "replayed parity witnesses",
            requested: BASIS,
        }
    })?;
    let mut projector = VacuumTensorProjector::new(family.coefficients(), "d")?;
    for (basis_position, (entry, coefficient)) in
        basis_entries.iter().zip(&image.coefficients).enumerate()
    {
        if coefficient.is_zero() {
            continue;
        }
        match entry.column {
            FourLoopComponentBasisColumn::Local {
                component_index,
                local_position,
            } => {
                let component = components.get(component_index).ok_or(
                    FourLoopComponentTransportError::ReplayMismatch {
                        leaf_id,
                        stage: "replayed local branch owner",
                    },
                )?;
                let mut lowered = component.local_powers.clone();
                let target = lowered.get_mut(local_position).ok_or(
                    FourLoopComponentTransportError::ReplayMismatch {
                        leaf_id,
                        stage: "replayed local branch position",
                    },
                )?;
                *target = target.checked_sub(1).ok_or(
                    FourLoopComponentTransportError::ArithmeticOverflow {
                        resource: "replayed lowered local power",
                    },
                )?;
                expected_branches.push(FourLoopComponentScalarBranch {
                    kind: FourLoopComponentScalarBranchKind::Local {
                        component_index,
                        local_position,
                    },
                    coefficient: family.coefficients().rational(coefficient),
                    lowered_component_powers: Some(lowered),
                });
            }
            FourLoopComponentBasisColumn::Cross {
                left_component,
                left_axis,
                right_component,
                right_axis,
            } => {
                let left_vector =
                    reference_loop_vector(components, left_component, left_axis, leaf_id)?;
                let right_vector =
                    reference_loop_vector(components, right_component, right_axis, leaf_id)?;
                // These are deliberately two rank-one calls.  One global
                // rank-two projection would prove a different statement.
                let left_zero = projector
                    .reduce(&TensorMonomial::new([IndexedVector::new(
                        left_vector,
                        LorentzIndex::new(0),
                    )]))?
                    .is_zero();
                let right_zero = projector
                    .reduce(&TensorMonomial::new([IndexedVector::new(
                        right_vector,
                        LorentzIndex::new(1),
                    )]))?
                    .is_zero();
                if !left_zero || !right_zero {
                    return Err(FourLoopComponentTransportError::ReplayMismatch {
                        leaf_id,
                        stage: "replayed rank-one component parity",
                    });
                }
                expected_parity.push(FourLoopComponentParityWitness {
                    basis_position,
                    coefficient: coefficient.clone(),
                    left_component,
                    left_axis,
                    right_component,
                    right_axis,
                    left_rank_one_zero: left_zero,
                    right_rank_one_zero: right_zero,
                });
            }
        }
    }
    if retained_branches != expected_branches || retained_parity != expected_parity {
        return Err(FourLoopComponentTransportError::ReplayMismatch {
            leaf_id,
            stage: "replayed scalar branches and component parity",
        });
    }
    Ok(())
}
