//! Exact lower-loop service for the transported `T1`, `B4`, `F5`, and `M6`
//! component-target manifest.
//!
//! This module deliberately stops before four-loop branch convolution.  It
//! authenticates the fixed 204-target manifest induced by
//! [`crate::FourLoopComponentTransport`], builds one finite three-loop
//! `D<=2,N<=1` certificate in the caller's exact Symbolica coefficient
//! context, and retains each target's ordinary semantic master-product
//! reduction.  The component owner is part of every target key even when a
//! lowering has pinched that component into a smaller three-loop sector.
//!
//! The three-loop table is a deterministic finite-box certificate over
//! `Q(d,m2)`.  Its five outputs are candidate terminals of that bounded
//! certificate, not a proof of unrestricted master minimality.  The underlying
//! generic sparse table does not retain compact source-row weights or a
//! separate exceptional-factor list.  Consequently every coefficient carries
//! the usual generic-domain caveat that denominator factors introduced by
//! exact pivots must be nonzero.

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::fmt;

use crate::{
    FamilyError, IbpGenerationError, IbpGenerator, Integral, LinearCombination, ReductionStats,
    VacuumFamily,
};
use crate::{
    MassiveVacuumMaster, OneLoopTadpoleConfig, OneLoopTadpoleError, OneLoopTadpoleReducer,
    ThreeLoopPipelineError, ThreeLoopReductionConfig, ThreeLoopReductionPipeline,
    equal_mass_three_loop_tetrahedron_in_context, three_loop_f5_d2n1_pipeline_config,
};
use rustred::{
    Coefficient, CoefficientContext, MasterProduct, MasterProductError, ProductLinearCombination,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};

pub const FOUR_LOOP_THREE_LOOP_SERVICE_TARGETS: usize = 204;
pub const FOUR_LOOP_THREE_LOOP_SERVICE_T1_TARGETS: usize = 4;
pub const FOUR_LOOP_THREE_LOOP_SERVICE_B4_TARGETS: usize = 41;
pub const FOUR_LOOP_THREE_LOOP_SERVICE_F5_TARGETS: usize = 89;
pub const FOUR_LOOP_THREE_LOOP_SERVICE_M6_TARGETS: usize = 70;

/// Every non-tadpole target has nine native `d/dk_i . k_j` rows.
pub const FOUR_LOOP_THREE_LOOP_SERVICE_NATIVE_IDENTITIES: usize =
    (FOUR_LOOP_THREE_LOOP_SERVICE_TARGETS - FOUR_LOOP_THREE_LOOP_SERVICE_T1_TARGETS) * 9;

/// Frozen exact aggregate semantic-output census for the 204-target service.
/// The looser five-term-per-target envelope would be 1,020 terms.
pub const FOUR_LOOP_THREE_LOOP_SERVICE_OUTPUT_TERM_BOUND: usize = 502;

/// Conservative retained semantic-output coefficient envelope. Construction
/// also measures the actual output byte count and rejects an overrun. This is
/// not a whole-object or Symbolica-workspace memory bound.
pub const FOUR_LOOP_THREE_LOOP_SERVICE_RETAINED_OUTPUT_COEFFICIENT_BYTE_BOUND: usize = 12_555;

/// FNV-1a checksum of the sorted exact target manifest.  Each record hashes a
/// stable one-byte owner tag, the little-endian power-vector length, and every
/// little-endian signed power.  This is a corruption/replay checksum, not a
/// cryptographic authentication primitive.
pub const FOUR_LOOP_THREE_LOOP_SERVICE_TARGET_MANIFEST_CHECKSUM: u64 = 0x9bb3_c1a6_d4ea_7bdd;

const FNV1A64_OFFSET: u64 = 0xcbf_29ce4_8422_2325;
const FNV1A64_PRIME: u64 = 0x100_0000_01b3;

/// Frozen `(owner, dot degree, numerator degree, labelled targets)` census.
pub const FOUR_LOOP_THREE_LOOP_SERVICE_DEGREE_CENSUS: [(MassiveVacuumMaster, u64, u64, usize); 17] = [
    (MassiveVacuumMaster::T1, 0, 0, 2),
    (MassiveVacuumMaster::T1, 1, 0, 1),
    (MassiveVacuumMaster::T1, 2, 0, 1),
    (MassiveVacuumMaster::B4, 0, 0, 5),
    (MassiveVacuumMaster::B4, 0, 1, 2),
    (MassiveVacuumMaster::B4, 1, 0, 16),
    (MassiveVacuumMaster::B4, 1, 1, 8),
    (MassiveVacuumMaster::B4, 2, 0, 10),
    (MassiveVacuumMaster::F5, 0, 0, 6),
    (MassiveVacuumMaster::F5, 0, 1, 1),
    (MassiveVacuumMaster::F5, 1, 0, 25),
    (MassiveVacuumMaster::F5, 1, 1, 5),
    (MassiveVacuumMaster::F5, 2, 0, 43),
    (MassiveVacuumMaster::F5, 2, 1, 9),
    (MassiveVacuumMaster::M6, 0, 0, 6),
    (MassiveVacuumMaster::M6, 1, 0, 24),
    (MassiveVacuumMaster::M6, 2, 0, 40),
];

/// Stable prescan key.  `owner` records the transported component identity;
/// `powers` are always expressed in that component's complete local basis.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FourLoopThreeLoopLocalTarget {
    owner: MassiveVacuumMaster,
    powers: Vec<i32>,
}

