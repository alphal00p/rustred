//! RustRed: a pure-Rust, Symbolica-backed port of LiteRed-style parametric IBP
//! derivation and reduction.
//!
//! The generic production path is loop-count and topology independent:
//! [`IntegralFamily`] authenticates a complete affine scalar-product basis and
//! [`ParametricIbpGenerator`] derives reusable ordinary and Lorentz-invariance
//! identities over the exact field `K(n)`. [`IndexShiftOperatorExpression`]
//! provides exact ordered `A`/`B` action and relation round trips; it is an
//! intermediate whose coefficients may still contain `n`, not LiteRed's
//! completed `ToAB` polynomial form. Loop/topology-authored recurrences are not
//! part of the generic production crate and are not sources of generic
//! parametric identities or future discovered rules.

mod exact_identity;

pub mod adaptive_rules;
pub mod algebra;
pub mod automatic_isps;
pub mod campaign;
pub mod certified_rewrite;
pub mod conditional_reelimination;
pub mod conditional_rules;
pub mod exact_sparse_elimination;
pub mod feynman_polynomials;
pub mod generated_symbolic_row_span;
pub mod generic_family;
pub mod generic_tensor_family;
pub mod generic_tensor_polynomial;
pub mod generic_tensor_projector;
pub mod guards;
pub mod master_product;
pub mod parametric_coefficient;
pub mod parametric_elimination;
pub mod parametric_ibp;
pub mod parametric_relation;
pub mod parametric_rules;
pub mod reduction_engine;
pub mod runtime;
pub mod sectors;
pub mod shift_operators;
pub mod symbolic_symmetry_transport;
pub mod symbolica_affine_denominator;
pub mod symbolica_integral_input;
pub mod symbolica_target_numerator;
pub mod symbolica_tensor_numerator;
pub mod symmetry;
pub mod symmetry_discovery;
pub mod tensor;
pub mod tensor_reduction_engine;
pub mod zero_sectors;

