//! Exact mass-normalized parent rows for the frozen four-loop next shell.
//!
//! This layer joins the compact next-shell inventory, authenticated component
//! transport, and the complementary T1/S2 and three-loop closure slices.  It
//! deliberately stops before Gaussian elimination: the result is the complete
//! ordered set of 1,968 canonical parent rows over `Q(d)` together with enough
//! compact provenance to replay every one of the 26,078 inventory paths.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;
use std::fmt::Write as _;

use crate::legacy_oracle_support::coefficient_degree::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use crate::{
    Coefficient, CoefficientContext, FourLoopComponentTransport, FourLoopCornerColumnId,
    FourLoopNextGenuineColumn, FourLoopNextInventory, FourLoopNextInventoryError, FourLoopNextLeaf,
    FourLoopNextRawRowId, FourLoopNextReplayedPath, FourLoopT1S2Closure, FourLoopThreeLoopClosure,
    MassiveVacuumMaster, MasterProduct, ProductLinearCombination,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

pub const FOUR_LOOP_NEXT_CLOSED_ROWS: usize = 1_968;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS: usize = 26_078;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES: usize = 4_230;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_PATHS: usize = 21_848;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS: usize = 1_066;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_COLUMNS: usize = 1_728;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_PRODUCT_COLUMNS: usize = 6;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS: usize = 1_734;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS: usize = 4_202;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_GROUPS: usize = 4_194;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_CANCELED_BOUNDARY_GROUPS: usize = 8;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_BOUNDARY_GROUPS: usize = 28;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_SURVIVING_BOUNDARY_GROUPS: usize = 20;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_CANCELED_BOUNDARY_GROUPS: usize = 8;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_CONTRIBUTORS: usize = 4_214;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_GROUPS: usize = 20_111;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_PATHS: usize = 118;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_BOUNDARY_GROUPS: usize = 17;

pub const FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTION_BOUND: usize = 45_275;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTION_BOUND: usize = 47_228;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND: usize = 3_237;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRY_BOUND: usize =
    FOUR_LOOP_NEXT_CLOSED_ROWS * FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTIONS: usize = 28_096;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTIONS: usize = 30_353;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES: usize = 22_424;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_ZERO_ROWS: usize = 0;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_WIDTH: usize = 45;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_MASS_POWER_STEPS: usize = 26_850;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_MULTIPLICATIONS: usize = 32_647;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_ADDITIONS: usize = 13_502;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_DIVISIONS: usize = 33_574;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_RETAINED_COEFFICIENT_TERMS: usize = 71_270;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_RETAINED_COEFFICIENT_BYTES: usize = 107_123;
pub const FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM: u64 = 0xa55ce4ffda6f8f5c;

const FNV1A64_OFFSET: u64 = 0xcbf29ce484222325;
const FNV1A64_PRIME: u64 = 0x100000001b3;

/// Independent resource envelopes for exact parent-row assembly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopNextClosedRowsConfig {
    pub max_rows: usize,
    pub max_paths: usize,
    pub max_plan_bindings: usize,
    pub max_occurrence_bindings: usize,
    pub max_boundary_groups: usize,
    pub max_boundary_group_contributors: usize,
    pub max_genuine_groups: usize,
    pub max_global_columns: usize,
    pub max_primary_contributions: usize,
    pub max_raw_audit_contributions: usize,
    pub max_collected_entries: usize,
    pub max_row_width: usize,
    pub max_mass_power_steps: usize,
    pub max_coefficient_operations: usize,
    pub max_coefficient_multiplications: usize,
    pub max_coefficient_additions: usize,
    pub max_coefficient_divisions: usize,
    pub max_coefficient_operation_terms: usize,
    pub max_coefficient_dense_terms: usize,
    pub max_coefficient_degree: usize,
    pub max_retained_coefficient_terms: usize,
    pub max_retained_coefficient_bytes: usize,
}

impl Default for FourLoopNextClosedRowsConfig {
    fn default() -> Self {
        Self {
            max_rows: FOUR_LOOP_NEXT_CLOSED_ROWS,
            max_paths: FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS,
            max_plan_bindings: FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS,
            max_occurrence_bindings: FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
            max_boundary_groups: FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS,
            max_boundary_group_contributors: FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
            max_genuine_groups: FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_GROUPS,
            max_global_columns: FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND,
            max_primary_contributions: FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTION_BOUND,
            max_raw_audit_contributions: FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTION_BOUND,
            max_collected_entries: FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRY_BOUND,
            max_row_width: FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND,
            max_mass_power_steps: 1_000_000,
            max_coefficient_operations: 5_000_000,
            max_coefficient_multiplications: 5_000_000,
            max_coefficient_additions: 5_000_000,
            max_coefficient_divisions: 5_000_000,
            max_coefficient_operation_terms: 1_000_000,
            max_coefficient_dense_terms: 1_000_000,
            max_coefficient_degree: 4_096,
            max_retained_coefficient_terms: 100_000_000,
            max_retained_coefficient_bytes: 1024 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopNextClosedRowsStatus {
    ExactFixedSeedParentRowsGenericQdEliminationPending,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum FourLoopNextClosureSlice {
    T1S2,
    ThreeLoop,
}

impl FourLoopNextClosureSlice {
    const fn stable_key(self) -> &'static str {
        match self {
            Self::T1S2 => "t1-s2",
            Self::ThreeLoop => "three-loop-component",
        }
    }
}

/// Binds one transport plan to exactly one complementary closure slice.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextPlanBinding {
    leaf_id: u32,
    transport_plan_index: u32,
    slice: FourLoopNextClosureSlice,
    closure_plan_index: u16,
}

impl FourLoopNextPlanBinding {
    pub const fn leaf_id(&self) -> u32 {
        self.leaf_id
    }
    pub const fn transport_plan_index(&self) -> u32 {
        self.transport_plan_index
    }
    pub const fn slice(&self) -> FourLoopNextClosureSlice {
        self.slice
    }
    pub const fn closure_plan_index(&self) -> u16 {
        self.closure_plan_index
    }

    #[doc(hidden)]
    pub fn with_slice_for_replay(&self, slice: FourLoopNextClosureSlice) -> Self {
        Self {
            slice,
            ..self.clone()
        }
    }

    #[doc(hidden)]
    pub fn with_closure_plan_index_for_replay(&self, closure_plan_index: u16) -> Self {
        Self {
            closure_plan_index,
            ..self.clone()
        }
    }
}

/// Binds one raw boundary path to transport, group, and closure coordinates.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextOccurrenceBinding {
    row_index: u16,
    path_index: u32,
    leaf_id: u32,
    transport_plan_index: u32,
    plan_binding_index: u32,
    boundary_group_index: u32,
    slice: FourLoopNextClosureSlice,
    closure_occurrence_index: u32,
}

impl FourLoopNextOccurrenceBinding {
    pub const fn row_index(&self) -> u16 {
        self.row_index
    }
    pub const fn path_index(&self) -> u32 {
        self.path_index
    }
    pub const fn leaf_id(&self) -> u32 {
        self.leaf_id
    }
    pub const fn transport_plan_index(&self) -> u32 {
        self.transport_plan_index
    }
    pub const fn plan_binding_index(&self) -> u32 {
        self.plan_binding_index
    }
    pub const fn boundary_group_index(&self) -> u32 {
        self.boundary_group_index
    }
    pub const fn slice(&self) -> FourLoopNextClosureSlice {
        self.slice
    }
    pub const fn closure_occurrence_index(&self) -> u32 {
        self.closure_occurrence_index
    }

    #[doc(hidden)]
    pub fn with_boundary_group_index_for_replay(&self, boundary_group_index: u32) -> Self {
        Self {
            boundary_group_index,
            ..self.clone()
        }
    }

    #[doc(hidden)]
    pub fn with_leaf_id_for_replay(&self, leaf_id: u32) -> Self {
        Self {
            leaf_id,
            ..self.clone()
        }
    }
}

/// Compact terminal disposition aligned one-for-one with a row's paths.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FourLoopNextPathDisposition {
    FamilyScaleless {
        leaf_id: u32,
    },
    ScalarCornerScaleless {
        leaf_id: u32,
    },
    Genuine {
        leaf_id: u32,
        column_index: u32,
    },
    Boundary {
        leaf_id: u32,
        occurrence_binding_index: u32,
        boundary_group_index: u32,
    },
}

impl FourLoopNextPathDisposition {
    pub const fn leaf_id(&self) -> u32 {
        match *self {
            Self::FamilyScaleless { leaf_id }
            | Self::ScalarCornerScaleless { leaf_id }
            | Self::Genuine { leaf_id, .. }
            | Self::Boundary { leaf_id, .. } => leaf_id,
        }
    }
}

/// One raw `(row, boundary leaf)` group, including groups canceled to zero.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextBoundaryGroup {
    row_index: u16,
    leaf_id: u32,
    plan_binding_index: u32,
    contributor_path_indices: Vec<u32>,
    collected_coefficient: Coefficient,
    seed_mass_weight: i64,
    boundary_mass_weight: i64,
    mass_bridge_exponent: i64,
    seed_to_boundary_coefficient: Coefficient,
    canceled: bool,
}

impl FourLoopNextBoundaryGroup {
    pub const fn row_index(&self) -> u16 {
        self.row_index
    }
    pub const fn leaf_id(&self) -> u32 {
        self.leaf_id
    }
    pub const fn plan_binding_index(&self) -> u32 {
        self.plan_binding_index
    }
    pub fn contributor_path_indices(&self) -> &[u32] {
        &self.contributor_path_indices
    }
    pub const fn collected_coefficient(&self) -> &Coefficient {
        &self.collected_coefficient
    }
    pub const fn seed_mass_weight(&self) -> i64 {
        self.seed_mass_weight
    }
    pub const fn boundary_mass_weight(&self) -> i64 {
        self.boundary_mass_weight
    }
    pub const fn mass_bridge_exponent(&self) -> i64 {
        self.mass_bridge_exponent
    }
    pub const fn seed_to_boundary_coefficient(&self) -> &Coefficient {
        &self.seed_to_boundary_coefficient
    }
    pub const fn canceled(&self) -> bool {
        self.canceled
    }

    #[doc(hidden)]
    pub fn with_contributor_path_indices_for_replay(&self, values: Vec<u32>) -> Self {
        Self {
            contributor_path_indices: values,
            ..self.clone()
        }
    }

    #[doc(hidden)]
    pub fn with_collected_coefficient_for_replay(&self, value: Coefficient) -> Self {
        Self {
            collected_coefficient: value,
            ..self.clone()
        }
    }

    #[doc(hidden)]
    pub fn with_mass_bridge_exponent_for_replay(&self, value: i64) -> Self {
        Self {
            mass_bridge_exponent: value,
            ..self.clone()
        }
    }

    #[doc(hidden)]
    pub fn with_seed_to_boundary_coefficient_for_replay(&self, value: Coefficient) -> Self {
        Self {
            seed_to_boundary_coefficient: value,
            ..self.clone()
        }
    }
}

/// One fully closed and mass-normalized parent row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopNextClosedRow {
    raw_id: FourLoopNextRawRowId,
    seed_mass_weight: i64,
    path_dispositions: Vec<FourLoopNextPathDisposition>,
    boundary_group_indices: Vec<u32>,
    row_scale: Coefficient,
    pivot_column_index: Option<u32>,
    entries: BTreeMap<FourLoopCornerColumnId, Coefficient>,
}

impl FourLoopNextClosedRow {
    pub const fn raw_id(&self) -> FourLoopNextRawRowId {
        self.raw_id
    }
    pub const fn seed_mass_weight(&self) -> i64 {
        self.seed_mass_weight
    }
    pub fn path_dispositions(&self) -> &[FourLoopNextPathDisposition] {
        &self.path_dispositions
    }
    pub fn boundary_group_indices(&self) -> &[u32] {
        &self.boundary_group_indices
    }
    pub const fn row_scale(&self) -> &Coefficient {
        &self.row_scale
    }
    pub const fn pivot_column_index(&self) -> Option<u32> {
        self.pivot_column_index
    }
    pub fn pivot(&self) -> Option<&FourLoopCornerColumnId> {
        self.entries.last_key_value().map(|(column, _)| column)
    }
    pub fn entries(&self) -> &BTreeMap<FourLoopCornerColumnId, Coefficient> {
        &self.entries
    }
    pub fn coefficient(&self, column: &FourLoopCornerColumnId) -> Option<&Coefficient> {
        self.entries.get(column)
    }

    #[doc(hidden)]
    pub fn with_row_scale_for_replay(&self, row_scale: Coefficient) -> Self {
        Self {
            row_scale,
            ..self.clone()
        }
    }

    #[doc(hidden)]
    pub fn with_coefficient_for_replay(
        &self,
        column: FourLoopCornerColumnId,
        coefficient: Coefficient,
    ) -> Self {
        let mut candidate = self.clone();
        candidate.entries.remove(&column);
        if !coefficient.is_zero() {
            candidate.entries.insert(column, coefficient);
        }
        candidate
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopNextClosedRowsStats {
    rows: usize,
    paths: usize,
    boundary_paths: usize,
    genuine_paths: usize,
    scaleless_paths: usize,
    plan_bindings: usize,
    occurrence_bindings: usize,
    raw_boundary_groups: usize,
    nonzero_boundary_groups: usize,
    canceled_boundary_groups: usize,
    repeated_boundary_groups: usize,
    repeated_surviving_boundary_groups: usize,
    repeated_canceled_boundary_groups: usize,
    nonzero_boundary_contributors: usize,
    genuine_groups: usize,
    genuine_columns: usize,
    product_columns: usize,
    global_columns: usize,
    primary_contributions: usize,
    raw_audit_contributions: usize,
    collected_entries: usize,
    zero_rows: usize,
    max_row_paths: usize,
    max_row_boundary_groups: usize,
    max_row_width: usize,
    mass_power_steps: usize,
    coefficient_multiplications: usize,
    coefficient_additions: usize,
    coefficient_divisions: usize,
    retained_coefficient_terms: usize,
    retained_coefficient_bytes: usize,
}

macro_rules! stat_getters {
    ($($name:ident),* $(,)?) => {
        $(pub const fn $name(self) -> usize { self.$name })*
    };
}

impl FourLoopNextClosedRowsStats {
    stat_getters!(
        rows,
        paths,
        boundary_paths,
        genuine_paths,
        scaleless_paths,
        plan_bindings,
        occurrence_bindings,
        raw_boundary_groups,
        nonzero_boundary_groups,
        canceled_boundary_groups,
        repeated_boundary_groups,
        repeated_surviving_boundary_groups,
        repeated_canceled_boundary_groups,
        nonzero_boundary_contributors,
        genuine_groups,
        genuine_columns,
        product_columns,
        global_columns,
        primary_contributions,
        raw_audit_contributions,
        collected_entries,
        zero_rows,
        max_row_paths,
        max_row_boundary_groups,
        max_row_width,
        mass_power_steps,
        coefficient_multiplications,
        coefficient_additions,
        coefficient_divisions,
        retained_coefficient_terms,
        retained_coefficient_bytes,
    );

    pub const fn coefficient_operations(self) -> usize {
        self.coefficient_multiplications + self.coefficient_additions + self.coefficient_divisions
    }
}

/// Exact parent-row certificate borrowing all four authenticated source layers.
pub struct FourLoopNextClosedRows<'sources, 'transport, 'inventory> {
    inventory: &'inventory FourLoopNextInventory,
    transport: &'transport FourLoopComponentTransport<'inventory>,
    t1s2: &'sources FourLoopT1S2Closure<'transport, 'inventory>,
    three_loop: &'sources FourLoopThreeLoopClosure<'transport, 'inventory>,
    config: FourLoopNextClosedRowsConfig,
    coefficient_context: CoefficientContext,
    plan_bindings: Vec<FourLoopNextPlanBinding>,
    occurrence_bindings: Vec<FourLoopNextOccurrenceBinding>,
    boundary_groups: Vec<FourLoopNextBoundaryGroup>,
    columns: Vec<FourLoopCornerColumnId>,
    rows: Vec<FourLoopNextClosedRow>,
    stats: FourLoopNextClosedRowsStats,
    checksum: u64,
}

impl<'sources, 'transport, 'inventory> FourLoopNextClosedRows<'sources, 'transport, 'inventory> {
    pub const SCHEMA: &'static str =
        "rustred-equal-mass-euclidean-four-loop-next-closed-parent-rows-v1";

