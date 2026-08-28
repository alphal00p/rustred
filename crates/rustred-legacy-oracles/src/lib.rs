//! Historical authored reduction and topology oracles for RustRed.
//!
//! This publish-disabled crate preserves finite lower-loop certificates,
//! topology fixtures, and differential adapters used to validate the generic
//! [`rustred`] engine. It is deliberately absent from the workspace default
//! members and must never become a dependency of production RustRed code.

mod concrete_engine;
pub mod families;
pub mod five_loop;
pub mod five_loop_boundary;
pub mod five_loop_d2;
pub mod five_loop_d3;
pub mod four_loop;
pub mod four_loop_boundary;
pub mod four_loop_boundary_halo;
pub mod four_loop_component_transport;
pub mod four_loop_corner_shell;
pub mod four_loop_genuine;
pub mod four_loop_halo;
pub mod four_loop_next_closed_rows;
pub mod four_loop_next_corner_cross_auth;
pub mod four_loop_next_elimination;
pub mod four_loop_next_inventory;
pub mod four_loop_next_manifest;
pub mod four_loop_next_modular_rank;
pub mod four_loop_polynomial_halo;
pub mod four_loop_t1s2_closure;
pub mod four_loop_three_loop_closure;
pub mod four_loop_three_loop_service;
pub mod one_loop;
pub mod product_boundary;
pub mod three_loop;
pub mod three_loop_b4_d2;
pub mod three_loop_boundary;
pub mod three_loop_f5_d2n1;
pub mod three_loop_pipeline;
pub mod three_loop_proper_dot;
pub mod three_loop_top_dot;
pub mod two_loop;
pub mod two_loop_pipeline;
pub mod two_loop_top_dot;
pub mod vakint_adapter;

pub use concrete_engine::{
    DEFAULT_MAX_TENSOR_EXPANSION_OPERATIONS, DEFAULT_MAX_TENSOR_EXPANSION_TERMS, Denominator,
    FamilyConstructionLimits, FamilyError, IbpGenerationError, IbpGenerator, IbpIdentity, Integral,
    LinearCombination, PropagatorSign, ReductionCacheError, ReductionCacheLimits, ReductionError,
    ReductionStats, ReductionTable, ScalarProductExpansion, SeedConfig, SeedGenerationError,
    SeedGenerationLimits, SparseReducer, TensorFamilyError, TensorFamilyReducer,
    TensorIntegralReduction, VacuumFamily, generate_seeds, try_generate_seeds,
    try_generate_seeds_with_limits,
};

