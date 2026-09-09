use std::fmt;

/// Hard construction or resource failure of the coordinate-case worklist.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SpiredCoordinateCaseWorklistError {
    GuardedDiscoveryCase {
        guard_branches: usize,
    },
    CaseCarrierFamilyMismatch,
    CaseCarrierContextMismatch,
    CaseCarrierSectorMismatch,
    NonEqualityCaseAxis {
        position: usize,
        carrier_lower: i64,
        carrier_upper: i64,
        case_lower: i64,
        case_upper: i64,
    },
    ChildOutsideParent,
    ResourceCountOverflow {
        resource: &'static str,
    },
    ResourceLimit {
        resource: &'static str,
        requested: usize,
        limit: usize,
    },
    AllocationFailure {
        resource: &'static str,
        requested: usize,
    },
    IdentityCollision,
    ExpectedCurrentCaseAbsent,
    ExpectedCurrentCaseMismatch,
    ReplacementChildFamilyMismatch,
    ReplacementChildContextMismatch,
    ReplacementChildSectorMismatch,
    ReplacementChildCarrierMismatch,
    ReplacementChildOutsideExpectedCurrent,
    ReplacementChildDimensionNotReduced {
        parent_dimension: usize,
        child_dimension: usize,
    },
    PreparedBatchQueueMismatch,
    StalePreparedBatch {
        prepared_revision: usize,
        current_revision: usize,
    },
    PreparedReplacementQueueMismatch,
    StalePreparedReplacement {
        prepared_revision: usize,
        current_revision: usize,
    },
    Invariant {
        detail: &'static str,
    },
}

impl fmt::Display for SpiredCoordinateCaseWorklistError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::GuardedDiscoveryCase { guard_branches } => write!(
                formatter,
                "a SpIReD equality-only discovery case cannot retain {guard_branches} guard branches"
            ),
            Self::CaseCarrierFamilyMismatch => formatter.write_str(
                "a SpIReD coordinate case and its declared carrier belong to different families",
            ),
            Self::CaseCarrierContextMismatch => formatter.write_str(
                "a SpIReD coordinate case and its declared carrier use different coefficient contexts",
            ),
            Self::CaseCarrierSectorMismatch => formatter.write_str(
                "a SpIReD coordinate case and its declared carrier belong to different sectors",
            ),
            Self::NonEqualityCaseAxis {
                position,
                carrier_lower,
                carrier_upper,
                case_lower,
                case_upper,
            } => write!(
                formatter,
                "SpIReD logical case axis {position} has bounds [{case_lower}, {case_upper}], but must be either its complete declared-carrier interval [{carrier_lower}, {carrier_upper}] or one contained singleton equality"
            ),
            Self::ChildOutsideParent => formatter.write_str(
                "a SpIReD coordinate-equality child is not contained in its logical parent case",
            ),
            Self::ResourceCountOverflow { resource } => {
                write!(
                    formatter,
                    "SpIRed coordinate-case {resource} overflowed usize"
                )
            }
            Self::ResourceLimit {
                resource,
                requested,
                limit,
            } => write!(
                formatter,
                "SpIRed coordinate-case {resource} requires {requested}, exceeding the configured limit {limit}"
            ),
            Self::AllocationFailure {
                resource,
                requested,
            } => write!(
                formatter,
                "could not reserve {requested} entries for SpIRed coordinate-case {resource}"
            ),
            Self::IdentityCollision => formatter.write_str(
                "two unequal SpIRed coordinate cases carry the same exact stratum identity",
            ),
            Self::ExpectedCurrentCaseAbsent => formatter.write_str(
                "a SpIRed coordinate-case transition expected a current case, but the worklist is empty",
            ),
            Self::ExpectedCurrentCaseMismatch => formatter.write_str(
                "a SpIRed coordinate-case transition does not target the highest-priority current case",
            ),
            Self::ReplacementChildFamilyMismatch => formatter.write_str(
                "a SpIRed coordinate-case replacement child belongs to a different family",
            ),
            Self::ReplacementChildContextMismatch => formatter.write_str(
                "a SpIRed coordinate-case replacement child belongs to a different coefficient context",
            ),
            Self::ReplacementChildSectorMismatch => formatter.write_str(
                "a SpIRed coordinate-case replacement child belongs to a different sector",
            ),
            Self::ReplacementChildCarrierMismatch => formatter.write_str(
                "a SpIReD coordinate-case replacement child belongs to a different declared logical carrier",
            ),
            Self::ReplacementChildOutsideExpectedCurrent => formatter.write_str(
                "a SpIRed coordinate-case replacement child is not contained in the expected current case",
            ),
            Self::ReplacementChildDimensionNotReduced {
                parent_dimension,
                child_dimension,
            } => write!(
                formatter,
                "a SpIRed coordinate-case replacement child has free dimension {child_dimension}, which does not strictly reduce its parent dimension {parent_dimension}"
            ),
            Self::PreparedBatchQueueMismatch => formatter.write_str(
                "a prepared SpIRed coordinate-case batch belongs to a different worklist",
            ),
            Self::StalePreparedBatch {
                prepared_revision,
                current_revision,
            } => write!(
                formatter,
                "a prepared SpIRed coordinate-case batch targets worklist revision {prepared_revision}, but the current revision is {current_revision}"
            ),
            Self::PreparedReplacementQueueMismatch => formatter.write_str(
                "a prepared SpIRed coordinate-case replacement belongs to a different worklist",
            ),
            Self::StalePreparedReplacement {
                prepared_revision,
                current_revision,
            } => write!(
                formatter,
                "a prepared SpIRed coordinate-case replacement targets worklist revision {prepared_revision}, but the current revision is {current_revision}"
            ),
            Self::Invariant { detail } => {
                write!(
                    formatter,
                    "SpIRed coordinate-case invariant failed: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for SpiredCoordinateCaseWorklistError {}