    pub fn build(
        inventory: &'inventory FourLoopNextInventory,
        transport: &'transport FourLoopComponentTransport<'inventory>,
        t1s2: &'sources FourLoopT1S2Closure<'transport, 'inventory>,
        three_loop: &'sources FourLoopThreeLoopClosure<'transport, 'inventory>,
        config: FourLoopNextClosedRowsConfig,
    ) -> Result<Self, FourLoopNextClosedRowsError> {
        Self::build_impl(inventory, transport, t1s2, three_loop, config)
    }

    pub fn preflight_config(
        config: FourLoopNextClosedRowsConfig,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        preflight_config(config)
    }

    pub const fn config(&self) -> FourLoopNextClosedRowsConfig {
        self.config
    }
    pub const fn status(&self) -> FourLoopNextClosedRowsStatus {
        FourLoopNextClosedRowsStatus::ExactFixedSeedParentRowsGenericQdEliminationPending
    }
    pub const fn coefficient_context(&self) -> &CoefficientContext {
        &self.coefficient_context
    }
    pub const fn inventory_schema(&self) -> &'static str {
        FourLoopNextInventory::SCHEMA
    }
    pub const fn transport_schema(&self) -> &'static str {
        FourLoopComponentTransport::SCHEMA
    }
    pub const fn t1s2_schema(&self) -> &'static str {
        FourLoopT1S2Closure::SCHEMA
    }
    pub const fn three_loop_schema(&self) -> &'static str {
        FourLoopThreeLoopClosure::SCHEMA
    }
    pub fn plan_bindings(&self) -> &[FourLoopNextPlanBinding] {
        &self.plan_bindings
    }
    pub fn occurrence_bindings(&self) -> &[FourLoopNextOccurrenceBinding] {
        &self.occurrence_bindings
    }
    pub fn boundary_groups(&self) -> &[FourLoopNextBoundaryGroup] {
        &self.boundary_groups
    }
    pub fn columns(&self) -> &[FourLoopCornerColumnId] {
        &self.columns
    }
    pub fn rows(&self) -> &[FourLoopNextClosedRow] {
        &self.rows
    }
    pub const fn stats(&self) -> FourLoopNextClosedRowsStats {
        self.stats
    }
    pub const fn checksum(&self) -> u64 {
        self.checksum
    }
    pub const fn inventory_seed_checksum(&self) -> u64 {
        self.inventory.manifest().seed_checksum()
    }
    pub const fn transport_source_seed_checksum(&self) -> u64 {
        self.transport.source_seed_checksum()
    }
    pub const fn t1s2_checksum(&self) -> u64 {
        self.t1s2.checksum()
    }
    pub const fn three_loop_checksum(&self) -> u64 {
        self.three_loop.checksum()
    }

    /// Rebuild all rows from the borrowed immutable sources and compare every
    /// retained record and checksum.  The already-built lower-loop services
    /// are reused; no FORM process or hidden external state is involved.
    pub fn replay(&self) -> Result<(), FourLoopNextClosedRowsError> {
        let replayed = Self::build_impl(
            self.inventory,
            self.transport,
            self.t1s2,
            self.three_loop,
            self.config,
        )?;
        if replayed
            .coefficient_context
            .has_same_variable_map(&self.coefficient_context)
            && replayed.plan_bindings == self.plan_bindings
            && replayed.occurrence_bindings == self.occurrence_bindings
            && replayed.boundary_groups == self.boundary_groups
            && replayed.columns == self.columns
            && replayed.rows == self.rows
            && replayed.stats == self.stats
            && replayed.checksum == self.checksum
        {
            Ok(())
        } else {
            Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "complete parent-row certificate",
            })
        }
    }

    #[doc(hidden)]
    pub fn replay_plan_binding_candidate(
        &self,
        index: usize,
        candidate: &FourLoopNextPlanBinding,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        authenticate_sources(self.inventory, self.transport, self.t1s2, self.three_loop)?;
        let expected = source_plan_binding_at(
            index,
            self.transport,
            self.t1s2,
            self.three_loop,
            self.config,
        )?;
        if &expected == candidate {
            Ok(())
        } else {
            Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "plan binding candidate",
            })
        }
    }

    #[doc(hidden)]
    pub fn replay_occurrence_binding_candidate(
        &self,
        index: usize,
        candidate: &FourLoopNextOccurrenceBinding,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        authenticate_sources(self.inventory, self.transport, self.t1s2, self.three_loop)?;
        let expected = source_occurrence_binding_at(
            index,
            self.inventory,
            self.transport,
            self.t1s2,
            self.three_loop,
            self.config,
        )?;
        if &expected == candidate {
            Ok(())
        } else {
            Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence binding candidate",
            })
        }
    }

    #[doc(hidden)]
    pub fn replay_boundary_group_candidate(
        &self,
        index: usize,
        candidate: &FourLoopNextBoundaryGroup,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        authenticate_sources(self.inventory, self.transport, self.t1s2, self.three_loop)?;
        let expected = source_boundary_group_at(
            index,
            self.inventory,
            self.transport,
            self.t1s2,
            self.three_loop,
            self.config,
        )?;
        if &expected == candidate {
            Ok(())
        } else {
            Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "boundary group candidate",
            })
        }
    }

    #[doc(hidden)]
    pub fn replay_row_candidate(
        &self,
        index: usize,
        candidate: &FourLoopNextClosedRow,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        authenticate_sources(self.inventory, self.transport, self.t1s2, self.three_loop)?;
        let expected = source_closed_row_at(
            index,
            self.inventory,
            self.transport,
            self.t1s2,
            self.three_loop,
            self.config,
        )?;
        if &expected == candidate {
            Ok(())
        } else {
            Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "closed row candidate",
            })
        }
    }

    fn build_impl(
        inventory: &'inventory FourLoopNextInventory,
        transport: &'transport FourLoopComponentTransport<'inventory>,
        t1s2: &'sources FourLoopT1S2Closure<'transport, 'inventory>,
        three_loop: &'sources FourLoopThreeLoopClosure<'transport, 'inventory>,
        config: FourLoopNextClosedRowsConfig,
    ) -> Result<Self, FourLoopNextClosedRowsError> {
        preflight_config(config)?;
        authenticate_sources(inventory, transport, t1s2, three_loop)?;

        let coefficient_context = t1s2.coefficient_context().clone();
        let mut arithmetic = CheckedArithmetic::new(coefficient_context.clone(), config)?;
        let mut retained_coefficients = RetainedCoefficientCharge::new(config);
        let plan_bindings = build_plan_bindings(transport, t1s2, three_loop, config)?;
        let plan_by_leaf = plan_bindings
            .iter()
            .enumerate()
            .map(|(index, binding)| (binding.leaf_id, index))
            .collect::<BTreeMap<_, _>>();

        let mut stats = FourLoopNextClosedRowsStats::default();
        stats.rows = inventory.rows().len();
        stats.plan_bindings = plan_bindings.len();

        let (boundary_groups, group_by_row_leaf, mut structural) = prescan_groups(
            inventory,
            &plan_by_leaf,
            &coefficient_context,
            &mut arithmetic,
            &mut retained_coefficients,
            config,
        )?;
        stats.paths = structural.paths;
        stats.boundary_paths = structural.boundary_paths;
        stats.genuine_paths = structural.genuine_paths;
        stats.scaleless_paths = structural.scaleless_paths;
        stats.raw_boundary_groups = boundary_groups.len();
        stats.nonzero_boundary_groups = structural.nonzero_boundary_groups;
        stats.canceled_boundary_groups = structural.canceled_boundary_groups;
        stats.repeated_boundary_groups = structural.repeated_boundary_groups;
        stats.repeated_surviving_boundary_groups = structural.repeated_surviving_boundary_groups;
        stats.repeated_canceled_boundary_groups = structural.repeated_canceled_boundary_groups;
        stats.nonzero_boundary_contributors = structural.nonzero_boundary_contributors;
        stats.genuine_groups = structural.genuine_groups;
        stats.max_row_paths = structural.max_row_paths;
        stats.max_row_boundary_groups = structural.max_row_boundary_groups;
        check_early_structural_stats(stats)?;

        let occurrence_bindings = build_occurrence_bindings(
            inventory,
            transport,
            t1s2,
            three_loop,
            &plan_bindings,
            &group_by_row_leaf,
            config,
        )?;
        stats.occurrence_bindings = occurrence_bindings.len();
        check_exact_count(
            "occurrence bindings",
            FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
            stats.occurrence_bindings,
        )?;
        let occurrence_by_coordinate = occurrence_bindings
            .iter()
            .enumerate()
            .map(|(index, occurrence)| ((occurrence.row_index, occurrence.path_index), index))
            .collect::<BTreeMap<_, _>>();

        let columns = build_columns(inventory, t1s2, three_loop, config)?;
        let column_indices = columns
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, column)| (column, index))
            .collect::<BTreeMap<_, _>>();
        stats.genuine_columns = columns
            .iter()
            .filter(|column| matches!(column, FourLoopCornerColumnId::Genuine { .. }))
            .count();
        stats.product_columns = columns.len() - stats.genuine_columns;
        stats.global_columns = columns.len();
        check_exact_count(
            "genuine columns",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_COLUMNS,
            stats.genuine_columns,
        )?;
        check_exact_count(
            "product columns",
            FOUR_LOOP_NEXT_CLOSED_ROWS_PRODUCT_COLUMNS,
            stats.product_columns,
        )?;
        check_exact_count(
            "global columns",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
            stats.global_columns,
        )?;

        let groups_by_row = index_groups_by_row(&boundary_groups, inventory.rows().len())?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(inventory.rows().len())
            .map_err(|_| FourLoopNextClosedRowsError::AllocationFailed {
                resource: "closed parent rows",
                requested: inventory.rows().len(),
            })?;
        let mut replay_cache = inventory.new_replay_cache();

        for (row_index, source_row) in inventory.rows().iter().enumerate() {
            let seed_mass_weight = powers_weight(source_row.raw_id().seed().powers())?;
            let replayed_paths = inventory.replay_row_paths_cached(row_index, &mut replay_cache)?;
            if replayed_paths.len() != source_row.paths().len() {
                return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "inventory row path replay length",
                });
            }
            let mut primary_genuine = BTreeMap::<FourLoopCornerColumnId, Coefficient>::new();
            let mut replayed_boundary_sums = BTreeMap::<u32, Coefficient>::new();
            let mut primary = BTreeMap::<FourLoopCornerColumnId, Coefficient>::new();
            let mut raw_audit = BTreeMap::<FourLoopCornerColumnId, Coefficient>::new();
            let mut path_dispositions = Vec::new();
            path_dispositions
                .try_reserve_exact(source_row.paths().len())
                .map_err(|_| FourLoopNextClosedRowsError::AllocationFailed {
                    resource: "path dispositions",
                    requested: source_row.paths().len(),
                })?;

            for (path_index, (compact, replayed)) in source_row
                .paths()
                .iter()
                .copied()
                .zip(&replayed_paths)
                .enumerate()
            {
                if replayed.leaf_id() != compact.leaf_id()
                    || inventory.leaves().get(compact.leaf_id() as usize) != Some(replayed.leaf())
                {
                    return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "inventory path leaf",
                    });
                }
                let leaf_id = compact.leaf_id();
                match replayed.leaf() {
                    FourLoopNextLeaf::FamilyScaleless { .. } => {
                        path_dispositions
                            .push(FourLoopNextPathDisposition::FamilyScaleless { leaf_id });
                    }
                    FourLoopNextLeaf::ScalarCornerScaleless { .. } => {
                        path_dispositions
                            .push(FourLoopNextPathDisposition::ScalarCornerScaleless { leaf_id });
                    }
                    FourLoopNextLeaf::Genuine(genuine) => {
                        inventory.authenticate_genuine_column(genuine)?;
                        let column = genuine_column_id(genuine);
                        let column_index = *column_indices.get(&column).ok_or(
                            FourLoopNextClosedRowsError::ReplayMismatch {
                                stage: "genuine column index",
                            },
                        )?;
                        path_dispositions.push(FourLoopNextPathDisposition::Genuine {
                            leaf_id,
                            column_index: checked_u32(column_index, "genuine column index")?,
                        });
                        arithmetic.add_sparse(
                            &mut primary_genuine,
                            column.clone(),
                            replayed.final_coefficient().clone(),
                        )?;
                        let normalized = arithmetic.apply_mass_power(
                            replayed.final_coefficient(),
                            seed_mass_weight.checked_sub(column.mass_weight()).ok_or(
                                FourLoopNextClosedRowsError::ArithmeticOverflow {
                                    resource: "raw genuine mass exponent",
                                },
                            )?,
                        )?;
                        arithmetic.check_mass_free(&normalized, source_row.raw_id(), &column)?;
                        arithmetic.add_sparse(&mut raw_audit, column, normalized)?;
                        structural.raw_audit_contributions = checked_add(
                            structural.raw_audit_contributions,
                            1,
                            "raw-audit contributions",
                        )?;
                    }
                    FourLoopNextLeaf::Boundary(_) => {
                        let occurrence_index = *occurrence_by_coordinate
                            .get(&(
                                checked_u16(row_index, "row index")?,
                                checked_u32(path_index, "path index")?,
                            ))
                            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                                stage: "boundary occurrence coordinate",
                            })?;
                        let occurrence = &occurrence_bindings[occurrence_index];
                        if occurrence.leaf_id != leaf_id {
                            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                                stage: "boundary occurrence leaf",
                            });
                        }
                        path_dispositions.push(FourLoopNextPathDisposition::Boundary {
                            leaf_id,
                            occurrence_binding_index: checked_u32(
                                occurrence_index,
                                "occurrence binding index",
                            )?,
                            boundary_group_index: occurrence.boundary_group_index,
                        });
                        arithmetic.add_sparse(
                            &mut replayed_boundary_sums,
                            leaf_id,
                            replayed.final_coefficient().clone(),
                        )?;
                        let binding = plan_bindings
                            .get(occurrence.plan_binding_index as usize)
                            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                                stage: "occurrence plan binding",
                            })?;
                        let (ordinary, _) = closure_combinations(binding, t1s2, three_loop)?;
                        for (product, closure_coefficient) in ordinary.terms() {
                            let column = FourLoopCornerColumnId::Product(product.clone());
                            let multiplied = arithmetic
                                .multiply(replayed.final_coefficient(), closure_coefficient)?;
                            let normalized = arithmetic.apply_mass_power(
                                &multiplied,
                                seed_mass_weight.checked_sub(column.mass_weight()).ok_or(
                                    FourLoopNextClosedRowsError::ArithmeticOverflow {
                                        resource: "raw boundary mass exponent",
                                    },
                                )?,
                            )?;
                            arithmetic.check_mass_free(
                                &normalized,
                                source_row.raw_id(),
                                &column,
                            )?;
                            arithmetic.add_sparse(&mut raw_audit, column, normalized)?;
                            structural.raw_audit_contributions = checked_add(
                                structural.raw_audit_contributions,
                                1,
                                "raw-audit contributions",
                            )?;
                        }
                    }
                }
            }

            for (column, coefficient) in primary_genuine {
                let normalized = arithmetic.apply_mass_power(
                    &coefficient,
                    seed_mass_weight.checked_sub(column.mass_weight()).ok_or(
                        FourLoopNextClosedRowsError::ArithmeticOverflow {
                            resource: "collected genuine mass exponent",
                        },
                    )?,
                )?;
                arithmetic.check_mass_free(&normalized, source_row.raw_id(), &column)?;
                arithmetic.add_sparse(&mut primary, column, normalized)?;
                structural.primary_contributions =
                    checked_add(structural.primary_contributions, 1, "primary contributions")?;
            }
            let mut boundary_group_indices = Vec::new();
            for &group_index in &groups_by_row[row_index] {
                let group = &boundary_groups[group_index];
                boundary_group_indices.push(checked_u32(group_index, "boundary group index")?);
                let replayed_sum = replayed_boundary_sums
                    .remove(&group.leaf_id)
                    .unwrap_or_else(|| coefficient_context.zero());
                if replayed_sum != group.collected_coefficient {
                    return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "row boundary group coefficient",
                    });
                }
                if group.canceled {
                    continue;
                }
                let binding = plan_bindings.get(group.plan_binding_index as usize).ok_or(
                    FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "boundary group plan binding",
                    },
                )?;
                let (_, normalized_closure) = closure_combinations(binding, t1s2, three_loop)?;
                for (product, closure_coefficient) in normalized_closure.terms() {
                    let column = FourLoopCornerColumnId::Product(product.clone());
                    let contribution = arithmetic
                        .multiply(&group.seed_to_boundary_coefficient, closure_coefficient)?;
                    arithmetic.check_mass_free(&contribution, source_row.raw_id(), &column)?;
                    arithmetic.add_sparse(&mut primary, column, contribution)?;
                    structural.primary_contributions =
                        checked_add(structural.primary_contributions, 1, "primary contributions")?;
                }
            }
            if !replayed_boundary_sums.is_empty() {
                return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "unconsumed boundary replay groups",
                });
            }
            if primary != raw_audit {
                return Err(FourLoopNextClosedRowsError::RowAssemblyMismatch { row_index });
            }

            if primary.len() > config.max_row_width {
                return Err(FourLoopNextClosedRowsError::ResourceLimit {
                    resource: "closed row width",
                    requested: primary.len() as u128,
                    limit: config.max_row_width as u128,
                });
            }
            stats.max_row_width = stats.max_row_width.max(primary.len());
            stats.collected_entries = checked_add(
                stats.collected_entries,
                primary.len(),
                "collected row entries",
            )?;
            if stats.collected_entries > config.max_collected_entries {
                return Err(FourLoopNextClosedRowsError::ResourceLimit {
                    resource: "collected row entries",
                    requested: stats.collected_entries as u128,
                    limit: config.max_collected_entries as u128,
                });
            }

            let (row_scale, pivot_column_index) = if let Some((pivot, scale)) = primary
                .last_key_value()
                .map(|(key, value)| (key.clone(), value.clone()))
            {
                for coefficient in primary.values_mut() {
                    *coefficient = arithmetic.divide(coefficient, &scale)?;
                    arithmetic.check_mass_free(coefficient, source_row.raw_id(), &pivot)?;
                }
                let pivot_index = *column_indices.get(&pivot).ok_or(
                    FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "pivot column index",
                    },
                )?;
                if primary.get(&pivot) != Some(&coefficient_context.one()) {
                    return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "unit canonical pivot",
                    });
                }
                (scale, Some(checked_u32(pivot_index, "pivot column index")?))
            } else {
                stats.zero_rows = checked_add(stats.zero_rows, 1, "zero rows")?;
                (coefficient_context.one(), None)
            };
            retained_coefficients.charge(&row_scale)?;
            for coefficient in primary.values() {
                retained_coefficients.charge(coefficient)?;
            }
            rows.push(FourLoopNextClosedRow {
                raw_id: source_row.raw_id(),
                seed_mass_weight,
                path_dispositions,
                boundary_group_indices,
                row_scale,
                pivot_column_index,
                entries: primary,
            });
        }

        stats.primary_contributions = structural.primary_contributions;
        stats.raw_audit_contributions = structural.raw_audit_contributions;
        stats.mass_power_steps = arithmetic.mass_power_steps;
        stats.coefficient_multiplications = arithmetic.multiplications;
        stats.coefficient_additions = arithmetic.additions;
        stats.coefficient_divisions = arithmetic.divisions;
        stats.retained_coefficient_terms = retained_coefficients.terms;
        stats.retained_coefficient_bytes = retained_coefficients.bytes;
        check_exact_stats(stats)?;
        check_actual_stats(stats, config)?;

        let checksum = closed_rows_checksum(
            inventory,
            transport,
            t1s2,
            three_loop,
            config,
            &coefficient_context,
            &plan_bindings,
            &occurrence_bindings,
            &boundary_groups,
            &columns,
            &rows,
            stats,
        );
        Ok(Self {
            inventory,
            transport,
            t1s2,
            three_loop,
            config,
            coefficient_context,
            plan_bindings,
            occurrence_bindings,
            boundary_groups,
            columns,
            rows,
            stats,
            checksum,
        })
    }
}