impl FourLoopThreeLoopLocalTarget {
    /// Construct a checked complete-basis target.  T1 uses one power slot;
    /// B4/F5/M6 use the same six-slot tetrahedron basis.  S2 belongs to the
    /// separate T1/S2 closure and is rejected here.
    pub fn new(
        owner: MassiveVacuumMaster,
        powers: impl Into<Vec<i32>>,
    ) -> Result<Self, FourLoopThreeLoopServiceError> {
        let powers = powers.into();
        validate_target_shape(owner, powers.len())?;
        Ok(Self { owner, powers })
    }

    pub fn tadpole(power: i32) -> Self {
        Self {
            owner: MassiveVacuumMaster::T1,
            powers: vec![power],
        }
    }

    pub fn three_loop(
        owner: MassiveVacuumMaster,
        powers: [i32; 6],
    ) -> Result<Self, FourLoopThreeLoopServiceError> {
        Self::new(owner, powers.to_vec())
    }

    pub const fn owner(&self) -> MassiveVacuumMaster {
        self.owner
    }

    pub fn powers(&self) -> &[i32] {
        &self.powers
    }

    pub fn stable_key(&self) -> String {
        format!(
            "rustred-four-loop-three-loop-local-target-v1:{}:[{}]",
            self.owner.stable_key(),
            self.powers
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join(",")
        )
    }
}

/// Conservative service limits.  Target-count, output-term, and retained-byte
/// envelopes are checked before the finite pipeline is built; actual output
/// terms and retained bytes are checked again while materializing results.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FourLoopThreeLoopServiceConfig {
    pub max_targets: usize,
    pub max_t1_targets: usize,
    pub max_b4_targets: usize,
    pub max_f5_targets: usize,
    pub max_m6_targets: usize,
    /// Aggregate semantic output terms retained across all targets.
    pub max_output_terms: usize,
    pub max_retained_output_coefficient_bytes: usize,
    pub one_loop: OneLoopTadpoleConfig,
}

impl Default for FourLoopThreeLoopServiceConfig {
    fn default() -> Self {
        Self {
            max_targets: FOUR_LOOP_THREE_LOOP_SERVICE_TARGETS,
            max_t1_targets: FOUR_LOOP_THREE_LOOP_SERVICE_T1_TARGETS,
            max_b4_targets: FOUR_LOOP_THREE_LOOP_SERVICE_B4_TARGETS,
            max_f5_targets: FOUR_LOOP_THREE_LOOP_SERVICE_F5_TARGETS,
            max_m6_targets: FOUR_LOOP_THREE_LOOP_SERVICE_M6_TARGETS,
            max_output_terms: FOUR_LOOP_THREE_LOOP_SERVICE_OUTPUT_TERM_BOUND,
            max_retained_output_coefficient_bytes:
                FOUR_LOOP_THREE_LOOP_SERVICE_RETAINED_OUTPUT_COEFFICIENT_BYTE_BOUND,
            one_loop: OneLoopTadpoleConfig {
                max_recurrence_steps: 2,
                max_coefficient_operations: 8,
                max_dense_term_operations: 24,
                max_coefficient_degree: 2,
            },
        }
    }
}

/// Exact semantics claimed by this service.  In particular this status does
/// not claim an unrestricted three-loop master basis or expose all pivot
/// exceptional factors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FourLoopThreeLoopServiceStatus {
    ExactFiniteBoxGenericQ,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FourLoopThreeLoopServiceStats {
    targets: usize,
    t1_targets: usize,
    b4_targets: usize,
    f5_targets: usize,
    m6_targets: usize,
    native_target_identities: usize,
    output_terms: usize,
    retained_output_coefficient_bytes: usize,
}

impl FourLoopThreeLoopServiceStats {
    pub const fn targets(self) -> usize {
        self.targets
    }
    pub const fn t1_targets(self) -> usize {
        self.t1_targets
    }
    pub const fn b4_targets(self) -> usize {
        self.b4_targets
    }
    pub const fn f5_targets(self) -> usize {
        self.f5_targets
    }
    pub const fn m6_targets(self) -> usize {
        self.m6_targets
    }
    pub const fn native_target_identities(self) -> usize {
        self.native_target_identities
    }
    pub const fn output_terms(self) -> usize {
        self.output_terms
    }
    pub const fn retained_output_coefficient_bytes(self) -> usize {
        self.retained_output_coefficient_bytes
    }
}

/// One retained ordinary local reduction.  No four-loop T1 convolution or
/// parent mass normalization has been applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FourLoopThreeLoopLocalReduction {
    target: FourLoopThreeLoopLocalTarget,
    ordinary: ProductLinearCombination<MassiveVacuumMaster>,
}

impl FourLoopThreeLoopLocalReduction {
    pub const fn target(&self) -> &FourLoopThreeLoopLocalTarget {
        &self.target
    }

    pub const fn ordinary(&self) -> &ProductLinearCombination<MassiveVacuumMaster> {
        &self.ordinary
    }

    #[doc(hidden)]
    pub fn with_target_for_replay(&self, target: FourLoopThreeLoopLocalTarget) -> Self {
        let mut candidate = self.clone();
        candidate.target = target;
        candidate
    }

    #[doc(hidden)]
    pub fn with_output_coefficient_for_replay(
        &self,
        product: &MasterProduct<MassiveVacuumMaster>,
        coefficient: Coefficient,
    ) -> Self {
        let mut candidate = self.clone();
        candidate.ordinary.remove(product);
        candidate.ordinary.add_term(product.clone(), coefficient);
        candidate
    }
}