pub use families::{
    equal_mass_two_loop_vacuum, equal_mass_two_loop_vacuum_in_context,
    equal_mass_two_loop_vacuum_reversed,
};
pub use five_loop::{FIVE_LOOP_BANANA_ROUTINGS, equal_mass_five_loop_banana};
pub use five_loop_boundary::{
    FIVE_LOOP_BANANA_AUXILIARIES, FIVE_LOOP_BANANA_AUXILIARY_LINE_PAIRS,
    FIVE_LOOP_BANANA_DENOMINATORS, FIVE_LOOP_BANANA_LOOP_MOMENTA, FIVE_LOOP_BANANA_PHYSICAL_LINES,
    FIVE_LOOP_BANANA_S6_ADJACENT_TRANSPOSITIONS, FIVE_LOOP_BANANA_S6_ORDER,
    FiveLoopBananaBoundaryConfig, FiveLoopBananaBoundaryError, FiveLoopBananaBoundaryReducer,
    FiveLoopBananaOrbitWitness, FiveLoopBananaPermutationError, FiveLoopBananaPhysicalPermutation,
    FiveLoopBananaProductNumeratorWitness, FiveLoopBananaScalarClass,
    five_loop_banana_oriented_line_routing, five_loop_banana_physical_orbit_witness,
};
pub use five_loop_d2::{FiveLoopBananaD2Config, FiveLoopBananaD2Error, FiveLoopBananaD2Reducer};
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
pub use four_loop::{
    FOUR_LOOP_BMW_ROUTINGS, FOUR_LOOP_FG_ROUTINGS, FOUR_LOOP_H_ROUTINGS, FOUR_LOOP_X_ROUTINGS,
    FourLoopTopology, equal_mass_four_loop_bmw, equal_mass_four_loop_fg, equal_mass_four_loop_h,
    equal_mass_four_loop_vacuum, equal_mass_four_loop_x,
};
pub use four_loop_boundary::{
    FourLoopBoundaryConfig, FourLoopBoundaryError, FourLoopBoundaryReducer,
    FourLoopComponentWitness, FourLoopFactorizationWitness, FourLoopLineCoordinate,
    FourLoopScalarClass, FourLoopSignedLineMatch, MassiveVacuumMaster,
};
pub use four_loop_boundary_halo::{
    FOUR_LOOP_BOUNDARY_HALO_BLOCKER_OCCURRENCES, FOUR_LOOP_BOUNDARY_HALO_FORMULA_DISPATCHES,
    FOUR_LOOP_BOUNDARY_HALO_OUTPUT_PRODUCTS, FOUR_LOOP_BOUNDARY_HALO_PRECOLLECTION_TERMS,
    FOUR_LOOP_BOUNDARY_HALO_PRODUCT_MULTIPLICATIONS,
    FOUR_LOOP_BOUNDARY_HALO_SIGNED_LINE_DISPATCHES, FOUR_LOOP_BOUNDARY_HALO_UNIQUE_WITNESS_PLANS,
    FourLoopBoundaryHaloConfig, FourLoopBoundaryHaloError, FourLoopBoundaryHaloPlan,
    FourLoopBoundaryHaloReducer, FourLoopBoundaryHaloReduction, FourLoopBoundaryHaloStats,
};
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
pub use four_loop_genuine::{
    FourLoopGenuineClass, FourLoopGenuineClassifier, FourLoopGenuineConfig,
    FourLoopGenuineCornerType, FourLoopGenuineError, FourLoopGenuineLineMatch,
    FourLoopGenuineWitness,
};
pub use four_loop_halo::{
    FOUR_LOOP_AFFINE_MAP_OPERATION_BOUND, FourLoopAffineDenominatorImage, FourLoopHaloColumnKey,
    FourLoopHaloConfig, FourLoopHaloError, FourLoopHaloMapper,
};
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
pub use four_loop_next_manifest::{
    FOUR_LOOP_NEXT_MANIFEST_CORNER_SEEDS, FOUR_LOOP_NEXT_MANIFEST_DOT_SEEDS,
    FOUR_LOOP_NEXT_MANIFEST_MIXED_SEEDS, FOUR_LOOP_NEXT_MANIFEST_NONZERO_SEED_ENTRIES,
    FOUR_LOOP_NEXT_MANIFEST_NUMERATOR_SEEDS, FOUR_LOOP_NEXT_MANIFEST_RAW_ROWS,
    FOUR_LOOP_NEXT_MANIFEST_RAW_TERM_INCIDENCE_BOUND, FOUR_LOOP_NEXT_MANIFEST_SEED_CHECKSUM,
    FOUR_LOOP_NEXT_MANIFEST_SEEDS, FourLoopNextManifest, FourLoopNextManifestConfig,
    FourLoopNextManifestError, FourLoopNextManifestStatus, FourLoopNextRawRowId,
    FourLoopNextRawRowIdError, FourLoopNextSeedId, FourLoopNextSeedPhase,
};
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
pub use one_loop::{
    OneLoopTadpoleConfig, OneLoopTadpoleError, OneLoopTadpoleReducer, OneLoopTadpoleReduction,
    OneLoopTadpoleStats,
};
pub use product_boundary::{ProductBoundaryConfig, ProductBoundaryError, ProductBoundaryReducer};
pub use three_loop::{
    THREE_LOOP_TETRAHEDRON_EDGES, THREE_LOOP_TETRAHEDRON_ROUTINGS,
    THREE_LOOP_TETRAHEDRON_SYMMETRY_GENERATORS, equal_mass_three_loop_tetrahedron,
    equal_mass_three_loop_tetrahedron_in_context, equal_mass_three_loop_vacuum,
};
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
pub use three_loop_boundary::{
    ThreeLoopBoundaryConfig, ThreeLoopBoundaryError, ThreeLoopBoundaryReducer,
};
pub use three_loop_f5_d2n1::{
    THREE_LOOP_F5_D2N1_CANONICAL_REPRESENTATIVE_POWERS, THREE_LOOP_F5_D2N1_IBPS_PER_TARGET,
    THREE_LOOP_F5_D2N1_LABELLED_TARGET_POWERS, THREE_LOOP_F5_D2N1_NATIVE_IDENTITIES,
    THREE_LOOP_F5_D2N1_ORBITS, THREE_LOOP_F5_D2N1_STABILIZER, THREE_LOOP_F5_D2N1_TARGETS,
    ThreeLoopF5D2N1Error, ThreeLoopF5D2N1Reducer, three_loop_f5_d2n1_pipeline_config,
};
pub use three_loop_pipeline::{
    ThreeLoopPipelineError, ThreeLoopReductionConfig, ThreeLoopReductionPipeline,
};
pub use three_loop_proper_dot::{
    THREE_LOOP_B4_MASK, THREE_LOOP_F5_CENTRAL_IBP_WEIGHTS, THREE_LOOP_F5_MASK,
    THREE_LOOP_F5_OUTER_IBP_WEIGHTS, THREE_LOOP_PROPER_DOT_RAW_TERM_BOUND,
    ThreeLoopProperDotConfig, ThreeLoopProperDotError, ThreeLoopProperDotProvenance,
    ThreeLoopProperDotReducer, ThreeLoopProperDotRewrite, ThreeLoopProperDotSector,
};
pub use three_loop_top_dot::{
    THREE_LOOP_TOP_DOT_IBP_WEIGHT_NUMERATORS, ThreeLoopTopDotConfig, ThreeLoopTopDotError,
    ThreeLoopTopDotReducer, ThreeLoopTopDotRewrite,
};
pub use two_loop::{TwoLoopBoundaryConfig, TwoLoopBoundaryError, TwoLoopBoundaryReducer};
pub use two_loop_pipeline::{
    TwoLoopPipelineError, TwoLoopReductionConfig, TwoLoopReductionPipeline,
};
pub use two_loop_top_dot::{
    TWO_LOOP_TOP_DOT_ACCUMULATION_OPERATIONS_PER_STATE, TWO_LOOP_TOP_DOT_EQUATION_TERM_BOUND,
    TWO_LOOP_TOP_DOT_IBP_WEIGHTS, TWO_LOOP_TOP_DOT_RAW_TERM_BOUND, TwoLoopTopDotConfig,
    TwoLoopTopDotError, TwoLoopTopDotNormalForm, TwoLoopTopDotPreflight, TwoLoopTopDotProvenance,
    TwoLoopTopDotReducer, TwoLoopTopDotRewrite, TwoLoopTopDotStats,
};
pub use vakint_adapter::{
    VakintAdapterError, VakintAdapterLimits, VakintAtomSyntax, VakintTwoLoopAdapter,
    VakintTwoLoopTerm,
};