#[derive(Debug)]
pub enum FourLoopNextClosedRowsError {
    Inventory(FourLoopNextInventoryError),
    SourceIdentityMismatch {
        source: &'static str,
    },
    CoefficientContextMismatch,
    CensusMismatch {
        resource: &'static str,
        expected: usize,
        actual: usize,
    },
    DuplicateBinding {
        resource: &'static str,
        leaf_id: u32,
    },
    UnsupportedProduct {
        product: MasterProduct<MassiveVacuumMaster>,
    },
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    AllocationFailed {
        resource: &'static str,
        requested: usize,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
    ZeroPivotCoefficient,
    ResidualMassDependence {
        raw_id: FourLoopNextRawRowId,
        column: FourLoopCornerColumnId,
        numerator_degree: u128,
        denominator_degree: u128,
    },
    ResidualBoundaryBridgeMassDependence {
        raw_id: FourLoopNextRawRowId,
        leaf_id: u32,
        numerator_degree: u128,
        denominator_degree: u128,
    },
    RowAssemblyMismatch {
        row_index: usize,
    },
    ReplayMismatch {
        stage: &'static str,
    },
}

impl From<FourLoopNextInventoryError> for FourLoopNextClosedRowsError {
    fn from(error: FourLoopNextInventoryError) -> Self {
        Self::Inventory(error)
    }
}

impl fmt::Display for FourLoopNextClosedRowsError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "four-loop next closed rows: {self:?}")
    }
}

impl Error for FourLoopNextClosedRowsError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Inventory(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Default)]
struct StructuralStats {
    paths: usize,
    boundary_paths: usize,
    genuine_paths: usize,
    scaleless_paths: usize,
    nonzero_boundary_groups: usize,
    canceled_boundary_groups: usize,
    repeated_boundary_groups: usize,
    repeated_surviving_boundary_groups: usize,
    repeated_canceled_boundary_groups: usize,
    nonzero_boundary_contributors: usize,
    genuine_groups: usize,
    max_row_paths: usize,
    max_row_boundary_groups: usize,
    primary_contributions: usize,
    raw_audit_contributions: usize,
}

fn preflight_config(
    config: FourLoopNextClosedRowsConfig,
) -> Result<(), FourLoopNextClosedRowsError> {
    for (resource, required, available) in [
        (
            "closed parent rows",
            FOUR_LOOP_NEXT_CLOSED_ROWS,
            config.max_rows,
        ),
        (
            "path dispositions",
            FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS,
            config.max_paths,
        ),
        (
            "plan bindings",
            FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS,
            config.max_plan_bindings,
        ),
        (
            "occurrence bindings",
            FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
            config.max_occurrence_bindings,
        ),
        (
            "raw boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS,
            config.max_boundary_groups,
        ),
        (
            "boundary group contributors",
            FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
            config.max_boundary_group_contributors,
        ),
        (
            "genuine row groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_GROUPS,
            config.max_genuine_groups,
        ),
        (
            "global columns",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND,
            config.max_global_columns,
        ),
        (
            "primary contributions",
            FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTION_BOUND,
            config.max_primary_contributions,
        ),
        (
            "raw-audit contributions",
            FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTION_BOUND,
            config.max_raw_audit_contributions,
        ),
        (
            "collected row entries",
            FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRY_BOUND,
            config.max_collected_entries,
        ),
        (
            "closed row width",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND,
            config.max_row_width,
        ),
    ] {
        if available < required {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource,
                requested: required as u128,
                limit: available as u128,
            });
        }
    }
    if config.max_coefficient_degree as u128 > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "configured coefficient exponent degree",
            requested: config.max_coefficient_degree as u128,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }
    Ok(())
}

fn authenticate_sources(
    inventory: &FourLoopNextInventory,
    transport: &FourLoopComponentTransport<'_>,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
) -> Result<(), FourLoopNextClosedRowsError> {
    if !std::ptr::eq(transport.inventory(), inventory) {
        return Err(FourLoopNextClosedRowsError::SourceIdentityMismatch {
            source: "component transport inventory",
        });
    }
    if !std::ptr::eq(t1s2.transport(), transport) {
        return Err(FourLoopNextClosedRowsError::SourceIdentityMismatch {
            source: "T1/S2 closure transport",
        });
    }
    if !std::ptr::eq(three_loop.transport(), transport) {
        return Err(FourLoopNextClosedRowsError::SourceIdentityMismatch {
            source: "three-loop closure transport",
        });
    }
    if !t1s2
        .coefficient_context()
        .has_same_variable_map(three_loop.coefficient_context())
        || t1s2.coefficient_context().parameter_names() != ["d", "m2"]
    {
        return Err(FourLoopNextClosedRowsError::CoefficientContextMismatch);
    }
    Ok(())
}

fn source_plan_binding_at(
    index: usize,
    transport: &FourLoopComponentTransport<'_>,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
    config: FourLoopNextClosedRowsConfig,
) -> Result<FourLoopNextPlanBinding, FourLoopNextClosedRowsError> {
    if transport.plans().len() > config.max_plan_bindings {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "plan bindings",
            requested: transport.plans().len() as u128,
            limit: config.max_plan_bindings as u128,
        });
    }
    let source =
        transport
            .plans()
            .get(index)
            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "plan binding candidate index",
            })?;
    let leaf_id = source.leaf_id();
    let mut t1s2_index = None;
    for (closure_index, plan) in t1s2.plans().iter().enumerate() {
        if plan.leaf_id() == leaf_id {
            if t1s2_index.replace(closure_index).is_some() {
                return Err(FourLoopNextClosedRowsError::DuplicateBinding {
                    resource: "T1/S2 plan leaf",
                    leaf_id,
                });
            }
            authenticate_plan_products(plan.ordinary(), plan.mass_normalized())?;
        }
    }
    let mut three_loop_index = None;
    for (closure_index, plan) in three_loop.plans().iter().enumerate() {
        if plan.leaf_id() == leaf_id {
            if three_loop_index.replace(closure_index).is_some() {
                return Err(FourLoopNextClosedRowsError::DuplicateBinding {
                    resource: "three-loop plan leaf",
                    leaf_id,
                });
            }
            authenticate_plan_products(plan.ordinary(), plan.mass_normalized())?;
        }
    }
    let (slice, closure_plan_index) = match (t1s2_index, three_loop_index) {
        (Some(closure_index), None) => (FourLoopNextClosureSlice::T1S2, closure_index),
        (None, Some(closure_index)) => (FourLoopNextClosureSlice::ThreeLoop, closure_index),
        _ => {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "plan binding candidate complementary source",
            });
        }
    };
    Ok(FourLoopNextPlanBinding {
        leaf_id,
        transport_plan_index: checked_u32(index, "transport plan index")?,
        slice,
        closure_plan_index: checked_u16(closure_plan_index, "closure plan index")?,
    })
}

fn source_boundary_leaf_ids(
    inventory: &FourLoopNextInventory,
    row_index: usize,
) -> Result<Vec<u32>, FourLoopNextClosedRowsError> {
    let row =
        inventory
            .rows()
            .get(row_index)
            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "source boundary leaf row index",
            })?;
    let mut leaf_ids = BTreeSet::new();
    for path in row.paths() {
        let leaf_id = path.leaf_id();
        let leaf = inventory.leaves().get(leaf_id as usize).ok_or(
            FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "source boundary leaf index",
            },
        )?;
        if matches!(leaf, FourLoopNextLeaf::Boundary(_)) {
            leaf_ids.insert(leaf_id);
        }
    }
    Ok(leaf_ids.into_iter().collect())
}

