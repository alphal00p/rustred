//! Exact lower-loop closure for the T1/S2-only part of the four-loop
//! next-shell component transport.
//!
//! This certificate closes every transported plan with product `T1^4`,
//! `T1^2*S2`, or `S2^2`.  It consumes only the scalar branches already
//! authenticated by [`crate::FourLoopComponentTransport`]: cross-component
//! tensor terms never enter this layer.  Components are reduced in their exact
//! parent Symbolica coefficient context, convolved as ordinary
//! `Q(d,m2)`-valued products, and only then mass-normalized.
//!
//! The remaining `T1*B4`, `T1*F5`, and `T1*M6` plans stay explicitly open.
//! Consequently this module makes no normalized-row, rank, elimination, or
//! four-loop-master claim.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{FamilyError, Integral, VacuumFamily};
use crate::{
    FourLoopComponentScalarBranchKind, FourLoopComponentTransport, FourLoopComponentTransportError,
    FourLoopComponentTransportOccurrence, FourLoopComponentTransportPlan, MassiveVacuumMaster,
    OneLoopTadpoleConfig, OneLoopTadpoleError, OneLoopTadpoleReducer, OneLoopTadpoleReduction,
    TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND, TWO_LOOP_TOP_DOT_RAW_TERM_BOUND, TwoLoopBoundaryError,
    TwoLoopBoundaryReducer, TwoLoopTopDotConfig, TwoLoopTopDotError, TwoLoopTopDotPreflight,
    TwoLoopTopDotReducer, equal_mass_two_loop_vacuum_in_context,
};
use rustred::legacy_oracle_support::coefficient_degree::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use rustred::{
    Coefficient, CoefficientContext, MasterProduct, MasterProductError, ProductLinearCombination,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

/// Exact plan census of the advertised slice.
pub const FOUR_LOOP_T1S2_CLOSURE_PLANS: usize = 243;
/// Exact number of plans deliberately left for the three-loop component
/// service.
pub const FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS: usize = 823;
/// The occurrence partition retains all source references, not just the
/// completed side.
pub const FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES: usize = 4_230;
pub const FOUR_LOOP_T1S2_CLOSURE_COMPONENTS: usize = 777;
pub const FOUR_LOOP_T1S2_CLOSURE_LOCAL_SLOTS: usize = 1_167;
/// Gross preflight envelope: all 243 plans treated as their widest possible
/// N1 branch shape. The retained exact census is reported in [`FourLoopT1S2ClosureStats`].
pub const FOUR_LOOP_T1S2_CLOSURE_SCALAR_BRANCHES: usize = 1_410;
/// Gross component-call envelope derived from the widest branch shape.
pub const FOUR_LOOP_T1S2_CLOSURE_COMPONENT_CALLS: usize = 4_366;
/// Gross labelled T1/S2 target universe through scalar D2 and one active
/// pinch; the exact authenticated cache can be smaller.
pub const FOUR_LOOP_T1S2_CLOSURE_UNIQUE_TARGETS: usize = 32;
/// Sequential checked-convolution envelope.  It includes intermediate
/// identity/component products rather than counting only final Cartesian
/// leaves.
pub const FOUR_LOOP_T1S2_CLOSURE_CONVOLUTION_PAIRS: usize = 7_460;
pub const FOUR_LOOP_T1S2_CLOSURE_PRECOLLECTION_TERMS: usize = 3_048;
pub const FOUR_LOOP_T1S2_CLOSURE_COLLECTED_TERMS: usize = 729;
/// Every allowed output differs from its parent input by at most four powers
/// of `m2`; charge that gross bound for all possible retained terms.
pub const FOUR_LOOP_T1S2_CLOSURE_MASS_POWER_STEPS: usize =
    FOUR_LOOP_T1S2_CLOSURE_COLLECTED_TERMS * 4;
/// Gross exact-arithmetic envelope outside the independently bounded local
/// services.
pub const FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_OPERATIONS: usize = 20_000;
pub const FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_DEGREE: u128 = 64;
pub const FOUR_LOOP_T1S2_CLOSURE_RETAINED_COEFFICIENT_BYTES: usize = 4 * 1024 * 1024;

const FNV1A64_OFFSET: u64 = 0xcbf29ce484222325;
const FNV1A64_PRIME: u64 = 0x100000001b3;

/// Conservative batch limits.  Defaults admit the complete authenticated
/// slice and reject undersized aggregate budgets before transport replay or
/// lower-loop coefficient work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopT1S2ClosureConfig {
    pub max_plans: usize,
    pub max_open_plans: usize,
    pub max_occurrences: usize,
    pub max_components: usize,
    pub max_local_slots: usize,
    pub max_scalar_branches: usize,
    pub max_component_calls: usize,
    pub max_unique_targets: usize,
    pub max_convolution_pair_operations: usize,
    pub max_precollection_terms: usize,
    pub max_collected_terms: usize,
    pub max_mass_power_steps: usize,
    pub max_coefficient_operations: usize,
    pub max_coefficient_degree: u128,
    pub max_retained_coefficient_bytes: usize,
    pub one_loop: OneLoopTadpoleConfig,
    pub two_loop: TwoLoopTopDotConfig,
}

impl Default for FourLoopT1S2ClosureConfig {
    fn default() -> Self {
        Self {
            max_plans: FOUR_LOOP_T1S2_CLOSURE_PLANS,
            max_open_plans: FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS,
            max_occurrences: FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES,
            max_components: FOUR_LOOP_T1S2_CLOSURE_COMPONENTS,
            max_local_slots: FOUR_LOOP_T1S2_CLOSURE_LOCAL_SLOTS,
            max_scalar_branches: FOUR_LOOP_T1S2_CLOSURE_SCALAR_BRANCHES,
            max_component_calls: FOUR_LOOP_T1S2_CLOSURE_COMPONENT_CALLS,
            max_unique_targets: FOUR_LOOP_T1S2_CLOSURE_UNIQUE_TARGETS,
            max_convolution_pair_operations: FOUR_LOOP_T1S2_CLOSURE_CONVOLUTION_PAIRS,
            max_precollection_terms: FOUR_LOOP_T1S2_CLOSURE_PRECOLLECTION_TERMS,
            max_collected_terms: FOUR_LOOP_T1S2_CLOSURE_COLLECTED_TERMS,
            max_mass_power_steps: FOUR_LOOP_T1S2_CLOSURE_MASS_POWER_STEPS,
            max_coefficient_operations: FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_OPERATIONS,
            max_coefficient_degree: FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_DEGREE,
            max_retained_coefficient_bytes: FOUR_LOOP_T1S2_CLOSURE_RETAINED_COEFFICIENT_BYTES,
            one_loop: OneLoopTadpoleConfig {
                max_recurrence_steps: 2,
                max_coefficient_operations: 8,
                max_dense_term_operations: 24,
                max_coefficient_degree: 2,
            },
            two_loop: TwoLoopTopDotConfig {
                max_explicit_terms: TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND,
                max_raw_terms: TWO_LOOP_TOP_DOT_RAW_TERM_BOUND,
                max_states: 28,
                max_coefficient_operations: 200,
                max_coefficient_degree: 6,
                max_boundary_formula_iterations: 5,
            },
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FourLoopT1S2ProductClass {
    T1Fourth,
    T1SquaredS2,
    S2Squared,
}

impl FourLoopT1S2ProductClass {
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::T1Fourth => "rustred-four-loop-t1s2-product-v1:T1^4",
            Self::T1SquaredS2 => "rustred-four-loop-t1s2-product-v1:T1^2*S2",
            Self::S2Squared => "rustred-four-loop-t1s2-product-v1:S2^2",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopT1S2ClosureStatus {
    ExactT1S2Slice,
}

/// The enclosing next-shell boundary remains partial until the 823 plans with
/// a genuine three-loop component are closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopT1S2ParentStatus {
    Partial {
        completed_plans: usize,
        open_plans: usize,
        completed_occurrences: usize,
        open_occurrences: usize,
    },
}

/// Deterministic actual-work census.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopT1S2ClosureStats {
    completed_plans: usize,
    open_plans: usize,
    completed_occurrences: usize,
    open_occurrences: usize,
    completed_rows: usize,
    open_rows: usize,
    mixed_rows: usize,
    components: usize,
    local_slots: usize,
    scalar_branches: usize,
    base_branches: usize,
    constant_branches: usize,
    local_t1_branches: usize,
    local_s2_branches: usize,
    component_calls: usize,
    t1_component_calls: usize,
    s2_component_calls: usize,
    unique_targets: usize,
    t1_targets: usize,
    s2_targets: usize,
    cache_hits: usize,
    convolution_pair_operations: usize,
    precollection_terms: usize,
    collected_terms: usize,
    mass_power_steps: usize,
    coefficient_operations: usize,
    retained_coefficient_bytes: usize,
    n0_plans: usize,
    n1_plans: usize,
}

macro_rules! stat_getters {
    ($($name:ident),* $(,)?) => {
        $(pub const fn $name(self) -> usize { self.$name })*
    };
}

impl FourLoopT1S2ClosureStats {
    stat_getters!(
        completed_plans,
        open_plans,
        completed_occurrences,
        open_occurrences,
        completed_rows,
        open_rows,
        mixed_rows,
        components,
        local_slots,
        scalar_branches,
        base_branches,
        constant_branches,
        local_t1_branches,
        local_s2_branches,
        component_calls,
        t1_component_calls,
        s2_component_calls,
        unique_targets,
        t1_targets,
        s2_targets,
        cache_hits,
        convolution_pair_operations,
        precollection_terms,
        collected_terms,
        mass_power_steps,
        coefficient_operations,
        retained_coefficient_bytes,
        n0_plans,
        n1_plans,
    );
}

/// Stable local target in a complete component denominator basis.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FourLoopT1S2LocalTarget {
    master: MassiveVacuumMaster,
    powers: Vec<i32>,
}

impl FourLoopT1S2LocalTarget {
    pub const fn master(&self) -> MassiveVacuumMaster {
        self.master
    }