#[derive(Debug)]
pub enum FourLoopThreeLoopServiceError {
    Family(FamilyError),
    Tadpole(OneLoopTadpoleError),
    Pipeline(ThreeLoopPipelineError),
    Ibp(IbpGenerationError),
    Product(MasterProductError),
    UnsupportedOwner {
        owner: MassiveVacuumMaster,
    },
    WrongPowerArity {
        owner: MassiveVacuumMaster,
        expected: usize,
        actual: usize,
    },
    DuplicateTarget {
        target: FourLoopThreeLoopLocalTarget,
    },
    CensusMismatch {
        resource: &'static str,
        expected: usize,
        actual: usize,
    },
    DegreeCensusMismatch,
    ManifestChecksumMismatch {
        expected: u64,
        actual: u64,
    },
    UnsupportedCoefficientContext,
    CoefficientContextMismatch,
    ResourceLimit {
        resource: &'static str,
        requested: u128,
        limit: u128,
    },
    AllocationFailed {
        resource: &'static str,
        requested: usize,
    },
    UnexpectedTerminal {
        target: FourLoopThreeLoopLocalTarget,
        terminal: Integral,
    },
    UnexpectedSemanticLoopWeight {
        target: FourLoopThreeLoopLocalTarget,
        product: MasterProduct<MassiveVacuumMaster>,
        expected: usize,
        actual: u128,
    },
    OutsideManifest {
        target: FourLoopThreeLoopLocalTarget,
    },
    ReplayMismatch {
        stage: &'static str,
    },
    ArithmeticOverflow {
        resource: &'static str,
    },
}

impl fmt::Display for FourLoopThreeLoopServiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "four-loop three-loop target service: {self:?}")
    }
}

impl Error for FourLoopThreeLoopServiceError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Family(error) => Some(error),
            Self::Tadpole(error) => Some(error),
            Self::Pipeline(error) => Some(error),
            Self::Ibp(error) => Some(error),
            Self::Product(error) => Some(error),
            _ => None,
        }
    }
}

impl From<FamilyError> for FourLoopThreeLoopServiceError {
    fn from(error: FamilyError) -> Self {
        Self::Family(error)
    }
}

impl From<OneLoopTadpoleError> for FourLoopThreeLoopServiceError {
    fn from(error: OneLoopTadpoleError) -> Self {
        Self::Tadpole(error)
    }
}

impl From<ThreeLoopPipelineError> for FourLoopThreeLoopServiceError {
    fn from(error: ThreeLoopPipelineError) -> Self {
        Self::Pipeline(error)
    }
}

impl From<IbpGenerationError> for FourLoopThreeLoopServiceError {
    fn from(error: IbpGenerationError) -> Self {
        Self::Ibp(error)
    }
}

impl From<MasterProductError> for FourLoopThreeLoopServiceError {
    fn from(error: MasterProductError) -> Self {
        Self::Product(error)
    }
}

/// Retained exact reductions of the complete transported local-target
/// manifest.  The two reducers share the caller's exact coefficient map and
/// are built once for the entire batch.
#[derive(Clone, Debug)]
pub struct FourLoopThreeLoopService {
    config: FourLoopThreeLoopServiceConfig,
    coefficient_context: CoefficientContext,
    tadpole: OneLoopTadpoleReducer,
    pipeline: ThreeLoopReductionPipeline,
    family_fingerprint: String,
    pipeline_config: ThreeLoopReductionConfig,
    pipeline_stats: ReductionStats,
    manifest_checksum: u64,
    targets: Vec<FourLoopThreeLoopLocalTarget>,
    reductions: Vec<FourLoopThreeLoopLocalReduction>,
    stats: FourLoopThreeLoopServiceStats,
    checksum: u64,
}

impl FourLoopThreeLoopService {
    pub const SCHEMA: &'static str = "rustred-four-loop-three-loop-target-service-v1";

    /// This caveat is part of the public certificate semantics, not merely an
    /// implementation note.
    pub const GENERIC_Q_CAVEAT: &'static str = "exact finite-box Q(d,m2) certificate; all exact-pivot denominator factors must be nonzero; unrestricted master minimality, compact source-row weights, and a complete exceptional-factor list are not claimed";

    pub fn preflight_config(
        config: FourLoopThreeLoopServiceConfig,
    ) -> Result<(), FourLoopThreeLoopServiceError> {
        preflight_config(config)
    }

    /// Authenticate and reduce an unordered stream of exact prescan targets.
    /// Duplicate entries are errors rather than implicit cache aliases.
    pub fn build(
        coefficient_context: CoefficientContext,
        targets: impl IntoIterator<Item = FourLoopThreeLoopLocalTarget>,
        config: FourLoopThreeLoopServiceConfig,
    ) -> Result<Self, FourLoopThreeLoopServiceError> {
        Self::build_impl(coefficient_context, targets, config)
    }