fn source_boundary_group_key_at(
    index: usize,
    inventory: &FourLoopNextInventory,
    config: FourLoopNextClosedRowsConfig,
) -> Result<(usize, u32), FourLoopNextClosedRowsError> {
    let mut group_count = 0_usize;
    let mut path_count = 0_usize;
    for (row_index, row) in inventory.rows().iter().enumerate() {
        path_count = checked_add(path_count, row.paths().len(), "candidate source paths")?;
        if path_count > config.max_paths {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource: "candidate source paths",
                requested: path_count as u128,
                limit: config.max_paths as u128,
            });
        }
        let leaf_ids = source_boundary_leaf_ids(inventory, row_index)?;
        for leaf_id in leaf_ids {
            if group_count == index {
                return Ok((row_index, leaf_id));
            }
            group_count = checked_add(group_count, 1, "candidate boundary groups")?;
            if group_count > config.max_boundary_groups {
                return Err(FourLoopNextClosedRowsError::ResourceLimit {
                    resource: "candidate boundary groups",
                    requested: group_count as u128,
                    limit: config.max_boundary_groups as u128,
                });
            }
        }
    }
    Err(FourLoopNextClosedRowsError::ReplayMismatch {
        stage: "boundary group candidate index",
    })
}

fn source_boundary_group_index(
    target_row_index: usize,
    target_leaf_id: u32,
    inventory: &FourLoopNextInventory,
    config: FourLoopNextClosedRowsConfig,
) -> Result<usize, FourLoopNextClosedRowsError> {
    if target_row_index >= inventory.rows().len() {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "boundary group source row index",
        });
    }
    let mut group_count = 0_usize;
    let mut path_count = 0_usize;
    for (row_index, row) in inventory
        .rows()
        .iter()
        .enumerate()
        .take(target_row_index + 1)
    {
        path_count = checked_add(path_count, row.paths().len(), "candidate source paths")?;
        if path_count > config.max_paths {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource: "candidate source paths",
                requested: path_count as u128,
                limit: config.max_paths as u128,
            });
        }
        for leaf_id in source_boundary_leaf_ids(inventory, row_index)? {
            if row_index == target_row_index && leaf_id == target_leaf_id {
                return Ok(group_count);
            }
            group_count = checked_add(group_count, 1, "candidate boundary groups")?;
            if group_count > config.max_boundary_groups {
                return Err(FourLoopNextClosedRowsError::ResourceLimit {
                    resource: "candidate boundary groups",
                    requested: group_count as u128,
                    limit: config.max_boundary_groups as u128,
                });
            }
        }
    }
    Err(FourLoopNextClosedRowsError::ReplayMismatch {
        stage: "boundary group source coordinate",
    })
}

fn source_occurrence_binding_at(
    index: usize,
    inventory: &FourLoopNextInventory,
    transport: &FourLoopComponentTransport<'_>,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
    config: FourLoopNextClosedRowsConfig,
) -> Result<FourLoopNextOccurrenceBinding, FourLoopNextClosedRowsError> {
    if transport.occurrences().len() != t1s2.occurrences().len()
        || transport.occurrences().len() != three_loop.occurrences().len()
    {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate closure occurrence vector lengths",
        });
    }
    if transport.occurrences().len() > config.max_occurrence_bindings {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "occurrence bindings",
            requested: transport.occurrences().len() as u128,
            limit: config.max_occurrence_bindings as u128,
        });
    }
    let source =
        transport
            .occurrences()
            .get(index)
            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence binding candidate index",
            })?;
    let t1s2_occurrence =
        t1s2.occurrences()
            .get(index)
            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "T1/S2 candidate occurrence index",
            })?;
    let three_loop_occurrence =
        three_loop
            .occurrences()
            .get(index)
            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "three-loop candidate occurrence index",
            })?;
    if source.row_index() != t1s2_occurrence.row_index()
        || source.path_index() != t1s2_occurrence.path_index()
        || source.leaf_id() != t1s2_occurrence.leaf_id()
        || source.row_index() != three_loop_occurrence.row_index()
        || source.path_index() != three_loop_occurrence.path_index()
        || source.leaf_id() != three_loop_occurrence.leaf_id()
    {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate zipped occurrence coordinates",
        });
    }
    let source_plan = transport.plans().get(source.plan_index() as usize).ok_or(
        FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate occurrence transport plan index",
        },
    )?;
    if source_plan.leaf_id() != source.leaf_id() {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate occurrence transport plan leaf",
        });
    }
    let plan_binding = source_plan_binding_at(
        source.plan_index() as usize,
        transport,
        t1s2,
        three_loop,
        config,
    )?;
    let (slice, closure_plan_index) = match (
        t1s2_occurrence.completed_plan_index(),
        three_loop_occurrence.completed_plan_index(),
    ) {
        (Some(completed), None) => (FourLoopNextClosureSlice::T1S2, completed),
        (None, Some(completed)) => (FourLoopNextClosureSlice::ThreeLoop, completed),
        _ => {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "candidate complementary occurrence partition",
            });
        }
    };
    if plan_binding.leaf_id != source.leaf_id()
        || plan_binding.slice != slice
        || plan_binding.closure_plan_index != closure_plan_index
    {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate occurrence closure plan",
        });
    }
    let row = inventory.rows().get(source.row_index() as usize).ok_or(
        FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate occurrence row index",
        },
    )?;
    let compact = row.paths().get(source.path_index() as usize).ok_or(
        FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate occurrence path index",
        },
    )?;
    if compact.leaf_id() != source.leaf_id()
        || !matches!(
            inventory.leaves().get(source.leaf_id() as usize),
            Some(FourLoopNextLeaf::Boundary(_))
        )
    {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate occurrence inventory boundary path",
        });
    }
    let boundary_group_index = source_boundary_group_index(
        source.row_index() as usize,
        source.leaf_id(),
        inventory,
        config,
    )?;
    Ok(FourLoopNextOccurrenceBinding {
        row_index: source.row_index(),
        path_index: source.path_index(),
        leaf_id: source.leaf_id(),
        transport_plan_index: source.plan_index(),
        plan_binding_index: source.plan_index(),
        boundary_group_index: checked_u32(boundary_group_index, "boundary group index")?,
        slice,
        closure_occurrence_index: checked_u32(index, "closure occurrence index")?,
    })
}

fn source_transport_plan_index_for_leaf(
    leaf_id: u32,
    transport: &FourLoopComponentTransport<'_>,
) -> Result<usize, FourLoopNextClosedRowsError> {
    let mut found = None;
    for (index, plan) in transport.plans().iter().enumerate() {
        if plan.leaf_id() == leaf_id {
            if found.replace(index).is_some() {
                return Err(FourLoopNextClosedRowsError::DuplicateBinding {
                    resource: "transport plan leaf",
                    leaf_id,
                });
            }
        }
    }
    found.ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
        stage: "candidate boundary group transport plan",
    })
}

#[allow(clippy::too_many_arguments)]
fn source_boundary_group_for_key(
    row_index: usize,
    leaf_id: u32,
    inventory: &FourLoopNextInventory,
    transport: &FourLoopComponentTransport<'_>,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
    replayed_paths: &[FourLoopNextReplayedPath],
    arithmetic: &mut CheckedArithmetic,
    config: FourLoopNextClosedRowsConfig,
) -> Result<FourLoopNextBoundaryGroup, FourLoopNextClosedRowsError> {
    let row =
        inventory
            .rows()
            .get(row_index)
            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "candidate boundary group row index",
            })?;
    if replayed_paths.len() != row.paths().len() {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate boundary group replay length",
        });
    }
    if !matches!(
        inventory.leaves().get(leaf_id as usize),
        Some(FourLoopNextLeaf::Boundary(_))
    ) {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate boundary group leaf",
        });
    }
    let mut contributor_path_indices = Vec::new();
    contributor_path_indices
        .try_reserve_exact(
            row.paths()
                .len()
                .min(config.max_boundary_group_contributors),
        )
        .map_err(|_| FourLoopNextClosedRowsError::AllocationFailed {
            resource: "candidate boundary group contributors",
            requested: row
                .paths()
                .len()
                .min(config.max_boundary_group_contributors),
        })?;
    let mut replayed_sum = None::<Coefficient>;
    for (path_index, (compact, replayed)) in
        row.paths().iter().copied().zip(replayed_paths).enumerate()
    {
        if replayed.leaf_id() != compact.leaf_id()
            || inventory.leaves().get(compact.leaf_id() as usize) != Some(replayed.leaf())
        {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "candidate boundary group replayed leaf",
            });
        }
        if compact.leaf_id() == leaf_id {
            if contributor_path_indices.len() >= config.max_boundary_group_contributors {
                return Err(FourLoopNextClosedRowsError::ResourceLimit {
                    resource: "candidate boundary group contributors",
                    requested: contributor_path_indices.len() as u128 + 1,
                    limit: config.max_boundary_group_contributors as u128,
                });
            }
            contributor_path_indices.push(checked_u32(path_index, "path index")?);
            replayed_sum = Some(match replayed_sum {
                Some(current) => arithmetic.add(&current, replayed.final_coefficient())?,
                None => {
                    arithmetic.check_existing(replayed.final_coefficient())?;
                    replayed.final_coefficient().clone()
                }
            });
        }
    }
    let collected_coefficient =
        replayed_sum.ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate boundary group empty source",
        })?;
    let mut retained = row
        .collected_boundaries()
        .iter()
        .filter(|entry| entry.leaf_id() == leaf_id);
    let retained_entry = retained.next();
    if retained.next().is_some() {
        return Err(FourLoopNextClosedRowsError::DuplicateBinding {
            resource: "retained boundary group leaf",
            leaf_id,
        });
    }
    let canceled = collected_coefficient.is_zero();
    match (canceled, retained_entry) {
        (true, None) => {}
        (false, Some(entry))
            if entry.coefficient() == &collected_coefficient
                && entry.contributor_path_indices() == contributor_path_indices => {}
        _ => {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "candidate replayed boundary collection",
            });
        }
    }
    let plan_binding_index = source_transport_plan_index_for_leaf(leaf_id, transport)?;
    let plan_binding =
        source_plan_binding_at(plan_binding_index, transport, t1s2, three_loop, config)?;
    if plan_binding.leaf_id != leaf_id {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate boundary group plan leaf",
        });
    }
    let seed_mass_weight = powers_weight(row.raw_id().seed().powers())?;
    let boundary_mass_weight = powers_weight(inventory.boundary_key(leaf_id)?.powers())?;
    let mass_bridge_exponent = seed_mass_weight.checked_sub(boundary_mass_weight).ok_or(
        FourLoopNextClosedRowsError::ArithmeticOverflow {
            resource: "candidate boundary mass bridge exponent",
        },
    )?;
    let seed_to_boundary_coefficient =
        arithmetic.apply_mass_power(&collected_coefficient, mass_bridge_exponent)?;
    arithmetic.check_mass_free_bridge(&seed_to_boundary_coefficient, row.raw_id(), leaf_id)?;
    Ok(FourLoopNextBoundaryGroup {
        row_index: checked_u16(row_index, "row index")?,
        leaf_id,
        plan_binding_index: checked_u32(plan_binding_index, "plan binding index")?,
        contributor_path_indices,
        collected_coefficient,
        seed_mass_weight,
        boundary_mass_weight,
        mass_bridge_exponent,
        seed_to_boundary_coefficient,
        canceled,
    })
}

fn source_boundary_group_at(
    index: usize,
    inventory: &FourLoopNextInventory,
    transport: &FourLoopComponentTransport<'_>,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
    config: FourLoopNextClosedRowsConfig,
) -> Result<FourLoopNextBoundaryGroup, FourLoopNextClosedRowsError> {
    let (row_index, leaf_id) = source_boundary_group_key_at(index, inventory, config)?;
    let replayed_paths = inventory.replay_row_paths(row_index)?;
    let mut arithmetic = CheckedArithmetic::new(t1s2.coefficient_context().clone(), config)?;
    let group = source_boundary_group_for_key(
        row_index,
        leaf_id,
        inventory,
        transport,
        t1s2,
        three_loop,
        &replayed_paths,
        &mut arithmetic,
        config,
    )?;
    let mut retained_coefficients = RetainedCoefficientCharge::new(config);
    retained_coefficients.charge(&group.collected_coefficient)?;
    retained_coefficients.charge(&group.seed_to_boundary_coefficient)?;
    Ok(group)
}