    pub fn powers(&self) -> &[i32] {
        &self.powers
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FourLoopT1S2LocalProof {
    Tadpole(OneLoopTadpoleReduction),
    Sunset {
        requested: Integral,
        oriented: Integral,
        preflight: TwoLoopTopDotPreflight,
        integral_output: crate::LinearCombination,
    },
}

/// One cached ordinary local reduction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopT1S2LocalReduction {
    target: FourLoopT1S2LocalTarget,
    service_schema: &'static str,
    ordinary: ProductLinearCombination<MassiveVacuumMaster>,
    proof: FourLoopT1S2LocalProof,
}

impl FourLoopT1S2LocalReduction {
    pub const fn target(&self) -> &FourLoopT1S2LocalTarget {
        &self.target
    }

    pub const fn service_schema(&self) -> &'static str {
        self.service_schema
    }

    pub const fn ordinary(&self) -> &ProductLinearCombination<MassiveVacuumMaster> {
        &self.ordinary
    }

    #[doc(hidden)]
    pub fn with_output_coefficient_for_replay(
        &self,
        product: &MasterProduct<MassiveVacuumMaster>,
        coefficient: Coefficient,
    ) -> Self {
        let mut candidate = self.clone();
        candidate.ordinary = replace_product_coefficient(&candidate.ordinary, product, coefficient);
        candidate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopT1S2ComponentUse {
    witness_index: usize,
    target_index: u16,
}

impl FourLoopT1S2ComponentUse {
    pub const fn witness_index(self) -> usize {
        self.witness_index
    }

    pub const fn target_index(self) -> u16 {
        self.target_index
    }
}

/// One transported scalar branch after component reduction and checked
/// convolution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopT1S2BranchClosure {
    branch_index: usize,
    kind: FourLoopComponentScalarBranchKind,
    coefficient: Coefficient,
    component_uses: Vec<FourLoopT1S2ComponentUse>,
    convolution_pair_operations: usize,
    ordinary_unscaled: ProductLinearCombination<MassiveVacuumMaster>,
    ordinary_scaled: ProductLinearCombination<MassiveVacuumMaster>,
}

impl FourLoopT1S2BranchClosure {
    pub const fn branch_index(&self) -> usize {
        self.branch_index
    }
    pub const fn kind(&self) -> FourLoopComponentScalarBranchKind {
        self.kind
    }
    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }
    pub fn component_uses(&self) -> &[FourLoopT1S2ComponentUse] {
        &self.component_uses
    }
    pub const fn convolution_pair_operations(&self) -> usize {
        self.convolution_pair_operations
    }
    pub const fn ordinary_unscaled(&self) -> &ProductLinearCombination<MassiveVacuumMaster> {
        &self.ordinary_unscaled
    }
    pub const fn ordinary_scaled(&self) -> &ProductLinearCombination<MassiveVacuumMaster> {
        &self.ordinary_scaled
    }
}

/// Complete ordinary and mass-normalized closure of one eligible transport
/// plan.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopT1S2PlanClosure {
    leaf_id: u32,
    product_class: FourLoopT1S2ProductClass,
    branches: Vec<FourLoopT1S2BranchClosure>,
    ordinary: ProductLinearCombination<MassiveVacuumMaster>,
    mass_normalized: ProductLinearCombination<MassiveVacuumMaster>,
}

impl FourLoopT1S2PlanClosure {
    pub const fn leaf_id(&self) -> u32 {
        self.leaf_id
    }
    pub const fn product_class(&self) -> FourLoopT1S2ProductClass {
        self.product_class
    }
    pub fn branches(&self) -> &[FourLoopT1S2BranchClosure] {
        &self.branches
    }
    pub const fn ordinary(&self) -> &ProductLinearCombination<MassiveVacuumMaster> {
        &self.ordinary
    }
    pub const fn mass_normalized(&self) -> &ProductLinearCombination<MassiveVacuumMaster> {
        &self.mass_normalized
    }

    #[doc(hidden)]
    pub fn with_branch_coefficient_for_replay(
        &self,
        branch_index: usize,
        coefficient: Coefficient,
    ) -> Self {
        let mut candidate = self.clone();
        if let Some(branch) = candidate.branches.get_mut(branch_index) {
            branch.coefficient = coefficient;
        }
        candidate
    }

    #[doc(hidden)]
    pub fn with_component_target_for_replay(
        &self,
        branch_index: usize,
        component_index: usize,
        target_index: u16,
    ) -> Self {
        let mut candidate = self.clone();
        if let Some(component_use) = candidate
            .branches
            .get_mut(branch_index)
            .and_then(|branch| branch.component_uses.get_mut(component_index))
        {
            component_use.target_index = target_index;
        }
        candidate
    }

    #[doc(hidden)]
    pub fn with_mass_normalized_coefficient_for_replay(
        &self,
        product: &MasterProduct<MassiveVacuumMaster>,
        coefficient: Coefficient,
    ) -> Self {
        let mut candidate = self.clone();
        candidate.mass_normalized =
            replace_product_coefficient(&candidate.mass_normalized, product, coefficient);
        candidate
    }
}

/// One exact source occurrence and its optional completed-plan reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopT1S2ClosureOccurrence {
    row_index: u16,
    path_index: u32,
    leaf_id: u32,
    completed_plan_index: Option<u16>,
}

impl FourLoopT1S2ClosureOccurrence {
    pub const fn row_index(self) -> u16 {
        self.row_index
    }
    pub const fn path_index(self) -> u32 {
        self.path_index
    }
    pub const fn leaf_id(self) -> u32 {
        self.leaf_id
    }
    pub const fn completed_plan_index(self) -> Option<u16> {
        self.completed_plan_index
    }

    #[doc(hidden)]
    pub const fn with_completed_plan_index_for_replay(self, value: Option<u16>) -> Self {
        Self {
            completed_plan_index: value,
            ..self
        }
    }
}

/// Replayable exact closure borrowing its authenticated component transport.
pub struct FourLoopT1S2Closure<'transport, 'inventory> {
    transport: &'transport FourLoopComponentTransport<'inventory>,
    config: FourLoopT1S2ClosureConfig,
    coefficient_context: CoefficientContext,
    targets: Vec<FourLoopT1S2LocalReduction>,
    plans: Vec<FourLoopT1S2PlanClosure>,
    open_leaf_ids: Vec<u32>,
    occurrences: Vec<FourLoopT1S2ClosureOccurrence>,
    stats: FourLoopT1S2ClosureStats,
    checksum: u64,
}

impl<'transport, 'inventory> FourLoopT1S2Closure<'transport, 'inventory> {
    pub const SCHEMA: &'static str = "rustred-four-loop-t1s2-component-closure-v1";

    pub const fn config(&self) -> FourLoopT1S2ClosureConfig {
        self.config
    }
    pub const fn status(&self) -> FourLoopT1S2ClosureStatus {
        FourLoopT1S2ClosureStatus::ExactT1S2Slice
    }
    pub const fn parent_status(&self) -> FourLoopT1S2ParentStatus {
        FourLoopT1S2ParentStatus::Partial {
            completed_plans: self.stats.completed_plans,
            open_plans: self.stats.open_plans,
            completed_occurrences: self.stats.completed_occurrences,
            open_occurrences: self.stats.open_occurrences,
        }
    }
    pub const fn stats(&self) -> FourLoopT1S2ClosureStats {
        self.stats
    }
    pub const fn checksum(&self) -> u64 {
        self.checksum
    }
    pub const fn coefficient_context(&self) -> &CoefficientContext {
        &self.coefficient_context
    }
    pub(crate) const fn transport(&self) -> &'transport FourLoopComponentTransport<'inventory> {
        self.transport
    }
    pub fn targets(&self) -> &[FourLoopT1S2LocalReduction] {
        &self.targets
    }
    pub fn plans(&self) -> &[FourLoopT1S2PlanClosure] {
        &self.plans
    }
    pub fn open_leaf_ids(&self) -> &[u32] {
        &self.open_leaf_ids
    }
    pub fn occurrences(&self) -> &[FourLoopT1S2ClosureOccurrence] {
        &self.occurrences
    }

    pub fn preflight_config(
        config: FourLoopT1S2ClosureConfig,
    ) -> Result<(), FourLoopT1S2ClosureError> {
        preflight_config(config)
    }

    pub fn build(
        transport: &'transport FourLoopComponentTransport<'inventory>,
        config: FourLoopT1S2ClosureConfig,
    ) -> Result<Self, FourLoopT1S2ClosureError> {
        Self::build_impl(transport, config, true)
    }