    fn build_impl(
        coefficient_context: CoefficientContext,
        targets: impl IntoIterator<Item = FourLoopThreeLoopLocalTarget>,
        config: FourLoopThreeLoopServiceConfig,
    ) -> Result<Self, FourLoopThreeLoopServiceError> {
        // These fixed aggregate checks precede target collection, coefficient
        // construction, and the expensive finite-pipeline build.
        preflight_config(config)?;
        let targets = collect_and_validate_manifest(targets, config)?;
        validate_coefficient_context(&coefficient_context)?;

        let tadpole =
            OneLoopTadpoleReducer::new(coefficient_context.clone(), "d", "m2", config.one_loop)?;
        let family = equal_mass_three_loop_tetrahedron_in_context(coefficient_context.clone())?;
        let pipeline_config = three_loop_f5_d2n1_pipeline_config();
        let pipeline = ThreeLoopReductionPipeline::build_for_family(family, pipeline_config)?;
        if !tadpole
            .coefficients()
            .has_same_variable_map(pipeline.family().coefficients())
        {
            return Err(FourLoopThreeLoopServiceError::CoefficientContextMismatch);
        }

        let family_fingerprint = pipeline.family().fingerprint();
        let pipeline_stats = pipeline.stats().clone();
        let manifest_checksum = target_manifest_checksum(&targets);
        debug_assert_eq!(
            manifest_checksum,
            FOUR_LOOP_THREE_LOOP_SERVICE_TARGET_MANIFEST_CHECKSUM
        );

        let mut stats = target_stats(&targets)?;
        let retained_targets = targets.clone();
        let mut reductions = Vec::new();
        reductions.try_reserve_exact(targets.len()).map_err(|_| {
            FourLoopThreeLoopServiceError::AllocationFailed {
                resource: "retained local reductions",
                requested: targets.len(),
            }
        })?;
        for target in targets {
            let reduction = build_local_reduction(&target, &tadpole, &pipeline)?;
            stats.output_terms = checked_add(
                stats.output_terms,
                reduction.ordinary.len(),
                "semantic output terms",
            )?;
            if stats.output_terms > config.max_output_terms {
                return Err(FourLoopThreeLoopServiceError::ResourceLimit {
                    resource: "semantic output terms",
                    requested: stats.output_terms as u128,
                    limit: config.max_output_terms as u128,
                });
            }
            stats.retained_output_coefficient_bytes = checked_add(
                stats.retained_output_coefficient_bytes,
                retained_output_coefficient_bytes(&reduction.ordinary)?,
                "retained output coefficient bytes",
            )?;
            if stats.retained_output_coefficient_bytes
                > config.max_retained_output_coefficient_bytes
            {
                return Err(FourLoopThreeLoopServiceError::ResourceLimit {
                    resource: "retained semantic-output coefficient bytes",
                    requested: stats.retained_output_coefficient_bytes as u128,
                    limit: config.max_retained_output_coefficient_bytes as u128,
                });
            }
            reductions.push(reduction);
        }

        validate_native_target_identities(&pipeline, &reductions)?;
        stats.native_target_identities = FOUR_LOOP_THREE_LOOP_SERVICE_NATIVE_IDENTITIES;

        let checksum = service_checksum(
            config,
            &coefficient_context,
            &family_fingerprint,
            pipeline_config,
            &pipeline_stats,
            pipeline.masters(),
            manifest_checksum,
            &reductions,
            stats,
        );

        Ok(Self {
            config,
            coefficient_context,
            tadpole,
            pipeline,
            family_fingerprint,
            pipeline_config,
            pipeline_stats,
            manifest_checksum,
            targets: retained_targets,
            reductions,
            stats,
            checksum,
        })
    }

    pub const fn status(&self) -> FourLoopThreeLoopServiceStatus {
        FourLoopThreeLoopServiceStatus::ExactFiniteBoxGenericQ
    }

    pub const fn generic_q_caveat(&self) -> &'static str {
        Self::GENERIC_Q_CAVEAT
    }

    pub const fn config(&self) -> FourLoopThreeLoopServiceConfig {
        self.config
    }

    pub const fn coefficient_context(&self) -> &CoefficientContext {
        &self.coefficient_context
    }

    pub fn family(&self) -> &VacuumFamily {
        self.pipeline.family()
    }

    pub fn family_fingerprint(&self) -> &str {
        &self.family_fingerprint
    }

    pub const fn pipeline_config(&self) -> ThreeLoopReductionConfig {
        self.pipeline_config
    }

    pub const fn pipeline_stats(&self) -> &ReductionStats {
        &self.pipeline_stats
    }

    pub const fn manifest_checksum(&self) -> u64 {
        self.manifest_checksum
    }

    /// Deterministic checksum of configuration, context labels, family and
    /// pipeline statistics, exact targets, and retained semantic reductions.
    pub const fn checksum(&self) -> u64 {
        self.checksum
    }

    pub const fn stats(&self) -> FourLoopThreeLoopServiceStats {
        self.stats
    }

    pub fn targets(&self) -> &[FourLoopThreeLoopLocalTarget] {
        &self.targets
    }

    pub fn reductions(&self) -> &[FourLoopThreeLoopLocalReduction] {
        &self.reductions
    }

    /// Canonical bytes retained by semantic output coefficients only. This
    /// does not meter the owned sparse table, caches, allocator overhead, or
    /// Symbolica workspaces.
    pub const fn retained_output_coefficient_bytes(&self) -> usize {
        self.stats.retained_output_coefficient_bytes
    }

    pub fn candidates(&self) -> &[Integral; 5] {
        self.pipeline.masters()
    }

    pub fn reduction(
        &self,
        target: &FourLoopThreeLoopLocalTarget,
    ) -> Result<&FourLoopThreeLoopLocalReduction, FourLoopThreeLoopServiceError> {
        self.reductions
            .binary_search_by(|candidate| candidate.target.cmp(target))
            .ok()
            .map(|index| &self.reductions[index])
            .ok_or_else(|| FourLoopThreeLoopServiceError::OutsideManifest {
                target: target.clone(),
            })
    }

    pub fn reduce_target(
        &self,
        target: &FourLoopThreeLoopLocalTarget,
    ) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopThreeLoopServiceError> {
        Ok(self.reduction(target)?.ordinary.clone())
    }

    /// Regenerate and validate all 1,800 native rows attached to the exact
    /// non-tadpole target manifest.
    pub fn validate_native_target_identities(&self) -> Result<(), FourLoopThreeLoopServiceError> {
        validate_native_target_identities(&self.pipeline, &self.reductions)
    }

    /// Recompute every retained semantic reduction against the immutable
    /// in-memory tadpole and authenticated finite pipeline. This avoids a new
    /// pipeline build while detecting cache corruption before plan replay.
    pub fn validate_retained_reductions(&self) -> Result<(), FourLoopThreeLoopServiceError> {
        for retained in &self.reductions {
            let rebuilt = build_local_reduction(&retained.target, &self.tadpole, &self.pipeline)?;
            if &rebuilt != retained {
                return Err(FourLoopThreeLoopServiceError::ReplayMismatch {
                    stage: "retained semantic reduction",
                });
            }
        }
        Ok(())
    }

    /// Rebuild the finite pipeline and every semantic target reduction in the
    /// same coefficient context, then compare all retained certificate state.
    pub fn replay(&self) -> Result<(), FourLoopThreeLoopServiceError> {
        let targets = self
            .reductions
            .iter()
            .map(|reduction| reduction.target.clone())
            .collect::<Vec<_>>();
        let rebuilt = Self::build_impl(self.coefficient_context.clone(), targets, self.config)?;
        if rebuilt.family_fingerprint != self.family_fingerprint
            || rebuilt.pipeline_config != self.pipeline_config
            || rebuilt.pipeline_stats != self.pipeline_stats
            || rebuilt.manifest_checksum != self.manifest_checksum
            || rebuilt.targets != self.targets
            || rebuilt.reductions != self.reductions
            || rebuilt.stats != self.stats
            || rebuilt.checksum != self.checksum
        {
            return Err(FourLoopThreeLoopServiceError::ReplayMismatch {
                stage: "complete target-service rebuild",
            });
        }
        Ok(())
    }

    /// Replay one altered record against its immutable manifest position and
    /// the retained native services.  Binding the position prevents swapping
    /// two targets which happen to have equal semantic reductions.
    #[doc(hidden)]
    pub fn replay_target_candidate(
        &self,
        target_index: usize,
        candidate: &FourLoopThreeLoopLocalReduction,
    ) -> Result<(), FourLoopThreeLoopServiceError> {
        let retained = self.reductions.get(target_index).ok_or(
            FourLoopThreeLoopServiceError::ReplayMismatch {
                stage: "candidate target index",
            },
        )?;
        if candidate.target != retained.target {
            return Err(FourLoopThreeLoopServiceError::ReplayMismatch {
                stage: "candidate target identity",
            });
        }
        let expected = build_local_reduction(&candidate.target, &self.tadpole, &self.pipeline)?;
        if &expected != candidate {
            return Err(FourLoopThreeLoopServiceError::ReplayMismatch {
                stage: "candidate semantic reduction",
            });
        }
        Ok(())
    }
}