#[allow(clippy::too_many_arguments)]
fn source_closed_row_at(
    row_index: usize,
    inventory: &FourLoopNextInventory,
    transport: &FourLoopComponentTransport<'_>,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
    config: FourLoopNextClosedRowsConfig,
) -> Result<FourLoopNextClosedRow, FourLoopNextClosedRowsError> {
    if inventory.rows().len() > config.max_rows {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "candidate source rows",
            requested: inventory.rows().len() as u128,
            limit: config.max_rows as u128,
        });
    }
    let source_row =
        inventory
            .rows()
            .get(row_index)
            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "closed row candidate index",
            })?;
    if source_row.paths().len() > config.max_paths {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "candidate row paths",
            requested: source_row.paths().len() as u128,
            limit: config.max_paths as u128,
        });
    }
    let coefficient_context = t1s2.coefficient_context().clone();
    let mut arithmetic = CheckedArithmetic::new(coefficient_context.clone(), config)?;
    let mut retained_coefficients = RetainedCoefficientCharge::new(config);
    let replayed_paths = inventory.replay_row_paths(row_index)?;
    if replayed_paths.len() != source_row.paths().len() {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate closed row replay length",
        });
    }

    let columns = build_columns(inventory, t1s2, three_loop, config)?;
    let column_indices = columns
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, column)| (column, index))
        .collect::<BTreeMap<_, _>>();
    let row_index_u16 = checked_u16(row_index, "row index")?;
    let boundary_leaf_ids = source_boundary_leaf_ids(inventory, row_index)?;
    let mut boundary_groups = BTreeMap::<u32, (usize, FourLoopNextBoundaryGroup)>::new();
    for leaf_id in boundary_leaf_ids {
        let group_index = source_boundary_group_index(row_index, leaf_id, inventory, config)?;
        let group = source_boundary_group_for_key(
            row_index,
            leaf_id,
            inventory,
            transport,
            t1s2,
            three_loop,
            &replayed_paths,
            &mut arithmetic,
            config,
        )?;
        retained_coefficients.charge(&group.collected_coefficient)?;
        retained_coefficients.charge(&group.seed_to_boundary_coefficient)?;
        if boundary_groups
            .insert(leaf_id, (group_index, group))
            .is_some()
        {
            return Err(FourLoopNextClosedRowsError::DuplicateBinding {
                resource: "candidate row boundary leaf",
                leaf_id,
            });
        }
    }
    for retained in source_row.collected_boundaries() {
        match boundary_groups.get(&retained.leaf_id()) {
            Some((_, group))
                if !group.canceled
                    && &group.collected_coefficient == retained.coefficient()
                    && group.contributor_path_indices.as_slice()
                        == retained.contributor_path_indices() => {}
            _ => {
                return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "candidate row retained boundary coverage",
                });
            }
        }
    }

    let mut occurrence_by_coordinate =
        BTreeMap::<(u16, u32), (usize, FourLoopNextOccurrenceBinding)>::new();
    for (occurrence_index, source) in transport.occurrences().iter().enumerate() {
        if source.row_index() != row_index_u16 {
            continue;
        }
        let binding = source_occurrence_binding_at(
            occurrence_index,
            inventory,
            transport,
            t1s2,
            three_loop,
            config,
        )?;
        if occurrence_by_coordinate
            .insert(
                (binding.row_index, binding.path_index),
                (occurrence_index, binding),
            )
            .is_some()
        {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "candidate row duplicate occurrence coordinate",
            });
        }
    }

    let seed_mass_weight = powers_weight(source_row.raw_id().seed().powers())?;
    let mut primary_genuine = BTreeMap::<FourLoopCornerColumnId, Coefficient>::new();
    let mut replayed_boundary_sums = BTreeMap::<u32, Coefficient>::new();
    let mut primary = BTreeMap::<FourLoopCornerColumnId, Coefficient>::new();
    let mut raw_audit = BTreeMap::<FourLoopCornerColumnId, Coefficient>::new();
    let mut path_dispositions = Vec::new();
    path_dispositions
        .try_reserve_exact(source_row.paths().len())
        .map_err(|_| FourLoopNextClosedRowsError::AllocationFailed {
            resource: "candidate row path dispositions",
            requested: source_row.paths().len(),
        })?;
    let mut primary_contributions = 0_usize;
    let mut raw_audit_contributions = 0_usize;

    for (path_index, (compact, replayed)) in source_row
        .paths()
        .iter()
        .copied()
        .zip(&replayed_paths)
        .enumerate()
    {
        if replayed.leaf_id() != compact.leaf_id()
            || inventory.leaves().get(compact.leaf_id() as usize) != Some(replayed.leaf())
        {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "candidate row inventory path leaf",
            });
        }
        let leaf_id = compact.leaf_id();
        match replayed.leaf() {
            FourLoopNextLeaf::FamilyScaleless { .. } => {
                path_dispositions.push(FourLoopNextPathDisposition::FamilyScaleless { leaf_id });
            }
            FourLoopNextLeaf::ScalarCornerScaleless { .. } => {
                path_dispositions
                    .push(FourLoopNextPathDisposition::ScalarCornerScaleless { leaf_id });
            }
            FourLoopNextLeaf::Genuine(genuine) => {
                inventory.authenticate_genuine_column(genuine)?;
                let column = genuine_column_id(genuine);
                let column_index = *column_indices.get(&column).ok_or(
                    FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "candidate row genuine column index",
                    },
                )?;
                path_dispositions.push(FourLoopNextPathDisposition::Genuine {
                    leaf_id,
                    column_index: checked_u32(column_index, "genuine column index")?,
                });
                arithmetic.add_sparse(
                    &mut primary_genuine,
                    column.clone(),
                    replayed.final_coefficient().clone(),
                )?;
                let exponent = seed_mass_weight.checked_sub(column.mass_weight()).ok_or(
                    FourLoopNextClosedRowsError::ArithmeticOverflow {
                        resource: "candidate raw genuine mass exponent",
                    },
                )?;
                let normalized =
                    arithmetic.apply_mass_power(replayed.final_coefficient(), exponent)?;
                arithmetic.check_mass_free(&normalized, source_row.raw_id(), &column)?;
                arithmetic.add_sparse(&mut raw_audit, column, normalized)?;
                raw_audit_contributions = charge_candidate_count(
                    raw_audit_contributions,
                    config.max_raw_audit_contributions,
                    "candidate raw-audit contributions",
                )?;
            }
            FourLoopNextLeaf::Boundary(_) => {
                let coordinate = (row_index_u16, checked_u32(path_index, "path index")?);
                let (occurrence_index, occurrence) =
                    occurrence_by_coordinate.remove(&coordinate).ok_or(
                        FourLoopNextClosedRowsError::ReplayMismatch {
                            stage: "candidate row boundary occurrence coordinate",
                        },
                    )?;
                if occurrence.leaf_id != leaf_id {
                    return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "candidate row boundary occurrence leaf",
                    });
                }
                let (expected_group_index, _) = boundary_groups.get(&leaf_id).ok_or(
                    FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "candidate row boundary group coordinate",
                    },
                )?;
                if occurrence.boundary_group_index as usize != *expected_group_index {
                    return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "candidate row occurrence boundary group",
                    });
                }
                path_dispositions.push(FourLoopNextPathDisposition::Boundary {
                    leaf_id,
                    occurrence_binding_index: checked_u32(
                        occurrence_index,
                        "occurrence binding index",
                    )?,
                    boundary_group_index: occurrence.boundary_group_index,
                });
                arithmetic.add_sparse(
                    &mut replayed_boundary_sums,
                    leaf_id,
                    replayed.final_coefficient().clone(),
                )?;
                let plan_binding = source_plan_binding_at(
                    occurrence.plan_binding_index as usize,
                    transport,
                    t1s2,
                    three_loop,
                    config,
                )?;
                let (ordinary, _) = closure_combinations(&plan_binding, t1s2, three_loop)?;
                for (product, closure_coefficient) in ordinary.terms() {
                    let column = FourLoopCornerColumnId::Product(product.clone());
                    let multiplied =
                        arithmetic.multiply(replayed.final_coefficient(), closure_coefficient)?;
                    let exponent = seed_mass_weight.checked_sub(column.mass_weight()).ok_or(
                        FourLoopNextClosedRowsError::ArithmeticOverflow {
                            resource: "candidate raw boundary mass exponent",
                        },
                    )?;
                    let normalized = arithmetic.apply_mass_power(&multiplied, exponent)?;
                    arithmetic.check_mass_free(&normalized, source_row.raw_id(), &column)?;
                    arithmetic.add_sparse(&mut raw_audit, column, normalized)?;
                    raw_audit_contributions = charge_candidate_count(
                        raw_audit_contributions,
                        config.max_raw_audit_contributions,
                        "candidate raw-audit contributions",
                    )?;
                }
            }
        }
    }
    if !occurrence_by_coordinate.is_empty() {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate row unconsumed source occurrences",
        });
    }

    for (column, coefficient) in primary_genuine {
        let exponent = seed_mass_weight.checked_sub(column.mass_weight()).ok_or(
            FourLoopNextClosedRowsError::ArithmeticOverflow {
                resource: "candidate collected genuine mass exponent",
            },
        )?;
        let normalized = arithmetic.apply_mass_power(&coefficient, exponent)?;
        arithmetic.check_mass_free(&normalized, source_row.raw_id(), &column)?;
        arithmetic.add_sparse(&mut primary, column, normalized)?;
        primary_contributions = charge_candidate_count(
            primary_contributions,
            config.max_primary_contributions,
            "candidate primary contributions",
        )?;
    }
    let mut boundary_group_indices = Vec::new();
    boundary_group_indices
        .try_reserve_exact(boundary_groups.len())
        .map_err(|_| FourLoopNextClosedRowsError::AllocationFailed {
            resource: "candidate row boundary group indices",
            requested: boundary_groups.len(),
        })?;
    for (leaf_id, (group_index, group)) in &boundary_groups {
        boundary_group_indices.push(checked_u32(*group_index, "boundary group index")?);
        let replayed_sum = replayed_boundary_sums
            .remove(leaf_id)
            .unwrap_or_else(|| coefficient_context.zero());
        if replayed_sum != group.collected_coefficient {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "candidate row boundary group coefficient",
            });
        }
        if group.canceled {
            continue;
        }
        let plan_binding = source_plan_binding_at(
            group.plan_binding_index as usize,
            transport,
            t1s2,
            three_loop,
            config,
        )?;
        let (_, normalized_closure) = closure_combinations(&plan_binding, t1s2, three_loop)?;
        for (product, closure_coefficient) in normalized_closure.terms() {
            let column = FourLoopCornerColumnId::Product(product.clone());
            let contribution =
                arithmetic.multiply(&group.seed_to_boundary_coefficient, closure_coefficient)?;
            arithmetic.check_mass_free(&contribution, source_row.raw_id(), &column)?;
            arithmetic.add_sparse(&mut primary, column, contribution)?;
            primary_contributions = charge_candidate_count(
                primary_contributions,
                config.max_primary_contributions,
                "candidate primary contributions",
            )?;
        }
    }
    if !replayed_boundary_sums.is_empty() {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "candidate row unconsumed boundary replay groups",
        });
    }
    if primary != raw_audit {
        return Err(FourLoopNextClosedRowsError::RowAssemblyMismatch { row_index });
    }
    if primary.len() > config.max_row_width {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "candidate closed row width",
            requested: primary.len() as u128,
            limit: config.max_row_width as u128,
        });
    }
    if primary.len() > config.max_collected_entries {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "candidate collected row entries",
            requested: primary.len() as u128,
            limit: config.max_collected_entries as u128,
        });
    }

    let (row_scale, pivot_column_index) = if let Some((pivot, scale)) = primary
        .last_key_value()
        .map(|(column, coefficient)| (column.clone(), coefficient.clone()))
    {
        for coefficient in primary.values_mut() {
            *coefficient = arithmetic.divide(coefficient, &scale)?;
            arithmetic.check_mass_free(coefficient, source_row.raw_id(), &pivot)?;
        }
        let pivot_index =
            *column_indices
                .get(&pivot)
                .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "candidate row pivot column index",
                })?;
        if primary.get(&pivot) != Some(&coefficient_context.one()) {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "candidate row unit canonical pivot",
            });
        }
        (scale, Some(checked_u32(pivot_index, "pivot column index")?))
    } else {
        (coefficient_context.one(), None)
    };

    retained_coefficients.charge(&row_scale)?;
    for coefficient in primary.values() {
        retained_coefficients.charge(coefficient)?;
    }
    Ok(FourLoopNextClosedRow {
        raw_id: source_row.raw_id(),
        seed_mass_weight,
        path_dispositions,
        boundary_group_indices,
        row_scale,
        pivot_column_index,
        entries: primary,
    })
}

fn charge_candidate_count(
    current: usize,
    limit: usize,
    resource: &'static str,
) -> Result<usize, FourLoopNextClosedRowsError> {
    let requested = checked_add(current, 1, resource)?;
    if requested > limit {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource,
            requested: requested as u128,
            limit: limit as u128,
        });
    }
    Ok(requested)
}

fn build_plan_bindings(
    transport: &FourLoopComponentTransport<'_>,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
    config: FourLoopNextClosedRowsConfig,
) -> Result<Vec<FourLoopNextPlanBinding>, FourLoopNextClosedRowsError> {
    let mut t1s2_by_leaf = BTreeMap::new();
    for (index, plan) in t1s2.plans().iter().enumerate() {
        if t1s2_by_leaf.insert(plan.leaf_id(), index).is_some() {
            return Err(FourLoopNextClosedRowsError::DuplicateBinding {
                resource: "T1/S2 plan leaf",
                leaf_id: plan.leaf_id(),
            });
        }
        authenticate_plan_products(plan.ordinary(), plan.mass_normalized())?;
    }
    let mut three_by_leaf = BTreeMap::new();
    for (index, plan) in three_loop.plans().iter().enumerate() {
        if three_by_leaf.insert(plan.leaf_id(), index).is_some() {
            return Err(FourLoopNextClosedRowsError::DuplicateBinding {
                resource: "three-loop plan leaf",
                leaf_id: plan.leaf_id(),
            });
        }
        authenticate_plan_products(plan.ordinary(), plan.mass_normalized())?;
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(transport.plans().len())
        .map_err(|_| FourLoopNextClosedRowsError::AllocationFailed {
            resource: "plan bindings",
            requested: transport.plans().len(),
        })?;
    let mut transport_leaves = BTreeSet::new();
    for (transport_index, plan) in transport.plans().iter().enumerate() {
        if !transport_leaves.insert(plan.leaf_id()) {
            return Err(FourLoopNextClosedRowsError::DuplicateBinding {
                resource: "transport plan leaf",
                leaf_id: plan.leaf_id(),
            });
        }
        let t1 = t1s2_by_leaf.get(&plan.leaf_id()).copied();
        let three = three_by_leaf.get(&plan.leaf_id()).copied();
        let (slice, closure_index) = match (t1, three) {
            (Some(index), None) => (FourLoopNextClosureSlice::T1S2, index),
            (None, Some(index)) => (FourLoopNextClosureSlice::ThreeLoop, index),
            _ => {
                return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "complementary plan partition",
                });
            }
        };
        output.push(FourLoopNextPlanBinding {
            leaf_id: plan.leaf_id(),
            transport_plan_index: checked_u32(transport_index, "transport plan index")?,
            slice,
            closure_plan_index: checked_u16(closure_index, "closure plan index")?,
        });
    }
    let closure_leaf_count = t1s2_by_leaf.len().checked_add(three_by_leaf.len()).ok_or(
        FourLoopNextClosedRowsError::ArithmeticOverflow {
            resource: "closure plan leaf union",
        },
    )?;
    if output.len() != closure_leaf_count
        || transport_leaves.len() != output.len()
        || t1s2_by_leaf
            .keys()
            .any(|leaf| three_by_leaf.contains_key(leaf))
    {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "exhaustive complementary plan leaf union",
        });
    }
    if output.len() > config.max_plan_bindings {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "plan bindings",
            requested: output.len() as u128,
            limit: config.max_plan_bindings as u128,
        });
    }
    Ok(output)
}

fn authenticate_plan_products(
    ordinary: &ProductLinearCombination<MassiveVacuumMaster>,
    normalized: &ProductLinearCombination<MassiveVacuumMaster>,
) -> Result<(), FourLoopNextClosedRowsError> {
    if ordinary.terms().keys().ne(normalized.terms().keys()) {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "closure ordinary/normalized product keys",
        });
    }
    for product in ordinary.terms().keys() {
        if !is_allowed_product(product) {
            return Err(FourLoopNextClosedRowsError::UnsupportedProduct {
                product: product.clone(),
            });
        }
    }
    Ok(())
}