    fn build_impl(
        transport: &'transport FourLoopComponentTransport<'inventory>,
        config: FourLoopT1S2ClosureConfig,
        authenticate_transport: bool,
    ) -> Result<Self, FourLoopT1S2ClosureError> {
        preflight_config(config)?;
        if authenticate_transport {
            transport.replay()?;
        }

        let prescan = prescan(transport, config)?;
        let services = LocalServices::new(prescan.coefficient_context.clone(), config)?;
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(prescan.target_keys.len())
            .map_err(|_| FourLoopT1S2ClosureError::AllocationFailed {
                resource: "T1/S2 target reductions",
                requested: prescan.target_keys.len(),
            })?;
        for target in &prescan.target_keys {
            let reduction = build_local_reduction(target, &services)?;
            replay_local_reduction(&reduction, &services)?;
            targets.push(reduction);
        }
        let target_indices = targets
            .iter()
            .enumerate()
            .map(|(index, reduction)| (reduction.target.clone(), index))
            .collect::<BTreeMap<_, _>>();

        let mut stats = prescan.stats;
        let mut arithmetic = CheckedArithmetic::new(
            prescan.coefficient_context.clone(),
            config.max_coefficient_operations,
            config.max_coefficient_degree,
        );
        let mut plans = Vec::new();
        plans
            .try_reserve_exact(prescan.completed_source_plan_indices.len())
            .map_err(|_| FourLoopT1S2ClosureError::AllocationFailed {
                resource: "T1/S2 plan closures",
                requested: prescan.completed_source_plan_indices.len(),
            })?;
        for &source_index in &prescan.completed_source_plan_indices {
            let source = &transport.plans()[source_index];
            plans.push(build_plan_closure(
                source,
                &targets,
                &target_indices,
                &mut arithmetic,
                &mut stats,
                config,
            )?);
        }
        stats.coefficient_operations = arithmetic.operations;
        check_actual_stats(config, stats)?;

        let completed_by_leaf = plans
            .iter()
            .enumerate()
            .map(|(index, plan)| (plan.leaf_id, index))
            .collect::<BTreeMap<_, _>>();
        let occurrences = build_occurrence_partition(transport, &completed_by_leaf, &mut stats)?;
        check_actual_stats(config, stats)?;

        stats.retained_coefficient_bytes = retained_coefficient_bytes(&targets, &plans)?;
        if stats.retained_coefficient_bytes > config.max_retained_coefficient_bytes {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource: "retained canonical coefficient bytes",
                requested: stats.retained_coefficient_bytes as u128,
                limit: config.max_retained_coefficient_bytes as u128,
            });
        }
        let checksum = closure_checksum(
            transport,
            config,
            &prescan.coefficient_context,
            &targets,
            &plans,
            &prescan.open_leaf_ids,
            &occurrences,
            stats,
        );

        Ok(Self {
            transport,
            config,
            coefficient_context: prescan.coefficient_context,
            targets,
            plans,
            open_leaf_ids: prescan.open_leaf_ids,
            occurrences,
            stats,
            checksum,
        })
    }

    /// Deterministically rebuild the complete slice, replay every local
    /// reduction against its native direct/IBP service, and compare all
    /// retained records, occurrence references, statistics, and checksum.
    pub fn replay(&self) -> Result<(), FourLoopT1S2ClosureError> {
        self.transport.replay()?;
        let rebuilt = Self::build_impl(self.transport, self.config, false)?;
        if rebuilt.targets != self.targets
            || rebuilt.plans != self.plans
            || rebuilt.open_leaf_ids != self.open_leaf_ids
            || rebuilt.occurrences != self.occurrences
            || rebuilt.stats != self.stats
            || rebuilt.checksum != self.checksum
        {
            return Err(FourLoopT1S2ClosureError::ReplayMismatch {
                leaf_id: u32::MAX,
                stage: "complete closure rebuild",
            });
        }
        Ok(())
    }

    /// Replay one bounded altered local target without cloning the batch.
    #[doc(hidden)]
    pub fn replay_target_candidate(
        &self,
        candidate: &FourLoopT1S2LocalReduction,
    ) -> Result<(), FourLoopT1S2ClosureError> {
        let services = LocalServices::new(self.coefficient_context.clone(), self.config)?;
        replay_local_reduction(candidate, &services)
    }

    /// Replay one bounded altered plan against the immutable transport and
    /// retained, independently replayed local target cache.
    #[doc(hidden)]
    pub fn replay_plan_candidate(
        &self,
        candidate: &FourLoopT1S2PlanClosure,
    ) -> Result<(), FourLoopT1S2ClosureError> {
        let source = self
            .transport
            .plans()
            .iter()
            .find(|plan| plan.leaf_id() == candidate.leaf_id)
            .ok_or(FourLoopT1S2ClosureError::ReplayMismatch {
                leaf_id: candidate.leaf_id,
                stage: "candidate source plan",
            })?;
        self.transport.replay_plan_candidate(source)?;
        let services = LocalServices::new(self.coefficient_context.clone(), self.config)?;
        for target in &self.targets {
            replay_local_reduction(target, &services)?;
        }
        let target_indices = self
            .targets
            .iter()
            .enumerate()
            .map(|(index, reduction)| (reduction.target.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let mut stats = FourLoopT1S2ClosureStats::default();
        let mut arithmetic = CheckedArithmetic::new(
            self.coefficient_context.clone(),
            self.config.max_coefficient_operations,
            self.config.max_coefficient_degree,
        );
        let expected = build_plan_closure(
            source,
            &self.targets,
            &target_indices,
            &mut arithmetic,
            &mut stats,
            self.config,
        )?;
        if &expected != candidate {
            return Err(FourLoopT1S2ClosureError::ReplayMismatch {
                leaf_id: candidate.leaf_id,
                stage: "candidate plan closure",
            });
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn replay_occurrence_candidate(
        &self,
        source_index: usize,
        candidate: FourLoopT1S2ClosureOccurrence,
    ) -> Result<(), FourLoopT1S2ClosureError> {
        let source = self.transport.occurrences().get(source_index).ok_or(
            FourLoopT1S2ClosureError::ReplayMismatch {
                leaf_id: candidate.leaf_id,
                stage: "candidate occurrence index",
            },
        )?;
        let completed = self
            .plans
            .iter()
            .position(|plan| plan.leaf_id == source.leaf_id())
            .map(|index| {
                u16::try_from(index).map_err(|_| FourLoopT1S2ClosureError::ArithmeticOverflow {
                    resource: "candidate completed plan index",
                })
            })
            .transpose()?;
        let expected = occurrence_record(*source, completed);
        if expected != candidate {
            return Err(FourLoopT1S2ClosureError::ReplayMismatch {
                leaf_id: candidate.leaf_id,
                stage: "candidate occurrence reference",
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum FourLoopT1S2ClosureError {
    Transport(FourLoopComponentTransportError),
    Family(FamilyError),
    Tadpole(OneLoopTadpoleError),
    Sunset(TwoLoopTopDotError),
    SunsetBoundary(TwoLoopBoundaryError),
    Product(MasterProductError),
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
    UnsupportedDomain {
        leaf_id: u32,
        reason: &'static str,
    },
    CoefficientContextMismatch,
    ProductOutsideSlice {
        leaf_id: u32,
        product: MasterProduct<MassiveVacuumMaster>,
    },
    ResidualMassDependence {
        leaf_id: u32,
        product: MasterProduct<MassiveVacuumMaster>,
        numerator_degree: u128,
        denominator_degree: u128,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
    ReplayMismatch {
        leaf_id: u32,
        stage: &'static str,
    },
}

impl From<FourLoopComponentTransportError> for FourLoopT1S2ClosureError {
    fn from(error: FourLoopComponentTransportError) -> Self {
        Self::Transport(error)
    }
}
impl From<FamilyError> for FourLoopT1S2ClosureError {
    fn from(error: FamilyError) -> Self {
        Self::Family(error)
    }
}
impl From<OneLoopTadpoleError> for FourLoopT1S2ClosureError {
    fn from(error: OneLoopTadpoleError) -> Self {
        Self::Tadpole(error)
    }
}
impl From<TwoLoopTopDotError> for FourLoopT1S2ClosureError {
    fn from(error: TwoLoopTopDotError) -> Self {
        Self::Sunset(error)
    }
}
impl From<TwoLoopBoundaryError> for FourLoopT1S2ClosureError {
    fn from(error: TwoLoopBoundaryError) -> Self {
        Self::SunsetBoundary(error)
    }
}
impl From<MasterProductError> for FourLoopT1S2ClosureError {
    fn from(error: MasterProductError) -> Self {
        Self::Product(error)
    }
}

impl fmt::Display for FourLoopT1S2ClosureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "four-loop T1/S2 closure: {self:?}")
    }
}
impl Error for FourLoopT1S2ClosureError {}

struct Prescan {
    coefficient_context: CoefficientContext,
    completed_source_plan_indices: Vec<usize>,
    open_leaf_ids: Vec<u32>,
    target_keys: Vec<FourLoopT1S2LocalTarget>,
    stats: FourLoopT1S2ClosureStats,
}

fn preflight_config(config: FourLoopT1S2ClosureConfig) -> Result<(), FourLoopT1S2ClosureError> {
    let minima = [
        (
            "T1/S2 completed plans",
            config.max_plans,
            FOUR_LOOP_T1S2_CLOSURE_PLANS,
        ),
        (
            "T1/S2 open plans",
            config.max_open_plans,
            FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS,
        ),
        (
            "T1/S2 occurrence partition",
            config.max_occurrences,
            FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES,
        ),
        (
            "T1/S2 components",
            config.max_components,
            FOUR_LOOP_T1S2_CLOSURE_COMPONENTS,
        ),
        (
            "T1/S2 local slots",
            config.max_local_slots,
            FOUR_LOOP_T1S2_CLOSURE_LOCAL_SLOTS,
        ),
        (
            "T1/S2 scalar branches",
            config.max_scalar_branches,
            FOUR_LOOP_T1S2_CLOSURE_SCALAR_BRANCHES,
        ),
        (
            "T1/S2 component calls",
            config.max_component_calls,
            FOUR_LOOP_T1S2_CLOSURE_COMPONENT_CALLS,
        ),
        (
            "T1/S2 unique targets",
            config.max_unique_targets,
            FOUR_LOOP_T1S2_CLOSURE_UNIQUE_TARGETS,
        ),
        (
            "T1/S2 convolution pairs",
            config.max_convolution_pair_operations,
            FOUR_LOOP_T1S2_CLOSURE_CONVOLUTION_PAIRS,
        ),
        (
            "T1/S2 precollection terms",
            config.max_precollection_terms,
            FOUR_LOOP_T1S2_CLOSURE_PRECOLLECTION_TERMS,
        ),
        (
            "T1/S2 collected terms",
            config.max_collected_terms,
            FOUR_LOOP_T1S2_CLOSURE_COLLECTED_TERMS,
        ),
        (
            "T1/S2 mass-power steps",
            config.max_mass_power_steps,
            FOUR_LOOP_T1S2_CLOSURE_MASS_POWER_STEPS,
        ),
        (
            "T1/S2 coefficient operations",
            config.max_coefficient_operations,
            FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_OPERATIONS,
        ),
        (
            "T1/S2 retained coefficient bytes",
            config.max_retained_coefficient_bytes,
            FOUR_LOOP_T1S2_CLOSURE_RETAINED_COEFFICIENT_BYTES,
        ),
    ];
    for (resource, actual, minimum) in minima {
        if actual < minimum {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource,
                requested: minimum as u128,
                limit: actual as u128,
            });
        }
    }
    if config.max_coefficient_degree < FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_DEGREE {
        return Err(FourLoopT1S2ClosureError::ResourceLimit {
            resource: "T1/S2 coefficient degree",
            requested: FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_DEGREE,
            limit: config.max_coefficient_degree,
        });
    }
    if config.max_coefficient_degree > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(FourLoopT1S2ClosureError::ResourceLimit {
            resource: "configured T1/S2 coefficient degree",
            requested: config.max_coefficient_degree,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }
    for (resource, actual, minimum) in [
        (
            "nested T1 recurrence steps",
            config.one_loop.max_recurrence_steps,
            2,
        ),
        (
            "nested T1 coefficient operations",
            config.one_loop.max_coefficient_operations,
            8,
        ),
    ] {
        if actual < minimum {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource,
                requested: minimum as u128,
                limit: actual as u128,
            });
        }
    }
    if config.one_loop.max_dense_term_operations < 24 {
        return Err(FourLoopT1S2ClosureError::ResourceLimit {
            resource: "nested T1 dense term operations",
            requested: 24,
            limit: config.one_loop.max_dense_term_operations,
        });
    }
    if config.one_loop.max_coefficient_degree < 2 {
        return Err(FourLoopT1S2ClosureError::ResourceLimit {
            resource: "nested T1 coefficient degree",
            requested: 2,
            limit: config.one_loop.max_coefficient_degree,
        });
    }
    if config.one_loop.max_coefficient_degree > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(FourLoopT1S2ClosureError::ResourceLimit {
            resource: "configured nested T1 coefficient degree",
            requested: config.one_loop.max_coefficient_degree,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }
    for (resource, actual, minimum) in [
        (
            "nested S2 explicit recurrence terms",
            config.two_loop.max_explicit_terms,
            TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND,
        ),
        (
            "nested S2 native provenance terms",
            config.two_loop.max_raw_terms,
            TWO_LOOP_TOP_DOT_RAW_TERM_BOUND,
        ),
        (
            "nested S2 normal-form states",
            config.two_loop.max_states,
            28,
        ),
        (
            "nested S2 coefficient operations",
            config.two_loop.max_coefficient_operations,
            200,
        ),
        (
            "nested S2 boundary iterations",
            config.two_loop.max_boundary_formula_iterations,
            5,
        ),
    ] {
        if actual < minimum {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource,
                requested: minimum as u128,
                limit: actual as u128,
            });
        }
    }
    if config.two_loop.max_coefficient_degree < 6 {
        return Err(FourLoopT1S2ClosureError::ResourceLimit {
            resource: "nested S2 coefficient degree",
            requested: 6,
            limit: config.two_loop.max_coefficient_degree,
        });
    }
    if config.two_loop.max_coefficient_degree > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(FourLoopT1S2ClosureError::ResourceLimit {
            resource: "configured nested S2 coefficient degree",
            requested: config.two_loop.max_coefficient_degree,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }
    Ok(())
}

fn prescan(
    transport: &FourLoopComponentTransport<'_>,
    config: FourLoopT1S2ClosureConfig,
) -> Result<Prescan, FourLoopT1S2ClosureError> {
    if transport.plans().len() != FOUR_LOOP_T1S2_CLOSURE_PLANS + FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS {
        return Err(FourLoopT1S2ClosureError::CensusMismatch {
            resource: "source transport plans",
            expected: FOUR_LOOP_T1S2_CLOSURE_PLANS + FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS,
            actual: transport.plans().len(),
        });
    }
    if transport.occurrences().len() != FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES {
        return Err(FourLoopT1S2ClosureError::CensusMismatch {
            resource: "source transport occurrences",
            expected: FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES,
            actual: transport.occurrences().len(),
        });
    }

    let mut completed_source_plan_indices = Vec::new();
    completed_source_plan_indices
        .try_reserve_exact(FOUR_LOOP_T1S2_CLOSURE_PLANS)
        .map_err(|_| FourLoopT1S2ClosureError::AllocationFailed {
            resource: "completed source plan indices",
            requested: FOUR_LOOP_T1S2_CLOSURE_PLANS,
        })?;
    let mut open_leaf_ids = Vec::new();
    open_leaf_ids
        .try_reserve_exact(FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS)
        .map_err(|_| FourLoopT1S2ClosureError::AllocationFailed {
            resource: "open source leaf ids",
            requested: FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS,
        })?;
    let mut targets = BTreeSet::new();
    let mut stats = FourLoopT1S2ClosureStats::default();
    let mut context = None::<CoefficientContext>;
    let mut class_counts = BTreeMap::<FourLoopT1S2ProductClass, usize>::new();

    for (source_index, plan) in transport.plans().iter().enumerate() {
        let Some(product_class) = classify_product(plan.key().product()) else {
            open_leaf_ids.push(plan.leaf_id());
            continue;
        };
        *class_counts.entry(product_class).or_default() += 1;
        completed_source_plan_indices.push(source_index);
        let (key, family) = transport.authenticated_source_context(plan.leaf_id())?;
        if key != plan.key() {
            return Err(FourLoopT1S2ClosureError::ReplayMismatch {
                leaf_id: plan.leaf_id(),
                stage: "prescan authenticated key",
            });
        }
        validate_parent_context(plan.leaf_id(), family, context.as_ref())?;
        context.get_or_insert_with(|| family.coefficients().clone());

        stats.components = checked_sum(stats.components, plan.components().len(), "components")?;
        stats.local_slots = checked_sum(
            stats.local_slots,
            plan.components()
                .iter()
                .map(|component| component.local_powers().len())
                .sum(),
            "local slots",
        )?;
        stats.scalar_branches = checked_sum(
            stats.scalar_branches,
            plan.scalar_branches().len(),
            "scalar branches",
        )?;
        if plan.affine_image().is_some() {
            stats.n1_plans = checked_sum(stats.n1_plans, 1, "N1 plans")?;
        } else {
            stats.n0_plans = checked_sum(stats.n0_plans, 1, "N0 plans")?;
        }
        for branch in plan.scalar_branches() {
            match branch.kind() {
                FourLoopComponentScalarBranchKind::Base => {
                    stats.base_branches = checked_sum(stats.base_branches, 1, "base branches")?;
                }
                FourLoopComponentScalarBranchKind::Constant => {
                    stats.constant_branches =
                        checked_sum(stats.constant_branches, 1, "constant branches")?;
                }
                FourLoopComponentScalarBranchKind::Local {
                    component_index, ..
                } => {
                    let master = plan
                        .components()
                        .get(component_index)
                        .ok_or(FourLoopT1S2ClosureError::ReplayMismatch {
                            leaf_id: plan.leaf_id(),
                            stage: "local branch owner in prescan",
                        })?
                        .master();
                    match master {
                        MassiveVacuumMaster::T1 => {
                            stats.local_t1_branches =
                                checked_sum(stats.local_t1_branches, 1, "local T1 branches")?;
                        }
                        MassiveVacuumMaster::S2 => {
                            stats.local_s2_branches =
                                checked_sum(stats.local_s2_branches, 1, "local S2 branches")?;
                        }
                        _ => {
                            return Err(FourLoopT1S2ClosureError::UnsupportedDomain {
                                leaf_id: plan.leaf_id(),
                                reason: "eligible product contains a non-T1/S2 local branch",
                            });
                        }
                    }
                }
            }
            let branch_targets = reconstruct_branch_targets(plan, branch)?;
            stats.component_calls = checked_sum(
                stats.component_calls,
                branch_targets.len(),
                "component calls",
            )?;
            for target in branch_targets {
                match target.master {
                    MassiveVacuumMaster::T1 => {
                        stats.t1_component_calls =
                            checked_sum(stats.t1_component_calls, 1, "T1 component calls")?;
                    }
                    MassiveVacuumMaster::S2 => {
                        stats.s2_component_calls =
                            checked_sum(stats.s2_component_calls, 1, "S2 component calls")?;
                    }
                    _ => unreachable!("eligible targets are T1/S2 only"),
                }
                validate_local_target(plan.leaf_id(), &target)?;
                targets.insert(target);
            }
        }
    }

    let expected_classes = BTreeMap::from([
        (FourLoopT1S2ProductClass::S2Squared, 52),
        (FourLoopT1S2ProductClass::T1SquaredS2, 91),
        (FourLoopT1S2ProductClass::T1Fourth, 100),
    ]);
    if class_counts != expected_classes {
        return Err(FourLoopT1S2ClosureError::ReplayMismatch {
            leaf_id: u32::MAX,
            stage: "T1/S2 product census",
        });
    }
    stats.completed_plans = completed_source_plan_indices.len();
    stats.open_plans = open_leaf_ids.len();
    stats.unique_targets = targets.len();
    stats.t1_targets = targets
        .iter()
        .filter(|target| target.master == MassiveVacuumMaster::T1)
        .count();
    stats.s2_targets = targets.len() - stats.t1_targets;
    stats.cache_hits = stats
        .component_calls
        .checked_sub(stats.unique_targets)
        .ok_or(FourLoopT1S2ClosureError::ArithmeticOverflow {
            resource: "target cache hits",
        })?;
    check_exact_structural_stats(stats)?;
    check_actual_stats(config, stats)?;
    let coefficient_context = context.ok_or(FourLoopT1S2ClosureError::ReplayMismatch {
        leaf_id: u32::MAX,
        stage: "missing completed coefficient context",
    })?;
    Ok(Prescan {
        coefficient_context,
        completed_source_plan_indices,
        open_leaf_ids,
        target_keys: targets.into_iter().collect(),
        stats,
    })
}

fn validate_parent_context(
    leaf_id: u32,
    family: &VacuumFamily,
    first: Option<&CoefficientContext>,
) -> Result<(), FourLoopT1S2ClosureError> {
    let context = family.coefficients();
    if !context
        .parameter_names()
        .iter()
        .map(String::as_str)
        .eq(["d", "m2"])
        || context.parameter("d").as_ref() != Some(family.dimension())
        || context.parameter("m2").is_none_or(|mass| mass.is_zero())
    {
        return Err(FourLoopT1S2ClosureError::UnsupportedDomain {
            leaf_id,
            reason: "parent is not the authenticated Q(d,m2) equal-mass domain",
        });
    }
    if first.is_some_and(|candidate| !candidate.has_same_variable_map(context)) {
        return Err(FourLoopT1S2ClosureError::CoefficientContextMismatch);
    }
    Ok(())
}

fn classify_product(
    product: &MasterProduct<MassiveVacuumMaster>,
) -> Option<FourLoopT1S2ProductClass> {
    let only = |allowed: &[MassiveVacuumMaster]| {
        product
            .factors()
            .keys()
            .all(|master| allowed.contains(master))
    };
    if only(&[MassiveVacuumMaster::T1]) && product.multiplicity(&MassiveVacuumMaster::T1) == 4 {
        Some(FourLoopT1S2ProductClass::T1Fourth)
    } else if only(&[MassiveVacuumMaster::T1, MassiveVacuumMaster::S2])
        && product.multiplicity(&MassiveVacuumMaster::T1) == 2
        && product.multiplicity(&MassiveVacuumMaster::S2) == 1
    {
        Some(FourLoopT1S2ProductClass::T1SquaredS2)
    } else if only(&[MassiveVacuumMaster::S2])
        && product.multiplicity(&MassiveVacuumMaster::S2) == 2
    {
        Some(FourLoopT1S2ProductClass::S2Squared)
    } else {
        None
    }
}

fn reconstruct_branch_targets(
    plan: &FourLoopComponentTransportPlan,
    branch: &crate::FourLoopComponentScalarBranch,
) -> Result<Vec<FourLoopT1S2LocalTarget>, FourLoopT1S2ClosureError> {
    let mut targets = plan
        .components()
        .iter()
        .map(|component| FourLoopT1S2LocalTarget {
            master: component.master(),
            powers: component.local_powers().to_vec(),
        })
        .collect::<Vec<_>>();
    match branch.kind() {
        FourLoopComponentScalarBranchKind::Base | FourLoopComponentScalarBranchKind::Constant => {
            if branch.lowered_component_powers().is_some() {
                return Err(FourLoopT1S2ClosureError::ReplayMismatch {
                    leaf_id: plan.leaf_id(),
                    stage: "base/constant branch lowering",
                });
            }
        }
        FourLoopComponentScalarBranchKind::Local {
            component_index,
            local_position,
        } => {
            let lowered = branch.lowered_component_powers().ok_or(
                FourLoopT1S2ClosureError::ReplayMismatch {
                    leaf_id: plan.leaf_id(),
                    stage: "local branch lowering",
                },
            )?;
            let component = plan.components().get(component_index).ok_or(
                FourLoopT1S2ClosureError::ReplayMismatch {
                    leaf_id: plan.leaf_id(),
                    stage: "local branch component",
                },
            )?;
            if lowered.len() != component.local_powers().len()
                || local_position >= lowered.len()
                || lowered.iter().enumerate().any(|(position, &power)| {
                    power
                        != component.local_powers()[position]
                            - i32::from(position == local_position)
                })
            {
                return Err(FourLoopT1S2ClosureError::ReplayMismatch {
                    leaf_id: plan.leaf_id(),
                    stage: "local branch exact shift",
                });
            }
            targets[component_index].powers = lowered.to_vec();
        }
    }
    Ok(targets)
}

fn validate_local_target(
    leaf_id: u32,
    target: &FourLoopT1S2LocalTarget,
) -> Result<(), FourLoopT1S2ClosureError> {
    match target.master {
        MassiveVacuumMaster::T1 => {
            if target.powers.len() != 1 || !(0..=3).contains(&target.powers[0]) {
                return Err(FourLoopT1S2ClosureError::UnsupportedDomain {
                    leaf_id,
                    reason: "T1 target is outside the exact scalar power 0..=3 domain",
                });
            }
        }
        MassiveVacuumMaster::S2 => {
            if target.powers.len() != 3
                || target.powers.iter().any(|power| *power < 0)
                || !(2..=3).contains(&target.powers.iter().filter(|power| **power > 0).count())
                || target
                    .powers
                    .iter()
                    .map(|power| (*power - 1).max(0))
                    .sum::<i32>()
                    > 2
            {
                return Err(FourLoopT1S2ClosureError::UnsupportedDomain {
                    leaf_id,
                    reason: "S2 target is outside the exact scalar D2/one-pinch domain",
                });
            }
        }
        _ => {
            return Err(FourLoopT1S2ClosureError::UnsupportedDomain {
                leaf_id,
                reason: "T1/S2 slice received a three-loop component",
            });
        }
    }
    Ok(())
}

struct LocalServices {
    tadpole: OneLoopTadpoleReducer,
    sunset: TwoLoopTopDotReducer,
}

impl LocalServices {
    fn new(
        context: CoefficientContext,
        config: FourLoopT1S2ClosureConfig,
    ) -> Result<Self, FourLoopT1S2ClosureError> {
        let tadpole = OneLoopTadpoleReducer::new(context.clone(), "d", "m2", config.one_loop)?;
        let sunset_family = equal_mass_two_loop_vacuum_in_context(context)?;
        let sunset = TwoLoopTopDotReducer::new(sunset_family, config.two_loop)?;
        if !tadpole
            .coefficients()
            .has_same_variable_map(sunset.family().coefficients())
        {
            return Err(FourLoopT1S2ClosureError::CoefficientContextMismatch);
        }
        Ok(Self { tadpole, sunset })
    }
}

fn build_local_reduction(
    target: &FourLoopT1S2LocalTarget,
    services: &LocalServices,
) -> Result<FourLoopT1S2LocalReduction, FourLoopT1S2ClosureError> {
    match target.master {
        MassiveVacuumMaster::T1 => {
            let reduction = services.tadpole.reduce_power(target.powers[0])?;
            let mut ordinary = ProductLinearCombination::new();
            if !reduction.coefficient().is_zero() {
                ordinary.add_term(
                    MasterProduct::from_factor(MassiveVacuumMaster::T1),
                    reduction.coefficient().clone(),
                );
            }
            check_local_loop_weight(target, &ordinary)?;
            Ok(FourLoopT1S2LocalReduction {
                target: target.clone(),
                service_schema: OneLoopTadpoleReducer::SCHEMA,
                ordinary,
                proof: FourLoopT1S2LocalProof::Tadpole(reduction),
            })
        }
        MassiveVacuumMaster::S2 => {
            let requested = Integral::new(target.powers.clone());
            let preflight = services.sunset.preflight(&requested)?;
            let oriented = services
                .sunset
                .family()
                .try_canonicalize(&requested)?
                .ok_or(FourLoopT1S2ClosureError::UnsupportedDomain {
                    leaf_id: u32::MAX,
                    reason: "S2 target canonicalized to a zero sector",
                })?;
            let integral_output = services.sunset.reduce_integral(&requested)?;
            let ordinary = adapt_sunset_output(&services.sunset, &integral_output)?;
            check_local_loop_weight(target, &ordinary)?;
            Ok(FourLoopT1S2LocalReduction {
                target: target.clone(),
                service_schema: "rustred-two-loop-top-dot-semantic-adapter-v1",
                ordinary,
                proof: FourLoopT1S2LocalProof::Sunset {
                    requested,
                    oriented,
                    preflight,
                    integral_output,
                },
            })
        }
        _ => Err(FourLoopT1S2ClosureError::UnsupportedDomain {
            leaf_id: u32::MAX,
            reason: "local service only accepts T1 or S2",
        }),
    }
}

fn replay_local_reduction(
    retained: &FourLoopT1S2LocalReduction,
    services: &LocalServices,
) -> Result<(), FourLoopT1S2ClosureError> {
    validate_local_target(u32::MAX, &retained.target)?;
    let rebuilt = build_local_reduction(&retained.target, services)?;
    match &rebuilt.proof {
        FourLoopT1S2LocalProof::Tadpole(reduction) => {
            services.tadpole.replay(reduction)?;
        }
        FourLoopT1S2LocalProof::Sunset {
            requested,
            integral_output,
            ..
        } => {
            if requested.powers().iter().all(|power| *power > 0)
                && requested != services.sunset.sunset_master()
            {
                services.sunset.validate_raw_ibp_provenance(requested)?;
            } else if requested.denominator_count() == 2 {
                let boundary = TwoLoopBoundaryReducer::new(services.sunset.family())?;
                if boundary.reduce_integral(requested)? != *integral_output {
                    return Err(FourLoopT1S2ClosureError::ReplayMismatch {
                        leaf_id: u32::MAX,
                        stage: "S2 boundary direct formula",
                    });
                }
            }
        }
    }
    if &rebuilt != retained {
        return Err(FourLoopT1S2ClosureError::ReplayMismatch {
            leaf_id: u32::MAX,
            stage: "local target reduction",
        });
    }
    Ok(())
}

fn adapt_sunset_output(
    reducer: &TwoLoopTopDotReducer,
    output: &crate::LinearCombination,
) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopT1S2ClosureError> {
    let mut adapted = ProductLinearCombination::new();
    for (integral, coefficient) in output.terms() {
        let product = if integral == reducer.sunset_master() {
            MasterProduct::from_factor(MassiveVacuumMaster::S2)
        } else if integral == reducer.product_master() {
            MasterProduct::try_from_multiplicities([(MassiveVacuumMaster::T1, 2)])?
        } else {
            return Err(FourLoopT1S2ClosureError::UnsupportedDomain {
                leaf_id: u32::MAX,
                reason: "two-loop service returned an unknown semantic terminal",
            });
        };
        if adapted.coefficient(&product).is_some() {
            return Err(FourLoopT1S2ClosureError::ReplayMismatch {
                leaf_id: u32::MAX,
                stage: "duplicate two-loop semantic terminal",
            });
        }
        adapted.add_term(product, coefficient.clone());
    }
    Ok(adapted)
}

fn check_local_loop_weight(
    target: &FourLoopT1S2LocalTarget,
    output: &ProductLinearCombination<MassiveVacuumMaster>,
) -> Result<(), FourLoopT1S2ClosureError> {
    let expected = target.master.loops() as u128;
    if output
        .terms()
        .keys()
        .any(|product| product_loop_weight(product) != expected)
    {
        return Err(FourLoopT1S2ClosureError::UnsupportedDomain {
            leaf_id: u32::MAX,
            reason: "local semantic output does not conserve loop count",
        });
    }
    Ok(())
}

fn build_plan_closure(
    source: &FourLoopComponentTransportPlan,
    targets: &[FourLoopT1S2LocalReduction],
    target_indices: &BTreeMap<FourLoopT1S2LocalTarget, usize>,
    arithmetic: &mut CheckedArithmetic,
    stats: &mut FourLoopT1S2ClosureStats,
    config: FourLoopT1S2ClosureConfig,
) -> Result<FourLoopT1S2PlanClosure, FourLoopT1S2ClosureError> {
    let product_class = classify_product(source.key().product()).ok_or(
        FourLoopT1S2ClosureError::UnsupportedDomain {
            leaf_id: source.leaf_id(),
            reason: "candidate plan is outside T1/S2 product closure",
        },
    )?;
    let mut branches = Vec::new();
    branches
        .try_reserve_exact(source.scalar_branches().len())
        .map_err(|_| FourLoopT1S2ClosureError::AllocationFailed {
            resource: "plan branch closures",
            requested: source.scalar_branches().len(),
        })?;
    let mut ordinary = ProductLinearCombination::new();
    for (branch_index, source_branch) in source.scalar_branches().iter().enumerate() {
        let branch_targets = reconstruct_branch_targets(source, source_branch)?;
        let mut component_uses = Vec::new();
        component_uses
            .try_reserve_exact(branch_targets.len())
            .map_err(|_| FourLoopT1S2ClosureError::AllocationFailed {
                resource: "branch component uses",
                requested: branch_targets.len(),
            })?;
        let mut unscaled = ProductLinearCombination::from_term(
            MasterProduct::identity(),
            arithmetic.context.one(),
        );
        let before_pairs = stats.convolution_pair_operations;
        for (witness_index, target) in branch_targets.into_iter().enumerate() {
            let &target_index =
                target_indices
                    .get(&target)
                    .ok_or(FourLoopT1S2ClosureError::ReplayMismatch {
                        leaf_id: source.leaf_id(),
                        stage: "component target cache lookup",
                    })?;
            let target_index_u16 = u16::try_from(target_index).map_err(|_| {
                FourLoopT1S2ClosureError::ArithmeticOverflow {
                    resource: "component target index",
                }
            })?;
            component_uses.push(FourLoopT1S2ComponentUse {
                witness_index,
                target_index: target_index_u16,
            });
            unscaled = checked_convolve(
                &unscaled,
                &targets[target_index].ordinary,
                arithmetic,
                stats,
                config,
            )?;
        }
        for product in unscaled.terms().keys() {
            if product_loop_weight(product) != 4 || !is_allowed_slice_product(product) {
                return Err(FourLoopT1S2ClosureError::ProductOutsideSlice {
                    leaf_id: source.leaf_id(),
                    product: product.clone(),
                });
            }
        }
        stats.precollection_terms = checked_sum(
            stats.precollection_terms,
            unscaled.len(),
            "branch precollection terms",
        )?;
        let scaled = checked_scale(&unscaled, source_branch.coefficient(), arithmetic)?;
        checked_add_combination(&mut ordinary, &scaled, arithmetic)?;
        branches.push(FourLoopT1S2BranchClosure {
            branch_index,
            kind: source_branch.kind(),
            coefficient: source_branch.coefficient().clone(),
            component_uses,
            convolution_pair_operations: stats
                .convolution_pair_operations
                .checked_sub(before_pairs)
                .ok_or(FourLoopT1S2ClosureError::ArithmeticOverflow {
                    resource: "branch convolution pair delta",
                })?,
            ordinary_unscaled: unscaled,
            ordinary_scaled: scaled,
        });
    }
    for product in ordinary.terms().keys() {
        if product_loop_weight(product) != 4 || !is_allowed_slice_product(product) {
            return Err(FourLoopT1S2ClosureError::ProductOutsideSlice {
                leaf_id: source.leaf_id(),
                product: product.clone(),
            });
        }
    }
    stats.collected_terms = checked_sum(stats.collected_terms, ordinary.len(), "collected terms")?;
    let mass_normalized = mass_normalize(source, &ordinary, arithmetic, stats, config)?;
    Ok(FourLoopT1S2PlanClosure {
        leaf_id: source.leaf_id(),
        product_class,
        branches,
        ordinary,
        mass_normalized,
    })
}

fn checked_convolve(
    left: &ProductLinearCombination<MassiveVacuumMaster>,
    right: &ProductLinearCombination<MassiveVacuumMaster>,
    arithmetic: &mut CheckedArithmetic,
    stats: &mut FourLoopT1S2ClosureStats,
    config: FourLoopT1S2ClosureConfig,
) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopT1S2ClosureError> {
    let pairs = left.len().checked_mul(right.len()).ok_or(
        FourLoopT1S2ClosureError::ArithmeticOverflow {
            resource: "convolution pair operations",
        },
    )?;
    stats.convolution_pair_operations = checked_sum(
        stats.convolution_pair_operations,
        pairs,
        "convolution pair operations",
    )?;
    if stats.convolution_pair_operations > config.max_convolution_pair_operations {
        return Err(FourLoopT1S2ClosureError::ResourceLimit {
            resource: "T1/S2 convolution pairs",
            requested: stats.convolution_pair_operations as u128,
            limit: config.max_convolution_pair_operations as u128,
        });
    }
    let mut output = ProductLinearCombination::new();
    for (left_product, left_coefficient) in left.terms() {
        for (right_product, right_coefficient) in right.terms() {
            let product = left_product.checked_multiply(right_product)?;
            let coefficient = arithmetic.multiply(left_coefficient, right_coefficient)?;
            checked_add_product_term(&mut output, product, coefficient, arithmetic)?;
            if output.len() > 3 {
                return Err(FourLoopT1S2ClosureError::ResourceLimit {
                    resource: "intermediate T1/S2 products",
                    requested: output.len() as u128,
                    limit: 3,
                });
            }
        }
    }
    Ok(output)
}

fn checked_scale(
    input: &ProductLinearCombination<MassiveVacuumMaster>,
    factor: &Coefficient,
    arithmetic: &mut CheckedArithmetic,
) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopT1S2ClosureError> {
    let mut output = ProductLinearCombination::new();
    if factor.is_zero() {
        return Ok(output);
    }
    for (product, coefficient) in input.terms() {
        output.add_term(product.clone(), arithmetic.multiply(coefficient, factor)?);
    }
    Ok(output)
}

fn checked_add_combination(
    output: &mut ProductLinearCombination<MassiveVacuumMaster>,
    input: &ProductLinearCombination<MassiveVacuumMaster>,
    arithmetic: &mut CheckedArithmetic,
) -> Result<(), FourLoopT1S2ClosureError> {
    for (product, coefficient) in input.terms() {
        checked_add_product_term(output, product.clone(), coefficient.clone(), arithmetic)?;
    }
    Ok(())
}

fn checked_add_product_term(
    output: &mut ProductLinearCombination<MassiveVacuumMaster>,
    product: MasterProduct<MassiveVacuumMaster>,
    coefficient: Coefficient,
    arithmetic: &mut CheckedArithmetic,
) -> Result<(), FourLoopT1S2ClosureError> {
    if coefficient.is_zero() {
        return Ok(());
    }
    let value = if let Some(current) = output.remove(&product) {
        arithmetic.add(&current, &coefficient)?
    } else {
        coefficient
    };
    output.add_term(product, value);
    Ok(())
}

fn mass_normalize(
    source: &FourLoopComponentTransportPlan,
    ordinary: &ProductLinearCombination<MassiveVacuumMaster>,
    arithmetic: &mut CheckedArithmetic,
    stats: &mut FourLoopT1S2ClosureStats,
    config: FourLoopT1S2ClosureConfig,
) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopT1S2ClosureError> {
    let input_weight = source
        .key()
        .powers()
        .iter()
        .try_fold(0_i64, |sum, &power| sum.checked_add(i64::from(power)))
        .ok_or(FourLoopT1S2ClosureError::ArithmeticOverflow {
            resource: "parent mass weight",
        })?;
    let mass_position = arithmetic
        .context
        .parameter_names()
        .iter()
        .position(|name| name == "m2")
        .ok_or(FourLoopT1S2ClosureError::CoefficientContextMismatch)?;
    let mass = arithmetic
        .context
        .parameter("m2")
        .ok_or(FourLoopT1S2ClosureError::CoefficientContextMismatch)?;
    arithmetic.charge()?;
    let inverse_mass = &arithmetic.context.one() / &mass;
    arithmetic.check_existing(&inverse_mass)?;
    let mut normalized = ProductLinearCombination::new();
    for (product, coefficient) in ordinary.terms() {
        let exponent = input_weight
            .checked_sub(product_mass_weight(product))
            .ok_or(FourLoopT1S2ClosureError::ArithmeticOverflow {
                resource: "mass-normalization exponent",
            })?;
        let steps = usize::try_from(exponent.unsigned_abs()).map_err(|_| {
            FourLoopT1S2ClosureError::ArithmeticOverflow {
                resource: "mass-normalization steps",
            }
        })?;
        stats.mass_power_steps = checked_sum(stats.mass_power_steps, steps, "mass-power steps")?;
        if stats.mass_power_steps > config.max_mass_power_steps {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource: "T1/S2 mass-power steps",
                requested: stats.mass_power_steps as u128,
                limit: config.max_mass_power_steps as u128,
            });
        }
        let factor = if exponent >= 0 { &mass } else { &inverse_mass };
        let mut value = coefficient.clone();
        for _ in 0..steps {
            value = arithmetic.multiply(&value, factor)?;
        }
        let (numerator_degree, denominator_degree) = coefficient_variable_degrees(&value)
            .get(mass_position)
            .copied()
            .ok_or(FourLoopT1S2ClosureError::CoefficientContextMismatch)?;
        if numerator_degree != 0 || denominator_degree != 0 {
            return Err(FourLoopT1S2ClosureError::ResidualMassDependence {
                leaf_id: source.leaf_id(),
                product: product.clone(),
                numerator_degree,
                denominator_degree,
            });
        }
        normalized.add_term(product.clone(), value);
    }
    Ok(normalized)
}

struct CheckedArithmetic {
    context: CoefficientContext,
    zero: Coefficient,
    max_operations: usize,
    max_degree: u128,
    operations: usize,
}

impl CheckedArithmetic {
    fn new(context: CoefficientContext, max_operations: usize, max_degree: u128) -> Self {
        let zero = context.zero();
        Self {
            context,
            zero,
            max_operations,
            max_degree,
            operations: 0,
        }
    }

    fn charge(&mut self) -> Result<(), FourLoopT1S2ClosureError> {
        self.operations =
            self.operations
                .checked_add(1)
                .ok_or(FourLoopT1S2ClosureError::ArithmeticOverflow {
                    resource: "coefficient operations",
                })?;
        if self.operations > self.max_operations {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource: "T1/S2 coefficient operations",
                requested: self.operations as u128,
                limit: self.max_operations as u128,
            });
        }
        Ok(())
    }

    fn check_existing(&self, value: &Coefficient) -> Result<(), FourLoopT1S2ClosureError> {
        if value.get_variables() != self.zero.get_variables() {
            return Err(FourLoopT1S2ClosureError::CoefficientContextMismatch);
        }
        let degree = coefficient_variable_degrees(value)
            .into_iter()
            .map(|(numerator, denominator)| numerator.max(denominator))
            .max()
            .unwrap_or(0);
        if degree > self.max_degree || !symbolica_coefficient_degree_is_representable(degree) {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource: "T1/S2 coefficient degree",
                requested: degree,
                limit: self.max_degree.min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT),
            });
        }
        Ok(())
    }

    fn multiply(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopT1S2ClosureError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        let requested = coefficient_product_degree_bound(left, right);
        if requested > self.max_degree || !symbolica_coefficient_degree_is_representable(requested)
        {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource: "T1/S2 coefficient product degree",
                requested,
                limit: self.max_degree.min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT),
            });
        }
        self.charge()?;
        let output = left * right;
        self.check_existing(&output)?;
        Ok(output)
    }

    fn add(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopT1S2ClosureError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        let requested = coefficient_sum_degree_bound(left, right);
        if requested > self.max_degree || !symbolica_coefficient_degree_is_representable(requested)
        {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource: "T1/S2 coefficient sum degree",
                requested,
                limit: self.max_degree.min(SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT),
            });
        }
        self.charge()?;
        let output = left + right;
        self.check_existing(&output)?;
        Ok(output)
    }
}