fn preflight_config(
    config: FourLoopThreeLoopServiceConfig,
) -> Result<(), FourLoopThreeLoopServiceError> {
    for (resource, actual, minimum) in [
        (
            "local target manifest",
            config.max_targets,
            FOUR_LOOP_THREE_LOOP_SERVICE_TARGETS,
        ),
        (
            "T1 local targets",
            config.max_t1_targets,
            FOUR_LOOP_THREE_LOOP_SERVICE_T1_TARGETS,
        ),
        (
            "B4-owner local targets",
            config.max_b4_targets,
            FOUR_LOOP_THREE_LOOP_SERVICE_B4_TARGETS,
        ),
        (
            "F5-owner local targets",
            config.max_f5_targets,
            FOUR_LOOP_THREE_LOOP_SERVICE_F5_TARGETS,
        ),
        (
            "M6-owner local targets",
            config.max_m6_targets,
            FOUR_LOOP_THREE_LOOP_SERVICE_M6_TARGETS,
        ),
        (
            "semantic output terms",
            config.max_output_terms,
            FOUR_LOOP_THREE_LOOP_SERVICE_OUTPUT_TERM_BOUND,
        ),
        (
            "retained semantic-output coefficient bytes",
            config.max_retained_output_coefficient_bytes,
            FOUR_LOOP_THREE_LOOP_SERVICE_RETAINED_OUTPUT_COEFFICIENT_BYTE_BOUND,
        ),
    ] {
        if actual < minimum {
            return Err(FourLoopThreeLoopServiceError::ResourceLimit {
                resource,
                requested: minimum as u128,
                limit: actual as u128,
            });
        }
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
            return Err(FourLoopThreeLoopServiceError::ResourceLimit {
                resource,
                requested: minimum as u128,
                limit: actual as u128,
            });
        }
    }
    if config.one_loop.max_dense_term_operations < 24 {
        return Err(FourLoopThreeLoopServiceError::ResourceLimit {
            resource: "nested T1 dense term operations",
            requested: 24,
            limit: config.one_loop.max_dense_term_operations,
        });
    }
    if config.one_loop.max_coefficient_degree < 2 {
        return Err(FourLoopThreeLoopServiceError::ResourceLimit {
            resource: "nested T1 coefficient degree",
            requested: 2,
            limit: config.one_loop.max_coefficient_degree,
        });
    }
    if config.one_loop.max_coefficient_degree > SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT {
        return Err(FourLoopThreeLoopServiceError::ResourceLimit {
            resource: "configured nested T1 coefficient degree",
            requested: config.one_loop.max_coefficient_degree,
            limit: SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
        });
    }
    Ok(())
}

fn validate_target_shape(
    owner: MassiveVacuumMaster,
    actual: usize,
) -> Result<(), FourLoopThreeLoopServiceError> {
    let expected = match owner {
        MassiveVacuumMaster::T1 => 1,
        MassiveVacuumMaster::B4 | MassiveVacuumMaster::F5 | MassiveVacuumMaster::M6 => 6,
        MassiveVacuumMaster::S2 => {
            return Err(FourLoopThreeLoopServiceError::UnsupportedOwner { owner });
        }
    };
    if actual != expected {
        return Err(FourLoopThreeLoopServiceError::WrongPowerArity {
            owner,
            expected,
            actual,
        });
    }
    Ok(())
}