pub use adaptive_rules::{
    ADAPTIVE_PARAMETRIC_RULE_SEARCH_V1_SCHEMA, AdaptiveParametricRuleProvider,
    AdaptiveRuleSearchError, AdaptiveRuleSearchLimits, AdaptiveRuleSearchStats,
};
pub use automatic_isps::{
    AUTOMATIC_ISP_COMPLETION_V1_SCHEMA, AUTOMATIC_ISP_COMPLETION_V2_SCHEMA, AutomaticIspCompletion,
    AutomaticIspCompletionError, AutomaticIspCompletionLimits, AutomaticIspCompletionStats,
};
pub use campaign::{ParallelExecution, ParallelExecutionError};
pub use certified_rewrite::{
    CERTIFIED_CONCRETE_REWRITE_V1_SCHEMA, CERTIFIED_CONCRETE_REWRITE_V2_SCHEMA,
    CERTIFIED_ZERO_REDUCTION_V1_SCHEMA, CertifiedConcreteRewrite, CertifiedConcreteRewriteProof,
    CertifiedRewriteDomainCondition, CertifiedRewriteDomainOrigin, CertifiedRewriteError,
    CertifiedRewriteLimits, CertifiedZeroReduction, CertifiedZeroReductionProof,
    ConcreteQuotientSourceRowProof, QuotientTermWitness,
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
pub use exact_sparse_elimination::{
    ExactSparseCoefficientLocation, ExactSparseDerivationReduction, ExactSparseDerivationTrace,
    ExactSparseElimination, ExactSparseEliminationConfig, ExactSparseEliminationError,
    ExactSparseEliminationStats, ExactSparsePivotRule, ExactSparseRow,
};
pub use feynman_polynomials::{
    FeynmanPolynomial, FeynmanPolynomialContext, FeynmanPolynomialError, FeynmanPolynomialLimits,
    RawFeynmanPolynomial, SymanzikPolynomials,
};
pub use generated_symbolic_row_span::{
    GENERATED_SYMBOLIC_ROW_SPAN_V1_SCHEMA, GeneratedSymbolicRowSpanCertificate,
    GeneratedSymbolicRowSpanCompiler, GeneratedSymbolicRowSpanConfig,
    GeneratedSymbolicRowSpanError, GeneratedSymbolicRowSpanLimits, GeneratedSymbolicRowSpanLineage,
    GeneratedSymbolicRowSpanStats, GeneratedSymbolicRowSpanStrategy,
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
pub use master_product::{
    MasterProduct, MasterProductError, ProductConvolutionError, ProductLinearCombination,
};
pub use parametric_coefficient::{
    BasePolynomial, CoefficientPolynomial, GuardedCoefficientSpecialization,
    GuardedParametricCoefficient, GuardedPartialCoefficientSpecialization,
    ParametricArithmeticLimits, ParametricCoefficient, ParametricCoefficientContext,
    ParametricCoefficientError, ParametricNonZeroCondition, ParametricPolynomial,
    PartialIndexAssignment, ResidualUnitAffineCoefficientCompositionStats,
    ResidualUnitAffineCompositionError, ResidualUnitAffineCompositionPlanLimits,
    ResidualUnitAffinePolynomialCompositionLimits, ResidualUnitAffinePolynomialCompositionStats,
    SpecializedNonZeroCondition,
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
    ConcreteReduction, PARAMETRIC_REDUCTION_RULE_V1_SCHEMA, PARAMETRIC_RULE_DERIVATION_V1_SCHEMA,
    ParametricReductionRule, ParametricReductionRuleCandidate, ParametricRuleApplication,
    ParametricRuleDerivation, ParametricRuleError, ParametricRuleInapplicability,
    ParametricRuleLimits, ParametricRuleUndecidability, RUNTIME_DESCENT_GUARD_V1_SCHEMA,
};
pub use reduction_engine::{
    ConcreteRuleApplicationTrace, ConcreteRuleDecision, ConcreteRuleProvider,
    ConcreteTerminalStatus, IncompleteReductionError, PARAMETRIC_REDUCTION_ENGINE_V1_SCHEMA,
    ParametricReductionEngine, ParametricReductionResult, ReductionEngineError,
    ReductionEngineLimits, ReductionEngineStats,
};
pub use sectors::{
    CutConstraint, IntegralComplexityComponent, IntegralComplexityKey, IntegralOrderingPolicy,
    RUSTRED_UNSHIFTED_ORDER_V1_ID, RUSTRED_UNSHIFTED_ORDER_V1_SCHEMA, SectorAnalysisStatus,
    SectorExclusion, SectorFoundationError, SectorMask, SectorPattern, SectorPatternMismatch,
    SectorPatternSlot, SectorRestrictions, StrictDescentWitness,
};
pub use shift_operators::{
    IndexShiftOperator, IndexShiftOperatorError, IndexShiftOperatorExpression,
    IndexShiftOperatorKind, IndexShiftOperatorLimits, IndexShiftOperatorMonomial,
    IndexShiftOperatorWord,
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
    INTERNAL_FAMILY_PERMUTATION_SYMMETRY_V1_SCHEMA, InternalSymmetryCompatibilityError,
    InternalSymmetryKeyTransportError, InternalSymmetryReplayError,
    VerifiedInternalFamilyPermutationSymmetry, compile_internal_family_permutation_symmetry,
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
pub use zero_sectors::{
    FullColumnRankWitness, PowerShiftPolicy, ZERO_SECTOR_CERTIFICATE_SCHEMA, ZeroSectorAnalyzer,
    ZeroSectorCertificate, ZeroSectorConditionSource, ZeroSectorDecision, ZeroSectorDomain,
    ZeroSectorDomainCondition, ZeroSectorError, ZeroSectorLimits, ZeroSectorResource,
};