fn prescan_groups(
    inventory: &FourLoopNextInventory,
    plan_by_leaf: &BTreeMap<u32, usize>,
    context: &CoefficientContext,
    arithmetic: &mut CheckedArithmetic,
    retained_coefficients: &mut RetainedCoefficientCharge,
    config: FourLoopNextClosedRowsConfig,
) -> Result<
    (
        Vec<FourLoopNextBoundaryGroup>,
        BTreeMap<(u16, u32), usize>,
        StructuralStats,
    ),
    FourLoopNextClosedRowsError,
> {
    let mut stats = StructuralStats::default();
    let mut raw_groups = BTreeMap::<(u16, u32), Vec<u32>>::new();
    let mut genuine_groups = BTreeSet::<(u16, FourLoopCornerColumnId)>::new();
    for (row_index, row) in inventory.rows().iter().enumerate() {
        stats.max_row_paths = stats.max_row_paths.max(row.paths().len());
        stats.paths = checked_add(stats.paths, row.paths().len(), "paths")?;
        let row_index_u16 = checked_u16(row_index, "row index")?;
        for (path_index, path) in row.paths().iter().copied().enumerate() {
            let leaf = inventory.leaves().get(path.leaf_id() as usize).ok_or(
                FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "prescan leaf index",
                },
            )?;
            match leaf {
                FourLoopNextLeaf::Boundary(_) => {
                    stats.boundary_paths = checked_add(stats.boundary_paths, 1, "boundary paths")?;
                    raw_groups
                        .entry((row_index_u16, path.leaf_id()))
                        .or_default()
                        .push(checked_u32(path_index, "path index")?);
                }
                FourLoopNextLeaf::Genuine(genuine) => {
                    inventory.authenticate_genuine_column(genuine)?;
                    stats.genuine_paths = checked_add(stats.genuine_paths, 1, "genuine paths")?;
                    genuine_groups.insert((row_index_u16, genuine_column_id(genuine)));
                }
                FourLoopNextLeaf::FamilyScaleless { .. }
                | FourLoopNextLeaf::ScalarCornerScaleless { .. } => {
                    stats.scaleless_paths =
                        checked_add(stats.scaleless_paths, 1, "scaleless paths")?;
                }
            }
        }
    }
    stats.genuine_groups = genuine_groups.len();

    let mut groups = Vec::new();
    groups.try_reserve_exact(raw_groups.len()).map_err(|_| {
        FourLoopNextClosedRowsError::AllocationFailed {
            resource: "raw boundary groups",
            requested: raw_groups.len(),
        }
    })?;
    let mut group_by_row_leaf = BTreeMap::new();
    let mut row_group_counts = BTreeMap::<u16, usize>::new();
    for ((row_index, leaf_id), contributors) in raw_groups {
        let row = &inventory.rows()[usize::from(row_index)];
        let collected = row
            .collected_boundaries()
            .iter()
            .find(|entry| entry.leaf_id() == leaf_id);
        let (coefficient, canceled) = match collected {
            Some(entry) => {
                if entry.coefficient().is_zero()
                    || entry.contributor_path_indices() != contributors.as_slice()
                {
                    return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "retained boundary group",
                    });
                }
                stats.nonzero_boundary_groups =
                    checked_add(stats.nonzero_boundary_groups, 1, "nonzero boundary groups")?;
                stats.nonzero_boundary_contributors = checked_add(
                    stats.nonzero_boundary_contributors,
                    contributors.len(),
                    "nonzero boundary contributors",
                )?;
                (entry.coefficient().clone(), false)
            }
            None => {
                stats.canceled_boundary_groups = checked_add(
                    stats.canceled_boundary_groups,
                    1,
                    "canceled boundary groups",
                )?;
                (context.zero(), true)
            }
        };
        if contributors.len() > 1 {
            stats.repeated_boundary_groups = checked_add(
                stats.repeated_boundary_groups,
                1,
                "repeated boundary groups",
            )?;
            if canceled {
                stats.repeated_canceled_boundary_groups = checked_add(
                    stats.repeated_canceled_boundary_groups,
                    1,
                    "repeated canceled boundary groups",
                )?;
            } else {
                stats.repeated_surviving_boundary_groups = checked_add(
                    stats.repeated_surviving_boundary_groups,
                    1,
                    "repeated surviving boundary groups",
                )?;
            }
        }
        let plan_binding_index =
            *plan_by_leaf
                .get(&leaf_id)
                .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "boundary group plan binding",
                })?;
        let seed_mass_weight = powers_weight(row.raw_id().seed().powers())?;
        let boundary_mass_weight = powers_weight(inventory.boundary_key(leaf_id)?.powers())?;
        let mass_bridge_exponent = seed_mass_weight.checked_sub(boundary_mass_weight).ok_or(
            FourLoopNextClosedRowsError::ArithmeticOverflow {
                resource: "boundary mass bridge exponent",
            },
        )?;
        let seed_to_boundary_coefficient =
            arithmetic.apply_mass_power(&coefficient, mass_bridge_exponent)?;
        arithmetic.check_mass_free_bridge(&seed_to_boundary_coefficient, row.raw_id(), leaf_id)?;
        let group_index = groups.len();
        group_by_row_leaf.insert((row_index, leaf_id), group_index);
        if !canceled {
            *row_group_counts.entry(row_index).or_default() += 1;
        }
        retained_coefficients.charge(&coefficient)?;
        retained_coefficients.charge(&seed_to_boundary_coefficient)?;
        groups.push(FourLoopNextBoundaryGroup {
            row_index,
            leaf_id,
            plan_binding_index: checked_u32(plan_binding_index, "plan binding index")?,
            contributor_path_indices: contributors,
            collected_coefficient: coefficient,
            seed_mass_weight,
            boundary_mass_weight,
            mass_bridge_exponent,
            seed_to_boundary_coefficient,
            canceled,
        });
    }
    stats.max_row_boundary_groups = row_group_counts.values().copied().max().unwrap_or(0);
    if groups.len() > config.max_boundary_groups {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "raw boundary groups",
            requested: groups.len() as u128,
            limit: config.max_boundary_groups as u128,
        });
    }
    if stats.boundary_paths > config.max_boundary_group_contributors {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "boundary group contributors",
            requested: stats.boundary_paths as u128,
            limit: config.max_boundary_group_contributors as u128,
        });
    }
    Ok((groups, group_by_row_leaf, stats))
}

fn build_occurrence_bindings(
    inventory: &FourLoopNextInventory,
    transport: &FourLoopComponentTransport<'_>,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
    plan_bindings: &[FourLoopNextPlanBinding],
    group_by_row_leaf: &BTreeMap<(u16, u32), usize>,
    config: FourLoopNextClosedRowsConfig,
) -> Result<Vec<FourLoopNextOccurrenceBinding>, FourLoopNextClosedRowsError> {
    if transport.occurrences().len() != t1s2.occurrences().len()
        || transport.occurrences().len() != three_loop.occurrences().len()
    {
        return Err(FourLoopNextClosedRowsError::ReplayMismatch {
            stage: "closure occurrence vector lengths",
        });
    }
    let mut output = Vec::new();
    output
        .try_reserve_exact(transport.occurrences().len())
        .map_err(|_| FourLoopNextClosedRowsError::AllocationFailed {
            resource: "occurrence bindings",
            requested: transport.occurrences().len(),
        })?;
    let mut seen = BTreeSet::new();
    for (index, ((source, t1), three)) in transport
        .occurrences()
        .iter()
        .zip(t1s2.occurrences())
        .zip(three_loop.occurrences())
        .enumerate()
    {
        if source.row_index() != t1.row_index()
            || source.path_index() != t1.path_index()
            || source.leaf_id() != t1.leaf_id()
            || source.row_index() != three.row_index()
            || source.path_index() != three.path_index()
            || source.leaf_id() != three.leaf_id()
        {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "zipped occurrence coordinates",
            });
        }
        if !seen.insert((source.row_index(), source.path_index())) {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "duplicate occurrence coordinate",
            });
        }
        let source_plan = transport.plans().get(source.plan_index() as usize).ok_or(
            FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence transport plan index",
            },
        )?;
        if source_plan.leaf_id() != source.leaf_id() {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence transport plan leaf",
            });
        }
        let plan_binding = plan_bindings.get(source.plan_index() as usize).ok_or(
            FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence plan binding index",
            },
        )?;
        if plan_binding.leaf_id != source.leaf_id()
            || plan_binding.transport_plan_index != source.plan_index()
        {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence plan binding identity",
            });
        }
        let (slice, completed_index) =
            match (t1.completed_plan_index(), three.completed_plan_index()) {
                (Some(completed), None) => (FourLoopNextClosureSlice::T1S2, completed),
                (None, Some(completed)) => (FourLoopNextClosureSlice::ThreeLoop, completed),
                _ => {
                    return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                        stage: "complementary occurrence partition",
                    });
                }
            };
        if slice != plan_binding.slice || completed_index != plan_binding.closure_plan_index {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence closure plan index",
            });
        }
        let row = inventory.rows().get(source.row_index() as usize).ok_or(
            FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence row index",
            },
        )?;
        if row
            .paths()
            .get(source.path_index() as usize)
            .is_none_or(|path| path.leaf_id() != source.leaf_id())
        {
            return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence inventory path",
            });
        }
        let group_index = *group_by_row_leaf
            .get(&(source.row_index(), source.leaf_id()))
            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "occurrence raw boundary group",
            })?;
        output.push(FourLoopNextOccurrenceBinding {
            row_index: source.row_index(),
            path_index: source.path_index(),
            leaf_id: source.leaf_id(),
            transport_plan_index: source.plan_index(),
            plan_binding_index: source.plan_index(),
            boundary_group_index: checked_u32(group_index, "boundary group index")?,
            slice,
            closure_occurrence_index: checked_u32(index, "closure occurrence index")?,
        });
    }
    if output.len() > config.max_occurrence_bindings {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "occurrence bindings",
            requested: output.len() as u128,
            limit: config.max_occurrence_bindings as u128,
        });
    }
    Ok(output)
}

fn build_columns(
    inventory: &FourLoopNextInventory,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
    config: FourLoopNextClosedRowsConfig,
) -> Result<Vec<FourLoopCornerColumnId>, FourLoopNextClosedRowsError> {
    let mut columns = BTreeSet::new();
    for leaf in inventory.leaves() {
        if let FourLoopNextLeaf::Genuine(genuine) = leaf {
            inventory.authenticate_genuine_column(genuine)?;
            columns.insert(genuine_column_id(genuine));
        }
    }
    for plan in t1s2.plans() {
        for product in plan.ordinary().terms().keys() {
            if !is_allowed_product(product) {
                return Err(FourLoopNextClosedRowsError::UnsupportedProduct {
                    product: product.clone(),
                });
            }
            columns.insert(FourLoopCornerColumnId::Product(product.clone()));
        }
    }
    for plan in three_loop.plans() {
        for product in plan.ordinary().terms().keys() {
            if !is_allowed_product(product) {
                return Err(FourLoopNextClosedRowsError::UnsupportedProduct {
                    product: product.clone(),
                });
            }
            columns.insert(FourLoopCornerColumnId::Product(product.clone()));
        }
    }
    if columns.len() > config.max_global_columns {
        return Err(FourLoopNextClosedRowsError::ResourceLimit {
            resource: "global columns",
            requested: columns.len() as u128,
            limit: config.max_global_columns as u128,
        });
    }
    Ok(columns.into_iter().collect())
}

fn index_groups_by_row(
    groups: &[FourLoopNextBoundaryGroup],
    row_count: usize,
) -> Result<Vec<Vec<usize>>, FourLoopNextClosedRowsError> {
    let mut output = vec![Vec::new(); row_count];
    for (index, group) in groups.iter().enumerate() {
        output
            .get_mut(group.row_index as usize)
            .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                stage: "boundary group row index",
            })?
            .push(index);
    }
    Ok(output)
}

fn closure_combinations<'a>(
    binding: &FourLoopNextPlanBinding,
    t1s2: &'a FourLoopT1S2Closure<'_, '_>,
    three_loop: &'a FourLoopThreeLoopClosure<'_, '_>,
) -> Result<
    (
        &'a ProductLinearCombination<MassiveVacuumMaster>,
        &'a ProductLinearCombination<MassiveVacuumMaster>,
    ),
    FourLoopNextClosedRowsError,
> {
    match binding.slice {
        FourLoopNextClosureSlice::T1S2 => {
            let plan = t1s2
                .plans()
                .get(binding.closure_plan_index as usize)
                .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "T1/S2 closure plan index",
                })?;
            if plan.leaf_id() != binding.leaf_id {
                return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "T1/S2 closure plan leaf",
                });
            }
            Ok((plan.ordinary(), plan.mass_normalized()))
        }
        FourLoopNextClosureSlice::ThreeLoop => {
            let plan = three_loop
                .plans()
                .get(binding.closure_plan_index as usize)
                .ok_or(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "three-loop closure plan index",
                })?;
            if plan.leaf_id() != binding.leaf_id {
                return Err(FourLoopNextClosedRowsError::ReplayMismatch {
                    stage: "three-loop closure plan leaf",
                });
            }
            Ok((plan.ordinary(), plan.mass_normalized()))
        }
    }
}

fn genuine_column_id(genuine: &FourLoopNextGenuineColumn) -> FourLoopCornerColumnId {
    FourLoopCornerColumnId::Genuine {
        corner_type: genuine.corner_type(),
        powers: *genuine.powers(),
    }
}

fn is_allowed_product(product: &MasterProduct<MassiveVacuumMaster>) -> bool {
    use MassiveVacuumMaster::{B4, F5, M6, S2, T1};
    let multiplicity = |master| product.multiplicity(&master);
    let allowed = (multiplicity(T1) == 4
        && multiplicity(S2) == 0
        && multiplicity(B4) == 0
        && multiplicity(F5) == 0
        && multiplicity(M6) == 0)
        || (multiplicity(T1) == 2
            && multiplicity(S2) == 1
            && multiplicity(B4) == 0
            && multiplicity(F5) == 0
            && multiplicity(M6) == 0)
        || (multiplicity(T1) == 0
            && multiplicity(S2) == 2
            && multiplicity(B4) == 0
            && multiplicity(F5) == 0
            && multiplicity(M6) == 0)
        || (multiplicity(T1) == 1
            && multiplicity(S2) == 0
            && multiplicity(B4) == 1
            && multiplicity(F5) == 0
            && multiplicity(M6) == 0)
        || (multiplicity(T1) == 1
            && multiplicity(S2) == 0
            && multiplicity(B4) == 0
            && multiplicity(F5) == 1
            && multiplicity(M6) == 0)
        || (multiplicity(T1) == 1
            && multiplicity(S2) == 0
            && multiplicity(B4) == 0
            && multiplicity(F5) == 0
            && multiplicity(M6) == 1);
    allowed && product.distinct_factor_count() <= 2
}