fn collect_and_validate_manifest(
    targets: impl IntoIterator<Item = FourLoopThreeLoopLocalTarget>,
    config: FourLoopThreeLoopServiceConfig,
) -> Result<Vec<FourLoopThreeLoopLocalTarget>, FourLoopThreeLoopServiceError> {
    let mut unique = BTreeSet::new();
    let mut owner_counts = BTreeMap::<MassiveVacuumMaster, usize>::new();
    for target in targets {
        validate_target_shape(target.owner, target.powers.len())?;
        if !unique.insert(target.clone()) {
            return Err(FourLoopThreeLoopServiceError::DuplicateTarget { target });
        }
        if unique.len() > config.max_targets {
            return Err(FourLoopThreeLoopServiceError::ResourceLimit {
                resource: "local target manifest",
                requested: unique.len() as u128,
                limit: config.max_targets as u128,
            });
        }
        let (resource, limit) = owner_target_limit(config, target.owner)?;
        let count = owner_counts.entry(target.owner).or_default();
        *count = checked_add(*count, 1, resource)?;
        if *count > limit {
            return Err(FourLoopThreeLoopServiceError::ResourceLimit {
                resource,
                requested: *count as u128,
                limit: limit as u128,
            });
        }
    }
    let targets = unique.into_iter().collect::<Vec<_>>();
    if targets.len() != FOUR_LOOP_THREE_LOOP_SERVICE_TARGETS {
        return Err(FourLoopThreeLoopServiceError::CensusMismatch {
            resource: "exact local targets",
            expected: FOUR_LOOP_THREE_LOOP_SERVICE_TARGETS,
            actual: targets.len(),
        });
    }

    let mut owners = BTreeMap::<MassiveVacuumMaster, usize>::new();
    let mut degrees = BTreeMap::<(MassiveVacuumMaster, u64, u64), usize>::new();
    for target in &targets {
        *owners.entry(target.owner).or_default() += 1;
        let (dots, numerators) = target_degrees(&target.powers);
        *degrees.entry((target.owner, dots, numerators)).or_default() += 1;
    }
    for (owner, expected, resource) in [
        (
            MassiveVacuumMaster::T1,
            FOUR_LOOP_THREE_LOOP_SERVICE_T1_TARGETS,
            "T1 target census",
        ),
        (
            MassiveVacuumMaster::B4,
            FOUR_LOOP_THREE_LOOP_SERVICE_B4_TARGETS,
            "B4-owner target census",
        ),
        (
            MassiveVacuumMaster::F5,
            FOUR_LOOP_THREE_LOOP_SERVICE_F5_TARGETS,
            "F5-owner target census",
        ),
        (
            MassiveVacuumMaster::M6,
            FOUR_LOOP_THREE_LOOP_SERVICE_M6_TARGETS,
            "M6-owner target census",
        ),
    ] {
        let actual = owners.get(&owner).copied().unwrap_or(0);
        if actual != expected {
            return Err(FourLoopThreeLoopServiceError::CensusMismatch {
                resource,
                expected,
                actual,
            });
        }
    }
    if owners.contains_key(&MassiveVacuumMaster::S2) || owners.len() != 4 {
        return Err(FourLoopThreeLoopServiceError::DegreeCensusMismatch);
    }

    let expected_degrees = FOUR_LOOP_THREE_LOOP_SERVICE_DEGREE_CENSUS
        .into_iter()
        .map(|(owner, dots, numerators, count)| ((owner, dots, numerators), count))
        .collect::<BTreeMap<_, _>>();
    if degrees != expected_degrees {
        return Err(FourLoopThreeLoopServiceError::DegreeCensusMismatch);
    }

    let checksum = target_manifest_checksum(&targets);
    if checksum != FOUR_LOOP_THREE_LOOP_SERVICE_TARGET_MANIFEST_CHECKSUM {
        return Err(FourLoopThreeLoopServiceError::ManifestChecksumMismatch {
            expected: FOUR_LOOP_THREE_LOOP_SERVICE_TARGET_MANIFEST_CHECKSUM,
            actual: checksum,
        });
    }
    Ok(targets)
}

fn owner_target_limit(
    config: FourLoopThreeLoopServiceConfig,
    owner: MassiveVacuumMaster,
) -> Result<(&'static str, usize), FourLoopThreeLoopServiceError> {
    Ok(match owner {
        MassiveVacuumMaster::T1 => ("T1 local targets", config.max_t1_targets),
        MassiveVacuumMaster::B4 => ("B4-owner local targets", config.max_b4_targets),
        MassiveVacuumMaster::F5 => ("F5-owner local targets", config.max_f5_targets),
        MassiveVacuumMaster::M6 => ("M6-owner local targets", config.max_m6_targets),
        MassiveVacuumMaster::S2 => {
            return Err(FourLoopThreeLoopServiceError::UnsupportedOwner { owner });
        }
    })
}

fn validate_coefficient_context(
    context: &CoefficientContext,
) -> Result<(), FourLoopThreeLoopServiceError> {
    if !context
        .parameter_names()
        .iter()
        .map(String::as_str)
        .eq(["d", "m2"])
        || context.parameter("d").is_none()
        || context.parameter("m2").is_none_or(|mass| mass.is_zero())
    {
        return Err(FourLoopThreeLoopServiceError::UnsupportedCoefficientContext);
    }
    Ok(())
}

fn target_degrees(powers: &[i32]) -> (u64, u64) {
    powers
        .iter()
        .fold((0_u64, 0_u64), |(mut dots, mut numerators), &power| {
            if power > 1 {
                dots += (i64::from(power) - 1) as u64;
            } else if power < 0 {
                numerators += i64::from(power).unsigned_abs();
            }
            (dots, numerators)
        })
}

fn target_manifest_checksum(targets: &[FourLoopThreeLoopLocalTarget]) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    for target in targets {
        hash_bytes(&mut hash, &[owner_tag(target.owner)]);
        hash_bytes(&mut hash, &(target.powers.len() as u64).to_le_bytes());
        for power in &target.powers {
            hash_bytes(&mut hash, &power.to_le_bytes());
        }
    }
    hash
}