fn build_occurrence_partition(
    transport: &FourLoopComponentTransport<'_>,
    completed_by_leaf: &BTreeMap<u32, usize>,
    stats: &mut FourLoopT1S2ClosureStats,
) -> Result<Vec<FourLoopT1S2ClosureOccurrence>, FourLoopT1S2ClosureError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(transport.occurrences().len())
        .map_err(|_| FourLoopT1S2ClosureError::AllocationFailed {
            resource: "T1/S2 occurrence partition",
            requested: transport.occurrences().len(),
        })?;
    let mut completed_rows = BTreeSet::new();
    let mut open_rows = BTreeSet::new();
    for source in transport.occurrences() {
        let plan = transport.plans().get(source.plan_index() as usize).ok_or(
            FourLoopT1S2ClosureError::ReplayMismatch {
                leaf_id: source.leaf_id(),
                stage: "occurrence source plan index",
            },
        )?;
        if plan.leaf_id() != source.leaf_id() {
            return Err(FourLoopT1S2ClosureError::ReplayMismatch {
                leaf_id: source.leaf_id(),
                stage: "occurrence source leaf",
            });
        }
        let completed = completed_by_leaf
            .get(&source.leaf_id())
            .copied()
            .map(|index| {
                u16::try_from(index).map_err(|_| FourLoopT1S2ClosureError::ArithmeticOverflow {
                    resource: "occurrence completed plan index",
                })
            })
            .transpose()?;
        if completed.is_some() {
            stats.completed_occurrences =
                checked_sum(stats.completed_occurrences, 1, "completed occurrences")?;
            completed_rows.insert(source.row_index());
        } else {
            stats.open_occurrences = checked_sum(stats.open_occurrences, 1, "open occurrences")?;
            open_rows.insert(source.row_index());
        }
        output.push(occurrence_record(*source, completed));
    }
    stats.completed_rows = completed_rows.len();
    stats.open_rows = open_rows.len();
    stats.mixed_rows = completed_rows.intersection(&open_rows).count();
    if stats.completed_occurrences + stats.open_occurrences != FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES {
        return Err(FourLoopT1S2ClosureError::ReplayMismatch {
            leaf_id: u32::MAX,
            stage: "occurrence partition union",
        });
    }
    Ok(output)
}