fn powers_weight(powers: &[i32; 10]) -> Result<i64, FourLoopNextClosedRowsError> {
    powers.iter().try_fold(0_i64, |sum, &power| {
        sum.checked_add(i64::from(power))
            .ok_or(FourLoopNextClosedRowsError::ArithmeticOverflow {
                resource: "mass weight",
            })
    })
}

struct CheckedArithmetic {
    zero: Coefficient,
    mass: Coefficient,
    mass_position: usize,
    config: FourLoopNextClosedRowsConfig,
    mass_power_steps: usize,
    multiplications: usize,
    additions: usize,
    divisions: usize,
}

impl CheckedArithmetic {
    fn new(
        context: CoefficientContext,
        config: FourLoopNextClosedRowsConfig,
    ) -> Result<Self, FourLoopNextClosedRowsError> {
        let mass_position = context
            .parameter_names()
            .iter()
            .position(|name| name == "m2")
            .ok_or(FourLoopNextClosedRowsError::CoefficientContextMismatch)?;
        let mass = context
            .parameter("m2")
            .ok_or(FourLoopNextClosedRowsError::CoefficientContextMismatch)?;
        Ok(Self {
            zero: context.zero(),
            mass,
            mass_position,
            config,
            mass_power_steps: 0,
            multiplications: 0,
            additions: 0,
            divisions: 0,
        })
    }

    fn check_existing(&self, coefficient: &Coefficient) -> Result<(), FourLoopNextClosedRowsError> {
        if coefficient.get_variables() != self.zero.get_variables() {
            return Err(FourLoopNextClosedRowsError::CoefficientContextMismatch);
        }
        let degree = coefficient_variable_degrees(coefficient)
            .into_iter()
            .map(|(numerator, denominator)| numerator.max(denominator))
            .max()
            .unwrap_or(0);
        self.check_degree(degree)?;
        let actual_terms = coefficient
            .numerator
            .nterms()
            .max(coefficient.denominator.nterms());
        if actual_terms > self.config.max_coefficient_operation_terms {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource: "coefficient operand/result terms",
                requested: actual_terms as u128,
                limit: self.config.max_coefficient_operation_terms as u128,
            });
        }
        self.check_dense_universe(existing_dense_bound(coefficient))
    }

    fn check_degree(&self, requested: u128) -> Result<(), FourLoopNextClosedRowsError> {
        if requested > self.config.max_coefficient_degree as u128
            || !symbolica_coefficient_degree_is_representable(requested)
        {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource: "Symbolica coefficient exponent degree",
                requested,
                limit: (self.config.max_coefficient_degree as u128)
                    .min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT),
            });
        }
        Ok(())
    }

    fn charge_operation(
        &self,
        next: usize,
        resource: &'static str,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        let total = self
            .multiplications
            .checked_add(self.additions)
            .and_then(|value| value.checked_add(self.divisions))
            .and_then(|value| value.checked_add(next))
            .ok_or(FourLoopNextClosedRowsError::ArithmeticOverflow {
                resource: "coefficient operations",
            })?;
        if total > self.config.max_coefficient_operations {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource,
                requested: total as u128,
                limit: self.config.max_coefficient_operations as u128,
            });
        }
        Ok(())
    }

    fn check_kind_count(
        &self,
        current: usize,
        limit: usize,
        resource: &'static str,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        let requested = current
            .checked_add(1)
            .ok_or(FourLoopNextClosedRowsError::ArithmeticOverflow { resource })?;
        if requested > limit {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource,
                requested: requested as u128,
                limit: limit as u128,
            });
        }
        Ok(())
    }

    fn check_dense_universe(&self, requested: u128) -> Result<(), FourLoopNextClosedRowsError> {
        if requested > self.config.max_coefficient_dense_terms as u128 {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource: "coefficient dense operand/result universe",
                requested,
                limit: self.config.max_coefficient_dense_terms as u128,
            });
        }
        Ok(())
    }

    fn multiply(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopNextClosedRowsError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        self.check_degree(coefficient_product_degree_bound(left, right))?;
        self.check_dense_universe(product_dense_bound(left, right))?;
        self.charge_operation(1, "coefficient multiplications")?;
        self.check_kind_count(
            self.multiplications,
            self.config.max_coefficient_multiplications,
            "coefficient multiplications",
        )?;
        self.multiplications = checked_add(self.multiplications, 1, "coefficient multiplications")?;
        let output = left * right;
        self.check_existing(&output)?;
        Ok(output)
    }

    fn add(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopNextClosedRowsError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        self.check_degree(coefficient_sum_degree_bound(left, right))?;
        self.check_dense_universe(sum_dense_bound(left, right))?;
        self.charge_operation(1, "coefficient additions")?;
        self.check_kind_count(
            self.additions,
            self.config.max_coefficient_additions,
            "coefficient additions",
        )?;
        self.additions = checked_add(self.additions, 1, "coefficient additions")?;
        let output = left + right;
        self.check_existing(&output)?;
        Ok(output)
    }

    fn divide(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopNextClosedRowsError> {
        if right.is_zero() {
            return Err(FourLoopNextClosedRowsError::ZeroPivotCoefficient);
        }
        self.check_existing(left)?;
        self.check_existing(right)?;
        self.check_degree(coefficient_quotient_degree_bound(left, right))?;
        self.check_dense_universe(quotient_dense_bound(left, right))?;
        self.charge_operation(1, "coefficient divisions")?;
        self.check_kind_count(
            self.divisions,
            self.config.max_coefficient_divisions,
            "coefficient divisions",
        )?;
        self.divisions = checked_add(self.divisions, 1, "coefficient divisions")?;
        let output = left / right;
        self.check_existing(&output)?;
        Ok(output)
    }

    fn add_sparse<K: Ord>(
        &mut self,
        output: &mut BTreeMap<K, Coefficient>,
        key: K,
        coefficient: Coefficient,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        if coefficient.is_zero() {
            return Ok(());
        }
        if let Some(current) = output.remove(&key) {
            let sum = self.add(&current, &coefficient)?;
            if !sum.is_zero() {
                output.insert(key, sum);
            }
        } else {
            self.check_existing(&coefficient)?;
            output.insert(key, coefficient);
        }
        Ok(())
    }

    fn apply_mass_power(
        &mut self,
        coefficient: &Coefficient,
        exponent: i64,
    ) -> Result<Coefficient, FourLoopNextClosedRowsError> {
        self.check_existing(coefficient)?;
        let steps = usize::try_from(exponent.unsigned_abs()).map_err(|_| {
            FourLoopNextClosedRowsError::ArithmeticOverflow {
                resource: "mass-power steps",
            }
        })?;
        self.mass_power_steps = checked_add(self.mass_power_steps, steps, "mass-power steps")?;
        if self.mass_power_steps > self.config.max_mass_power_steps {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource: "mass-power steps",
                requested: self.mass_power_steps as u128,
                limit: self.config.max_mass_power_steps as u128,
            });
        }
        let mass = self.mass.clone();
        let mut value = coefficient.clone();
        for _ in 0..steps {
            value = if exponent >= 0 {
                self.multiply(&value, &mass)?
            } else {
                self.divide(&value, &mass)?
            };
        }
        Ok(value)
    }

    fn check_mass_free(
        &self,
        coefficient: &Coefficient,
        raw_id: FourLoopNextRawRowId,
        column: &FourLoopCornerColumnId,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        self.check_existing(coefficient)?;
        let (numerator_degree, denominator_degree) = coefficient_variable_degrees(coefficient)
            .get(self.mass_position)
            .copied()
            .ok_or(FourLoopNextClosedRowsError::CoefficientContextMismatch)?;
        if numerator_degree != 0 || denominator_degree != 0 {
            return Err(FourLoopNextClosedRowsError::ResidualMassDependence {
                raw_id,
                column: column.clone(),
                numerator_degree,
                denominator_degree,
            });
        }
        Ok(())
    }

    fn check_mass_free_bridge(
        &self,
        coefficient: &Coefficient,
        raw_id: FourLoopNextRawRowId,
        leaf_id: u32,
    ) -> Result<(), FourLoopNextClosedRowsError> {
        self.check_existing(coefficient)?;
        let (numerator_degree, denominator_degree) = coefficient_variable_degrees(coefficient)
            .get(self.mass_position)
            .copied()
            .ok_or(FourLoopNextClosedRowsError::CoefficientContextMismatch)?;
        if numerator_degree != 0 || denominator_degree != 0 {
            return Err(
                FourLoopNextClosedRowsError::ResidualBoundaryBridgeMassDependence {
                    raw_id,
                    leaf_id,
                    numerator_degree,
                    denominator_degree,
                },
            );
        }
        Ok(())
    }
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

fn dense_monomial_bound(degrees: impl IntoIterator<Item = u128>) -> u128 {
    degrees.into_iter().fold(1_u128, |count, degree| {
        count.saturating_mul(degree.saturating_add(1))
    })
}

fn existing_dense_bound(value: &Coefficient) -> u128 {
    let degrees = coefficient_variable_degrees(value);
    dense_monomial_bound(degrees.iter().map(|&(numerator, _)| numerator)).max(dense_monomial_bound(
        degrees.iter().map(|&(_, denominator)| denominator),
    ))
}

fn product_dense_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    let left = coefficient_variable_degrees(left);
    let right = coefficient_variable_degrees(right);
    dense_monomial_bound(
        left.iter()
            .zip(&right)
            .map(|(&(ln, _), &(rn, _))| ln.saturating_add(rn)),
    )
    .max(dense_monomial_bound(
        left.iter()
            .zip(&right)
            .map(|(&(_, ld), &(_, rd))| ld.saturating_add(rd)),
    ))
}

fn sum_dense_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    let left = coefficient_variable_degrees(left);
    let right = coefficient_variable_degrees(right);
    dense_monomial_bound(
        left.iter()
            .zip(&right)
            .map(|(&(ln, ld), &(rn, rd))| ln.saturating_add(rd).max(rn.saturating_add(ld))),
    )
    .max(dense_monomial_bound(
        left.iter()
            .zip(&right)
            .map(|(&(_, ld), &(_, rd))| ld.saturating_add(rd)),
    ))
}

fn quotient_dense_bound(left: &Coefficient, right: &Coefficient) -> u128 {
    let left = coefficient_variable_degrees(left);
    let right = coefficient_variable_degrees(right);
    dense_monomial_bound(
        left.iter()
            .zip(&right)
            .map(|(&(ln, _), &(_, rd))| ln.saturating_add(rd)),
    )
    .max(dense_monomial_bound(
        left.iter()
            .zip(&right)
            .map(|(&(_, ld), &(rn, _))| ld.saturating_add(rn)),
    ))
}

struct RetainedCoefficientCharge {
    config: FourLoopNextClosedRowsConfig,
    terms: usize,
    bytes: usize,
}

impl RetainedCoefficientCharge {
    const fn new(config: FourLoopNextClosedRowsConfig) -> Self {
        Self {
            config,
            terms: 0,
            bytes: 0,
        }
    }

    fn charge(&mut self, coefficient: &Coefficient) -> Result<(), FourLoopNextClosedRowsError> {
        let terms = coefficient
            .numerator
            .nterms()
            .checked_add(coefficient.denominator.nterms())
            .ok_or(FourLoopNextClosedRowsError::ArithmeticOverflow {
                resource: "retained coefficient terms",
            })?;
        let requested_terms = checked_add(self.terms, terms, "retained coefficient terms")?;
        if requested_terms > self.config.max_retained_coefficient_terms {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource: "retained coefficient terms",
                requested: requested_terms as u128,
                limit: self.config.max_retained_coefficient_terms as u128,
            });
        }
        let serialized_bytes = bounded_display_len(
            coefficient,
            self.bytes,
            self.config.max_retained_coefficient_bytes,
        )?;
        let requested_bytes =
            checked_add(self.bytes, serialized_bytes, "retained coefficient bytes")?;
        if requested_bytes > self.config.max_retained_coefficient_bytes {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource: "retained coefficient bytes",
                requested: requested_bytes as u128,
                limit: self.config.max_retained_coefficient_bytes as u128,
            });
        }
        self.terms = requested_terms;
        self.bytes = requested_bytes;
        Ok(())
    }
}

fn check_exact_count(
    resource: &'static str,
    expected: usize,
    actual: usize,
) -> Result<(), FourLoopNextClosedRowsError> {
    if actual != expected {
        return Err(FourLoopNextClosedRowsError::CensusMismatch {
            resource,
            expected,
            actual,
        });
    }
    Ok(())
}

fn check_early_structural_stats(
    stats: FourLoopNextClosedRowsStats,
) -> Result<(), FourLoopNextClosedRowsError> {
    for (resource, expected, actual) in [
        ("closed rows", FOUR_LOOP_NEXT_CLOSED_ROWS, stats.rows),
        ("paths", FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS, stats.paths),
        (
            "boundary paths",
            FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
            stats.boundary_paths,
        ),
        (
            "genuine paths",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_PATHS,
            stats.genuine_paths,
        ),
        ("scaleless paths", 0, stats.scaleless_paths),
        (
            "plan bindings",
            FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS,
            stats.plan_bindings,
        ),
        (
            "raw boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS,
            stats.raw_boundary_groups,
        ),
        (
            "nonzero boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_GROUPS,
            stats.nonzero_boundary_groups,
        ),
        (
            "canceled boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_CANCELED_BOUNDARY_GROUPS,
            stats.canceled_boundary_groups,
        ),
        (
            "repeated boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_BOUNDARY_GROUPS,
            stats.repeated_boundary_groups,
        ),
        (
            "repeated surviving boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_SURVIVING_BOUNDARY_GROUPS,
            stats.repeated_surviving_boundary_groups,
        ),
        (
            "repeated canceled boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_CANCELED_BOUNDARY_GROUPS,
            stats.repeated_canceled_boundary_groups,
        ),
        (
            "nonzero boundary contributors",
            FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_CONTRIBUTORS,
            stats.nonzero_boundary_contributors,
        ),
        (
            "genuine row groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_GROUPS,
            stats.genuine_groups,
        ),
        (
            "maximum row paths",
            FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_PATHS,
            stats.max_row_paths,
        ),
        (
            "maximum row boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_BOUNDARY_GROUPS,
            stats.max_row_boundary_groups,
        ),
    ] {
        check_exact_count(resource, expected, actual)?;
    }
    Ok(())
}

