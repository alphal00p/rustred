//! RustRed: a pure-Rust, Symbolica-backed port of LiteRed-style parametric IBP
//! derivation and reduction.
//!
//! The generic production path is loop-count and topology independent:
//! [`IntegralFamily`] authenticates a complete affine scalar-product basis and
//! [`ParametricIbpGenerator`] derives reusable ordinary and Lorentz-invariance
//! identities over the exact field `K(n)`. [`IndexShiftOperatorExpression`]
//! provides exact ordered `A`/`B` action and relation round trips; it is an
//! intermediate whose coefficients may still contain `n`, not LiteRed's
//! completed `ToAB` polynomial form. All loop/topology-authored fixture and
//! reducer modules are legacy finite-certificate/compatibility oracles; they
//! are not the source of the generic parametric identities or future
//! discovered rules. Those modules, including the topology-specific Vakint
//! adapter, are excluded from the default surface and are available only
//! through the `legacy-authored-oracles` feature.

mod coverage_decision_dag;
mod direct_bad_formula;
mod exact_identity;
mod parametric_sector_formula_ir;
mod parametric_sector_mtbdd;
mod parametric_sector_mtbdd_certificate;

pub mod adaptive_rules;
pub mod affine_locus_bound_relation;
pub mod affine_parametric_ordering;
pub mod affine_prepare_point_schedule;
pub mod affine_prepare_points;
pub mod automatic_isps;
pub mod base_specialization;
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
#[cfg(feature = "legacy-authored-oracles")]
pub mod families;
pub mod family;
pub mod family_sector_inventory;
pub mod feynman_polynomials;
#[cfg(feature = "legacy-authored-oracles")]
pub mod five_loop;
#[cfg(feature = "legacy-authored-oracles")]
pub mod five_loop_boundary;
#[cfg(feature = "legacy-authored-oracles")]
pub mod five_loop_d2;
#[cfg(feature = "legacy-authored-oracles")]
pub mod five_loop_d3;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_boundary;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_boundary_halo;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_component_transport;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_corner_shell;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_genuine;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_halo;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_next_closed_rows;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_next_corner_cross_auth;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_next_elimination;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_next_inventory;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_next_manifest;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_next_modular_rank;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_polynomial_halo;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_t1s2_closure;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_three_loop_closure;
#[cfg(feature = "legacy-authored-oracles")]
pub mod four_loop_three_loop_service;
pub(crate) mod generated_affine_initial_global_affine_terminal;
pub(crate) mod generated_affine_parametric_ordering;
pub(crate) mod generated_affine_prepare_point_schedule;
pub(crate) mod generated_affine_residual_boolean_cover;
mod generated_affine_residual_case_bound_relation;
pub(crate) mod generated_affine_residual_case_inventory;
mod generated_affine_residual_case_pivot_target_matching;
mod generated_affine_residual_case_premises;
mod generated_affine_residual_case_reelimination;
mod generated_affine_residual_group_exact_database;
mod generated_affine_residual_group_exact_physical_row;
mod generated_affine_residual_group_exact_recenter_kernel;
mod generated_affine_residual_group_exact_relation;
mod generated_affine_residual_group_exact_session;
mod generated_affine_residual_group_exact_targets;
mod generated_affine_residual_group_physical_key;
mod generated_affine_residual_group_solve_plan;
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
#[cfg(test)]
mod generated_residual_affine_group_effective_coverage_tests;
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
pub mod ibp;
pub mod integral;
pub mod linear;
pub mod master_policy;
pub mod master_product;
#[cfg(feature = "legacy-authored-oracles")]
pub mod one_loop;
pub mod parametric_coefficient;
pub mod parametric_elimination;
pub mod parametric_ibp;
pub mod parametric_relation;
pub mod parametric_rules;
pub mod parametric_sector_coverage;
pub mod parametric_sector_provider;
pub mod persistent_parametric_elimination;
#[cfg(feature = "legacy-authored-oracles")]
pub mod product_boundary;
pub mod product_locus_boolean_cover;
pub mod reduction;
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
pub mod symbolica_target_numerator;
pub mod symbolica_tensor_numerator;
pub mod symmetry;
pub mod symmetry_discovery;
pub mod tensor;
pub mod tensor_family;
pub mod tensor_reduction_engine;
#[cfg(feature = "legacy-authored-oracles")]
pub mod three_loop;
#[cfg(feature = "legacy-authored-oracles")]
pub mod three_loop_b4_d2;
#[cfg(feature = "legacy-authored-oracles")]
pub mod three_loop_boundary;
#[cfg(feature = "legacy-authored-oracles")]
pub mod three_loop_f5_d2n1;
#[cfg(feature = "legacy-authored-oracles")]
pub mod three_loop_pipeline;
#[cfg(feature = "legacy-authored-oracles")]
pub mod three_loop_proper_dot;
#[cfg(feature = "legacy-authored-oracles")]
pub mod three_loop_top_dot;
#[cfg(feature = "legacy-authored-oracles")]
pub mod two_loop;
#[cfg(feature = "legacy-authored-oracles")]
pub mod two_loop_pipeline;
#[cfg(feature = "legacy-authored-oracles")]
pub mod two_loop_top_dot;
#[cfg(feature = "legacy-authored-oracles")]
pub mod vakint_adapter;
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
    AUTOMATIC_ISP_COMPLETION_V1_SCHEMA, AutomaticIspCompletion, AutomaticIspCompletionError,
    AutomaticIspCompletionLimits, AutomaticIspCompletionStats,
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
pub use exact::ExactRational;
pub use exact_sparse_elimination::{
    ExactSparseCoefficientLocation, ExactSparseDerivationReduction, ExactSparseDerivationTrace,
    ExactSparseElimination, ExactSparseEliminationConfig, ExactSparseEliminationError,
    ExactSparseEliminationStats, ExactSparsePivotRule, ExactSparseRow,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use families::{
    equal_mass_two_loop_vacuum, equal_mass_two_loop_vacuum_in_context,
    equal_mass_two_loop_vacuum_reversed,
};
pub use family::{
    Denominator, FamilyConstructionLimits, FamilyError, PropagatorSign, ScalarProductExpansion,
    VacuumFamily,
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
#[cfg(feature = "legacy-authored-oracles")]
pub use five_loop::{FIVE_LOOP_BANANA_ROUTINGS, equal_mass_five_loop_banana};
#[cfg(feature = "legacy-authored-oracles")]
pub use five_loop_boundary::{
    FIVE_LOOP_BANANA_AUXILIARIES, FIVE_LOOP_BANANA_AUXILIARY_LINE_PAIRS,
    FIVE_LOOP_BANANA_DENOMINATORS, FIVE_LOOP_BANANA_LOOP_MOMENTA, FIVE_LOOP_BANANA_PHYSICAL_LINES,
    FIVE_LOOP_BANANA_S6_ADJACENT_TRANSPOSITIONS, FIVE_LOOP_BANANA_S6_ORDER,
    FiveLoopBananaBoundaryConfig, FiveLoopBananaBoundaryError, FiveLoopBananaBoundaryReducer,
    FiveLoopBananaOrbitWitness, FiveLoopBananaPermutationError, FiveLoopBananaPhysicalPermutation,
    FiveLoopBananaProductNumeratorWitness, FiveLoopBananaScalarClass,
    five_loop_banana_oriented_line_routing, five_loop_banana_physical_orbit_witness,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use five_loop_d2::{FiveLoopBananaD2Config, FiveLoopBananaD2Error, FiveLoopBananaD2Reducer};
#[cfg(feature = "legacy-authored-oracles")]
pub use five_loop_d3::{
    FIVE_LOOP_BANANA_D3_ALGEBRAIC_CANDIDATE_BOUND, FIVE_LOOP_BANANA_D3_ALGEBRAIC_ROWS,
    FIVE_LOOP_BANANA_D3_COLLECTED_NONZERO_BOUND, FIVE_LOOP_BANANA_D3_ELIMINATION_UPDATE_BOUND,
    FIVE_LOOP_BANANA_D3_GLOBAL_COLUMNS, FIVE_LOOP_BANANA_D3_MOMENT_POWER_CLASSES,
    FIVE_LOOP_BANANA_D3_NATIVE_EXPANSION_BOUND, FIVE_LOOP_BANANA_D3_NATIVE_RAW_ORIGINS,
    FIVE_LOOP_BANANA_D3_NONZERO_RAW_ROWS, FIVE_LOOP_BANANA_D3_PROPER_BOUNDARY_ROWS,
    FIVE_LOOP_BANANA_D3_RANK, FIVE_LOOP_BANANA_D3_RAW_GRAPH_TERM_BOUND,
    FIVE_LOOP_BANANA_D3_SEED_ORBITS, FIVE_LOOP_BANANA_D3_SOURCE_WEIGHT_BOUND,
    FIVE_LOOP_BANANA_D3_SYMMETRY_IMAGE_BOUND, FiveLoopBananaD3AlgebraicKind,
    FiveLoopBananaD3AlgebraicRowId, FiveLoopBananaD3BoundaryClosure, FiveLoopBananaD3BoundaryRowId,
    FiveLoopBananaD3Column, FiveLoopBananaD3ConditionSource, FiveLoopBananaD3Config,
    FiveLoopBananaD3Error, FiveLoopBananaD3NativeRowId, FiveLoopBananaD3NonzeroCondition,
    FiveLoopBananaD3NormalizedRow, FiveLoopBananaD3PivotRule, FiveLoopBananaD3RowId,
    FiveLoopBananaD3Seed, FiveLoopBananaD3SeedOrbit, FiveLoopBananaD3Shell, FiveLoopBananaD3Stats,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop::{
    FOUR_LOOP_BMW_ROUTINGS, FOUR_LOOP_FG_ROUTINGS, FOUR_LOOP_H_ROUTINGS, FOUR_LOOP_X_ROUTINGS,
    FourLoopTopology, equal_mass_four_loop_bmw, equal_mass_four_loop_fg, equal_mass_four_loop_h,
    equal_mass_four_loop_vacuum, equal_mass_four_loop_x,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_boundary::{
    FourLoopBoundaryConfig, FourLoopBoundaryError, FourLoopBoundaryReducer,
    FourLoopComponentWitness, FourLoopFactorizationWitness, FourLoopLineCoordinate,
    FourLoopScalarClass, FourLoopSignedLineMatch, MassiveVacuumMaster,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_boundary_halo::{
    FOUR_LOOP_BOUNDARY_HALO_BLOCKER_OCCURRENCES, FOUR_LOOP_BOUNDARY_HALO_FORMULA_DISPATCHES,
    FOUR_LOOP_BOUNDARY_HALO_OUTPUT_PRODUCTS, FOUR_LOOP_BOUNDARY_HALO_PRECOLLECTION_TERMS,
    FOUR_LOOP_BOUNDARY_HALO_PRODUCT_MULTIPLICATIONS,
    FOUR_LOOP_BOUNDARY_HALO_SIGNED_LINE_DISPATCHES, FOUR_LOOP_BOUNDARY_HALO_UNIQUE_WITNESS_PLANS,
    FourLoopBoundaryHaloConfig, FourLoopBoundaryHaloError, FourLoopBoundaryHaloPlan,
    FourLoopBoundaryHaloReducer, FourLoopBoundaryHaloReduction, FourLoopBoundaryHaloStats,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_component_transport::{
    FOUR_LOOP_COMPONENT_TRANSPORT_AFFINE_CONSTANTS,
    FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENT_MAP_ENTRIES, FOUR_LOOP_COMPONENT_TRANSPORT_COMPONENTS,
    FOUR_LOOP_COMPONENT_TRANSPORT_CROSS_COEFFICIENTS,
    FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_COEFFICIENTS, FOUR_LOOP_COMPONENT_TRANSPORT_LOCAL_SLOTS,
    FOUR_LOOP_COMPONENT_TRANSPORT_LOOP_MAP_ENTRIES, FOUR_LOOP_COMPONENT_TRANSPORT_OCCURRENCES,
    FOUR_LOOP_COMPONENT_TRANSPORT_PARITY_PROJECTIONS, FOUR_LOOP_COMPONENT_TRANSPORT_PLANS,
    FOUR_LOOP_COMPONENT_TRANSPORT_RATIONAL_OPERATIONS,
    FOUR_LOOP_COMPONENT_TRANSPORT_SCALAR_BRANCHES,
    FOUR_LOOP_COMPONENT_TRANSPORT_SIGNED_LINE_REPLAYS,
    FOUR_LOOP_COMPONENT_TRANSPORT_TRANSFORMED_COEFFICIENTS, FourLoopComponentAffineImage,
    FourLoopComponentBasisColumn, FourLoopComponentParityWitness, FourLoopComponentScalarBranch,
    FourLoopComponentScalarBranchKind, FourLoopComponentTransport,
    FourLoopComponentTransportConfig, FourLoopComponentTransportError,
    FourLoopComponentTransportOccurrence, FourLoopComponentTransportPlan,
    FourLoopComponentTransportStats, FourLoopComponentTransportStatus, FourLoopTransportComponent,
    FourLoopTransportLineAssignment,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_corner_shell::{
    FOUR_LOOP_CORNER_SHELL_COLLECTED_NONZERO_BOUND,
    FOUR_LOOP_CORNER_SHELL_ELIMINATION_UPDATE_BOUND, FOUR_LOOP_CORNER_SHELL_GLOBAL_COLUMN_BOUND,
    FOUR_LOOP_CORNER_SHELL_NORMALIZATION_CONTRIBUTION_BOUND, FOUR_LOOP_CORNER_SHELL_RAW_ROWS,
    FOUR_LOOP_CORNER_SHELL_RAW_TERM_INCIDENCE_BOUND, FOUR_LOOP_CORNER_SHELL_SOURCE_WEIGHT_BOUND,
    FourLoopBoundaryHaloCensusKey, FourLoopBoundaryHaloClosure, FourLoopCornerBlockedRow,
    FourLoopCornerColumnId, FourLoopCornerNormalizedRow, FourLoopCornerPivotRule,
    FourLoopCornerRawRowId, FourLoopCornerShellCertificate, FourLoopCornerShellConfig,
    FourLoopCornerShellError, FourLoopCornerShellStatus, FourLoopReferenceTopology,
    UnsupportedBoundaryHalo, four_loop_corner_seed,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_genuine::{
    FourLoopGenuineClass, FourLoopGenuineClassifier, FourLoopGenuineConfig,
    FourLoopGenuineCornerType, FourLoopGenuineError, FourLoopGenuineLineMatch,
    FourLoopGenuineWitness,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_halo::{
    FOUR_LOOP_AFFINE_MAP_OPERATION_BOUND, FourLoopAffineDenominatorImage, FourLoopHaloColumnKey,
    FourLoopHaloConfig, FourLoopHaloError, FourLoopHaloMapper,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_next_closed_rows::{
    FOUR_LOOP_NEXT_CLOSED_ROWS, FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_OCCURRENCES,
    FOUR_LOOP_NEXT_CLOSED_ROWS_BOUNDARY_PLANS, FOUR_LOOP_NEXT_CLOSED_ROWS_CANCELED_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_CHECKSUM, FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_ADDITIONS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_DIVISIONS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_COEFFICIENT_MULTIPLICATIONS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRIES, FOUR_LOOP_NEXT_CLOSED_ROWS_COLLECTED_ENTRY_BOUND,
    FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_COLUMNS, FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_GENUINE_PATHS, FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMN_BOUND,
    FOUR_LOOP_NEXT_CLOSED_ROWS_GLOBAL_COLUMNS, FOUR_LOOP_NEXT_CLOSED_ROWS_MASS_POWER_STEPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_BOUNDARY_GROUPS, FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_PATHS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_MAX_ROW_WIDTH,
    FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_CONTRIBUTORS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_NONZERO_BOUNDARY_GROUPS, FOUR_LOOP_NEXT_CLOSED_ROWS_PATHS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTION_BOUND,
    FOUR_LOOP_NEXT_CLOSED_ROWS_PRIMARY_CONTRIBUTIONS, FOUR_LOOP_NEXT_CLOSED_ROWS_PRODUCT_COLUMNS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTION_BOUND,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_AUDIT_CONTRIBUTIONS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RAW_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_CANCELED_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_REPEATED_SURVIVING_BOUNDARY_GROUPS,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RETAINED_COEFFICIENT_BYTES,
    FOUR_LOOP_NEXT_CLOSED_ROWS_RETAINED_COEFFICIENT_TERMS, FOUR_LOOP_NEXT_CLOSED_ROWS_ZERO_ROWS,
    FourLoopNextBoundaryGroup, FourLoopNextClosedRow, FourLoopNextClosedRows,
    FourLoopNextClosedRowsConfig, FourLoopNextClosedRowsError, FourLoopNextClosedRowsStats,
    FourLoopNextClosedRowsStatus, FourLoopNextClosureSlice, FourLoopNextOccurrenceBinding,
    FourLoopNextPathDisposition, FourLoopNextPlanBinding,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_next_corner_cross_auth::{
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_COLUMNS, FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ENTRIES,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_RANK, FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_BASE_ROWS,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_CHECKSUM,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_COEFFICIENT_PROJECTIONS,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_EMBEDDED_ENTRIES,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_INHERITED_COLUMNS,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_D1_N0, FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_D1_N1,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_PIVOTED_NONTERMINALS,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_PRODUCTS,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_SCALARS,
    FOUR_LOOP_NEXT_CORNER_CROSS_AUTH_RETAINED_TERMINALS, FourLoopNextCornerCrossAuth,
    FourLoopNextCornerCrossAuthError, FourLoopNextCornerCrossAuthRowSide,
    FourLoopNextCornerCrossAuthStats, FourLoopNextCornerCrossAuthStatus,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_next_elimination::{
    FOUR_LOOP_NEXT_ELIMINATION_CHECKSUM, FOUR_LOOP_NEXT_ELIMINATION_COLUMNS,
    FOUR_LOOP_NEXT_ELIMINATION_CONSERVATIVE_CONDITION_SLOTS,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_ARITHMETIC_UPDATES, FOUR_LOOP_NEXT_ELIMINATION_EXACT_CHECKSUM,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_MAXIMUM_COEFFICIENT_DEGREE,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_MAXIMUM_ROW_WIDTH,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_PIVOT_REDUCTIONS,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_REPLAY_REDUCTIONS,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_REPLAY_UPDATES,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_COEFFICIENT_BYTES,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_COEFFICIENT_TERMS,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_RETAINED_ENTRIES,
    FOUR_LOOP_NEXT_ELIMINATION_EXACT_VERIFICATION_REDUCTIONS,
    FOUR_LOOP_NEXT_ELIMINATION_FREE_UNRESOLVED_COLUMNS, FOUR_LOOP_NEXT_ELIMINATION_INPUT_ENTRIES,
    FOUR_LOOP_NEXT_ELIMINATION_MAXIMUM_INPUT_ROW_WIDTH,
    FOUR_LOOP_NEXT_ELIMINATION_MAXIMUM_TRACE_REDUCTIONS,
    FOUR_LOOP_NEXT_ELIMINATION_MODULAR_CANDIDATE_RANK, FOUR_LOOP_NEXT_ELIMINATION_MODULAR_IMAGES,
    FOUR_LOOP_NEXT_ELIMINATION_PARENT_COEFFICIENT_DENOMINATOR_SLOTS,
    FOUR_LOOP_NEXT_ELIMINATION_PARENT_ROW_SCALE_SLOTS, FOUR_LOOP_NEXT_ELIMINATION_PIVOT_RULES,
    FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_COEFFICIENT_BYTES,
    FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_COEFFICIENT_TERMS,
    FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_RHS_ENTRIES,
    FOUR_LOOP_NEXT_ELIMINATION_PROJECTED_SOURCE_CHECKSUM, FOUR_LOOP_NEXT_ELIMINATION_RANK,
    FOUR_LOOP_NEXT_ELIMINATION_RULE_RHS_DENOMINATOR_SLOTS, FOUR_LOOP_NEXT_ELIMINATION_SOURCE_ROWS,
    FOUR_LOOP_NEXT_ELIMINATION_TRACE_DIVISOR_SLOTS,
    FOUR_LOOP_NEXT_ELIMINATION_TRACE_FACTOR_DENOMINATOR_SLOTS,
    FOUR_LOOP_NEXT_ELIMINATION_TRACE_REDUCTIONS, FourLoopNextElimination,
    FourLoopNextEliminationConditionStatus, FourLoopNextEliminationConditions,
    FourLoopNextEliminationConfig, FourLoopNextEliminationError, FourLoopNextEliminationPivotRule,
    FourLoopNextEliminationStats, FourLoopNextEliminationStatus, FourLoopNextEliminationTrace,
    FourLoopNextEliminationTraceReduction,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_next_inventory::{
    FOUR_LOOP_NEXT_INVENTORY_CACHED_UNIT_PATH_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_CLASSIFICATION_CACHE_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_COEFFICIENT_ADDITION_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_COEFFICIENT_MULTIPLICATION_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_FULL_POWER_KEY_BOUND, FOUR_LOOP_NEXT_INVENTORY_INITIAL_PATH_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_MAPPER_CACHE_BOUND, FOUR_LOOP_NEXT_INVENTORY_MAPPERS,
    FOUR_LOOP_NEXT_INVENTORY_PATH_BOUND, FOUR_LOOP_NEXT_INVENTORY_RAW_INCIDENCES,
    FOUR_LOOP_NEXT_INVENTORY_RAW_ROWS, FOUR_LOOP_NEXT_INVENTORY_RECURSION_DEPTH,
    FOUR_LOOP_NEXT_INVENTORY_RECURSIVE_PATH_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_RETAINED_DYNAMIC_BYTE_BOUND, FOUR_LOOP_NEXT_INVENTORY_ROW_PATH_BOUND,
    FOUR_LOOP_NEXT_INVENTORY_UNIT_CACHE_ENTRY_BOUND, FourLoopNextBoundaryKey,
    FourLoopNextBoundaryOccurrence, FourLoopNextBoundaryTargetSummary,
    FourLoopNextCollectedBoundary, FourLoopNextCompactPath, FourLoopNextGenuineColumn,
    FourLoopNextInventory, FourLoopNextInventoryConfig, FourLoopNextInventoryError,
    FourLoopNextInventoryRow, FourLoopNextInventoryStats, FourLoopNextInventoryStatus,
    FourLoopNextLeaf, FourLoopNextReplayStep, FourLoopNextReplayedPath,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_next_manifest::{
    FOUR_LOOP_NEXT_MANIFEST_CORNER_SEEDS, FOUR_LOOP_NEXT_MANIFEST_DOT_SEEDS,
    FOUR_LOOP_NEXT_MANIFEST_MIXED_SEEDS, FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES,
    FOUR_LOOP_NEXT_MANIFEST_NUMERATOR_SEEDS, FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS,
    FOUR_LOOP_NEXT_MANIFEST_RAW_TERM_INCIDENCE_BOUND, FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM,
    FOUR_LOOP_NEXT_MANIFEST_SEEDS, FourLoopNextManifest, FourLoopNextManifestConfig,
    FourLoopNextManifestError, FourLoopNextManifestStatus, FourLoopNextRawRowId,
    FourLoopNextRawRowIdError, FourLoopNextSeedId, FourLoopNextSeedPhase,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_next_modular_rank::{
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_CANCELLATIONS, FOUR_LOOP_NEXT_MODULAR_DISCOVERY_CHECKSUM,
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_CLEARED_PIVOTS,
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_COLUMN_CATALOG_CHECKSUM,
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_DEPENDENT_ROWS,
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_FIELD_WORK_UNITS, FOUR_LOOP_NEXT_MODULAR_DISCOVERY_FILL_IN,
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_FREE_COLUMNS, FOUR_LOOP_NEXT_MODULAR_DISCOVERY_IMAGES,
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_MATRIX_CHECKSUMS,
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_PEAK_LIVE_NONZEROS,
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_PEAK_ROW_NONZEROS,
    FOUR_LOOP_NEXT_MODULAR_DISCOVERY_PIVOT_CHECKSUMS, FOUR_LOOP_NEXT_MODULAR_DISCOVERY_RANK,
    FourLoopNextModularFillStats, FourLoopNextModularImage, FourLoopNextModularImageReport,
    FourLoopNextModularPivot, FourLoopNextModularRankConfig, FourLoopNextModularRankError,
    FourLoopNextModularRankReport, FourLoopNextModularRankStatus,
    discover_four_loop_next_modular_rank, discover_four_loop_next_modular_rank_at_images,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_polynomial_halo::{
    FOUR_LOOP_POLYNOMIAL_HALO_COLLECTED_MONOMIALS, FOUR_LOOP_POLYNOMIAL_HALO_CONVOLUTION_PRODUCTS,
    FOUR_LOOP_POLYNOMIAL_HALO_FACTOR_TERMS,
    FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_CONVOLUTION_PRODUCT_BOUND,
    FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_OUTPUT_BRANCH_BOUND,
    FOUR_LOOP_POLYNOMIAL_HALO_MANIFEST_ROW_RAW_COLLECTED_TERM_BOUND,
    FOUR_LOOP_POLYNOMIAL_HALO_NUMERATOR_FACTORS, FOUR_LOOP_POLYNOMIAL_HALO_OUTPUT_BRANCHES,
    FourLoopPolynomialBranch, FourLoopPolynomialBranchKind, FourLoopPolynomialHaloConfig,
    FourLoopPolynomialHaloError, FourLoopPolynomialHaloMapper, FourLoopPolynomialHaloStats,
    FourLoopPolynomialMapWitness, FourLoopPolynomialMonomial, FourLoopPolynomialRawRowMap,
    FourLoopPolynomialRawRowStats, FourLoopPolynomialRawTermMap,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_t1s2_closure::{
    FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_DEGREE, FOUR_LOOP_T1S2_CLOSURE_COEFFICIENT_OPERATIONS,
    FOUR_LOOP_T1S2_CLOSURE_COLLECTED_TERMS, FOUR_LOOP_T1S2_CLOSURE_COMPONENT_CALLS,
    FOUR_LOOP_T1S2_CLOSURE_COMPONENTS, FOUR_LOOP_T1S2_CLOSURE_CONVOLUTION_PAIRS,
    FOUR_LOOP_T1S2_CLOSURE_LOCAL_SLOTS, FOUR_LOOP_T1S2_CLOSURE_MASS_POWER_STEPS,
    FOUR_LOOP_T1S2_CLOSURE_OCCURRENCES, FOUR_LOOP_T1S2_CLOSURE_OPEN_PLANS,
    FOUR_LOOP_T1S2_CLOSURE_PLANS, FOUR_LOOP_T1S2_CLOSURE_PRECOLLECTION_TERMS,
    FOUR_LOOP_T1S2_CLOSURE_RETAINED_COEFFICIENT_BYTES, FOUR_LOOP_T1S2_CLOSURE_SCALAR_BRANCHES,
    FOUR_LOOP_T1S2_CLOSURE_UNIQUE_TARGETS, FourLoopT1S2BranchClosure, FourLoopT1S2Closure,
    FourLoopT1S2ClosureConfig, FourLoopT1S2ClosureError, FourLoopT1S2ClosureOccurrence,
    FourLoopT1S2ClosureStats, FourLoopT1S2ClosureStatus, FourLoopT1S2ComponentUse,
    FourLoopT1S2LocalReduction, FourLoopT1S2LocalTarget, FourLoopT1S2ParentStatus,
    FourLoopT1S2PlanClosure, FourLoopT1S2ProductClass,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_three_loop_closure::{
    FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_DEGREE,
    FOUR_LOOP_THREE_LOOP_CLOSURE_COEFFICIENT_OPERATION_BOUND,
    FOUR_LOOP_THREE_LOOP_CLOSURE_COLLECTED_TERM_BOUND,
    FOUR_LOOP_THREE_LOOP_CLOSURE_COMPLETED_OCCURRENCES,
    FOUR_LOOP_THREE_LOOP_CLOSURE_COMPLETED_ROWS, FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENT_CALLS,
    FOUR_LOOP_THREE_LOOP_CLOSURE_COMPONENTS, FOUR_LOOP_THREE_LOOP_CLOSURE_CONVOLUTION_PAIR_BOUND,
    FOUR_LOOP_THREE_LOOP_CLOSURE_LOCAL_SLOTS, FOUR_LOOP_THREE_LOOP_CLOSURE_MASS_POWER_STEP_BOUND,
    FOUR_LOOP_THREE_LOOP_CLOSURE_MIXED_ROWS, FOUR_LOOP_THREE_LOOP_CLOSURE_OCCURRENCES,
    FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_OCCURRENCES, FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_PLANS,
    FOUR_LOOP_THREE_LOOP_CLOSURE_OUTSIDE_ROWS, FOUR_LOOP_THREE_LOOP_CLOSURE_PLANS,
    FOUR_LOOP_THREE_LOOP_CLOSURE_PRECOLLECTION_TERM_BOUND,
    FOUR_LOOP_THREE_LOOP_CLOSURE_RETAINED_OUTPUT_COEFFICIENT_BYTES,
    FOUR_LOOP_THREE_LOOP_CLOSURE_SCALAR_BRANCHES, FOUR_LOOP_THREE_LOOP_CLOSURE_UNIQUE_TARGETS,
    FourLoopThreeLoopBranchClosure, FourLoopThreeLoopClosure, FourLoopThreeLoopClosureConfig,
    FourLoopThreeLoopClosureError, FourLoopThreeLoopClosureOccurrence,
    FourLoopThreeLoopClosureStats, FourLoopThreeLoopClosureStatus, FourLoopThreeLoopComponentUse,
    FourLoopThreeLoopParentStatus, FourLoopThreeLoopPlanClosure, FourLoopThreeLoopProductClass,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use four_loop_three_loop_service::{
    FOUR_LOOP_THREE_LOOP_SERVICE_B4_TARGETS, FOUR_LOOP_THREE_LOOP_SERVICE_DEGREE_CENSUS,
    FOUR_LOOP_THREE_LOOP_SERVICE_F5_TARGETS, FOUR_LOOP_THREE_LOOP_SERVICE_M6_TARGETS,
    FOUR_LOOP_THREE_LOOP_SERVICE_NATIVE_IDENTITIES, FOUR_LOOP_THREE_LOOP_SERVICE_OUTPUT_TERM_BOUND,
    FOUR_LOOP_THREE_LOOP_SERVICE_RETAINED_OUTPUT_COEFFICIENT_BYTE_BOUND,
    FOUR_LOOP_THREE_LOOP_SERVICE_T1_TARGETS, FOUR_LOOP_THREE_LOOP_SERVICE_TARGET_MANIFEST_CHECKSUM,
    FOUR_LOOP_THREE_LOOP_SERVICE_TARGETS, FourLoopThreeLoopLocalReduction,
    FourLoopThreeLoopLocalTarget, FourLoopThreeLoopService, FourLoopThreeLoopServiceConfig,
    FourLoopThreeLoopServiceError, FourLoopThreeLoopServiceStats, FourLoopThreeLoopServiceStatus,
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
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PARAMETRIC_REDUCTION_V1_SCHEMA,
    AuthenticatedVacuumCovariantTensorPolynomialLowering,
    AuthenticatedVacuumCovariantTensorPolynomialParametricReduction,
    AuthenticatedVacuumCovariantTensorPolynomialProjection, CovariantTensorPolynomialMonomial,
    GENERIC_VACUUM_COVARIANT_TENSOR_POLYNOMIAL_PROJECTION_V1_SCHEMA, GenericTensorPolynomialError,
    GenericTensorPolynomialLimits, GenericTensorPolynomialStats,
    GenericVacuumTensorPolynomialProjector, TensorPolynomialProjectionContribution,
    TensorPolynomialProjectionOrigin, TensorPolynomialReductionEngineError,
    TensorPolynomialWeightGuardOrigin, TensorPolynomialWeightNonZeroCondition,
    WeightedCovariantTensorMonomial,
};
pub use generic_tensor_projector::{
    AUTHENTICATED_VACUUM_TENSOR_LOWERING_V1_SCHEMA, AuthenticatedVacuumCovariantTensorProjection,
    AuthenticatedVacuumTensorLowering, AuthenticatedVacuumTensorProjection,
    CovariantTensorMonomial, GENERIC_VACUUM_COVARIANT_TENSOR_PROJECTION_V1_SCHEMA,
    GENERIC_VACUUM_TENSOR_PROJECTION_V1_SCHEMA, GenericCovariantTensorNumerator,
    GenericCovariantTensorTerm, GenericTensorProjectionDomain, GenericTensorProjectionStats,
    GenericTensorProjectorError, GenericTensorProjectorLimits, GenericVacuumTensorProjector,
    IndexedSpectatorVector, SpectatorScalarProduct, SpectatorScalarProductMonomial,
    SpectatorVector, TensorCovariantStructure, TensorLoopReference, TensorProjectionGuardOrigin,
    TensorProjectionNonZeroCondition, VacuumCovariantPrecontractionWitness,
    VacuumCovariantTensorProjectionWitness, VacuumCovariantVectorContractionWitness,
    VacuumMetricContractionWitness, VacuumTensorProjectionWitness,
};
pub use guards::{CoefficientLocation, GuardOrigin, GuardRowId};
pub use ibp::{IbpGenerationError, IbpGenerator, IbpIdentity};
pub use integral::Integral;
pub use linear::LinearCombination;
pub use master_policy::{
    MasterPolicyError, MasterPolicyLimits, MasterPolicyProvider, MasterPolicyTerminal,
};
pub use master_product::{
    MasterProduct, MasterProductError, ProductConvolutionError, ProductLinearCombination,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use one_loop::{
    OneLoopTadpoleConfig, OneLoopTadpoleError, OneLoopTadpoleReducer, OneLoopTadpoleReduction,
    OneLoopTadpoleStats,
};
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
#[cfg(feature = "legacy-authored-oracles")]
pub use product_boundary::{ProductBoundaryConfig, ProductBoundaryError, ProductBoundaryReducer};
pub use product_locus_boolean_cover::{
    RESIDUAL_PRODUCT_LOCUS_BOOLEAN_COVER_V1_SCHEMA, ResidualProductLocusBooleanCoverCertificate,
    ResidualProductLocusBooleanCoverCompiler, ResidualProductLocusBooleanCoverError,
    ResidualProductLocusBooleanCoverLimits, ResidualProductLocusBooleanCoverStats,
    ResidualProductLocusBooleanDecision, ResidualProductLocusBooleanEmptyReason,
    ResidualProductLocusBooleanNode, ResidualProductLocusBooleanNodeOutcome,
    ResidualProductLocusBooleanPolarity,
};
pub use reduction::{
    ReductionCacheError, ReductionCacheLimits, ReductionError, ReductionStats, ReductionTable,
    SeedConfig, SeedGenerationError, SeedGenerationLimits, SparseReducer, generate_seeds,
    try_generate_seeds, try_generate_seeds_with_limits,
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
    AFFINE_FAMILY_MAP_V1_SCHEMA, AffineDenominatorMap, AffineScalarProductMap,
    DenominatorRowAction, ExactMatrix, JacobianWitness, MomentumMap, SymmetryVerificationError,
    SymmetryVerificationLimits, SymmetryVerificationStats, VerifiedAffineFamilyMap,
    verify_affine_family_map,
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
pub use tensor_family::{
    DEFAULT_MAX_TENSOR_EXPANSION_OPERATIONS, DEFAULT_MAX_TENSOR_EXPANSION_TERMS, TensorFamilyError,
    TensorFamilyReducer, TensorIntegralReduction,
};
pub use tensor_reduction_engine::{
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_LOWERING_V1_SCHEMA,
    AUTHENTICATED_VACUUM_COVARIANT_TENSOR_PARAMETRIC_REDUCTION_V1_SCHEMA,
    AUTHENTICATED_VACUUM_TENSOR_PARAMETRIC_REDUCTION_V1_SCHEMA,
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
#[cfg(feature = "legacy-authored-oracles")]
pub use three_loop::{
    THREE_LOOP_TETRAHEDRON_EDGES, THREE_LOOP_TETRAHEDRON_ROUTINGS,
    THREE_LOOP_TETRAHEDRON_SYMMETRY_GENERATORS, equal_mass_three_loop_tetrahedron,
    equal_mass_three_loop_tetrahedron_in_context, equal_mass_three_loop_vacuum,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use three_loop_b4_d2::{
    THREE_LOOP_B4_D2_BOUNDARY_CALL_BOUND, THREE_LOOP_B4_D2_COLLECTED_NONZERO_BOUND,
    THREE_LOOP_B4_D2_ELIMINATION_UPDATE_BOUND, THREE_LOOP_B4_D2_GLOBAL_COLUMN_BOUND,
    THREE_LOOP_B4_D2_RAW_ROWS, THREE_LOOP_B4_D2_RAW_TERM_INCIDENCE_BOUND,
    THREE_LOOP_B4_D2_SEED_ORBITS, THREE_LOOP_B4_D2_SOURCE_WEIGHT_BOUND,
    THREE_LOOP_B4_D2_SYMMETRY_IMAGE_BOUND, ThreeLoopB4BoundaryColumn, ThreeLoopB4D2Column,
    ThreeLoopB4D2ConditionSource, ThreeLoopB4D2Config, ThreeLoopB4D2Error,
    ThreeLoopB4D2NonzeroCondition, ThreeLoopB4D2NormalizedRow, ThreeLoopB4D2PivotRule,
    ThreeLoopB4D2RawRowId, ThreeLoopB4D2Seed, ThreeLoopB4D2SeedOrbit, ThreeLoopB4D2Shell,
    ThreeLoopB4D2Stats,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use three_loop_boundary::{
    ThreeLoopBoundaryConfig, ThreeLoopBoundaryError, ThreeLoopBoundaryReducer,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use three_loop_f5_d2n1::{
    THREE_LOOP_F5_D2N1_CANONICAL_REPRESENTATIVE_POWERS, THREE_LOOP_F5_D2N1_IBPS_PER_TARGET,
    THREE_LOOP_F5_D2N1_LABELLED_TARGET_POWERS, THREE_LOOP_F5_D2N1_NATIVE_IDENTITIES,
    THREE_LOOP_F5_D2N1_ORBITS, THREE_LOOP_F5_D2N1_STABILIZER, THREE_LOOP_F5_D2N1_TARGETS,
    ThreeLoopF5D2N1Error, ThreeLoopF5D2N1Reducer, three_loop_f5_d2n1_pipeline_config,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use three_loop_pipeline::{
    ThreeLoopPipelineError, ThreeLoopReductionConfig, ThreeLoopReductionPipeline,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use three_loop_proper_dot::{
    THREE_LOOP_B4_MASK, THREE_LOOP_F5_CENTRAL_IBP_WEIGHTS, THREE_LOOP_F5_MASK,
    THREE_LOOP_F5_OUTER_IBP_WEIGHTS, THREE_LOOP_PROPER_DOT_RAW_TERM_BOUND,
    ThreeLoopProperDotConfig, ThreeLoopProperDotError, ThreeLoopProperDotProvenance,
    ThreeLoopProperDotReducer, ThreeLoopProperDotRewrite, ThreeLoopProperDotSector,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use three_loop_top_dot::{
    THREE_LOOP_TOP_DOT_IBP_WEIGHT_NUMERATORS, ThreeLoopTopDotConfig, ThreeLoopTopDotError,
    ThreeLoopTopDotReducer, ThreeLoopTopDotRewrite,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use two_loop::{TwoLoopBoundaryConfig, TwoLoopBoundaryError, TwoLoopBoundaryReducer};
#[cfg(feature = "legacy-authored-oracles")]
pub use two_loop_pipeline::{
    TwoLoopPipelineError, TwoLoopReductionConfig, TwoLoopReductionPipeline,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use two_loop_top_dot::{
    TWO_LOOP_TOP_DOT_ACCUMULATION_OPERATIONS_PER_STATE, TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND,
    TWO_LOOP_TOP_DOT_IBP_WEIGHTS, TWO_LOOP_TOP_DOT_RAW_TERM_BOUND, TwoLoopTopDotConfig,
    TwoLoopTopDotError, TwoLoopTopDotNormalForm, TwoLoopTopDotPreflight, TwoLoopTopDotProvenance,
    TwoLoopTopDotReducer, TwoLoopTopDotRewrite, TwoLoopTopDotStats,
};
#[cfg(feature = "legacy-authored-oracles")]
pub use vakint_adapter::{
    VakintAdapterError, VakintAdapterLimits, VakintAtomSyntax, VakintTwoLoopAdapter,
    VakintTwoLoopTerm,
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