fn occurrence_record(
    source: FourLoopComponentTransportOccurrence,
    completed_plan_index: Option<u16>,
) -> FourLoopT1S2ClosureOccurrence {
    FourLoopT1S2ClosureOccurrence {
        row_index: source.row_index(),
        path_index: source.path_index(),
        leaf_id: source.leaf_id(),
        completed_plan_index,
    }
}

fn check_exact_structural_stats(
    stats: FourLoopT1S2ClosureStats,
) -> Result<(), FourLoopT1S2ClosureError> {
    for (resource, expected, actual) in [
        (
            "completed plans",
            FOUR_LOOP_T1S2_CLOSURE_PLANS,
            stats.completed_plans,
        ),
        (
            "open plans",
            FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS,
            stats.open_plans,
        ),
        (
            "components",
            FOUR_LOOP_T1S2_CLOSURE_COMPONENTS,
            stats.components,
        ),
        (
            "local slots",
            FOUR_LOOP_T1S2_CLOSURE_LOCAL_SLOTS,
            stats.local_slots,
        ),
    ] {
        if actual != expected {
            return Err(FourLoopT1S2ClosureError::CensusMismatch {
                resource,
                expected,
                actual,
            });
        }
    }
    Ok(())
}

fn check_actual_stats(
    config: FourLoopT1S2ClosureConfig,
    stats: FourLoopT1S2ClosureStats,
) -> Result<(), FourLoopT1S2ClosureError> {
    for (resource, requested, limit) in [
        ("completed plans", stats.completed_plans, config.max_plans),
        ("open plans", stats.open_plans, config.max_open_plans),
        (
            "occurrence partition",
            stats.completed_occurrences + stats.open_occurrences,
            config.max_occurrences,
        ),
        ("components", stats.components, config.max_components),
        ("local slots", stats.local_slots, config.max_local_slots),
        (
            "scalar branches",
            stats.scalar_branches,
            config.max_scalar_branches,
        ),
        (
            "component calls",
            stats.component_calls,
            config.max_component_calls,
        ),
        (
            "unique targets",
            stats.unique_targets,
            config.max_unique_targets,
        ),
        (
            "convolution pair operations",
            stats.convolution_pair_operations,
            config.max_convolution_pair_operations,
        ),
        (
            "precollection terms",
            stats.precollection_terms,
            config.max_precollection_terms,
        ),
        (
            "collected terms",
            stats.collected_terms,
            config.max_collected_terms,
        ),
        (
            "mass-power steps",
            stats.mass_power_steps,
            config.max_mass_power_steps,
        ),
        (
            "coefficient operations",
            stats.coefficient_operations,
            config.max_coefficient_operations,
        ),
    ] {
        if requested > limit {
            return Err(FourLoopT1S2ClosureError::ResourceLimit {
                resource,
                requested: requested as u128,
                limit: limit as u128,
            });
        }
    }
    Ok(())
}