fn check_exact_stats(
    stats: FourLoopNextClosedRowsStats,
) -> Result<(), FourLoopNextClosedRowsError> {
    for (resource, expected, actual) in [
        ("closed rows", FOUR_LOOP_NEXT_CLOSED_ROWS, stats.rows),
        ("paths", FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS, stats.paths),
        (
            "boundary paths",
            FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
            stats.boundary_paths,
        ),
        (
            "genuine paths",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_PATHS,
            stats.genuine_paths,
        ),
        ("scaleless paths", 0, stats.scaleless_paths),
        (
            "plan bindings",
            FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS,
            stats.plan_bindings,
        ),
        (
            "occurrence bindings",
            FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
            stats.occurrence_bindings,
        ),
        (
            "raw boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS,
            stats.raw_boundary_groups,
        ),
        (
            "nonzero boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_GROUPS,
            stats.nonzero_boundary_groups,
        ),
        (
            "canceled boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_CANCELED_BOUNDARY_GROUPS,
            stats.canceled_boundary_groups,
        ),
        (
            "repeated boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_BOUNDARY_GROUPS,
            stats.repeated_boundary_groups,
        ),
        (
            "repeated surviving boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_SURVIVING_BOUNDARY_GROUPS,
            stats.repeated_surviving_boundary_groups,
        ),
        (
            "repeated canceled boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_CANCELED_BOUNDARY_GROUPS,
            stats.repeated_canceled_boundary_groups,
        ),
        (
            "nonzero boundary contributors",
            FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_CONTRIBUTORS,
            stats.nonzero_boundary_contributors,
        ),
        (
            "genuine row groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_GROUPS,
            stats.genuine_groups,
        ),
        (
            "genuine columns",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_COLUMNS,
            stats.genuine_columns,
        ),
        (
            "product columns",
            FOUR_LOOP_NEXT_CLOSED_ROWS_PRODUCT_COLUMNS,
            stats.product_columns,
        ),
        (
            "global columns",
            FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS,
            stats.global_columns,
        ),
        (
            "maximum row paths",
            FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_PATHS,
            stats.max_row_paths,
        ),
        (
            "maximum row boundary groups",
            FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_BOUNDARY_GROUPS,
            stats.max_row_boundary_groups,
        ),
    ] {
        if actual != expected {
            return Err(FourLoopNextClosedRowsError::CensusMismatch {
                resource,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

fn check_actual_stats(
    stats: FourLoopNextClosedRowsStats,
    config: FourLoopNextClosedRowsConfig,
) -> Result<(), FourLoopNextClosedRowsError> {
    for (resource, requested, limit) in [
        ("closed rows", stats.rows, config.max_rows),
        ("path dispositions", stats.paths, config.max_paths),
        (
            "plan bindings",
            stats.plan_bindings,
            config.max_plan_bindings,
        ),
        (
            "occurrence bindings",
            stats.occurrence_bindings,
            config.max_occurrence_bindings,
        ),
        (
            "raw boundary groups",
            stats.raw_boundary_groups,
            config.max_boundary_groups,
        ),
        (
            "boundary group contributors",
            stats.boundary_paths,
            config.max_boundary_group_contributors,
        ),
        (
            "genuine row groups",
            stats.genuine_groups,
            config.max_genuine_groups,
        ),
        (
            "global columns",
            stats.global_columns,
            config.max_global_columns,
        ),
        (
            "primary contributions",
            stats.primary_contributions,
            config.max_primary_contributions,
        ),
        (
            "raw-audit contributions",
            stats.raw_audit_contributions,
            config.max_raw_audit_contributions,
        ),
        (
            "collected row entries",
            stats.collected_entries,
            config.max_collected_entries,
        ),
        (
            "closed row width",
            stats.max_row_width,
            config.max_row_width,
        ),
        (
            "mass-power steps",
            stats.mass_power_steps,
            config.max_mass_power_steps,
        ),
        (
            "coefficient operations",
            stats.coefficient_operations(),
            config.max_coefficient_operations,
        ),
        (
            "coefficient multiplications",
            stats.coefficient_multiplications,
            config.max_coefficient_multiplications,
        ),
        (
            "coefficient additions",
            stats.coefficient_additions,
            config.max_coefficient_additions,
        ),
        (
            "coefficient divisions",
            stats.coefficient_divisions,
            config.max_coefficient_divisions,
        ),
        (
            "retained coefficient terms",
            stats.retained_coefficient_terms,
            config.max_retained_coefficient_terms,
        ),
        (
            "retained coefficient bytes",
            stats.retained_coefficient_bytes,
            config.max_retained_coefficient_bytes,
        ),
    ] {
        if requested > limit {
            return Err(FourLoopNextClosedRowsError::ResourceLimit {
                resource,
                requested: requested as u128,
                limit: limit as u128,
            });
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn closed_rows_checksum(
    inventory: &FourLoopNextInventory,
    transport: &FourLoopComponentTransport<'_>,
    t1s2: &FourLoopT1S2Closure<'_, '_>,
    three_loop: &FourLoopThreeLoopClosure<'_, '_>,
    config: FourLoopNextClosedRowsConfig,
    context: &CoefficientContext,
    plans: &[FourLoopNextPlanBinding],
    occurrences: &[FourLoopNextOccurrenceBinding],
    groups: &[FourLoopNextBoundaryGroup],
    columns: &[FourLoopCornerColumnId],
    rows: &[FourLoopNextClosedRow],
    stats: FourLoopNextClosedRowsStats,
) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_bytes(&mut hash, FourLoopNextClosedRows::SCHEMA.as_bytes());
    hash_bytes(&mut hash, FourLoopNextInventory::SCHEMA.as_bytes());
    hash_bytes(&mut hash, FourLoopComponentTransport::SCHEMA.as_bytes());
    hash_bytes(&mut hash, FourLoopT1S2Closure::SCHEMA.as_bytes());
    hash_bytes(&mut hash, FourLoopThreeLoopClosure::SCHEMA.as_bytes());
    hash_u64(&mut hash, inventory.manifest().seed_checksum());
    hash_u64(&mut hash, transport.source_seed_checksum());
    hash_u64(&mut hash, t1s2.checksum());
    hash_u64(&mut hash, three_loop.checksum());
    for name in context.parameter_names() {
        hash_bytes(&mut hash, name.as_bytes());
    }
    hash_config(&mut hash, config);
    for plan in plans {
        hash_u64(&mut hash, u64::from(plan.leaf_id));
        hash_u64(&mut hash, u64::from(plan.transport_plan_index));
        hash_bytes(&mut hash, plan.slice.stable_key().as_bytes());
        hash_u64(&mut hash, u64::from(plan.closure_plan_index));
    }
    for occurrence in occurrences {
        for value in [
            u64::from(occurrence.row_index),
            u64::from(occurrence.path_index),
            u64::from(occurrence.leaf_id),
            u64::from(occurrence.transport_plan_index),
            u64::from(occurrence.plan_binding_index),
            u64::from(occurrence.boundary_group_index),
            u64::from(occurrence.closure_occurrence_index),
        ] {
            hash_u64(&mut hash, value);
        }
        hash_bytes(&mut hash, occurrence.slice.stable_key().as_bytes());
    }
    for group in groups {
        hash_u64(&mut hash, u64::from(group.row_index));
        hash_u64(&mut hash, u64::from(group.leaf_id));
        hash_u64(&mut hash, u64::from(group.plan_binding_index));
        for &path in &group.contributor_path_indices {
            hash_u64(&mut hash, u64::from(path));
        }
        hash_display(&mut hash, &group.collected_coefficient);
        hash_bytes(&mut hash, &group.seed_mass_weight.to_le_bytes());
        hash_bytes(&mut hash, &group.boundary_mass_weight.to_le_bytes());
        hash_bytes(&mut hash, &group.mass_bridge_exponent.to_le_bytes());
        hash_display(&mut hash, &group.seed_to_boundary_coefficient);
        hash_u64(&mut hash, u64::from(group.canceled));
    }
    for column in columns {
        hash_bytes(&mut hash, column.stable_key().as_bytes());
    }
    for row in rows {
        hash_bytes(&mut hash, row.raw_id.stable_key().as_bytes());
        hash_bytes(&mut hash, &row.seed_mass_weight.to_le_bytes());
        for disposition in &row.path_dispositions {
            hash_disposition(&mut hash, disposition);
        }
        for &group in &row.boundary_group_indices {
            hash_u64(&mut hash, u64::from(group));
        }
        hash_display(&mut hash, &row.row_scale);
        hash_u64(
            &mut hash,
            row.pivot_column_index.map_or(u64::MAX, u64::from),
        );
        for (column, coefficient) in &row.entries {
            hash_bytes(&mut hash, column.stable_key().as_bytes());
            hash_display(&mut hash, coefficient);
        }
    }
    hash_stats(&mut hash, stats);
    hash
}

fn hash_disposition(hash: &mut u64, disposition: &FourLoopNextPathDisposition) {
    match *disposition {
        FourLoopNextPathDisposition::FamilyScaleless { leaf_id } => {
            hash_bytes(hash, b"family-scaleless");
            hash_u64(hash, u64::from(leaf_id));
        }
        FourLoopNextPathDisposition::ScalarCornerScaleless { leaf_id } => {
            hash_bytes(hash, b"corner-scaleless");
            hash_u64(hash, u64::from(leaf_id));
        }
        FourLoopNextPathDisposition::Genuine {
            leaf_id,
            column_index,
        } => {
            hash_bytes(hash, b"genuine");
            hash_u64(hash, u64::from(leaf_id));
            hash_u64(hash, u64::from(column_index));
        }
        FourLoopNextPathDisposition::Boundary {
            leaf_id,
            occurrence_binding_index,
            boundary_group_index,
        } => {
            hash_bytes(hash, b"boundary");
            hash_u64(hash, u64::from(leaf_id));
            hash_u64(hash, u64::from(occurrence_binding_index));
            hash_u64(hash, u64::from(boundary_group_index));
        }
    }
}

fn hash_config(hash: &mut u64, config: FourLoopNextClosedRowsConfig) {
    for value in [
        config.max_rows,
        config.max_paths,
        config.max_plan_bindings,
        config.max_occurrence_bindings,
        config.max_boundary_groups,
        config.max_boundary_group_contributors,
        config.max_genuine_groups,
        config.max_global_columns,
        config.max_primary_contributions,
        config.max_raw_audit_contributions,
        config.max_collected_entries,
        config.max_row_width,
        config.max_mass_power_steps,
        config.max_coefficient_operations,
        config.max_coefficient_multiplications,
        config.max_coefficient_additions,
        config.max_coefficient_divisions,
        config.max_coefficient_operation_terms,
        config.max_coefficient_dense_terms,
        config.max_coefficient_degree,
        config.max_retained_coefficient_terms,
        config.max_retained_coefficient_bytes,
    ] {
        hash_u64(hash, value as u64);
    }
}

fn hash_stats(hash: &mut u64, stats: FourLoopNextClosedRowsStats) {
    for value in [
        stats.rows,
        stats.paths,
        stats.boundary_paths,
        stats.genuine_paths,
        stats.scaleless_paths,
        stats.plan_bindings,
        stats.occurrence_bindings,
        stats.raw_boundary_groups,
        stats.nonzero_boundary_groups,
        stats.canceled_boundary_groups,
        stats.repeated_boundary_groups,
        stats.repeated_surviving_boundary_groups,
        stats.repeated_canceled_boundary_groups,
        stats.nonzero_boundary_contributors,
        stats.genuine_groups,
        stats.genuine_columns,
        stats.product_columns,
        stats.global_columns,
        stats.primary_contributions,
        stats.raw_audit_contributions,
        stats.collected_entries,
        stats.zero_rows,
        stats.max_row_paths,
        stats.max_row_boundary_groups,
        stats.max_row_width,
        stats.mass_power_steps,
        stats.coefficient_multiplications,
        stats.coefficient_additions,
        stats.coefficient_divisions,
        stats.retained_coefficient_terms,
        stats.retained_coefficient_bytes,
    ] {
        hash_u64(hash, value as u64);
    }
}

struct BoundedLengthWriter {
    length: usize,
    limit: usize,
}

impl fmt::Write for BoundedLengthWriter {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let next = self.length.checked_add(value.len()).ok_or(fmt::Error)?;
        if next > self.limit {
            return Err(fmt::Error);
        }
        self.length = next;
        Ok(())
    }
}

fn bounded_display_len(
    value: &Coefficient,
    used: usize,
    total_limit: usize,
) -> Result<usize, FourLoopNextClosedRowsError> {
    let remaining = total_limit.saturating_sub(used);
    let mut writer = BoundedLengthWriter {
        length: 0,
        limit: remaining,
    };
    write!(&mut writer, "{value}").map_err(|_| FourLoopNextClosedRowsError::ResourceLimit {
        resource: "retained coefficient bytes",
        requested: total_limit as u128 + 1,
        limit: total_limit as u128,
    })?;
    Ok(writer.length)
}

struct HashWriter<'a> {
    hash: &'a mut u64,
}

impl fmt::Write for HashWriter<'_> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        for byte in value.bytes() {
            *self.hash ^= u64::from(byte);
            *self.hash = self.hash.wrapping_mul(FNV1A64_PRIME);
        }
        Ok(())
    }
}

fn hash_display(hash: &mut u64, value: &Coefficient) {
    let mut writer = HashWriter { hash };
    write!(&mut writer, "{value}").expect("hash writer is infallible");
    *hash ^= u64::from(0xff_u8);
    *hash = hash.wrapping_mul(FNV1A64_PRIME);
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes.iter().copied().chain([0xff]) {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, FourLoopNextClosedRowsError> {
    left.checked_add(right)
        .ok_or(FourLoopNextClosedRowsError::ArithmeticOverflow { resource })
}

fn checked_u16(value: usize, resource: &'static str) -> Result<u16, FourLoopNextClosedRowsError> {
    u16::try_from(value).map_err(|_| FourLoopNextClosedRowsError::ArithmeticOverflow { resource })
}

fn checked_u32(value: usize, resource: &'static str) -> Result<u32, FourLoopNextClosedRowsError> {
    u32::try_from(value).map_err(|_| FourLoopNextClosedRowsError::ArithmeticOverflow { resource })
}