const fn owner_tag(owner: MassiveVacuumMaster) -> u8 {
    match owner {
        MassiveVacuumMaster::T1 => 1,
        MassiveVacuumMaster::S2 => 2,
        MassiveVacuumMaster::B4 => 3,
        MassiveVacuumMaster::F5 => 4,
        MassiveVacuumMaster::M6 => 5,
    }
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(FNV1A64_PRIME);
    }
}

fn target_stats(
    targets: &[FourLoopThreeLoopLocalTarget],
) -> Result<FourLoopThreeLoopServiceStats, FourLoopThreeLoopServiceError> {
    let mut stats = FourLoopThreeLoopServiceStats {
        targets: targets.len(),
        ..FourLoopThreeLoopServiceStats::default()
    };
    for target in targets {
        match target.owner {
            MassiveVacuumMaster::T1 => stats.t1_targets += 1,
            MassiveVacuumMaster::B4 => stats.b4_targets += 1,
            MassiveVacuumMaster::F5 => stats.f5_targets += 1,
            MassiveVacuumMaster::M6 => stats.m6_targets += 1,
            MassiveVacuumMaster::S2 => {
                return Err(FourLoopThreeLoopServiceError::UnsupportedOwner {
                    owner: target.owner,
                });
            }
        }
    }
    Ok(stats)
}

fn build_local_reduction(
    target: &FourLoopThreeLoopLocalTarget,
    tadpole: &OneLoopTadpoleReducer,
    pipeline: &ThreeLoopReductionPipeline,
) -> Result<FourLoopThreeLoopLocalReduction, FourLoopThreeLoopServiceError> {
    let ordinary = match target.owner {
        MassiveVacuumMaster::T1 => {
            let reduction = tadpole.reduce_power(target.powers[0])?;
            if reduction.coefficient().is_zero() {
                ProductLinearCombination::new()
            } else {
                ProductLinearCombination::from_term(
                    MasterProduct::from_factor(MassiveVacuumMaster::T1),
                    reduction.coefficient().clone(),
                )
            }
        }
        MassiveVacuumMaster::B4 | MassiveVacuumMaster::F5 | MassiveVacuumMaster::M6 => {
            // Dispatch by the actual six powers through the common tetrahedron
            // service.  The owner label must never be mistaken for the sector:
            // transported lowering can pinch B4/F5/M6 targets into boundaries.
            let integral = Integral::new(target.powers.clone());
            let reduced = pipeline.reduce_integral(&integral)?;
            adapt_three_loop_semantics(target, &reduced)?
        }
        MassiveVacuumMaster::S2 => {
            return Err(FourLoopThreeLoopServiceError::UnsupportedOwner {
                owner: target.owner,
            });
        }
    };

    for product in ordinary.terms().keys() {
        let actual = product_loop_weight(product);
        let expected = target.owner.loops();
        if actual != expected as u128 {
            return Err(
                FourLoopThreeLoopServiceError::UnexpectedSemanticLoopWeight {
                    target: target.clone(),
                    product: product.clone(),
                    expected,
                    actual,
                },
            );
        }
    }
    Ok(FourLoopThreeLoopLocalReduction {
        target: target.clone(),
        ordinary,
    })
}

fn adapt_three_loop_semantics(
    target: &FourLoopThreeLoopLocalTarget,
    reduced: &LinearCombination,
) -> Result<ProductLinearCombination<MassiveVacuumMaster>, FourLoopThreeLoopServiceError> {
    let mut ordinary = ProductLinearCombination::new();
    for (terminal, coefficient) in reduced.terms() {
        let product = semantic_product(terminal)?.ok_or_else(|| {
            FourLoopThreeLoopServiceError::UnexpectedTerminal {
                target: target.clone(),
                terminal: terminal.clone(),
            }
        })?;
        ordinary.add_term(product, coefficient.clone());
    }
    Ok(ordinary)
}

fn semantic_product(
    terminal: &Integral,
) -> Result<Option<MasterProduct<MassiveVacuumMaster>>, FourLoopThreeLoopServiceError> {
    Ok(match terminal.powers() {
        [1, 1, 1, 0, 0, 0] => Some(MasterProduct::try_from_multiplicities([(
            MassiveVacuumMaster::T1,
            3,
        )])?),
        [1, 1, 1, 1, 0, 0] => Some(MasterProduct::try_from_factors([
            MassiveVacuumMaster::T1,
            MassiveVacuumMaster::S2,
        ])?),
        [1, 1, 0, 1, 0, 1] => Some(MasterProduct::from_factor(MassiveVacuumMaster::B4)),
        [1, 1, 1, 1, 1, 0] => Some(MasterProduct::from_factor(MassiveVacuumMaster::F5)),
        [1, 1, 1, 1, 1, 1] => Some(MasterProduct::from_factor(MassiveVacuumMaster::M6)),
        _ => None,
    })
}

fn product_loop_weight(product: &MasterProduct<MassiveVacuumMaster>) -> u128 {
    product
        .factors()
        .iter()
        .map(|(master, multiplicity)| (master.loops() as u128) * u128::from(*multiplicity))
        .sum()
}