fn checked_sum(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, FourLoopT1S2ClosureError> {
    left.checked_add(right)
        .ok_or(FourLoopT1S2ClosureError::ArithmeticOverflow { resource })
}

fn product_loop_weight(product: &MasterProduct<MassiveVacuumMaster>) -> u128 {
    product
        .factors()
        .iter()
        .map(|(master, multiplicity)| (master.loops() as u128) * u128::from(*multiplicity))
        .sum()
}

fn product_mass_weight(product: &MasterProduct<MassiveVacuumMaster>) -> i64 {
    product
        .factors()
        .iter()
        .map(|(master, multiplicity)| i64::from(*multiplicity) * master.physical_lines() as i64)
        .sum()
}

fn is_allowed_slice_product(product: &MasterProduct<MassiveVacuumMaster>) -> bool {
    classify_product(product).is_some()
}

fn replace_product_coefficient(
    input: &ProductLinearCombination<MassiveVacuumMaster>,
    product: &MasterProduct<MassiveVacuumMaster>,
    coefficient: Coefficient,
) -> ProductLinearCombination<MassiveVacuumMaster> {
    let mut output = input.clone();
    output.remove(product);
    output.add_term(product.clone(), coefficient);
    output
}

fn retained_coefficient_bytes(
    targets: &[FourLoopT1S2LocalReduction],
    plans: &[FourLoopT1S2PlanClosure],
) -> Result<usize, FourLoopT1S2ClosureError> {
    let mut bytes = 0_usize;
    let mut charge = |coefficient: &Coefficient| -> Result<(), FourLoopT1S2ClosureError> {
        bytes = bytes.checked_add(coefficient.to_string().len()).ok_or(
            FourLoopT1S2ClosureError::ArithmeticOverflow {
                resource: "retained coefficient bytes",
            },
        )?;
        Ok(())
    };
    for target in targets {
        for coefficient in target.ordinary.terms().values() {
            charge(coefficient)?;
        }
        if let FourLoopT1S2LocalProof::Sunset {
            integral_output, ..
        } = &target.proof
        {
            for coefficient in integral_output.terms().values() {
                charge(coefficient)?;
            }
        }
    }
    for plan in plans {
        for branch in &plan.branches {
            charge(&branch.coefficient)?;
            for combination in [&branch.ordinary_unscaled, &branch.ordinary_scaled] {
                for coefficient in combination.terms().values() {
                    charge(coefficient)?;
                }
            }
        }
        for combination in [&plan.ordinary, &plan.mass_normalized] {
            for coefficient in combination.terms().values() {
                charge(coefficient)?;
            }
        }
    }
    Ok(bytes)
}

fn closure_checksum(
    transport: &FourLoopComponentTransport<'_>,
    config: FourLoopT1S2ClosureConfig,
    coefficient_context: &CoefficientContext,
    targets: &[FourLoopT1S2LocalReduction],
    plans: &[FourLoopT1S2PlanClosure],
    open_leaf_ids: &[u32],
    occurrences: &[FourLoopT1S2ClosureOccurrence],
    stats: FourLoopT1S2ClosureStats,
) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_bytes(&mut hash, FourLoopT1S2Closure::SCHEMA.as_bytes());
    hash_bytes(&mut hash, transport.source_schema().as_bytes());
    hash_u64(&mut hash, transport.source_seed_checksum());
    hash_config(&mut hash, config);
    hash_u64(
        &mut hash,
        coefficient_context.parameter_names().len() as u64,
    );
    for name in coefficient_context.parameter_names() {
        hash_bytes(&mut hash, name.as_bytes());
    }
    for target in targets {
        hash_bytes(&mut hash, target.master_key().as_bytes());
        hash_i32_slice(&mut hash, &target.target.powers);
        hash_bytes(&mut hash, target.service_schema.as_bytes());
        hash_combination(&mut hash, &target.ordinary);
        match &target.proof {
            FourLoopT1S2LocalProof::Tadpole(reduction) => {
                hash_u64(&mut hash, 0);
                hash_u64(&mut hash, reduction.power() as u32 as u64);
                hash_coefficient(&mut hash, reduction.coefficient());
                let reduction_stats = reduction.stats();
                hash_u64(&mut hash, reduction_stats.recurrence_steps() as u64);
                hash_u64(&mut hash, reduction_stats.coefficient_operations() as u64);
                hash_u128(&mut hash, reduction_stats.dense_term_operation_bound());
                hash_u128(&mut hash, reduction_stats.coefficient_degree_bound());
            }
            FourLoopT1S2LocalProof::Sunset {
                requested,
                oriented,
                preflight,
                integral_output,
            } => {
                hash_u64(&mut hash, 1);
                hash_i32_slice(&mut hash, requested.powers());
                hash_i32_slice(&mut hash, oriented.powers());
                hash_u128(&mut hash, preflight.state_upper_bound());
                hash_u128(&mut hash, preflight.coefficient_operation_upper_bound());
                hash_u128(&mut hash, preflight.coefficient_degree_bound());
                hash_u128(&mut hash, preflight.boundary_formula_iterations());
                hash_integral_combination(&mut hash, integral_output);
            }
        }
    }
    for (plan, source) in plans.iter().zip(
        transport
            .plans()
            .iter()
            .filter(|source| classify_product(source.key().product()).is_some()),
    ) {
        hash_u64(&mut hash, u64::from(plan.leaf_id));
        hash_bytes(&mut hash, plan.product_class.stable_key().as_bytes());
        hash_bytes(&mut hash, source.key().family_fingerprint().as_bytes());
        hash_i32_slice(&mut hash, source.key().powers());
        for branch in &plan.branches {
            hash_u64(&mut hash, branch.branch_index as u64);
            hash_branch_kind(&mut hash, branch.kind);
            hash_coefficient(&mut hash, &branch.coefficient);
            for component_use in &branch.component_uses {
                hash_u64(&mut hash, component_use.witness_index as u64);
                hash_u64(&mut hash, u64::from(component_use.target_index));
            }
            hash_u64(&mut hash, branch.convolution_pair_operations as u64);
            hash_combination(&mut hash, &branch.ordinary_unscaled);
            hash_combination(&mut hash, &branch.ordinary_scaled);
        }
        hash_combination(&mut hash, &plan.ordinary);
        hash_combination(&mut hash, &plan.mass_normalized);
    }
    hash_u64(&mut hash, open_leaf_ids.len() as u64);
    for &leaf_id in open_leaf_ids {
        hash_u64(&mut hash, u64::from(leaf_id));
    }
    for occurrence in occurrences {
        hash_u64(&mut hash, u64::from(occurrence.row_index));
        hash_u64(&mut hash, u64::from(occurrence.path_index));
        hash_u64(&mut hash, u64::from(occurrence.leaf_id));
        hash_u64(
            &mut hash,
            occurrence.completed_plan_index.map_or(u64::MAX, u64::from),
        );
    }
    for value in [
        stats.completed_plans,
        stats.open_plans,
        stats.completed_occurrences,
        stats.open_occurrences,
        stats.completed_rows,
        stats.open_rows,
        stats.mixed_rows,
        stats.components,
        stats.local_slots,
        stats.scalar_branches,
        stats.base_branches,
        stats.constant_branches,
        stats.local_t1_branches,
        stats.local_s2_branches,
        stats.component_calls,
        stats.t1_component_calls,
        stats.s2_component_calls,
        stats.unique_targets,
        stats.t1_targets,
        stats.s2_targets,
        stats.cache_hits,
        stats.convolution_pair_operations,
        stats.precollection_terms,
        stats.collected_terms,
        stats.mass_power_steps,
        stats.coefficient_operations,
        stats.retained_coefficient_bytes,
        stats.n0_plans,
        stats.n1_plans,
    ] {
        hash_u64(&mut hash, value as u64);
    }
    hash
}

