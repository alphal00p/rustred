use rustred::algebra::{ExactAlgebraError, IndexedAlgebraError};
use rustred::family::IntegralKeyError;
use rustred::foundry::artifact::{ArtifactError, ArtifactPersistenceError};
use rustred::reduction::ReductionError;
use rustred::sector;

use super::super::error::{AppError, AppErrorKind};

pub(super) fn map_artifact_load_error(error: ArtifactPersistenceError) -> AppError {
    let kind = artifact_persistence_error_kind(&error, ArtifactCodecOperation::Load);
    AppError::new(kind, format!("invalid closing artifact: {error}"))
}

pub(super) fn map_artifact_encoding_error(error: ArtifactPersistenceError) -> AppError {
    let kind = artifact_persistence_error_kind(&error, ArtifactCodecOperation::Encode);
    AppError::new(kind, format!("cannot encode closing artifact: {error}"))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ArtifactCodecOperation {
    Encode,
    Load,
}

fn artifact_persistence_error_kind(
    error: &ArtifactPersistenceError,
    operation: ArtifactCodecOperation,
) -> AppErrorKind {
    match error {
        ArtifactPersistenceError::UnsupportedSchema { .. } => AppErrorKind::Schema,
        ArtifactPersistenceError::ResourceCountOverflow { .. }
        | ArtifactPersistenceError::ResourceLimit { .. }
        | ArtifactPersistenceError::AllocationFailure { .. } => match operation {
            ArtifactCodecOperation::Encode => AppErrorKind::OutputLimit,
            ArtifactCodecOperation::Load => AppErrorKind::Limit,
        },
        ArtifactPersistenceError::Artifact(error) => match operation {
            ArtifactCodecOperation::Load => artifact_validation_error_kind(error),
            ArtifactCodecOperation::Encode => trusted_artifact_encoding_error_kind(error),
        },
        ArtifactPersistenceError::SemanticMismatch { .. } => match operation {
            ArtifactCodecOperation::Encode => AppErrorKind::InternalInvariant,
            ArtifactCodecOperation::Load => AppErrorKind::Input,
        },
        ArtifactPersistenceError::InvalidMagic
        | ArtifactPersistenceError::InvalidSection { .. }
        | ArtifactPersistenceError::Truncated { .. }
        | ArtifactPersistenceError::TrailingBytes { .. }
        | ArtifactPersistenceError::InvalidUtf8 { .. }
        | ArtifactPersistenceError::InvalidCoefficient { .. }
        | ArtifactPersistenceError::NonCanonicalCoefficient { .. }
        | ArtifactPersistenceError::UnsupportedFeature { .. } => match operation {
            ArtifactCodecOperation::Encode => AppErrorKind::Serialization,
            ArtifactCodecOperation::Load => AppErrorKind::Input,
        },
    }
}

fn trusted_artifact_encoding_error_kind(error: &ArtifactError) -> AppErrorKind {
    match artifact_validation_error_kind(error) {
        AppErrorKind::Schema => AppErrorKind::Schema,
        AppErrorKind::Limit | AppErrorKind::OutputLimit => AppErrorKind::OutputLimit,
        AppErrorKind::Execution | AppErrorKind::License => AppErrorKind::Execution,
        AppErrorKind::InternalInvariant => AppErrorKind::InternalInvariant,
        AppErrorKind::Input
        | AppErrorKind::Lowering
        | AppErrorKind::Derivation
        | AppErrorKind::Serialization => AppErrorKind::InternalInvariant,
    }
}

fn artifact_validation_error_kind(error: &ArtifactError) -> AppErrorKind {
    match error {
        ArtifactError::UnsupportedSchema { .. } => AppErrorKind::Schema,
        ArtifactError::Family(error) if integral_family_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        ArtifactError::Identity(error) if parametric_ibp_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        ArtifactError::ParametricRule(error) => parametric_rule_error_kind(error),
        ArtifactError::Relation(error) if parametric_relation_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        ArtifactError::IndexedAlgebra(error) => untrusted_indexed_algebra_error_kind(error),
        ArtifactError::IntegralKey(error) if integral_key_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        ArtifactError::Ordering(error) if sector_error_is_limit(error) => AppErrorKind::Limit,
        ArtifactError::ZeroAnalysis(error) if zero_analysis_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        ArtifactError::CoefficientContext(_)
        | ArtifactError::Family(_)
        | ArtifactError::Identity(_)
        | ArtifactError::RuleCell(_)
        | ArtifactError::Relation(_)
        | ArtifactError::TranslatedSource(_)
        | ArtifactError::Symmetry(_)
        | ArtifactError::SymmetryPermutation(_)
        | ArtifactError::Canonicalization(_)
        | ArtifactError::IntegralKey(_)
        | ArtifactError::Ordering(_)
        | ArtifactError::ZeroAnalysis(_)
        | ArtifactError::WrongFamily
        | ArtifactError::WrongCoefficientContext
        | ArtifactError::WrongArity { .. }
        | ArtifactError::InvalidMasterManifest
        | ArtifactError::InvalidZeroTerminal
        | ArtifactError::InvalidFactorization { .. }
        | ArtifactError::InvalidCanonicalizer
        | ArtifactError::UnsupportedClosureShape
        | ArtifactError::InvalidRuleShape { .. }
        | ArtifactError::InvalidDescentWitness { .. }
        | ArtifactError::UnprovedGuardApplicability { .. }
        | ArtifactError::InvalidReplayEvidence { .. } => AppErrorKind::Input,
    }
}

pub(super) fn map_reduction_error(context: &'static str, error: ReductionError) -> AppError {
    let kind = reduction_error_kind(&error);
    AppError::new(kind, format!("{context}: {error}"))
}

fn reduction_error_kind(error: &ReductionError) -> AppErrorKind {
    match error {
        ReductionError::RuleApplicationLimit { .. }
        | ReductionError::AllocationFailure { .. }
        | ReductionError::CacheLimit { .. }
        | ReductionError::CacheCoefficientTermLimit { .. }
        | ReductionError::CacheCoefficientByteLimit { .. }
        | ReductionError::CacheResourceCountOverflow { .. }
        | ReductionError::PendingFrameLimit { .. }
        | ReductionError::IndexOverflow { .. }
        | ReductionError::CommonMassPowerOverflow => AppErrorKind::Limit,
        ReductionError::IntegralKey(error) if integral_key_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        ReductionError::IndexedAlgebra(error) if indexed_algebra_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        ReductionError::ExactAlgebra(error) if exact_algebra_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        ReductionError::Ordering(error) if sector_error_is_limit(error) => AppErrorKind::Limit,
        ReductionError::WrongArity { .. }
        | ReductionError::OutsideCertifiedRootDomain { .. }
        | ReductionError::ZeroCommonMass
        | ReductionError::IntegralKey(IntegralKeyError::EmptyPowers) => AppErrorKind::Input,
        ReductionError::UncoveredIntegral { .. }
        | ReductionError::MissingCommonMassHomogeneityProof
        | ReductionError::IndexedAlgebra(IndexedAlgebraError::Symbolica(_)) => {
            AppErrorKind::Execution
        }
        ReductionError::CycleDetected { .. }
        | ReductionError::ReducerInvariant { .. }
        | ReductionError::UnexpectedDependencyMaster { .. }
        | ReductionError::RuleCell(_)
        | ReductionError::Canonicalization(_)
        | ReductionError::IntegralKey(_)
        | ReductionError::IndexedAlgebra(_)
        | ReductionError::ExactAlgebra(_)
        | ReductionError::Ordering(_) => AppErrorKind::InternalInvariant,
    }
}

fn zero_analysis_error_is_limit(error: &rustred::sector::zero::Error) -> bool {
    matches!(
        error,
        rustred::sector::zero::Error::ResourceLimit { .. }
            | rustred::sector::zero::Error::ResourceCountOverflow { .. }
            | rustred::sector::zero::Error::AllocationFailure { .. }
            | rustred::sector::zero::Error::MatrixDimensionOverflow { .. }
    )
}

fn exact_algebra_error_is_limit(error: &ExactAlgebraError) -> bool {
    matches!(
        error,
        ExactAlgebraError::ExponentLimit { .. }
            | ExactAlgebraError::ExponentArithmeticOverflow { .. }
            | ExactAlgebraError::ResourceLimit { .. }
            | ExactAlgebraError::ResourceCountOverflow { .. }
    )
}

fn indexed_algebra_error_is_limit(error: &IndexedAlgebraError) -> bool {
    match error {
        IndexedAlgebraError::ResourceLimit { .. }
        | IndexedAlgebraError::ResourceCountOverflow { .. }
        | IndexedAlgebraError::AllocationFailure { .. } => true,
        IndexedAlgebraError::ExactAlgebra(error) => exact_algebra_error_is_limit(error),
        IndexedAlgebraError::EmptyIndexSpace
        | IndexedAlgebraError::InvalidScope
        | IndexedAlgebraError::IndexSymbolRegistrationFailure { .. }
        | IndexedAlgebraError::IndexSymbolCollision { .. }
        | IndexedAlgebraError::WrongContext
        | IndexedAlgebraError::WrongIndexArity { .. }
        | IndexedAlgebraError::FixedIndexOutOfRange { .. }
        | IndexedAlgebraError::DuplicateFixedIndex { .. }
        | IndexedAlgebraError::ZeroDenominator
        | IndexedAlgebraError::Symbolica(_) => false,
    }
}

fn untrusted_indexed_algebra_error_kind(error: &IndexedAlgebraError) -> AppErrorKind {
    if indexed_algebra_error_is_limit(error) {
        return AppErrorKind::Limit;
    }
    match error {
        IndexedAlgebraError::IndexSymbolRegistrationFailure { .. }
        | IndexedAlgebraError::IndexSymbolCollision { .. } => AppErrorKind::InternalInvariant,
        IndexedAlgebraError::Symbolica(_) => AppErrorKind::Execution,
        IndexedAlgebraError::EmptyIndexSpace
        | IndexedAlgebraError::InvalidScope
        | IndexedAlgebraError::WrongContext
        | IndexedAlgebraError::WrongIndexArity { .. }
        | IndexedAlgebraError::FixedIndexOutOfRange { .. }
        | IndexedAlgebraError::DuplicateFixedIndex { .. }
        | IndexedAlgebraError::ZeroDenominator
        | IndexedAlgebraError::ExactAlgebra(_)
        | IndexedAlgebraError::ResourceLimit { .. }
        | IndexedAlgebraError::ResourceCountOverflow { .. }
        | IndexedAlgebraError::AllocationFailure { .. } => AppErrorKind::Input,
    }
}

fn integral_key_error_is_limit(error: &IntegralKeyError) -> bool {
    matches!(
        error,
        IntegralKeyError::PowerCountOverflow | IntegralKeyError::AllocationFailure { .. }
    )
}

fn sector_error_is_limit(error: &sector::Error) -> bool {
    matches!(
        error,
        sector::Error::AllocationFailure { .. } | sector::Error::ComplexityOverflow { .. }
    )
}

fn integral_family_error_is_limit(error: &rustred::family::IntegralFamilyError) -> bool {
    use rustred::family::IntegralFamilyError;

    match error {
        IntegralFamilyError::ScalarProductCountOverflow { .. }
        | IntegralFamilyError::ResourceLimit { .. }
        | IntegralFamilyError::ResourceCountOverflow { .. }
        | IntegralFamilyError::AllocationFailure { .. }
        | IntegralFamilyError::MatrixDimensionOverflow { .. } => true,
        IntegralFamilyError::InvalidCoefficient { error, .. }
        | IntegralFamilyError::ExactAlgebra(error) => exact_algebra_error_is_limit(error),
        IntegralFamilyError::NoLoopMomenta
        | IntegralFamilyError::EmptyMomentumLabel { .. }
        | IntegralFamilyError::DuplicateMomentumLabel { .. }
        | IntegralFamilyError::MomentumLabelOverlap { .. }
        | IntegralFamilyError::WrongDenominatorCount { .. }
        | IntegralFamilyError::WrongDenominatorRowSize { .. }
        | IntegralFamilyError::WrongPowerShiftCount { .. }
        | IntegralFamilyError::WrongExternalGramRowCount { .. }
        | IntegralFamilyError::WrongExternalGramColumnCount { .. }
        | IntegralFamilyError::AsymmetricExternalGram { .. }
        | IntegralFamilyError::ForeignCoefficientContext { .. }
        | IntegralFamilyError::SingularDenominatorBasis
        | IntegralFamilyError::LoopMomentumOutOfRange { .. }
        | IntegralFamilyError::ExternalMomentumOutOfRange { .. }
        | IntegralFamilyError::ScalarProductOutOfRange { .. }
        | IntegralFamilyError::DenominatorOutOfRange { .. }
        | IntegralFamilyError::InternalVerificationFailure { .. } => false,
    }
}

fn identity_condition_error_is_limit(error: &rustred::identity::IdentityConditionError) -> bool {
    use rustred::identity::IdentityConditionError;

    match error {
        IdentityConditionError::ResourceLimit { .. }
        | IdentityConditionError::ResourceCountOverflow { .. } => true,
        IdentityConditionError::Coefficient(error) => indexed_algebra_error_is_limit(error),
        IdentityConditionError::MissingSource => false,
    }
}

fn parametric_relation_error_is_limit(error: &rustred::identity::ParametricRelationError) -> bool {
    use rustred::identity::ParametricRelationError;

    match error {
        ParametricRelationError::ResourceCountOverflow { .. }
        | ParametricRelationError::AllocationFailure { .. } => true,
        ParametricRelationError::IdentityCondition(error) => {
            identity_condition_error_is_limit(error)
        }
        ParametricRelationError::Coefficient(error) => indexed_algebra_error_is_limit(error),
        ParametricRelationError::EmptyIndexSpace
        | ParametricRelationError::WrongArity { .. }
        | ParametricRelationError::IndexOutOfRange { .. }
        | ParametricRelationError::IndexOverflow { .. }
        | ParametricRelationError::WrongContext
        | ParametricRelationError::WrongFamily
        | ParametricRelationError::UnsatisfiableDomain => false,
    }
}

fn parametric_ibp_error_is_limit(error: &rustred::identity::ParametricIbpError) -> bool {
    use rustred::identity::ParametricIbpError;

    match error {
        ParametricIbpError::AllocationFailure { .. }
        | ParametricIbpError::RowCountOverflow { .. } => true,
        ParametricIbpError::IdentityCondition(error) => identity_condition_error_is_limit(error),
        ParametricIbpError::Coefficient(error) => indexed_algebra_error_is_limit(error),
        ParametricIbpError::Relation(error) => parametric_relation_error_is_limit(error),
        ParametricIbpError::Family(error) => integral_family_error_is_limit(error),
        ParametricIbpError::RowOrdinalOutOfRange { .. }
        | ParametricIbpError::WrongSourceRowCount { .. }
        | ParametricIbpError::SourceRowLayoutMismatch { .. }
        | ParametricIbpError::SourceRowScopeMismatch { .. }
        | ParametricIbpError::SourceRowOrdinalMismatch { .. }
        | ParametricIbpError::CompletedSourceScopeMismatch => false,
    }
}

fn anchored_rule_error_kind(error: &rustred::foundry::anchored::AnchoredRuleError) -> AppErrorKind {
    use rustred::foundry::anchored::AnchoredRuleError;

    match error {
        AnchoredRuleError::ResourceCountOverflow { .. }
        | AnchoredRuleError::ResourceLimit { .. }
        | AnchoredRuleError::AllocationFailure { .. } => AppErrorKind::Limit,
        AnchoredRuleError::IndexedAlgebra(error) => untrusted_indexed_algebra_error_kind(error),
        AnchoredRuleError::ExactAlgebra(error) if exact_algebra_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        AnchoredRuleError::IntegralKey(error) if integral_key_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        AnchoredRuleError::Ordering(error) if sector_error_is_limit(error) => AppErrorKind::Limit,
        AnchoredRuleError::NativePanic { .. }
        | AnchoredRuleError::ReducerRejectedChronologicalRow { .. } => AppErrorKind::Execution,
        AnchoredRuleError::ReducerInvariant { .. } => AppErrorKind::InternalInvariant,
        AnchoredRuleError::EmptySourceRows
        | AnchoredRuleError::WrongAnchorArity { .. }
        | AnchoredRuleError::WrongTargetIntegralArity { .. }
        | AnchoredRuleError::TargetIntegralAbsent
        | AnchoredRuleError::TargetIntegralNotPivot
        | AnchoredRuleError::TargetHasNoStrictlyDescendingRule
        | AnchoredRuleError::TargetBackSubstitutionUsesProvenancePivot { .. }
        | AnchoredRuleError::WrongSourceContext { .. }
        | AnchoredRuleError::WrongSourceFamily { .. }
        | AnchoredRuleError::AnchorIndexOverflow { .. }
        | AnchoredRuleError::UnsatisfiedSourceCondition { .. }
        | AnchoredRuleError::NoStrictlyDescendingRule
        | AnchoredRuleError::ReplayMismatch { .. }
        | AnchoredRuleError::ExactAlgebra(_)
        | AnchoredRuleError::IntegralKey(_)
        | AnchoredRuleError::Ordering(_) => AppErrorKind::Input,
    }
}

fn parametric_rule_error_kind(
    error: &rustred::foundry::parametric::ParametricRuleError,
) -> AppErrorKind {
    use rustred::foundry::parametric::ParametricRuleError;

    match error {
        ParametricRuleError::ResourceCountOverflow { .. }
        | ParametricRuleError::ResourceLimit { .. }
        | ParametricRuleError::AllocationFailure { .. } => AppErrorKind::Limit,
        ParametricRuleError::IndexedAlgebra(error) => untrusted_indexed_algebra_error_kind(error),
        ParametricRuleError::IntegralKey(error) if integral_key_error_is_limit(error) => {
            AppErrorKind::Limit
        }
        ParametricRuleError::Ordering(error) if sector_error_is_limit(error) => AppErrorKind::Limit,
        ParametricRuleError::Anchored(error) => anchored_rule_error_kind(error),
        ParametricRuleError::NativePanic { .. }
        | ParametricRuleError::ReducerRejectedChronologicalRow { .. } => AppErrorKind::Execution,
        ParametricRuleError::ReducerInvariant { .. } => AppErrorKind::InternalInvariant,
        ParametricRuleError::EmptySourceRows
        | ParametricRuleError::WrongAnchorArity { .. }
        | ParametricRuleError::WrongTargetShiftArity { .. }
        | ParametricRuleError::TargetShiftAbsent
        | ParametricRuleError::TargetShiftNotPivot
        | ParametricRuleError::TargetHasNoUniformlyDescendingRule
        | ParametricRuleError::TargetBackSubstitutionUsesProvenancePivot { .. }
        | ParametricRuleError::WrongSourceContext { .. }
        | ParametricRuleError::WrongSourceFamily { .. }
        | ParametricRuleError::IdenticallyZeroSourceCondition { .. }
        | ParametricRuleError::AnchorOutsideInterior
        | ParametricRuleError::DegenerateSinglePointInterior
        | ParametricRuleError::ActivationLeakRequiresRefinement { .. }
        | ParametricRuleError::SectorMonotoneTermNotDescending { .. }
        | ParametricRuleError::PointOutsideSectorMonotoneDomain
        | ParametricRuleError::AnchorIndexOverflow { .. }
        | ParametricRuleError::NoStrictlyDescendingRule
        | ParametricRuleError::ReplayMismatch { .. }
        | ParametricRuleError::GuardVanishedAtAnchor { .. }
        | ParametricRuleError::AnchorPivotMismatch
        | ParametricRuleError::AnchorRightHandSideMismatch
        | ParametricRuleError::AnchorSourceCombinationMismatch
        | ParametricRuleError::IntegralKey(_)
        | ParametricRuleError::Ordering(_) => AppErrorKind::Input,
    }
}

#[cfg(test)]
mod tests {
    use rustred::family::IntegralKey;
    use rustred::foundry::anchored::AnchoredRuleError;
    use rustred::foundry::parametric::ParametricRuleError;

    use super::*;

    #[test]
    fn durable_encoder_failures_keep_schema_output_and_serialization_categories() {
        assert_eq!(
            map_artifact_encoding_error(ArtifactPersistenceError::UnsupportedSchema { actual: 2 })
                .kind(),
            AppErrorKind::Schema
        );
        let resource_errors = [
            ArtifactPersistenceError::ResourceCountOverflow {
                resource: "encoded artifact bytes",
            },
            ArtifactPersistenceError::ResourceLimit {
                resource: "encoded artifact bytes",
                requested: 2,
                limit: 1,
            },
            ArtifactPersistenceError::AllocationFailure {
                resource: "encoded artifact bytes",
                requested: 2,
            },
        ];
        for error in resource_errors {
            assert_eq!(
                map_artifact_encoding_error(error).kind(),
                AppErrorKind::OutputLimit
            );
        }
        assert_eq!(
            map_artifact_encoding_error(ArtifactPersistenceError::InvalidCoefficient {
                field: "source coefficient",
            })
            .kind(),
            AppErrorKind::Serialization
        );
        assert_eq!(
            map_artifact_encoding_error(ArtifactPersistenceError::SemanticMismatch {
                field: "sealed artifact",
            })
            .kind(),
            AppErrorKind::InternalInvariant
        );
    }

    #[test]
    fn durable_loader_schema_and_resource_failures_have_stable_categories() {
        assert_eq!(
            map_artifact_load_error(ArtifactPersistenceError::UnsupportedSchema { actual: 2 })
                .kind(),
            AppErrorKind::Schema
        );
        let resource_errors = [
            ArtifactPersistenceError::ResourceCountOverflow {
                resource: "artifact arity",
            },
            ArtifactPersistenceError::ResourceLimit {
                resource: "artifact arity",
                requested: 4_097,
                limit: 4_096,
            },
            ArtifactPersistenceError::AllocationFailure {
                resource: "artifact arity",
                requested: 4_097,
            },
        ];
        for error in resource_errors {
            assert_eq!(map_artifact_load_error(error).kind(), AppErrorKind::Limit);
        }
    }

    #[test]
    fn every_direct_reducer_resource_ceiling_maps_to_limit() {
        let resource_errors = [
            ReductionError::RuleApplicationLimit {
                requested: 2,
                limit: 1,
            },
            ReductionError::AllocationFailure {
                resource: "pending reduction frames",
                requested: 2,
            },
            ReductionError::CacheLimit {
                requested: 2,
                limit: 1,
            },
            ReductionError::CacheCoefficientTermLimit {
                requested: 2,
                limit: 1,
            },
            ReductionError::CacheCoefficientByteLimit {
                requested: 2,
                limit: 1,
            },
            ReductionError::CacheResourceCountOverflow {
                resource: "coefficient terms",
            },
            ReductionError::PendingFrameLimit {
                requested: 2,
                limit: 1,
            },
        ];
        for error in resource_errors {
            assert_eq!(reduction_error_kind(&error), AppErrorKind::Limit);
        }
    }

    #[test]
    fn reducer_invariants_remain_distinct_from_application_failures() {
        assert_eq!(
            reduction_error_kind(&ReductionError::ReducerInvariant {
                detail: "test invariant",
            }),
            AppErrorKind::InternalInvariant
        );
        assert_eq!(
            reduction_error_kind(&ReductionError::UncoveredIntegral {
                target: IntegralKey::try_new([2]).unwrap(),
            }),
            AppErrorKind::Execution
        );
    }

    #[test]
    fn anchored_parametric_failures_are_classified_recursively() {
        let category = |error| parametric_rule_error_kind(&ParametricRuleError::Anchored(error));
        assert_eq!(
            category(AnchoredRuleError::NativePanic {
                operation: "test operation",
            }),
            AppErrorKind::Execution
        );
        assert_eq!(
            category(AnchoredRuleError::ReducerRejectedChronologicalRow { source_ordinal: 0 }),
            AppErrorKind::Execution
        );
        assert_eq!(
            category(AnchoredRuleError::ReducerInvariant {
                detail: "test invariant",
            }),
            AppErrorKind::InternalInvariant
        );
        assert_eq!(
            category(AnchoredRuleError::ResourceLimit {
                resource: "anchored rows",
                requested: 2,
                limit: 1,
            }),
            AppErrorKind::Limit
        );
    }
}