fn validate_native_target_identities(
    pipeline: &ThreeLoopReductionPipeline,
    reductions: &[FourLoopThreeLoopLocalReduction],
) -> Result<(), FourLoopThreeLoopServiceError> {
    let mut identities = Vec::new();
    identities
        .try_reserve_exact(FOUR_LOOP_THREE_LOOP_SERVICE_NATIVE_IDENTITIES)
        .map_err(|_| FourLoopThreeLoopServiceError::AllocationFailed {
            resource: "native target identities",
            requested: FOUR_LOOP_THREE_LOOP_SERVICE_NATIVE_IDENTITIES,
        })?;
    let generator = IbpGenerator::new(pipeline.family());
    for reduction in reductions {
        if reduction.target.owner == MassiveVacuumMaster::T1 {
            continue;
        }
        identities
            .extend(generator.try_generate_raw(&Integral::new(reduction.target.powers.clone()))?);
    }
    if identities.len() != FOUR_LOOP_THREE_LOOP_SERVICE_NATIVE_IDENTITIES {
        return Err(FourLoopThreeLoopServiceError::CensusMismatch {
            resource: "native target identities",
            expected: FOUR_LOOP_THREE_LOOP_SERVICE_NATIVE_IDENTITIES,
            actual: identities.len(),
        });
    }
    pipeline.validate_identities(&identities)?;
    Ok(())
}

fn retained_output_coefficient_bytes(
    combination: &ProductLinearCombination<MassiveVacuumMaster>,
) -> Result<usize, FourLoopThreeLoopServiceError> {
    combination
        .terms()
        .values()
        .try_fold(0_usize, |total, coefficient| {
            total.checked_add(coefficient.to_string().len()).ok_or(
                FourLoopThreeLoopServiceError::ArithmeticOverflow {
                    resource: "retained output coefficient bytes",
                },
            )
        })
}

fn service_checksum(
    config: FourLoopThreeLoopServiceConfig,
    context: &CoefficientContext,
    family_fingerprint: &str,
    pipeline_config: ThreeLoopReductionConfig,
    pipeline_stats: &ReductionStats,
    candidates: &[Integral; 5],
    manifest_checksum: u64,
    reductions: &[FourLoopThreeLoopLocalReduction],
    stats: FourLoopThreeLoopServiceStats,
) -> u64 {
    let mut hash = FNV1A64_OFFSET;
    hash_length_prefixed(&mut hash, FourLoopThreeLoopService::SCHEMA.as_bytes());
    for name in context.parameter_names() {
        hash_length_prefixed(&mut hash, name.as_bytes());
    }
    hash_length_prefixed(&mut hash, family_fingerprint.as_bytes());
    hash_bytes(&mut hash, &manifest_checksum.to_le_bytes());
    for value in [
        u64::from(pipeline_config.max_dots),
        u64::from(pipeline_config.max_numerator_degree),
        pipeline_config.max_seed_candidates as u64,
        pipeline_config.max_tadpole_steps as u64,
        u64::from(pipeline_config.max_two_loop_dots),
        pipeline_config.max_two_loop_seed_candidates as u64,
        pipeline_config.max_two_loop_boundary_terms as u64,
    ] {
        hash_bytes(&mut hash, &value.to_le_bytes());
    }
    hash_bytes(&mut hash, &(candidates.len() as u64).to_le_bytes());
    for candidate in candidates {
        hash_bytes(&mut hash, &(candidate.powers().len() as u64).to_le_bytes());
        for power in candidate.powers() {
            hash_bytes(&mut hash, &power.to_le_bytes());
        }
    }
    for value in [
        config.max_targets,
        config.max_t1_targets,
        config.max_b4_targets,
        config.max_f5_targets,
        config.max_m6_targets,
        config.max_output_terms,
        config.max_retained_output_coefficient_bytes,
        config.one_loop.max_recurrence_steps,
        config.one_loop.max_coefficient_operations,
    ] {
        hash_bytes(&mut hash, &(value as u64).to_le_bytes());
    }
    for value in [
        config.one_loop.max_dense_term_operations,
        config.one_loop.max_coefficient_degree,
    ] {
        hash_bytes(&mut hash, &value.to_le_bytes());
    }
    for value in [
        pipeline_stats.input_equations,
        pipeline_stats.rules,
        pipeline_stats.dependent_equations,
        pipeline_stats.maximum_terms,
    ] {
        hash_bytes(&mut hash, &(value as u64).to_le_bytes());
    }
    for reduction in reductions {
        hash_bytes(&mut hash, &[owner_tag(reduction.target.owner)]);
        hash_bytes(
            &mut hash,
            &(reduction.target.powers.len() as u64).to_le_bytes(),
        );
        for power in &reduction.target.powers {
            hash_bytes(&mut hash, &power.to_le_bytes());
        }
        hash_bytes(&mut hash, &(reduction.ordinary.len() as u64).to_le_bytes());
        for (product, coefficient) in reduction.ordinary.terms() {
            hash_bytes(&mut hash, &(product.factors().len() as u64).to_le_bytes());
            for (master, multiplicity) in product.factors() {
                hash_bytes(&mut hash, &[owner_tag(*master)]);
                hash_bytes(&mut hash, &multiplicity.to_le_bytes());
            }
            hash_length_prefixed(&mut hash, coefficient.to_string().as_bytes());
        }
    }
    for value in [
        stats.targets,
        stats.t1_targets,
        stats.b4_targets,
        stats.f5_targets,
        stats.m6_targets,
        stats.native_target_identities,
        stats.output_terms,
        stats.retained_output_coefficient_bytes,
    ] {
        hash_bytes(&mut hash, &(value as u64).to_le_bytes());
    }
    hash
}

fn hash_length_prefixed(hash: &mut u64, bytes: &[u8]) {
    hash_bytes(hash, &(bytes.len() as u64).to_le_bytes());
    hash_bytes(hash, bytes);
}

fn checked_add(
    left: usize,
    right: usize,
    resource: &'static str,
) -> Result<usize, FourLoopThreeLoopServiceError> {
    left.checked_add(right)
        .ok_or(FourLoopThreeLoopServiceError::ArithmeticOverflow { resource })
}
