//! RustRed: a pure-Rust, Symbolica-backed port of LiteRed-style parametric IBP
//! derivation and reduction.
//!
//! The generic production path is loop-count and topology independent:
//! [`IntegralFamily`] authenticates a complete affine scalar-product basis and
//! [`ParametricIbpGenerator`] derives reusable ordinary and Lorentz-invariance
//! identities over the exact field `K(n)`. [`IndexShiftOperatorExpression`]
//! provides exact ordered `A`/`B` action and relation round trips; it is an
//! intermediate whose coefficients may still contain `n`, not LiteRed's
//! completed `ToAB` polynomial form. Loop/topology-authored fixtures and
//! finite reducers live in the separate, publish-disabled
//! `rustred-legacy-oracles` package. They are validation evidence rather than
//! sources of generic parametric identities or future discovered rules.

mod canonical_parametric_locus_table;
mod coverage_decision_dag;
mod direct_bad_formula;
mod direct_bad_formula_arbitrary;
mod exact_identity;
mod parametric_sector_formula_affine_terminal;
mod parametric_sector_formula_ir;
mod parametric_sector_formula_residual;
#[cfg(test)]
mod parametric_sector_k21_test_support;
mod parametric_sector_mtbdd;
#[cfg(test)]
mod parametric_sector_mtbdd_certificate;
mod parametric_sector_normalized_source;
#[cfg(test)]
mod parametric_sector_one_pass_tests;
mod solver;
pub(crate) mod symbolica_coefficient_matrix;

pub mod adaptive_rules;
pub mod affine_locus_bound_relation;
pub mod affine_parametric_ordering;
pub mod affine_prepare_point_schedule;
pub mod affine_prepare_points;
pub mod automatic_isps;
pub mod base_specialization;
pub mod campaign;
pub mod certified_rewrite;
pub mod certified_rule_provider;
pub mod certified_symmetry_provider;
pub mod coefficient;
pub mod conditional_reelimination;
pub mod conditional_rules;
pub mod coordinate_equality_loci;
pub mod cylindrical_ordering;
pub mod cylindrical_prepare_point_schedule;
pub mod cylindrical_prepare_points;
pub mod exact;
pub mod exact_sparse_elimination;
pub mod family_sector_inventory;
pub mod feynman_polynomials;
pub(crate) mod generated_affine_initial_global_affine_terminal;
pub(crate) mod generated_affine_parametric_ordering;
pub(crate) mod generated_affine_prepare_point_schedule;
pub(crate) mod generated_affine_residual_boolean_cover;
mod generated_affine_residual_case_bound_relation;
mod generated_affine_residual_case_bound_unit_equality_refinement;
mod generated_affine_residual_case_completed_bound_row;
pub(crate) mod generated_affine_residual_case_inventory;
pub(crate) mod generated_affine_residual_case_mapped_nonzero;
mod generated_affine_residual_case_pivot_target_matching;
mod generated_affine_residual_case_premises;
mod generated_affine_residual_case_reelimination;
pub(crate) mod generated_affine_residual_case_unit_equality_refinement;
mod generated_affine_residual_group_exact_publication;
mod generated_affine_residual_group_exact_publication_handoff;
#[cfg(test)]
mod generated_affine_residual_group_exact_publication_tests;
mod generated_affine_residual_group_exact_when_bad_conditions;
mod generated_affine_residual_group_exact_when_bad_materialization;
mod generated_affine_residual_group_exact_when_bad_partition;
mod generated_affine_residual_group_ready_publication;
#[cfg(test)]
mod generated_affine_residual_group_ready_publication_tests;
pub(crate) mod generated_affine_residual_source_authority;
pub mod generated_cylindrical_candidate_authority;
pub mod generated_cylindrical_family_source_set;
pub mod generated_cylindrical_persistent_elimination;
pub mod generated_cylindrical_residual_start;
pub mod generated_cylindrical_row_system;
pub mod generated_cylindrical_sector_coverage;
pub mod generated_cylindrical_sector_provider;
pub mod generated_cylindrical_sector_root_start;
pub mod generated_cylindrical_when_bad;
pub mod generated_family_depth_growth;
pub mod generated_family_fixed_point;
pub mod generated_family_fixed_point_provider;
pub mod generated_family_rule_provider;
pub mod generated_family_rule_system;
pub(crate) mod generated_provider_stack;
pub mod generated_residual_affine_branch_bound_relation;
pub mod generated_residual_affine_branch_reelimination;
pub mod generated_residual_affine_case_inventory;
pub(crate) mod generated_residual_affine_condition_accumulator;
#[cfg(test)]
mod generated_residual_affine_condition_accumulator_tests;
pub(crate) mod generated_residual_affine_group_effective_coverage;
pub mod generated_residual_affine_pivot_target_matching;
pub mod generated_residual_affine_when_bad;
pub(crate) mod generated_residual_affine_when_bad_compilation;
#[cfg(test)]
mod generated_residual_affine_when_bad_compilation_tests;
pub(crate) mod generated_residual_affine_when_bad_descent;
pub(crate) mod generated_residual_affine_when_bad_pullback_gate;
pub(crate) mod generated_sector_affine_effective_coverage;
#[cfg(test)]
mod generated_sector_affine_effective_coverage_tests;
pub(crate) mod generated_sector_affine_effective_residual_queue;
#[cfg(test)]
mod generated_sector_affine_effective_residual_queue_tests;
pub(crate) mod generated_sector_affine_provider;
pub mod generated_sector_conditional_provider;
pub mod generated_sector_discovery;
pub mod generated_sector_live_leaf_queue;
pub mod generated_symbolic_row_span;
pub mod generated_when_bad;
pub mod generic_family;
pub mod generic_tensor_family;
pub mod generic_tensor_polynomial;
pub mod generic_tensor_projector;
pub mod guards;
#[cfg(feature = "legacy-oracle-support")]
#[doc(hidden)]
pub mod legacy_oracle_support;
pub mod master_policy;
pub mod master_product;
pub mod parallel_execution;
pub mod parametric_coefficient;
pub mod parametric_elimination;
pub mod parametric_ibp;
pub mod parametric_relation;
pub mod parametric_rules;
pub mod parametric_sector_coverage;
pub mod parametric_sector_provider;
pub mod persistent_parametric_elimination;
pub mod product_locus_boolean_cover;
pub mod reduction_engine;
pub mod residual_affine_atom_rows;
pub mod residual_affine_branch_guard_composition;
pub mod residual_affine_branch_system;
pub(crate) mod residual_affine_integer_lattice_kernel;
pub mod residual_affine_integer_system;
pub mod residual_unit_affine_index_map;
pub mod sectors;
pub mod shift_operators;
pub mod symbolic_sector_cases;
pub mod symbolic_symmetry_transport;
pub mod symbolica_affine_denominator;
pub mod symbolica_integral_input;
pub mod symbolica_runtime;
pub mod symbolica_target_numerator;
pub mod symbolica_tensor_numerator;
pub mod symmetry;
pub mod symmetry_discovery;
pub mod tensor;
pub mod tensor_reduction_engine;
pub mod when_bad;
pub mod zero_sector_provider;
pub mod zero_sectors;