fn hash_config(hash: &mut u64, config: FourLoopT1S2ClosureConfig) {
    for value in [
        config.max_plans,
        config.max_open_plans,
        config.max_occurrences,
        config.max_components,
        config.max_local_slots,
        config.max_scalar_branches,
        config.max_component_calls,
        config.max_unique_targets,
        config.max_convolution_pair_operations,
        config.max_precollection_terms,
        config.max_collected_terms,
        config.max_mass_power_steps,
        config.max_coefficient_operations,
        config.max_retained_coefficient_bytes,
        config.one_loop.max_recurrence_steps,
        config.one_loop.max_coefficient_operations,
        config.two_loop.max_explicit_terms,
        config.two_loop.max_raw_terms,
        config.two_loop.max_states,
        config.two_loop.max_coefficient_operations,
        config.two_loop.max_boundary_formula_iterations,
    ] {
        hash_u64(hash, value as u64);
    }
    for value in [
        config.max_coefficient_degree,
        config.one_loop.max_dense_term_operations,
        config.one_loop.max_coefficient_degree,
        config.two_loop.max_coefficient_degree,
    ] {
        hash_u128(hash, value);
    }
}

impl FourLoopT1S2LocalReduction {
    fn master_key(&self) -> &'static str {
        self.target.master.stable_key()
    }
}

