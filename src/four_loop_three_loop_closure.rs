//! Exact plan-level closure of the three-loop-component slice of the
//! four-loop next-shell transport.
//!
//! This layer consumes the 823 transported plans whose scalar-corner product
//! is `T1*B4`, `T1*F5`, or `T1*M6`.  It preserves witness-indexed component
//! identity through local reduction, performs ordinary product convolution in
//! `Q(d,m2)`, and applies parent mass normalization only after collection.
//!
//! The sibling [`crate::FourLoopT1S2Closure`] owns the other 243 plans.  Thus
//! this certificate proves one exact finite slice; it does not by itself
//! construct normalized four-loop rows, a next-shell rank, or an unrestricted
//! master basis.  Its delegated sparse three-loop equalities have generic
//! rational-function-field semantics and do not inventory exceptional
//! dimension factors.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::legacy_oracle_support::coefficient_degree::{
    coefficient_product_degree_bound, coefficient_sum_degree_bound, coefficient_variable_degrees,
    symbolica_coefficient_degree_is_representable,
};
use crate::{
    Coefficient, CoefficientContext, FourLoopComponentScalarBranch,
    FourLoopComponentScalarBranchKind, FourLoopComponentTransport, FourLoopComponentTransportError,
    FourLoopComponentTransportOccurrence, FourLoopComponentTransportPlan,
    FourLoopThreeLoopLocalTarget, FourLoopThreeLoopService, FourLoopThreeLoopServiceConfig,
    FourLoopThreeLoopServiceError, MassiveVacuumMaster, MasterProduct, MasterProductError,
    ProductLinearCombination, SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

pub const FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS: usize = 823;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_PLANS: usize = 243;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_OCCURRENCES: usize = 4_230;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_COMPLETED_OCCURRENCES: usize = 3_096;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_OCCURRENCES: usize = 1_134;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_COMPLETED_ROWS: usize = 969;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_ROWS: usize = 511;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_MIXED_ROWS: usize = 191;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENTS: usize = 1_646;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_LOCAL_SLOTS: usize = 5_761;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_SCALAR_BRANCHES: usize = 1_884;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENT_CALLS: usize = 3_768;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_UNIQUE_TARGETS: usize = 204;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_CONVOLUTION_PAIR_BOUND: usize = 7_356;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_PRECOLLECTION_TERM_BOUND: usize = 3_598;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_COLLECTED_TERM_BOUND: usize = 2_159;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_MASS_POWER_STEP_BOUND: usize = 4_279;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_OPERATION_BOUND: usize = 17_456;
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_DEGREE: u128 = 256;
/// Semantic-output coefficient bytes retained by the service and plan
/// witnesses. This does not claim to bound the owned sparse table, allocator
/// overhead, caches, or Symbolica workspaces.
pub const FOUR_LOOP_THREE_LOOP_CLOSURE_RETAINED_OUTPUT_COEFFICIENT_BYTES: usize = 256_603;

const FNV1A64_OFFSET: u64 = 0xcbf29ce484222325;
const FNV1A64_PRIME: u64 = 0x100000001b3;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopThreeLoopClosureConfig {
    pub max_plans: usize,
    pub max_outside_plans: usize,
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
    pub max_retained_output_coefficient_bytes: usize,
    pub service: FourLoopThreeLoopServiceConfig,
}

impl Default for FourLoopThreeLoopClosureConfig {
    fn default() -> Self {
        Self {
            max_plans: FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS,
            max_outside_plans: FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_PLANS,
            max_occurrences: FOUR_LOOP_THREE_LOOP_CLOSURE_OCCURRENCES,
            max_components: FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENTS,
            max_local_slots: FOUR_LOOP_THREE_LOOP_CLOSURE_LOCAL_SLOTS,
            max_scalar_branches: FOUR_LOOP_THREE_LOOP_CLOSURE_SCALAR_BRANCHES,
            max_component_calls: FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENT_CALLS,
            max_unique_targets: FOUR_LOOP_THREE_LOOP_CLOSURE_UNIQUE_TARGETS,
            max_convolution_pair_operations: FOUR_LOOP_THREE_LOOP_CLOSURE_CONVOLUTION_PAIR_BOUND,
            max_precollection_terms: FOUR_LOOP_THREE_LOOP_CLOSURE_PRECOLLECTION_TERM_BOUND,
            max_collected_terms: FOUR_LOOP_THREE_LOOP_CLOSURE_COLLECTED_TERM_BOUND,
            max_mass_power_steps: FOUR_LOOP_THREE_LOOP_CLOSURE_MASS_POWER_STEP_BOUND,
            max_coefficient_operations: FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_OPERATION_BOUND,
            max_coefficient_degree: FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_DEGREE,
            max_retained_output_coefficient_bytes:
                FOUR_LOOP_THREE_LOOP_CLOSURE_RETAINED_OUTPUT_COEFFICIENT_BYTES,
            service: FourLoopThreeLoopServiceConfig::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FourLoopThreeLoopProductClass {
    T1B4,
    T1F5,
    T1M6,
}

impl FourLoopThreeLoopProductClass {
    pub const fn stable_key(self) -> &'static str {
        match self {
            Self::T1B4 => "rustred-four-loop-three-loop-product-v1:T1*B4",
            Self::T1F5 => "rustred-four-loop-three-loop-product-v1:T1*F5",
            Self::T1M6 => "rustred-four-loop-three-loop-product-v1:T1*M6",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopThreeLoopClosureStatus {
    ExactThreeLoopComponentSliceGenericQ,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopThreeLoopParentStatus {
    Partial {
        completed_plans: usize,
        outside_plans: usize,
        completed_occurrences: usize,
        outside_occurrences: usize,
    },
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopThreeLoopClosureStats {
    completed_plans: usize,
    outside_plans: usize,
    completed_occurrences: usize,
    outside_occurrences: usize,
    completed_rows: usize,
    outside_rows: usize,
    mixed_rows: usize,
    components: usize,
    local_slots: usize,
    scalar_branches: usize,
    base_branches: usize,
    constant_branches: usize,
    local_t1_branches: usize,
    local_b4_branches: usize,
    local_f5_branches: usize,
    local_m6_branches: usize,
    component_calls: usize,
    t1_component_calls: usize,
    b4_component_calls: usize,
    f5_component_calls: usize,
    m6_component_calls: usize,
    unique_targets: usize,
    cache_hits: usize,
    convolution_pair_operations: usize,
    precollection_terms: usize,
    collected_terms: usize,
    mass_power_steps: usize,
    coefficient_operations: usize,
    retained_output_coefficient_bytes: usize,
    n0_plans: usize,
    n1_plans: usize,
}

macro_rules! closure_stat_getters {
    ($($name:ident),* $(,)?) => { $(pub const fn $name(self) -> usize { self.$name })* };
}

impl FourLoopThreeLoopClosureStats {
    closure_stat_getters!(
        completed_plans,
        outside_plans,
        completed_occurrences,
        outside_occurrences,
        completed_rows,
        outside_rows,
        mixed_rows,
        components,
        local_slots,
        scalar_branches,
        base_branches,
        constant_branches,
        local_t1_branches,
        local_b4_branches,
        local_f5_branches,
        local_m6_branches,
        component_calls,
        t1_component_calls,
        b4_component_calls,
        f5_component_calls,
        m6_component_calls,
        unique_targets,
        cache_hits,
        convolution_pair_operations,
        precollection_terms,
        collected_terms,
        mass_power_steps,
        coefficient_operations,
        retained_output_coefficient_bytes,
        n0_plans,
        n1_plans,
    );
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopThreeLoopComponentUse {
    witness_index: usize,
    target_index: u16,
}

impl FourLoopThreeLoopComponentUse {
    pub const fn witness_index(self) -> usize {
        self.witness_index
    }
    pub const fn target_index(self) -> u16 {
        self.target_index
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopThreeLoopBranchClosure {
    branch_index: usize,
    kind: FourLoopComponentScalarBranchKind,
    coefficient: Coefficient,
    component_uses: Vec<FourLoopThreeLoopComponentUse>,
    convolution_pair_operations: usize,
    ordinary_unscaled: ProductLinearCombination<MassiveVacuumMaster>,
    ordinary_scaled: ProductLinearCombination<MassiveVacuumMaster>,
}

impl FourLoopThreeLoopBranchClosure {
    pub const fn branch_index(&self) -> usize {
        self.branch_index
    }
    pub const fn kind(&self) -> FourLoopComponentScalarBranchKind {
        self.kind
    }
    pub const fn coefficient(&self) -> &Coefficient {
        &self.coefficient
    }
    pub fn component_uses(&self) -> &[FourLoopThreeLoopComponentUse] {
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopThreeLoopPlanClosure {
    leaf_id: u32,
    product_class: FourLoopThreeLoopProductClass,
    branches: Vec<FourLoopThreeLoopBranchClosure>,
    ordinary: ProductLinearCombination<MassiveVacuumMaster>,
    mass_normalized: ProductLinearCombination<MassiveVacuumMaster>,
}

impl FourLoopThreeLoopPlanClosure {
    pub const fn leaf_id(&self) -> u32 {
        self.leaf_id
    }
    pub const fn product_class(&self) -> FourLoopThreeLoopProductClass {
        self.product_class
    }
    pub fn branches(&self) -> &[FourLoopThreeLoopBranchClosure] {
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
        index: usize,
        coefficient: Coefficient,
    ) -> Self {
        let mut candidate = self.clone();
        if let Some(branch) = candidate.branches.get_mut(index) {
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
        candidate.mass_normalized.remove(product);
        candidate
            .mass_normalized
            .add_term(product.clone(), coefficient);
        candidate
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopThreeLoopClosureOccurrence {
    row_index: u16,
    path_index: u32,
    leaf_id: u32,
    completed_plan_index: Option<u16>,
}

impl FourLoopThreeLoopClosureOccurrence {
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

pub struct FourLoopThreeLoopClosure<'transport, 'inventory> {
    transport: &'transport FourLoopComponentTransport<'inventory>,
    config: FourLoopThreeLoopClosureConfig,
    context: CoefficientContext,
    service: FourLoopThreeLoopService,
    plans: Vec<FourLoopThreeLoopPlanClosure>,
    outside_leaf_ids: Vec<u32>,
    occurrences: Vec<FourLoopThreeLoopClosureOccurrence>,
    stats: FourLoopThreeLoopClosureStats,
    checksum: u64,
}

impl<'transport, 'inventory> FourLoopThreeLoopClosure<'transport, 'inventory> {
    pub const SCHEMA: &'static str = "rustred-four-loop-three-loop-component-closure-v1";
    pub const fn config(&self) -> FourLoopThreeLoopClosureConfig {
        self.config
    }
    pub const fn status(&self) -> FourLoopThreeLoopClosureStatus {
        FourLoopThreeLoopClosureStatus::ExactThreeLoopComponentSliceGenericQ
    }
    pub const fn parent_status(&self) -> FourLoopThreeLoopParentStatus {
        FourLoopThreeLoopParentStatus::Partial {
            completed_plans: self.stats.completed_plans,
            outside_plans: self.stats.outside_plans,
            completed_occurrences: self.stats.completed_occurrences,
            outside_occurrences: self.stats.outside_occurrences,
        }
    }
    pub const fn stats(&self) -> FourLoopThreeLoopClosureStats {
        self.stats
    }
    pub const fn checksum(&self) -> u64 {
        self.checksum
    }
    pub const fn coefficient_context(&self) -> &CoefficientContext {
        &self.context
    }
    pub(crate) const fn transport(&self) -> &'transport FourLoopComponentTransport<'inventory> {
        self.transport
    }
    pub const fn service(&self) -> &FourLoopThreeLoopService {
        &self.service
    }
    pub fn plans(&self) -> &[FourLoopThreeLoopPlanClosure] {
        &self.plans
    }
    pub fn outside_leaf_ids(&self) -> &[u32] {
        &self.outside_leaf_ids
    }
    pub fn occurrences(&self) -> &[FourLoopThreeLoopClosureOccurrence] {
        &self.occurrences
    }

    pub fn preflight_config(
        config: FourLoopThreeLoopClosureConfig,
    ) -> Result<(), FourLoopThreeLoopClosureError> {
        preflight_config(config)
    }

    pub fn build(
        transport: &'transport FourLoopComponentTransport<'inventory>,
        config: FourLoopThreeLoopClosureConfig,
    ) -> Result<Self, FourLoopThreeLoopClosureError> {
        Self::build_impl(transport, config, true)
    }

    fn build_impl(
        transport: &'transport FourLoopComponentTransport<'inventory>,
        config: FourLoopThreeLoopClosureConfig,
        authenticate_transport: bool,
    ) -> Result<Self, FourLoopThreeLoopClosureError> {
        preflight_config(config)?;
        if authenticate_transport {
            transport.replay()?;
        }
        let prescan = prescan(transport, config)?;
        let service = FourLoopThreeLoopService::build(
            prescan.context.clone(),
            prescan.targets.clone(),
            config.service,
        )?;
        if service.targets() != prescan.targets.as_slice() {
            return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
                leaf_id: u32::MAX,
                stage: "service target order",
            });
        }
        let target_indices = service
            .targets()
            .iter()
            .enumerate()
            .map(|(index, target)| (target.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let mut stats = prescan.stats;
        let mut arithmetic = CheckedArithmetic::new(prescan.context.clone(), config);
        let mut plans = Vec::new();
        plans
            .try_reserve_exact(prescan.completed_indices.len())
            .map_err(|_| FourLoopThreeLoopClosureError::AllocationFailed {
                resource: "three-loop plan closures",
                requested: prescan.completed_indices.len(),
            })?;
        for &index in &prescan.completed_indices {
            plans.push(build_plan_closure(
                transport.plans().get(index).ok_or(
                    FourLoopThreeLoopClosureError::ReplayMismatch {
                        leaf_id: u32::MAX,
                        stage: "source plan index",
                    },
                )?,
                &service,
                &target_indices,
                &mut arithmetic,
                &mut stats,
                config,
            )?);
        }
        stats.coefficient_operations = arithmetic.operations;
        let completed_by_leaf = plans
            .iter()
            .enumerate()
            .map(|(index, plan)| (plan.leaf_id, index))
            .collect::<BTreeMap<_, _>>();
        let occurrences = build_occurrence_partition(transport, &completed_by_leaf, &mut stats)?;
        check_occurrence_partition(stats)?;
        stats.retained_output_coefficient_bytes = retained_output_bytes(&service, &plans)?;
        check_actual_stats(config, stats)?;
        let checksum = closure_checksum(
            transport,
            config,
            &prescan.context,
            &service,
            &plans,
            &prescan.outside_leaf_ids,
            &occurrences,
            stats,
        );
        Ok(Self {
            transport,
            config,
            context: prescan.context,
            service,
            plans,
            outside_leaf_ids: prescan.outside_leaf_ids,
            occurrences,
            stats,
            checksum,
        })
    }

    pub fn replay(&self) -> Result<(), FourLoopThreeLoopClosureError> {
        self.transport.replay()?;
        // `build_impl` below constructs and authenticates one fresh service.
        // Calling `self.service.replay()` here as well would perform the same
        // expensive three-loop deterministic rebuild twice.
        let rebuilt = Self::build_impl(self.transport, self.config, false)?;
        if rebuilt.plans != self.plans
            || rebuilt.outside_leaf_ids != self.outside_leaf_ids
            || rebuilt.occurrences != self.occurrences
            || rebuilt.stats != self.stats
            || rebuilt.checksum != self.checksum
        {
            return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
                leaf_id: u32::MAX,
                stage: "complete three-loop closure rebuild",
            });
        }
        Ok(())
    }

    #[doc(hidden)]
    pub fn replay_plan_candidate(
        &self,
        candidate: &FourLoopThreeLoopPlanClosure,
    ) -> Result<(), FourLoopThreeLoopClosureError> {
        let source = self
            .transport
            .plans()
            .iter()
            .find(|plan| plan.leaf_id() == candidate.leaf_id)
            .ok_or(FourLoopThreeLoopClosureError::ReplayMismatch {
                leaf_id: candidate.leaf_id,
                stage: "candidate source plan",
            })?;
        self.transport.replay_plan_candidate(source)?;
        self.service.validate_retained_reductions()?;
        let target_indices = self
            .service
            .targets()
            .iter()
            .enumerate()
            .map(|(index, target)| (target.clone(), index))
            .collect::<BTreeMap<_, _>>();
        let mut stats = FourLoopThreeLoopClosureStats::default();
        let mut arithmetic = CheckedArithmetic::new(self.context.clone(), self.config);
        let expected = build_plan_closure(
            source,
            &self.service,
            &target_indices,
            &mut arithmetic,
            &mut stats,
            self.config,
        )?;
        if &expected != candidate {
            return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
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
        candidate: FourLoopThreeLoopClosureOccurrence,
    ) -> Result<(), FourLoopThreeLoopClosureError> {
        let source = self.transport.occurrences().get(source_index).ok_or(
            FourLoopThreeLoopClosureError::ReplayMismatch {
                leaf_id: candidate.leaf_id,
                stage: "candidate occurrence source index",
            },
        )?;
        let completed = self
            .plans
            .iter()
            .position(|plan| plan.leaf_id == source.leaf_id())
            .map(|index| {
                u16::try_from(index).map_err(|_| {
                    FourLoopThreeLoopClosureError::ArithmeticOverflow {
                        resource: "candidate completed plan index",
                    }
                })
            })
            .transpose()?;
        if occurrence_record(*source, completed) != candidate {
            return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
                leaf_id: candidate.leaf_id,
                stage: "candidate occurrence reference",
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
pub enum FourLoopThreeLoopClosureError {
    Transport(FourLoopComponentTransportError),
    Service(FourLoopThreeLoopServiceError),
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

impl From<FourLoopComponentTransportError> for FourLoopThreeLoopClosureError {
    fn from(value: FourLoopComponentTransportError) -> Self {
        Self::Transport(value)
    }
}
impl From<FourLoopThreeLoopServiceError> for FourLoopThreeLoopClosureError {
    fn from(value: FourLoopThreeLoopServiceError) -> Self {
        Self::Service(value)
    }
}
impl From<MasterProductError> for FourLoopThreeLoopClosureError {
    fn from(value: MasterProductError) -> Self {
        Self::Product(value)
    }
}
impl fmt::Display for FourLoopThreeLoopClosureError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "four-loop three-loop-component closure: {self:?}")
    }
}
impl Error for FourLoopThreeLoopClosureError {}

struct Prescan {
    context: CoefficientContext,
    completed_indices: Vec<usize>,
    outside_leaf_ids: Vec<u32>,
    targets: Vec<FourLoopThreeLoopLocalTarget>,
    stats: FourLoopThreeLoopClosureStats,
}

fn preflight_config(
    config: FourLoopThreeLoopClosureConfig,
) -> Result<(), FourLoopThreeLoopClosureError> {
    for (resource, limit, minimum) in [
        (
            "three-loop plans",
            config.max_plans,
            FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS,
        ),
        (
            "outside plans",
            config.max_outside_plans,
            FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_PLANS,
        ),
        (
            "occurrences",
            config.max_occurrences,
            FOUR_LOOP_THREE_LOOP_CLOSURE_OCCURRENCES,
        ),
        (
            "components",
            config.max_components,
            FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENTS,
        ),
        (
            "local slots",
            config.max_local_slots,
            FOUR_LOOP_THREE_LOOP_CLOSURE_LOCAL_SLOTS,
        ),
        (
            "scalar branches",
            config.max_scalar_branches,
            FOUR_LOOP_THREE_LOOP_CLOSURE_SCALAR_BRANCHES,
        ),
        (
            "component calls",
            config.max_component_calls,
            FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENT_CALLS,
        ),
        (
            "unique targets",
            config.max_unique_targets,
            FOUR_LOOP_THREE_LOOP_CLOSURE_UNIQUE_TARGETS,
        ),
        (
            "convolution pairs",
            config.max_convolution_pair_operations,
            FOUR_LOOP_THREE_LOOP_CLOSURE_CONVOLUTION_PAIR_BOUND,
        ),
        (
            "precollection terms",
            config.max_precollection_terms,
            FOUR_LOOP_THREE_LOOP_CLOSURE_PRECOLLECTION_TERM_BOUND,
        ),
        (
            "collected terms",
            config.max_collected_terms,
            FOUR_LOOP_THREE_LOOP_CLOSURE_COLLECTED_TERM_BOUND,
        ),
        (
            "mass-power steps",
            config.max_mass_power_steps,
            FOUR_LOOP_THREE_LOOP_CLOSURE_MASS_POWER_STEP_BOUND,
        ),
        (
            "coefficient operations",
            config.max_coefficient_operations,
            FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_OPERATION_BOUND,
        ),
        (
            "retained coefficient bytes",
            config.max_retained_output_coefficient_bytes,
            FOUR_LOOP_THREE_LOOP_CLOSURE_RETAINED_OUTPUT_COEFFICIENT_BYTES,
        ),
    ] {
        if limit < minimum {
            return Err(FourLoopThreeLoopClosureError::ResourceLimit {
                resource,
                requested: minimum as u128,
                limit: limit as u128,
            });
        }
    }
    if config.max_coefficient_degree < FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_DEGREE {
        return Err(FourLoopThreeLoopClosureError::ResourceLimit {
            resource: "coefficient degree",
            requested: FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_DEGREE,
            limit: config.max_coefficient_degree,
        });
    }
    if config.max_coefficient_degree > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(FourLoopThreeLoopClosureError::ResourceLimit {
            resource: "configured coefficient degree",
            requested: config.max_coefficient_degree,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }
    FourLoopThreeLoopService::preflight_config(config.service)?;
    Ok(())
}

fn prescan(
    transport: &FourLoopComponentTransport<'_>,
    config: FourLoopThreeLoopClosureConfig,
) -> Result<Prescan, FourLoopThreeLoopClosureError> {
    if transport.plans().len()
        != FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS + FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_PLANS
    {
        return Err(FourLoopThreeLoopClosureError::CensusMismatch {
            resource: "source plans",
            expected: 1_066,
            actual: transport.plans().len(),
        });
    }
    if transport.occurrences().len() != FOUR_LOOP_THREE_LOOP_CLOSURE_OCCURRENCES {
        return Err(FourLoopThreeLoopClosureError::CensusMismatch {
            resource: "source occurrences",
            expected: FOUR_LOOP_THREE_LOOP_CLOSURE_OCCURRENCES,
            actual: transport.occurrences().len(),
        });
    }
    let mut completed_indices = Vec::new();
    completed_indices
        .try_reserve_exact(FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS)
        .map_err(|_| FourLoopThreeLoopClosureError::AllocationFailed {
            resource: "completed source plan indices",
            requested: FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS,
        })?;
    let mut outside_leaf_ids = Vec::new();
    outside_leaf_ids
        .try_reserve_exact(FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_PLANS)
        .map_err(|_| FourLoopThreeLoopClosureError::AllocationFailed {
            resource: "outside source leaf ids",
            requested: FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_PLANS,
        })?;
    let mut targets = BTreeSet::new();
    let mut stats = FourLoopThreeLoopClosureStats::default();
    let mut context = None::<CoefficientContext>;
    let mut classes = BTreeMap::new();
    for (index, plan) in transport.plans().iter().enumerate() {
        let Some(class) = classify_product(plan.key().product()) else {
            outside_leaf_ids.push(plan.leaf_id());
            continue;
        };
        *classes.entry(class).or_insert(0usize) += 1;
        completed_indices.push(index);
        let (key, family) = transport.authenticated_source_context(plan.leaf_id())?;
        if key != plan.key() {
            return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
                leaf_id: plan.leaf_id(),
                stage: "authenticated source key",
            });
        }
        let parent = family.coefficients();
        if !parent
            .parameter_names()
            .iter()
            .map(String::as_str)
            .eq(["d", "m2"])
            || parent.parameter("d").as_ref() != Some(family.dimension())
            || parent.parameter("m2").is_none()
        {
            return Err(FourLoopThreeLoopClosureError::UnsupportedDomain {
                leaf_id: plan.leaf_id(),
                reason: "parent is not authenticated Q(d,m2)",
            });
        }
        if context
            .as_ref()
            .is_some_and(|first| !first.has_same_variable_map(parent))
        {
            return Err(FourLoopThreeLoopClosureError::CoefficientContextMismatch);
        }
        context.get_or_insert_with(|| parent.clone());
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
            stats.n1_plans += 1;
        } else {
            stats.n0_plans += 1;
        }
        for branch in plan.scalar_branches() {
            match branch.kind() {
                FourLoopComponentScalarBranchKind::Base => stats.base_branches += 1,
                FourLoopComponentScalarBranchKind::Constant => stats.constant_branches += 1,
                FourLoopComponentScalarBranchKind::Local {
                    component_index, ..
                } => match plan
                    .components()
                    .get(component_index)
                    .ok_or(FourLoopThreeLoopClosureError::ReplayMismatch {
                        leaf_id: plan.leaf_id(),
                        stage: "local branch owner",
                    })?
                    .master()
                {
                    MassiveVacuumMaster::T1 => stats.local_t1_branches += 1,
                    MassiveVacuumMaster::B4 => stats.local_b4_branches += 1,
                    MassiveVacuumMaster::F5 => stats.local_f5_branches += 1,
                    MassiveVacuumMaster::M6 => stats.local_m6_branches += 1,
                    MassiveVacuumMaster::S2 => {
                        return Err(FourLoopThreeLoopClosureError::UnsupportedDomain {
                            leaf_id: plan.leaf_id(),
                            reason: "three-loop slice contains S2",
                        });
                    }
                },
            }
            for target in reconstruct_targets(plan, branch)? {
                stats.component_calls += 1;
                match target.owner() {
                    MassiveVacuumMaster::T1 => stats.t1_component_calls += 1,
                    MassiveVacuumMaster::B4 => stats.b4_component_calls += 1,
                    MassiveVacuumMaster::F5 => stats.f5_component_calls += 1,
                    MassiveVacuumMaster::M6 => stats.m6_component_calls += 1,
                    MassiveVacuumMaster::S2 => unreachable!(),
                }
                targets.insert(target);
            }
        }
    }
    if classes
        != BTreeMap::from([
            (FourLoopThreeLoopProductClass::T1B4, 223),
            (FourLoopThreeLoopProductClass::T1F5, 494),
            (FourLoopThreeLoopProductClass::T1M6, 106),
        ])
    {
        return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
            leaf_id: u32::MAX,
            stage: "product class census",
        });
    }
    stats.completed_plans = completed_indices.len();
    stats.outside_plans = outside_leaf_ids.len();
    stats.unique_targets = targets.len();
    stats.cache_hits = stats
        .component_calls
        .checked_sub(stats.unique_targets)
        .ok_or(FourLoopThreeLoopClosureError::ArithmeticOverflow {
            resource: "cache hits",
        })?;
    check_structural_stats(stats)?;
    check_actual_stats(config, stats)?;
    Ok(Prescan {
        context: context.ok_or(FourLoopThreeLoopClosureError::ReplayMismatch {
            leaf_id: u32::MAX,
            stage: "missing context",
        })?,
        completed_indices,
        outside_leaf_ids,
        targets: targets.into_iter().collect(),
        stats,
    })
}

fn classify_product(
    product: &MasterProduct<MassiveVacuumMaster>,
) -> Option<FourLoopThreeLoopProductClass> {
    if product.total_factor_count() != 2 || product.multiplicity(&MassiveVacuumMaster::T1) != 1 {
        return None;
    }
    if product.multiplicity(&MassiveVacuumMaster::B4) == 1 {
        Some(FourLoopThreeLoopProductClass::T1B4)
    } else if product.multiplicity(&MassiveVacuumMaster::F5) == 1 {
        Some(FourLoopThreeLoopProductClass::T1F5)
    } else if product.multiplicity(&MassiveVacuumMaster::M6) == 1 {
        Some(FourLoopThreeLoopProductClass::T1M6)
    } else {
        None
    }
}

fn reconstruct_targets(
    plan: &FourLoopComponentTransportPlan,
    branch: &FourLoopComponentScalarBranch,
) -> Result<Vec<FourLoopThreeLoopLocalTarget>, FourLoopThreeLoopClosureError> {
    let mut targets = plan
        .components()
        .iter()
        .map(|component| {
            FourLoopThreeLoopLocalTarget::new(component.master(), component.local_powers().to_vec())
        })
        .collect::<Result<Vec<_>, _>>()?;
    match branch.kind() {
        FourLoopComponentScalarBranchKind::Base | FourLoopComponentScalarBranchKind::Constant => {
            if branch.lowered_component_powers().is_some() {
                return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
                    leaf_id: plan.leaf_id(),
                    stage: "base/constant lowering",
                });
            }
        }
        FourLoopComponentScalarBranchKind::Local {
            component_index,
            local_position,
        } => {
            let component = plan.components().get(component_index).ok_or(
                FourLoopThreeLoopClosureError::ReplayMismatch {
                    leaf_id: plan.leaf_id(),
                    stage: "local component",
                },
            )?;
            let lowered = branch.lowered_component_powers().ok_or(
                FourLoopThreeLoopClosureError::ReplayMismatch {
                    leaf_id: plan.leaf_id(),
                    stage: "missing lowering",
                },
            )?;
            if lowered.len() != component.local_powers().len()
                || local_position >= lowered.len()
                || lowered.iter().enumerate().any(|(position, power)| {
                    *power
                        != component.local_powers()[position]
                            - i32::from(position == local_position)
                })
            {
                return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
                    leaf_id: plan.leaf_id(),
                    stage: "exact local lowering",
                });
            }
            targets[component_index] =
                FourLoopThreeLoopLocalTarget::new(component.master(), lowered.to_vec())?;
        }
    }
    Ok(targets)
}

fn build_plan_closure(
    source: &FourLoopComponentTransportPlan,
    service: &FourLoopThreeLoopService,
    target_indices: &BTreeMap<FourLoopThreeLoopLocalTarget, usize>,
    arithmetic: &mut CheckedArithmetic,
    stats: &mut FourLoopThreeLoopClosureStats,
    config: FourLoopThreeLoopClosureConfig,
) -> Result<FourLoopThreeLoopPlanClosure, FourLoopThreeLoopClosureError> {
    let product_class = classify_product(source.key().product()).ok_or(
        FourLoopThreeLoopClosureError::UnsupportedDomain {
            leaf_id: source.leaf_id(),
            reason: "plan outside three-loop slice",
        },
    )?;
    let mut branches = Vec::new();
    branches
        .try_reserve_exact(source.scalar_branches().len())
        .map_err(|_| FourLoopThreeLoopClosureError::AllocationFailed {
            resource: "plan branch closures",
            requested: source.scalar_branches().len(),
        })?;
    let mut ordinary = ProductLinearCombination::new();
    for (branch_index, branch) in source.scalar_branches().iter().enumerate() {
        let branch_targets = reconstruct_targets(source, branch)?;
        let mut component_uses = Vec::new();
        component_uses
            .try_reserve_exact(branch_targets.len())
            .map_err(|_| FourLoopThreeLoopClosureError::AllocationFailed {
                resource: "branch component uses",
                requested: branch_targets.len(),
            })?;
        let mut unscaled = ProductLinearCombination::from_term(
            MasterProduct::identity(),
            arithmetic.context.one(),
        );
        let before = stats.convolution_pair_operations;
        for (component_index, target) in branch_targets.into_iter().enumerate() {
            let index = *target_indices.get(&target).ok_or(
                FourLoopThreeLoopClosureError::ReplayMismatch {
                    leaf_id: source.leaf_id(),
                    stage: "target cache lookup",
                },
            )?;
            let witness_index = source
                .components()
                .get(component_index)
                .ok_or(FourLoopThreeLoopClosureError::ReplayMismatch {
                    leaf_id: source.leaf_id(),
                    stage: "component witness index",
                })?
                .witness_index();
            component_uses.push(FourLoopThreeLoopComponentUse {
                witness_index,
                target_index: u16::try_from(index).map_err(|_| {
                    FourLoopThreeLoopClosureError::ArithmeticOverflow {
                        resource: "target index",
                    }
                })?,
            });
            unscaled = convolve(
                &unscaled,
                service
                    .reductions()
                    .get(index)
                    .ok_or(FourLoopThreeLoopClosureError::ReplayMismatch {
                        leaf_id: source.leaf_id(),
                        stage: "target reduction index",
                    })?
                    .ordinary(),
                arithmetic,
                stats,
                config,
            )?;
        }
        validate_products(source.leaf_id(), &unscaled)?;
        stats.precollection_terms = checked_sum(
            stats.precollection_terms,
            unscaled.len(),
            "precollection terms",
        )?;
        let scaled = scale(&unscaled, branch.coefficient(), arithmetic)?;
        add_combination(&mut ordinary, &scaled, arithmetic)?;
        branches.push(FourLoopThreeLoopBranchClosure {
            branch_index,
            kind: branch.kind(),
            coefficient: branch.coefficient().clone(),
            component_uses,
            convolution_pair_operations: stats.convolution_pair_operations - before,
            ordinary_unscaled: unscaled,
            ordinary_scaled: scaled,
        });
    }
    validate_products(source.leaf_id(), &ordinary)?;
    stats.collected_terms = checked_sum(stats.collected_terms, ordinary.len(), "collected terms")?;
    let mass_normalized = mass_normalize(source, &ordinary, arithmetic, stats, config)?;
    Ok(FourLoopThreeLoopPlanClosure {
        leaf_id: source.leaf_id(),
        product_class,
        branches,
        ordinary,
        mass_normalized,
    })
}

fn validate_products(
    leaf_id: u32,
    value: &ProductLinearCombination<MassiveVacuumMaster>,
) -> Result<(), FourLoopThreeLoopClosureError> {
    for product in value.terms().keys() {
        if product_loop_weight(product) != 4 || !is_allowed_output(product) {
            return Err(FourLoopThreeLoopClosureError::ProductOutsideSlice {
                leaf_id,
                product: product.clone(),
            });
        }
    }
    Ok(())
}
fn is_allowed_output(product: &MasterProduct<MassiveVacuumMaster>) -> bool {
    product
        == &MasterProduct::try_from_multiplicities([(MassiveVacuumMaster::T1, 4)]).expect("small")
        || product
            == &MasterProduct::try_from_multiplicities([
                (MassiveVacuumMaster::T1, 2),
                (MassiveVacuumMaster::S2, 1),
            ])
            .expect("small")
        || classify_product(product).is_some()
}
fn convolve(
    left: &ProductLinearCombination<MassiveVacuumMaster>,
    right: &ProductLinearCombination<MassiveVacuumMaster>,
    arithmetic: &mut CheckedArithmetic,
    stats: &mut FourLoopThreeLoopClosureStats,
    config: FourLoopThreeLoopClosureConfig,
) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopThreeLoopClosureError> {
    let pairs = left.len().checked_mul(right.len()).ok_or(
        FourLoopThreeLoopClosureError::ArithmeticOverflow {
            resource: "convolution pairs",
        },
    )?;
    stats.convolution_pair_operations = checked_sum(
        stats.convolution_pair_operations,
        pairs,
        "convolution pairs",
    )?;
    if stats.convolution_pair_operations > config.max_convolution_pair_operations {
        return Err(FourLoopThreeLoopClosureError::ResourceLimit {
            resource: "convolution pairs",
            requested: stats.convolution_pair_operations as u128,
            limit: config.max_convolution_pair_operations as u128,
        });
    }
    let mut output = ProductLinearCombination::new();
    for (lp, lc) in left.terms() {
        for (rp, rc) in right.terms() {
            add_term(
                &mut output,
                lp.checked_multiply(rp)?,
                arithmetic.multiply(lc, rc)?,
                arithmetic,
            )?;
        }
    }
    Ok(output)
}
fn scale(
    input: &ProductLinearCombination<MassiveVacuumMaster>,
    factor: &Coefficient,
    arithmetic: &mut CheckedArithmetic,
) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopThreeLoopClosureError> {
    let mut output = ProductLinearCombination::new();
    for (product, coefficient) in input.terms() {
        add_term(
            &mut output,
            product.clone(),
            arithmetic.multiply(coefficient, factor)?,
            arithmetic,
        )?;
    }
    Ok(output)
}
fn add_combination(
    output: &mut ProductLinearCombination<MassiveVacuumMaster>,
    input: &ProductLinearCombination<MassiveVacuumMaster>,
    arithmetic: &mut CheckedArithmetic,
) -> Result<(), FourLoopThreeLoopClosureError> {
    for (product, coefficient) in input.terms() {
        add_term(output, product.clone(), coefficient.clone(), arithmetic)?;
    }
    Ok(())
}
fn add_term(
    output: &mut ProductLinearCombination<MassiveVacuumMaster>,
    product: MasterProduct<MassiveVacuumMaster>,
    coefficient: Coefficient,
    arithmetic: &mut CheckedArithmetic,
) -> Result<(), FourLoopThreeLoopClosureError> {
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
    stats: &mut FourLoopThreeLoopClosureStats,
    config: FourLoopThreeLoopClosureConfig,
) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopThreeLoopClosureError> {
    let input_weight = source
        .key()
        .powers()
        .iter()
        .try_fold(0_i64, |sum, power| sum.checked_add(i64::from(*power)))
        .ok_or(FourLoopThreeLoopClosureError::ArithmeticOverflow {
            resource: "input mass weight",
        })?;
    let mass_position = arithmetic
        .context
        .parameter_names()
        .iter()
        .position(|name| name == "m2")
        .ok_or(FourLoopThreeLoopClosureError::CoefficientContextMismatch)?;
    let mass = arithmetic
        .context
        .parameter("m2")
        .ok_or(FourLoopThreeLoopClosureError::CoefficientContextMismatch)?;
    arithmetic.charge()?;
    let inverse = &arithmetic.context.one() / &mass;
    arithmetic.check_existing(&inverse)?;
    let mut output = ProductLinearCombination::new();
    for (product, coefficient) in ordinary.terms() {
        let exponent = input_weight
            .checked_sub(product_mass_weight(product))
            .ok_or(FourLoopThreeLoopClosureError::ArithmeticOverflow {
                resource: "mass exponent",
            })?;
        let steps = usize::try_from(exponent.unsigned_abs()).map_err(|_| {
            FourLoopThreeLoopClosureError::ArithmeticOverflow {
                resource: "mass steps",
            }
        })?;
        stats.mass_power_steps = checked_sum(stats.mass_power_steps, steps, "mass steps")?;
        if stats.mass_power_steps > config.max_mass_power_steps {
            return Err(FourLoopThreeLoopClosureError::ResourceLimit {
                resource: "mass steps",
                requested: stats.mass_power_steps as u128,
                limit: config.max_mass_power_steps as u128,
            });
        }
        let factor = if exponent >= 0 { &mass } else { &inverse };
        let mut value = coefficient.clone();
        for _ in 0..steps {
            value = arithmetic.multiply(&value, factor)?;
        }
        let (n, d) = coefficient_variable_degrees(&value)
            .get(mass_position)
            .copied()
            .ok_or(FourLoopThreeLoopClosureError::CoefficientContextMismatch)?;
        if n != 0 || d != 0 {
            return Err(FourLoopThreeLoopClosureError::ResidualMassDependence {
                leaf_id: source.leaf_id(),
                product: product.clone(),
                numerator_degree: n,
                denominator_degree: d,
            });
        }
        output.add_term(product.clone(), value);
    }
    Ok(output)
}

struct CheckedArithmetic {
    context: CoefficientContext,
    zero: Coefficient,
    config: FourLoopThreeLoopClosureConfig,
    operations: usize,
}
impl CheckedArithmetic {
    fn new(context: CoefficientContext, config: FourLoopThreeLoopClosureConfig) -> Self {
        let zero = context.zero();
        Self {
            context,
            zero,
            config,
            operations: 0,
        }
    }
    fn charge(&mut self) -> Result<(), FourLoopThreeLoopClosureError> {
        self.operations = self.operations.checked_add(1).ok_or(
            FourLoopThreeLoopClosureError::ArithmeticOverflow {
                resource: "coefficient operations",
            },
        )?;
        if self.operations > self.config.max_coefficient_operations {
            return Err(FourLoopThreeLoopClosureError::ResourceLimit {
                resource: "coefficient operations",
                requested: self.operations as u128,
                limit: self.config.max_coefficient_operations as u128,
            });
        }
        Ok(())
    }
    fn check_existing(&self, value: &Coefficient) -> Result<(), FourLoopThreeLoopClosureError> {
        if value.get_variables() != self.zero.get_variables() {
            return Err(FourLoopThreeLoopClosureError::CoefficientContextMismatch);
        }
        let degree = coefficient_variable_degrees(value)
            .into_iter()
            .map(|(n, d)| n.max(d))
            .max()
            .unwrap_or(0);
        if degree > self.config.max_coefficient_degree
            || !symbolica_coefficient_degree_is_representable(degree)
        {
            return Err(FourLoopThreeLoopClosureError::ResourceLimit {
                resource: "coefficient degree",
                requested: degree,
                limit: self.config.max_coefficient_degree,
            });
        }
        Ok(())
    }
    fn multiply(
        &mut self,
        left: &Coefficient,
        right: &Coefficient,
    ) -> Result<Coefficient, FourLoopThreeLoopClosureError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        let degree = coefficient_product_degree_bound(left, right);
        if degree > self.config.max_coefficient_degree {
            return Err(FourLoopThreeLoopClosureError::ResourceLimit {
                resource: "coefficient product degree",
                requested: degree,
                limit: self.config.max_coefficient_degree,
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
    ) -> Result<Coefficient, FourLoopThreeLoopClosureError> {
        self.check_existing(left)?;
        self.check_existing(right)?;
        let degree = coefficient_sum_degree_bound(left, right);
        if degree > self.config.max_coefficient_degree {
            return Err(FourLoopThreeLoopClosureError::ResourceLimit {
                resource: "coefficient sum degree",
                requested: degree,
                limit: self.config.max_coefficient_degree,
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
    completed: &BTreeMap<u32, usize>,
    stats: &mut FourLoopThreeLoopClosureStats,
) -> Result<Vec<FourLoopThreeLoopClosureOccurrence>, FourLoopThreeLoopClosureError> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(transport.occurrences().len())
        .map_err(|_| FourLoopThreeLoopClosureError::AllocationFailed {
            resource: "three-loop occurrence partition",
            requested: transport.occurrences().len(),
        })?;
    let mut completed_rows = BTreeSet::new();
    let mut outside_rows = BTreeSet::new();
    for source in transport.occurrences() {
        let plan = transport.plans().get(source.plan_index() as usize).ok_or(
            FourLoopThreeLoopClosureError::ReplayMismatch {
                leaf_id: source.leaf_id(),
                stage: "occurrence source plan index",
            },
        )?;
        if plan.leaf_id() != source.leaf_id() {
            return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
                leaf_id: source.leaf_id(),
                stage: "occurrence source leaf",
            });
        }
        let index = completed
            .get(&source.leaf_id())
            .copied()
            .map(|value| {
                u16::try_from(value).map_err(|_| {
                    FourLoopThreeLoopClosureError::ArithmeticOverflow {
                        resource: "completed plan index",
                    }
                })
            })
            .transpose()?;
        if index.is_some() {
            stats.completed_occurrences += 1;
            completed_rows.insert(source.row_index());
        } else {
            stats.outside_occurrences += 1;
            outside_rows.insert(source.row_index());
        }
        output.push(occurrence_record(*source, index));
    }
    stats.completed_rows = completed_rows.len();
    stats.outside_rows = outside_rows.len();
    stats.mixed_rows = completed_rows.intersection(&outside_rows).count();
    Ok(output)
}
fn occurrence_record(
    source: FourLoopComponentTransportOccurrence,
    completed_plan_index: Option<u16>,
) -> FourLoopThreeLoopClosureOccurrence {
    FourLoopThreeLoopClosureOccurrence {
        row_index: source.row_index(),
        path_index: source.path_index(),
        leaf_id: source.leaf_id(),
        completed_plan_index,
    }
}

fn check_structural_stats(
    stats: FourLoopThreeLoopClosureStats,
) -> Result<(), FourLoopThreeLoopClosureError> {
    for (resource, expected, actual) in [
        ("plans", 823, stats.completed_plans),
        ("outside plans", 243, stats.outside_plans),
        ("components", 1646, stats.components),
        ("local slots", 5761, stats.local_slots),
        ("branches", 1884, stats.scalar_branches),
        ("calls", 3768, stats.component_calls),
        ("targets", 204, stats.unique_targets),
        ("N0", 443, stats.n0_plans),
        ("N1", 380, stats.n1_plans),
        ("base", 443, stats.base_branches),
        ("constant", 323, stats.constant_branches),
        ("local T1", 186, stats.local_t1_branches),
        ("local B4", 220, stats.local_b4_branches),
        ("local F5", 656, stats.local_f5_branches),
        ("local M6", 56, stats.local_m6_branches),
        ("T1 calls", 1884, stats.t1_component_calls),
        ("B4 calls", 444, stats.b4_component_calls),
        ("F5 calls", 1260, stats.f5_component_calls),
        ("M6 calls", 180, stats.m6_component_calls),
    ] {
        if actual != expected {
            return Err(FourLoopThreeLoopClosureError::CensusMismatch {
                resource,
                expected,
                actual,
            });
        }
    }
    Ok(())
}
fn check_actual_stats(
    config: FourLoopThreeLoopClosureConfig,
    stats: FourLoopThreeLoopClosureStats,
) -> Result<(), FourLoopThreeLoopClosureError> {
    for (resource, requested, limit) in [
        ("plans", stats.completed_plans, config.max_plans),
        (
            "outside plans",
            stats.outside_plans,
            config.max_outside_plans,
        ),
        (
            "occurrences",
            stats.completed_occurrences + stats.outside_occurrences,
            config.max_occurrences,
        ),
        ("components", stats.components, config.max_components),
        ("local slots", stats.local_slots, config.max_local_slots),
        (
            "branches",
            stats.scalar_branches,
            config.max_scalar_branches,
        ),
        ("calls", stats.component_calls, config.max_component_calls),
        ("targets", stats.unique_targets, config.max_unique_targets),
        (
            "convolution pairs",
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
            "mass steps",
            stats.mass_power_steps,
            config.max_mass_power_steps,
        ),
        (
            "coefficient operations",
            stats.coefficient_operations,
            config.max_coefficient_operations,
        ),
        (
            "retained coefficient bytes",
            stats.retained_output_coefficient_bytes,
            config.max_retained_output_coefficient_bytes,
        ),
    ] {
        if requested > limit {
            return Err(FourLoopThreeLoopClosureError::ResourceLimit {
                resource,
                requested: requested as u128,
                limit: limit as u128,
            });
        }
    }
    Ok(())
}

fn check_occurrence_partition(
    stats: FourLoopThreeLoopClosureStats,
) -> Result<(), FourLoopThreeLoopClosureError> {
    if stats.completed_occurrences != FOUR_LOOP_THREE_LOOP_CLOSURE_COMPLETED_OCCURRENCES
        || stats.outside_occurrences != FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_OCCURRENCES
        || stats.completed_rows != FOUR_LOOP_THREE_LOOP_CLOSURE_COMPLETED_ROWS
        || stats.outside_rows != FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_ROWS
        || stats.mixed_rows != FOUR_LOOP_THREE_LOOP_CLOSURE_MIXED_ROWS
    {
        return Err(FourLoopThreeLoopClosureError::ReplayMismatch {
            leaf_id: u32::MAX,
            stage: "occurrence and row census",
        });
    }
    Ok(())
}
fn retained_output_bytes(
    service: &FourLoopThreeLoopService,
    plans: &[FourLoopThreeLoopPlanClosure],
) -> Result<usize, FourLoopThreeLoopClosureError> {
    let mut total = service.retained_output_coefficient_bytes();
    let mut add = |coefficient: &Coefficient| -> Result<(), FourLoopThreeLoopClosureError> {
        total = total.checked_add(coefficient.to_string().len()).ok_or(
            FourLoopThreeLoopClosureError::ArithmeticOverflow {
                resource: "retained bytes",
            },
        )?;
        Ok(())
    };
    for plan in plans {
        for branch in &plan.branches {
            add(&branch.coefficient)?;
            for value in branch
                .ordinary_unscaled
                .terms()
                .values()
                .chain(branch.ordinary_scaled.terms().values())
            {
                add(value)?;
            }
        }
        for value in plan
            .ordinary
            .terms()
            .values()
            .chain(plan.mass_normalized.terms().values())
        {
            add(value)?;
        }
    }
    Ok(total)
}
fn checked_sum(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, FourLoopThreeLoopClosureError> {
    left.checked_add(right)
        .ok_or(FourLoopThreeLoopClosureError::ArithmeticOverflow { resource })
}
fn product_loop_weight(product: &MasterProduct<MassiveVacuumMaster>) -> u128 {
    product
        .factors()
        .iter()
        .map(|(master, multiplicity)| master.loops() as u128 * u128::from(*multiplicity))
        .sum()
}
fn product_mass_weight(product: &MasterProduct<MassiveVacuumMaster>) -> i64 {
    product
        .factors()
        .iter()
        .map(|(master, multiplicity)| i64::from(*multiplicity) * master.physical_lines() as i64)
        .sum()
}

fn closure_checksum(
    transport: &FourLoopComponentTransport<'_>,
    config: FourLoopThreeLoopClosureConfig,
    context: &CoefficientContext,
    service: &FourLoopThreeLoopService,
    plans: &[FourLoopThreeLoopPlanClosure],
    outside: &[u32],
    occurrences: &[FourLoopThreeLoopClosureOccurrence],
    stats: FourLoopThreeLoopClosureStats,
) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_bytes(&mut hash, FourLoopThreeLoopClosure::SCHEMA.as_bytes());
    hash_bytes(&mut hash, transport.source_schema().as_bytes());
    hash_u64(&mut hash, transport.source_seed_checksum());
    for name in context.parameter_names() {
        hash_bytes(&mut hash, name.as_bytes());
    }
    hash_u64(&mut hash, service.checksum());
    for (plan, source) in plans.iter().zip(
        transport
            .plans()
            .iter()
            .filter(|source| classify_product(source.key().product()).is_some()),
    ) {
        hash_u64(&mut hash, u64::from(plan.leaf_id));
        hash_bytes(&mut hash, plan.product_class.stable_key().as_bytes());
        hash_bytes(&mut hash, source.key().family_fingerprint().as_bytes());
        for power in source.key().powers() {
            hash_bytes(&mut hash, &power.to_le_bytes());
        }
        for branch in &plan.branches {
            hash_u64(&mut hash, branch.branch_index as u64);
            hash_branch_kind(&mut hash, branch.kind);
            hash_u64(&mut hash, branch.convolution_pair_operations as u64);
            hash_bytes(&mut hash, branch.coefficient.to_string().as_bytes());
            for usage in &branch.component_uses {
                hash_u64(&mut hash, usage.witness_index as u64);
                hash_u64(&mut hash, u64::from(usage.target_index));
            }
            hash_combination(&mut hash, &branch.ordinary_unscaled);
            hash_combination(&mut hash, &branch.ordinary_scaled);
        }
        hash_combination(&mut hash, &plan.ordinary);
        hash_combination(&mut hash, &plan.mass_normalized);
    }
    for value in outside {
        hash_u64(&mut hash, u64::from(*value));
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
        stats.outside_plans,
        stats.completed_occurrences,
        stats.outside_occurrences,
        stats.completed_rows,
        stats.outside_rows,
        stats.mixed_rows,
        stats.components,
        stats.local_slots,
        stats.scalar_branches,
        stats.base_branches,
        stats.constant_branches,
        stats.local_t1_branches,
        stats.local_b4_branches,
        stats.local_f5_branches,
        stats.local_m6_branches,
        stats.component_calls,
        stats.t1_component_calls,
        stats.b4_component_calls,
        stats.f5_component_calls,
        stats.m6_component_calls,
        stats.unique_targets,
        stats.cache_hits,
        stats.convolution_pair_operations,
        stats.precollection_terms,
        stats.collected_terms,
        stats.mass_power_steps,
        stats.coefficient_operations,
        stats.retained_output_coefficient_bytes,
        config.max_coefficient_operations,
    ] {
        hash_u64(&mut hash, value as u64);
    }
    for value in [
        config.max_plans,
        config.max_outside_plans,
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
        config.max_retained_output_coefficient_bytes,
    ] {
        hash_u64(&mut hash, value as u64);
    }
    hash_u64(&mut hash, config.max_coefficient_degree as u64);
    hash_u64(&mut hash, stats.n0_plans as u64);
    hash_u64(&mut hash, stats.n1_plans as u64);
    hash
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
fn hash_combination(hash: &mut u64, value: &ProductLinearCombination<MassiveVacuumMaster>) {
    hash_u64(hash, value.len() as u64);
    for (product, coefficient) in value.terms() {
        for (master, multiplicity) in product.factors() {
            hash_bytes(hash, master.stable_key().as_bytes());
            hash_u64(hash, u64::from(*multiplicity));
        }
        hash_bytes(hash, coefficient.to_string().as_bytes());
    }
}
fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
    *hash ^= 0xff;
    *hash = hash.wrapping_mul(FNV1A64_PRIME);
}
fn hash_u64(hash: &mut u64, value: u64) {
    hash_bytes(hash, &value.to_le_bytes());
}