pub use adaptive_rules::{
    ADAPTIVE_PARAMETRIC_RULE_SEARCH_V1_SCHEMA, AdaptiveParametricRuleProvider,
    AdaptiveRuleSearchError, AdaptiveRuleSearchLimits, AdaptiveRuleSearchStats,
};
pub use affine_locus_bound_relation::{
    AFFINE_LOCUS_BOUND_PARAMETRIC_RELATION_V1_SCHEMA, AffineLocusBaseAssumption,
    AffineLocusBoundParametricRelation, AffineLocusBoundRelationCompilation,
    AffineLocusBoundRelationCompiler, AffineLocusBoundRelationError,
    AffineLocusBoundRelationLimits, AffineLocusBoundRelationStats,
    AffineLocusConcreteSpecializationLimits, AffineLocusUnavailableReason,
    AffineLocusUnavailableRowCertificate,
};
pub use affine_parametric_ordering::{
    AFFINE_START_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA,
    AFFINE_START_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA, AffineParametricOrderingError,
    AffineParametricOrderingLimits, AffineParametricOrderingStats, AffineStartGeometryRef,
    AffineStartIntegralComplexityKey, AffineStartParametricEliminationOrdering,
    AffineStartReplayAuthority, AffineStartSourceCertificate, AffineStartSourceKind,
    RUSTRED_AFFINE_START_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
};
pub use affine_prepare_point_schedule::{
    AFFINE_PREPARE_POINT_SCHEDULE_V1_SCHEMA, AffinePreparePointScheduleCertificate,
    AffinePreparePointScheduleError, AffinePreparePointScheduleLimits,
    AffinePreparePointScheduleStats,
};
pub use affine_prepare_points::{
    AFFINE_PREPARE_POINT_LAYER_V1_SCHEMA, AffinePreparePointError, AffinePreparePointLayer,
    AffinePreparePointLimits, AffinePreparePointStats,
};
pub use automatic_isps::{
    AUTOMATIC_ISP_COMPLETION_V1_SCHEMA, AUTOMATIC_ISP_COMPLETION_V2_SCHEMA, AutomaticIspCompletion,
    AutomaticIspCompletionError, AutomaticIspCompletionLimits, AutomaticIspCompletionStats,
};
pub use base_specialization::{
    BaseCoefficientProvenance, BaseKinematicSpecialization, BaseParameterImage,
    BaseSpecializationError, BaseSpecializationGuard, BaseSpecializationGuardProvenance,
    BaseSpecializationLimits, FamilyDomainConditionSource, FamilyDomainEvaluation,
    FamilyDomainEvaluationStatus, GuardedBaseCoefficient, InapplicableFamilyDomainCondition,
    SpecializedBasePolynomial,
};
pub use certified_rewrite::{
    CERTIFIED_CONCRETE_REWRITE_V1_SCHEMA, CERTIFIED_CONCRETE_REWRITE_V2_SCHEMA,
    CERTIFIED_ZERO_REDUCTION_V1_SCHEMA, CertifiedConcreteRewrite, CertifiedConcreteRewriteProof,
    CertifiedRewriteDomainCondition, CertifiedRewriteDomainOrigin, CertifiedRewriteError,
    CertifiedRewriteLimits, CertifiedZeroReduction, CertifiedZeroReductionProof,
    ConcreteQuotientSourceRowProof, QuotientTermWitness,
};
pub use certified_rule_provider::{
    CERTIFIED_FAMILY_RULE_PROVIDER_V1_SCHEMA, CERTIFIED_FAMILY_RULE_PROVIDER_V2_SCHEMA,
    CERTIFIED_FAMILY_RULE_PROVIDER_V3_SCHEMA, CertifiedFamilyRuleProvider,
    CertifiedFamilyRuleProviderError, CertifiedFamilyRuleProviderLimits,
};
pub use certified_symmetry_provider::{
    CERTIFIED_SYMMETRY_CANONICALIZING_RULE_PROVIDER_V1_SCHEMA,
    CertifiedSymmetryCanonicalizingRuleProvider, CertifiedSymmetryCanonicalizingRuleProviderError,
    CertifiedSymmetryCanonicalizingRuleProviderLimits,
    CertifiedSymmetryCanonicalizingRuleProviderStats,
};
pub use coefficient::{
    Coefficient, CoefficientContext, CoefficientContextError, CoefficientPolynomialPart,
    CoefficientProjectionError, ExactAlgebraError, ExactAlgebraLimits, ExactAlgebraOperation,
    SYMBOLICA_COEFFICIENT_EXPONENT_LIMIT,
};
pub use conditional_reelimination::{
    ConditionalCenteredPivotLocus, GENERATED_PARTIAL_REELIMINATION_V1_SCHEMA,
    GENERATED_PARTIAL_REELIMINATION_V2_SCHEMA, GeneratedPartialBaseAssumptionWitness,
    GeneratedPartialReeliminationCertificate, GeneratedPartialReeliminationCompilation,
    GeneratedPartialReeliminationCompiler, GeneratedPartialReeliminationEmptySystem,
    GeneratedPartialReeliminationError, GeneratedPartialReeliminationLimits,
    GeneratedPartialReeliminationStats, GeneratedPartialSourceAuthentication,
    GeneratedPartialSourceRowOutcome, GeneratedPartialSourceRowWitness,
};
pub use conditional_rules::{
    CONDITIONAL_PARAMETRIC_RULE_V1_SCHEMA, ConditionalConcreteReduction, ConditionalParametricRule,
    ConditionalParametricRuleApplication, ConditionalParametricRuleError,
    ConditionalParametricRuleInapplicability, ConditionalParametricRuleLimits,
};
pub use coordinate_equality_loci::{
    COORDINATE_EQUALITY_LOCUS_V1_SCHEMA, CoordinateAssignmentWitness,
    CoordinateEqualityEmptyReason, CoordinateEqualityLeafStatus,
    CoordinateEqualityLocusCertificate, CoordinateEqualityLocusError,
    CoordinateEqualityLocusExtractor, CoordinateEqualityLocusLimits, CoordinateEqualityLocusStats,
    CoordinateLocusPredicateWitness, UnresolvedCoordinatePredicate,
};
pub use cylindrical_ordering::{
    CYLINDRICAL_INTEGRAL_COMPLEXITY_KEY_V1_SCHEMA,
    CYLINDRICAL_PARAMETRIC_ELIMINATION_ORDERING_V1_SCHEMA, CylindricalIntegralComplexityKey,
    CylindricalOrderingError, CylindricalOrderingLimits, CylindricalParametricEliminationOrdering,
    RUSTRED_CYLINDRICAL_UNSHIFTED_ORDER_V1_KEY_SCHEMA,
};
pub use cylindrical_prepare_point_schedule::{
    CYLINDRICAL_PREPARE_POINT_SCHEDULE_V1_SCHEMA, CylindricalPreparePointScheduleCertificate,
    CylindricalPreparePointScheduleError, CylindricalPreparePointScheduleLimits,
    CylindricalPreparePointScheduleStats,
};
pub use cylindrical_prepare_points::{
    CYLINDRICAL_PREPARE_POINT_LAYER_V1_SCHEMA, CylindricalPreparePointError,
    CylindricalPreparePointLayer, CylindricalPreparePointLimits, CylindricalPreparePointStats,
};
pub use exact::{ExactRational, ExactRationalError};
pub use exact_sparse_elimination::{
    ExactSparseCoefficientLocation, ExactSparseDerivationReduction, ExactSparseDerivationTrace,
    ExactSparseElimination, ExactSparseEliminationConfig, ExactSparseEliminationError,
    ExactSparseEliminationStats, ExactSparsePivotRule, ExactSparseRow,
};
pub use family_sector_inventory::{
    FAMILY_SECTOR_INVENTORY_V1_SCHEMA, FORMAL_GENERIC_POWER_SHIFT_POLICY_V1_ID,
    FamilySectorInventoryCertificate, FamilySectorInventoryCompiler, FamilySectorInventoryEntry,
    FamilySectorInventoryError, FamilySectorInventoryLimits, FamilySectorInventoryStats,
    FamilySectorInventoryStatus, UnresolvedSectorSolveOrderEntry,
};
pub use feynman_polynomials::{
    FeynmanPolynomial, FeynmanPolynomialContext, FeynmanPolynomialError, FeynmanPolynomialLimits,
    RawFeynmanPolynomial, SymanzikPolynomials,
};
pub use generated_cylindrical_candidate_authority::{
    GENERATED_CYLINDRICAL_CANDIDATE_AUTHORITY_V1_SCHEMA, GeneratedCylindricalCandidateAuthority,
    GeneratedCylindricalCandidateAuthorityError, GeneratedCylindricalCandidateAuthorityLimits,
    GeneratedCylindricalCandidateAuthorityStats, GeneratedCylindricalCandidateOrderingAuthority,
    GeneratedCylindricalCenteredAssignment, GeneratedCylindricalGlobalCandidateAuthority,
    GeneratedCylindricalLocusBoundCandidateAuthority,
};
pub use generated_cylindrical_family_source_set::{
    GENERATED_CYLINDRICAL_FAMILY_SOURCE_SET_V1_SCHEMA,
    GeneratedCylindricalFamilyInventoryInterruption, GeneratedCylindricalFamilySourceBudget,
    GeneratedCylindricalFamilySourceSetCertificate, GeneratedCylindricalFamilySourceSetCompiler,
    GeneratedCylindricalFamilySourceSetError, GeneratedCylindricalFamilySourceSetLimits,
    GeneratedCylindricalFamilySourceSetStats,
};
/// Persistent generated-row elimination authority.
///
/// The exported V1 schema string is identification-only: this crate does not
/// read or replay V1/V2 payloads. Current compilation and replay require V3.
pub use generated_cylindrical_persistent_elimination::{
    GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V1_SCHEMA,
    GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V2_SCHEMA,
    GENERATED_CYLINDRICAL_PERSISTENT_ELIMINATION_V3_SCHEMA,
    GeneratedCylindricalPersistentBaseAssumptionWitness,
    GeneratedCylindricalPersistentEliminationBatch,
    GeneratedCylindricalPersistentEliminationCertificate,
    GeneratedCylindricalPersistentEliminationError, GeneratedCylindricalPersistentEliminationEvent,
    GeneratedCylindricalPersistentEliminationLimits,
    GeneratedCylindricalPersistentEliminationOutcome,
    GeneratedCylindricalPersistentEliminationRowOutcome,
    GeneratedCylindricalPersistentEliminationStats, GeneratedCylindricalPersistentGuardedPivot,
    GeneratedCylindricalPersistentPivotBaseAssumptions,
    GeneratedCylindricalPersistentResolvedBaseAssumption,
};
pub use generated_cylindrical_residual_start::{
    GENERATED_CYLINDRICAL_RESIDUAL_START_V1_SCHEMA, GeneratedCylindricalResidualStartCertificate,
    GeneratedCylindricalResidualStartError, GeneratedCylindricalResidualStartLimits,
    GeneratedCylindricalResidualStartStats, GeneratedCylindricalStartCompleteness,
};
pub use generated_cylindrical_row_system::{
    GENERATED_CYLINDRICAL_ROW_SYSTEM_V1_SCHEMA, GENERATED_CYLINDRICAL_ROW_SYSTEM_V2_SCHEMA,
    GeneratedCylindricalRowSystemCertificate, GeneratedCylindricalRowSystemError,
    GeneratedCylindricalRowSystemLimits, GeneratedCylindricalRowSystemStartCertificate,
    GeneratedCylindricalRowSystemStats, GeneratedCylindricalSourceRowOutcome,
    GeneratedCylindricalSourceRowWitness,
};
pub use generated_cylindrical_sector_coverage::{
    GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V1_SCHEMA,
    GENERATED_CYLINDRICAL_SECTOR_COVERAGE_V2_SCHEMA, GeneratedCylindricalSectorCoverageAttempt,
    GeneratedCylindricalSectorCoverageBatchProvenance,
    GeneratedCylindricalSectorCoverageCertificate, GeneratedCylindricalSectorCoverageCompiler,
    GeneratedCylindricalSectorCoverageError, GeneratedCylindricalSectorCoverageLimits,
    GeneratedCylindricalSectorCoverageStats, GeneratedCylindricalSectorLeafDisposition,
};
pub use generated_cylindrical_sector_provider::{
    GENERATED_CYLINDRICAL_SECTOR_RULE_PROVIDER_V1_SCHEMA, GeneratedCylindricalSectorRuleProvider,
    GeneratedCylindricalSectorRuleProviderBuildStats, GeneratedCylindricalSectorRuleProviderError,
    GeneratedCylindricalSectorRuleProviderLimits, GeneratedCylindricalSectorRuleProviderStats,
};
pub use generated_cylindrical_sector_root_start::{
    GENERATED_CYLINDRICAL_SECTOR_ROOT_START_V1_SCHEMA,
    GeneratedCylindricalSectorRootStartCertificate, GeneratedCylindricalSectorRootStartError,
    GeneratedCylindricalSectorRootStartLimits, GeneratedCylindricalSectorRootStartStats,
};
pub use generated_cylindrical_when_bad::{
    GENERATED_CYLINDRICAL_WHEN_BAD_V1_SCHEMA, GeneratedCylindricalWhenBadCertificate,
    GeneratedCylindricalWhenBadCompilation, GeneratedCylindricalWhenBadCompiler,
    GeneratedCylindricalWhenBadUnsupported,
};
pub use generated_family_depth_growth::{
    GENERATED_FAMILY_DEPTH_GROWTH_PROVIDER_V1_SCHEMA, GENERATED_FAMILY_DEPTH_GROWTH_V1_SCHEMA,
    GeneratedFamilyDepthGrowthAttemptOutcome, GeneratedFamilyDepthGrowthCertificate,
    GeneratedFamilyDepthGrowthCompiler, GeneratedFamilyDepthGrowthConditionalError,
    GeneratedFamilyDepthGrowthConfig, GeneratedFamilyDepthGrowthError,
    GeneratedFamilyDepthGrowthFinalStatus, GeneratedFamilyDepthGrowthLimits,
    GeneratedFamilyDepthGrowthMasterError, GeneratedFamilyDepthGrowthMaterialRef,
    GeneratedFamilyDepthGrowthProvider, GeneratedFamilyDepthGrowthProviderError,
    GeneratedFamilyDepthGrowthRound, GeneratedFamilyDepthGrowthSectorAttempt,
    GeneratedFamilyDepthGrowthSectorStatus, GeneratedFamilyDepthGrowthSelectionPolicy,
    GeneratedFamilyDepthGrowthStackError, GeneratedFamilyDepthGrowthStage,
    GeneratedFamilyDepthGrowthStats, GeneratedResidualLeafIdentity, GeneratedResidualLeafKind,
    GeneratedResidualMeasure, GeneratedSectorResidualSummary,
};
pub use generated_family_fixed_point::{
    GENERATED_FAMILY_FIXED_POINT_PROVIDER_V1_SCHEMA, GENERATED_FAMILY_FIXED_POINT_V1_SCHEMA,
    GeneratedAcceptedCandidateOrigin, GeneratedAcceptedCandidateReference,
    GeneratedAnchorWitnessSearchExhaustionReason, GeneratedFamilyFixedPointAttemptOutcome,
    GeneratedFamilyFixedPointBasePreparation, GeneratedFamilyFixedPointBasePreparationOutcome,
    GeneratedFamilyFixedPointCertificate, GeneratedFamilyFixedPointCompiler,
    GeneratedFamilyFixedPointConfig, GeneratedFamilyFixedPointError,
    GeneratedFamilyFixedPointFinalStatus, GeneratedFamilyFixedPointInterruption,
    GeneratedFamilyFixedPointLimits, GeneratedFamilyFixedPointRound,
    GeneratedFamilyFixedPointSectorAttempt, GeneratedFamilyFixedPointSectorStatus,
    GeneratedFamilyFixedPointSelectionPolicy, GeneratedFamilyFixedPointStage,
    GeneratedFamilyFixedPointStats, GeneratedFixedPointMaterialLocator,
    GeneratedFixedPointMaterialRef, GeneratedFixedPointResidualLeafReference,
    GeneratedFixedPointResidualSummary, GeneratedResidualAnchorOrigin,
    GeneratedResidualAnchorSearch, GeneratedResidualCandidateLocator,
    GeneratedResidualCandidateOutcome, GeneratedResidualCandidateVisit,
};
pub use generated_family_fixed_point_provider::{
    GeneratedFamilyFixedPointConditionalProviderError,
    GeneratedFamilyFixedPointMasterProviderError, GeneratedFamilyFixedPointProvider,
    GeneratedFamilyFixedPointProviderBuildStats, GeneratedFamilyFixedPointProviderError,
    GeneratedFamilyFixedPointProviderInterruptionLocation, GeneratedFamilyFixedPointProviderLimits,
    GeneratedFamilyFixedPointProviderStackError, GeneratedFamilyFixedPointSymmetryProviderError,
};
pub use generated_family_rule_provider::{
    GENERATED_FAMILY_RULE_SYSTEM_PROVIDER_V2_SCHEMA, GeneratedFamilyConditionalProviderError,
    GeneratedFamilyMasterProviderError, GeneratedFamilyRuleSystemProvider,
    GeneratedFamilyRuleSystemProviderBuildStats, GeneratedFamilyRuleSystemProviderError,
    GeneratedFamilyRuleSystemProviderLimits, GeneratedFamilyRuleSystemProviderStackError,
    GeneratedFamilySymmetryProviderError,
};
pub use generated_family_rule_system::{
    GENERATED_FAMILY_RULE_SYSTEM_V1_SCHEMA, GeneratedFamilyPipelineStage,
    GeneratedFamilyRuleSystemCertificate, GeneratedFamilyRuleSystemCompiler,
    GeneratedFamilyRuleSystemConfig, GeneratedFamilyRuleSystemError,
    GeneratedFamilyRuleSystemLimits, GeneratedFamilyRuleSystemStats,
    GeneratedFamilyRuleSystemStrategy, GeneratedFamilySectorFailure, GeneratedFamilySectorResource,
    GeneratedFamilySectorStatus, GeneratedFamilySectorTranscript,
};
pub use generated_residual_affine_branch_bound_relation::{
    GENERATED_RESIDUAL_AFFINE_BRANCH_BOUND_RELATION_V1_SCHEMA,
    GeneratedResidualAffineBranchBaseAssumption, GeneratedResidualAffineBranchBoundConditionClass,
    GeneratedResidualAffineBranchBoundConditionSource,
    GeneratedResidualAffineBranchBoundConditionWitness,
    GeneratedResidualAffineBranchBoundParametricRelation,
    GeneratedResidualAffineBranchBoundRelationCompilation,
    GeneratedResidualAffineBranchBoundRelationCompiler,
    GeneratedResidualAffineBranchBoundRelationError,
    GeneratedResidualAffineBranchBoundRelationLimits,
    GeneratedResidualAffineBranchBoundRelationStats,
    GeneratedResidualAffineBranchConcreteSpecializationLimits,
    GeneratedResidualAffineBranchEmptyCertificate, GeneratedResidualAffineBranchEmptyReason,
    GeneratedResidualAffineBranchUnavailableReason,
    GeneratedResidualAffineBranchUnavailableRowCertificate,
};
pub use generated_residual_affine_branch_reelimination::{
    GENERATED_RESIDUAL_AFFINE_BRANCH_REELIMINATION_V1_SCHEMA,
    GeneratedResidualAffineBranchConcreteReplayLimits,
    GeneratedResidualAffineBranchConcreteReplayStats,
    GeneratedResidualAffineBranchReeliminationCertificate,
    GeneratedResidualAffineBranchReeliminationCompilation,
    GeneratedResidualAffineBranchReeliminationCompiler,
    GeneratedResidualAffineBranchReeliminationEmptyBranch,
    GeneratedResidualAffineBranchReeliminationError,
    GeneratedResidualAffineBranchReeliminationLimits,
    GeneratedResidualAffineBranchReeliminationNoAvailableRows,
    GeneratedResidualAffineBranchReeliminationRowOutcome,
    GeneratedResidualAffineBranchReeliminationRowWitness,
    GeneratedResidualAffineBranchReeliminationStats,
};
pub use generated_residual_affine_case_inventory::{
    GENERATED_RESIDUAL_AFFINE_CASE_INVENTORY_V1_SCHEMA,
    GeneratedResidualAffineCaseInventoryCertificate, GeneratedResidualAffineCaseInventoryCompiler,
    GeneratedResidualAffineCaseInventoryError, GeneratedResidualAffineCaseInventoryLimits,
    GeneratedResidualAffineCaseInventoryStats, GeneratedResidualAffineCaseLocator,
    GeneratedResidualAffineContiguousCaseGroup, GeneratedResidualAffineInventoryCase,
    GeneratedResidualAffineInventoryTerminal, GeneratedResidualAffineInventoryTerminalOutcome,
};
pub use generated_residual_affine_pivot_target_matching::{
    GENERATED_RESIDUAL_AFFINE_PIVOT_TARGET_MATCHING_V1_SCHEMA,
    GeneratedResidualAffinePendingWhenBad, GeneratedResidualAffinePivotTargetMatchingCertificate,
    GeneratedResidualAffinePivotTargetMatchingCompiler,
    GeneratedResidualAffinePivotTargetMatchingError,
    GeneratedResidualAffinePivotTargetMatchingLimits,
    GeneratedResidualAffinePivotTargetMatchingStats, GeneratedResidualAffinePivotTargetOutcome,
    GeneratedResidualAffineRecenteringBoundaryKind, GeneratedResidualAffineRejectedNoTargetCase,
    GeneratedResidualAffineRejectedRecenteringBoundary,
};
pub use generated_residual_affine_when_bad::{
    AFFINE_WHEN_BAD_RELATIVE_PARTITION_V1_SCHEMA, AffineWhenBadAtom, AffineWhenBadClauseProvenance,
    AffineWhenBadInheritedTruth, AffineWhenBadRelativeCase, AffineWhenBadRelativeCaseError,
    AffineWhenBadRelativeCaseId, AffineWhenBadRelativeCaseLimits, AffineWhenBadRelativeCaseStats,
    AffineWhenBadRelativeLeafClassification, AffineWhenBadRelativeLeafDisposition,
    AffineWhenBadRelativePartitionCertificate, AffineWhenBadRelativePredicate,
    AffineWhenBadRelativeSplit, AffineWhenBadRelativeSplitTrigger,
};
pub use generated_sector_conditional_provider::{
    GENERATED_SECTOR_CONDITIONAL_RULE_PROVIDER_V1_SCHEMA, GeneratedSectorConditionalRuleProvenance,
    GeneratedSectorConditionalRuleProvider, GeneratedSectorConditionalRuleProviderBuildStats,
    GeneratedSectorConditionalRuleProviderError, GeneratedSectorConditionalRuleProviderLimits,
    GeneratedSectorConditionalRuleProviderStats, GeneratedSectorSkippedConditionalLocus,
};
pub use generated_sector_discovery::{
    GENERATED_SECTOR_DISCOVERY_V1_SCHEMA, GENERATED_SECTOR_DISCOVERY_V2_SCHEMA,
    GENERATED_SECTOR_DISCOVERY_V3_SCHEMA, GENERATED_SECTOR_DISCOVERY_V4_SCHEMA,
    GeneratedSectorDiscoveryCertificate, GeneratedSectorDiscoveryCompiler,
    GeneratedSectorDiscoveryError, GeneratedSectorDiscoveryLimits, GeneratedSectorDiscoveryStats,
    GeneratedSectorSearchAnchorRequest, GeneratedSectorSearchAnchorTranscript,
};
pub use generated_sector_live_leaf_queue::{
    GENERATED_SECTOR_LIVE_LEAF_QUEUE_V1_SCHEMA, GENERATED_SECTOR_LIVE_LEAF_QUEUE_V2_SCHEMA,
    GeneratedSectorIndexBoundaryInterruption, GeneratedSectorIndexBoundaryWitness,
    GeneratedSectorLiveLeafOutcome, GeneratedSectorLiveLeafQueueCertificate,
    GeneratedSectorLiveLeafQueueCompiler, GeneratedSectorLiveLeafQueueError,
    GeneratedSectorLiveLeafQueueLimits, GeneratedSectorLiveLeafQueueStats,
    GeneratedSectorLiveLeafWorkItem, GeneratedSectorQueuedSourceDisposition,
};
pub use generated_symbolic_row_span::{
    GENERATED_SYMBOLIC_ROW_SPAN_V1_SCHEMA, GeneratedSymbolicRowSpanCertificate,
    GeneratedSymbolicRowSpanCompiler, GeneratedSymbolicRowSpanConfig,
    GeneratedSymbolicRowSpanError, GeneratedSymbolicRowSpanLimits, GeneratedSymbolicRowSpanLineage,
    GeneratedSymbolicRowSpanStats, GeneratedSymbolicRowSpanStrategy,
};
pub use generated_when_bad::{
    GENERATED_SOURCE_AUTHENTICATION_V1_SCHEMA, GENERATED_SOURCE_AUTHENTICATION_V2_SCHEMA,
    GENERATED_WHEN_BAD_V1_SCHEMA, GENERATED_WHEN_BAD_V2_SCHEMA,
    GeneratedSourceAuthenticationCertificate, GeneratedSourceAuthenticationStats,
    GeneratedSourceAuthenticator, GeneratedSourceRowMode, GeneratedSourceRowWitness,
    GeneratedWhenBadCertificate, GeneratedWhenBadCompilation, GeneratedWhenBadCompiler,
    GeneratedWhenBadError, GeneratedWhenBadLimits, GeneratedWhenBadSourceAuthentication,
    GeneratedWhenBadUnsupported,
};
pub use generic_family::{
    AffineDenominator, BaseNonZeroCondition, ContractionMomentum, DenominatorExpansion,
    FamilyDomain, GenericFamily, GenericFamilyError, IntegralFamily,
    IntegralFamilyFingerprintStats, IntegralFamilyLimits, ScalarProductCoordinate,
};
pub use generic_tensor_family::{
    GENERIC_TENSOR_FAMILY_LOWERING_V1_SCHEMA, GenericScalarProductMonomial,
    GenericTensorFamilyError, GenericTensorFamilyLimits, GenericTensorFamilyReducer,
    GenericTensorFamilyStats, GenericTensorIntegralReduction, GenericTensorNumerator,
    GenericTensorTerm, LoweredTensorCoefficient, TensorLoweringDomain, TensorLoweringGuardOrigin,
    TensorLoweringNonZeroCondition, TensorLoweringOrigin,
};
pub use generic_tensor_polynomial::{
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_LOWERING_V1_SCHEMA,
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_LOWERING_V2_SCHEMA,
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PARAMETRIC_REDUCTION_V1_SCHEMA,
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PARAMETRIC_REDUCTION_V2_SCHEMA,
    AuthenticatedVacuumCovariantTensorPolynomialLowering,
    AuthenticatedVacuumCovariantTensorPolynomialParametricReduction,
    AuthenticatedVacuumCovariantTensorPolynomialProjection, CovariantTensorPolynomialMonomial,
    GENERIC_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PROJECTION_V1_SCHEMA,
    GENERIC_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PROJECTION_V2_SCHEMA, GenericTensorPolynomialError,
    GenericTensorPolynomialLimits, GenericTensorPolynomialStats,
    GenericVacuumTensorPolynomialProjector, TensorPolynomialProjectionContribution,
    TensorPolynomialProjectionOrigin, TensorPolynomialReductionEngineError,
    TensorPolynomialWeightGuardOrigin, TensorPolynomialWeightNonZeroCondition,
    WeightedCovariantTensorMonomial,
};
pub use generic_tensor_projector::{
    AUTHENTICATED_VACUUM_TENSOR_LOWERING_V1_SCHEMA, AUTHENTICATED_VACUUM_TENSOR_LOWERING_V2_SCHEMA,
    AuthenticatedVacuumCovariantTensorProjection, AuthenticatedVacuumTensorLowering,
    AuthenticatedVacuumTensorProjection, CovariantTensorMonomial,
    GENERIC_VACUUM_COVARIANT_TENSOR_PROJECTION_V1_SCHEMA,
    GENERIC_VACUUM_COVARIANT_TENSOR_PROJECTION_V2_SCHEMA,
    GENERIC_VACUUM_TENSOR_PROJECTION_V1_SCHEMA, GENERIC_VACUUM_TENSOR_PROJECTION_V2_SCHEMA,
    GenericCovariantTensorNumerator, GenericCovariantTensorTerm, GenericTensorProjectionDomain,
    GenericTensorProjectionStats, GenericTensorProjectorError, GenericTensorProjectorLimits,
    GenericVacuumTensorProjector, IndexedSpectatorVector, SpectatorScalarProduct,
    SpectatorScalarProductMonomial, SpectatorVector, TensorCovariantStructure, TensorLoopReference,
    TensorProjectionGuardOrigin, TensorProjectionNonZeroCondition,
    VacuumCovariantPrecontractionWitness, VacuumCovariantTensorProjectionWitness,
    VacuumCovariantVectorContractionWitness, VacuumMetricContractionWitness,
    VacuumTensorProjectionWitness,
};
pub use guards::{CoefficientLocation, GuardOrigin, GuardRowId};
pub use master_policy::{
    MasterPolicyError, MasterPolicyLimits, MasterPolicyProvider, MasterPolicyTerminal,
};
pub use master_product::{
    MasterProduct, MasterProductError, ProductConvolutionError, ProductLinearCombination,
};
pub use parallel_execution::{ParallelExecution, ParallelExecutionError};
pub use parametric_coefficient::{
    BasePolynomial, CoefficientPolynomial, GuardedCoefficientSpecialization,
    GuardedParametricCoefficient, GuardedPartialCoefficientSpecialization,
    ParametricArithmeticLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricNonZeroCondition, ParametricPolynomial,
    PartialIndexAssignment, RESIDUAL_UNIT_AFFINE_COMPOSITION_V1_SCHEMA,
    ResidualUnitAffineCoefficientCompositionStats, ResidualUnitAffineCompositionError,
    ResidualUnitAffineCompositionPlanLimits, ResidualUnitAffinePolynomialCompositionLimits,
    ResidualUnitAffinePolynomialCompositionStats, SpecializedNonZeroCondition,
};
pub use parametric_elimination::{
    PARAMETRIC_ELIMINATION_V1_SCHEMA, PARAMETRIC_SOURCE_MANIFEST_V1_SCHEMA, ParametricElimination,
    ParametricEliminationError, ParametricEliminationLimits, ParametricEliminationOrdering,
    ParametricEliminationReduction, ParametricEliminationStats, ParametricEliminationTrace,
    ParametricPivotEquation,
};
pub use parametric_ibp::{
    ParametricIbpConfig, ParametricIbpError, ParametricIbpGenerator, ParametricIbpRelations,
};
pub use parametric_relation::{
    ConcreteIntegralKey, ConcreteRelation, IndexShift, IndexSpace,
    PARAMETRIC_RELATION_MANIFEST_V1_SCHEMA, PARAMETRIC_RELATION_MANIFEST_V2_SCHEMA,
    PARTIAL_PARAMETRIC_RELATION_SPECIALIZATION_V1_SCHEMA, ParametricRelation,
    ParametricRelationError, ParametricRowId, PartialParametricRelationSpecialization,
    PartialParametricRelationSpecializationLimits, PartialParametricRelationSpecializationStats,
    PartialSpecializationBaseAssumption,
};
pub use parametric_rules::{
    ConcreteReduction, GeneratedCylindricalApplicationMismatch,
    PARAMETRIC_REDUCTION_RULE_V1_SCHEMA, PARAMETRIC_RULE_DERIVATION_V1_SCHEMA,
    ParametricReductionRule, ParametricReductionRuleCandidate, ParametricRuleApplication,
    ParametricRuleDerivation, ParametricRuleError, ParametricRuleInapplicability,
    ParametricRuleLimits, ParametricRuleUndecidability, RUNTIME_DESCENT_GUARD_V1_SCHEMA,
};
pub use parametric_sector_coverage::{
    PARAMETRIC_SECTOR_COVERAGE_V1_SCHEMA, PARAMETRIC_SECTOR_COVERAGE_V2_SCHEMA,
    PARAMETRIC_SECTOR_COVERAGE_V3_SCHEMA, PARAMETRIC_SECTOR_COVERAGE_V4_SCHEMA,
    ParametricSectorCoverageCertificate, ParametricSectorCoverageCompiler,
    ParametricSectorCoverageError, ParametricSectorCoverageLimits, ParametricSectorCoverageStats,
    ParametricSectorEmptyLocusReason, ParametricSectorLeafClassification,
    ParametricSectorLeafDisposition, ParametricSectorProductZeroDecomposition,
    SectorCoverageCandidateAttempt,
};
pub use parametric_sector_provider::{
    PARAMETRIC_SECTOR_RULE_PROVIDER_V1_SCHEMA, ParametricSectorRuleProvider,
    ParametricSectorRuleProviderError, ParametricSectorRuleProviderLimits,
    ParametricSectorRuleProviderStats,
};
pub use persistent_parametric_elimination::{
    PERSISTENT_PARAMETRIC_ELIMINATION_REFERENCE_V1_SCHEMA, PersistentParametricEliminationBatch,
    PersistentParametricEliminationCertificate, PersistentParametricEliminationDatabase,
    PersistentParametricEliminationError, PersistentParametricEliminationEvent,
    PersistentParametricEliminationLimits, PersistentParametricEliminationRowOutcome,
    PersistentParametricEliminationStats,
};
pub use product_locus_boolean_cover::{
    RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA, ResidualProductLocusBooleanCoverCertificate,
    ResidualProductLocusBooleanCoverCompiler, ResidualProductLocusBooleanCoverError,
    ResidualProductLocusBooleanCoverLimits, ResidualProductLocusBooleanCoverStats,
    ResidualProductLocusBooleanDecision, ResidualProductLocusBooleanEmptyReason,
    ResidualProductLocusBooleanNode, ResidualProductLocusBooleanNodeOutcome,
    ResidualProductLocusBooleanPolarity,
};
pub use reduction_engine::{
    ConcreteRuleApplicationTrace, ConcreteRuleDecision, ConcreteRuleProvider,
    ConcreteTerminalStatus, IncompleteReductionError, PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA,
    ParametricReductionEngine, ParametricReductionResult, ReductionEngineError,
    ReductionEngineLimits, ReductionEngineStats,
};
pub use residual_affine_atom_rows::{
    RESIDUAL_AFFINE_ATOM_ROW_V1_SCHEMA, ResidualAffineAtomRowCertificate,
    ResidualAffineAtomRowError, ResidualAffineAtomRowLimits, ResidualAffineAtomRowOutcome,
    ResidualAffineAtomRowStats, ResidualAffineAtomRowUnsupported, ResidualAffineBaseBlockWitness,
    ResidualAffinePrimitiveRow, ResidualAffinePrimitiveRowError,
};
pub use residual_affine_branch_guard_composition::{
    RESIDUAL_AFFINE_BRANCH_GUARD_COMPOSITION_V1_SCHEMA,
    ResidualAffineBranchGuardCompositionCertificate, ResidualAffineBranchGuardCompositionClass,
    ResidualAffineBranchGuardCompositionEntry, ResidualAffineBranchGuardCompositionError,
    ResidualAffineBranchGuardCompositionLimits, ResidualAffineBranchGuardCompositionStats,
};
pub use residual_affine_branch_system::{
    RESIDUAL_AFFINE_BRANCH_SYSTEM_V1_SCHEMA, ResidualAffineBranchEmptyReason,
    ResidualAffineBranchSystemCertificate, ResidualAffineBranchSystemError,
    ResidualAffineBranchSystemLimits, ResidualAffineBranchSystemOutcome,
    ResidualAffineBranchSystemStats, ResidualAffineBranchUnsupportedReason,
    ResidualAffineBranchZeroAtomOutcome, ResidualAffineBranchZeroAtomRecognition,
};
pub use residual_affine_integer_system::{
    RESIDUAL_AFFINE_INTEGER_SYSTEM_V1_SCHEMA, ResidualAffineIntegerEmptyWitness,
    ResidualAffineIntegerFinalRow, ResidualAffineIntegerMap, ResidualAffineIntegerRowOperation,
    ResidualAffineIntegerSystemCertificate, ResidualAffineIntegerSystemError,
    ResidualAffineIntegerSystemInputError, ResidualAffineIntegerSystemInputRow,
    ResidualAffineIntegerSystemLimits, ResidualAffineIntegerSystemOutcome,
    ResidualAffineIntegerSystemStats, ResidualAffineIntegerSystemUnsupported,
};
pub use residual_unit_affine_index_map::{
    RESIDUAL_UNIT_AFFINE_INDEX_MAP_V1_SCHEMA, ResidualUnitAffineIndexMapCertificate,
    ResidualUnitAffineIndexMapError, ResidualUnitAffineIndexMapLimits,
    ResidualUnitAffineIndexMapStats, ResidualUnitAffineIndexMapUnsupported,
};
pub use sectors::{
    CutConstraint, IntegralComplexityComponent, IntegralComplexityKey, IntegralOrderingPolicy,
    RUSTRED_UNSHIFTED_ORDER_V1_ID, RUSTRED_UNSHIFTED_ORDER_V1_SCHEMA, SectorAnalysisStatus,
    SectorDepthRange, SectorEnumerationLimits, SectorExclusion, SectorFoundationError, SectorMask,
    SectorPattern, SectorPatternMismatch, SectorPatternSlot, SectorRestrictions,
    StrictDescentWitness,
};
pub use shift_operators::{
    IndexShiftOperator, IndexShiftOperatorError, IndexShiftOperatorExpression,
    IndexShiftOperatorKind, IndexShiftOperatorLimits, IndexShiftOperatorMonomial,
    IndexShiftOperatorWord,
};
pub use symbolic_sector_cases::{
    SYMBOLIC_SECTOR_CASE_PARTITION_V1_SCHEMA, SectorOrthantConstraint, SectorOrthantSide,
    SymbolicPolynomialPredicate, SymbolicPolynomialPredicateKind, SymbolicSectorCase,
    SymbolicSectorCaseError, SymbolicSectorCaseId, SymbolicSectorCaseLimits,
    SymbolicSectorCasePartitionBuilder, SymbolicSectorCasePartitionCertificate,
    SymbolicSectorCaseSplit, SymbolicSectorCaseSplitChildren, SymbolicSectorCaseStats,
    SymbolicSectorOrthant,
};
pub use symbolic_symmetry_transport::{
    SYMBOLIC_SYMMETRY_ROW_TRANSPORT_V1_SCHEMA, SymbolicSymmetryRowTransportCertificate,
    SymbolicSymmetryRowTransportCompiler, SymbolicSymmetryRowTransportError,
    SymbolicSymmetryRowTransportLimits, SymbolicSymmetryRowTransportStats,
};
pub use symbolica_affine_denominator::{
    CompiledSymbolicaAffineDenominator, SYMBOLICA_AFFINE_DENOMINATOR_V1_SCHEMA,
    SymbolicaAffineDenominatorCompiler, SymbolicaAffineDenominatorError,
    SymbolicaAffineDenominatorLimits, SymbolicaAffineDenominatorStats,
};
pub use symbolica_integral_input::{
    ExternalGramInputV1, LoweredSymbolicaDenominatorV1, LoweredSymbolicaProjectV1,
    NormalizedProjectInputV1, NormalizedProjectPartsV1, NormalizedProjectSourceV1,
    NormalizedPropagatorV1, NormalizedTargetV1, ParameterSourceV1, PropagatorInputV1,
    RUSTRED_LOWERED_SYMBOLICA_PROJECT_V1_SCHEMA, RUSTRED_PROJECT_TOML_V1_SCHEMA,
    RUSTRED_SYMBOLICA_INTEGRAL_V1_SCHEMA, SymbolicaIntegralInputCompiler,
    SymbolicaIntegralInputError, SymbolicaIntegralInputLimits, SymbolicaIntegralInputStats,
    SymbolicaProjectLoweringError, SymbolicaProjectLoweringLimits, TextExternalGramInputV1,
    TextProjectPartsV1, TextPropagatorInputV1,
};
pub use symbolica_runtime::{
    canonical_symbolica_atom, symbolica_atom_packed_byte_size, symbolica_integer_significant_bits,
    symbolica_integer_structural_byte_size, symbolica_runtime_version,
};
pub use symbolica_target_numerator::{
    CompiledSymbolicaTargetV1, SYMBOLICA_COMPILED_TARGET_V1_SCHEMA,
    SymbolicaTargetCompilationStats, SymbolicaTargetNumeratorCompiler,
    SymbolicaTargetNumeratorError, SymbolicaTargetNumeratorLimits,
};
pub use symbolica_tensor_numerator::{
    CompiledSymbolicaTensorNumerator, SymbolicaIndexAllocation, SymbolicaIndexAllocationOrigin,
    SymbolicaSpectatorAllocation, SymbolicaTensorCompilationStats,
    SymbolicaTensorNumeratorCompiler, SymbolicaTensorNumeratorError,
    SymbolicaTensorNumeratorLimits, SymbolicaTensorSyntax,
    SymbolicaWeightedCovariantTensorMonomial,
};
pub use symmetry::{
    AFFINE_FAMILY_MAP_V1_SCHEMA, AFFINE_FAMILY_MAP_V2_SCHEMA, AffineDenominatorMap,
    AffineScalarProductMap, DenominatorRowAction, ExactMatrix, JacobianWitness, MomentumMap,
    SymmetryVerificationError, SymmetryVerificationLimits, SymmetryVerificationStats,
    VerifiedAffineFamilyMap, verify_affine_family_map,
};
pub use symmetry_discovery::{
    BOUNDED_INTEGER_VACUUM_SYMMETRY_SEARCH_V1_SCHEMA,
    INTERNAL_FAMILY_PERMUTATION_SYMMETRY_V1_SCHEMA, InternalSymmetryCompatibilityError,
    InternalSymmetryKeyTransportError, InternalSymmetryReplayError,
    InternalSymmetrySearchCompletion, InternalSymmetrySearchError, InternalSymmetrySearchLimits,
    InternalSymmetrySearchReport, InternalSymmetrySearchStats,
    VerifiedInternalFamilyPermutationSymmetry, compile_internal_family_permutation_symmetry,
    discover_bounded_vacuum_internal_symmetries,
};
pub use tensor::{
    IndexedVector, LoopVector, LorentzIndex, Metric, MetricPairing, ScalarProduct,
    ScalarProductMonomial, SlotPairing, TensorConstructionLimits, TensorError, TensorMonomial,
    TensorReduction, TensorTerm, VacuumTensorProjector, perfect_matching_count, perfect_matchings,
};
pub use tensor_reduction_engine::{
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_LOWERING_V1_SCHEMA,
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_LOWERING_V2_SCHEMA,
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_PARAMETRIC_REDUCTION_V1_SCHEMA,
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_PARAMETRIC_REDUCTION_V2_SCHEMA,
    AUTHENTICATED_VACUUM_TENSOR_PARAMETRIC_REDUCTION_V1_SCHEMA,
    AUTHENTICATED_VACUUM_TENSOR_PARAMETRIC_REDUCTION_V2_SCHEMA,
    AuthenticatedVacuumCovariantTensorLowering,
    AuthenticatedVacuumCovariantTensorParametricReduction,
    AuthenticatedVacuumCovariantTensorReductionDomains,
    AuthenticatedVacuumTensorParametricReduction, AuthenticatedVacuumTensorReductionDomains,
    COVARIANT_TENSOR_PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA, CovariantTensorLoweringStats,
    CovariantTensorParametricReductionResult, IncompleteTensorReductionError,
    ScalarReductionWitness, TENSOR_PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA, TensorIntegralLeaf,
    TensorParametricReductionComposer, TensorParametricReductionResult, TensorReducedCoefficient,
    TensorReductionCertificateError, TensorReductionEngineError, TensorReductionEngineLimits,
    TensorReductionEngineStats, TensorReductionGuard, TensorReductionTermOrigin,
    TensorScalarSource,
};
pub use when_bad::{
    WHEN_BAD_COMPILER_V1_SCHEMA, WHEN_BAD_COMPILER_V2_SCHEMA, WhenBadBoundaryHazardKind,
    WhenBadCandidateBinding, WhenBadCandidateSourceAuthority, WhenBadCertificate,
    WhenBadCompilation, WhenBadCompiler, WhenBadCompilerError, WhenBadCompilerLimits,
    WhenBadCompilerStats, WhenBadDescentComponent, WhenBadDomainCondition,
    WhenBadDomainConditionSource, WhenBadLeafClassification, WhenBadLeafDisposition,
    WhenBadLeakEvent, WhenBadLeakNumeratorGate, WhenBadOrderingAuthority,
    WhenBadSourceAuthentication, WhenBadUniformDescentWitness, WhenBadUnsupported,
    WhenBadUnsupportedReason,
};
pub use zero_sector_provider::{
    CERTIFIED_ZERO_SECTOR_RULE_PROVIDER_V1_SCHEMA, CertifiedZeroSectorRuleProvider,
    CertifiedZeroSectorRuleProviderError,
};
pub use zero_sectors::{
    FullColumnRankWitness, PowerShiftPolicy, ZERO_SECTOR_CERTIFICATE_SCHEMA, ZeroSectorAnalysis,
    ZeroSectorAnalyzer, ZeroSectorCertificate, ZeroSectorConditionSource, ZeroSectorDecision,
    ZeroSectorDomain, ZeroSectorDomainCondition, ZeroSectorError, ZeroSectorLimits,
    ZeroSectorResource,
};