fn hash_u64(hash: &mut u64, value: u64) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn hash_u128(hash: &mut u64, value: u128) {
    for byte in value.to_le_bytes() {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    hash_u64(hash, bytes.len() as u64);
    for &byte in bytes {
        *hash ^= u64::from(byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn hash_i32_slice(hash: &mut u64, values: &[i32]) {
    hash_u64(hash, values.len() as u64);
    for &value in values {
        hash_u64(hash, value as u32 as u64);
    }
}

fn hash_coefficient(hash: &mut u64, coefficient: &Coefficient) {
    hash_bytes(hash, coefficient.to_string().as_bytes());
}

fn hash_product(hash: &mut u64, product: &MasterProduct<MassiveVacuumMaster>) {
    hash_u64(hash, product.factors().len() as u64);
    for (master, multiplicity) in product.factors() {
        hash_bytes(hash, master.stable_key().as_bytes());
        hash_u64(hash, u64::from(*multiplicity));
    }
}

fn hash_combination(hash: &mut u64, combination: &ProductLinearCombination<MassiveVacuumMaster>) {
    hash_u64(hash, combination.len() as u64);
    for (product, coefficient) in combination.terms() {
        hash_product(hash, product);
        hash_coefficient(hash, coefficient);
    }
}

fn hash_integral_combination(hash: &mut u64, combination: &crate::LinearCombination) {
    hash_u64(hash, combination.len() as u64);
    for (integral, coefficient) in combination.terms() {
        hash_i32_slice(hash, integral.powers());
        hash_coefficient(hash, coefficient);
    }
}

fn hash_branch_kind(hash: &mut u64, kind: FourLoopComponentScalarBranchKind) {
    match kind {
        FourLoopComponentScalarBranchKind::Base => hash_u64(hash, 0),
        FourLoopComponentScalarBranchKind::Constant => hash_u64(hash, 1),
        FourLoopComponentScalarBranchKind::Local {
            component_index,
            local_position,
        } => {
            hash_u64(hash, 2);
            hash_u64(hash, component_index as u64);
            hash_u64(hash, local_position as u64);
        }
    }
}
